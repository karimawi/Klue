//! State shared by every window: the data file, passwords, refreshes and alerts.
//! Windows subscribe to revision bumps and re-read a [`Snapshot`].

use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use chrono::{DateTime, Utc};
use klue_core::{
    Carrier, FetchError, LineType, Usage,
    alerts::{self, AlertEvent},
    captcha::{BoxFuture, CaptchaSolver, SolverChain, default_ocr},
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

/// Something another window (the tray widget) wants the main window to do.
#[derive(Debug, Clone, PartialEq)]
pub enum UiRequest {
    Show,
    OpenSettings,
    ShowAccount(String),
}

/// Captcha "solver" that asks the person in front of the screen.
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
    revision: watch::Sender<u64>,
    ocr: Option<Arc<dyn CaptchaSolver>>,
    prompt: Arc<dyn CaptchaSolver>,
    captcha_rx: Mutex<Option<mpsc::UnboundedReceiver<CaptchaRequest>>>,
    ui_tx: mpsc::UnboundedSender<UiRequest>,
    ui_rx: Mutex<Option<mpsc::UnboundedReceiver<UiRequest>>>,
    debug_dir: Option<PathBuf>,
    pub demo: bool,
}

/// How often the background loop wakes up to look for due refreshes and reminders.
pub const TICK: Duration = Duration::from_secs(60);
/// Don't re-sign-in more often than this, even on demand (WE answers rapid sign-ins with captchas).
const MIN_INTERACTIVE_GAP: chrono::Duration = chrono::Duration::seconds(20);

impl Shared {
    pub fn new(store: Store, demo: bool) -> Arc<Self> {
        let data = if demo { demo::data() } else { store.load() };
        let (tx, rx) = mpsc::unbounded_channel();
        let (ui_tx, ui_rx) = mpsc::unbounded_channel();
        Arc::new(Self {
            secrets: Secrets::new(),
            data: Mutex::new(data),
            refreshing: Mutex::new(BTreeSet::new()),
            revision: watch::channel(0).0,
            ocr: if demo { None } else { default_ocr() },
            prompt: Arc::new(PromptSolver { tx }),
            captcha_rx: Mutex::new(Some(rx)),
            ui_tx,
            ui_rx: Mutex::new(Some(ui_rx)),
            debug_dir: std::env::var_os("KLUE_DEBUG_DIR").map(PathBuf::from),
            store,
            demo,
        })
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            data: self.data.lock().expect("data lock").clone(),
            refreshing: self.refreshing.lock().expect("refreshing lock").clone(),
            demo: self.demo,
        }
    }

    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.revision.subscribe()
    }

    fn bump(&self) {
        self.revision.send_modify(|r| *r += 1);
    }

    /// Change the data, save it and tell every window.
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

    pub fn take_ui_requests(&self) -> Option<mpsc::UnboundedReceiver<UiRequest>> {
        self.ui_rx.lock().ok()?.take()
    }

    pub fn request_ui(&self, request: UiRequest) {
        let _ = self.ui_tx.send(request);
    }

    fn solver(&self, interactive: bool, manual_first: bool) -> SolverChain {
        let mut chain: Vec<Arc<dyn CaptchaSolver>> = Vec::new();
        if !manual_first {
            chain.extend(self.ocr.clone());
        }
        if interactive {
            chain.push(self.prompt.clone());
        }
        SolverChain(chain)
    }

    fn context<'a>(&self, solver: &'a SolverChain) -> FetchContext<'a> {
        FetchContext { solver: Some(solver), debug_dir: self.debug_dir.clone() }
    }

    /// Sign in with new credentials (the add-number flow). `manual_captcha` skips OCR,
    /// for retrying after WE rejected OCR's answer.
    pub async fn verify(
        &self,
        carrier: Carrier,
        line: LineType,
        number: &str,
        password: &str,
        manual_captcha: bool,
    ) -> Result<Usage, FetchError> {
        let number = format::normalize(number, line)?;
        if self.data.lock().expect("data lock").accounts.iter().any(|a| a.carrier == carrier && a.number == number) {
            return Err(FetchError::Carrier(crate::i18n::strings(self.settings().lang).already_added.into()));
        }
        if self.demo {
            return demo::verify(carrier, &number, password).await;
        }
        let solver = self.solver(true, manual_captcha);
        carriers::fetch(carrier, line, &number, password, &self.context(&solver)).await
    }

    /// Save a verified number. Returns the new account's id.
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
        account.apply_usage(usage);
        let id = account.id.clone();
        if !self.demo && !self.secrets.set(&id, password) {
            tracing::warn!("password for {id} is kept in memory only");
        }
        self.update(|d| d.accounts.push(account));
        Ok(id)
    }

    pub fn remove_account(&self, id: &str) {
        if !self.demo {
            self.secrets.delete(id);
        }
        self.update(|d| d.accounts.retain(|a| a.id != id));
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

    /// Refresh one account. `interactive` refreshes (the Refresh button) may ask the user
    /// to type a captcha; background ones only use OCR.
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
            match self.secrets.get(id) {
                None => Err(FetchError::Carrier(crate::i18n::strings(self.settings().lang).no_password.into())),
                Some(password) => {
                    let manual_first = interactive && matches!(account.last_error, Some(FetchError::CaptchaUnsolved));
                    let solver = self.solver(interactive, manual_first);
                    carriers::fetch(account.carrier, account.line, &account.number, &password, &self.context(&solver)).await
                }
            }
        };
        if let Err(e) = &result {
            tracing::warn!("refreshing {} failed: {e}", account.id);
        }
        self.update(|d| {
            if let Some(a) = d.account_mut(id) {
                match result {
                    Ok(usage) => a.apply_usage(usage),
                    Err(e) => a.apply_error(e),
                }
            }
        });
        self.set_refreshing(id, false);
    }

    /// Refresh everything that's due, one account at a time.
    pub async fn refresh_due(&self) {
        let (interval, due): (chrono::Duration, Vec<String>) = {
            let data = self.data.lock().expect("data lock");
            let interval = chrono::Duration::from_std(data.settings.refresh.duration()).unwrap_or(chrono::Duration::hours(1));
            let now = Utc::now();
            let due = data
                .accounts
                .iter()
                .filter(|a| a.carrier.supports_line(a.line))
                // A rejected password stays rejected; retrying it in the background could
                // get the account locked. The user retries with the Refresh button.
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

    pub async fn refresh_all(&self) {
        let ids: Vec<String> = self.data.lock().expect("data lock").accounts.iter().map(|a| a.id.clone()).collect();
        for id in ids {
            self.refresh(&id, true).await;
        }
    }

    /// Alerts that are due now; they're marked as shown.
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
