//! The main window: title bar, carrier rail, number tabs, dashboard and dialogs.

mod dashboard;
mod empty;
mod modals;
mod rail;
mod tabs;
mod titlebar;
pub mod tray;

use std::{collections::HashMap, sync::Arc};

use base64::Engine;
use chrono::{DateTime, Utc};
use dioxus::{desktop::use_window, prelude::*};
use klue_core::{
    Carrier,
    store::{Account, Lang},
};
use tokio::sync::oneshot;

use crate::{
    i18n,
    state::{Shared, Snapshot, TICK, UiRequest},
    systray, theme,
};

#[derive(Debug, Clone, PartialEq)]
pub enum Modal {
    None,
    Add,
    Rename(String),
    Notify(String),
    Settings,
}

/// A captcha waiting for the user to type it.
pub struct CaptchaAsk {
    pub uri: String,
    pub reply: Option<oneshot::Sender<Option<String>>>,
}

/// Per-window UI state.
#[derive(Clone, Copy)]
pub struct Ui {
    pub carrier: Signal<Carrier>,
    /// Selected number per carrier (account id).
    pub selected: Signal<HashMap<Carrier, String>>,
    pub modal: Signal<Modal>,
    pub captcha: Signal<Option<CaptchaAsk>>,
    /// Ticks every 30 s so "Synced 2 min ago" and countdowns stay current.
    pub now: Signal<DateTime<Utc>>,
}

impl Ui {
    /// The account shown for the selected carrier.
    pub fn current(&self, snap: &Snapshot) -> Option<Account> {
        let carrier = *self.carrier.read();
        let selected = self.selected.read().get(&carrier).cloned();
        let mut accounts = snap.data.accounts_for(carrier);
        match selected {
            Some(id) => snap.data.account(&id).filter(|a| a.carrier == carrier).cloned().or_else(|| accounts.next().cloned()),
            None => accounts.next().cloned(),
        }
    }

    pub fn select(mut self, account: &Account) {
        self.carrier.set(account.carrier);
        self.selected.write().insert(account.carrier, account.id.clone());
    }
}

pub fn use_shared() -> Arc<Shared> {
    use_context::<Arc<Shared>>()
}

pub fn use_snap() -> Signal<Snapshot> {
    use_context::<Signal<Snapshot>>()
}

pub fn use_ui() -> Ui {
    use_context::<Ui>()
}

/// Mirror the shared state into a signal for this window.
pub fn use_snapshot_provider() -> Signal<Snapshot> {
    let shared = use_shared();
    let mut snap = use_signal(|| shared.snapshot());
    use_future(move || {
        let shared = shared.clone();
        async move {
            let mut rx = shared.subscribe();
            while rx.changed().await.is_ok() {
                snap.set(shared.snapshot());
            }
        }
    });
    use_context_provider(|| snap)
}

pub fn dir(lang: Lang) -> &'static str {
    if lang == Lang::Ar { "rtl" } else { "ltr" }
}

pub fn lang_tag(lang: Lang) -> &'static str {
    if lang == Lang::Ar { "ar" } else { "en" }
}

fn image_uri(bytes: &[u8]) -> String {
    let mime = match bytes {
        [0xFF, 0xD8, ..] => "image/jpeg",
        [b'G', b'I', b'F', ..] => "image/gif",
        _ => "image/png",
    };
    format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))
}

#[component]
pub fn App() -> Element {
    let shared = use_shared();
    let snap = use_snapshot_provider();
    let desktop = use_window();

    let first_carrier = {
        let s = snap.peek();
        Carrier::ALL.into_iter().find(|c| s.data.accounts_for(*c).next().is_some()).unwrap_or(Carrier::We)
    };
    let ui = use_context_provider(|| Ui {
        carrier: Signal::new(first_carrier),
        selected: Signal::new(HashMap::new()),
        modal: Signal::new(Modal::None),
        captcha: Signal::new(None),
        now: Signal::new(Utc::now()),
    });

    systray::use_systray(shared.clone());
    use_hook({
        let desktop = desktop.clone();
        move || {
            // Without a tray icon a hidden window could never come back.
            if !systray::TRAY_OK.load(std::sync::atomic::Ordering::Relaxed) {
                desktop.set_close_behavior(dioxus::desktop::WindowCloseBehaviour::WindowCloses);
                desktop.set_visible(true);
            }
        }
    });

    // Demo only: open the tray quick view at start (the tray icon can't be clicked in CI).
    use_future({
        let (shared, desktop) = (shared.clone(), desktop.clone());
        move || {
            let (shared, desktop) = (shared.clone(), desktop.clone());
            async move {
                if shared.demo && std::env::var_os("KLUE_OPEN_QUICK_VIEW").is_some() {
                    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
                    systray::open_popup(shared, desktop, None);
                }
            }
        }
    });

    // Clock.
    use_future(move || async move {
        let mut now = ui.now;
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            now.set(Utc::now());
        }
    });

    // Background refreshes and alerts.
    use_future({
        let shared = shared.clone();
        move || {
            let shared = shared.clone();
            async move {
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                loop {
                    shared.refresh_due().await;
                    if !shared.demo {
                        let lang = shared.settings().lang;
                        for (account, event) in shared.take_due_alerts(Utc::now()) {
                            crate::notify::alert(&account, &event, lang);
                        }
                    }
                    tokio::time::sleep(TICK).await;
                }
            }
        }
    });

    // Captchas the user has to type.
    use_future({
        let (shared, desktop) = (shared.clone(), desktop.clone());
        move || {
            let (shared, desktop) = (shared.clone(), desktop.clone());
            async move {
                let Some(mut rx) = shared.take_captcha_requests() else { return };
                while let Some(req) = rx.recv().await {
                    let mut captcha = ui.captcha;
                    if let Some(old) = captcha.write().take().and_then(|c| c.reply) {
                        let _ = old.send(None);
                    }
                    captcha.set(Some(CaptchaAsk { uri: image_uri(&req.image), reply: Some(req.reply) }));
                    systray::show(&desktop);
                }
            }
        }
    });

    // Requests from the tray popup.
    use_future({
        let (shared, desktop) = (shared.clone(), desktop.clone());
        move || {
            let (shared, desktop) = (shared.clone(), desktop.clone());
            async move {
                let Some(mut rx) = shared.take_ui_requests() else { return };
                while let Some(req) = rx.recv().await {
                    systray::show(&desktop);
                    let mut modal = ui.modal;
                    match req {
                        UiRequest::Show => {}
                        UiRequest::OpenSettings => modal.set(Modal::Settings),
                        UiRequest::ShowAccount(id) => {
                            if let Some(a) = shared.snapshot().data.account(&id) {
                                ui.select(a);
                            }
                        }
                    }
                }
            }
        }
    });

    let s = snap.read();
    let lang = s.data.settings.lang;
    let style = theme::root_style(theme::tokens(s.data.settings.theme, *ui.carrier.read(), false), lang);

    rsx! {
        div { class: "rs", dir: dir(lang), lang: lang_tag(lang), style: "{style}",
            titlebar::TitleBar {}
            div { class: "body",
                rail::Rail {}
                div { class: "main",
                    tabs::Tabs {}
                    MainPane {}
                }
            }
            modals::Modals {}
            modals::CaptchaPrompt {}
        }
    }
}

#[component]
fn MainPane() -> Element {
    let snap = use_snap();
    let ui = use_ui();
    let s = snap.read();
    let carrier = *ui.carrier.read();
    if !carrier.is_supported() {
        return rsx! { empty::ComingSoon { carrier } };
    }
    match ui.current(&s) {
        Some(account) => rsx! { dashboard::Dashboard { account } },
        None => rsx! { empty::Empty { carrier } },
    }
}

/// Inline SVG icon.
#[component]
pub fn Icon(svg: &'static str, #[props(default)] spin: bool) -> Element {
    rsx! { span { class: if spin { "icon spin" } else { "icon" }, dangerous_inner_html: svg } }
}

#[component]
pub fn Toggle(on: bool, onclick: EventHandler<()>) -> Element {
    rsx! {
        button {
            class: if on { "toggle on" } else { "toggle" },
            role: "switch",
            "aria-checked": "{on}",
            onclick: move |_| onclick.call(()),
            span {}
        }
    }
}

/// A carrier's logo on its coloured tile.
#[component]
pub fn Tile(carrier: Carrier, size: &'static str, logo_size: &'static str) -> Element {
    let logo = crate::assets::logo(carrier);
    rsx! {
        div { class: "tile", style: "width:{size};height:{size};background:{logo.tile}",
            img { src: "{logo.uri}", alt: "", style: "width:{logo_size};height:{logo_size}" }
        }
    }
}

pub fn strings(lang: Lang) -> &'static i18n::Strings {
    i18n::strings(lang)
}
