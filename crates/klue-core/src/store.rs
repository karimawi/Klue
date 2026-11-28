//! Settings and accounts in one JSON file; passwords in the OS credential store.
//!
//! Passwords are saved under service `Klue`, user `<carrier>:<number>` (`we:0223584417`).
//! The vault also keeps the account list (`Klue` / `accounts`), so a fresh data file can
//! find them again.

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
    /// Recolour the app with the selected carrier's colours.
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
    #[serde(rename = "3h")]
    Hours3,
    #[serde(rename = "6h")]
    Hours6,
}

impl RefreshEvery {
    pub const ALL: [RefreshEvery; 4] = [RefreshEvery::Minutes15, RefreshEvery::Hour1, RefreshEvery::Hours3, RefreshEvery::Hours6];

    pub fn duration(self) -> std::time::Duration {
        std::time::Duration::from_secs(match self {
            RefreshEvery::Minutes15 => 15 * 60,
            RefreshEvery::Hour1 => 60 * 60,
            RefreshEvery::Hours3 => 3 * 60 * 60,
            RefreshEvery::Hours6 => 6 * 60 * 60,
        })
    }
}

/// How times of day are written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Clock {
    /// "14:07".
    #[default]
    #[serde(rename = "24h")]
    H24,
    /// "2:07 PM".
    #[serde(rename = "12h")]
    H12,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: Theme,
    pub lang: Lang,
    pub clock: Clock,
    /// How often numbers refresh, unless they have their own [`Account::refresh`].
    pub refresh: RefreshEvery,
    /// The carriers in the order the user put them; carriers missing here follow in
    /// [`Carrier::ALL`] order. See [`AppData::carriers`].
    pub carrier_order: Vec<Carrier>,
    pub launch_at_startup: bool,
    pub keep_in_tray: bool,
    /// Alert settings for all numbers; new numbers start with them.
    pub alerts: AlertDefaults,
    /// A release the user chose to skip.
    pub skipped_update: Option<String>,
    /// The version whose disclaimer was accepted.
    pub accepted_disclaimer: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::Light,
            lang: Lang::En,
            clock: Clock::H24,
            refresh: RefreshEvery::Hour1,
            carrier_order: Vec::new(),
            launch_at_startup: false,
            keep_in_tray: true,
            alerts: AlertDefaults::default(),
            skipped_update: None,
            accepted_disclaimer: None,
        }
    }
}

/// Alert settings applied to all numbers from Settings. New numbers and new bundles start
/// with them.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AlertDefaults {
    /// Low-quota alert on each number's main quota.
    pub main_quota: bool,
    /// Low-quota alerts on the other bundles.
    pub other_bundles: bool,
    /// Threshold as a share of the bundle, since units differ between numbers. `None` means
    /// the default 10%.
    pub percent: Option<f64>,
    pub renewal: RenewalReminder,
    pub renewal_day: bool,
}

impl Default for AlertDefaults {
    fn default() -> Self {
        Self { main_quota: true, other_bundles: false, percent: None, renewal: RenewalReminder::default(), renewal_day: false }
    }
}

impl AlertDefaults {
    /// The alert a new bundle gets.
    pub fn bundle_alert(&self, bundle: &Bundle, is_main: bool) -> BundleAlert {
        let enabled = if is_main { self.main_quota } else { self.other_bundles };
        match self.percent {
            Some(value) => BundleAlert { enabled, value, mode: ThresholdMode::Percent },
            None => BundleAlert::default_for(bundle, enabled),
        }
    }

    /// The share shown in Settings.
    pub fn percent_or_default(&self) -> f64 {
        self.percent.unwrap_or(10.0)
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
    /// The number the line signs in with, when it isn't `number`: an Orange home internet
    /// line signs in with the account's My Orange number.
    #[serde(default)]
    pub login: Option<String>,
    #[serde(default)]
    pub nickname: Option<String>,
    #[serde(default)]
    pub owner_name: Option<String>,
    /// How often this number refreshes; `None` follows [`Settings::refresh`].
    #[serde(default)]
    pub refresh: Option<RefreshEvery>,
    /// Put away: its password stays saved, but it isn't checked or listed until it's restored.
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub alerts: AlertPrefs,
    #[serde(default)]
    pub usage: Option<Usage>,
    #[serde(default)]
    pub last_error: Option<FetchError>,
    #[serde(default)]
    pub last_attempt: Option<DateTime<Utc>>,
    /// WE refuses sign-ins until this time.
    #[serde(default)]
    pub blocked_until: Option<DateTime<Utc>>,
    /// Alerts already sent this cycle.
    #[serde(default)]
    pub fired: Vec<String>,
    pub created_at: DateTime<Utc>,
    /// The bundle the user picked as the main quota.
    #[serde(default)]
    pub main_bundle: Option<String>,
    /// Usage samples for the graphs.
    #[serde(default)]
    pub history: crate::history::History,
}

impl Account {
    pub fn new(carrier: Carrier, line: LineType, number: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            carrier,
            line,
            number,
            login: None,
            nickname: None,
            owner_name: None,
            refresh: None,
            archived: false,
            alerts: AlertPrefs::default(),
            usage: None,
            last_error: None,
            last_attempt: None,
            blocked_until: None,
            fired: Vec::new(),
            created_at: Utc::now(),
            main_bundle: None,
            history: Default::default(),
        }
    }

    /// The vault user name: carrier and number, and for a phone line its kind too, since it
    /// can share its number with the home internet on it.
    pub fn credential_user(&self) -> String {
        match self.line {
            LineType::Voice => format!("{}:{}:voice", self.carrier.id(), self.number),
            _ => format!("{}:{}", self.carrier.id(), self.number),
        }
    }

    /// What the line signs in with: [`Account::login`], else its own number.
    pub fn sign_in_number(&self) -> &str {
        self.login.as_deref().unwrap_or(&self.number)
    }

    /// How often this number refreshes: its own setting, else the one for all numbers.
    pub fn refresh_every(&self, settings: &Settings) -> RefreshEvery {
        self.refresh.unwrap_or(settings.refresh)
    }

    /// When the last successful refresh was.
    pub fn updated_at(&self) -> Option<DateTime<Utc>> {
        self.usage.as_ref().map(|u| u.fetched_at)
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

    /// The main quota (the user's pick, or the default).
    pub fn main_bundle(&self) -> Option<&Bundle> {
        self.usage.as_ref().and_then(|u| u.main_for(self.main_bundle.as_deref()))
    }

    pub fn main_alert_triggered(&self) -> bool {
        self.main_bundle().is_some_and(|b| self.alert_for(b, true).is_triggered(b))
    }

    /// Records a successful refresh. New bundles get alerts from `defaults`.
    pub fn apply_usage(&mut self, mut usage: Usage, defaults: &AlertDefaults) {
        if let Some(name) = usage.owner_name.clone() {
            self.owner_name = Some(name);
        }
        keep_sizes(self.usage.as_ref(), &mut usage);
        self.history.record(usage.fetched_at, &usage.bundles);
        let main = usage.main_for(self.main_bundle.as_deref()).map(|m| m.key.clone());
        for b in &usage.bundles {
            if !self.alerts.bundles.contains_key(&b.key) {
                self.alerts.bundles.insert(b.key.clone(), defaults.bundle_alert(b, main.as_deref() == Some(b.key.as_str())));
            }
        }
        self.usage = Some(usage);
        self.last_error = None;
        self.blocked_until = None;
        self.last_attempt = Some(Utc::now());
    }

    /// Resets every alert to `defaults`; alerts already sent can fire again.
    pub fn reset_alerts(&mut self, defaults: &AlertDefaults) {
        self.alerts.renewal = defaults.renewal;
        self.alerts.renewal_day = defaults.renewal_day;
        self.alerts.bundles.clear();
        if let Some(usage) = &self.usage {
            for (i, b) in usage.ordered(self.main_bundle.as_deref()).into_iter().enumerate() {
                self.alerts.bundles.insert(b.key.clone(), defaults.bundle_alert(b, i == 0));
            }
        }
        self.fired.clear();
    }

    /// Records a failed refresh, keeping the last good data.
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

/// A carrier that doesn't say how big a bundle is (Orange's units) reports it as full. For
/// such a bundle, the most it had this cycle (same key, unit and end date) stands as its size,
/// so it shows what was used, and the graphs don't take every refresh for a new cycle. Without
/// an end date there's no telling a cycle, so the size stays as reported.
fn keep_sizes(previous: Option<&Usage>, usage: &mut Usage) {
    let Some(previous) = previous else { return };
    for b in usage.bundles.iter_mut().filter(|b| b.total <= b.remaining && b.expires_at.is_some()) {
        let before = previous.bundles.iter().find(|p| p.key == b.key && p.unit == b.unit && p.expires_at == b.expires_at);
        if let Some(before) = before
            && before.total > b.total
        {
            b.total = before.total;
        }
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
    /// The home page's tabs; see [`AppData::groups`].
    pub groups: Vec<Group>,
}

impl Default for AppData {
    fn default() -> Self {
        Self { version: 1, settings: Settings::default(), accounts: Vec::new(), groups: Vec::new() }
    }
}

/// A tab on the home page: cards to glance at together.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Group {
    pub id: String,
    /// `None` until it's renamed; the app names it ("My lines").
    #[serde(default)]
    pub name: Option<String>,
    /// In the order they show.
    #[serde(default)]
    pub cards: Vec<Card>,
}

impl Group {
    /// The id of the group there is before any is made.
    pub const FIRST: &'static str = "home";

    fn first() -> Self {
        Self { id: Self::FIRST.into(), name: None, cards: Vec::new() }
    }

    pub fn has(&self, card: &Card) -> bool {
        self.cards.iter().any(|c| c.same(card))
    }
}

/// A card on a home tab: a number and one of its bundles. A number can have a card for each.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Card {
    /// The account's id.
    pub line: String,
    /// A bundle's key, or `None` for the main bundle, whichever that is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle: Option<String>,
    /// The bundle's names when it was picked, for when the carrier stops listing it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_ar: Option<String>,
}

impl Card {
    /// A number's main bundle, whichever it is.
    pub fn main(line: &str) -> Self {
        Self { line: line.into(), bundle: None, name: None, name_ar: None }
    }

    pub fn bundle(line: &str, bundle: &Bundle) -> Self {
        Self { line: line.into(), bundle: Some(bundle.key.clone()), name: Some(bundle.name.clone()), name_ar: bundle.name_ar.clone() }
    }

    /// Whether both show the same number's same bundle.
    pub fn same(&self, other: &Card) -> bool {
        self.line == other.line && self.bundle == other.bundle
    }
}

impl AppData {
    /// Applies the all-numbers alert settings to every number.
    pub fn apply_alert_defaults(&mut self) {
        let defaults = self.settings.alerts;
        for account in &mut self.accounts {
            account.reset_alerts(&defaults);
        }
    }

    /// A carrier's numbers, in the order the user put them. Archived ones aren't listed.
    pub fn accounts_for(&self, carrier: Carrier) -> impl Iterator<Item = &Account> {
        self.accounts.iter().filter(move |a| a.carrier == carrier && !a.archived)
    }

    /// The archived numbers, in the order the user put them.
    pub fn archived(&self) -> impl Iterator<Item = &Account> {
        self.accounts.iter().filter(|a| a.archived)
    }

    /// Every carrier, in the order the user put them.
    pub fn carriers(&self) -> Vec<Carrier> {
        let mut out: Vec<Carrier> = Vec::with_capacity(Carrier::ALL.len());
        for c in self.settings.carrier_order.iter().copied().chain(Carrier::ALL) {
            if !out.contains(&c) {
                out.push(c);
            }
        }
        out
    }

    /// Puts `carrier` where `onto` is; the ones between shift over.
    pub fn move_carrier(&mut self, carrier: Carrier, onto: Carrier) {
        let mut order = self.carriers();
        if let (Some(from), Some(to)) = (order.iter().position(|c| *c == carrier), order.iter().position(|c| *c == onto)) {
            let moved = order.remove(from);
            order.insert(to, moved);
            self.settings.carrier_order = order;
        }
    }

    /// Puts number `id` where number `onto` is, among the same carrier's numbers.
    pub fn move_account(&mut self, id: &str, onto: &str) {
        let position = |id: &str| self.accounts.iter().position(|a| a.id == id);
        let (Some(from), Some(to)) = (position(id), position(onto)) else { return };
        if self.accounts[from].carrier != self.accounts[to].carrier {
            return;
        }
        let moved = self.accounts.remove(from);
        self.accounts.insert(to, moved);
    }

    pub fn account(&self, id: &str) -> Option<&Account> {
        self.accounts.iter().find(|a| a.id == id)
    }

    pub fn account_mut(&mut self, id: &str) -> Option<&mut Account> {
        self.accounts.iter_mut().find(|a| a.id == id)
    }

    /// The home page's groups, in the user's order. There's always one.
    pub fn groups(&self) -> Vec<Group> {
        if self.groups.is_empty() { vec![Group::first()] } else { self.groups.clone() }
    }

    pub fn group(&self, id: &str) -> Option<Group> {
        self.groups().into_iter().find(|g| g.id == id)
    }

    /// The group, made real if it's the first one and only implied so far.
    fn group_mut(&mut self, id: &str) -> Option<&mut Group> {
        if self.groups.is_empty() && id == Group::FIRST {
            self.groups.push(Group::first());
        }
        self.groups.iter_mut().find(|g| g.id == id)
    }

    /// A group's cards whose numbers are still listed, in its order.
    pub fn group_cards(&self, id: &str) -> Vec<(&Card, &Account)> {
        let Some(group) = self.groups.iter().find(|g| g.id == id) else { return Vec::new() };
        group.cards.iter().filter_map(|card| Some((card, self.account(&card.line).filter(|a| !a.archived)?))).collect()
    }

    /// How many numbers a group shows, counting one with several cards once.
    pub fn group_line_count(&self, id: &str) -> usize {
        let mut lines: Vec<&str> = self.group_cards(id).iter().map(|(card, _)| card.line.as_str()).collect();
        lines.sort_unstable();
        lines.dedup();
        lines.len()
    }

    /// Adds a group after the others. Returns its id.
    pub fn add_group(&mut self, name: Option<String>) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        self.insert_group(id.clone(), name);
        id
    }

    /// Adds a group with the id given, after the others. An id already used is ignored.
    pub fn insert_group(&mut self, id: String, name: Option<String>) {
        if self.groups.is_empty() {
            self.groups.push(Group::first());
        }
        if self.groups.iter().any(|g| g.id == id) {
            return;
        }
        self.groups.push(Group { id, name: name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()), cards: Vec::new() });
    }

    /// An empty name goes back to the app's name for it.
    pub fn rename_group(&mut self, id: &str, name: &str) {
        let name = Some(name.trim().to_string()).filter(|n| !n.is_empty());
        if let Some(group) = self.group_mut(id) {
            group.name = name;
        }
    }

    /// Removes a group. The last one is emptied instead, since there's always one.
    pub fn remove_group(&mut self, id: &str) {
        if self.groups.len() > 1 {
            self.groups.retain(|g| g.id != id);
        } else if let Some(group) = self.group_mut(id) {
            *group = Group { id: group.id.clone(), name: None, cards: Vec::new() };
        }
    }

    /// Puts a card in a group (at the end) or takes it out.
    pub fn set_card(&mut self, id: &str, card: Card, on: bool) {
        let Some(group) = self.group_mut(id) else { return };
        if on && !group.has(&card) {
            group.cards.push(card);
        } else if !on {
            group.cards.retain(|c| !c.same(&card));
        }
    }

    /// Puts group `id` where group `onto` is.
    pub fn move_group(&mut self, id: &str, onto: &str) {
        let position = |id: &str| self.groups.iter().position(|g| g.id == id);
        let (Some(from), Some(to)) = (position(id), position(onto)) else { return };
        let moved = self.groups.remove(from);
        self.groups.insert(to, moved);
    }

    /// Puts `card` where `onto` is in a group.
    pub fn move_card(&mut self, group: &str, card: &Card, onto: &Card) {
        let Some(group) = self.groups.iter_mut().find(|g| g.id == group) else { return };
        let position = |card: &Card| group.cards.iter().position(|c| c.same(card));
        let (Some(from), Some(to)) = (position(card), position(onto)) else { return };
        let moved = group.cards.remove(from);
        group.cards.insert(to, moved);
    }

    /// Forgets a number, in the groups too.
    pub fn remove_account(&mut self, id: &str) {
        self.accounts.retain(|a| a.id != id);
        for group in &mut self.groups {
            group.cards.retain(|c| c.line != id);
        }
    }

    /// Most recent successful refresh across the numbers still checked.
    pub fn last_sync(&self) -> Option<DateTime<Utc>> {
        self.accounts.iter().filter(|a| !a.archived).filter_map(|a| a.usage.as_ref().map(|u| u.fetched_at)).max()
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

    /// A corrupt file is moved aside, not overwritten.
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

/// Passwords in the OS credential store. Without one (Linux with no Secret Service) they
/// only live in memory for the session.
#[derive(Default)]
pub struct Secrets {
    memory: Mutex<HashMap<String, String>>,
}

/// What the vault keeps about an account besides its password.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VaultAccount {
    pub carrier: Carrier,
    pub line: LineType,
    pub number: String,
    /// [`Account::login`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub login: Option<String>,
    /// [`Account::archived`].
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub archived: bool,
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
                // 0.1 keyed passwords by the account's random id; move them.
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

    /// Saves the account list in the vault (passwords are separate entries).
    pub fn save_index(&self, accounts: &[Account]) {
        let list: Vec<VaultAccount> = accounts
            .iter()
            .map(|a| VaultAccount {
                carrier: a.carrier,
                line: a.line,
                number: a.number.clone(),
                login: a.login.clone(),
                archived: a.archived,
                nickname: a.nickname.clone(),
            })
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
    /// Adds accounts the vault knows and the data file doesn't. Returns how many.
    pub fn restore(&mut self, from_vault: Vec<VaultAccount>) -> usize {
        let mut added = 0;
        for v in from_vault {
            if self.accounts.iter().any(|a| a.carrier == v.carrier && a.line == v.line && a.number == v.number) {
                continue;
            }
            let mut account = Account::new(v.carrier, v.line, v.number);
            account.login = v.login;
            account.archived = v.archived;
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
            name_ar: None,
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
            VaultAccount { carrier: Carrier::We, line: LineType::Landline, number: "0223584417".into(), login: None, archived: false, nickname: None },
            VaultAccount { carrier: Carrier::Vodafone, line: LineType::Mobile, number: "01000000000".into(), login: None, archived: false, nickname: Some("Work".into()) },
            VaultAccount { carrier: Carrier::Orange, line: LineType::Landline, number: "0233456789".into(), login: Some("01200000000".into()), archived: true, nickname: None },
        ];
        assert_eq!(data.restore(vault), 2);
        assert_eq!(data.accounts.len(), 3);
        assert_eq!(data.accounts[1].nickname.as_deref(), Some("Work"));
        assert_eq!(data.accounts[1].credential_user(), "vf:01000000000");
        assert_eq!(data.accounts[1].sign_in_number(), "01000000000");
        // An Orange home internet line is its landline, signed in to with the My Orange number.
        assert_eq!(data.accounts[2].credential_user(), "or:0233456789");
        assert_eq!(data.accounts[2].sign_in_number(), "01200000000");
        // It was archived, and stays so.
        assert!(data.accounts[2].archived);
    }

    #[test]
    fn the_vault_index_keeps_the_sign_in_number() {
        let mut account = Account::new(Carrier::Orange, LineType::Landline, "0233456789".into());
        account.login = Some("01200000000".into());
        let saved = serde_json::to_string(&VaultAccount {
            carrier: account.carrier,
            line: account.line,
            number: account.number.clone(),
            login: account.login.clone(),
            archived: false,
            nickname: None,
        })
        .unwrap();
        let back: VaultAccount = serde_json::from_str(&saved).unwrap();
        assert_eq!(back.login.as_deref(), Some("01200000000"));
        // Older indexes have no `login`, and lines without one don't write it.
        let old: VaultAccount = serde_json::from_str(r#"{"carrier":"we","line":"landline","number":"0223584417"}"#).unwrap();
        assert_eq!((old.login.as_deref(), old.archived), (None, false));
        let written = serde_json::to_string(&old).unwrap();
        assert!(!written.contains("login") && !written.contains("archived"), "{written}");
    }

    #[test]
    fn bundles_without_a_size_keep_the_most_they_had_this_cycle() {
        let end = Utc::now() + chrono::Duration::days(5);
        let units = |remaining: f64, end| Bundle {
            key: "kart-el-kebir-bundles".into(),
            kind: BundleKind::Other,
            unit: Unit::Units,
            expires_at: Some(end),
            ..bundle(remaining, remaining)
        };
        let mut a = Account::new(Carrier::Orange, LineType::Mobile, "01200000000".into());
        let defaults = AlertDefaults::default();
        a.apply_usage(usage(vec![units(425.0, end)]), &defaults);
        a.apply_usage(usage(vec![units(380.0, end)]), &defaults);
        let b = &a.usage.as_ref().unwrap().bundles[0];
        assert_eq!((b.remaining, b.total), (380.0, 425.0));
        // A recharge raises it, and a new cycle starts over.
        a.apply_usage(usage(vec![units(600.0, end)]), &defaults);
        assert_eq!(a.usage.as_ref().unwrap().bundles[0].total, 600.0);
        a.apply_usage(usage(vec![units(300.0, end + chrono::Duration::days(30))]), &defaults);
        assert_eq!(a.usage.as_ref().unwrap().bundles[0].total, 300.0);
        // Without an end date to tell the cycle by, the size stays as the carrier says.
        a.apply_usage(usage(vec![bundle(140.0, 60.0)]), &defaults);
        a.apply_usage(usage(vec![bundle(25.0, 25.0)]), &defaults);
        assert_eq!(a.usage.as_ref().unwrap().bundles[0].total, 25.0);
    }

    #[test]
    fn carriers_keep_the_users_order() {
        let mut data = AppData::default();
        assert_eq!(data.carriers(), Carrier::ALL);
        data.move_carrier(Carrier::Orange, Carrier::We);
        assert_eq!(data.carriers(), [Carrier::Orange, Carrier::We, Carrier::Vodafone, Carrier::Etisalat]);
        // Dragged down past the others, it takes the last one's place.
        data.move_carrier(Carrier::We, Carrier::Etisalat);
        assert_eq!(data.carriers(), [Carrier::Orange, Carrier::Vodafone, Carrier::Etisalat, Carrier::We]);
        // An order saved before a carrier existed, or with repeats, still lists each once.
        data.settings.carrier_order = vec![Carrier::Vodafone, Carrier::Vodafone];
        assert_eq!(data.carriers(), [Carrier::Vodafone, Carrier::We, Carrier::Orange, Carrier::Etisalat]);
    }

    #[test]
    fn numbers_move_among_their_carriers_numbers() {
        let mut data = AppData::default();
        for (carrier, number) in [(Carrier::We, "0223584417"), (Carrier::Vodafone, "01000000000"), (Carrier::We, "0348712290"), (Carrier::We, "0403456789")] {
            data.accounts.push(Account::new(carrier, carrier.default_line(), number.into()));
        }
        let id = |data: &AppData, n: usize| data.accounts[n].id.clone();
        let we = |data: &AppData| data.accounts_for(Carrier::We).map(|a| a.number.clone()).collect::<Vec<_>>();
        let (first, vodafone, last) = (id(&data, 0), id(&data, 1), id(&data, 3));
        data.move_account(&last, &first);
        assert_eq!(we(&data), ["0403456789", "0223584417", "0348712290"]);
        // Forward too: it takes the other one's place.
        data.move_account(&first, &id(&data, 3));
        assert_eq!(we(&data), ["0403456789", "0348712290", "0223584417"]);
        // Not across carriers, and Vodafone's number stays put.
        let before = data.accounts.clone();
        data.move_account(&first, &vodafone);
        data.move_account(&first, "gone");
        assert_eq!(data.accounts, before);
        assert_eq!(data.accounts_for(Carrier::Vodafone).count(), 1);
    }

    #[test]
    fn a_phone_line_keeps_its_own_password_beside_its_home_internet() {
        let internet = Account::new(Carrier::We, LineType::Landline, "0223584417".into());
        let phone = Account::new(Carrier::We, LineType::Voice, "0223584417".into());
        assert_eq!(internet.credential_user(), "we:0223584417");
        assert_eq!(phone.credential_user(), "we:0223584417:voice");
    }

    #[test]
    fn the_home_page_has_a_group_from_the_start() {
        let mut data = AppData::default();
        let ids: Vec<String> = ["0223584417", "0348712290", "0233456789"]
            .into_iter()
            .map(|n| {
                let a = Account::new(Carrier::We, LineType::Landline, n.into());
                let id = a.id.clone();
                data.accounts.push(a);
                id
            })
            .collect();
        // One group, implied until something is put in it.
        assert_eq!(data.groups().len(), 1);
        assert!(data.groups.is_empty());
        data.set_card(Group::FIRST, Card::main(&ids[1]), true);
        data.set_card(Group::FIRST, Card::main(&ids[0]), true);
        data.set_card(Group::FIRST, Card::main(&ids[0]), true);
        assert_eq!(data.groups.len(), 1);
        let shown = |data: &AppData, g: &str| data.group_cards(g).iter().map(|(_, a)| a.number.clone()).collect::<Vec<_>>();
        assert_eq!(shown(&data, Group::FIRST), ["0348712290", "0223584417"]);
        data.move_card(Group::FIRST, &Card::main(&ids[0]), &Card::main(&ids[1]));
        assert_eq!(shown(&data, Group::FIRST), ["0223584417", "0348712290"]);

        // Archived numbers stay in the group but don't show; removed ones leave it.
        data.accounts[1].archived = true;
        assert_eq!(shown(&data, Group::FIRST), ["0223584417"]);
        data.remove_account(&ids[0]);
        assert_eq!(data.groups[0].cards, [Card::main(&ids[1])]);
    }

    #[test]
    fn a_number_can_have_a_card_for_each_bundle() {
        let mut data = AppData::default();
        let mut a = Account::new(Carrier::We, LineType::Landline, "0223584417".into());
        let minutes = Bundle { key: "minutes".into(), kind: BundleKind::Minutes, name: "Minutes".into(), name_ar: Some("دقائق".into()), unit: Unit::Minutes, ..bundle(1000.0, 610.0) };
        a.apply_usage(usage(vec![bundle(140.0, 60.0), minutes.clone()]), &AlertDefaults::default());
        let id = a.id.clone();
        data.accounts.push(a);

        data.set_card(Group::FIRST, Card::main(&id), true);
        data.set_card(Group::FIRST, Card::bundle(&id, &minutes), true);
        data.set_card(Group::FIRST, Card::bundle(&id, &minutes), true);
        let cards: Vec<Option<String>> = data.group_cards(Group::FIRST).iter().map(|(c, _)| c.bundle.clone()).collect();
        assert_eq!(cards, [None, Some("minutes".into())]);
        assert_eq!(data.group_line_count(Group::FIRST), 1);
        // It keeps the bundle's names for when the carrier stops listing it.
        let card = &data.groups[0].cards[1];
        assert_eq!((card.name.as_deref(), card.name_ar.as_deref()), (Some("Minutes"), Some("دقائق")));

        // Cards move and go by number and bundle, whatever names they kept.
        let renamed = Card { name: Some("Old name".into()), ..Card::bundle(&id, &minutes) };
        data.move_card(Group::FIRST, &renamed, &Card::main(&id));
        assert_eq!(data.groups[0].cards[0].bundle.as_deref(), Some("minutes"));
        data.set_card(Group::FIRST, renamed, false);
        assert_eq!(data.groups[0].cards, [Card::main(&id)]);
        data.remove_account(&id);
        assert!(data.groups[0].cards.is_empty());
    }

    #[test]
    fn groups_are_made_renamed_moved_and_removed() {
        let mut data = AppData::default();
        let family = data.add_group(Some(" Family ".into()));
        let work = data.add_group(None);
        let names = |data: &AppData| data.groups().iter().map(|g| g.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(&data), [None, Some("Family".into()), None]);
        data.rename_group(&work, "Work");
        data.rename_group(Group::FIRST, "Mine");
        data.move_group(&work, Group::FIRST);
        assert_eq!(names(&data), [Some("Work".into()), Some("Mine".into()), Some("Family".into())]);
        data.rename_group(&family, "  ");
        assert_eq!(data.group(&family).unwrap().name, None);
        data.remove_group(&family);
        data.remove_group(Group::FIRST);
        assert_eq!(data.groups().len(), 1);
        // The last one left is emptied, not removed.
        data.set_card(&work, Card::main("x"), true);
        data.remove_group(&work);
        assert_eq!(data.groups(), [Group { id: work, name: None, cards: Vec::new() }]);
    }

    #[test]
    fn archived_numbers_arent_listed() {
        let mut data = AppData::default();
        for number in ["0223584417", "0348712290"] {
            data.accounts.push(Account::new(Carrier::We, LineType::Landline, number.into()));
        }
        data.accounts[0].apply_usage(usage(vec![bundle(140.0, 60.0)]), &AlertDefaults::default());
        data.accounts[0].archived = true;
        assert_eq!(data.accounts_for(Carrier::We).map(|a| a.number.as_str()).collect::<Vec<_>>(), ["0348712290"]);
        assert_eq!(data.archived().map(|a| a.number.as_str()).collect::<Vec<_>>(), ["0223584417"]);
        // Its last refresh doesn't count as Klue's.
        assert_eq!(data.last_sync(), None);
        // Nor does a carrier with only archived numbers show; the order keeps its place.
        data.accounts[1].archived = true;
        assert_eq!(data.accounts_for(Carrier::We).count(), 0);
        assert_eq!(data.carriers()[0], Carrier::We);
    }

    #[test]
    fn numbers_can_refresh_on_their_own_schedule() {
        let settings = Settings::default();
        let mut a = Account::new(Carrier::Orange, LineType::Mobile, "01200000000".into());
        assert_eq!(a.refresh_every(&settings), RefreshEvery::Hour1);
        a.refresh = Some(RefreshEvery::Hours6);
        assert_eq!(a.refresh_every(&settings), RefreshEvery::Hours6);
        // 3 hours, between 1 and 6.
        let three: RefreshEvery = serde_json::from_str(r#""3h""#).unwrap();
        assert_eq!((three, three.duration().as_secs()), (RefreshEvery::Hours3, 3 * 60 * 60));
        assert_eq!(RefreshEvery::ALL, [RefreshEvery::Minutes15, RefreshEvery::Hour1, RefreshEvery::Hours3, RefreshEvery::Hours6]);
        assert_eq!(a.updated_at(), None);
    }

    #[test]
    fn older_settings_get_the_new_defaults() {
        let old: Settings = serde_json::from_str(r#"{"theme":"dark","lang":"ar","refresh":"6h"}"#).unwrap();
        assert_eq!((old.clock, old.carrier_order.len(), old.refresh), (Clock::H24, 0, RefreshEvery::Hours6));
        let twelve: Settings = serde_json::from_str(r#"{"clock":"12h","carrier_order":["orange","we"]}"#).unwrap();
        assert_eq!((twelve.clock, twelve.carrier_order.as_slice()), (Clock::H12, [Carrier::Orange, Carrier::We].as_slice()));
        let saved = serde_json::to_string(&Settings { clock: Clock::H12, carrier_order: vec![Carrier::Orange], ..Settings::default() }).unwrap();
        assert!(saved.contains(r#""clock":"12h""#) && saved.contains(r#""carrier_order":["orange"]"#), "{saved}");
        let account: Account = serde_json::from_str(&serde_json::to_string(&Account::new(Carrier::We, LineType::Landline, "0223584417".into())).unwrap()).unwrap();
        assert_eq!(account.refresh, None);
    }

    #[test]
    fn default_alert_is_ten_percent() {
        assert_eq!(BundleAlert::default_for(&bundle(140.0, 10.0), true).value, 14.0);
        assert_eq!(BundleAlert::default_for(&bundle(5.0, 1.0), true).value, 1.0);
    }

    fn usage(bundles: Vec<Bundle>) -> Usage {
        Usage {
            owner_name: None,
            package_name: None,
            package_name_ar: None,
            balance_egp: None,
            owed_egp: None,
            bundles,
            subscribed_at: None,
            renews_at: None,
            fetched_at: Utc::now(),
            info: Default::default(),
            payments: None,
        }
    }

    #[test]
    fn alert_settings_for_all_numbers() {
        let topup = Bundle { key: "topup".into(), kind: BundleKind::Addon, ..bundle(25.0, 20.0) };
        let mut data = AppData::default();
        for number in ["0223584417", "0348712290"] {
            let mut a = Account::new(Carrier::We, LineType::Landline, number.into());
            a.apply_usage(usage(vec![bundle(140.0, 60.0), topup.clone()]), &data.settings.alerts);
            a.mark_fired("quota:data:-".into());
            data.accounts.push(a);
        }
        // Defaults: the main quota at 10%, the others off.
        let a = &data.accounts[0];
        assert_eq!(a.alerts.bundles["data"], BundleAlert { enabled: true, value: 14.0, mode: ThresholdMode::Amount });
        assert!(!a.alerts.bundles["topup"].enabled);

        data.settings.alerts = AlertDefaults {
            main_quota: true,
            other_bundles: true,
            percent: Some(20.0),
            renewal: RenewalReminder { enabled: true, value: 12, unit: LeadUnit::Hours },
            renewal_day: true,
        };
        data.apply_alert_defaults();
        for a in &data.accounts {
            assert_eq!(a.alerts.bundles["data"], BundleAlert { enabled: true, value: 20.0, mode: ThresholdMode::Percent });
            assert!(a.alerts.bundles["topup"].enabled);
            assert_eq!((a.alerts.renewal.value, a.alerts.renewal.unit, a.alerts.renewal_day), (12, LeadUnit::Hours, true));
            assert!(a.fired.is_empty(), "re-armed");
        }

        // New numbers and new bundles start from them too.
        let mut fresh = Account::new(Carrier::Vodafone, LineType::Mobile, "01000000000".into());
        fresh.reset_alerts(&data.settings.alerts);
        fresh.apply_usage(usage(vec![bundle(4750.0, 1310.0)]), &data.settings.alerts);
        assert_eq!(fresh.alerts.bundles["data"].mode, ThresholdMode::Percent);
        assert!(fresh.alerts.renewal_day);
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
