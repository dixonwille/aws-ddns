use ddns_core::{
    client::Client,
    error::{LambdaError, ResponseError, ResponseErrors},
};
use http::{
    header::{HeaderMap, HeaderValue},
    StatusCode,
};
use lambda_http::{
    handler,
    lambda::{self, Context},
    Body, IntoResponse, Request, RequestExt, Response,
};
use std::{collections::HashSet, net::Ipv4Addr, str::FromStr};

#[tokio::main]
async fn main() -> Result<(), LambdaError> {
    lambda::run(handler(nic)).await?;
    Ok(())
}

async fn nic(request: Request, _: Context) -> Result<impl IntoResponse, LambdaError> {
    match parse_request(request).map_err(ResponseError::from) {
        Ok(req) => {
            let client = Client::default();
            match client
                .validate_user(req.username, req.password, req.user_agent, &req.hostnames)
                .await
            {
                Ok(_) => match client.update_hostnames(&req.hostnames, &req.ip).await {
                    Ok(_) => Ok(Response::builder()
                        .status(StatusCode::OK)
                        .header("Content-Type", "text/plain")
                        .body(Body::from("OK"))?),
                    Err(e) => Ok(e.into_response()),
                },
                Err(e) => Ok(e.into_response()),
            }
        }
        Err(e) => Ok(e.into_response()),
    }
}

struct NicRequest {
    hostnames: Vec<String>,
    ip: Ipv4Addr,
    user_agent: String,
    username: String,
    password: String,
}

impl Default for NicRequest {
    fn default() -> Self {
        NicRequest {
            hostnames: Vec::new(),
            ip: Ipv4Addr::new(127, 0, 0, 1),
            user_agent: String::new(),
            username: String::new(),
            password: String::new(),
        }
    }
}

fn parse_request(request: Request) -> Result<NicRequest, ResponseErrors> {
    let mut errs = ResponseErrors::default();
    let mut req = NicRequest::default();

    let headers = request.headers();

    match headers.get_header_value("User-Agent") {
        Ok(u) => match u.to_str() {
            Ok(agent) => req.user_agent = agent.into(),
            Err(e) => errs.add(e.into()),
        },
        Err(e) => errs.add(e),
    };

    match headers.get_header_value("Authorization") {
        Ok(a) => {
            if let Err(e) = parse_authorization(&mut req, a) {
                errs.add(e);
            }
        }
        Err(e) => errs.add(e),
    };

    let queries = request.query_string_parameters();

    match queries.get_all("hostname") {
        Some(hostnamegroups) => {
            let hostnames: Vec<String> = hostnamegroups
                .into_iter()
                .map(|hostnamegroup| {
                    hostnamegroup
                        .split(',')
                        .map(|s| s.to_owned())
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<String>>()
                })
                .flatten()
                .collect();
            if hostnames.is_empty() {
                errs.add(ResponseError::MissingQuery("hostname".into()));
            } else {
                let mut set = HashSet::new();
                for h in &hostnames {
                    if !set.contains(h) {
                        set.insert(h);
                    } else {
                        errs.add(ResponseError::InvalidQuery(
                            "hostname".into(),
                            "duplicate entries".into(),
                        ));
                        break;
                    }
                }
            }
            req.hostnames = hostnames;
        }
        None => errs.add(ResponseError::MissingQuery("hostname".into())),
    };

    match queries.get("myip") {
        Some(i) => {
            match Ipv4Addr::from_str(i) {
                Ok(i) => {
                    req.ip = i;
                }
                Err(_) => {
                    errs.add(ResponseError::InvalidQuery(
                        "myip".into(),
                        "not a valid IPv4 address".into(),
                    ));
                }
            };
        }
        None => errs.add(ResponseError::MissingQuery("myip".into())),
    };

    errs.into_result(req)
}

trait HeaderMapExt {
    fn get_header_value(&self, key: &str) -> Result<&HeaderValue, ResponseError>;
}

impl HeaderMapExt for HeaderMap {
    fn get_header_value(&self, key: &str) -> Result<&HeaderValue, ResponseError> {
        self.get(key)
            .ok_or_else(|| ResponseError::MissingHeader(key.into()))
    }
}

fn parse_authorization(req: &mut NicRequest, header: &HeaderValue) -> Result<(), ResponseError> {
    let raw_auth = String::from_utf8(base64::decode(
        header
            .to_str()?
            .strip_prefix("Basic ")
            .ok_or(ResponseError::MalformedAuthorizationHeader)?,
    )?)?;
    let auth_parts: Vec<&str> = raw_auth.splitn(2, ':').collect();
    if auth_parts.len() != 2 {
        return Err(ResponseError::MalformedAuthorizationHeader);
    }
    req.username = auth_parts[0].into();
    req.password = auth_parts[1].into();
    Ok(())
}
