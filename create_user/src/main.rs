use http::StatusCode;
use lambda::{error::HandlerError, Context};
use lambda_http::{lambda, IntoResponse, Request, RequestExt, Response};
use serde_derive::Deserialize;
use std::collections::HashSet;

#[tokio::main]
async fn main() {
    lambda!(create_user)
}

fn create_user(request: Request, _: Context) -> Result<impl IntoResponse, HandlerError> {
    let req: Option<CreateUserRequest> = request.payload().unwrap_or_else(|_| None);
    Ok(Response::builder()
        .status(StatusCode::IM_A_TEAPOT)
        .body("I'm a Teapot")
        .expect("there was an issue creating the response"))
}

#[derive(Deserialize)]
struct CreateUserRequest {
    username: String,
    password: String,
    domains: HashSet<String>,
}
