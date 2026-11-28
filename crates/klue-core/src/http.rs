use std::{
    sync::{OnceLock, RwLock},
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use wreq::tls::trust::CertStore;

use crate::{Carrier, FetchError};

/// A proxy for carrier requests, for when a carrier turns away requests from abroad (Klue
/// Server on a VPS in another country).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProxyConfig {
    /// `http://`, `https://` or `socks5://`, with `user:password@` if it needs them.
    pub url: String,
    /// The carriers whose requests go through it.
    pub carriers: Vec<Carrier>,
}

static PROXY: RwLock<Option<ProxyConfig>> = RwLock::new(None);

/// Sends the chosen carriers' requests through `proxy` from now on, or none.
pub fn set_proxy(proxy: Option<ProxyConfig>) {
    if let Ok(mut current) = PROXY.write() {
        *current = proxy;
    }
}

fn proxy_for(carrier: Carrier) -> Result<Option<wreq::Proxy>, FetchError> {
    let current = PROXY.read().ok().and_then(|p| p.clone());
    match current.filter(|p| p.carriers.contains(&carrier)) {
        Some(p) => wreq::Proxy::all(&p.url).map(Some).map_err(|e| FetchError::Network(format!("proxy: {e}"))),
        None => Ok(None),
    }
}

/// The carrier's own site, for checking a proxy reaches it.
fn home_page(carrier: Carrier) -> &'static str {
    match carrier {
        Carrier::We => "https://my.te.eg/",
        Carrier::Vodafone => "https://web.vodafone.com.eg/",
        Carrier::Orange => "https://services.orange.eg/",
        Carrier::Etisalat => "https://www.etisalat.eg/",
    }
}

/// Reaches `carrier`'s site through `url` and says how long it took.
pub async fn try_proxy(url: &str, carrier: Carrier) -> Result<Duration, FetchError> {
    let proxy = wreq::Proxy::all(url).map_err(|e| FetchError::Network(format!("proxy: {e}")))?;
    let client = wreq::Client::builder()
        .emulation(wreq_util::Emulation::Chrome131)
        .proxy(proxy)
        .redirect(wreq::redirect::Policy::none())
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| FetchError::Network(e.to_string()))?;
    let started = Instant::now();
    client.get(home_page(carrier)).send().await?;
    Ok(started.elapsed())
}

/// A client with its own cookie jar that looks like Chrome 131 (TLS, HTTP/2 and headers).
/// Callers handle redirects.
pub(crate) fn browser_client(carrier: Carrier) -> Result<wreq::Client, FetchError> {
    let mut builder = wreq::Client::builder()
        .emulation(wreq_util::Emulation::Chrome131)
        .cookie_store(true)
        .redirect(wreq::redirect::Policy::none())
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(15));
    if let Some(store) = env_cert_store() {
        builder = builder.tls_cert_store(store.clone());
    }
    if let Some(proxy) = proxy_for(carrier)? {
        builder = builder.proxy(proxy);
    }
    builder.build().map_err(|e| FetchError::Network(e.to_string()))
}

/// A client with its own cookie jar that looks like an Android app built on okhttp 4.12:
/// okhttp's TLS and HTTP/2 fingerprint, its user agent, and gzip as the only encoding.
/// Callers handle redirects.
pub(crate) fn okhttp_client(carrier: Carrier) -> Result<wreq::Client, FetchError> {
    use wreq::header::{ACCEPT, HeaderMap, HeaderValue, USER_AGENT};
    let emulation = wreq_util::Emulation::builder().profile(wreq_util::Profile::OkHttp4_12).headers(false).build();
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static("okhttp/4.12.0"));
    headers.insert(ACCEPT, HeaderValue::from_static("*/*"));
    // The emulation sets its own default headers, so these come after it.
    let mut builder = wreq::Client::builder()
        .emulation(emulation)
        .default_headers(headers)
        .gzip(true)
        .brotli(false)
        .deflate(false)
        .zstd(false)
        .cookie_store(true)
        .redirect(wreq::redirect::Policy::none())
        .timeout(Duration::from_secs(40))
        .connect_timeout(Duration::from_secs(15));
    if let Some(store) = env_cert_store() {
        builder = builder.tls_cert_store(store.clone());
    }
    if let Some(proxy) = proxy_for(carrier)? {
        builder = builder.proxy(proxy);
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
