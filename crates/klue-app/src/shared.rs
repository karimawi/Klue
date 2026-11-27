//! State shared by the UI and background work: data file, passwords, refreshes, alerts.
//! Windows watch a revision counter and re-read a [`Snapshot`].

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
    store::{Account, AppData, RefreshEvery, Secrets, Settings, Store},
    transfer::{self, Export, TransferError},
};
use tokio::sync::{mpsc, oneshot, watch};

use crate::demo;

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

/// A captcha the user is asked to type.
pub struct CaptchaRequest {
    pub image: Vec<u8>,
    pub reply: oneshot::Sender<Option<String>>,
}

/// Captcha "solver" that asks the user.
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

pub struct Shared {
    store: Store,
    secrets: Secrets,
    data: Mutex<AppData>,
    refreshing: Mutex<BTreeSet<String>>,
    refreshing_all: AtomicBool,
    revision: watch::Sender<u64>,
    ocr: Option<Arc<dyn CaptchaSolver>>,
    prompt: Arc<dyn CaptchaSolver>,
    captcha_rx: Mutex<Option<mpsc::UnboundedReceiver<CaptchaRequest>>>,
    debug_dir: Option<PathBuf>,
    pub demo: bool,
}

/// How often the background loop checks for due refreshes and reminders.
pub const TICK: Duration = Duration::from_secs(60);
/// Minimum time between sign-ins, even on demand (WE answers rapid ones with captchas).
const MIN_INTERACTIVE_GAP: chrono::Duration = chrono::Duration::seconds(20);

impl Shared {
    pub fn new(store: Store, demo: bool) -> Arc<Self> {
        let secrets = Secrets::new();
        let data = if demo {
            demo::data()
        } else {
            let mut data = store.load();
            // Accounts in the vault but not in the data file come back, without their history.
            let restored = data.restore(secrets.load_index());
            if restored > 0 {
                tracing::info!("restored {restored} account(s) from the credential store");
                if let Err(e) = store.save(&data) {
                    tracing::error!("couldn't save {}: {e}", store.dir().display());
                }
            }
            secrets.save_index(&data.accounts);
            data
        };
        let (tx, rx) = mpsc::unbounded_channel();
        Arc::new(Self {
            secrets,
            data: Mutex::new(data),
            refreshing: Mutex::new(BTreeSet::new()),
            refreshing_all: AtomicBool::new(false),
            revision: watch::channel(0).0,
            ocr: if demo { None } else { default_solver() },
            prompt: Arc::new(PromptSolver { tx }),
            captcha_rx: Mutex::new(Some(rx)),
            debug_dir: std::env::var_os("KLUE_DEBUG_DIR").map(PathBuf::from),
            store,
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

    /// Changes the data, saves it and notifies the windows.
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

    pub fn settings(&self) -> Settings {
        self.data.lock().expect("data lock").settings.clone()
    }

    pub fn take_captcha_requests(&self) -> Option<mpsc::UnboundedReceiver<CaptchaRequest>> {
        self.captcha_rx.lock().ok()?.take()
    }

    /// Captchas go to the model, and WE is asked for another when it isn't sure. The user is
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
        let already = |archived: bool| {
            let s = crate::i18n::strings(self.settings().lang);
            FetchError::Carrier(if archived { s.already_archived } else { s.already_added }.into())
        };
        if carrier.sign_in_line(line) == line
            && let Some(archived) = added(line, &sign_in)
        {
            return Err(already(archived));
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
            return others.into_iter().find(|(other, _)| *other == line).map(|(_, usage)| usage).ok_or_else(|| already(archived));
        }
        Ok(usage)
    }

    /// The lines on the account just verified that Klue doesn't have yet (see
    /// [`carriers::other_lines`]).
    /// `own` is the number of the line just read.
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
        if !self.demo && !self.secrets.set(&account, password) {
            tracing::warn!("password for {id} is kept in memory only");
        }
        self.update(|d| d.accounts.push(account));
        self.save_vault_index();
        Ok(id)
    }

    /// An export file with the numbers `ids`, their passwords sealed with `passphrase` when
    /// there is one. Making the key takes a moment, so this runs off the UI thread.
    pub fn export(&self, ids: &[String], passphrase: Option<&str>) -> Result<Vec<u8>, TransferError> {
        let data = self.data.lock().expect("data lock").clone();
        let accounts: Vec<&Account> = ids.iter().filter_map(|id| data.account(id)).collect();
        let passwords: BTreeMap<String, String> = match passphrase {
            // Demo numbers have no saved passwords.
            Some(_) => accounts
                .iter()
                .filter_map(|a| if self.demo { Some("demo".to_string()) } else { self.secrets.get(a) }.map(|p| (a.id.clone(), p)))
                .collect(),
            None => BTreeMap::new(),
        };
        Ok(Export::new(&accounts, crate::about::VERSION, passphrase.map(|p| (p, &passwords)))?.to_json())
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
            if !self.demo && !self.secrets.set(account, password) {
                tracing::warn!("password for {} is kept in memory only", account.id);
            }
        }
        self.save_vault_index();
        ids
    }

    /// An empty nickname clears it.
    pub fn rename(&self, id: &str, nickname: &str) {
        let nickname = Some(nickname.trim().to_string()).filter(|n| !n.is_empty());
        self.update(|d| {
            if let Some(a) = d.account_mut(id) {
                a.nickname = nickname;
            }
        });
        self.save_vault_index();
    }

    /// Archives a number (it keeps its password but isn't checked or listed), or brings it back.
    pub fn set_archived(&self, id: &str, archived: bool) {
        self.update(|d| {
            if let Some(a) = d.account_mut(id) {
                a.archived = archived;
            }
        });
        self.save_vault_index();
    }

    /// `None` follows the interval for all numbers.
    pub fn set_refresh(&self, id: &str, every: Option<RefreshEvery>) {
        self.update(|d| {
            if let Some(a) = d.account_mut(id) {
                a.refresh = every;
            }
        });
    }

    /// Puts `carrier` where `onto` is in the carrier rail.
    pub fn move_carrier(&self, carrier: Carrier, onto: Carrier) {
        self.update(|d| d.move_carrier(carrier, onto));
    }

    /// Puts number `id` where number `onto` is among its carrier's tabs.
    pub fn move_account(&self, id: &str, onto: &str) {
        self.update(|d| d.move_account(id, onto));
        self.save_vault_index();
    }

    /// `None` resets to the default main quota.
    pub fn set_main_bundle(&self, id: &str, bundle: Option<String>) {
        self.update(|d| {
            if let Some(a) = d.account_mut(id) {
                a.main_bundle = bundle;
            }
        });
    }

    fn save_vault_index(&self) {
        if !self.demo {
            let accounts = self.data.lock().expect("data lock").accounts.clone();
            self.secrets.save_index(&accounts);
        }
    }

    pub fn remove_account(&self, id: &str) {
        let account = self.data.lock().expect("data lock").account(id).cloned();
        if !self.demo
            && let Some(account) = &account
        {
            self.secrets.delete(account);
        }
        self.update(|d| d.accounts.retain(|a| a.id != id));
        self.save_vault_index();
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

    /// `interactive` refreshes (the Refresh button) may ask the user for a captcha; background
    /// ones don't.
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
            match self.secrets.get(&account) {
                None => Err(FetchError::Carrier(crate::i18n::strings(self.settings().lang).no_password.into())),
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

    /// Due alerts, marked as shown.
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
