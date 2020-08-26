use ddns_core::error::{LambdaError, ResponseError, ResponseErrors};
use http::StatusCode;
use lambda_http::{
    handler,
    lambda::{self, Context},
    Body, IntoResponse, Request, RequestExt, Response,
};
use serde::Deserialize;
use std::collections::HashSet;

#[tokio::main]
async fn main() -> Result<(), LambdaError> {
    lambda::run(handler(create_user)).await?;
    Ok(())
}

async fn create_user(request: Request, _: Context) -> Result<impl IntoResponse, LambdaError> {
    match parse_request(request).map_err(ResponseError::from) {
        Ok(req) => Ok(Response::builder()
            .status(StatusCode::OK)
            .body(Body::from(format!(
                "Welcome, {}, your password will be {} and domains {:?}",
                req.username, req.password, req.domains
            )))?),
        Err(e) => Ok(e.into_response()),
    }
}

fn parse_request(request: Request) -> Result<CreateUserRequest, ResponseErrors> {
    let mut req = CreateUserRequest::default();
    let mut errs = ResponseErrors::default();

    match request.payload::<CreateUserRequest>() {
        Ok(r) => match r {
            Some(r) => {
                if r.username.is_empty() {
                    errs.add(ResponseError::MissingField("username".into()));
                } else {
                    if r.username.len() < 7 {
                        errs.add(ResponseError::InvalidField(
                            "username".into(),
                            "is less than 7 characters long".into(),
                        ))
                    }
                    if r.username.contains(':') {
                        errs.add(ResponseError::InvalidField(
                            "username".into(),
                            "contains a colon (:)".into(),
                        ))
                    }
                }

                if r.password.is_empty() {
                    errs.add(ResponseError::MissingField("password".into()));
                } else if r.password.len() < 7 {
                    errs.add(ResponseError::InvalidField(
                        "password".into(),
                        "is less than 7 characters long".into(),
                    ))
                }

                if r.domains.is_empty() {
                    errs.add(ResponseError::MissingField("domains".into()));
                }
                req = r;
            }
            None => {
                errs.add(ResponseError::MissingField("username".into()));
                errs.add(ResponseError::MissingField("password".into()));
                errs.add(ResponseError::MissingField("domains".into()));
            }
        },
        Err(e) => {
            errs.add(ResponseError::ParseError(format!("{}", e)));
        }
    }
    errs.into_result(req)
}

#[derive(Deserialize, Default)]
struct CreateUserRequest {
    username: String,
    password: String,
    domains: HashSet<String>,
}
