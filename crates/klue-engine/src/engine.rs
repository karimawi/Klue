//! Lines, passwords and refreshes, behind a revision counter. Windows watch the counter and
//! re-read a [`Snapshot`]; Klue Server sends the changes on to its devices.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use chrono::{DateTime, Utc};
use klue_core::{
    Carrier, FetchError, LineType, Usage,
    alerts::{self, AlertEvent},
    captcha::{BoxFuture, CaptchaSolver, default_solver},
    carriers::{self, FetchContext},
    format,
    store::{Account, AppData, Settings, Store},
    transfer::{self, Export, TransferError},
};
use tokio::sync::{mpsc, oneshot, watch};

use crate::{
    demo,
    ops::{self, Op},
    passwords::PasswordStore,
    remote::{Copied, Copy, Reading, SharedSettings},
};

/// What a window renders from.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub data: AppData,
    /// Accounts with a refresh in flight.
    pub refreshing: BTreeSet<String>,
    /// A Refresh all is running (its refreshes are also in `refreshing`).
    pub refreshing_all: bool,
    pub demo: bool,
}

impl Snapshot {
    pub fn is_refreshing(&self, id: &str) -> bool {
        self.refreshing.contains(id)
    }
}

/// A captcha the model couldn't read, for a person to type.
pub struct CaptchaRequest {
    pub image: Vec<u8>,
    pub reply: oneshot::Sender<Option<String>>,
}

/// Captcha "solver" that asks a person, through [`Engine::take_captcha_requests`].
struct PromptSolver {
    tx: mpsc::UnboundedSender<CaptchaRequest>,
}

impl CaptchaSolver for PromptSolver {
    fn name(&self) -> &str {
        "prompt"
    }

    fn solve<'a>(&'a self, image: &'a [u8]) -> BoxFuture<'a, Option<String>> {
        Box::pin(async move {
            let (reply, answer) = oneshot::channel();
            self.tx.send(CaptchaRequest { image: image.to_vec(), reply }).ok()?;
            answer.await.ok().flatten()
        })
    }
}

pub struct Engine {
    store: Store,
    passwords: Box<dyn PasswordStore>,
    data: Mutex<AppData>,
    refreshing: Mutex<BTreeSet<String>>,
    refreshing_all: AtomicBool,
    revision: watch::Sender<u64>,
    ocr: Option<Arc<dyn CaptchaSolver>>,
    prompt: Arc<dyn CaptchaSolver>,
    captcha_rx: Mutex<Option<mpsc::UnboundedReceiver<CaptchaRequest>>>,
    debug_dir: Option<PathBuf>,
    /// The app's or server's version, written into export files.
    version: String,
    pub demo: bool,
}

/// How often the background loop checks for due refreshes and reminders.
pub const TICK: Duration = Duration::from_secs(60);
/// Minimum time between sign-ins, even on demand (WE answers rapid ones with captchas).
const MIN_INTERACTIVE_GAP: chrono::Duration = chrono::Duration::seconds(20);

impl Engine {
    /// Demo mode makes its lines up and keeps nothing.
    pub fn new(store: Store, passwords: Box<dyn PasswordStore>, demo: bool, version: &str) -> Arc<Self> {
        let data = if demo {
            demo::data()
        } else {
            let mut data = store.load();
            // Accounts with a saved password but missing from the data file come back,
            // without their history.
            let restored = data.restore(passwords.load_index());
            if restored > 0 {
                tracing::info!("restored {restored} account(s) from the credential store");
                if let Err(e) = store.save(&data) {
                    tracing::error!("couldn't save {}: {e}", store.dir().display());
                }
            }
            passwords.save_index(&data.accounts);
            data
        };
        let (tx, rx) = mpsc::unbounded_channel();
        Arc::new(Self {
            passwords,
            data: Mutex::new(data),
            refreshing: Mutex::new(BTreeSet::new()),
            refreshing_all: AtomicBool::new(false),
            revision: watch::channel(0).0,
            ocr: if demo { None } else { default_solver() },
            prompt: Arc::new(PromptSolver { tx }),
            captcha_rx: Mutex::new(Some(rx)),
            debug_dir: std::env::var_os("KLUE_DEBUG_DIR").map(PathBuf::from),
            store,
            version: version.to_string(),
            demo,
        })
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            data: self.data.lock().expect("data lock").clone(),
            refreshing: self.refreshing.lock().expect("refreshing lock").clone(),
            refreshing_all: self.refreshing_all.load(Ordering::Relaxed),
            demo: self.demo,
        }
    }

    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.revision.subscribe()
    }

    fn bump(&self) {
        self.revision.send_modify(|r| *r += 1);
    }

    /// Changes the data, saves it and notifies the windows. For what's this computer's own
    /// (theme, language and the like); shared changes go through [`apply`](Self::apply).
    pub fn update<R>(&self, f: impl FnOnce(&mut AppData) -> R) -> R {
        let (result, data) = {
            let mut data = self.data.lock().expect("data lock");
            let r = f(&mut data);
            (r, data.clone())
        };
        if !self.demo
            && let Err(e) = self.store.save(&data) {
                tracing::error!("couldn't save {}: {e}", self.store.dir().display());
            }
        self.bump();
        result
    }

    /// Applies a change to the lines, Home tabs or shared settings.
    pub fn apply(&self, op: Op) {
        if let Op::RemoveAccount { id } = &op {
            let account = self.data.lock().expect("data lock").account(id).cloned();
            if !self.demo
                && let Some(account) = &account
            {
                self.passwords.delete(account);
            }
        }
        self.update(|d| ops::apply(d, &op));
        if op.touches_index() {
            self.save_vault_index();
        }
    }

    pub fn settings(&self) -> Settings {
        self.data.lock().expect("data lock").settings.clone()
    }

    pub fn take_captcha_requests(&self) -> Option<mpsc::UnboundedReceiver<CaptchaRequest>> {
        self.captcha_rx.lock().ok()?.take()
    }

    /// Captchas go to the model, and WE is asked for another when it isn't sure. A person is
    /// only asked if the model declines them all and `interactive` is set.
    fn context(&self, interactive: bool) -> FetchContext<'_> {
        FetchContext {
            solver: self.ocr.as_deref(),
            ask: if interactive { Some(&*self.prompt) } else { None },
            debug_dir: self.debug_dir.clone(),
        }
    }

    /// Signs in with new credentials (Add a number). `number` is the one the line signs in
    /// with; an Orange home internet line is only known, and checked, once Orange names it.
    pub async fn verify(&self, carrier: Carrier, line: LineType, number: &str, password: &str) -> Result<Usage, FetchError> {
        let sign_in = format::normalize(number, carrier.sign_in_line(line))?;
        // Whether the line is in Klue already, and if so whether it's archived. A phone line
        // can have the same number as its home internet, so the kind counts too.
        let added = |line: LineType, number: &str| {
            self.data.lock().expect("data lock").accounts.iter().find(|a| a.carrier == carrier && a.line == line && a.number == number).map(|a| a.archived)
        };
        if carrier.sign_in_line(line) == line
            && let Some(archived) = added(line, &sign_in)
        {
            return Err(FetchError::AlreadyAdded { archived });
        }
        let usage = if self.demo {
            demo::verify(carrier, line, &sign_in, password).await?
        } else {
            carriers::fetch(carrier, line, &sign_in, None, password, &self.context(true)).await?
        };
        let own = carriers::line_number(carrier, line, &sign_in, &usage)?;
        if let Some(archived) = added(line, &own) {
            // The account can hold another line of this kind that isn't in Klue yet (WE).
            let others = self.other_lines(carrier, line, number, &own, password).await;
            return others.into_iter().find(|(other, _)| *other == line).map(|(_, usage)| usage).ok_or(FetchError::AlreadyAdded { archived });
        }
        Ok(usage)
    }

    /// The lines on the account just verified that Klue doesn't have yet (see
    /// [`carriers::other_lines`]). `own` is the number of the line just read.
    pub async fn other_lines(&self, carrier: Carrier, line: LineType, number: &str, own: &str, password: &str) -> Vec<(LineType, Usage)> {
        let Ok(sign_in) = format::normalize(number, carrier.sign_in_line(line)) else { return Vec::new() };
        let found = if self.demo {
            demo::other_lines(carrier, line, &sign_in).await
        } else {
            carriers::other_lines(carrier, line, &sign_in, own, password, &self.context(false)).await
        };
        let data = self.data.lock().expect("data lock");
        found
            .into_iter()
            .filter(|(other, usage)| {
                carriers::line_number(carrier, *other, &sign_in, usage)
                    .is_ok_and(|number| !data.accounts.iter().any(|a| a.carrier == carrier && a.line == *other && a.number == number))
            })
            .collect()
    }

    /// Saves a verified number. Returns its id.
    pub fn add_account(
        &self,
        carrier: Carrier,
        line: LineType,
        number: &str,
        password: &str,
        nickname: Option<String>,
        usage: Usage,
    ) -> Result<String, FetchError> {
        let sign_in = format::normalize(number, carrier.sign_in_line(line))?;
        let own = carriers::line_number(carrier, line, &sign_in, &usage)?;
        let mut account = Account::new(carrier, line, own.clone());
        account.login = (own != sign_in).then_some(sign_in);
        account.nickname = nickname.map(|n| n.trim().to_string()).filter(|n| !n.is_empty());
        // New numbers start with the alert settings for all numbers.
        let defaults = self.settings().alerts;
        account.reset_alerts(&defaults);
        account.apply_usage(usage, &defaults);
        let id = account.id.clone();
        self.save_password(&account, password);
        self.update(|d| d.accounts.push(account));
        self.save_vault_index();
        Ok(id)
    }

    fn save_password(&self, account: &Account, password: &str) {
        if !self.demo && !self.passwords.set(account, password) {
            tracing::warn!("password for {} is kept in memory only", account.id);
        }
    }

    /// A number's saved password. Demo numbers have none, so they get a stand-in.
    pub fn password(&self, account: &Account) -> Option<String> {
        if self.demo { Some("demo".to_string()) } else { self.passwords.get(account) }
    }

    /// An export file with the numbers `ids`, their passwords sealed with `passphrase` when
    /// there is one. Making the key takes a moment, so this runs off the UI thread.
    pub fn export(&self, ids: &[String], passphrase: Option<&str>) -> Result<Vec<u8>, TransferError> {
        let data = self.data.lock().expect("data lock").clone();
        let accounts: Vec<&Account> = ids.iter().filter_map(|id| data.account(id)).collect();
        let passwords: BTreeMap<String, String> = match passphrase {
            Some(_) => accounts.iter().filter_map(|a| self.password(a).map(|p| (a.id.clone(), p))).collect(),
            None => BTreeMap::new(),
        };
        Ok(Export::new(&accounts, &self.version, passphrase.map(|p| (p, &passwords)))?.to_json())
    }

    /// Takes in lines from an export file, each with its password if the file had one. A
    /// number already here gets the file's history; a new one is added as it was, archived
    /// or not. Returns their ids.
    pub fn import(&self, lines: Vec<(transfer::Line, Option<String>)>) -> Vec<String> {
        let mut added = Vec::new();
        let ids = self.update(|d| {
            let mut ids = Vec::new();
            for (line, password) in &lines {
                if let Some(account) = d.accounts.iter_mut().find(|a| line.is(a)) {
                    line.merge_into(account);
                    ids.push(account.id.clone());
                } else if let Some(password) = password
                    && line.carrier.supports_line(line.line)
                {
                    let account = line.to_account();
                    ids.push(account.id.clone());
                    added.push((account.clone(), password.clone()));
                    d.accounts.push(account);
                }
            }
            ids
        });
        for (account, password) in &added {
            self.save_password(account, password);
        }
        self.save_vault_index();
        ids
    }

    /// Everything, with every password: for a server's devices, or to copy up to one.
    pub fn copy_out(&self) -> Copy {
        let data = self.data.lock().expect("data lock").clone();
        let passwords = data.accounts.iter().filter_map(|a| self.password(a).map(|p| (a.id.clone(), p))).collect();
        Copy::of(&data, passwords)
    }

    /// Takes in a whole copy from another computer. A line already here keeps its id and
    /// gets the copy's history and, if newer, its reading; a new one comes with its
    /// password. Home tabs and shared settings come from the copy when there were none
    /// here.
    pub fn take_copy(&self, copy: Copy) -> Copied {
        let mut saved = Vec::new();
        let copied = self.update(|d| {
            let fresh = d.accounts.is_empty();
            let mut copied = Copied::default();
            // The copy's ids for lines that are here under their own.
            let mut ids: BTreeMap<String, String> = BTreeMap::new();
            for mut theirs in copy.accounts {
                let password = copy.passwords.get(&theirs.id).cloned();
                if let Some(mine) = d.accounts.iter_mut().find(|a| a.carrier == theirs.carrier && a.line == theirs.line && a.number == theirs.number) {
                    ids.insert(theirs.id.clone(), mine.id.clone());
                    merge_line(mine, theirs);
                    if let Some(password) = password.filter(|_| self.password(mine).is_none()) {
                        saved.push((mine.clone(), password));
                    }
                    copied.merged += 1;
                } else {
                    if d.accounts.iter().any(|a| a.id == theirs.id) {
                        let id = Account::new(theirs.carrier, theirs.line, theirs.number.clone()).id;
                        ids.insert(theirs.id.clone(), id.clone());
                        theirs.id = id;
                    }
                    if let Some(password) = password {
                        saved.push((theirs.clone(), password));
                    }
                    d.accounts.push(theirs);
                    copied.added += 1;
                }
            }
            let map = |id: &str| ids.get(id).cloned().unwrap_or_else(|| id.to_string());
            let had_tabs = d.groups.iter().any(|g| !g.cards.is_empty());
            for mut group in copy.groups {
                for card in &mut group.cards {
                    card.line = map(&card.line);
                }
                if !had_tabs {
                    d.groups.retain(|g| g.id != group.id);
                    d.groups.push(group);
                } else if !d.groups.iter().any(|g| g.id == group.id) {
                    d.groups.push(group);
                }
            }
            if fresh {
                copy.settings.apply_to(&mut d.settings);
            }
            copied
        });
        for (account, password) in &saved {
            self.save_password(account, password);
        }
        self.save_vault_index();
        copied
    }

    /// Makes this the server's copy: its lines, passwords, Home tabs and shared settings,
    /// for a computer switching back from a server. This computer's own settings stay.
    pub fn replace_with(&self, copy: Copy) {
        let old: Vec<Account> = self.data.lock().expect("data lock").accounts.clone();
        if !self.demo {
            for account in old.iter().filter(|a| !copy.accounts.iter().any(|c| c.credential_user() == a.credential_user())) {
                self.passwords.delete(account);
            }
        }
        for account in &copy.accounts {
            if let Some(password) = copy.passwords.get(&account.id) {
                self.save_password(account, password);
            }
        }
        self.update(|d| {
            d.accounts = copy.accounts;
            d.groups = copy.groups;
            copy.settings.apply_to(&mut d.settings);
        });
        self.save_vault_index();
    }

    /// The shared settings, as a server keeps them.
    pub fn shared_settings(&self) -> SharedSettings {
        SharedSettings::of(&self.data.lock().expect("data lock").settings)
    }

    /// Readings a computer took while the server was away: the newer reading wins, the
    /// histories merge, and alerts it showed count as sent.
    pub fn take_readings(&self, readings: Vec<Reading>) {
        self.update(|d| {
            for r in readings {
                let Some(a) = d.account_mut(&r.id) else { continue };
                a.history.merge(&r.history);
                if a.usage.as_ref().is_none_or(|u| u.fetched_at < r.usage.fetched_at) {
                    a.last_attempt = Some(r.usage.fetched_at);
                    a.last_error = None;
                    a.usage = Some(r.usage);
                }
                for key in r.fired {
                    if !a.fired.contains(&key) {
                        a.fired.push(key);
                    }
                }
            }
        });
    }

    /// Removes every line, password and Home tab.
    pub fn erase(&self) {
        let accounts: Vec<Account> = self.data.lock().expect("data lock").accounts.clone();
        if !self.demo {
            for account in &accounts {
                self.passwords.delete(account);
            }
        }
        self.update(|d| {
            d.accounts.clear();
            d.groups.clear();
        });
        self.save_vault_index();
    }

    /// An empty nickname clears it.
    pub fn rename(&self, id: &str, nickname: &str) {
        self.apply(Op::Rename { id: id.into(), nickname: nickname.into() });
    }

    /// Archives a number (it keeps its password but isn't checked or listed), or brings it back.
    pub fn set_archived(&self, id: &str, archived: bool) {
        self.apply(Op::SetArchived { id: id.into(), archived });
    }

    /// `None` follows the interval for all numbers.
    pub fn set_refresh(&self, id: &str, every: Option<klue_core::store::RefreshEvery>) {
        self.apply(Op::SetRefresh { id: id.into(), every });
    }

    /// Puts `carrier` where `onto` is in the carrier rail.
    pub fn move_carrier(&self, carrier: Carrier, onto: Carrier) {
        self.apply(Op::MoveCarrier { carrier, onto });
    }

    /// Puts number `id` where number `onto` is among its carrier's tabs.
    pub fn move_account(&self, id: &str, onto: &str) {
        self.apply(Op::MoveAccount { id: id.into(), onto: onto.into() });
    }

    /// `None` resets to the default main quota.
    pub fn set_main_bundle(&self, id: &str, bundle: Option<String>) {
        self.apply(Op::SetMainBundle { id: id.into(), bundle });
    }

    pub fn remove_account(&self, id: &str) {
        self.apply(Op::RemoveAccount { id: id.into() });
    }

    fn save_vault_index(&self) {
        if !self.demo {
            let accounts = self.data.lock().expect("data lock").accounts.clone();
            self.passwords.save_index(&accounts);
        }
    }

    fn set_refreshing(&self, id: &str, on: bool) -> bool {
        let changed = {
            let mut set = self.refreshing.lock().expect("refreshing lock");
            if on { set.insert(id.to_string()) } else { set.remove(id) }
        };
        if changed {
            self.bump();
        }
        changed
    }

    /// `interactive` refreshes (the Refresh button) may ask a person for a captcha;
    /// background ones don't.
    pub async fn refresh(&self, id: &str, interactive: bool) {
        let Some(account) = self.data.lock().expect("data lock").account(id).cloned() else { return };
        if !account.carrier.supports_line(account.line) || account.archived {
            return;
        }
        let now = Utc::now();
        if account.blocked_until.is_some_and(|until| until > now) {
            tracing::info!("{} is captcha-blocked until {:?}", account.id, account.blocked_until);
            return;
        }
        if interactive && account.last_attempt.is_some_and(|t| now - t < MIN_INTERACTIVE_GAP) && account.last_error.is_none() {
            return;
        }
        if !self.set_refreshing(id, true) {
            return; // already running
        }

        let result = if self.demo {
            demo::refresh(&account).await
        } else {
            match self.passwords.get(&account) {
                None => Err(FetchError::NoPassword),
                Some(password) => carriers::fetch_account(&account, &password, &self.context(interactive)).await,
            }
        };
        if let Err(e) = &result {
            tracing::warn!("refreshing {} failed: {e}", account.id);
        }
        self.update(|d| {
            let defaults = d.settings.alerts;
            if let Some(a) = d.account_mut(id) {
                match result {
                    Ok(usage) => a.apply_usage(usage, &defaults),
                    Err(e) => a.apply_error(e),
                }
            }
        });
        self.set_refreshing(id, false);
    }

    /// Refreshes everything that's due, one at a time. Each number has its own interval, or
    /// the one for all numbers.
    pub async fn refresh_due(&self) {
        let due: Vec<String> = {
            let data = self.data.lock().expect("data lock");
            let now = Utc::now();
            data.accounts
                .iter()
                .filter(|a| a.carrier.supports_line(a.line) && !a.archived)
                // A rejected password stays rejected: retrying in the background could lock the account.
                .filter(|a| !matches!(a.last_error, Some(FetchError::BadCredentials)))
                .filter(|a| {
                    let interval = chrono::Duration::from_std(a.refresh_every(&data.settings).duration()).unwrap_or(chrono::Duration::hours(1));
                    a.last_attempt.is_none_or(|t| now - t >= interval)
                })
                .map(|a| a.id.clone())
                .collect()
        };
        tracing::debug!(count = due.len(), "background refresh");
        for id in due {
            self.refresh(&id, false).await;
        }
    }

    /// Refreshes every number in turn. Numbers already refreshing are skipped, and a second
    /// Refresh all while one runs does nothing.
    pub async fn refresh_all(&self) {
        if self.refreshing_all.swap(true, Ordering::Relaxed) {
            return;
        }
        self.bump();
        let ids: Vec<String> = self.data.lock().expect("data lock").accounts.iter().filter(|a| !a.archived).map(|a| a.id.clone()).collect();
        for id in ids {
            self.refresh(&id, true).await;
        }
        self.refreshing_all.store(false, Ordering::Relaxed);
        self.bump();
    }

    /// Due alerts, marked as sent.
    pub fn take_due_alerts(&self, now: DateTime<Utc>) -> Vec<(Account, AlertEvent)> {
        let due: Vec<(Account, AlertEvent)> = {
            let data = self.data.lock().expect("data lock");
            data.accounts
                .iter()
                .filter(|a| !a.archived)
                .flat_map(|a| alerts::due(a, now).into_iter().map(move |e| (a.clone(), e)))
                .collect()
        };
        if !due.is_empty() {
            self.update(|d| {
                for (acc, event) in &due {
                    if let Some(a) = d.account_mut(&acc.id) {
                        a.mark_fired(event.key());
                    }
                }
            });
        }
        due
    }
}

/// A line from another copy, merged into this one: both histories, the newer reading, and
/// the alerts either has sent. Names and settings set here stay.
fn merge_line(mine: &mut Account, theirs: Account) {
    mine.history.merge(&theirs.history);
    let newer = match (&mine.usage, &theirs.usage) {
        (Some(m), Some(t)) => t.fetched_at > m.fetched_at,
        (None, Some(_)) => true,
        _ => false,
    };
    if newer {
        mine.usage = theirs.usage;
        mine.last_attempt = theirs.last_attempt;
        mine.last_error = theirs.last_error;
        mine.owner_name = theirs.owner_name.or(mine.owner_name.take());
    }
    if mine.nickname.is_none() {
        mine.nickname = theirs.nickname;
    }
    for key in theirs.fired {
        if !mine.fired.contains(&key) {
            mine.fired.push(key);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use klue_core::store::{Card, Group, VaultAccount};

    use super::*;

    /// Passwords kept in memory, for tests.
    #[derive(Default)]
    struct Memory(Mutex<HashMap<String, String>>);

    impl PasswordStore for Memory {
        fn get(&self, account: &Account) -> Option<String> {
            self.0.lock().unwrap().get(&account.credential_user()).cloned()
        }
        fn set(&self, account: &Account, password: &str) -> bool {
            self.0.lock().unwrap().insert(account.credential_user(), password.into());
            true
        }
        fn delete(&self, account: &Account) {
            self.0.lock().unwrap().remove(&account.credential_user());
        }
        fn save_index(&self, _: &[Account]) {}
        fn load_index(&self) -> Vec<VaultAccount> {
            Vec::new()
        }
    }

    fn engine(name: &str) -> Arc<Engine> {
        let dir = std::env::temp_dir().join(format!("klue-engine-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Engine::new(Store::open(dir).unwrap(), Box::<Memory>::default(), false, "test")
    }

    #[test]
    fn copies_merge_lines_both_sides_have_and_bring_the_rest_with_passwords() {
        let demo = demo::data();
        let (home, phone) = (demo.accounts[0].clone(), demo.accounts[2].clone());

        // The server has the home internet, with its own id and an older reading.
        let server = engine("server");
        let mut older = home.clone();
        older.id = "server-id".into();
        if let Some(u) = older.usage.as_mut() {
            u.fetched_at -= chrono::Duration::days(1);
        }
        server.update(|d| d.accounts.push(older));

        // A computer copies up both lines and a Home tab with both.
        let tab = Group { id: "tab".into(), name: Some("Mine".into()), cards: vec![Card::main(&home.id), Card::main(&phone.id)] };
        let copy = Copy {
            accounts: vec![home.clone(), phone.clone()],
            passwords: [(home.id.clone(), "home-pass".to_string()), (phone.id.clone(), "phone-pass".to_string())].into(),
            groups: vec![tab],
            settings: SharedSettings::of(&demo.settings),
        };
        let copied = server.take_copy(copy);
        assert_eq!((copied.added, copied.merged), (1, 1));

        let data = server.snapshot().data;
        assert_eq!(data.accounts.len(), 2);
        let merged = data.account("server-id").expect("kept its id");
        assert_eq!(merged.usage.as_ref().map(|u| u.fetched_at), home.usage.as_ref().map(|u| u.fetched_at), "the newer reading wins");
        // The tab points at the server's id for the line it already had.
        let lines: Vec<&str> = data.groups.iter().find(|g| g.id == "tab").unwrap().cards.iter().map(|c| c.line.as_str()).collect();
        assert_eq!(lines, ["server-id", phone.id.as_str()]);
        assert_eq!(server.password(data.account(&phone.id).unwrap()).as_deref(), Some("phone-pass"));
        assert_eq!(server.password(merged).as_deref(), Some("home-pass"));

        // And back down: a computer switching back gets the server's copy.
        let computer = engine("computer");
        computer.replace_with(server.copy_out());
        let back = computer.snapshot().data;
        assert_eq!(back.accounts.len(), 2);
        assert_eq!(computer.password(back.account(&phone.id).unwrap()).as_deref(), Some("phone-pass"));

        server.erase();
        assert!(server.snapshot().data.accounts.is_empty());
        assert_eq!(server.password(&phone), None);
    }
}
