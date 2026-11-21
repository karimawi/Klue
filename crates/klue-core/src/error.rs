use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Why reading an account failed. Serializable so the last error can be shown after a restart.
#[derive(Debug, Clone, PartialEq, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "type", content = "detail", rename_all = "snake_case")]
pub enum FetchError {
    /// The carrier rejected the number/password pair.
    #[error("the carrier rejected this number and password")]
    BadCredentials,
    /// The carrier asked for a captcha and no solver could read any of the ones it handed out.
    #[error("the carrier asked for a captcha that couldn't be read")]
    CaptchaUnsolved,
    /// The carrier didn't accept the captcha code that was sent.
    #[error("the carrier didn't accept the captcha code")]
    CaptchaRejected,
    /// Too many sign-ins: the carrier refuses to hand out captchas until this time.
    #[error("the carrier is blocking sign-ins until {0}")]
    CaptchaBlocked(DateTime<Utc>),
    /// The carrier returned an error message we don't recognise.
    #[error("the carrier said: {0}")]
    Carrier(String),
    #[error("couldn't reach the carrier: {0}")]
    Network(String),
    #[error("unexpected response from the carrier: {0}")]
    Unexpected(String),
    #[error("this number format isn't supported: {0}")]
    InvalidNumber(String),
    #[error("this carrier or line type isn't supported yet")]
    Unsupported,
}

impl FetchError {
    /// Errors that mean retrying soon is pointless or harmful.
    pub fn is_auth_problem(&self) -> bool {
        matches!(self, FetchError::BadCredentials)
    }
}

impl From<wreq::Error> for FetchError {
    fn from(e: wreq::Error) -> Self {
        if e.is_decode() {
            FetchError::Unexpected(e.to_string())
        } else {
            FetchError::Network(e.to_string())
        }
    }
}
