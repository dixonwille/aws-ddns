use crate::error::ResponseError;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaChaRng;
use rusoto_core::Region;
use rusoto_dynamodb::{AttributeValue, DynamoDb, DynamoDbClient, GetItemInput, PutItemInput};
use rusoto_route53::{
    Change, ChangeBatch, ChangeResourceRecordSetsRequest, ListHostedZonesRequest, ResourceRecord,
    ResourceRecordSet, Route53, Route53Client,
};
use std::{
    collections::{HashMap, HashSet},
    convert::{TryFrom, TryInto},
    env,
    net::Ipv4Addr,
};

pub struct Client {
    db: DynamoDbClient,
    dns: Route53Client,
    users_table_name: String,
}

impl Default for Client {
    fn default() -> Self {
        Client {
            db: DynamoDbClient::new(Region::default()),
            dns: Route53Client::new(Region::default()),
            users_table_name: env::var("USERS_TABLE_NAME")
                .expect("unable to find USERS_TABLE_NAME"),
        }
    }
}

impl Client {
    pub async fn get_user(&self, username: impl AsRef<str>) -> Result<User, ResponseError> {
        let mut input = GetItemInput::default();
        input.table_name = self.users_table_name.clone();
        input.key.insert(
            "username".into(),
            AttributeValue::from_string(username.as_ref().to_owned()),
        );
        match self.db.get_item(input).await {
            Ok(resp) => match resp.item {
                Some(item) => item.try_into(),
                None => Err(ResponseError::NotFound(format!(
                    "{} user",
                    username.as_ref()
                ))),
            },
            Err(e) => Err(ResponseError::DbError(format!("{}", e))),
        }
    }

    pub async fn put_user(&self, user: User) -> Result<(), ResponseError> {
        let mut input = PutItemInput::default();
        input.item = user.into();
        input.table_name = self.users_table_name.clone();
        match self.db.put_item(input).await {
            Ok(_) => Ok(()),
            Err(e) => Err(ResponseError::DbError(format!("{}", e))),
        }
    }

    pub async fn validate_user(
        &self,
        username: impl AsRef<str>,
        raw_pass: impl AsRef<str>,
        _user_agent: impl AsRef<str>,
        hostnames: &[String],
    ) -> Result<(), ResponseError> {
        let user = self.get_user(username).await?;
        if !user.compare_password(raw_pass)? {
            return Err(ResponseError::InvalidCredentials);
        }
        for host in hostnames {
            if !user.has_domain(host) {
                return Err(ResponseError::HostnameValidation(host.to_owned()));
            }
        }
        Ok(())
    }

    pub async fn update_hostnames(
        &self,
        hostnames: &[String],
        ip: &Ipv4Addr,
    ) -> Result<(), ResponseError> {
        let zones = self.list_all_hosted_zones().await?;
        let mut map: HashMap<String, Vec<(String, Ipv4Addr)>> = HashMap::new();

        for host in hostnames {
            for zone in &zones {
                if host.ends_with::<&str>(zone.0.as_ref()) {
                    match map.get(&zone.1) {
                        Some(v) => {
                            let mut v = v.clone();
                            v.push((host.clone(), ip.to_owned()));
                            map.insert(zone.1.clone(), v.clone());
                        }
                        None => {
                            map.insert(zone.1.clone(), vec![(host.clone(), ip.to_owned())]);
                        }
                    }
                    break;
                }
            }
        }
        for zone in map {
            self.update_zone_records(zone.0, zone.1).await?;
        }
        Ok(())
    }

    async fn update_zone_records(
        &self,
        zone_id: String,
        records: Vec<(String, Ipv4Addr)>,
    ) -> Result<(), ResponseError> {
        let mut req = ChangeResourceRecordSetsRequest {
            change_batch: ChangeBatch {
                comment: None,
                changes: Vec::new(),
            },
            hosted_zone_id: zone_id,
        };
        for record in records {
            req.change_batch.changes.push(Change {
                action: "UPSERT".to_owned(),
                resource_record_set: ResourceRecordSet {
                    alias_target: None,
                    failover: None,
                    geo_location: None,
                    health_check_id: None,
                    multi_value_answer: None,
                    name: record.0,
                    region: None,
                    resource_records: Some(vec![ResourceRecord {
                        value: format!("{}", record.1),
                    }]),
                    set_identifier: None,
                    ttl: Some(300),
                    traffic_policy_instance_id: None,
                    type_: "A".to_owned(),
                    weight: None,
                },
            })
        }
        if let Err(e) = self.dns.change_resource_record_sets(req).await {
            Err(ResponseError::Route53Error(format!("{}", e)))
        } else {
            Ok(())
        }
    }

    async fn list_all_hosted_zones(&self) -> Result<Vec<(String, String)>, ResponseError> {
        let (mut zones, mut next_marker) = self.list_hosted_zones(None).await?;
        while next_marker.is_some() {
            let (more_zones, new_marker) = self.list_hosted_zones(next_marker).await?;
            next_marker = new_marker;
            for zone in more_zones {
                zones.push((zone.0, zone.1));
            }
        }
        Ok(zones)
    }

    async fn list_hosted_zones(
        &self,
        marker: Option<String>,
    ) -> Result<(Vec<(String, String)>, Option<String>), ResponseError> {
        let mut map: Vec<(String, String)> = Vec::new();
        let mut req = ListHostedZonesRequest::default();
        req.marker = marker;
        let mut next_marker: Option<String> = None;
        match self.dns.list_hosted_zones(req).await {
            Ok(resp) => {
                if resp.is_truncated {
                    next_marker = resp.next_marker
                }
                for zone in resp.hosted_zones {
                    match zone.config {
                        Some(c) => match c.private_zone {
                            Some(false) | None => {
                                map.push((zone.name.trim_end_matches('.').to_owned(), zone.id));
                            }
                            _ => {}
                        },
                        None => {
                            map.push((zone.name.trim_end_matches('.').to_owned(), zone.id));
                        }
                    }
                }
            }
            Err(e) => return Err(ResponseError::Route53Error(format!("{}", e))),
        }
        Ok((map, next_marker))
    }
}

pub struct User {
    username: String,
    password: String,
    domains: HashSet<String>,
}

impl User {
    pub fn new(
        username: impl AsRef<str>,
        password: impl AsRef<str>,
        domains: HashSet<String>,
    ) -> Result<Self, ResponseError> {
        let mut user = User {
            username: username.as_ref().to_owned(),
            password: String::new(),
            domains,
        };
        user.set_password(password.as_ref().to_owned())?;
        Ok(user)
    }

    fn set_password(&mut self, pass: impl AsRef<str>) -> Result<(), ResponseError> {
        let mut config = argon2::Config::default();
        config.variant = argon2::Variant::Argon2id;
        let mut rng = ChaChaRng::from_entropy();
        let mut salt: [u8; 64] = [0; 64];
        rng.fill(&mut salt);
        let hash = argon2::hash_encoded(pass.as_ref().as_bytes(), &salt, &config)?;
        self.password = hash;
        Ok(())
    }

    fn compare_password(&self, raw_pass: impl AsRef<str>) -> Result<bool, ResponseError> {
        let verify = argon2::verify_encoded(self.password.as_ref(), raw_pass.as_ref().as_bytes())?;
        Ok(verify)
    }

    fn has_domain(&self, domain: impl AsRef<str>) -> bool {
        self.domains.contains(domain.as_ref())
    }
}

impl TryFrom<HashMap<String, AttributeValue>> for User {
    type Error = ResponseError;

    fn try_from(value: HashMap<String, AttributeValue>) -> Result<Self, Self::Error> {
        Ok(User {
            username: value.get_string_att_value("username")?,
            password: value.get_string_att_value("password")?,
            domains: value.get_string_set_att_value("domains")?,
        })
    }
}

impl Into<HashMap<String, AttributeValue>> for User {
    fn into(self) -> HashMap<String, AttributeValue> {
        let mut map = HashMap::new();
        map.insert(
            "username".to_owned(),
            AttributeValue::from_string(self.username),
        );
        map.insert(
            "password".to_owned(),
            AttributeValue::from_string(self.password),
        );
        map.insert(
            "domains".to_owned(),
            AttributeValue::from_string_set(self.domains),
        );
        map
    }
}

trait AttributeValueExt {
    type Error;
    fn get_string(&self) -> Result<String, Self::Error>;
    fn from_string(value: String) -> Self;
    fn get_string_set(&self) -> Result<HashSet<String>, Self::Error>;
    fn from_string_set(value: HashSet<String>) -> Self;
}

impl AttributeValueExt for AttributeValue {
    type Error = ResponseError;

    fn get_string(&self) -> Result<String, Self::Error> {
        match &self.s {
            Some(v) => Ok(v.to_owned()),
            None => Err(ResponseError::DbError("not of type string".into())),
        }
    }

    fn from_string(value: String) -> Self {
        let mut att = AttributeValue::default();
        att.s = Some(value);
        att
    }

    fn get_string_set(&self) -> Result<HashSet<String>, Self::Error> {
        match &self.ss {
            Some(v) => Ok(v.iter().map(|s| s.to_owned()).collect::<HashSet<String>>()),
            None => Err(ResponseError::DbError("not of type string set".into())),
        }
    }

    fn from_string_set(value: HashSet<String>) -> Self {
        let mut att = AttributeValue::default();
        att.ss = Some(value.iter().map(|s| s.to_owned()).collect());
        att
    }
}

trait MapAttributeValueExt<K: AsRef<str>> {
    type Error;
    fn get_string_att_value(&self, key: K) -> Result<String, Self::Error>;
    fn get_string_set_att_value(&self, key: K) -> Result<HashSet<String>, Self::Error>;
}

impl<K: AsRef<str>> MapAttributeValueExt<K> for HashMap<String, AttributeValue> {
    type Error = ResponseError;

    fn get_string_att_value(&self, key: K) -> Result<String, Self::Error> {
        match self.get(key.as_ref()) {
            Some(att) => att.get_string(),
            None => Err(ResponseError::DbError(format!(
                "{} not in map",
                key.as_ref()
            ))),
        }
    }

    fn get_string_set_att_value(&self, key: K) -> Result<HashSet<String>, Self::Error> {
        match self.get(key.as_ref()) {
            Some(att) => att.get_string_set(),
            None => Err(ResponseError::DbError(format!(
                "{} not in map",
                key.as_ref()
            ))),
        }
    }
}
