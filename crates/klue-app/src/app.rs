//! The iced program. Klue runs as a daemon: the tray icon and background refreshes live
//! for the whole session, while the main window and the tray popup are opened when needed
//! and destroyed when hidden, so an idle Klue keeps no window (or its memory) around.

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

use crate::{
    assets, autostart, notify,
    shared::{Shared, Snapshot, TICK},
    systray::{self, Anchor, Tray},
    ui::{self, modals::Modal},
};

/// The shared state, reachable from subscription streams (which can't capture).
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

#[derive(Debug, Clone)]
pub enum Message {
    Init,
    /// The shared data changed.
    Revision,
    Tick,
    Window(window::Id, window::Event),
    Tray(systray::Event),
    /// Another Klue was started: show this one (or its quick view).
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
    /// The popup's settings button: open the main window on Settings.
    PopupSettings,
    SelectCarrier(Carrier),
    SelectAccount(String),
    SetTheme(Theme),
    SetLang(Lang),
    Refresh(String),
    RefreshAll,
    SetMain(String, Option<String>),
    OpenModal(ModalKind),
    CloseModal,
    Modal(ui::modals::Msg),
    /// Tab and Shift+Tab move between fields.
    FocusNext,
    FocusPrevious,
    /// Escape closes the popup, the captcha prompt or the open dialog.
    Escape(window::Id),
    Done,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ModalKind {
    Add,
    Rename(String),
    Alerts(String),
    Settings,
}

pub struct App {
    pub shared: Arc<Shared>,
    pub snap: Snapshot,
    pub now: DateTime<Utc>,
    pub main: Option<window::Id>,
    pub popup: Option<window::Id>,
    popup_anchor: Option<Anchor>,
    /// Whether the popup has had the focus yet: it closes when it loses it.
    popup_focused: bool,
    popup_closed_at: Option<Instant>,
    tray: Option<Tray>,
    tray_lang: Lang,
    start_minimized: bool,
    start_quick_view: bool,
    /// The carrier shown in the main window, and the number picked for each carrier.
    pub carrier: Carrier,
    pub selected: HashMap<Carrier, String>,
    pub modal: Modal,
    pub captcha: Option<CaptchaAsk>,
}

pub fn run(shared: Arc<Shared>, minimized: bool, quick_view: bool) -> iced::Result {
    let _ = SHARED.set(shared.clone());
    let mut daemon = iced::daemon(move || App::new(shared.clone(), minimized, quick_view), App::update, App::view)
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
    fn new(shared: Arc<Shared>, minimized: bool, quick_view: bool) -> (Self, Task<Message>) {
        let snap = shared.snapshot();
        let carrier = Carrier::ALL.into_iter().find(|c| snap.data.accounts_for(*c).next().is_some()).unwrap_or(Carrier::We);
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
            modal: Modal::None,
            captcha: None,
        };
        // The tray icon is created on the first update: on macOS the event loop must be
        // running by then.
        (app, Task::done(Message::Init))
    }

    pub fn lang(&self) -> Lang {
        self.snap.data.settings.lang
    }

    /// The account shown for the selected carrier.
    pub fn current(&self) -> Option<&Account> {
        let data = &self.snap.data;
        let picked = self.selected.get(&self.carrier).and_then(|id| data.account(id)).filter(|a| a.carrier == self.carrier);
        picked.or_else(|| data.accounts_for(self.carrier).next())
    }

    fn title(&self, _window: window::Id) -> String {
        "Klue".into()
    }

    fn theme(&self, _window: window::Id) -> iced::Theme {
        match self.snap.data.settings.theme {
            Theme::Dark => iced::Theme::Dark,
            _ => iced::Theme::Light,
        }
    }

    fn view(&self, window: window::Id) -> ui::El<'_> {
        if Some(window) == self.popup {
            ui::popup::view(self)
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
            // Countdowns and "synced 2 min ago" stay current while a window is open.
            if self.main.is_some() || self.popup.is_some() {
                iced::time::every(Duration::from_secs(30)).map(|_| Message::Tick)
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
                let refresh = Task::perform(async move { shared.refresh_due().await }, |_| Message::Done);
                let window = if self.start_quick_view {
                    self.toggle_popup(None)
                } else if self.start_minimized && self.tray.is_some() {
                    Task::none()
                } else {
                    self.show_main(None)
                };
                Task::batch([window, refresh])
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
                let alerts = self.shared.take_due_alerts(self.now);
                for (account, event) in &alerts {
                    notify::alert(account, event, self.lang());
                }
                let shared = self.shared.clone();
                Task::perform(async move { shared.refresh_due().await }, |_| Message::Done)
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
                iced::exit()
            }

            Message::ShowMain(account) => {
                let close = self.popup.map(window::close).unwrap_or_else(Task::none);
                Task::batch([close, self.show_main(account)])
            }
            Message::PopupSettings => {
                let show = self.update(Message::ShowMain(None));
                self.modal = Modal::open(ModalKind::Settings, &self.snap, self.carrier);
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
            Message::SetTheme(theme) => {
                self.shared.update(|d| d.settings.theme = theme);
                Task::none()
            }
            Message::SetLang(lang) => {
                self.shared.update(|d| d.settings.lang = lang);
                Task::none()
            }
            Message::Refresh(id) => {
                let shared = self.shared.clone();
                Task::perform(async move { shared.refresh(&id, true).await }, |_| Message::Done)
            }
            Message::RefreshAll => {
                let shared = self.shared.clone();
                Task::perform(async move { shared.refresh_all().await }, |_| Message::Done)
            }
            Message::SetMain(id, key) => {
                self.shared.set_main_bundle(&id, key);
                Task::none()
            }
            Message::OpenModal(kind) => {
                let first = match kind {
                    ModalKind::Add => Some(ui::modals::ADD_NUMBER),
                    ModalKind::Rename(_) => Some(ui::modals::RENAME_INPUT),
                    _ => None,
                };
                self.modal = Modal::open(kind, &self.snap, self.carrier);
                first.map(operation::focus).unwrap_or_else(Task::none)
            }
            Message::CloseModal => {
                self.modal = Modal::None;
                Task::none()
            }
            Message::Modal(msg) => ui::modals::update(self, msg),
            Message::FocusNext => operation::focus_next(),
            Message::FocusPrevious => operation::focus_previous(),
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
            Message::Done => Task::none(),
        }
    }

    fn on_main(&self, f: impl FnOnce(window::Id) -> Task<Message>) -> Task<Message> {
        self.main.map(f).unwrap_or_else(Task::none)
    }

    /// Open the main window, or bring it forward; optionally on one number.
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

    /// The close button: back to the tray (dropping the window) or quit.
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
            let which = if Some(id) == self.popup { "popup" } else if Some(id) == self.main { "main" } else { "other" };
            tracing::debug!("{which} window: {event:?}");
        }
        match event {
            window::Event::CloseRequested if Some(id) == self.main => self.close_main(),
            window::Event::CloseRequested if Some(id) == self.popup => window::close(id),
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
                Task::none()
            }
            // The popup is a transient panel: clicking anywhere else closes it. (Some
            // platforms report a loss of focus before it ever had it; that one doesn't count.)
            window::Event::Focused if Some(id) == self.popup => {
                self.popup_focused = true;
                Task::none()
            }
            window::Event::Unfocused if Some(id) == self.popup && self.popup_focused => window::close(id),
            // Going back to the main window also puts the popup away.
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
        // A click on the tray icon first takes focus from the popup, which closes it;
        // that click shouldn't reopen it.
        if self.popup_closed_at.is_some_and(|t| t.elapsed() < Duration::from_millis(400)) {
            return Task::none();
        }
        self.popup_anchor = anchor;
        self.popup_focused = false;
        let (id, open) = window::open(popup_settings());
        self.popup = Some(id);
        open.map(|_| Message::Done)
    }

    /// Put the popup next to the tray icon (or in the corner where trays live), then show it.
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
        // Shown only once it's in place, then focused (focusing a hidden window does nothing).
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
        // Klue draws its own title bar. macOS keeps a real titled window (rounded
        // corners, shadow, resizing) with its traffic lights over Klue's title bar.
        decorations: cfg!(target_os = "macos"),
        icon: window_icon(),
        exit_on_close_request: false,
        platform_specific: platform(false),
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

/// Keyboard shortcuts. Escape counts even when a text field took it (to unfocus itself).
fn keys(event: iced::Event, _status: iced::event::Status, window: window::Id) -> Option<Message> {
    use iced::keyboard::{Event, Key, key::Named};
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
