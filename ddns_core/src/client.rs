use crate::error::ResponseError;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaChaRng;
use rusoto_core::Region;
use rusoto_dynamodb::{AttributeValue, DynamoDb, DynamoDbClient, GetItemInput, PutItemInput};
use std::{
    collections::{HashMap, HashSet},
    convert::{TryFrom, TryInto},
    env,
};

pub struct Client {
    db: DynamoDbClient,
    users_table_name: String,
}

impl Default for Client {
    fn default() -> Self {
        Client {
            db: DynamoDbClient::new(Region::default()),
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

    pub fn set_password(&mut self, pass: String) -> Result<(), ResponseError> {
        let mut config = argon2::Config::default();
        config.variant = argon2::Variant::Argon2id;
        let mut rng = ChaChaRng::from_entropy();
        let mut salt: [u8; 64] = [0; 64];
        rng.fill(&mut salt);
        let hash = argon2::hash_encoded(pass.as_bytes(), &salt, &config)?;
        self.password = hash;
        Ok(())
    }

    pub fn compare_password(&self, crypt_pass: String) -> Result<bool, ResponseError> {
        let verify = argon2::verify_encoded(self.password.as_ref(), crypt_pass.as_bytes())?;
        Ok(verify)
    }

    pub fn has_domain(&self, domain: impl AsRef<str>) -> bool {
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
