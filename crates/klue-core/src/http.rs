use std::time::Duration;

use serde_json::Value;

use crate::FetchError;

/// A fresh client per sign-in, with its own cookie jar, that presents Chrome 131's TLS
/// and HTTP/2 fingerprint and headers.
/// Redirects are handled by the callers.
pub(crate) fn browser_client() -> Result<wreq::Client, FetchError> {
    wreq::Client::builder()
        .emulation(wreq_util::Emulation::Chrome131)
        .cookie_store(true)
        .redirect(wreq::redirect::Policy::none())
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| FetchError::Network(e.to_string()))
}

/// Read a response body as JSON, turning HTML error pages into a readable error.
pub(crate) async fn json_body(resp: wreq::Response, what: &str) -> Result<Value, FetchError> {
    let status = resp.status();
    let text = resp.text().await?;
    serde_json::from_str(&text).map_err(|_| {
        let snippet: String = text.chars().take(120).collect();
        FetchError::Unexpected(format!("{what}: HTTP {} with a non-JSON body: {snippet}", status.as_u16()))
    })
}
