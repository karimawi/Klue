//! Tray icon and menu. tray-icon's global handlers forward events to the UI over a
//! channel.

use std::sync::{Mutex, OnceLock};

use iced::futures::{SinkExt, Stream};
use klue_core::store::Lang;
use tokio::sync::mpsc;
use tray_icon::{
    Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};

use crate::{assets, i18n};

#[derive(Debug, Clone)]
pub enum Event {
    /// Left click, with the icon's position if the platform gives it.
    Click(Option<Anchor>),
    DoubleClick,
    Open,
    QuickView,
    RefreshAll,
    Quit,
}

/// Where the tray icon is, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Anchor {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Keeps the icon and menu items alive.
pub struct Tray {
    _icon: TrayIcon,
    items: [MenuItem; 4],
}

/// Sender for the tray handlers, and the receiver until the UI takes it.
type Channel = (mpsc::UnboundedSender<Event>, Mutex<Option<mpsc::UnboundedReceiver<Event>>>);

static CHANNEL: OnceLock<Channel> = OnceLock::new();

fn sender() -> mpsc::UnboundedSender<Event> {
    CHANNEL
        .get_or_init(|| {
            let (tx, rx) = mpsc::unbounded_channel();
            (tx, Mutex::new(Some(rx)))
        })
        .0
        .clone()
}

/// `None` if there's no tray (closing the window then quits).
pub fn create(lang: Lang) -> Option<Tray> {
    let tx = sender();
    TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
        let forwarded = match event {
            TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, rect, .. } => Some(Event::Click(anchor(rect))),
            TrayIconEvent::DoubleClick { button: MouseButton::Left, .. } => Some(Event::DoubleClick),
            _ => None,
        };
        if let Some(e) = forwarded {
            let _ = tx.send(e);
        }
    }));
    let tx = sender();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let forwarded = match event.id.0.as_str() {
            "open" => Some(Event::Open),
            "quick" => Some(Event::QuickView),
            "refresh" => Some(Event::RefreshAll),
            "quit" => Some(Event::Quit),
            _ => None,
        };
        if let Some(e) = forwarded {
            let _ = tx.send(e);
        }
    }));

    let s = i18n::strings(lang);
    let items = [
        MenuItem::with_id("open", s.open_app, true, None),
        MenuItem::with_id("quick", s.quick_view, true, None),
        MenuItem::with_id("refresh", s.refresh_all, true, None),
        MenuItem::with_id("quit", s.quit, true, None),
    ];
    let menu = Menu::new();
    let sep = PredefinedMenuItem::separator();
    let _ = menu.append_items(&[&items[0], &items[1], &sep, &items[2], &sep, &items[3]]);

    let mut builder = TrayIconBuilder::new().with_menu(Box::new(menu)).with_menu_on_left_click(false).with_tooltip("Klue");
    if let Some(icon) = icon() {
        builder = builder.with_icon(icon);
    }
    match builder.build() {
        Ok(icon) => Some(Tray { _icon: icon, items }),
        Err(e) => {
            tracing::warn!("no tray icon: {e}");
            None
        }
    }
}

impl Tray {
    pub fn set_lang(&self, lang: Lang) {
        let s = i18n::strings(lang);
        for (item, label) in self.items.iter().zip([s.open_app, s.quick_view, s.refresh_all, s.quit]) {
            item.set_text(label);
        }
    }
}

fn icon() -> Option<Icon> {
    let image = image::load_from_memory(assets::TRAY_ICON_PNG).ok()?.into_rgba8();
    let (w, h) = image.dimensions();
    Icon::from_rgba(image.into_raw(), w, h).ok()
}

fn anchor(rect: tray_icon::Rect) -> Option<Anchor> {
    (rect.size.width > 0 && rect.size.height > 0).then_some(Anchor {
        x: rect.position.x,
        y: rect.position.y,
        width: rect.size.width as f64,
        height: rect.size.height as f64,
    })
}

/// Tray events for the UI.
pub fn events() -> impl Stream<Item = Event> {
    iced::stream::channel(16, async |mut output| {
        let _ = sender();
        let Some(mut rx) = CHANNEL.get().and_then(|(_, rx)| rx.lock().ok()?.take()) else { return };
        while let Some(event) = rx.recv().await {
            if output.send(event).await.is_err() {
                break;
            }
        }
    })
}
