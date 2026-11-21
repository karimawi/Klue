use dioxus::prelude::*;
use klue_core::{LineType, store::Account};

use super::{Icon, Modal, strings, use_snap, use_ui};
use crate::assets::icons;

/// One notebook-divider tab per number.
#[component]
pub fn Tabs() -> Element {
    let snap = use_snap();
    let ui = use_ui();
    let s = snap.read();
    let carrier = *ui.carrier.read();
    let tx = strings(s.data.settings.lang);
    let current = ui.current(&s).map(|a| a.id);
    let accounts: Vec<Account> = s.data.accounts_for(carrier).cloned().collect();
    let mut modal = ui.modal;
    rsx! {
        div { class: "strip",
            for account in accounts {
                Tab { key: "{account.id}", active: current.as_deref() == Some(account.id.as_str()), account }
            }
            if carrier.is_supported() {
                button { class: "ghost", onclick: move |_| modal.set(Modal::Add),
                    Icon { svg: icons::PLUS_16 }
                    "{tx.add_number}"
                }
            }
        }
    }
}

#[component]
fn Tab(account: Account, active: bool) -> Element {
    let ui = use_ui();
    let label = account.display_name();
    let number = account.pretty_number();
    let low = account.main_alert_triggered();
    let icon = if account.line == LineType::Mobile { icons::MOBILE } else { icons::LANDLINE };
    let select = account.clone();
    rsx! {
        button { class: if active { "tab on" } else { "tab" }, onclick: move |_| ui.select(&select),
            if active {
                span { class: "curve-l" }
                span { class: "curve-r" }
                span { class: "accent" }
            }
            Icon { svg: icon }
            div { class: "text",
                span { class: "label", "{label}" }
                span { class: "number", dir: "ltr", "{number}" }
            }
            if low {
                div { class: "dot" }
            }
        }
    }
}
