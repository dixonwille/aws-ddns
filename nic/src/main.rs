use ddns_core::error::{Error, Errors, LambdaError};
use http::header::{HeaderMap, HeaderValue};
use http::StatusCode;
use lambda_http::{
    handler,
    lambda::{self, Context},
    IntoResponse, Request, RequestExt, Response,
};
use std::collections::HashSet;

#[tokio::main]
async fn main() -> Result<(), LambdaError> {
    lambda::run(handler(nic)).await?;
    Ok(())
}

async fn nic(request: Request, _: Context) -> Result<impl IntoResponse, LambdaError> {
    let req = parse_request(request).map_err(|e| Error::from(e))?;
    Ok(Response::builder().status(StatusCode::OK).body(format!(
        "Will update {:?} with {:?} if {:?} and {:?}/{:?} are valid",
        req.hostnames, req.ip, req.user_agent, req.username, req.password
    ))?)
}

#[derive(Default)]
struct NicRequest {
    hostnames: HashSet<String>,
    ip: String,
    user_agent: String,
    username: String,
    password: String,
}

fn parse_request(request: Request) -> Result<NicRequest, Errors> {
    let mut errs = Errors::new();
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
            req.hostnames = hostnamegroups
                .into_iter()
                .map(|hostnamegroup| hostnamegroup.split(",").collect())
                .collect()
        }
        None => errs.add(Error::MissingQuery("hostname".into())),
    };

    match queries.get("myip") {
        Some(i) => req.ip = i.into(),
        None => errs.add(Error::MissingQuery("myip".into())),
    };

    
    errs.into_result(req)
}

trait HeaderMapExt {
    fn get_header_value(&self, key: &str) -> Result<&HeaderValue, Error>;
}

impl HeaderMapExt for HeaderMap {
    fn get_header_value(&self, key: &str) -> Result<&HeaderValue, Error> {
        self.get(key).ok_or(Error::MissingHeader(key.into()))
    }
}

fn parse_authorization(req: &mut NicRequest, header: &HeaderValue) -> Result<(), Error> {
    let raw_auth = String::from_utf8(base64::decode(
        header
            .to_str()?
            .strip_prefix("Basic ")
            .ok_or(Error::MalformedAuthorizationHeader)?,
    )?)?;
    let auth_parts: Vec<&str> = raw_auth.splitn(2, ":").collect();
    if auth_parts.len() != 2 {
        return Err(Error::MalformedAuthorizationHeader);
    }
    req.username = auth_parts[0].into();
    req.password = auth_parts[1].into();
    Ok(())
}