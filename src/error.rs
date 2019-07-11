use std::fmt;

use reqwest::header::InvalidHeaderValue;

// Crate-wide Result type
pub type Result<T> = ::std::result::Result<T, Error>;

// Crate-wide Error type
#[derive(Debug, PartialEq)]
pub enum Error {
    AboveLimit(usize, usize),
    Http(u16),
    Serial(String),
    Redirect(String),
    CannotSendRequest(String),
    CannotCreateClient(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::AboveLimit(limit, max) => write!(
                f,
                "{} is above the max limit for ordered queries ({})",
                limit, max
            ),
            Error::Http(code) => write!(f, "HTTP error: {}", code),
            Error::Serial(msg) => write!(f, "Serialization error: {}", msg),
            Error::Redirect(msg) => write!(f, "Redirect error: {}", msg),
            Error::CannotSendRequest(msg) => write!(f, "Couldn't send request: {}", msg),
            Error::CannotCreateClient(msg) => write!(f, "Couldn't create client: {}", msg),
        }
    }
}

impl From<InvalidHeaderValue> for Error {
    fn from(_: InvalidHeaderValue) -> Error {
        Error::CannotCreateClient(String::from("Invalid User-Agent value"))
    }
}
