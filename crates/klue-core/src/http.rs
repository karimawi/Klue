use std::{sync::OnceLock, time::Duration};

use serde_json::Value;
use wreq::tls::trust::CertStore;

use crate::FetchError;

/// A client with its own cookie jar that looks like Chrome 131 (TLS, HTTP/2 and headers).
/// Callers handle redirects.
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

/// Plain HTTPS for GitHub: follows redirects, `timeout` for the whole response.
pub(crate) fn plain_client(timeout: Duration) -> Result<wreq::Client, FetchError> {
    let mut builder = wreq::Client::builder()
        .redirect(wreq::redirect::Policy::limited(10))
        .timeout(timeout)
        .connect_timeout(Duration::from_secs(15));
    if let Some(store) = env_cert_store() {
        builder = builder.tls_cert_store(store.clone());
    }
    builder.build().map_err(|e| FetchError::Network(e.to_string()))
}

/// CAs from `SSL_CERT_FILE` / `SSL_CERT_DIR`, replacing the bundled roots like OpenSSL
/// does. Needed behind TLS-inspecting proxies.
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

/// Parses JSON, turning HTML error pages into a readable error.
pub(crate) async fn json_body(resp: wreq::Response, what: &str) -> Result<Value, FetchError> {
    let status = resp.status();
    let text = resp.text().await?;
    serde_json::from_str(&text).map_err(|_| {
        let snippet: String = text.chars().take(120).collect();
        FetchError::Unexpected(format!("{what}: HTTP {} with a non-JSON body: {snippet}", status.as_u16()))
    })
}
