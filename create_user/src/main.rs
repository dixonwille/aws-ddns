use http::StatusCode;
use lambda_http::{
    handler,
    lambda::{self, Context},
    IntoResponse, Request, RequestExt, Response,
};
use serde_derive::Deserialize;
use std::collections::HashSet;

type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

#[tokio::main]
async fn main() -> Result<(), Error> {
    lambda::run(handler(create_user)).await?;
    Ok(())
}

async fn create_user(request: Request, _: Context) -> Result<impl IntoResponse, Error> {
    let req: Option<CreateUserRequest> = request.payload()?;
    let req = match req {
        Some(r) => r,
        None => return Err(Box::from("must supply a body to the request")),
    };
    Ok(Response::builder().status(StatusCode::OK).body(format!(
        "Welcome, {}, your password will be {} and domains {:?}",
        req.username, req.password, req.domains
    ))?)
}

#[derive(Deserialize)]
struct CreateUserRequest {
    username: String,
    password: String,
    domains: HashSet<String>,
}
