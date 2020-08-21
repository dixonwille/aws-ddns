use http::StatusCode;
use lambda::{error::HandlerError, Context};
use lambda_http::{lambda, IntoResponse, Request, Response};

#[tokio::main]
async fn main() {
    lambda!(create_user)
}

fn create_user(_: Request, _: Context) -> Result<impl IntoResponse, HandlerError> {
    Ok(Response::builder()
        .status(StatusCode::IM_A_TEAPOT)
        .body("I'm a Teapot")
        .expect("there was an issue creating the response"))
}