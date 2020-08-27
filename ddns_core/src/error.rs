use http::{header::ToStrError, Error as httpError, Response, StatusCode};
use lambda_http::{Body, IntoResponse};
use serde::Serialize;

pub type LambdaError = Box<dyn std::error::Error + Send + Sync + 'static>;

#[derive(Debug, Clone)]
pub enum ResponseError {
    MissingHeader(String),
    MissingQuery(String),
    InvalidQuery(String, String),
    MissingField(String),
    InvalidField(String, String),
    MalformedAuthorizationHeader,
    ParseError(String),
    Http(String),
    Base64Decode(base64::DecodeError),
    FromUtf8Error(std::string::FromUtf8Error),
    MultipleErrors(Vec<ResponseError>),
    UserExists,
    InvalidCredentials,
    HostnameValidation(String),

    DbError(String),
    Route53Error(String),
    NotFound(String),
    Argon(String),
}

impl std::fmt::Display for ResponseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResponseError::MissingHeader(_) => write!(f, "missing header"),
            ResponseError::MissingQuery(_) => write!(f, "missing query"),
            ResponseError::InvalidQuery(_, _) => write!(f, "invalid query"),
            ResponseError::MissingField(_) => write!(f, "missing field"),
            ResponseError::InvalidField(_, _) => write!(f, "invalid field"),
            ResponseError::MalformedAuthorizationHeader => {
                write!(f, "malformed Authorization header")
            }
            ResponseError::Http(_) => write!(f, "http error"),
            ResponseError::Base64Decode(_) => write!(f, "issue decoding base64"),
            ResponseError::FromUtf8Error(_) => write!(f, "could not convert bytes to utf8"),
            ResponseError::ParseError(_) => write!(f, "could not parse object"),
            ResponseError::MultipleErrors(_) => write!(f, "many errors have occured"),
            ResponseError::UserExists => write!(f, "user already exist"),
            ResponseError::InvalidCredentials => write!(f, "credentials are not valid"),
            ResponseError::HostnameValidation(_) => write!(f, "not authorized to update hostname"),
            ResponseError::DbError(_) => write!(f, "error occured in database"),
            ResponseError::Route53Error(_) => write!(f, "error occured in route53"),
            ResponseError::NotFound(_) => write!(f, "item was not found"),
            ResponseError::Argon(_) => write!(f, "issue with hashing algorithm"),
        }
    }
}

impl std::error::Error for ResponseError {}

impl From<httpError> for ResponseError {
    fn from(e: httpError) -> Self {
        ResponseError::Http(format!("{}", e))
    }
}

impl From<ToStrError> for ResponseError {
    fn from(e: ToStrError) -> Self {
        ResponseError::ParseError(format!("{}", e))
    }
}

impl From<base64::DecodeError> for ResponseError {
    fn from(e: base64::DecodeError) -> Self {
        ResponseError::Base64Decode(e)
    }
}

impl From<std::string::FromUtf8Error> for ResponseError {
    fn from(e: std::string::FromUtf8Error) -> Self {
        ResponseError::FromUtf8Error(e)
    }
}

impl From<argon2::Error> for ResponseError {
    fn from(e: argon2::Error) -> Self {
        ResponseError::Argon(format!("{}", e))
    }
}

impl From<ResponseErrors> for ResponseError {
    fn from(es: ResponseErrors) -> Self {
        if es.inner.len() == 1 {
            return es.inner[0].clone();
        }
        ResponseError::MultipleErrors(es.inner)
    }
}

impl ResponseError {
    fn status(&self) -> StatusCode {
        match self {
            ResponseError::MissingHeader(_) => StatusCode::BAD_REQUEST,
            ResponseError::MissingQuery(_) => StatusCode::BAD_REQUEST,
            ResponseError::InvalidQuery(_, _) => StatusCode::BAD_REQUEST,
            ResponseError::MissingField(_) => StatusCode::BAD_REQUEST,
            ResponseError::InvalidField(_, _) => StatusCode::BAD_REQUEST,
            ResponseError::MalformedAuthorizationHeader => StatusCode::BAD_REQUEST,
            ResponseError::ParseError(_) => StatusCode::BAD_REQUEST,
            ResponseError::Http(_) => StatusCode::BAD_REQUEST,
            ResponseError::Base64Decode(_) => StatusCode::BAD_REQUEST,
            ResponseError::FromUtf8Error(_) => StatusCode::BAD_REQUEST,
            ResponseError::MultipleErrors(_) => StatusCode::BAD_REQUEST,
            ResponseError::UserExists => StatusCode::BAD_REQUEST,
            ResponseError::InvalidCredentials => StatusCode::UNAUTHORIZED,
            ResponseError::HostnameValidation(_) => StatusCode::UNAUTHORIZED,
            ResponseError::DbError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ResponseError::Route53Error(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ResponseError::NotFound(_) => StatusCode::NOT_FOUND,
            ResponseError::Argon(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
    fn info(&self) -> Option<ResponseErrorInfo> {
        match self {
            ResponseError::MissingHeader(h) => Some(ResponseErrorInfo::from(h)),
            ResponseError::MissingQuery(q) => Some(ResponseErrorInfo::from(q)),
            ResponseError::InvalidQuery(k, r) => {
                Some(ResponseErrorInfo::from(format!("{} {}", k, r)))
            }
            ResponseError::MissingField(f) => Some(ResponseErrorInfo::from(f)),
            ResponseError::InvalidField(k, r) => {
                Some(ResponseErrorInfo::from(format!("{} {}", k, r)))
            }
            ResponseError::MalformedAuthorizationHeader => None,
            ResponseError::ParseError(e) => Some(ResponseErrorInfo::from(e)),
            ResponseError::Http(e) => Some(ResponseErrorInfo::from(e)),
            ResponseError::Base64Decode(e) => Some(ResponseErrorInfo::from(format!("{}", e))),
            ResponseError::FromUtf8Error(e) => Some(ResponseErrorInfo::from(format!("{}", e))),
            ResponseError::MultipleErrors(e) => Some(ResponseErrorInfo::from(e)),
            ResponseError::UserExists => None,
            ResponseError::InvalidCredentials => None,
            ResponseError::HostnameValidation(h) => Some(ResponseErrorInfo::from(h)),
            ResponseError::DbError(_) => None,
            ResponseError::Route53Error(_) => None,
            ResponseError::NotFound(_) => None,
            ResponseError::Argon(_) => None,
        }
    }
    fn as_json(&self) -> ResponseErrorJson {
        ResponseErrorJson {
            message: format!("{}", self),
            info: self.info(),
        }
    }
}

#[derive(Serialize)]
struct ResponseErrorJson {
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    info: Option<ResponseErrorInfo>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum ResponseErrorInfo {
    MoreInfo(String),
    ManyErrors(Vec<ResponseErrorJson>),
}

impl From<&String> for ResponseErrorInfo {
    fn from(s: &String) -> Self {
        ResponseErrorInfo::MoreInfo(s.to_owned())
    }
}

impl From<String> for ResponseErrorInfo {
    fn from(s: String) -> Self {
        ResponseErrorInfo::MoreInfo(s)
    }
}

impl From<&Vec<ResponseError>> for ResponseErrorInfo {
    fn from(es: &Vec<ResponseError>) -> Self {
        let mut errors = Vec::new();
        for e in es {
            errors.push(e.as_json());
        }
        ResponseErrorInfo::ManyErrors(errors)
    }
}

impl IntoResponse for ResponseError {
    fn into_response(self) -> Response<Body> {
        let status = self.status();
        let body = self.as_json();
        Response::builder()
            .status(status)
            .header("Content-Type", "application/json")
            .body(Body::from(
                serde_json::to_string(&body).expect("unable to turn body into json"),
            ))
            .expect("unable to create response")
    }
}

#[derive(Default)]
pub struct ResponseErrors {
    inner: Vec<ResponseError>,
}

impl ResponseErrors {
    pub fn add(&mut self, e: ResponseError) {
        self.inner.push(e)
    }

    fn is_empty(&self) -> bool {
        self.inner.len() == 0
    }

    pub fn into_result<T>(self, res: T) -> Result<T, Self> {
        if self.is_empty() {
            Ok(res)
        } else {
            Err(self)
        }
    }
}

impl From<ResponseError> for ResponseErrors {
    fn from(e: ResponseError) -> Self {
        ResponseErrors { inner: vec![e] }
    }
}

impl IntoIterator for ResponseErrors {
    type Item = ResponseError;
    type IntoIter = std::vec::IntoIter<Self::Item>;
    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_iter()
    }
}
