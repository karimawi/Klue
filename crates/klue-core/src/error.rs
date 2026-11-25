use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::LineType;

/// Why a refresh failed. Serializable so it can be shown after a restart.
#[derive(Debug, Clone, PartialEq, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "type", content = "detail", rename_all = "snake_case")]
pub enum FetchError {
    /// Wrong number or password.
    #[error("the carrier rejected this number and password")]
    BadCredentials,
    /// The captcha couldn't be read.
    #[error("the carrier asked for a captcha that couldn't be read")]
    CaptchaUnsolved,
    /// The carrier rejected the captcha code.
    #[error("the carrier didn't accept the captcha code")]
    CaptchaRejected,
    /// Too many sign-ins: WE gives no captchas, or Orange's firewall rejects requests, until
    /// this time.
    #[error("the carrier is blocking sign-ins until {0}")]
    CaptchaBlocked(DateTime<Utc>),
    /// The account has no line of this type: an Orange account without an Orange mobile line,
    /// or without a linked landline (or linked to another one now).
    #[error("the account has no {0:?} line to read")]
    NoLine(LineType),
    /// A carrier error message we don't recognise.
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
    /// Errors where retrying soon won't help.
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
