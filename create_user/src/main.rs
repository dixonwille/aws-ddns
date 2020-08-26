use ddns_core::{
    client::{Client, User},
    error::{LambdaError, ResponseError, ResponseErrors},
};
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
        Ok(req) => {
            let client = Client::default();
            let resp = match client.get_user(&req.username).await {
                Ok(_) => ResponseError::UserExists.into_response(),
                Err(ResponseError::NotFound(_)) => {
                    match User::new(&req.username, &req.password, req.domains.clone()) {
                        Ok(user) => match client.put_user(user).await {
                            Ok(_) => Response::builder()
                                .status(StatusCode::CREATED)
                                .body(Body::from(()))?,
                            Err(e) => e.into_response(),
                        },
                        Err(e) => e.into_response(),
                    }
                }
                Err(e) => e.into_response(),
            };
            Ok(resp)
        }
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
