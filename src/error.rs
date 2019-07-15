use std::fmt;

use reqwest_mock::header::InvalidHeaderValue;

/// Result type for `rs621`, using [`rs621::error::Error`].
///
/// [`rs621::error::Error`]: enum.Error.html
pub type Result<T> = ::std::result::Result<T, Error>;

/// Enum for `rs621` errors.
#[derive(Debug, PartialEq)]
pub enum Error {
    /// The given value for the some option is above the maximum value allowed in its context.
    /// E.g.: `order:score limit:350` is an invalid request because the maximum limit for ordered
    /// queries is 320.
    ///
    /// The first value is the name of the option, the second is the value that was given to it and
    /// the third is the biggest value allowed.
    AboveLimit(String, u64, u64),
    /// An HTTP error has occurred. The `u16` value is the HTTP error code.
    Http(u16),
    /// Serialization error. Contains a description of the error.
    Serial(String),
    /// Post JSON parsing error. The first value is the key of the invalid value, the second is its
    /// value.
    PostDeserialization(String, String),
    /// The request couldn't be send. Contains a description of the error.
    CannotSendRequest(String),
    /// The client couldn't be created. Contains a description of the error.
    CannotCreateClient(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::AboveLimit(option, value, max) => write!(
                f,
                "{}:{} is above the maximum value allowed in this context ({})",
                option, value, max
            ),
            Error::Http(code) => write!(f, "HTTP error: {}", code),
            Error::Serial(msg) => write!(f, "Serialization error: {}", msg),
            Error::PostDeserialization(k, v) => {
                write!(f, "Post JSON: invalid value for field \"{}\": {}", k, v)
            }
            Error::CannotSendRequest(msg) => write!(f, "Couldn't send request: {}", msg),
            Error::CannotCreateClient(msg) => write!(f, "Couldn't create client: {}", msg),
        }
    }
}

impl From<InvalidHeaderValue> for Error {
    fn from(e: InvalidHeaderValue) -> Error {
        Error::CannotCreateClient(format!("Invalid header value: {}", e))
    }
}
