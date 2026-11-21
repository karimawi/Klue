use dioxus::{desktop::use_window, prelude::*};
use klue_core::store::{Lang, Theme};

use super::{Icon, strings, use_shared, use_snap, use_ui};
use crate::{assets::icons, i18n, systray};

#[component]
pub fn TitleBar() -> Element {
    let shared = use_shared();
    let snap = use_snap();
    let ui = use_ui();
    let desktop = use_window();

    let s = snap.read();
    let settings = &s.data.settings;
    let tx = strings(settings.lang);
    let synced = if s.refreshing.is_empty() { i18n::synced(s.data.last_sync(), *ui.now.read(), settings.lang) } else { tx.syncing.to_string() };
    let hide_on_close = settings.keep_in_tray && systray::TRAY_OK.load(std::sync::atomic::Ordering::Relaxed);

    let themes = [(Theme::Light, tx.light), (Theme::Dark, tx.dark), (Theme::Match, tx.match_)];
    let langs = [(Lang::En, "EN"), (Lang::Ar, "ع")];
    let (current_theme, current_lang) = (settings.theme, settings.lang);
    let (d_drag, d_max, d_min, d_max2, d_close) = (desktop.clone(), desktop.clone(), desktop.clone(), desktop.clone(), desktop.clone());

    rsx! {
        div { class: "titlebar",
            onmousedown: move |_| d_drag.drag(),
            ondoubleclick: move |_| d_max.toggle_maximized(),
            div { class: "brand-dot" }
            div { class: "app-name", "{tx.app}" }
            div { class: "synced", "{synced}" }
            div { class: "spacer" }
            div { class: "chseg", onmousedown: |e| e.stop_propagation(), ondoubleclick: |e| e.stop_propagation(),
                for (theme, label) in themes {
                    button {
                        class: if current_theme == theme { "on" } else { "" },
                        onclick: {
                            let shared = shared.clone();
                            move |_| shared.update(|d| d.settings.theme = theme)
                        },
                        "{label}"
                    }
                }
            }
            div { class: "chseg", onmousedown: |e| e.stop_propagation(), ondoubleclick: |e| e.stop_propagation(),
                for (lang, label) in langs {
                    button {
                        class: if current_lang == lang { "on" } else { "" },
                        onclick: {
                            let shared = shared.clone();
                            move |_| shared.update(|d| d.settings.lang = lang)
                        },
                        "{label}"
                    }
                }
            }
            div { class: "winbtns", onmousedown: |e| e.stop_propagation(), ondoubleclick: |e| e.stop_propagation(),
                button { title: "Minimize", onclick: move |_| d_min.set_minimized(true), Icon { svg: icons::WIN_MIN } }
                button { title: "Maximize", onclick: move |_| d_max2.toggle_maximized(), Icon { svg: icons::WIN_MAX } }
                button { class: "close", title: "Close",
                    onclick: move |_| if hide_on_close { d_close.set_visible(false) } else { d_close.close() },
                    Icon { svg: icons::WIN_CLOSE }
                }
            }
        }
    }
}
