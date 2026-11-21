//! The tray quick view (mockup frames 1s–1u): every number at a glance.

use std::sync::Arc;

use chrono::Utc;
use dioxus::{
    desktop::{
        tao::event::{Event, WindowEvent},
        use_window, use_wry_event_handler,
    },
    prelude::*,
};
use klue_core::{format, store::Lang};

use super::{Icon, Tile, dir, lang_tag, strings, use_snapshot_provider};
use crate::{
    assets::icons,
    i18n,
    state::{Shared, UiRequest},
    systray::{self, MainWindow},
    theme,
};

#[component]
pub fn TrayApp() -> Element {
    let shared = use_context::<Arc<Shared>>();
    let main = use_context::<MainWindow>();
    let snap = use_snapshot_provider();
    let me = use_window();
    let mut now = use_signal(Utc::now);

    // Close when it loses focus, like a menu.
    let me_blur = me.clone();
    use_wry_event_handler(move |event, _| {
        if let Event::WindowEvent { event: WindowEvent::Focused(false), window_id, .. } = event
            && *window_id == me_blur.id() {
                me_blur.close();
            }
    });
    use_drop(systray::popup_closed);
    use_future(move || async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            now.set(Utc::now());
        }
    });

    let s = snap.read();
    let lang = s.data.settings.lang;
    let tx = strings(lang);
    let style = theme::root_style(theme::tokens(s.data.settings.theme, klue_core::Carrier::We, true), lang);
    let synced = if s.refreshing.is_empty() { i18n::synced(s.data.last_sync(), now(), lang) } else { tx.syncing.to_string() };
    let busy = !s.refreshing.is_empty();
    let accounts = s.data.accounts.clone();

    let (sh_refresh, sh_open, sh_settings) = (shared.clone(), shared.clone(), shared.clone());
    let (main_open, main_settings) = (main.clone(), main.clone());
    let (me_open, me_settings) = (me.clone(), me.clone());

    rsx! {
        div { class: "rs", dir: dir(lang), lang: lang_tag(lang), style: "{style}",
            div { class: "tray-head", onmousedown: { let me = me.clone(); move |_| me.drag() },
                div { class: "dotmark" }
                div { class: "nm", "{tx.app}" }
                div { class: "synced", "{synced}" }
                button {
                    class: if busy { "icon-btn spin" } else { "icon-btn" },
                    title: tx.refresh_all,
                    onmousedown: |e| e.stop_propagation(),
                    onclick: move |_| {
                        let shared = sh_refresh.clone();
                        spawn(async move { shared.refresh_all().await });
                    },
                    Icon { svg: icons::REFRESH }
                }
            }
            div { class: "tray-list",
                if accounts.is_empty() {
                    div { class: "tray-empty", "{tx.tray_empty}" }
                }
                for account in accounts {
                    {
                        let main_bundle = account.usage.as_ref().and_then(|u| u.main()).cloned();
                        let renews = account.usage.as_ref().and_then(|u| u.renews_at);
                        let days = renews.map(|r| ((r - now()).num_seconds() as f64 / 86_400.0).ceil().max(0.0) as i64);
                        let number = account.pretty_number();
                        let sub = match (days, lang) {
                            (Some(d), Lang::En) => format!("Renews in {d}d · {number}"),
                            (Some(d), Lang::Ar) => format!("يتجدد بعد {d} يوم · {number}"),
                            (None, _) => number.clone(),
                        };
                        let (rem, width, low) = match &main_bundle {
                            Some(b) => (
                                format!("{} {}", format::amount(b.remaining, b.unit), i18n::unit_name(b.unit, lang)),
                                b.remaining_pct(),
                                account.alert_for(b, true).is_triggered(b),
                            ),
                            None => ("—".to_string(), 0.0, false),
                        };
                        let label = account.display_name();
                        let carrier = account.carrier;
                        let (shared, main, me, id) = (shared.clone(), main.clone(), me.clone(), account.id.clone());
                        rsx! {
                            div { key: "{account.id}", class: "tray-row", style: "cursor:pointer",
                                onclick: move |_| {
                                    systray::bring_main(&shared, &main, UiRequest::ShowAccount(id.clone()));
                                    me.close();
                                },
                                Tile { carrier, size: "34px", logo_size: crate::assets::logo(carrier).tray }
                                div { class: "text",
                                    div { class: "a", "{label}" }
                                    div { class: "b", "{sub}" }
                                }
                                div { class: "right",
                                    div { class: "rem", style: if low { "color:var(--acTx)" } else { "color:var(--tx)" }, "{rem}" }
                                    div { class: "mini", div { style: "width:{width}%" } }
                                }
                            }
                        }
                    }
                }
            }
            div { class: "tray-foot",
                button { class: "open",
                    onclick: move |_| {
                        systray::bring_main(&sh_open, &main_open, UiRequest::Show);
                        me_open.close();
                    },
                    "{tx.open_app}"
                }
                button { class: "icon-btn", title: tx.settings,
                    onclick: move |_| {
                        systray::bring_main(&sh_settings, &main_settings, UiRequest::OpenSettings);
                        me_settings.close();
                    },
                    Icon { svg: icons::SLIDERS_16 }
                }
            }
        }
    }
}
