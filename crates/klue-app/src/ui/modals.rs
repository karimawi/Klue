//! Dialogs: add number (1i, 1k–1n), rename (1o), alerts (1p), settings (1j, 1q), captcha.

use std::collections::BTreeMap;

use dioxus::prelude::*;
use klue_core::{
    Carrier, FetchError, LineType, Unit, Usage, format,
    store::{Account, AlertPrefs, BundleAlert, Lang, LeadUnit, RefreshEvery, Theme, ThresholdMode},
};

use super::{Icon, Modal, Tile, Toggle, strings, use_shared, use_snap, use_ui};
use crate::{assets::icons, autostart, i18n, theme};

#[component]
pub fn Modals() -> Element {
    let ui = use_ui();
    let snap = use_snap();
    let modal = ui.modal.read().clone();
    let account = |id: &str| snap.read().data.account(id).cloned();
    let content = match modal {
        Modal::None => return rsx! {},
        Modal::Add => rsx! { AddNumber {} },
        Modal::Settings => rsx! { SettingsDialog {} },
        Modal::Rename(id) => match account(&id) {
            Some(account) => rsx! { Rename { account } },
            None => return rsx! {},
        },
        Modal::Notify(id) => match account(&id) {
            Some(account) if account.usage.is_some() => rsx! { Alerts { account } },
            _ => return rsx! {},
        },
    };
    let mut m = ui.modal;
    rsx! {
        div { class: "overlay", onmousedown: move |_| m.set(Modal::None),
            div { class: "modal", onmousedown: |e| e.stop_propagation(), {content} }
        }
    }
}

#[component]
fn Head(kicker: String, title: String) -> Element {
    let ui = use_ui();
    let mut m = ui.modal;
    rsx! {
        div { class: "m-head",
            div {
                div { class: "kicker", "{kicker}" }
                div { class: "title", "{title}" }
            }
            button { class: "m-close", onclick: move |_| m.set(Modal::None), Icon { svg: icons::CLOSE_16 } }
        }
    }
}

// ---------------------------------------------------------------------------------------
// Add number

#[derive(Debug, Clone, PartialEq)]
enum Phase {
    Form,
    Verifying,
    Verified(Box<Usage>),
    Error(FetchError),
}

#[component]
fn AddNumber() -> Element {
    let shared = use_shared();
    let snap = use_snap();
    let ui = use_ui();
    let lang = snap.read().data.settings.lang;
    let tx = strings(lang);
    let ar = lang == Lang::Ar;

    let start = {
        let c = *ui.carrier.peek();
        if c.is_supported() { c } else { Carrier::We }
    };
    let mut carrier = use_signal(|| start);
    let mut line = use_signal(|| start.default_line());
    let mut number = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut show_password = use_signal(|| false);
    let mut nickname = use_signal(String::new);
    let mut phase = use_signal(|| Phase::Form);
    let mut manual_captcha = use_signal(|| false);

    let verify = use_callback({
        let shared = shared.clone();
        move |_: ()| {
            if matches!(*phase.peek(), Phase::Verifying) {
                return;
            }
            let (shared, c, l, n, p, manual) =
                (shared.clone(), *carrier.peek(), *line.peek(), number.peek().clone(), password.peek().clone(), *manual_captcha.peek());
            if n.trim().is_empty() || p.is_empty() {
                return;
            }
            phase.set(Phase::Verifying);
            spawn(async move {
                match shared.verify(c, l, &n, &p, manual).await {
                    Ok(usage) => phase.set(Phase::Verified(Box::new(usage))),
                    Err(e) => {
                        if e == FetchError::CaptchaUnsolved {
                            manual_captcha.set(true);
                        }
                        phase.set(Phase::Error(e));
                    }
                }
            });
        }
    });

    let add = use_callback({
        let shared = shared.clone();
        move |_: ()| {
            let Phase::Verified(usage) = phase.peek().clone() else { return };
            let nick = Some(nickname.peek().clone());
            let c = *carrier.peek();
            match shared.add_account(c, *line.peek(), &number.peek(), &password.peek(), nick, *usage) {
                Ok(id) => {
                    if let Some(account) = shared.snapshot().data.account(&id) {
                        ui.select(account);
                    }
                    let mut m = ui.modal;
                    m.set(Modal::None);
                }
                Err(e) => phase.set(Phase::Error(e)),
            }
        }
    });

    let c = carrier();
    let current_phase = phase();
    let verifying = matches!(current_phase, Phase::Verifying);
    let verified = matches!(current_phase, Phase::Verified(_));
    let error = match &current_phase {
        Phase::Error(e) => Some(e.clone()),
        _ => None,
    };
    let carrier_name = i18n::carrier_name(c, lang);
    let app = i18n::carrier_app(c, lang);
    let hint = if ar { format!("نفس كلمة المرور المستخدمة في {app}.") } else { format!("The same password you use in {app}.") };
    let owner = match &current_phase {
        Phase::Verified(u) => u.owner_name.clone().unwrap_or_else(|| tx.owner_unknown.into()),
        Phase::Verifying => tx.fetching.into(),
        _ => tx.auto_fill.into(),
    };
    let status = match &current_phase {
        Phase::Verified(_) => format!("✓ {}", tx.verified),
        Phase::Verifying => {
            if ar { format!("جارٍ تسجيل الدخول إلى {carrier_name}…") } else { format!("Signing in to {carrier_name}…") }
        }
        _ => String::new(),
    };
    let primary = match &current_phase {
        Phase::Verified(_) => tx.add_number,
        Phase::Verifying => tx.verifying,
        Phase::Error(_) => tx.try_again,
        Phase::Form => tx.verify,
    };
    let ready = !number().trim().is_empty() && !password().is_empty();
    let placeholder = if line() == LineType::Mobile { "010 1234 5678" } else { "02 2345 6789" };
    let number_align = if ar { "text-align:right" } else { "text-align:left" };
    let soon = tx.soon;
    let mut m = ui.modal;

    rsx! {
        Head { kicker: tx.new_number.to_string(), title: tx.add_title.to_string() }
        div { class: "m-body",
            div {
                div { class: "flabel", "{tx.carrier}" }
                div { class: "carriers",
                    for option in Carrier::ALL {
                        button {
                            class: if option == c { "carrier-opt on" } else if option.is_supported() { "carrier-opt" } else { "carrier-opt soon" },
                            disabled: !option.is_supported() || verifying,
                            title: if option.is_supported() { "" } else { tx.coming_soon },
                            onclick: move |_| {
                                carrier.set(option);
                                line.set(option.default_line());
                                phase.set(Phase::Form);
                                manual_captcha.set(false);
                            },
                            Tile { carrier: option, size: "24px", logo_size: crate::assets::logo(option).small }
                            "{i18n::carrier_name(option, lang)}"
                            if !option.is_supported() {
                                span { class: "soon", "{soon}" }
                            }
                        }
                    }
                }
            }
            div { class: "pair",
                div {
                    div { class: "flabel", "{tx.line_type}" }
                    div { class: "seg",
                        for (option, label) in [(LineType::Mobile, tx.mobile), (LineType::Landline, tx.landline)] {
                            button {
                                class: if line() == option { "on" } else { "" },
                                disabled: !c.supports_line(option) || verifying,
                                title: if c.supports_line(option) { "" } else { tx.coming_soon },
                                onclick: move |_| line.set(option),
                                "{label}"
                            }
                        }
                    }
                }
                div {
                    div { class: "flabel", "{tx.number}" }
                    input {
                        class: "field num",
                        dir: "ltr",
                        style: "{number_align}",
                        r#type: "tel",
                        placeholder,
                        initial_value: "{number.peek()}",
                        disabled: verifying || verified,
                        autofocus: true,
                        oninput: move |e| {
                            number.set(e.value());
                            if matches!(*phase.peek(), Phase::Error(_)) {
                                phase.set(Phase::Form);
                            }
                        },
                        onkeydown: move |e| if e.key() == Key::Enter { verify.call(()) },
                    }
                }
            }
            div {
                div { class: "flabel", "{tx.password}" }
                div { class: if error == Some(FetchError::BadCredentials) { "field err" } else { "field" },
                    input {
                        class: if show_password() { "" } else { "masked" },
                        r#type: if show_password() { "text" } else { "password" },
                        initial_value: "{password.peek()}",
                        disabled: verifying || verified,
                        oninput: move |e| {
                            password.set(e.value());
                            if matches!(*phase.peek(), Phase::Error(_)) {
                                phase.set(Phase::Form);
                            }
                        },
                        onkeydown: move |e| if e.key() == Key::Enter { verify.call(()) },
                    }
                    button { class: "eye", onclick: move |_| show_password.set(!show_password()),
                        Icon { svg: if show_password() { icons::EYE_OFF } else { icons::EYE } }
                    }
                }
                div { class: "hint", "{hint}" }
                if verifying {
                    div { class: "progress", div {} }
                }
            }
            if let Some(e) = error.clone() {
                div { class: "notice",
                    Icon { svg: icons::WARNING }
                    div { class: "text",
                        div { class: "title", "{i18n::error_title(&e, lang)}" }
                        div { class: "body", "{i18n::error_text(&e, c, lang)}" }
                    }
                }
            }
            div {
                div { class: "flabel-row",
                    span { class: "flabel", style: "margin:0", "{tx.owner_name}" }
                    if verified {
                        span { class: "from accent", "{tx.from_account}" }
                    }
                }
                div { class: if verified { "owner ok" } else { "owner" },
                    span { "{owner}" }
                    if verified {
                        span { class: "icon accent", style: "flex:none", dangerous_inner_html: icons::CHECK }
                    }
                }
            }
            if verified {
                div {
                    div { class: "flabel", "{tx.nick_optional}" }
                    input {
                        class: "field focus",
                        placeholder: tx.nick_ph,
                        autofocus: true,
                        oninput: move |e| nickname.set(e.value()),
                        onkeydown: move |e| if e.key() == Key::Enter { add.call(()) },
                    }
                }
            }
        }
        div { class: "m-foot",
            span { class: "status", "{status}" }
            button { class: "btn-outline", onclick: move |_| m.set(Modal::None), "{tx.cancel}" }
            button {
                class: "btn-primary wide",
                disabled: verifying || (!verified && !ready),
                onclick: move |_| if verified { add.call(()) } else { verify.call(()) },
                "{primary}"
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// Rename

#[component]
fn Rename(account: Account) -> Element {
    let shared = use_shared();
    let snap = use_snap();
    let ui = use_ui();
    let lang = snap.read().data.settings.lang;
    let tx = strings(lang);
    let mut draft = use_signal(|| account.nickname.clone().unwrap_or_default());
    // Bumped to re-mount the (uncontrolled) nickname field after "Use owner name".
    let mut field_key = use_signal(|| 0_u32);
    let mut confirm_remove = use_signal(|| false);
    let mut m = ui.modal;

    let owner = account.owner_name.clone().unwrap_or_else(|| tx.owner_unknown.into());
    let fallback = account.owner_name.clone().unwrap_or_else(|| account.pretty_number());
    let shown = if draft().trim().is_empty() { fallback } else { draft() };
    let number = account.pretty_number();
    let icon = if account.line == LineType::Mobile { icons::MOBILE } else { icons::LANDLINE };
    let kicker = format!("{} · {}", i18n::carrier_name(account.carrier, lang), number);
    let (id_save, id_remove) = (account.id.clone(), account.id.clone());
    let shared_remove = shared.clone();

    rsx! {
        Head { kicker, title: tx.rename_title.to_string() }
        div { class: "m-body",
            div {
                div { class: "flabel-row",
                    span { class: "flabel", style: "margin:0", "{tx.owner_name}" }
                    span { class: "from muted", "{tx.from_account}" }
                }
                div { class: "owner plain", span { "{owner}" } }
            }
            div {
                div { class: "flabel", "{tx.nickname}" }
                input {
                    key: "{field_key}",
                    class: "field focus",
                    initial_value: "{draft.peek()}",
                    placeholder: "{owner}",
                    autofocus: true,
                    oninput: move |e| draft.set(e.value()),
                }
                div { class: "hint", "{tx.nick_hint}" }
            }
            div {
                div { class: "flabel", "{tx.shown_as}" }
                div { class: "preview-tab",
                    Icon { svg: icon }
                    div { class: "text",
                        div { class: "a", "{shown}" }
                        div { class: "b num", dir: "ltr", "{number}" }
                    }
                }
            }
            div { class: "danger-zone",
                span { class: "t", "{tx.remove_hint}" }
                button {
                    class: "link",
                    onclick: move |_| {
                        if confirm_remove() {
                            shared_remove.remove_account(&id_remove);
                            m.set(Modal::None);
                        } else {
                            confirm_remove.set(true);
                        }
                    },
                    if confirm_remove() { "{tx.remove_confirm}" } else { "{tx.remove}" }
                }
            }
        }
        div { class: "m-foot",
            button { class: "link", style: "margin-inline-end:auto", onclick: move |_| {
                    draft.set(String::new());
                    field_key += 1;
                }, "{tx.use_owner}" }
            button { class: "btn-outline", onclick: move |_| m.set(Modal::None), "{tx.cancel}" }
            button {
                class: "btn-primary",
                onclick: move |_| {
                    let nick = draft.peek().trim().to_string();
                    let id = id_save.clone();
                    shared.update(|d| {
                        if let Some(a) = d.account_mut(&id) {
                            a.nickname = (!nick.is_empty()).then_some(nick);
                        }
                    });
                    m.set(Modal::None);
                },
                "{tx.save}"
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// Alerts

#[component]
fn Alerts(account: Account) -> Element {
    let shared = use_shared();
    let snap = use_snap();
    let ui = use_ui();
    let lang = snap.read().data.settings.lang;
    let tx = strings(lang);
    let ar = lang == Lang::Ar;
    let usage = account.usage.clone().unwrap_or_else(|| unreachable!("Alerts is only shown with usage"));

    let initial = {
        let mut prefs = account.alerts.clone();
        for (i, b) in usage.bundles.iter().enumerate() {
            prefs.bundles.entry(b.key.clone()).or_insert_with(|| BundleAlert::default_for(b, i == 0));
        }
        prefs
    };
    let texts_init: BTreeMap<String, String> =
        initial.bundles.iter().map(|(k, v)| (k.clone(), format::amount_short(v.value, Unit::Gb))).collect();
    let mut draft = use_signal(|| initial);
    let mut texts = use_signal(|| texts_init);
    let mut lead_text = use_signal(|| account.alerts.renewal.value.to_string());
    let mut m = ui.modal;

    let prefs: AlertPrefs = draft();
    let display = account.display_name();
    let carrier_name = i18n::carrier_name(account.carrier, lang);
    let main = usage.main().cloned();
    let package = usage.package_name.clone().unwrap_or_default();
    let preview_body = main
        .as_ref()
        .map(|b| {
            let alert = prefs.bundles.get(&b.key).copied().unwrap_or_else(|| BundleAlert::default_for(b, true));
            let left = format!("{} {}", format::amount_short(alert.threshold(b), b.unit), i18n::unit_name(b.unit, lang));
            if ar { format!("متبقي {left} فقط من باقة {package}") } else { format!("Only {left} left on {package}") }
        })
        .unwrap_or_default();
    let id = account.id.clone();

    rsx! {
        Head { kicker: format!("{carrier_name} · {}", account.pretty_number()), title: display.clone() }
        div { class: "m-body tight",
            div { class: "caps", "{tx.quota_alerts}" }
            div { class: "list",
                for bundle in usage.bundles.clone() {
                    {
                        let key = bundle.key.clone();
                        let alert = prefs.bundles.get(&key).copied().unwrap_or_else(|| BundleAlert::default_for(&bundle, false));
                        let sub = format!(
                            "{} · {} / {} {}",
                            i18n::kind_name(bundle.kind, lang),
                            format::amount(bundle.remaining, bundle.unit),
                            format::amount_short(bundle.total, bundle.unit),
                            i18n::unit_name(bundle.unit, lang)
                        );
                        let text = texts.read().get(&key).cloned().unwrap_or_default();
                        let (k1, k2, k3, k4) = (key.clone(), key.clone(), key.clone(), key.clone());
                        let unit_name = i18n::unit_name(bundle.unit, lang);
                        let is_gb = bundle.unit == Unit::Gb;
                        let dim = if alert.enabled { "" } else { "dim" };
                        rsx! {
                            div { key: "{key}", class: "alert-row",
                                Toggle {
                                    on: alert.enabled,
                                    onclick: move |_| {
                                        if let Some(a) = draft.write().bundles.get_mut(&k1) {
                                            a.enabled = !a.enabled;
                                        }
                                    },
                                }
                                div { class: "nm",
                                    div { class: "a", "{bundle.name}" }
                                    div { class: "b", "{sub}" }
                                }
                                input {
                                    class: "small-field {dim}",
                                    inputmode: "decimal",
                                    initial_value: "{text}",
                                    oninput: move |e| {
                                        let v = e.value();
                                        if let Ok(n) = v.trim().parse::<f64>()
                                            && let Some(a) = draft.write().bundles.get_mut(&k2) {
                                                a.value = n.max(0.0);
                                            }
                                        texts.write().insert(k2.clone(), v);
                                    },
                                }
                                div { class: "seg {dim}",
                                    if is_gb {
                                        button {
                                            class: if alert.mode == ThresholdMode::Amount { "on" } else { "" },
                                            onclick: move |_| if let Some(a) = draft.write().bundles.get_mut(&k3) { a.mode = ThresholdMode::Amount },
                                            "{unit_name}"
                                        }
                                        button {
                                            class: if alert.mode == ThresholdMode::Percent { "on" } else { "" },
                                            onclick: move |_| if let Some(a) = draft.write().bundles.get_mut(&k4) { a.mode = ThresholdMode::Percent },
                                            "%"
                                        }
                                    } else {
                                        span { class: "on", "{unit_name}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            div { class: "caps", style: "margin-top:6px", "{tx.renewal_reminder}" }
            div { class: "list",
                div { class: "renew-row",
                    Toggle { on: prefs.renewal.enabled, onclick: move |_| { let mut d = draft.write(); d.renewal.enabled = !d.renewal.enabled; } }
                    span { class: "strong", "{tx.remind_me}" }
                    input {
                        class: "small-field w52",
                        inputmode: "numeric",
                        initial_value: "{lead_text.peek()}",
                        oninput: move |e| {
                            let v = e.value();
                            if let Ok(n) = v.trim().parse::<u32>() {
                                draft.write().renewal.value = n.min(999);
                            }
                            lead_text.set(v);
                        },
                    }
                    div { class: "seg",
                        for (unit, label) in [(LeadUnit::Days, tx.days), (LeadUnit::Hours, tx.hours)] {
                            button {
                                class: if prefs.renewal.unit == unit { "on" } else { "" },
                                onclick: move |_| draft.write().renewal.unit = unit,
                                "{label}"
                            }
                        }
                    }
                    span { class: "t muted", "{tx.before_renewal}" }
                }
                div { class: "renew-row",
                    Toggle { on: prefs.renewal_day, onclick: move |_| { let mut d = draft.write(); d.renewal_day = !d.renewal_day; } }
                    span { class: "t", "{tx.renew_day}" }
                }
            }
            div { class: "flabel", style: "margin:4px 0 0", "{tx.preview}" }
            div { class: "preview-card",
                div { class: "sq" }
                div { class: "text",
                    div { class: "a", "{carrier_name} · {display}" }
                    div { class: "b", "{preview_body}" }
                }
            }
        }
        div { class: "m-foot end",
            button { class: "btn-outline", onclick: move |_| m.set(Modal::None), "{tx.cancel}" }
            button {
                class: "btn-primary",
                onclick: move |_| {
                    let prefs = draft.peek().clone();
                    let id = id.clone();
                    shared.update(|d| {
                        if let Some(a) = d.account_mut(&id) {
                            a.alerts = prefs;
                            // New thresholds re-arm alerts that already fired this cycle.
                            a.fired.clear();
                        }
                    });
                    m.set(Modal::None);
                },
                "{tx.save}"
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// Settings

#[component]
fn SettingsDialog() -> Element {
    let shared = use_shared();
    let snap = use_snap();
    let ui = use_ui();
    let settings = snap.read().data.settings.clone();
    let lang = settings.lang;
    let tx = strings(lang);
    let mut m = ui.modal;
    let ar = lang == Lang::Ar;
    let refresh_opts = [
        (RefreshEvery::Minutes15, if ar { "15 د" } else { "15 min" }),
        (RefreshEvery::Hour1, if ar { "ساعة" } else { "1 hr" }),
        (RefreshEvery::Hours6, if ar { "6 س" } else { "6 hr" }),
    ];
    let (s1, s2, s3, s4, s5) = (shared.clone(), shared.clone(), shared.clone(), shared.clone(), shared.clone());
    let demo = snap.read().demo;

    rsx! {
        Head { kicker: tx.app.to_string(), title: tx.settings_title.to_string() }
        div { class: "m-body settings",
            div {
                div { class: "flabel", style: "margin-bottom:8px", "{tx.theme}" }
                div { class: "themes",
                    for (theme, label) in [(Theme::Light, tx.light), (Theme::Dark, tx.dark), (Theme::Match, tx.match_long)] {
                        button {
                            class: if settings.theme == theme { "theme-card on" } else { "theme-card" },
                            onclick: {
                                let shared = s1.clone();
                                move |_| shared.update(|d| d.settings.theme = theme)
                            },
                            div { class: "sw",
                                for color in theme::swatches(theme) {
                                    div { style: "background:{color}" }
                                }
                            }
                            div { class: "nm", "{label}" }
                        }
                    }
                }
                div { class: "hint", style: "margin-top:8px", "{tx.match_hint}" }
            }
            div { class: "two",
                div {
                    div { class: "flabel", "{tx.language}" }
                    div { class: "seg",
                        for (option, label, font) in [(Lang::En, "English", "'DM Sans', sans-serif"), (Lang::Ar, "العربية", "'Google Sans', sans-serif")] {
                            button {
                                class: if lang == option { "on" } else { "" },
                                style: "font-family:{font}",
                                onclick: {
                                    let shared = s2.clone();
                                    move |_| shared.update(|d| d.settings.lang = option)
                                },
                                "{label}"
                            }
                        }
                    }
                }
                div {
                    div { class: "flabel", "{tx.check_every}" }
                    div { class: "seg refresh",
                        for (option, label) in refresh_opts {
                            button {
                                class: if settings.refresh == option { "on" } else { "" },
                                onclick: {
                                    let shared = s3.clone();
                                    move |_| shared.update(|d| d.settings.refresh = option)
                                },
                                "{label}"
                            }
                        }
                    }
                }
            }
            div { class: "list",
                div { class: "sys-row",
                    span { class: "t", "{tx.startup}" }
                    Toggle {
                        on: settings.launch_at_startup,
                        onclick: move |_| {
                            let on = !s4.settings().launch_at_startup;
                            s4.update(|d| d.settings.launch_at_startup = on);
                            if !demo {
                                autostart::apply(on);
                            }
                        },
                    }
                }
                div { class: "sys-row",
                    span { class: "t", "{tx.tray}" }
                    Toggle {
                        on: settings.keep_in_tray,
                        onclick: move |_| s5.update(|d| d.settings.keep_in_tray = !d.settings.keep_in_tray),
                    }
                }
            }
        }
        div { class: "m-foot",
            span { class: "disclaimer", "{tx.disclaimer}" }
            button { class: "btn-primary", onclick: move |_| m.set(Modal::None), "{tx.done}" }
        }
    }
}

// ---------------------------------------------------------------------------------------
// Captcha typed by the user

#[component]
pub fn CaptchaPrompt() -> Element {
    let snap = use_snap();
    let ui = use_ui();
    let lang = snap.read().data.settings.lang;
    let tx = strings(lang);
    let mut code = use_signal(String::new);
    let mut captcha = ui.captcha;
    let Some(uri) = captcha.read().as_ref().map(|c| c.uri.clone()) else { return rsx! {} };

    let mut answer = move |value: Option<String>| {
        if let Some(reply) = captcha.write().take().and_then(|c| c.reply) {
            let _ = reply.send(value);
        }
        code.set(String::new());
    };

    rsx! {
        div { class: "overlay top",
            div { class: "modal small",
                div { class: "m-head",
                    div {
                        div { class: "kicker", "{i18n::carrier_name(Carrier::We, lang)}" }
                        div { class: "title", "{tx.captcha_title}" }
                    }
                    button { class: "m-close", onclick: move |_| answer(None), Icon { svg: icons::CLOSE_16 } }
                }
                div { class: "m-body",
                    div { class: "hint", style: "margin:0;font-size:13px", "{tx.captcha_body}" }
                    img { class: "captcha-img", src: "{uri}", alt: "" }
                    input {
                        class: "field focus",
                        dir: "ltr",
                        placeholder: tx.captcha_ph,
                        autofocus: true,
                        oninput: move |e| code.set(e.value()),
                        onkeydown: move |e| if e.key() == Key::Enter && !code.peek().trim().is_empty() { answer(Some(code.peek().trim().to_string())) },
                    }
                }
                div { class: "m-foot end",
                    button { class: "btn-outline", onclick: move |_| answer(None), "{tx.cancel}" }
                    button {
                        class: "btn-primary",
                        disabled: code().trim().is_empty(),
                        onclick: move |_| answer(Some(code.peek().trim().to_string())),
                        "{tx.continue_}"
                    }
                }
            }
        }
    }
}
