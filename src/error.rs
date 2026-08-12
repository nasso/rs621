use url::Url;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("Request to {url} returned HTTP code {code} (reason: {reason:?})")]
    Http {
        url: Url,
        code: u16,
        reason: Option<String>,
    },

    #[error("reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Couldn't send request: {0}")]
    CannotSendRequest(String),

    #[error("Couldn't create client: {0}")]
    CannotCreateClient(String),

    #[error("Invalid header value: {0}")]
    InvalidHeaderValue(String),

    #[error("Malformed URL: {0}")]
    UrlParse(#[from] url::ParseError),
}

/// Result type for `rs621`, using [`rs621::error::Error`].
///
/// [`rs621::error::Error`]: enum.Error.html
pub type Result<T, E = Error> = std::result::Result<T, E>;
