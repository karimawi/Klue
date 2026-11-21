//! The number dashboard (mockup frames 1a–1h): quota hero, renewal column, bundles table.

use dioxus::prelude::*;
use klue_core::{
    Bundle, Unit, Usage, format,
    store::{Account, BundleAlert, Lang, LeadUnit, ThresholdMode},
};

use super::{Icon, Modal, Toggle, strings, use_shared, use_snap, use_ui};
use crate::{assets::icons, i18n};

#[component]
pub fn Dashboard(account: Account) -> Element {
    let shared = use_shared();
    let snap = use_snap();
    let ui = use_ui();
    let s = snap.read();
    let lang = s.data.settings.lang;
    let tx = strings(lang);
    let refreshing = s.is_refreshing(&account.id);

    let kicker = format!("{} · {}", i18n::line_name(account.line, lang), i18n::carrier_name(account.carrier, lang));
    let display = account.display_name();
    let owner = account.owner_name.clone().unwrap_or_else(|| tx.owner_unknown.to_string());
    let number = account.pretty_number();
    let error = account.last_error.as_ref().map(|e| i18n::error_text(e, account.carrier, lang));

    let id = account.id.clone();
    let (id_rename, id_alerts) = (id.clone(), id.clone());
    let mut modal = ui.modal;
    let refresh = move |_| {
        let (shared, id) = (shared.clone(), id.clone());
        spawn(async move { shared.refresh(&id, true).await });
    };

    rsx! {
        div { class: "dash",
            div { class: "dash-head",
                div { class: "who",
                    div { class: "kicker", "{kicker}" }
                    h1 { "{display}" }
                    div { class: "reg",
                        span { "{tx.registered_to}" }
                        strong { "{owner}" }
                        span { "·" }
                        span { class: "num", dir: "ltr", "{number}" }
                    }
                }
                div { class: "tools",
                    button { class: "btn-tool", disabled: refreshing, onclick: refresh,
                        Icon { svg: icons::REFRESH, spin: refreshing }
                        "{tx.refresh}"
                    }
                    button { class: "btn-tool", onclick: move |_| modal.set(Modal::Rename(id_rename.clone())),
                        Icon { svg: icons::PENCIL }
                        "{tx.rename}"
                    }
                    button { class: "btn-tool", onclick: move |_| modal.set(Modal::Notify(id_alerts.clone())),
                        Icon { svg: icons::BELL_15 }
                        "{tx.alerts}"
                    }
                }
            }
            if let Some(error) = error {
                div { class: "notice",
                    Icon { svg: icons::WARNING }
                    div { class: "text",
                        div { class: "title", "{tx.refresh_failed}" }
                        div { class: "body", "{error}" }
                    }
                }
            }
            match account.usage.clone() {
                Some(usage) => rsx! { Overview { account: account.clone(), usage } },
                None => rsx! {
                    div { style: "padding:22px 0;max-width:360px",
                        div { class: "muted", "{tx.waiting_first_sync}" }
                        if refreshing {
                            div { class: "progress", div {} }
                        }
                    }
                },
            }
        }
    }
}

fn unit_label(unit: Unit, lang: Lang) -> &'static str {
    i18n::unit_name(unit, lang)
}

/// "Alert at 10 GB" / "Notify me when 10 GB remain" pieces for a bundle alert.
fn threshold_text(alert: &BundleAlert, unit: Unit, lang: Lang) -> String {
    match alert.mode {
        ThresholdMode::Percent => format!("{}%", format::amount_short(alert.value, Unit::Gb)),
        ThresholdMode::Amount => format!("{} {}", format::amount_short(alert.value, unit), unit_label(unit, lang)),
    }
}

#[component]
fn Overview(account: Account, usage: Usage) -> Element {
    let shared = use_shared();
    let snap = use_snap();
    let ui = use_ui();
    let lang = snap.read().data.settings.lang;
    let tx = strings(lang);
    let now = *ui.now.read();
    let ar = lang == Lang::Ar;
    let mut modal = ui.modal;

    // Quota hero.
    let main = usage.main().cloned();
    let package = usage.package_name.clone().or_else(|| main.as_ref().map(|m| m.name.clone())).unwrap_or_default();
    let hero = main.as_ref().map(|m| {
        let alert = account.alert_for(m, true);
        let unit = unit_label(m.unit, lang);
        let pct = (m.remaining_pct() * 10.0).round() / 10.0;
        let threshold = alert.threshold(m);
        let low = alert.is_triggered(m);
        let used = format::amount(m.used(), m.unit);
        let total = format::amount_short(m.total, m.unit);
        let mark = threshold_text(&alert, m.unit, lang);
        (
            format::amount(m.remaining, m.unit),
            i18n::left_label(m.unit, lang),
            if ar { format!("من {total} {unit}") } else { format!("of {total} {unit}") },
            format!("{pct:.1}"),
            low,
            alert.enabled,
            if m.total > 0.0 { (threshold / m.total * 100.0).clamp(0.0, 100.0) } else { 0.0 },
            if ar { format!("تم استخدام {used} {unit}") } else { format!("{used} {unit} used") },
            if !alert.enabled { String::new() } else if ar { format!("حد التنبيه {mark}") } else { format!("Alert at {mark}") },
            if !alert.enabled {
                (if ar { "لا يوجد تنبيه للرصيد" } else { "No quota alert set" }).to_string()
            } else if ar {
                format!("نبّهني عندما يتبقى {mark}")
            } else {
                format!("Notify me when {mark} remain")
            },
        )
    });

    // Renewal column.
    let start = usage.cycle_start();
    let renews = usage.renews_at;
    let renew_long = renews.map(|r| i18n::date_long(r, lang)).unwrap_or_else(|| "—".into());
    let countdown = renews.map(|r| i18n::countdown(r, now, lang)).unwrap_or_default();
    let (cycle_text, cycle_pct) = match (start, renews) {
        (Some(st), Some(r)) if r > st => {
            let day = ((now - st).num_seconds() as f64 / 86_400.0).round().max(0.0) as i64;
            let days = ((r - st).num_seconds() as f64 / 86_400.0).round() as i64;
            let pct = ((now - st).num_seconds() as f64 / (r - st).num_seconds() as f64 * 100.0).clamp(0.0, 100.0);
            (if ar { format!("اليوم {day} من {days}") } else { format!("Day {day} of {days}") }, pct)
        }
        _ => (String::new(), 0.0),
    };
    let sub_short = start.map(|d| i18n::date_short(d, lang)).unwrap_or_else(|| "—".into());
    let renew_short = renews.map(|d| i18n::date_short(d, lang)).unwrap_or_else(|| "—".into());
    let reminder = account.alerts.renewal;
    let lead = i18n::count(
        reminder.value as i64,
        if reminder.unit == LeadUnit::Days { i18n::Span::Day } else { i18n::Span::Hour },
        lang,
    );
    let remind_text = match (reminder.enabled, ar) {
        (true, false) => format!("Remind me {lead} before"),
        (true, true) => format!("ذكّرني قبل التجديد بـ {lead}"),
        (false, false) => "Renewal reminder off".into(),
        (false, true) => "تذكير التجديد غير مفعّل".into(),
    };

    let extras: Vec<Bundle> = usage.extras().to_vec();
    let id = account.id.clone();
    let (id_a, id_b, id_c) = (id.clone(), id.clone(), id.clone());
    let toggle_reminder = move |_| {
        let id = id_a.clone();
        shared.update(|d| {
            if let Some(a) = d.account_mut(&id) {
                a.alerts.renewal.enabled = !a.alerts.renewal.enabled;
            }
        });
    };

    rsx! {
        div { class: "grid",
            div { class: "quota",
                div { class: "row-between",
                    div { class: "caps", "{tx.package}" }
                    if hero.as_ref().is_some_and(|h| h.4) {
                        span { class: "pill", "{tx.below_alert}" }
                    }
                }
                div { class: "pkg", "{package}" }
                if let Some((rem, left, of, pct, low, has_alert, alert_pos, used, mark, alert_text)) = hero {
                    div { class: "figure",
                        div { class: "left",
                            div { class: "big", dir: "ltr", style: if low { "color:var(--acTx)" } else { "color:var(--tx)" }, "{rem}" }
                            div { class: "big-label",
                                div { class: "l1", "{left}" }
                                div { class: "l2", "{of}" }
                            }
                        }
                        div { class: "pct",
                            div { class: "v", dir: "ltr", "{pct}%" }
                            div { class: "l", "{tx.remaining}" }
                        }
                    }
                    div { class: "meter",
                        div { class: "fill", style: "width:{pct}%" }
                        if has_alert {
                            div { class: "mark", style: "inset-inline-start:{alert_pos}%" }
                        }
                    }
                    div { class: "legend",
                        span { "{used}" }
                        span { "{mark}" }
                    }
                    div { class: "foot-row",
                        Icon { svg: icons::BELL_16 }
                        span { class: "t", "{alert_text}" }
                        button { class: "link", onclick: move |_| modal.set(Modal::Notify(id_b.clone())), "{tx.change}" }
                    }
                }
            }
            div { class: "renew",
                div { class: "caps", "{tx.next_renewal}" }
                div { class: "date", "{renew_long}" }
                div { class: "countdown", "{countdown}" }
                div { class: "cycle",
                    span { "{tx.cycle}" }
                    span { "{cycle_text}" }
                }
                div { class: "thin", div { class: "fill", style: "width:{cycle_pct}%" } }
                div { class: "dates",
                    div {
                        div { class: "k", "{tx.subscribed}" }
                        div { class: "v", "{sub_short}" }
                    }
                    div { style: "text-align:end",
                        div { class: "k", "{tx.renews}" }
                        div { class: "v", "{renew_short}" }
                    }
                }
                div { class: "grow" }
                div { class: "foot-row",
                    Toggle { on: reminder.enabled, onclick: toggle_reminder }
                    span { class: "t", "{remind_text}" }
                    button { class: "link", onclick: move |_| modal.set(Modal::Notify(id_c.clone())), "{tx.change}" }
                }
            }
        }
        if !extras.is_empty() {
            div { class: "bundles",
                h2 { "{tx.bundles}" }
                div { class: "brow head",
                    span { "{tx.col_bundle}" }
                    span { "{tx.col_rem}" }
                    span { "{tx.col_total}" }
                    span { "{tx.col_left}" }
                    span { "{tx.col_alert}" }
                }
                for bundle in extras {
                    BundleRow { key: "{bundle.key}", alert: account.alert_for(&bundle, false), bundle }
                }
            }
        }
    }
}

#[component]
fn BundleRow(bundle: Bundle, alert: BundleAlert) -> Element {
    let snap = use_snap();
    let lang = snap.read().data.settings.lang;
    let unit = unit_label(bundle.unit, lang);
    let pct = (bundle.remaining_pct() * 10.0).round() / 10.0;
    let low = alert.is_triggered(&bundle);
    let rem = format!("{} {unit}", format::amount(bundle.remaining, bundle.unit));
    let total = format!("{} {unit}", format::amount_short(bundle.total, bundle.unit));
    let alert_text = if alert.enabled { format!("≤ {}", threshold_text(&alert, bundle.unit, lang)) } else { strings(lang).off.to_string() };
    let kind = i18n::kind_name(bundle.kind, lang);
    rsx! {
        div { class: "brow",
            div { class: "name",
                div { class: "n", "{bundle.name}" }
                div { class: "k", "{kind}" }
            }
            div { class: "rem", style: if low { "color:var(--acTx)" } else { "color:var(--tx)" }, "{rem}" }
            div { class: "tot", "{total}" }
            div { class: "left",
                div { class: "thin", div { class: "fill", style: "width:{pct}%" } }
                span { class: "p", dir: "ltr", "{pct:.1}%" }
            }
            div { class: "al", style: if alert.enabled { "color:var(--tx)" } else { "color:var(--mu)" }, "{alert_text}" }
        }
    }
}
