//! The iced program. It runs as a daemon: the tray icon and refreshes live all session,
//! and windows are destroyed when hidden.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};

use chrono::{DateTime, Utc};
use iced::{
    Point, Size, Subscription, Task,
    font::Weight,
    futures::{SinkExt, Stream},
    widget::operation,
    window,
};
use klue_core::{
    Carrier,
    store::{Account, Lang, Theme},
};
use tokio::sync::oneshot;

use klue_core::update::{Release, Version};

use crate::{
    about, assets, autostart, i18n, notify, updater,
    shared::{Shared, Snapshot, TICK},
    systray::{self, Anchor, Tray},
    ui::{self, modals::Modal},
};

/// Shared state, for subscriptions (which can't capture).
pub static SHARED: OnceLock<Arc<Shared>> = OnceLock::new();

const MAIN_SIZE: Size = Size::new(1200.0, 760.0);
const MAIN_MIN: Size = Size::new(960.0, 640.0);
pub const POPUP_SIZE: Size = Size::new(360.0, 560.0);

/// A captcha waiting for the user to type it.
#[derive(Debug, Clone)]
pub struct CaptchaAsk {
    pub image: iced::widget::image::Handle,
    pub input: String,
    reply: Arc<Mutex<Option<oneshot::Sender<Option<String>>>>>,
}

impl CaptchaAsk {
    fn answer(&self, code: Option<String>) {
        if let Some(reply) = self.reply.lock().ok().and_then(|mut r| r.take()) {
            let _ = reply.send(code);
        }
    }
}

/// What can be dragged to a new place: a carrier in the rail, or a number among its
/// carrier's tabs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DragItem {
    Carrier(Carrier),
    Line(String),
}

/// A drag in progress: `over` is where it would land.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Drag {
    pub item: DragItem,
    pub over: DragItem,
}

#[derive(Debug, Clone)]
pub enum Message {
    Init,
    /// The shared data changed.
    Revision,
    Tick,
    /// Refreshes finished; their alerts go out now.
    Refreshed,
    Window(window::Id, window::Event),
    Tray(systray::Event),
    /// Another Klue was started.
    SecondInstance(crate::instance::Request),
    Captcha(CaptchaAsk),
    CaptchaInput(String),
    CaptchaSubmit,
    CaptchaCancel,
    PopupPlaced(window::Id, f32, Option<Size>),

    // Window chrome.
    Drag,
    DragResize(window::Direction),
    Minimize,
    ToggleMaximize,
    CloseMain,
    Quit,

    // Navigation and data.
    ShowMain(Option<String>),
    /// The popup's settings button.
    PopupSettings,
    SelectCarrier(Carrier),
    SelectAccount(String),
    /// A carrier or tab was pressed: a click, or the start of a drag.
    DragStart(DragItem),
    /// The pointer went over a carrier or tab.
    DragOver(DragItem),
    /// The left button came up anywhere: drops a drag, or picks what was clicked.
    PointerUp,
    SetTheme(Theme),
    SetLang(Lang),
    Refresh(String),
    RefreshAll,
    SetMain(String, Option<String>),
    /// Quota card arrows: next (`true`) or previous bundle.
    HeroStep(String, bool),
    /// Usage graph: days (`false`) or hours (`true`).
    UsageHours(bool),
    OpenModal(ModalKind),
    CloseModal,
    Modal(ui::modals::Msg),
    /// Tab and Shift+Tab move between fields.
    FocusNext,
    FocusPrevious,
    /// Closes the popup, the captcha prompt or the open dialog.
    Escape(window::Id),
    /// The disclaimer: accept, or quit.
    AcceptDisclaimer,
    /// The debug console (--debug).
    OpenConsole,
    ConsoleTick,
    ConsoleLevel(tracing::Level),
    ConsoleSearch(String),
    ConsoleFollow,
    ConsoleCopy,
    ConsoleClear,
    OpenUrl(&'static str),
    OpenUrlOwned(String),
    /// `manual` from About; also shows a skipped release.
    CheckUpdates { manual: bool },
    UpdateChecked { manual: bool, result: Result<Option<Box<Release>>, String> },
    ShowUpdate,
    UpdateLater,
    UpdateSkip,
    UpdateInstall,
    UpdateProgress(u64, Option<u64>),
    UpdateDownloaded(Result<std::path::PathBuf, String>),
    UpdateInstalled(Result<updater::Outcome, String>),
    Done,
}

/// Update check and install state.
#[derive(Debug, Clone, PartialEq)]
pub enum UpdateState {
    Idle,
    Checking,
    UpToDate,
    CheckFailed,
    /// A newer release from `main`; `shown` while its dialog is open.
    Available { release: Box<Release>, shown: bool },
    Downloading { release: Box<Release>, got: u64, total: Option<u64> },
    Installing { release: Box<Release> },
    /// Opened in the system installer (disk image or package).
    Opened { release: Box<Release> },
    Failed { release: Box<Release>, message: String },
}

impl UpdateState {
    pub fn release(&self) -> Option<&Release> {
        match self {
            UpdateState::Available { release, .. }
            | UpdateState::Downloading { release, .. }
            | UpdateState::Installing { release }
            | UpdateState::Opened { release }
            | UpdateState::Failed { release, .. } => Some(release),
            _ => None,
        }
    }

    /// Whether the update dialog is up.
    pub fn dialog(&self) -> bool {
        !matches!(self, UpdateState::Idle | UpdateState::Checking | UpdateState::UpToDate | UpdateState::CheckFailed | UpdateState::Available { shown: false, .. })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ModalKind {
    /// The Add wizard, from the kind of line when the carrier is given.
    Add(Option<Carrier>),
    Rename(String),
    Alerts(String),
    Settings,
    /// The numbers put away with Archive.
    Archived,
}

pub struct App {
    pub shared: Arc<Shared>,
    pub snap: Snapshot,
    pub now: DateTime<Utc>,
    pub main: Option<window::Id>,
    pub popup: Option<window::Id>,
    popup_anchor: Option<Anchor>,
    /// The popup closes when it loses focus, but only after it had it.
    popup_focused: bool,
    popup_closed_at: Option<Instant>,
    tray: Option<Tray>,
    tray_lang: Lang,
    start_minimized: bool,
    start_quick_view: bool,
    /// The carrier shown, and the number picked per carrier.
    pub carrier: Carrier,
    pub selected: HashMap<Carrier, String>,
    /// The bundle each number's quota card shows, by key.
    pub hero: HashMap<String, String>,
    /// Prefer the hours view when it's available.
    pub usage_hours: bool,
    /// A carrier or tab being dragged to a new place.
    pub drag: Option<Drag>,
    pub modal: Modal,
    pub captcha: Option<CaptchaAsk>,
    pub update: UpdateState,
    /// Main window size, for fitting dialogs.
    pub main_size: Size,
    /// Time of the last tick. A much longer gap means the computer slept.
    last_tick: DateTime<Utc>,
    /// `--debug`: shows the console button.
    pub debug: bool,
    pub console: Option<window::Id>,
    pub log: LogView,
}

/// Debug console state.
#[derive(Debug, Clone, PartialEq)]
pub struct LogView {
    pub lines: Vec<crate::console::Line>,
    pub revision: u64,
    /// Lowest level shown.
    pub level: tracing::Level,
    pub search: String,
    /// Stick to the newest line.
    pub follow: bool,
}

impl Default for LogView {
    fn default() -> Self {
        Self { lines: Vec::new(), revision: u64::MAX, level: tracing::Level::TRACE, search: String::new(), follow: true }
    }
}

/// Lines shown at once.
const CONSOLE_LINES: usize = 1500;
pub const CONSOLE_SCROLL: &str = "console-log";

pub fn run(shared: Arc<Shared>, minimized: bool, quick_view: bool, debug: bool) -> iced::Result {
    let _ = SHARED.set(shared.clone());
    let mut daemon = iced::daemon(move || App::new(shared.clone(), minimized, quick_view, debug), App::update, App::view)
        .title(App::title)
        .theme(App::theme)
        .subscription(App::subscription)
        .default_font(assets::font(Lang::En, Weight::Normal))
        .antialiasing(true);
    for font in assets::FONTS {
        daemon = daemon.font(font);
    }
    daemon.run()
}

impl App {
    fn new(shared: Arc<Shared>, minimized: bool, quick_view: bool, debug: bool) -> (Self, Task<Message>) {
        let snap = shared.snapshot();
        let carrier = snap.data.carriers().into_iter().find(|c| snap.data.accounts_for(*c).next().is_some()).unwrap_or(Carrier::We);
        let tray_lang = snap.data.settings.lang;
        let app = Self {
            shared,
            snap,
            now: Utc::now(),
            main: None,
            popup: None,
            popup_anchor: None,
            popup_focused: false,
            popup_closed_at: None,
            tray: None,
            tray_lang,
            start_minimized: minimized,
            start_quick_view: quick_view,
            carrier,
            selected: HashMap::new(),
            hero: HashMap::new(),
            usage_hours: false,
            drag: None,
            modal: Modal::None,
            captcha: None,
            update: UpdateState::Idle,
            main_size: MAIN_SIZE,
            last_tick: Utc::now(),
            debug,
            console: None,
            log: LogView::default(),
        };
        // Create the tray icon on the first update: macOS needs the event loop running.
        (app, Task::done(Message::Init))
    }

    pub fn lang(&self) -> Lang {
        self.snap.data.settings.lang
    }

    /// This version's disclaimer hasn't been accepted. No sign-ins until it is.
    pub fn disclaimer_due(&self) -> bool {
        !self.shared.demo && self.snap.data.settings.accepted_disclaimer.as_deref() != Some(about::VERSION)
    }

    /// The picked carrier if it has numbers, else the first one that does.
    pub fn shown_carrier(&self) -> Option<Carrier> {
        let data = &self.snap.data;
        let has_numbers = |c: &Carrier| data.accounts_for(*c).next().is_some();
        Some(self.carrier).filter(has_numbers).or_else(|| data.carriers().into_iter().find(has_numbers))
    }

    /// The shown account.
    pub fn current(&self) -> Option<&Account> {
        let data = &self.snap.data;
        let carrier = self.shown_carrier()?;
        let picked = self.selected.get(&carrier).and_then(|id| data.account(id)).filter(|a| a.carrier == carrier && !a.archived);
        picked.or_else(|| data.accounts_for(carrier).next())
    }

    fn title(&self, window: window::Id) -> String {
        if Some(window) == self.console { "Klue debug console".into() } else { "Klue".into() }
    }

    /// The window background. The renderer clears to it, so views don't paint a full-window
    /// quad (tiny-skia redraws those through a clip mask on every scroll frame).
    fn theme(&self, window: window::Id) -> iced::Theme {
        let popup = Some(window) == self.popup;
        let carrier = if popup { Carrier::We } else { self.shown_carrier().unwrap_or(Carrier::We) };
        let p = crate::theme::palette(self.snap.data.settings.theme, carrier, popup);
        let base = if p.bg.r + p.bg.g + p.bg.b < 1.5 { iced::Theme::Dark } else { iced::Theme::Light };
        iced::Theme::custom("Klue", iced::theme::Palette { background: p.bg, text: p.tx, ..base.palette() })
    }

    fn view(&self, window: window::Id) -> ui::El<'_> {
        if Some(window) == self.popup {
            ui::popup::view(self)
        } else if Some(window) == self.console {
            ui::console::view(self)
        } else if Some(window) == self.main {
            ui::window::view(self)
        } else {
            iced::widget::Space::new().into()
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            window::events().map(|(id, e)| Message::Window(id, e)),
            iced::time::every(TICK).map(|_| Message::Tick),
            // Keep countdowns and "synced 2 min ago" current.
            if self.main.is_some() || self.popup.is_some() {
                iced::time::every(Duration::from_secs(30)).map(|_| Message::Tick)
            } else {
                Subscription::none()
            },
            // The console polls the log four times a second.
            if self.console.is_some() {
                iced::time::every(Duration::from_millis(250)).map(|_| Message::ConsoleTick)
            } else {
                Subscription::none()
            },
            Subscription::run(revisions),
            Subscription::run(captchas),
            Subscription::run(systray::events).map(Message::Tray),
            Subscription::run(crate::instance::requests).map(Message::SecondInstance),
            iced::event::listen_with(keys),
        ])
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Init => {
                self.tray = systray::create(self.lang());
                let shared = self.shared.clone();
                let refresh = if self.disclaimer_due() {
                    Task::none()
                } else {
                    Task::perform(async move { shared.refresh_due().await }, |_| Message::Refreshed)
                };
                // The disclaimer needs a window, even when starting in the tray.
                let window = if self.disclaimer_due() {
                    self.show_main(None)
                } else if self.start_quick_view {
                    self.toggle_popup(None)
                } else if self.start_minimized && self.tray.is_some() {
                    Task::none()
                } else {
                    self.show_main(None)
                };
                // Reminders that came due while closed go out now; quota alerts follow the refresh.
                self.send_due_alerts();
                // Check for updates once the first refreshes have started.
                let check = if self.shared.demo {
                    Task::none()
                } else {
                    Task::perform(tokio::time::sleep(Duration::from_secs(5)), |_| Message::CheckUpdates { manual: false })
                };
                Task::batch([window, refresh, check])
            }
            Message::Revision => {
                self.snap = self.shared.snapshot();
                let lang = self.lang();
                if lang != self.tray_lang {
                    self.tray_lang = lang;
                    if let Some(tray) = &self.tray {
                        tray.set_lang(lang);
                    }
                }
                Task::none()
            }
            Message::Tick => {
                self.now = Utc::now();
                // Timers stop during sleep, so the first tick after waking is far ahead: send what came
                // due and refresh everything.
                let gap = self.now - self.last_tick;
                self.last_tick = self.now;
                if gap > chrono::Duration::from_std(TICK * 3).unwrap_or_default() {
                    tracing::info!("woke up after {} min", gap.num_minutes());
                }
                if self.disclaimer_due() {
                    return Task::none();
                }
                self.send_due_alerts();
                let shared = self.shared.clone();
                Task::perform(async move { shared.refresh_due().await }, |_| Message::Refreshed)
            }
            Message::Refreshed => {
                self.send_due_alerts();
                Task::none()
            }
            Message::Window(id, event) => self.window_event(id, event),
            Message::Tray(event) => self.tray_event(event),
            Message::SecondInstance(crate::instance::Request::Show) => self.show_main(None),
            Message::SecondInstance(crate::instance::Request::QuickView) => self.toggle_popup(None),
            Message::Captcha(ask) => {
                if let Some(old) = self.captcha.replace(ask) {
                    old.answer(None);
                }
                Task::batch([self.show_main(None), operation::focus(ui::modals::CAPTCHA_INPUT)])
            }
            Message::CaptchaInput(text) => {
                if let Some(c) = &mut self.captcha {
                    c.input = text.chars().filter(|c| c.is_ascii_alphanumeric()).take(8).collect();
                }
                Task::none()
            }
            Message::CaptchaSubmit => {
                if let Some(c) = self.captcha.take() {
                    let code = c.input.trim().to_string();
                    c.answer(Some(code).filter(|c| !c.is_empty()));
                }
                Task::none()
            }
            Message::CaptchaCancel => {
                if let Some(c) = self.captcha.take() {
                    c.answer(None);
                }
                Task::none()
            }
            Message::PopupPlaced(id, scale, monitor) => self.place_popup(id, scale, monitor),

            Message::Drag => self.on_main(window::drag),
            Message::DragResize(direction) => self.on_main(move |id| window::drag_resize(id, direction)),
            Message::Minimize => self.on_main(|id| window::minimize(id, true)),
            Message::ToggleMaximize => self.on_main(window::toggle_maximize),
            Message::CloseMain => self.close_main(),
            Message::Quit => {
                if let Some(c) = self.captcha.take() {
                    c.answer(None);
                }
                crate::sound::cleanup();
                iced::exit()
            }

            Message::ShowMain(account) => {
                let close = self.popup.map(window::close).unwrap_or_else(Task::none);
                Task::batch([close, self.show_main(account)])
            }
            Message::PopupSettings => {
                let show = self.update(Message::ShowMain(None));
                self.modal = Modal::open(ModalKind::Settings, &self.snap);
                show
            }
            Message::SelectCarrier(carrier) => {
                self.carrier = carrier;
                Task::none()
            }
            Message::SelectAccount(id) => {
                if let Some(account) = self.snap.data.account(&id) {
                    self.carrier = account.carrier;
                    self.selected.insert(account.carrier, id);
                }
                Task::none()
            }
            Message::DragStart(item) => {
                self.drag = Some(Drag { over: item.clone(), item });
                Task::none()
            }
            Message::DragOver(over) => {
                if let Some(drag) = &mut self.drag
                    && matches!((&drag.item, &over), (DragItem::Carrier(_), DragItem::Carrier(_)) | (DragItem::Line(_), DragItem::Line(_)))
                {
                    drag.over = over;
                }
                Task::none()
            }
            Message::PointerUp => {
                let Some(Drag { item, over }) = self.drag.take() else { return Task::none() };
                // Dropped somewhere else: it moves there. Either way it's picked, like a click.
                match (item, over) {
                    (DragItem::Carrier(carrier), DragItem::Carrier(onto)) => {
                        if carrier != onto {
                            self.shared.move_carrier(carrier, onto);
                        }
                        self.update(Message::SelectCarrier(carrier))
                    }
                    (DragItem::Line(id), DragItem::Line(onto)) => {
                        if id != onto {
                            self.shared.move_account(&id, &onto);
                        }
                        self.update(Message::SelectAccount(id))
                    }
                    _ => Task::none(),
                }
            }
            Message::SetTheme(theme) => {
                self.shared.update(|d| d.settings.theme = theme);
                Task::none()
            }
            Message::SetLang(lang) => {
                self.shared.update(|d| d.settings.lang = lang);
                Task::none()
            }
            Message::Refresh(_) | Message::RefreshAll if self.disclaimer_due() => Task::none(),
            Message::Refresh(id) => {
                let shared = self.shared.clone();
                Task::perform(async move { shared.refresh(&id, true).await }, |_| Message::Refreshed)
            }
            Message::RefreshAll => {
                let shared = self.shared.clone();
                Task::perform(async move { shared.refresh_all().await }, |_| Message::Refreshed)
            }
            Message::SetMain(id, key) => {
                self.shared.set_main_bundle(&id, key);
                Task::none()
            }
            Message::HeroStep(id, forward) => {
                let keys: Vec<String> = self
                    .snap
                    .data
                    .account(&id)
                    .and_then(|a| Some(a.usage.as_ref()?.ordered(a.main_bundle.as_deref()).iter().map(|b| b.key.clone()).collect()))
                    .unwrap_or_default();
                if !keys.is_empty() {
                    let at = self.hero.get(&id).and_then(|k| keys.iter().position(|x| x == k)).unwrap_or(0);
                    let next = if forward { (at + 1) % keys.len() } else { (at + keys.len() - 1) % keys.len() };
                    self.hero.insert(id, keys[next].clone());
                }
                Task::none()
            }
            Message::UsageHours(hours) => {
                self.usage_hours = hours;
                Task::none()
            }
            Message::OpenModal(kind) => {
                let first = match kind {
                    ModalKind::Rename(_) => Some(ui::modals::RENAME_INPUT),
                    _ => None,
                };
                self.modal = Modal::open(kind, &self.snap);
                first.map(operation::focus).unwrap_or_else(Task::none)
            }
            Message::CloseModal => {
                self.modal = Modal::None;
                Task::none()
            }
            Message::Modal(msg) => ui::modals::update(self, msg),
            Message::FocusNext => operation::focus_next(),
            Message::FocusPrevious => operation::focus_previous(),
            Message::Escape(_) if self.disclaimer_due() => Task::none(),
            Message::Escape(_) if self.drag.is_some() => {
                self.drag = None;
                Task::none()
            }
            Message::Escape(window) => {
                if Some(window) == self.popup {
                    window::close(window)
                } else if self.captcha.is_some() {
                    self.update(Message::CaptchaCancel)
                } else {
                    self.modal = Modal::None;
                    Task::none()
                }
            }
            Message::OpenConsole => {
                if let Some(id) = self.console {
                    return window::gain_focus(id);
                }
                let (id, open) = window::open(console_settings());
                self.console = Some(id);
                self.log.revision = u64::MAX;
                Task::batch([open.map(|_| Message::Done), self.update(Message::ConsoleTick)])
            }
            Message::ConsoleTick => {
                let revision = crate::console::revision();
                if revision == self.log.revision {
                    return Task::none();
                }
                self.log.revision = revision;
                let (level, search) = (self.log.level, self.log.search.to_lowercase());
                self.log.lines = crate::console::lines(CONSOLE_LINES, |l| {
                    l.level <= level && (search.is_empty() || l.text.to_lowercase().contains(&search) || l.target.to_lowercase().contains(&search))
                });
                if self.log.follow {
                    operation::snap_to_end(CONSOLE_SCROLL)
                } else {
                    Task::none()
                }
            }
            Message::ConsoleLevel(level) => {
                self.log.level = level;
                self.log.revision = u64::MAX;
                self.update(Message::ConsoleTick)
            }
            Message::ConsoleSearch(text) => {
                self.log.search = text;
                self.log.revision = u64::MAX;
                self.update(Message::ConsoleTick)
            }
            Message::ConsoleFollow => {
                self.log.follow = !self.log.follow;
                if self.log.follow { operation::snap_to_end(CONSOLE_SCROLL) } else { Task::none() }
            }
            Message::ConsoleCopy => {
                let text = self.log.lines.iter().map(crate::console::Line::plain).collect::<Vec<_>>().join("\n");
                iced::clipboard::write(text)
            }
            Message::ConsoleClear => {
                crate::console::clear();
                self.update(Message::ConsoleTick)
            }
            Message::AcceptDisclaimer => {
                self.shared.update(|d| d.settings.accepted_disclaimer = Some(about::VERSION.to_string()));
                self.snap = self.shared.snapshot();
                let shared = self.shared.clone();
                Task::perform(async move { shared.refresh_due().await }, |_| Message::Refreshed)
            }
            Message::OpenUrl(url) => {
                about::open_url(url);
                Task::none()
            }
            Message::OpenUrlOwned(url) => {
                about::open_url_owned(&url);
                Task::none()
            }
            Message::CheckUpdates { manual } => self.check_updates(manual),
            Message::UpdateChecked { manual, result } => self.update_checked(manual, result),
            Message::ShowUpdate => {
                if let UpdateState::Available { shown, .. } = &mut self.update {
                    *shown = true;
                }
                self.show_main(None)
            }
            Message::UpdateLater => {
                self.update = match std::mem::replace(&mut self.update, UpdateState::Idle) {
                    UpdateState::Available { release, .. } | UpdateState::Failed { release, .. } => UpdateState::Available { release, shown: false },
                    other @ (UpdateState::Downloading { .. } | UpdateState::Installing { .. }) => other,
                    _ => UpdateState::Idle,
                };
                Task::none()
            }
            Message::UpdateSkip => {
                if let Some(release) = self.update.release() {
                    let version = release.version.to_string();
                    self.shared.update(|d| d.settings.skipped_update = Some(version));
                }
                self.update = UpdateState::Idle;
                Task::none()
            }
            Message::UpdateInstall => self.install_update(),
            Message::UpdateProgress(got, total) => {
                if let UpdateState::Downloading { got: g, total: t, .. } = &mut self.update {
                    (*g, *t) = (got, total);
                }
                Task::none()
            }
            Message::UpdateDownloaded(result) => {
                let Some(release) = self.update.release().cloned() else { return Task::none() };
                match result {
                    Ok(file) => {
                        self.update = UpdateState::Installing { release: Box::new(release) };
                        Task::perform(async move { tokio::task::spawn_blocking(move || updater::install(&file)).await.map_err(|e| e.to_string())? }, Message::UpdateInstalled)
                    }
                    Err(message) => {
                        self.update = UpdateState::Failed { release: Box::new(release), message };
                        Task::none()
                    }
                }
            }
            Message::UpdateInstalled(result) => {
                let Some(release) = self.update.release().cloned() else { return Task::none() };
                match result {
                    // The new version waits for this one to exit.
                    Ok(updater::Outcome::Restart) => self.update(Message::Quit),
                    Ok(updater::Outcome::Opened) => {
                        self.update = UpdateState::Opened { release: Box::new(release) };
                        Task::none()
                    }
                    Err(message) => {
                        tracing::warn!("update failed: {message}");
                        self.update = UpdateState::Failed { release: Box::new(release), message };
                        Task::none()
                    }
                }
            }
            Message::Done => Task::none(),
        }
    }

    /// Sends the alerts due now, with one sound.
    fn send_due_alerts(&mut self) {
        if self.disclaimer_due() {
            return;
        }
        let alerts = self.shared.take_due_alerts(Utc::now());
        if !alerts.is_empty() {
            tracing::info!("{} alert(s) due", alerts.len());
            notify::send(&alerts, self.lang());
        }
    }

    fn check_updates(&mut self, manual: bool) -> Task<Message> {
        if matches!(self.update, UpdateState::Checking | UpdateState::Downloading { .. } | UpdateState::Installing { .. }) {
            return Task::none();
        }
        self.update = UpdateState::Checking;
        if self.shared.demo {
            let result = Ok(manual.then(|| Box::new(crate::demo::update())));
            return Task::perform(tokio::time::sleep(Duration::from_millis(900)), move |_| Message::UpdateChecked { manual, result: result.clone() });
        }
        let current = Version::parse(about::VERSION).unwrap_or(Version { major: 0, minor: 0, patch: 0, pre: None });
        Task::perform(
            async move { klue_core::update::newer_than(&current).await.map(|r| r.map(Box::new)).map_err(|e| e.to_string()) },
            move |result| Message::UpdateChecked { manual, result },
        )
    }

    fn update_checked(&mut self, manual: bool, result: Result<Option<Box<Release>>, String>) -> Task<Message> {
        self.update = match result {
            Ok(Some(release)) => {
                let skipped = self.snap.data.settings.skipped_update.as_deref() == Some(release.version.to_string().as_str());
                if skipped && !manual {
                    tracing::info!("update {} is skipped", release.version);
                    UpdateState::Idle
                } else {
                    tracing::info!("update available: {}", release.version);
                    // Without a window it waits; a notification says there's an update.
                    if self.main.is_none() && !manual {
                        notify::notice(i18n::update_available(&release.version.to_string(), self.lang()), i18n::strings(self.lang()).update_notify_body.into());
                    }
                    UpdateState::Available { release, shown: true }
                }
            }
            Ok(None) => UpdateState::UpToDate,
            Err(e) => {
                tracing::info!("update check failed: {e}");
                UpdateState::CheckFailed
            }
        };
        Task::none()
    }

    /// Downloads this platform's file, then installs it.
    fn install_update(&mut self) -> Task<Message> {
        let Some(release) = self.update.release().cloned() else { return Task::none() };
        let Some(asset) = release.asset(updater::suffixes()).cloned() else {
            // No file for this platform: open the release page.
            about::open_url_owned(&release.page);
            return Task::none();
        };
        let dir = updater::download_dir(&release.version.to_string());
        self.update = UpdateState::Downloading { release: Box::new(release), got: 0, total: (asset.size > 0).then_some(asset.size) };
        Task::run(
            iced::stream::channel(16, move |mut out: iced::futures::channel::mpsc::Sender<Message>| async move {
                let mut shown = 0u64;
                let result = klue_core::update::download(&asset, &dir, |got, total| {
                    // One message per percent (or per MB without a total).
                    let step = total.map_or(1 << 20, |t| (t / 100).max(1));
                    if got / step != shown / step {
                        shown = got;
                        let _ = out.try_send(Message::UpdateProgress(got, total));
                    }
                })
                .await
                .map_err(|e| e.to_string());
                let _ = out.send(Message::UpdateDownloaded(result)).await;
            }),
            |m| m,
        )
    }

    fn on_main(&self, f: impl FnOnce(window::Id) -> Task<Message>) -> Task<Message> {
        self.main.map(f).unwrap_or_else(Task::none)
    }

    /// Opens or raises the main window, optionally on one number.
    fn show_main(&mut self, account: Option<String>) -> Task<Message> {
        if let Some(id) = account {
            let _ = self.update(Message::SelectAccount(id));
        }
        if let Some(id) = self.main {
            return Task::batch([window::minimize(id, false), window::gain_focus(id)]);
        }
        let (id, open) = window::open(main_settings());
        self.main = Some(id);
        open.map(|_| Message::Done)
    }

    /// Close button: back to the tray, or quit.
    fn close_main(&mut self) -> Task<Message> {
        let keep = self.snap.data.settings.keep_in_tray && self.tray.is_some();
        if !keep {
            return self.update(Message::Quit);
        }
        if let Some(c) = self.captcha.take() {
            c.answer(None);
        }
        self.modal = Modal::None;
        self.on_main(window::close)
    }

    fn window_event(&mut self, id: window::Id, event: window::Event) -> Task<Message> {
        if !matches!(event, window::Event::RedrawRequested(_) | window::Event::Moved(_)) {
            let which = if Some(id) == self.popup {
                "popup"
            } else if Some(id) == self.main {
                "main"
            } else if Some(id) == self.console {
                "console"
            } else {
                "other"
            };
            tracing::debug!("{which} window: {event:?}");
        }
        if Some(id) == self.main
            && let window::Event::Opened { size, .. } | window::Event::Resized(size) = event
        {
            self.main_size = size;
        }
        match event {
            window::Event::CloseRequested if Some(id) == self.main => self.close_main(),
            window::Event::CloseRequested if Some(id) == self.popup => window::close(id),
            window::Event::CloseRequested if Some(id) == self.console => window::close(id),
            window::Event::Closed => {
                if Some(id) == self.main {
                    self.main = None;
                    if self.tray.is_none() {
                        return iced::exit();
                    }
                }
                if Some(id) == self.popup {
                    self.popup = None;
                    self.popup_closed_at = Some(Instant::now());
                }
                if Some(id) == self.console {
                    self.console = None;
                }
                Task::none()
            }
            // The popup closes when focus moves elsewhere. Some platforms report a focus loss before
            // it ever had focus; ignore that.
            window::Event::Focused if Some(id) == self.popup => {
                self.popup_focused = true;
                Task::none()
            }
            window::Event::Unfocused if Some(id) == self.popup && self.popup_focused => window::close(id),
            // Opening the main window also hides the popup.
            window::Event::Focused if Some(id) == self.main => self.popup.map(window::close).unwrap_or_else(Task::none),
            window::Event::Opened { .. } if Some(id) == self.popup => {
                window::scale_factor(id).then(move |scale| window::monitor_size(id).map(move |monitor| Message::PopupPlaced(id, scale, monitor)))
            }
            _ => Task::none(),
        }
    }

    fn tray_event(&mut self, event: systray::Event) -> Task<Message> {
        match event {
            systray::Event::Click(anchor) => self.toggle_popup(anchor),
            systray::Event::DoubleClick => self.update(Message::ShowMain(None)),
            systray::Event::Open => self.update(Message::ShowMain(None)),
            systray::Event::QuickView => self.toggle_popup(None),
            systray::Event::RefreshAll => self.update(Message::RefreshAll),
            systray::Event::Quit => self.update(Message::Quit),
        }
    }

    fn toggle_popup(&mut self, anchor: Option<Anchor>) -> Task<Message> {
        if let Some(id) = self.popup {
            return window::close(id);
        }
        if self.disclaimer_due() {
            return self.show_main(None);
        }
        // Clicking the tray icon steals focus from the popup and closes it; don't reopen it.
        if self.popup_closed_at.is_some_and(|t| t.elapsed() < Duration::from_millis(400)) {
            return Task::none();
        }
        self.popup_anchor = anchor;
        self.popup_focused = false;
        let (id, open) = window::open(popup_settings());
        self.popup = Some(id);
        open.map(|_| Message::Done)
    }

    /// Places the popup by the tray icon (or the usual tray corner), then shows it.
    fn place_popup(&mut self, id: window::Id, scale: f32, monitor: Option<Size>) -> Task<Message> {
        let monitor = monitor.unwrap_or(Size::new(1920.0, 1080.0));
        let (w, h) = (POPUP_SIZE.width, POPUP_SIZE.height);
        let scale = if scale > 0.0 { scale } else { 1.0 };
        let point = match self.popup_anchor {
            Some(a) => {
                let (x, y, aw, ah) = (a.x as f32 / scale, a.y as f32 / scale, a.width as f32 / scale, a.height as f32 / scale);
                let x = (x + aw / 2.0 - w / 2.0).clamp(8.0, (monitor.width - w - 8.0).max(8.0));
                let y = if y > monitor.height / 2.0 { y - h - 8.0 } else { y + ah + 8.0 };
                Point::new(x, y.max(8.0))
            }
            None if cfg!(target_os = "macos") => Point::new(monitor.width - w - 16.0, 36.0),
            None => Point::new(monitor.width - w - 16.0, monitor.height - h - 64.0),
        };
        // Show after moving, then focus (focusing a hidden window does nothing).
        window::move_to(id, point).chain(window::set_mode(id, window::Mode::Windowed)).chain(window::gain_focus(id))
    }
}

fn window_icon() -> Option<window::Icon> {
    let image = image::load_from_memory(assets::APP_ICON_PNG).ok()?.into_rgba8();
    let (w, h) = image.dimensions();
    window::icon::from_rgba(image.into_raw(), w, h).ok()
}

fn main_settings() -> window::Settings {
    window::Settings {
        size: MAIN_SIZE,
        min_size: Some(MAIN_MIN),
        position: window::Position::Centered,
        // Custom title bar. On macOS the window keeps its frame and traffic lights.
        decorations: cfg!(target_os = "macos"),
        icon: window_icon(),
        exit_on_close_request: false,
        platform_specific: platform(false),
        ..Default::default()
    }
}

/// The debug console uses the normal system frame.
fn console_settings() -> window::Settings {
    window::Settings {
        size: Size::new(980.0, 620.0),
        min_size: Some(Size::new(600.0, 360.0)),
        position: window::Position::Centered,
        icon: window_icon(),
        exit_on_close_request: false,
        ..Default::default()
    }
}

fn popup_settings() -> window::Settings {
    window::Settings {
        size: POPUP_SIZE,
        position: window::Position::Centered,
        visible: false,
        resizable: false,
        decorations: false,
        level: window::Level::AlwaysOnTop,
        icon: window_icon(),
        exit_on_close_request: false,
        platform_specific: platform(true),
        ..Default::default()
    }
}

#[allow(unused_variables)]
fn platform(popup: bool) -> window::settings::PlatformSpecific {
    #[cfg(target_os = "windows")]
    {
        window::settings::PlatformSpecific {
            drag_and_drop: false,
            skip_taskbar: popup,
            undecorated_shadow: true,
            corner_preference: window::settings::platform::CornerPreference::Round,
        }
    }
    #[cfg(target_os = "macos")]
    {
        window::settings::PlatformSpecific { title_hidden: true, titlebar_transparent: true, fullsize_content_view: true }
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        window::settings::PlatformSpecific { application_id: "klue".into(), ..Default::default() }
    }
}

/// Escape counts even when a text field captured it. A left button release anywhere ends a
/// drag, wherever the pointer went.
fn keys(event: iced::Event, _status: iced::event::Status, window: window::Id) -> Option<Message> {
    use iced::keyboard::{Event, Key, key::Named};
    if let iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)) = event {
        return Some(Message::PointerUp);
    }
    let iced::Event::Keyboard(Event::KeyPressed { key: Key::Named(named), modifiers, .. }) = event else { return None };
    match named {
        Named::Tab if modifiers.shift() => Some(Message::FocusPrevious),
        Named::Tab => Some(Message::FocusNext),
        Named::Escape => Some(Message::Escape(window)),
        _ => None,
    }
}

/// Revision bumps from the shared state.
fn revisions() -> impl Stream<Item = Message> {
    iced::stream::channel(8, async |mut output| {
        let Some(shared) = SHARED.get() else { return };
        let mut rx = shared.subscribe();
        while rx.changed().await.is_ok() {
            if output.send(Message::Revision).await.is_err() {
                break;
            }
        }
    })
}

/// Captchas the user is asked to type.
fn captchas() -> impl Stream<Item = Message> {
    iced::stream::channel(4, async |mut output| {
        let Some(mut rx) = SHARED.get().and_then(|s| s.take_captcha_requests()) else { return };
        while let Some(request) = rx.recv().await {
            let ask = CaptchaAsk {
                image: iced::widget::image::Handle::from_bytes(request.image),
                input: String::new(),
                reply: Arc::new(Mutex::new(Some(request.reply))),
            };
            if output.send(Message::Captcha(ask)).await.is_err() {
                break;
            }
        }
    })
}

/// Settings toggles that touch the OS.
pub fn set_launch_at_startup(shared: &Shared, on: bool) {
    shared.update(|d| d.settings.launch_at_startup = on);
    if !shared.demo {
        autostart::apply(on);
    }
}
