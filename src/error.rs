use reqwest::header::InvalidHeaderValue;

use custom_error::custom_error;

/// Result type for `rs621`, using [`rs621::error::Error`].
///
/// [`rs621::error::Error`]: enum.Error.html
pub type Result<T> = std::result::Result<T, Error>;

custom_error! { #[derive(PartialEq)] pub Error
    AboveLimit{option: String, val: u64, max: u64} =
        "{option}:{val} is above the maximum value allowed in this context ({max})",

    Http{code: u16, reason: Option<String>} = @{
        format!("HTTP error {}{}", code, match reason {
            Some(reason) => format!(": {}", reason),
            // Give em a generic reason
            None => match code {
                200 => String::from(" OK: Request was successful"),
                403 => String::from(" Forbidden: Access denied. May indicate that your request lacks a User-Agent header."),
                404 => String::from(" Not Found"),
                412 => String::from(" Precondition failed"),
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
        })
    },

    Serial{desc: String} = "Serialization error: {desc}",

    Deserialization{desc: String} = "Deserialization error: {desc}",

    CannotSendRequest{desc: String} = "Couldn't send request: {desc}",

    CannotCreateClient{desc: String} = "Couldn't create client: {desc}",

    InvalidHeaderValue{desc: String} = "Invalid header value",
}

impl From<InvalidHeaderValue> for Error {
    fn from(e: InvalidHeaderValue) -> Error {
        Error::InvalidHeaderValue {
            desc: format!("Invalid header value: {}", e),
        }
    }
}

impl From<serde_json::error::Error> for Error {
    fn from(e: serde_json::error::Error) -> Error {
        Error::Deserialization {
            desc: format!("{:#?}", e),
        }
    }
}
