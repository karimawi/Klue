use dioxus::prelude::*;
use klue_core::{Carrier, store::Lang};

use super::{Icon, Modal, strings, use_snap, use_ui};
use crate::{assets::icons, i18n};

/// No numbers for this carrier yet (mockup frame 1r).
#[component]
pub fn Empty(carrier: Carrier) -> Element {
    let snap = use_snap();
    let ui = use_ui();
    let lang = snap.read().data.settings.lang;
    let tx = strings(lang);
    let name = i18n::carrier_name(carrier, lang);
    let (title, cta) = match lang {
        Lang::En => (format!("No {name} numbers yet"), format!("Add a {name} number")),
        Lang::Ar => (format!("لا توجد أرقام {name} بعد"), format!("إضافة رقم {name}")),
    };
    let mut modal = ui.modal;
    rsx! {
        div { class: "empty",
            div { class: "inner",
                div { class: "kicker", "{name} · {tx.empty_kicker}" }
                h1 { "{title}" }
                p { "{tx.empty_body}" }
                div { class: "steps",
                    for (i, step) in tx.steps.iter().enumerate() {
                        div {
                            div { class: "n", "0{i + 1}" }
                            div { class: "t", "{step}" }
                        }
                    }
                }
                div { style: "display:flex;gap:8px",
                    button { class: "cta", onclick: move |_| modal.set(Modal::Add),
                        Icon { svg: icons::PLUS_16_BOLD }
                        "{cta}"
                    }
                }
            }
        }
    }
}

/// Orange and e&: not supported yet.
#[component]
pub fn ComingSoon(carrier: Carrier) -> Element {
    let snap = use_snap();
    let lang = snap.read().data.settings.lang;
    let tx = strings(lang);
    let name = i18n::carrier_name(carrier, lang);
    let title = match lang {
        Lang::En => format!("{name} support is coming soon"),
        Lang::Ar => format!("دعم {name} قريباً"),
    };
    rsx! {
        div { class: "empty",
            div { class: "inner",
                div { class: "kicker", "{name} · {tx.coming_soon}" }
                h1 { "{title}" }
                p { "{tx.coming_soon_body}" }
            }
        }
    }
}
