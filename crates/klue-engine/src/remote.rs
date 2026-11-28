//! What the app and Klue Server say to each other over HTTPS. Both build on these types, so
//! they can't drift apart. See docs/server.md.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use klue_core::{
    Carrier, LineType, Usage,
    alerts::AlertEvent,
    history::History,
    store::{Account, AlertDefaults, AppData, Clock, Group, Lang, RefreshEvery, Settings},
};
use serde::{Deserialize, Serialize};

/// The port the server listens on unless told otherwise.
pub const PORT: u16 = 8443;

/// Request headers that say which device is asking.
pub const DEVICE_ID: &str = "x-klue-device";
pub const DEVICE_NAME: &str = "x-klue-device-name";
pub const APP_VERSION: &str = "x-klue-version";

/// What a key holds: the secret the app sends, and the fingerprint of the certificate it
/// trusts. Written `<secret>.<fingerprint>`, both base64url, or as
/// `klue://<host>:<port>#<secret>.<fingerprint>` when the server knows its address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Key {
    pub secret: String,
    /// SHA-256 of the certificate, base64url.
    pub fingerprint: String,
    pub address: Option<String>,
}

impl Key {
    pub fn parse(text: &str) -> Option<Key> {
        let text = text.trim();
        let (address, rest) = match text.strip_prefix("klue://") {
            Some(rest) => {
                let (host, key) = rest.split_once('#')?;
                (Some(host.trim_end_matches('/').to_string()).filter(|h| !h.is_empty()), key)
            }
            None => (None, text),
        };
        let (secret, fingerprint) = rest.split_once('.')?;
        let ok = |s: &str| s.len() >= 16 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        (ok(secret) && ok(fingerprint)).then(|| Key { secret: secret.to_string(), fingerprint: fingerprint.to_string(), address })
    }

    pub fn to_text(&self) -> String {
        match &self.address {
            Some(address) => format!("klue://{address}#{}.{}", self.secret, self.fingerprint),
            None => format!("{}.{}", self.secret, self.fingerprint),
        }
    }
}

/// The settings every device shares once a server is connected; the rest of
/// [`Settings`] belongs to each computer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SharedSettings {
    pub refresh: RefreshEvery,
    pub carrier_order: Vec<Carrier>,
    pub alerts: AlertDefaults,
}

impl SharedSettings {
    pub fn of(s: &Settings) -> Self {
        Self { refresh: s.refresh, carrier_order: s.carrier_order.clone(), alerts: s.alerts }
    }

    pub fn apply_to(&self, s: &mut Settings) {
        s.refresh = self.refresh;
        s.carrier_order = self.carrier_order.clone();
        s.alerts = self.alerts;
    }
}

/// What changed since a revision. A full one replaces everything.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sync {
    /// Changes with each start of the server; a revision from another start means a full sync.
    pub epoch: String,
    pub rev: u64,
    pub full: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<SharedSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub groups: Option<Vec<Group>>,
    /// Every line's id in order, when the order changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<Vec<String>>,
    /// Lines that changed. Their history holds only samples recorded since, unless the line
    /// is in `history_full`.
    pub accounts: Vec<Account>,
    #[serde(default)]
    pub history_full: BTreeSet<String>,
    #[serde(default)]
    pub removed: Vec<String>,
    #[serde(default)]
    pub refreshing: BTreeSet<String>,
    #[serde(default)]
    pub refreshing_all: bool,
}

impl Sync {
    /// Takes this in on a device's copy of the server's data.
    pub fn apply_to(&self, mirror: &mut AppData) {
        if self.full {
            mirror.accounts.clear();
            mirror.groups.clear();
        }
        if let Some(settings) = &self.settings {
            settings.apply_to(&mut mirror.settings);
        }
        if let Some(groups) = &self.groups {
            mirror.groups = groups.clone();
        }
        mirror.accounts.retain(|a| !self.removed.contains(&a.id));
        for incoming in &self.accounts {
            match mirror.accounts.iter_mut().find(|a| a.id == incoming.id) {
                Some(mine) if !self.history_full.contains(&incoming.id) => {
                    let history = std::mem::take(&mut mine.history);
                    *mine = incoming.clone();
                    mine.history = history;
                    mine.history.merge(&incoming.history);
                }
                Some(mine) => *mine = incoming.clone(),
                None => mirror.accounts.push(incoming.clone()),
            }
        }
        if let Some(order) = &self.order {
            mirror.accounts.sort_by_key(|a| order.iter().position(|id| *id == a.id).unwrap_or(usize::MAX));
        }
    }
}

/// What the event stream sends.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    /// Sent first, and whenever something changed: sync from your revision.
    Rev { epoch: String, rev: u64 },
    Refreshing { ids: BTreeSet<String>, all: bool },
    /// An alert for the desktop, worded by the app.
    Alert { event: AlertEvent },
    /// A captcha the server's model couldn't read, during a refresh asked for from an app.
    Captcha { id: String, image: String },
    /// Answered, from somewhere; stop asking.
    CaptchaDone { id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyRequest {
    pub carrier: Carrier,
    pub line: LineType,
    pub number: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OthersRequest {
    pub carrier: Carrier,
    pub line: LineType,
    pub number: String,
    pub own: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddRequest {
    pub carrier: Carrier,
    pub line: LineType,
    pub number: String,
    pub password: String,
    pub nickname: Option<String>,
    pub usage: Usage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Added {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshRequest {
    /// `None` refreshes everything.
    pub ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptchaAnswer {
    pub code: Option<String>,
}

/// Everything, both ways: up when a computer connects, down when it switches back.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Copy {
    pub accounts: Vec<Account>,
    /// By account id.
    pub passwords: BTreeMap<String, String>,
    pub groups: Vec<Group>,
    pub settings: SharedSettings,
}

impl Copy {
    /// The lines, Home tabs and shared settings in `data`, with `passwords`.
    pub fn of(data: &AppData, passwords: BTreeMap<String, String>) -> Self {
        Self { accounts: data.accounts.clone(), passwords, groups: data.groups.clone(), settings: SharedSettings::of(&data.settings) }
    }
}

/// What a copy did on the server.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Copied {
    /// Lines the server didn't have.
    pub added: usize,
    /// Lines it had, merged with the copy's.
    pub merged: usize,
}

/// A reading taken on a computer while the server couldn't be reached.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reading {
    pub id: String,
    pub usage: Usage,
    pub history: History,
    /// Alerts shown on that computer, so the server doesn't send them again.
    pub fired: Vec<String>,
}

/// How the server is doing, for Settings → Server.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Health {
    pub name: String,
    pub version: String,
    pub started_at: DateTime<Utc>,
    pub now: DateTime<Utc>,
    pub lines: usize,
    /// Lines whose last check failed.
    pub failing: Vec<String>,
    pub last_refresh: Option<DateTime<Utc>>,
    pub devices: Vec<Device>,
    /// Free space where the server keeps its data.
    pub disk_free: Option<u64>,
    pub problems: Vec<Problem>,
}

impl Health {
    pub fn connected(&self) -> usize {
        self.devices.iter().filter(|d| d.connected).count()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    /// Has the app open and listening now.
    #[serde(default)]
    pub connected: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Problem {
    LinesFailing { count: usize },
    DiskLow { free: u64 },
    /// A channel's last send failed.
    Channel { channel: String, error: String },
}

/// Which alerts a channel sends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AlertKinds {
    /// A bundle running low.
    pub quota: bool,
    /// A renewal coming up, and renewal day.
    pub renewal: bool,
}

impl Default for AlertKinds {
    fn default() -> Self {
        Self { quota: true, renewal: true }
    }
}

impl AlertKinds {
    pub fn wants(&self, event: &AlertEvent) -> bool {
        match event {
            AlertEvent::QuotaLow { .. } => self.quota,
            AlertEvent::RenewalSoon { .. } | AlertEvent::RenewalToday { .. } => self.renewal,
        }
    }
}

/// The server's own settings, edited from the app. Secrets (tokens, passwords, webhook
/// addresses) aren't here: they're set one by one with [`SecretUpdate`] and never sent
/// back; `secrets` lists which are set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerSettings {
    pub name: String,
    /// The language of messages and reports.
    pub lang: Lang,
    pub clock: Clock,
    pub proxy: ProxySettings,
    pub telegram: TelegramSettings,
    pub email: EmailSettings,
    pub webhooks: Vec<Webhook>,
    pub report: ReportSettings,
    /// Which secrets are set, by name (see [`secret`]).
    pub secrets: BTreeSet<String>,
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            name: "Klue Server".into(),
            lang: Lang::En,
            clock: Clock::H24,
            proxy: ProxySettings::default(),
            telegram: TelegramSettings::default(),
            email: EmailSettings::default(),
            webhooks: Vec::new(),
            report: ReportSettings::default(),
            secrets: BTreeSet::new(),
        }
    }
}

/// Names of the secrets settings refer to.
pub mod secret {
    pub const TELEGRAM_TOKEN: &str = "telegram.token";
    pub const SMTP_PASSWORD: &str = "email.password";
    pub const PROXY_PASSWORD: &str = "proxy.password";

    pub fn webhook_url(id: &str) -> String {
        format!("webhook.{id}.url")
    }

    /// Signs generic webhooks.
    pub fn webhook_signing(id: &str) -> String {
        format!("webhook.{id}.signing")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretUpdate {
    pub name: String,
    /// `None` removes it.
    pub value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ProxySettings {
    pub enabled: bool,
    /// `socks5://host:1080`, `http://host:3128` or `https://…`; the password is a secret.
    pub url: String,
    pub username: String,
    /// Empty: every carrier.
    pub carriers: Vec<Carrier>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProxyTest {
    pub carrier: Carrier,
    /// How long reaching the carrier took, or why it failed.
    pub millis: Option<u64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TelegramSettings {
    /// The bot's @name, once the token is checked.
    pub bot: Option<String>,
    pub chats: Vec<TelegramChat>,
    pub alerts: AlertKinds,
    pub daily_report: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelegramChat {
    pub id: i64,
    /// The person's or group's name, as Telegram gives it.
    pub name: String,
}

/// A link that, opened in Telegram, links that chat to the bot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelegramLink {
    pub url: String,
    pub expires: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SmtpSecurity {
    /// TLS from the start (usually port 465).
    #[default]
    Tls,
    /// Plain, then STARTTLS (usually 587).
    StartTls,
    /// No encryption, for a relay on the same network.
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct EmailSettings {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub security: SmtpSecurity,
    pub username: String,
    /// "Klue <klue@example.com>".
    pub from: String,
    pub to: Vec<String>,
    pub alerts: AlertKinds,
    pub daily_report: bool,
}

impl Default for EmailSettings {
    fn default() -> Self {
        Self { enabled: false, host: String::new(), port: 465, security: SmtpSecurity::Tls, username: String::new(), from: String::new(), to: Vec::new(), alerts: AlertKinds::default(), daily_report: true }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebhookKind {
    Discord,
    Slack,
    GoogleChat,
    Teams,
    Ntfy,
    /// Any address: Klue's own JSON, signed if there's a signing secret.
    Json,
}

impl WebhookKind {
    pub const ALL: [WebhookKind; 6] = [WebhookKind::Discord, WebhookKind::Slack, WebhookKind::GoogleChat, WebhookKind::Teams, WebhookKind::Ntfy, WebhookKind::Json];
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Webhook {
    pub id: String,
    pub name: String,
    pub kind: WebhookKind,
    #[serde(default)]
    pub alerts: AlertKinds,
    #[serde(default)]
    pub daily_report: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReportSettings {
    /// The lines in reports, by id; `None` is every line, new ones too.
    pub lines: Option<Vec<String>>,
    pub period: ReportPeriod,
    /// When the daily report goes out, "21:30", in the server's time zone.
    pub daily_at: String,
}

impl Default for ReportSettings {
    fn default() -> Self {
        Self { lines: None, period: ReportPeriod::Cycle, daily_at: "21:30".into() }
    }
}

/// How far back reports look (the same as klue-report's).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportPeriod {
    #[default]
    Cycle,
    Week,
    Month,
}

/// A test message's result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sent {
    pub ok: bool,
    pub error: Option<String>,
}

/// The server's answer when a request fails: what went wrong, for the app to word.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum Failure {
    /// Signing in to the carrier failed.
    Fetch { detail: klue_core::FetchError },
    BadRequest { detail: String },
    NotFound,
    Internal { detail: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_read_with_or_without_an_address() {
        let key = Key { secret: "s3cr3t-abcdefghijklmnop".into(), fingerprint: "fp_0123456789abcdefghij".into(), address: None };
        assert_eq!(Key::parse(&key.to_text()), Some(key.clone()));
        let with = Key { address: Some("203.0.113.7:8443".into()), ..key.clone() };
        assert_eq!(with.to_text(), "klue://203.0.113.7:8443#s3cr3t-abcdefghijklmnop.fp_0123456789abcdefghij");
        assert_eq!(Key::parse(&format!("  {}\n", with.to_text())), Some(with));
        assert_eq!(Key::parse("not a key"), None);
        assert_eq!(Key::parse("short.key"), None);
    }
}
