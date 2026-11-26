//! Moving numbers to another computer: an export file with each number's settings, its
//! latest reading and its usage history, and if the user wants, its password sealed with a
//! passphrase they pick (Argon2id makes the key, XChaCha20-Poly1305 seals each password).
//!
//! Nothing goes through a server, so a computer that imports a number signs in to the
//! carrier by itself from then on. The file is JSON, so Klue Server can read it too.

use std::collections::BTreeMap;

use argon2::{Algorithm, Argon2, Params, Version};
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use chacha20poly1305::{
    KeyInit, XChaCha20Poly1305, XNonce,
    aead::{Aead, Payload},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    Carrier, LineType, Usage,
    history::History,
    store::{Account, AlertPrefs, RefreshEvery},
};

/// Export files end in `.klue`.
pub const EXTENSION: &str = "klue";
const FORMAT: &str = "klue-export";
const VERSION: u32 = 1;
/// The shortest passphrase that seals passwords.
pub const MIN_PASSPHRASE: usize = 8;
const KDF: &str = "argon2id";
/// OWASP's Argon2id settings: 19 MiB, two passes, one lane.
const M_COST: u32 = 19 * 1024;
const T_COST: u32 = 2;
const P_COST: u32 = 1;
const NONCE: usize = 24;
const TAG: usize = 16;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum TransferError {
    #[error("this isn't a Klue export file")]
    NotAnExport,
    #[error("this file comes from a newer Klue")]
    TooNew,
    #[error("the passphrase is wrong")]
    WrongPassphrase,
    #[error("the passwords couldn't be sealed")]
    Seal,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Export {
    format: String,
    version: u32,
    pub exported_at: DateTime<Utc>,
    /// The Klue version that wrote it.
    pub app_version: String,
    /// How the passphrase becomes the key, when passwords are sealed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    seal: Option<Seal>,
    pub lines: Vec<Line>,
}

/// Argon2id's settings and salt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Seal {
    kdf: String,
    m_cost: u32,
    t_cost: u32,
    p_cost: u32,
    salt: String,
}

/// One number as another computer needs it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Line {
    pub carrier: Carrier,
    pub line: LineType,
    pub number: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub login: Option<String>,
    #[serde(default)]
    pub nickname: Option<String>,
    #[serde(default)]
    pub owner_name: Option<String>,
    #[serde(default)]
    pub refresh: Option<RefreshEvery>,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub alerts: AlertPrefs,
    #[serde(default)]
    pub main_bundle: Option<String>,
    #[serde(default)]
    pub usage: Option<Usage>,
    #[serde(default)]
    pub history: History,
    pub created_at: DateTime<Utc>,
    /// The password, sealed: base64 of the nonce and then the sealed bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    password: Option<String>,
}

impl Export {
    /// The numbers in `accounts`. With `sealed`, the passwords it holds (by account id) are
    /// sealed with its passphrase; numbers it has no password for go without.
    pub fn new(accounts: &[&Account], app_version: &str, sealed: Option<(&str, &BTreeMap<String, String>)>) -> Result<Self, TransferError> {
        let seal = sealed.map(|_| Seal { kdf: KDF.into(), m_cost: M_COST, t_cost: T_COST, p_cost: P_COST, salt: B64.encode(rand::random::<[u8; 16]>()) });
        let key = match (&seal, sealed) {
            (Some(seal), Some((passphrase, _))) => Some(seal.key(passphrase).map_err(|_| TransferError::Seal)?),
            _ => None,
        };
        let mut lines = Vec::with_capacity(accounts.len());
        for account in accounts {
            let mut line = Line::from(*account);
            if let (Some(key), Some((_, passwords))) = (&key, sealed)
                && let Some(password) = passwords.get(&account.id)
            {
                line.password = Some(seal_secret(key, &line.aad(), password)?);
            }
            lines.push(line);
        }
        Ok(Export { format: FORMAT.into(), version: VERSION, exported_at: Utc::now(), app_version: app_version.into(), seal, lines })
    }

    pub fn to_json(&self) -> Vec<u8> {
        serde_json::to_vec_pretty(self).unwrap_or_default()
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, TransferError> {
        #[derive(Deserialize)]
        struct Head {
            format: String,
            version: u32,
        }
        let head: Head = serde_json::from_slice(bytes).map_err(|_| TransferError::NotAnExport)?;
        if head.format != FORMAT {
            return Err(TransferError::NotAnExport);
        }
        if head.version > VERSION {
            return Err(TransferError::TooNew);
        }
        serde_json::from_slice(bytes).map_err(|e| {
            tracing::info!("unreadable export file: {e}");
            TransferError::NotAnExport
        })
    }

    /// The passwords, opened with `passphrase`, one per line (`None` where there's none).
    pub fn passwords(&self, passphrase: &str) -> Result<Vec<Option<String>>, TransferError> {
        let Some(seal) = &self.seal else { return Ok(vec![None; self.lines.len()]) };
        if !self.lines.iter().any(Line::has_password) {
            return Ok(vec![None; self.lines.len()]);
        }
        let key = seal.key(passphrase)?;
        self.lines.iter().map(|line| line.password.as_deref().map(|sealed| open_secret(&key, &line.aad(), sealed)).transpose()).collect()
    }
}

impl Seal {
    fn key(&self, passphrase: &str) -> Result<[u8; 32], TransferError> {
        // A damaged or made-up file shouldn't make Klue churn for minutes or eat its memory.
        if self.kdf != KDF || self.m_cost > 256 * 1024 || self.t_cost > 10 || self.p_cost > 4 {
            return Err(TransferError::NotAnExport);
        }
        let salt = B64.decode(&self.salt).map_err(|_| TransferError::NotAnExport)?;
        let params = Params::new(self.m_cost, self.t_cost, self.p_cost, Some(32)).map_err(|_| TransferError::NotAnExport)?;
        let mut key = [0u8; 32];
        Argon2::new(Algorithm::Argon2id, Version::V0x13, params).hash_password_into(passphrase.as_bytes(), &salt, &mut key).map_err(|_| TransferError::NotAnExport)?;
        Ok(key)
    }
}

fn seal_secret(key: &[u8; 32], aad: &str, secret: &str) -> Result<String, TransferError> {
    let cipher = XChaCha20Poly1305::new_from_slice(key).map_err(|_| TransferError::Seal)?;
    let nonce: [u8; NONCE] = rand::random();
    let sealed = cipher.encrypt(&XNonce::from(nonce), Payload { msg: secret.as_bytes(), aad: aad.as_bytes() }).map_err(|_| TransferError::Seal)?;
    let mut bytes = nonce.to_vec();
    bytes.extend(sealed);
    Ok(B64.encode(bytes))
}

/// The AEAD tag only checks out with the right key, so a wrong passphrase shows up here.
fn open_secret(key: &[u8; 32], aad: &str, sealed: &str) -> Result<String, TransferError> {
    let bytes = B64.decode(sealed).map_err(|_| TransferError::NotAnExport)?;
    if bytes.len() < NONCE + TAG {
        return Err(TransferError::NotAnExport);
    }
    let (nonce, data) = bytes.split_at(NONCE);
    let nonce: [u8; NONCE] = nonce.try_into().map_err(|_| TransferError::NotAnExport)?;
    let cipher = XChaCha20Poly1305::new_from_slice(key).map_err(|_| TransferError::NotAnExport)?;
    let open = cipher.decrypt(&XNonce::from(nonce), Payload { msg: data, aad: aad.as_bytes() }).map_err(|_| TransferError::WrongPassphrase)?;
    String::from_utf8(open).map_err(|_| TransferError::NotAnExport)
}

impl From<&Account> for Line {
    fn from(a: &Account) -> Self {
        Line {
            carrier: a.carrier,
            line: a.line,
            number: a.number.clone(),
            login: a.login.clone(),
            nickname: a.nickname.clone(),
            owner_name: a.owner_name.clone(),
            refresh: a.refresh,
            archived: a.archived,
            alerts: a.alerts.clone(),
            main_bundle: a.main_bundle.clone(),
            usage: a.usage.clone(),
            history: a.history.clone(),
            created_at: a.created_at,
            password: None,
        }
    }
}

impl Line {
    pub fn has_password(&self) -> bool {
        self.password.is_some()
    }

    /// What its name shows as, like [`Account::display_name`].
    pub fn display_name(&self) -> String {
        self.nickname.clone().or_else(|| self.owner_name.clone()).unwrap_or_else(|| crate::format::pretty_number(&self.number))
    }

    /// Whether `account` is this number.
    pub fn is(&self, account: &Account) -> bool {
        account.carrier == self.carrier && account.line == self.line && account.number == self.number
    }

    /// A new account for this computer: its own id, no alerts sent yet, and due for a
    /// refresh.
    pub fn to_account(&self) -> Account {
        let mut a = Account::new(self.carrier, self.line, self.number.clone());
        a.login = self.login.clone();
        a.nickname = self.nickname.clone();
        a.owner_name = self.owner_name.clone();
        a.refresh = self.refresh;
        a.archived = self.archived;
        a.alerts = self.alerts.clone();
        a.main_bundle = self.main_bundle.clone();
        a.usage = self.usage.clone();
        a.history = self.history.clone();
        a.created_at = self.created_at;
        a
    }

    /// Brings this copy into the same number on this computer: the usage history both have,
    /// and the latest reading if this one is newer. Its settings here stay as they are.
    pub fn merge_into(&self, account: &mut Account) {
        account.history.merge(&self.history);
        if self.usage.as_ref().map(|u| u.fetched_at) > account.usage.as_ref().map(|u| u.fetched_at) {
            account.usage = self.usage.clone();
        }
        if account.nickname.is_none() {
            account.nickname = self.nickname.clone();
        }
        if account.owner_name.is_none() {
            account.owner_name = self.owner_name.clone();
        }
    }

    /// Binds a sealed password to its number, so it can't be moved to another one.
    fn aad(&self) -> String {
        format!("klue:{}:{}", self.carrier.id(), self.number)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Bundle, BundleKind, Unit};
    use chrono::Duration;

    fn account(number: &str, remaining: f64, at: DateTime<Utc>) -> Account {
        let mut a = Account::new(Carrier::Orange, LineType::Mobile, number.into());
        a.nickname = Some("Work".into());
        let bundle = Bundle {
            key: "data".into(),
            kind: BundleKind::Data,
            name: "GO 200".into(),
            name_ar: None,
            unit: Unit::Gb,
            total: 20.0,
            remaining,
            starts_at: None,
            expires_at: None,
            state: Default::default(),
            settle_by: None,
        };
        a.usage = Some(Usage {
            owner_name: Some("A. Customer".into()),
            package_name: Some("GO 200".into()),
            package_name_ar: None,
            balance_egp: None,
            owed_egp: None,
            bundles: vec![bundle.clone()],
            subscribed_at: None,
            renews_at: None,
            fetched_at: at,
            info: Default::default(),
            payments: None,
        });
        a.history.record(at, &[bundle]);
        a
    }

    #[test]
    fn a_number_goes_across_with_its_history() {
        // History keeps whole seconds.
        let now = DateTime::from_timestamp(Utc::now().timestamp(), 0).unwrap();
        let a = account("01200000000", 12.5, now);
        let file = Export::new(&[&a], "0.5.0", None).unwrap();
        let back = Export::parse(&file.to_json()).unwrap();
        assert_eq!(back, file);
        assert_eq!(back.passwords("anything").unwrap(), vec![None]);
        let b = back.lines[0].to_account();
        assert_ne!(b.id, a.id);
        assert_eq!((b.number.as_str(), b.nickname.as_deref(), b.usage.as_ref(), &b.history), ("01200000000", Some("Work"), a.usage.as_ref(), &a.history));
        assert!(b.last_attempt.is_none() && b.fired.is_empty());
    }

    #[test]
    fn passwords_open_only_with_the_passphrase() {
        let now = Utc::now();
        let (a, b) = (account("01200000000", 12.5, now), account("01211111111", 3.0, now));
        let passwords = BTreeMap::from([(a.id.clone(), "s3cret!".to_string())]);
        let file = Export::new(&[&a, &b], "0.5.0", Some(("correct horse", &passwords))).unwrap();
        let text = String::from_utf8(file.to_json()).unwrap();
        assert!(!text.contains("s3cret!"));
        let file = Export::parse(text.as_bytes()).unwrap();
        assert_eq!(file.passwords("correct horse").unwrap(), vec![Some("s3cret!".to_string()), None]);
        assert_eq!(file.passwords("wrong horse"), Err(TransferError::WrongPassphrase));

        // A sealed password moved to another number doesn't open.
        let mut moved = file.clone();
        moved.lines[1].password = moved.lines[0].password.take();
        assert_eq!(moved.passwords("correct horse"), Err(TransferError::WrongPassphrase));
    }

    #[test]
    fn other_files_are_turned_away() {
        assert_eq!(Export::parse(b"{\"accounts\": []}"), Err(TransferError::NotAnExport));
        assert_eq!(Export::parse(b"not json"), Err(TransferError::NotAnExport));
        let newer = serde_json::json!({"format": "klue-export", "version": 99, "lines": []});
        assert_eq!(Export::parse(newer.to_string().as_bytes()), Err(TransferError::TooNew));
    }

    #[test]
    fn merging_keeps_both_histories_and_the_newer_reading() {
        let now = Utc::now();
        let mut here = account("01200000000", 10.0, now - Duration::hours(5));
        here.nickname = Some("Mine".into());
        here.refresh = Some(RefreshEvery::Hours3);
        let mut there = account("01200000000", 8.0, now - Duration::hours(1));
        let bundle = there.usage.as_ref().unwrap().bundles[0].clone();
        there.history.record(now - Duration::hours(3), &[Bundle { remaining: 9.0, ..bundle }]);
        let line = Export::new(&[&there], "0.5.0", None).unwrap().lines.remove(0);
        assert!(line.is(&here));

        line.merge_into(&mut here);
        let at: Vec<f64> = here.history.samples("data").iter().map(|s| s.remaining).collect();
        assert_eq!(at, [10.0, 9.0, 8.0]);
        assert_eq!(here.usage.as_ref().map(|u| u.bundles[0].remaining), Some(8.0));
        // Its own settings stay.
        assert_eq!((here.nickname.as_deref(), here.refresh), (Some("Mine"), Some(RefreshEvery::Hours3)));

        // An older copy changes nothing but the history.
        let old = Export::new(&[&account("01200000000", 11.0, now - Duration::hours(9))], "0.5.0", None).unwrap().lines.remove(0);
        old.merge_into(&mut here);
        assert_eq!(here.usage.as_ref().map(|u| u.bundles[0].remaining), Some(8.0));
        assert_eq!(here.history.samples("data").len(), 4);
    }
}
