use dioxus::prelude::*;
use klue_core::Carrier;

use super::{Icon, Modal, Tile, strings, use_snap, use_ui};
use crate::{assets::icons, i18n};

#[component]
pub fn Rail() -> Element {
    let snap = use_snap();
    let ui = use_ui();
    let tx = strings(snap.read().data.settings.lang);
    let mut modal = ui.modal;
    rsx! {
        div { class: "rail",
            for carrier in Carrier::ALL {
                RailItem { key: "{carrier.id()}", carrier }
            }
            div { class: "spacer" }
            button { class: "rail-tool", onclick: move |_| modal.set(Modal::Add),
                Icon { svg: icons::PLUS_18 }
                "{tx.add}"
            }
            button { class: "rail-tool", onclick: move |_| modal.set(Modal::Settings),
                Icon { svg: icons::SLIDERS_18 }
                "{tx.settings}"
            }
        }
    }
}

#[component]
fn RailItem(carrier: Carrier) -> Element {
    let snap = use_snap();
    let ui = use_ui();
    let s = snap.read();
    let lang = s.data.settings.lang;
    let on = *ui.carrier.read() == carrier;
    let soon = !carrier.is_supported();
    let count = s.data.accounts_for(carrier).count();
    let class = match (on, soon) {
        (true, true) => "rail-item on soon",
        (true, false) => "rail-item on",
        (false, true) => "rail-item soon",
        (false, false) => "rail-item",
    };
    let name = i18n::carrier_name(carrier, lang);
    let soon_label = strings(lang).soon;
    let mut current = ui.carrier;
    rsx! {
        button { class, title: if soon { strings(lang).coming_soon } else { "" }, onclick: move |_| current.set(carrier),
            div { class: "bar" }
            div { style: "position:relative",
                Tile { carrier, size: "46px", logo_size: crate::assets::logo(carrier).rail }
                if soon {
                    div { class: "badge soon", "{soon_label}" }
                } else {
                    div { class: "badge", "{count}" }
                }
            }
            div { class: "label", "{name}" }
        }
    }
}
