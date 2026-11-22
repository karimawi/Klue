use std::{sync::OnceLock, time::Duration};

use serde_json::Value;
use wreq::tls::trust::CertStore;

use crate::FetchError;

/// A fresh client per sign-in, with its own cookie jar, that presents Chrome 131's TLS
/// and HTTP/2 fingerprint and headers.
/// Redirects are handled by the callers.
pub(crate) fn browser_client() -> Result<wreq::Client, FetchError> {
    let mut builder = wreq::Client::builder()
        .emulation(wreq_util::Emulation::Chrome131)
        .cookie_store(true)
        .redirect(wreq::redirect::Policy::none())
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(15));
    if let Some(store) = env_cert_store() {
        builder = builder.tls_cert_store(store.clone());
    }
    builder.build().map_err(|e| FetchError::Network(e.to_string()))
}

/// The CA certificates named by `SSL_CERT_FILE` / `SSL_CERT_DIR`, which then replace
/// the bundled Mozilla roots, as in OpenSSL and curl. Networks that inspect TLS with
/// their own CA (corporate proxies, sandboxes) set these. `None` when neither is set.
fn env_cert_store() -> Option<&'static CertStore> {
    static STORE: OnceLock<Option<CertStore>> = OnceLock::new();
    STORE
        .get_or_init(|| {
            if std::env::var_os("SSL_CERT_FILE").is_none() && std::env::var_os("SSL_CERT_DIR").is_none() {
                return None;
            }
            CertStore::builder()
                .set_default_paths()
                .build()
                .inspect_err(|e| tracing::warn!("ignoring SSL_CERT_FILE/SSL_CERT_DIR: {e}"))
                .ok()
        })
        .as_ref()
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
