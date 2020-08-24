use http::header::ToStrError;
use http::Error as httpError;

pub type LambdaError = Box<dyn std::error::Error + Send + Sync + 'static>;

#[derive(Debug)]
pub enum Error {
    MissingHeader(String),
    MissingQuery(String),
    MissingField(String),

    MalformedAuthorizationHeader,
    UnknownContentType(String),
    UsernameAlreadyExist(String),
    InvalidCredentials,
    ParseError,

    Http(httpError),
    Base64Decode(base64::DecodeError),
    FromUtf8Error(std::string::FromUtf8Error),
    UnableToHashPassowrd,
    MultipleErrors(Vec<Error>),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::MissingHeader(h) => write!(f, "missing header \"{}\"", h),
            Error::MissingQuery(q) => write!(f, "missing query \"{}\"", q),
            Error::MissingField(field) => write!(f, "missing field \"{}\"", field),
            Error::MalformedAuthorizationHeader => write!(f, "malformed Authorization header"),
            Error::UnknownContentType(content) => write!(f, "unknown Content-Type header value \"{}\"", content),
            Error::UsernameAlreadyExist(u) => write!(f, "username \"{}\" already exists", u),
            Error::InvalidCredentials => write!(f, "invalid username and passowrd"),
            Error::Http(e) => write!(f, "http error: {}", e),
            Error::Base64Decode(e) => write!(f, "issue decoding base64: {}", e),
            Error::FromUtf8Error(e) => write!(f, "could not convert bytes to utf8: {}", e),
            Error::UnableToHashPassowrd => write!(f, "unable to create hash of the password"),
            Error::ParseError => write!(f, "could not parse object"),
            Error::MultipleErrors(_) => write!(f, "many errors have occured"),
        }
    }
}

impl std::error::Error for Error {}

impl From<httpError> for Error {
    fn from(e: httpError) -> Self {
        Error::Http(e)
    }
}

impl From<ToStrError> for Error {
    fn from(_: ToStrError) -> Self {
        Error::ParseError
    }
}

impl From<base64::DecodeError> for Error {
    fn from(e: base64::DecodeError) -> Self{
        Error::Base64Decode(e)
    }
}

impl From<std::string::FromUtf8Error> for Error {
    fn from(e: std::string::FromUtf8Error) -> Self{
        Error::FromUtf8Error(e)
    }
}

impl From<Errors> for Error {
    fn from(es: Errors) -> Self {
        Error::MultipleErrors(es.inner)
    }
}


pub struct Errors {
    inner: Vec<Error>,
}

impl Errors {
    pub fn new() -> Self {
        Errors { inner: Vec::new() }
    }

    pub fn add(&mut self, e: Error) {
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

impl From<Error> for Errors {
    fn from(e: Error) -> Self {
        Errors { inner: vec![e] }
    }
}

impl IntoIterator for Errors {
    type Item = Error;
    type IntoIter = std::vec::IntoIter<Self::Item>;
    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_iter()
    }
}
