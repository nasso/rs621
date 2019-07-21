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
    /// An HTTP error has occurred. The first value is the error code, the second is the reason of
    /// the failure given by the API, if available.
    Http(u16, Option<String>),
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
            Error::Http(code, reason) => write!(f, "HTTP error {}{}", code, match reason {
                Some(reason) => format!(": {}", reason),
                // Give em a generic reason
                None => match code {
                    200 => String::from(" OK: Request was successful"),
                    403 => String::from(" Forbidden: Access denied. May indicate that your request lacks a User-Agent header."),
                    404 => String::from(" Not Found"),
                    420 => String::from(" Invalid Record: Record could not be saved"),
                    421 => String::from(" User Throttled: User is throttled, try again later"),
                    422 => String::from(" Locked: The resource is locked and cannot be modified"),
                    423 => String::from(" Already Exists: Resource already exists"),
                    424 => String::from(" Invalid Parameters: The given parameters were invalid"),
                    500 => String::from(" Internal Server Error: Some unknown error occurred on the server"),
                    502 => String::from(" Bad Gateway: A gateway server received an invalid response from the e621 servers"),
                    503 => String::from(" Service Unavailable: Server cannot currently handle the request or you have exceeded the request rate limit. Try again later or decrease your rate of requests."),
                    520 => String::from(" Unknown Error: Unexpected server response which violates protocol"),
                    522 => String::from(" Origin Connection Time-out: CloudFlare's attempt to connect to the e621 servers timed out"),
                    524 => String::from(" Origin Connection Time-out: A connection was established between CloudFlare and the e621 servers, but it timed out before an HTTP response was received"),
                    525 => String::from(" SSL Handshake Failed: The SSL handshake between CloudFlare and the e621 servers failed"),
                    _ => String::new(),
                },
            }),
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
