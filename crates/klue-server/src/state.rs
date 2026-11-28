//! Everything the server keeps while it runs, and the background work: following the
//! engine's changes, refreshing on schedule, and passing captchas to whoever can answer.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex, RwLock},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use chrono::{DateTime, Utc};
use klue_core::{Carrier, ProxyConfig, alerts::AlertEvent, store::Account};
use klue_engine::{
    Engine,
    remote::{Event, ServerSettings, secret},
};
use tokio::sync::{broadcast, oneshot};

use crate::{devices::Devices, sync::Tracker, vault::Vault};

const SETTINGS_FILE: &str = "server.json";
/// How long a captcha waits for someone to type it.
const CAPTCHA_WAIT: std::time::Duration = std::time::Duration::from_secs(180);

pub struct State {
    pub engine: Arc<Engine>,
    pub vault: Arc<Vault>,
    pub settings: RwLock<ServerSettings>,
    pub tracker: Mutex<Tracker>,
    pub events: broadcast::Sender<Event>,
    pub devices: Devices,
    pub captchas: Mutex<HashMap<String, oneshot::Sender<Option<String>>>>,
    /// The last error of each channel that failed, by channel name.
    pub channel_errors: Mutex<HashMap<String, String>>,
    pub secret: RwLock<String>,
    pub started_at: DateTime<Utc>,
    pub data_dir: PathBuf,
}

impl State {
    pub fn new(engine: Arc<Engine>, vault: Arc<Vault>, secret: String, data_dir: PathBuf) -> Arc<Self> {
        let settings: ServerSettings = std::fs::read(data_dir.join(SETTINGS_FILE)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        let mut tracker = Tracker::new();
        tracker.observe(&engine.snapshot().data);
        let state = Arc::new(Self {
            engine,
            vault,
            settings: RwLock::new(settings),
            tracker: Mutex::new(tracker),
            events: broadcast::channel(256).0,
            devices: Devices::open(data_dir.join("devices.json")),
            captchas: Mutex::new(HashMap::new()),
            channel_errors: Mutex::new(HashMap::new()),
            secret: RwLock::new(secret),
            started_at: Utc::now(),
            data_dir,
        });
        state.apply_proxy();
        state
    }

    pub fn settings(&self) -> ServerSettings {
        let mut s = self.settings.read().map(|s| s.clone()).unwrap_or_default();
        s.secrets = self.vault.names();
        s
    }

    /// Saves new settings (their `secrets` list is ignored: secrets are set on their own).
    pub fn set_settings(&self, mut new: ServerSettings) {
        new.secrets.clear();
        // Chats are linked from Telegram, not from the app; a list sent back can only drop some.
        if let Ok(current) = self.settings.read() {
            new.telegram.chats.retain(|c| current.telegram.chats.contains(c));
            new.telegram.bot = current.telegram.bot.clone().or(new.telegram.bot);
        }
        if let Ok(mut s) = self.settings.write() {
            *s = new;
        }
        self.save_settings();
        self.apply_proxy();
    }

    fn save_settings(&self) {
        let Ok(s) = self.settings.read() else { return };
        let path = self.data_dir.join(SETTINGS_FILE);
        if let Ok(bytes) = serde_json::to_vec_pretty(&*s)
            && let Err(e) = std::fs::write(&path, bytes)
        {
            tracing::error!("couldn't save {}: {e}", path.display());
        }
    }

    /// Hands the carrier proxy to the carriers' clients, or takes it away.
    pub fn apply_proxy(&self) {
        let proxy = self.settings.read().ok().map(|s| s.proxy.clone()).filter(|p| p.enabled && !p.url.trim().is_empty());
        let config = proxy.map(|p| ProxyConfig {
            url: proxy_url(&p.url, &p.username, self.vault.get(secret::PROXY_PASSWORD).as_deref()),
            carriers: if p.carriers.is_empty() { Carrier::ALL.to_vec() } else { p.carriers },
        });
        tracing::info!("carrier proxy: {}", if config.is_some() { "on" } else { "off" });
        klue_core::set_proxy(config);
    }

    pub fn send(&self, event: Event) {
        // No one listening is fine.
        let _ = self.events.send(event);
    }

    /// An alert that came due: to the desktops listening, then to the channels.
    pub async fn alert(self: &Arc<Self>, account: &Account, event: AlertEvent) {
        self.send(Event::Alert { event: event.clone() });
        crate::channels::alert(self, account, &event).await;
    }
}

/// The proxy's address with its sign-in put in, as clients take it.
pub fn proxy_url(url: &str, username: &str, password: Option<&str>) -> String {
    let url = url.trim();
    if username.is_empty() {
        return url.to_string();
    }
    let encode = |s: &str| s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect::<String>();
    let auth = match password {
        Some(p) => format!("{}:{}@", encode(username), encode(p)),
        None => format!("{}@", encode(username)),
    };
    match url.split_once("://") {
        Some((scheme, rest)) => format!("{scheme}://{auth}{rest}"),
        None => format!("http://{auth}{url}"),
    }
}

/// Follows the engine: each change becomes a revision for the devices, and refreshes in
/// flight are passed on.
pub async fn follow(state: Arc<State>) {
    let mut changes = state.engine.subscribe();
    while changes.changed().await.is_ok() {
        let snap = state.engine.snapshot();
        let (rev, refreshing) = {
            let Ok(mut t) = state.tracker.lock() else { continue };
            let changed = t.observe(&snap.data);
            let refreshing = t.refreshing(&snap.refreshing, snap.refreshing_all);
            (changed.then(|| (t.epoch().to_string(), t.rev())), refreshing)
        };
        if let Some((epoch, rev)) = rev {
            state.send(Event::Rev { epoch, rev });
        }
        if refreshing {
            state.send(Event::Refreshing { ids: snap.refreshing.clone(), all: snap.refreshing_all });
        }
    }
}

/// Refreshes what's due, sends the alerts that came due, and the daily report when it's
/// time.
pub async fn schedule(state: Arc<State>) {
    let mut tick = tokio::time::interval(klue_engine::TICK);
    loop {
        tick.tick().await;
        state.engine.refresh_due().await;
        for (account, event) in state.engine.take_due_alerts(Utc::now()) {
            state.alert(&account, event).await;
        }
        crate::channels::daily_report_if_due(&state).await;
    }
}

/// Captchas the model couldn't read, during refreshes asked for from an app: shown to the
/// apps listening (and the Telegram chats), and answered by whoever's first.
pub async fn relay_captchas(state: Arc<State>) {
    let Some(mut requests) = state.engine.take_captcha_requests() else { return };
    while let Some(request) = requests.recv().await {
        let id = uuid::Uuid::new_v4().simple().to_string();
        let (tx, rx) = oneshot::channel();
        if let Ok(mut pending) = state.captchas.lock() {
            pending.insert(id.clone(), tx);
        }
        state.send(Event::Captcha { id: id.clone(), image: B64.encode(&request.image) });
        crate::channels::captcha(&state, &id, &request.image).await;
        let state = state.clone();
        tokio::spawn(async move {
            let answer = tokio::time::timeout(CAPTCHA_WAIT, rx).await.ok().and_then(Result::ok).flatten();
            if let Ok(mut pending) = state.captchas.lock() {
                pending.remove(&id);
            }
            state.send(Event::CaptchaDone { id });
            let _ = request.reply.send(answer);
        });
    }
}

/// Answers a captcha. `false` if it's no longer waiting.
pub fn answer_captcha(state: &State, id: &str, code: Option<String>) -> bool {
    let Some(tx) = state.captchas.lock().ok().and_then(|mut p| p.remove(id)) else { return false };
    tx.send(code).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_sign_ins_are_put_in_the_address() {
        assert_eq!(proxy_url("socks5://10.0.0.2:1080", "", None), "socks5://10.0.0.2:1080");
        assert_eq!(proxy_url("http://proxy.example:3128", "me", Some("p@ss:word")), "http://me:p%40ss%3Aword@proxy.example:3128");
        assert_eq!(proxy_url("proxy.example:3128", "me", None), "http://me@proxy.example:3128");
    }
}
