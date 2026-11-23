//! State shared by the UI and background work: data file, passwords, refreshes, alerts.
//! Windows watch a revision counter and re-read a [`Snapshot`].

use std::{
    collections::BTreeSet,
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
    store::{Account, AppData, Secrets, Settings, Store},
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

    /// Signs in with new credentials (Add a number).
    pub async fn verify(&self, carrier: Carrier, line: LineType, number: &str, password: &str) -> Result<Usage, FetchError> {
        let number = format::normalize(number, line)?;
        if self.data.lock().expect("data lock").accounts.iter().any(|a| a.carrier == carrier && a.number == number) {
            return Err(FetchError::Carrier(crate::i18n::strings(self.settings().lang).already_added.into()));
        }
        if self.demo {
            return demo::verify(carrier, &number, password).await;
        }
        carriers::fetch(carrier, line, &number, password, &self.context(true)).await
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
        let number = format::normalize(number, line)?;
        let mut account = Account::new(carrier, line, number);
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
        if !account.carrier.supports_line(account.line) {
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
                Some(password) => {
                    carriers::fetch(account.carrier, account.line, &account.number, &password, &self.context(interactive)).await
                }
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

    /// Refreshes everything that's due, one at a time.
    pub async fn refresh_due(&self) {
        let (interval, due): (chrono::Duration, Vec<String>) = {
            let data = self.data.lock().expect("data lock");
            let interval = chrono::Duration::from_std(data.settings.refresh.duration()).unwrap_or(chrono::Duration::hours(1));
            let now = Utc::now();
            let due = data
                .accounts
                .iter()
                .filter(|a| a.carrier.supports_line(a.line))
                // A rejected password stays rejected: retrying in the background could lock the account.
                .filter(|a| !matches!(a.last_error, Some(FetchError::BadCredentials)))
                .filter(|a| a.last_attempt.is_none_or(|t| now - t >= interval))
                .map(|a| a.id.clone())
                .collect();
            (interval, due)
        };
        tracing::debug!(?interval, count = due.len(), "background refresh");
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
        let ids: Vec<String> = self.data.lock().expect("data lock").accounts.iter().map(|a| a.id.clone()).collect();
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
