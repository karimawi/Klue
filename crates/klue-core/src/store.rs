//! Local persistence: one JSON file for settings and accounts, and the OS credential
//! store (Windows Credential Manager, macOS Keychain, Secret Service) for passwords.
//!
//! Credentials are stored under service `Klue` with the account's carrier and number as
//! the user name (`we:0223584417`), and the vault also keeps a small list of the accounts
//! (`Klue` / `accounts`), so any Klue version, or a fresh data file, finds them again.

use std::{
    collections::{BTreeMap, HashMap},
    io,
    path::{Path, PathBuf},
    sync::Mutex,
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{Bundle, Carrier, FetchError, LineType, Unit, Usage, format};

const DATA_FILE: &str = "klue.json";
const KEYRING_SERVICE: &str = "Klue";

/// `%APPDATA%\Klue\data` on Windows, `~/.local/share/klue` on Linux,
/// `~/Library/Application Support/Klue` on macOS.
pub fn default_data_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "Klue").map(|d| d.data_dir().to_path_buf())
}

// ---------------------------------------------------------------------------------------
// Settings

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Light,
    Dark,
    /// Recolour the app with the selected carrier's brand colours.
    Match,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    #[default]
    En,
    Ar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RefreshEvery {
    #[serde(rename = "15m")]
    Minutes15,
    #[default]
    #[serde(rename = "1h")]
    Hour1,
    #[serde(rename = "6h")]
    Hours6,
}

impl RefreshEvery {
    pub fn duration(self) -> std::time::Duration {
        std::time::Duration::from_secs(match self {
            RefreshEvery::Minutes15 => 15 * 60,
            RefreshEvery::Hour1 => 60 * 60,
            RefreshEvery::Hours6 => 6 * 60 * 60,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: Theme,
    pub lang: Lang,
    pub refresh: RefreshEvery,
    pub launch_at_startup: bool,
    pub keep_in_tray: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self { theme: Theme::Light, lang: Lang::En, refresh: RefreshEvery::Hour1, launch_at_startup: false, keep_in_tray: true }
    }
}

// ---------------------------------------------------------------------------------------
// Accounts and their alert preferences

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThresholdMode {
    /// In the bundle's own unit (GB, minutes...).
    #[default]
    Amount,
    Percent,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BundleAlert {
    pub enabled: bool,
    pub value: f64,
    #[serde(default)]
    pub mode: ThresholdMode,
}

impl BundleAlert {
    /// Default threshold: 10% of the bundle, rounded, at least 1 GB.
    pub fn default_for(bundle: &Bundle, enabled: bool) -> Self {
        let tenth = (bundle.total * 0.1).round();
        let value = if bundle.unit == Unit::Gb { tenth.max(1.0) } else { tenth.max(0.0) };
        Self { enabled, value, mode: ThresholdMode::Amount }
    }

    /// Threshold in the bundle's unit.
    pub fn threshold(&self, bundle: &Bundle) -> f64 {
        match self.mode {
            ThresholdMode::Amount => self.value,
            ThresholdMode::Percent => bundle.total * self.value / 100.0,
        }
    }

    pub fn is_triggered(&self, bundle: &Bundle) -> bool {
        self.enabled && bundle.total > 0.0 && bundle.remaining <= self.threshold(bundle)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LeadUnit {
    #[default]
    Days,
    Hours,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenewalReminder {
    pub enabled: bool,
    pub value: u32,
    pub unit: LeadUnit,
}

impl Default for RenewalReminder {
    fn default() -> Self {
        Self { enabled: true, value: 2, unit: LeadUnit::Days }
    }
}

impl RenewalReminder {
    pub fn lead(&self) -> chrono::Duration {
        match self.unit {
            LeadUnit::Days => chrono::Duration::days(self.value as i64),
            LeadUnit::Hours => chrono::Duration::hours(self.value as i64),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AlertPrefs {
    /// Per-bundle quota alerts, keyed by [`Bundle::key`].
    pub bundles: BTreeMap<String, BundleAlert>,
    pub renewal: RenewalReminder,
    /// Also notify on the day the package renews.
    pub renewal_day: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub carrier: Carrier,
    pub line: LineType,
    /// Normalised local number, e.g. `01001285532`.
    pub number: String,
    #[serde(default)]
    pub nickname: Option<String>,
    #[serde(default)]
    pub owner_name: Option<String>,
    #[serde(default)]
    pub alerts: AlertPrefs,
    #[serde(default)]
    pub usage: Option<Usage>,
    #[serde(default)]
    pub last_error: Option<FetchError>,
    #[serde(default)]
    pub last_attempt: Option<DateTime<Utc>>,
    /// WE refuses sign-ins until then after too many captchas.
    #[serde(default)]
    pub blocked_until: Option<DateTime<Utc>>,
    /// Alerts already shown, so each fires once per cycle.
    #[serde(default)]
    pub fired: Vec<String>,
    pub created_at: DateTime<Utc>,
    /// Key of the bundle the user chose as this line's main quota.
    #[serde(default)]
    pub main_bundle: Option<String>,
}

impl Account {
    pub fn new(carrier: Carrier, line: LineType, number: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            carrier,
            line,
            number,
            nickname: None,
            owner_name: None,
            alerts: AlertPrefs::default(),
            usage: None,
            last_error: None,
            last_attempt: None,
            blocked_until: None,
            fired: Vec::new(),
            created_at: Utc::now(),
            main_bundle: None,
        }
    }

    /// The vault user name for this account's password: carrier and number.
    pub fn credential_user(&self) -> String {
        format!("{}:{}", self.carrier.id(), self.number)
    }

    /// Nickname, else registered owner, else the number.
    pub fn display_name(&self) -> String {
        self.nickname
            .clone()
            .filter(|n| !n.trim().is_empty())
            .or_else(|| self.owner_name.clone())
            .unwrap_or_else(|| format::pretty_number(&self.number))
    }

    pub fn pretty_number(&self) -> String {
        format::pretty_number(&self.number)
    }

    /// The alert for a bundle, or the default (on for the main quota, off otherwise).
    pub fn alert_for(&self, bundle: &Bundle, is_main: bool) -> BundleAlert {
        self.alerts.bundles.get(&bundle.key).copied().unwrap_or_else(|| BundleAlert::default_for(bundle, is_main))
    }

    /// The main quota, as the user chose it (or the default).
    pub fn main_bundle(&self) -> Option<&Bundle> {
        self.usage.as_ref().and_then(|u| u.main_for(self.main_bundle.as_deref()))
    }

    pub fn main_alert_triggered(&self) -> bool {
        self.main_bundle().is_some_and(|b| self.alert_for(b, true).is_triggered(b))
    }

    /// Record a successful refresh.
    pub fn apply_usage(&mut self, usage: Usage) {
        if let Some(name) = usage.owner_name.clone() {
            self.owner_name = Some(name);
        }
        for (i, b) in usage.bundles.iter().enumerate() {
            if !self.alerts.bundles.contains_key(&b.key) {
                self.alerts.bundles.insert(b.key.clone(), BundleAlert::default_for(b, i == 0));
            }
        }
        self.usage = Some(usage);
        self.last_error = None;
        self.blocked_until = None;
        self.last_attempt = Some(Utc::now());
    }

    /// Record a failed refresh. The previous snapshot is kept.
    pub fn apply_error(&mut self, error: FetchError) {
        if let FetchError::CaptchaBlocked(until) = &error {
            self.blocked_until = Some(*until);
        }
        self.last_error = Some(error);
        self.last_attempt = Some(Utc::now());
    }

    pub fn mark_fired(&mut self, key: String) {
        if !self.fired.contains(&key) {
            self.fired.push(key);
        }
        let excess = self.fired.len().saturating_sub(64);
        self.fired.drain(..excess);
    }
}

// ---------------------------------------------------------------------------------------
// The data file

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppData {
    pub version: u32,
    pub settings: Settings,
    pub accounts: Vec<Account>,
}

impl Default for AppData {
    fn default() -> Self {
        Self { version: 1, settings: Settings::default(), accounts: Vec::new() }
    }
}

impl AppData {
    pub fn accounts_for(&self, carrier: Carrier) -> impl Iterator<Item = &Account> {
        self.accounts.iter().filter(move |a| a.carrier == carrier)
    }

    pub fn account(&self, id: &str) -> Option<&Account> {
        self.accounts.iter().find(|a| a.id == id)
    }

    pub fn account_mut(&mut self, id: &str) -> Option<&mut Account> {
        self.accounts.iter_mut().find(|a| a.id == id)
    }

    /// Most recent successful refresh across all accounts.
    pub fn last_sync(&self) -> Option<DateTime<Utc>> {
        self.accounts.iter().filter_map(|a| a.usage.as_ref().map(|u| u.fetched_at)).max()
    }
}

/// Reads and writes `klue.json` in the data directory.
#[derive(Debug, Clone)]
pub struct Store {
    dir: PathBuf,
}

impl Store {
    pub fn open_default() -> io::Result<Self> {
        let dir = default_data_dir().ok_or_else(|| io::Error::other("no home directory"))?;
        Self::open(dir)
    }

    pub fn open(dir: impl Into<PathBuf>) -> io::Result<Self> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Load the data file. A corrupt file is moved aside rather than overwritten.
    pub fn load(&self) -> AppData {
        let path = self.dir.join(DATA_FILE);
        let Ok(text) = std::fs::read_to_string(&path) else { return AppData::default() };
        match serde_json::from_str(&text) {
            Ok(data) => data,
            Err(e) => {
                let backup = self.dir.join(format!("{DATA_FILE}.corrupt-{}", Utc::now().format("%Y%m%d%H%M%S")));
                tracing::error!("{} is unreadable ({e}); moved to {}", path.display(), backup.display());
                let _ = std::fs::rename(&path, backup);
                AppData::default()
            }
        }
    }

    /// Write atomically (temp file + rename).
    pub fn save(&self, data: &AppData) -> io::Result<()> {
        let path = self.dir.join(DATA_FILE);
        let tmp = self.dir.join(format!("{DATA_FILE}.tmp"));
        std::fs::write(&tmp, serde_json::to_vec_pretty(data)?)?;
        std::fs::rename(tmp, path)
    }
}

// ---------------------------------------------------------------------------------------
// Passwords

/// Account passwords, kept in the OS credential store. If no credential store is
/// available (e.g. Linux without a Secret Service), passwords are kept in memory for
/// this session only and never written to disk.
#[derive(Default)]
pub struct Secrets {
    memory: Mutex<HashMap<String, String>>,
}

/// What the vault remembers about an account besides its password.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VaultAccount {
    pub carrier: Carrier,
    pub line: LineType,
    pub number: String,
    #[serde(default)]
    pub nickname: Option<String>,
}

const INDEX_USER: &str = "accounts";

impl Secrets {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, account: &Account) -> Option<String> {
        let user = account.credential_user();
        if let Some(p) = self.memory.lock().ok().and_then(|m| m.get(&user).cloned()) {
            return Some(p);
        }
        match read_entry(&user) {
            Ok(p) => Some(p),
            Err(e) => {
                // Klue 0.1 keyed passwords by the account's random id; move them over.
                if let Ok(p) = read_entry(&account.id) {
                    if write_entry(&user, &p).is_ok() {
                        let _ = keyring::Entry::new(KEYRING_SERVICE, &account.id).and_then(|e| e.delete_credential());
                    }
                    return Some(p);
                }
                tracing::warn!("no stored password for {user}: {e}");
                None
            }
        }
    }

    /// Store a password. Returns `false` when it could only be kept in memory.
    pub fn set(&self, account: &Account, password: &str) -> bool {
        let user = account.credential_user();
        if let Ok(mut m) = self.memory.lock() {
            m.insert(user.clone(), password.to_string());
        }
        match write_entry(&user, password) {
            Ok(()) => true,
            Err(e) => {
                tracing::warn!("credential store unavailable, keeping the password in memory only: {e}");
                false
            }
        }
    }

    pub fn delete(&self, account: &Account) {
        let user = account.credential_user();
        if let Ok(mut m) = self.memory.lock() {
            m.remove(&user);
        }
        for name in [user.as_str(), account.id.as_str()] {
            if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, name) {
                let _ = entry.delete_credential();
            }
        }
    }

    /// Record the account list in the vault (no passwords; those are separate entries).
    pub fn save_index(&self, accounts: &[Account]) {
        let list: Vec<VaultAccount> = accounts
            .iter()
            .map(|a| VaultAccount { carrier: a.carrier, line: a.line, number: a.number.clone(), nickname: a.nickname.clone() })
            .collect();
        if let Ok(json) = serde_json::to_string(&list)
            && let Err(e) = write_entry(INDEX_USER, &json)
        {
            tracing::debug!("couldn't save the account list to the credential store: {e}");
        }
    }

    pub fn load_index(&self) -> Vec<VaultAccount> {
        read_entry(INDEX_USER).ok().and_then(|json| serde_json::from_str(&json).ok()).unwrap_or_default()
    }
}

fn read_entry(user: &str) -> keyring::Result<String> {
    keyring::Entry::new(KEYRING_SERVICE, user)?.get_password()
}

fn write_entry(user: &str, secret: &str) -> keyring::Result<()> {
    keyring::Entry::new(KEYRING_SERVICE, user)?.set_password(secret)
}

impl AppData {
    /// Add accounts the vault knows about but the data file doesn't (a new data file, or
    /// one written by another Klue version). Returns how many were added.
    pub fn restore(&mut self, from_vault: Vec<VaultAccount>) -> usize {
        let mut added = 0;
        for v in from_vault {
            if self.accounts.iter().any(|a| a.carrier == v.carrier && a.number == v.number) {
                continue;
            }
            let mut account = Account::new(v.carrier, v.line, v.number);
            account.nickname = v.nickname;
            self.accounts.push(account);
            added += 1;
        }
        added
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BundleKind;

    fn bundle(total: f64, remaining: f64) -> Bundle {
        Bundle {
            key: "data".into(),
            kind: BundleKind::Data,
            name: "Data".into(),
            unit: Unit::Gb,
            total,
            remaining,
            starts_at: None,
            expires_at: None,
            state: Default::default(),
            settle_by: None,
        }
    }

    #[test]
    fn restores_accounts_from_the_vault() {
        let mut data = AppData::default();
        data.accounts.push(Account::new(Carrier::We, LineType::Landline, "0223584417".into()));
        let vault = vec![
            VaultAccount { carrier: Carrier::We, line: LineType::Landline, number: "0223584417".into(), nickname: None },
            VaultAccount { carrier: Carrier::Vodafone, line: LineType::Mobile, number: "01000000000".into(), nickname: Some("Work".into()) },
        ];
        assert_eq!(data.restore(vault), 1);
        assert_eq!(data.accounts.len(), 2);
        assert_eq!(data.accounts[1].nickname.as_deref(), Some("Work"));
        assert_eq!(data.accounts[1].credential_user(), "vf:01000000000");
    }

    #[test]
    fn default_alert_is_ten_percent() {
        assert_eq!(BundleAlert::default_for(&bundle(140.0, 10.0), true).value, 14.0);
        assert_eq!(BundleAlert::default_for(&bundle(5.0, 1.0), true).value, 1.0);
    }

    #[test]
    fn percent_thresholds() {
        let alert = BundleAlert { enabled: true, value: 20.0, mode: ThresholdMode::Percent };
        assert!(alert.is_triggered(&bundle(50.0, 9.4)));
        assert!(!alert.is_triggered(&bundle(50.0, 10.5)));
    }

    #[test]
    fn round_trips_and_survives_corruption() {
        let dir = std::env::temp_dir().join(format!("klue-store-{}", uuid::Uuid::new_v4()));
        let store = Store::open(&dir).unwrap();
        let mut data = AppData::default();
        data.accounts.push(Account::new(Carrier::We, LineType::Landline, "0223584417".into()));
        data.settings.theme = Theme::Match;
        store.save(&data).unwrap();
        assert_eq!(store.load(), data);

        std::fs::write(dir.join(DATA_FILE), "{not json").unwrap();
        assert_eq!(store.load(), AppData::default());
        assert!(std::fs::read_dir(&dir).unwrap().any(|e| e.unwrap().file_name().to_string_lossy().contains("corrupt")));
        let _ = std::fs::remove_dir_all(dir);
    }
}
