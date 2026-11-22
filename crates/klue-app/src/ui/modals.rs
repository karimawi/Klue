//! Dialogs: add a number, rename, alerts, settings, and the captcha prompt.

use std::collections::BTreeMap;

use iced::{
    Alignment, Color, Length, Task,
    font::Weight,
    widget::{Space, button, column, container, image, row, scrollable, stack, text_input},
};
use klue_core::{
    Carrier, FetchError, LineType, Unit, Usage, format,
    store::{AlertPrefs, BundleAlert, Lang, LeadUnit, RefreshEvery, Theme, ThresholdMode},
};

use super::{Ctx, El, gap, hline, tile, window::field};
use crate::{
    app::{self, App, CaptchaAsk, Message, ModalKind},
    assets::Icon,
    i18n,
    shared::Snapshot,
    theme,
};

/// Fields that take the focus when their dialog opens.
pub const ADD_NUMBER: &str = "add-number";
const ADD_PASSWORD: &str = "add-password";
const ADD_NICKNAME: &str = "add-nickname";
pub const RENAME_INPUT: &str = "rename-nickname";
pub const CAPTCHA_INPUT: &str = "captcha-input";

pub enum Modal {
    None,
    Add(AddForm),
    Rename(RenameForm),
    Alerts(AlertsForm),
    Settings,
}

pub struct AddForm {
    carrier: Carrier,
    line: LineType,
    number: String,
    password: String,
    show_password: bool,
    nickname: String,
    phase: Phase,
}

#[derive(Debug, Clone)]
enum Phase {
    Form,
    Verifying,
    Verified(Box<Usage>),
    Error(FetchError),
}

pub struct RenameForm {
    id: String,
    draft: String,
    confirm_remove: bool,
}

pub struct AlertsForm {
    id: String,
    draft: AlertPrefs,
    /// What's typed in each threshold field, by bundle key.
    values: BTreeMap<String, String>,
    lead: String,
}

#[derive(Debug, Clone)]
pub enum Msg {
    AddCarrier(Carrier),
    AddLine(LineType),
    AddNumber(String),
    AddPassword(String),
    AddShowPassword,
    AddNickname(String),
    AddSubmit,
    AddResult(Result<Usage, FetchError>),

    RenameInput(String),
    RenameUseOwner,
    RenameSave,
    Remove,

    AlertEnabled(String),
    AlertValue(String, String),
    AlertMode(String, ThresholdMode),
    ReminderEnabled,
    ReminderValue(String),
    ReminderUnit(LeadUnit),
    RenewalDay,
    AlertsSave,
    /// The switch on the dashboard: saved right away.
    ToggleReminder(String),

    Startup,
    KeepInTray,
    RefreshEvery(RefreshEvery),
}

impl Modal {
    pub fn open(kind: ModalKind, snap: &Snapshot, carrier: Carrier) -> Self {
        match kind {
            ModalKind::Add => {
                let carrier = if carrier.is_supported() { carrier } else { Carrier::We };
                Modal::Add(AddForm {
                    carrier,
                    line: carrier.default_line(),
                    number: String::new(),
                    password: String::new(),
                    show_password: false,
                    nickname: String::new(),
                    phase: Phase::Form,
                })
            }
            ModalKind::Rename(id) => match snap.data.account(&id) {
                Some(a) => Modal::Rename(RenameForm { id, draft: a.nickname.clone().unwrap_or_default(), confirm_remove: false }),
                None => Modal::None,
            },
            ModalKind::Alerts(id) => match snap.data.account(&id) {
                Some(a) => {
                    let mut draft = a.alerts.clone();
                    let main_key = a.main_bundle().map(|b| b.key.clone());
                    if let Some(usage) = &a.usage {
                        for b in &usage.bundles {
                            draft.bundles.entry(b.key.clone()).or_insert_with(|| BundleAlert::default_for(b, main_key.as_deref() == Some(b.key.as_str())));
                        }
                    }
                    let values = draft.bundles.iter().map(|(k, v)| (k.clone(), format::amount_short(v.value, Unit::Gb))).collect();
                    let lead = draft.renewal.value.to_string();
                    Modal::Alerts(AlertsForm { id, draft, values, lead })
                }
                None => Modal::None,
            },
            ModalKind::Settings => Modal::Settings,
        }
    }
}

pub fn update(app: &mut App, msg: Msg) -> Task<Message> {
    let shared = app.shared.clone();
    match (&mut app.modal, msg) {
        (Modal::Add(f), Msg::AddCarrier(c)) if c.is_supported() && !matches!(f.phase, Phase::Verifying) => {
            f.carrier = c;
            f.line = c.default_line();
            f.phase = Phase::Form;
        }
        (Modal::Add(f), Msg::AddLine(l)) if f.carrier.supports_line(l) => f.line = l,
        (Modal::Add(f), Msg::AddNumber(n)) => {
            f.number = n;
            if matches!(f.phase, Phase::Error(_)) {
                f.phase = Phase::Form;
            }
        }
        (Modal::Add(f), Msg::AddPassword(p)) => {
            f.password = p;
            if matches!(f.phase, Phase::Error(_)) {
                f.phase = Phase::Form;
            }
        }
        (Modal::Add(f), Msg::AddShowPassword) => f.show_password = !f.show_password,
        (Modal::Add(f), Msg::AddNickname(n)) => f.nickname = n,
        (Modal::Add(f), Msg::AddSubmit) => match &f.phase {
            Phase::Verifying => {}
            Phase::Verified(usage) => {
                let usage = (**usage).clone();
                match shared.add_account(f.carrier, f.line, &f.number, &f.password, Some(f.nickname.clone()), usage) {
                    Ok(id) => {
                        app.modal = Modal::None;
                        app.snap = shared.snapshot();
                        return Task::done(Message::SelectAccount(id));
                    }
                    Err(e) => f.phase = Phase::Error(e),
                }
            }
            Phase::Form | Phase::Error(_) => {
                if f.number.trim().is_empty() || f.password.is_empty() {
                    return Task::none();
                }
                f.phase = Phase::Verifying;
                let (carrier, line, number, password) = (f.carrier, f.line, f.number.clone(), f.password.clone());
                return Task::perform(async move { shared.verify(carrier, line, &number, &password).await }, |r| Message::Modal(Msg::AddResult(r)));
            }
        },
        (Modal::Add(f), Msg::AddResult(result)) => {
            // Carry on typing where it makes sense: a nickname, or the password again.
            let next = match &result {
                Ok(_) => Some(ADD_NICKNAME),
                Err(FetchError::BadCredentials) => Some(ADD_PASSWORD),
                Err(_) => None,
            };
            f.phase = match result {
                Ok(usage) => Phase::Verified(Box::new(usage)),
                Err(e) => Phase::Error(e),
            };
            if let Some(id) = next {
                return iced::widget::operation::focus(id);
            }
        }

        (Modal::Rename(f), Msg::RenameInput(t)) => f.draft = t,
        (Modal::Rename(f), Msg::RenameUseOwner) => f.draft.clear(),
        (Modal::Rename(f), Msg::RenameSave) => {
            shared.rename(&f.id, &f.draft);
            app.modal = Modal::None;
        }
        (Modal::Rename(f), Msg::Remove) => {
            if f.confirm_remove {
                shared.remove_account(&f.id);
                app.modal = Modal::None;
            } else {
                f.confirm_remove = true;
            }
        }

        (Modal::Alerts(f), Msg::AlertEnabled(key)) => {
            if let Some(a) = f.draft.bundles.get_mut(&key) {
                a.enabled = !a.enabled;
            }
        }
        (Modal::Alerts(f), Msg::AlertValue(key, text)) => {
            let text: String = text.chars().filter(|c| c.is_ascii_digit() || *c == '.').take(7).collect();
            if let Ok(n) = text.parse::<f64>()
                && let Some(a) = f.draft.bundles.get_mut(&key)
            {
                a.value = n.max(0.0);
            }
            f.values.insert(key, text);
        }
        (Modal::Alerts(f), Msg::AlertMode(key, mode)) => {
            if let Some(a) = f.draft.bundles.get_mut(&key) {
                a.mode = mode;
            }
        }
        (Modal::Alerts(f), Msg::ReminderEnabled) => f.draft.renewal.enabled = !f.draft.renewal.enabled,
        (Modal::Alerts(f), Msg::ReminderValue(text)) => {
            let text: String = text.chars().filter(char::is_ascii_digit).take(3).collect();
            if let Ok(n) = text.parse::<u32>() {
                f.draft.renewal.value = n.max(1);
            }
            f.lead = text;
        }
        (Modal::Alerts(f), Msg::ReminderUnit(u)) => f.draft.renewal.unit = u,
        (Modal::Alerts(f), Msg::RenewalDay) => f.draft.renewal_day = !f.draft.renewal_day,
        (Modal::Alerts(f), Msg::AlertsSave) => {
            let (id, prefs) = (f.id.clone(), f.draft.clone());
            shared.update(|d| {
                if let Some(a) = d.account_mut(&id) {
                    a.alerts = prefs;
                    // New thresholds re-arm alerts that already fired this cycle.
                    a.fired.clear();
                }
            });
            app.modal = Modal::None;
        }
        (_, Msg::ToggleReminder(id)) => shared.update(|d| {
            if let Some(a) = d.account_mut(&id) {
                a.alerts.renewal.enabled = !a.alerts.renewal.enabled;
            }
        }),

        (_, Msg::Startup) => {
            let on = !shared.settings().launch_at_startup;
            app::set_launch_at_startup(&shared, on);
        }
        (_, Msg::KeepInTray) => shared.update(|d| d.settings.keep_in_tray = !d.settings.keep_in_tray),
        (_, Msg::RefreshEvery(r)) => shared.update(|d| d.settings.refresh = r),
        _ => {}
    }
    Task::none()
}

pub fn view<'a>(ctx: &Ctx<'a>) -> Option<El<'a>> {
    match &ctx.app.modal {
        Modal::None => None,
        Modal::Add(f) => Some(add(ctx, f)),
        Modal::Rename(f) => ctx.app.snap.data.account(&f.id).map(|a| rename(ctx, f, a)),
        Modal::Alerts(f) => ctx.app.snap.data.account(&f.id).map(|a| alerts(ctx, f, a)),
        Modal::Settings => Some(settings(ctx)),
    }
}

// ---------------------------------------------------------------------------------------
// Frame

fn dialog<'a>(ctx: &Ctx<'a>, width: f32, kicker: String, title: String, close: Message, body: El<'a>, foot: El<'a>) -> El<'a> {
    let p = ctx.p;
    let close = button(container(ctx.icon(Icon::Close, 16.0, p.tx)).center(Length::Fill)).width(32).height(32).padding(0).style(theme::ghost(p.tx, p.sf, 9.0)).on_press(close);
    let head = ctx
        .row(vec![
            container(column![ctx.kicker(&kicker), ctx.bold(title, 21.0)].spacing(2).align_x(ctx.start())).width(Length::Fill).align_x(ctx.start()).into(),
            close.into(),
        ])
        .align_y(Alignment::Start)
        .spacing(16);
    let body = scrollable(container(body).padding([18, 22])).style(theme::scroller(p)).height(Length::Shrink);
    container(column![container(head).padding(super::pad4(18.0, 22.0, 14.0, 22.0)), hline(p.dv, 2.0), body, hline(p.dv, 2.0), container(foot).padding([14, 22])])
        .width(width)
        .max_height(720)
        .style(move |_| theme::panel(p, 16.0))
        .clip(true)
        .into()
}

fn label<'a>(ctx: &Ctx<'a>, text: &str) -> El<'a> {
    container(ctx.muted(text.to_string(), 12.0)).width(Length::Fill).align_x(ctx.start()).into()
}

fn notice<'a>(ctx: &Ctx<'a>, e: &FetchError, carrier: Carrier) -> El<'a> {
    let p = ctx.p;
    let text = column![ctx.bold(i18n::error_title(e, ctx.lang), 14.0), ctx.body(i18n::error_text(e, carrier, ctx.lang), 13.0)].spacing(2).align_x(ctx.start()).width(Length::Fill);
    container(ctx.row(vec![ctx.icon(Icon::Warning, 18.0, p.danger), text.into()]).spacing(12).align_y(Alignment::Start))
        .padding([12, 14])
        .style(move |_| theme::fill(theme::fade(p.danger, 0.12), 10.0))
        .into()
}

/// A read-only box in the shape of a field (the owner name).
fn readout<'a>(ctx: &Ctx<'a>, text: String, highlight: bool, check: bool) -> El<'a> {
    let p = ctx.p;
    let mut items = vec![container(ctx.text(text, 14.0, if highlight { Weight::Semibold } else { Weight::Normal }, if highlight { p.tx } else { p.mu })).width(Length::Fill).align_x(ctx.start()).into()];
    if check {
        items.push(ctx.icon(Icon::Check, 16.0, p.ac_tx));
    }
    container(ctx.row(items).spacing(8))
        .height(38)
        .padding([0, 10])
        .align_y(Alignment::Center)
        .style(move |_| theme::outlined(if highlight { p.sf } else { Color::TRANSPARENT }, if highlight { p.ac } else { p.dv }, 9.0))
        .into()
}

// ---------------------------------------------------------------------------------------
// Add a number

fn add<'a>(ctx: &Ctx<'a>, f: &'a AddForm) -> El<'a> {
    let p = ctx.p;
    let s = ctx.s;
    let ar = ctx.rtl();
    let verifying = matches!(f.phase, Phase::Verifying);
    let verified = matches!(f.phase, Phase::Verified(_));
    let error = match &f.phase {
        Phase::Error(e) => Some(e),
        _ => None,
    };
    let carrier_name = i18n::carrier_name(f.carrier, ctx.lang);
    let app_name = i18n::carrier_app(f.carrier, ctx.lang);

    let carriers: Vec<El<'a>> = Carrier::ALL
        .into_iter()
        .map(|c| {
            let on = c == f.carrier;
            let soon = !c.is_supported();
            let t = crate::assets::tile(c);
            let content = ctx.row(vec![tile(c, 24.0, t.small, 7.0, soon), ctx.text(i18n::carrier_name(c, ctx.lang), 13.0, Weight::Semibold, if soon { p.mu } else { p.tx }).into()]).spacing(8);
            let b = button(content)
                .width(Length::Fill)
                .padding([7, 8])
                .style(move |_, status| {
                    let hovered = matches!(status, button::Status::Hovered) && !soon;
                    button::Style {
                        background: (on || hovered).then_some(iced::Background::Color(p.sf)),
                        border: theme::border(if on { p.ac } else { p.dv }, 2.0, 9.0),
                        text_color: p.tx,
                        ..Default::default()
                    }
                })
                .on_press_maybe((!soon && !verifying).then_some(Message::Modal(Msg::AddCarrier(c))));
            let el: El<'a> = if soon {
                let badge = container(ctx.text(s.soon, 9.5, Weight::Semibold, p.bg)).padding([1, 6]).style(move |_| theme::fill(p.tx, 8.0));
                stack([container(b).padding(super::pad4(8.0, 0.0, 0.0, 0.0)).into(), container(badge).width(Length::Fill).align_x(ctx.end()).into()]).into()
            } else {
                container(b).padding(super::pad4(8.0, 0.0, 0.0, 0.0)).into()
            };
            container(el).width(Length::FillPortion(1)).into()
        })
        .collect();

    let line_seg = ctx.segmented(
        [(LineType::Mobile, s.mobile), (LineType::Landline, s.landline)]
            .into_iter()
            .map(|(l, label)| (label.to_string(), f.line == l, (f.carrier.supports_line(l) && !verifying).then_some(Message::Modal(Msg::AddLine(l)))))
            .collect(),
        38.0,
        13.0,
    );
    let placeholder = if f.line == LineType::Mobile { "010 1234 5678" } else { "02 2345 6789" };
    let mut number = field(ctx, placeholder, &f.number, |t| Message::Modal(Msg::AddNumber(t)), false).font(crate::assets::numbers(Weight::Normal)).on_submit(Message::Modal(Msg::AddSubmit));
    if verifying || verified {
        number = text_input(placeholder, &f.number).padding([9, 10]).size(14).font(crate::assets::numbers(Weight::Normal)).style(theme::field(p, false));
    }
    let number = number.id(ADD_NUMBER);
    let pair = ctx.row(vec![
        column![label(ctx, s.line_type), gap(6.0), container(line_seg).width(Length::Shrink)].width(Length::Shrink).into(),
        column![label(ctx, s.number), gap(6.0), number].width(Length::Fill).into(),
    ])
    .spacing(12)
    .align_y(Alignment::Start);

    let bad_password = error == Some(&FetchError::BadCredentials);
    let mut password = text_input("", &f.password)
        .id(ADD_PASSWORD)
        .secure(!f.show_password)
        .padding([9, 10])
        .size(14)
        .font(crate::assets::numbers(Weight::Normal))
        .style(theme::field(p, bad_password))
        .on_submit(Message::Modal(Msg::AddSubmit));
    if !(verifying || verified) {
        password = password.on_input(|t| Message::Modal(Msg::AddPassword(t)));
    }
    let eye = button(container(ctx.icon(if f.show_password { Icon::EyeOff } else { Icon::Eye }, 16.0, p.mu)).center(Length::Fill))
        .width(38)
        .height(38)
        .padding(0)
        .style(theme::ghost(p.mu, p.sf, 9.0))
        .on_press(Message::Modal(Msg::AddShowPassword));
    let hint = if ar { format!("نفس كلمة المرور المستخدمة في {app_name}.") } else { format!("The same password you use in {app_name}.") };
    let mut pass_col = column![label(ctx, s.password), gap(6.0), ctx.row(vec![password.into(), eye.into()]).spacing(6), gap(6.0), ctx.muted(hint, 12.0)].align_x(ctx.start());
    if verifying {
        pass_col = pass_col.push(gap(8.0)).push(ctx.meter(0.35, 3.0, p.ac));
    }

    let owner = match &f.phase {
        Phase::Verified(u) => u.owner_name.clone().unwrap_or_else(|| s.owner_unknown.into()),
        Phase::Verifying => s.fetching.into(),
        _ => s.auto_fill.into(),
    };
    let mut owner_label = vec![label(ctx, s.owner_name)];
    if verified {
        owner_label.push(container(ctx.text(s.from_account, 11.0, Weight::Semibold, p.ac_tx)).width(Length::Shrink).into());
    }

    let mut body = column![
        column![label(ctx, s.carrier), ctx.row(carriers).spacing(6)].spacing(0),
        pair,
        pass_col,
    ]
    .spacing(16);
    if let Some(e) = error {
        body = body.push(notice(ctx, e, f.carrier));
    }
    body = body.push(column![ctx.row(owner_label), gap(6.0), readout(ctx, owner, verified, verified)]);
    if verified {
        body = body.push(column![
            label(ctx, s.nick_optional),
            gap(6.0),
            field(ctx, s.nick_ph, &f.nickname, |t| Message::Modal(Msg::AddNickname(t)), false).on_submit(Message::Modal(Msg::AddSubmit)).id(ADD_NICKNAME)
        ]);
    }

    let status = match &f.phase {
        Phase::Verified(_) => format!("✓ {}", s.verified),
        Phase::Verifying => {
            if ar {
                format!("جارٍ تسجيل الدخول إلى {carrier_name}…")
            } else {
                format!("Signing in to {carrier_name}…")
            }
        }
        _ => String::new(),
    };
    let primary = match &f.phase {
        Phase::Verified(_) => s.add_number,
        Phase::Verifying => s.verifying,
        Phase::Error(_) => s.try_again,
        Phase::Form => s.verify,
    };
    let ready = !f.number.trim().is_empty() && !f.password.is_empty();
    let foot = ctx.row(vec![
        container(ctx.muted(status, 12.5)).width(Length::Fill).align_x(ctx.start()).into(),
        ctx.outline(s.cancel, Some(Message::CloseModal)).into(),
        ctx.primary(primary, (!verifying && (verified || ready)).then_some(Message::Modal(Msg::AddSubmit))).width(150).into(),
    ])
    .spacing(8);
    dialog(ctx, 520.0, s.new_number.to_string(), s.add_title.to_string(), Message::CloseModal, body.into(), foot.into())
}

// ---------------------------------------------------------------------------------------
// Rename

fn rename<'a>(ctx: &Ctx<'a>, f: &'a RenameForm, account: &'a klue_core::store::Account) -> El<'a> {
    let p = ctx.p;
    let s = ctx.s;
    let owner = account.owner_name.clone().unwrap_or_else(|| s.owner_unknown.into());
    let fallback = account.owner_name.clone().unwrap_or_else(|| account.pretty_number());
    let shown = if f.draft.trim().is_empty() { fallback } else { f.draft.clone() };
    let icon = if account.line == LineType::Mobile { Icon::Mobile } else { Icon::Landline };
    let preview = container(
        ctx.row(vec![
            ctx.icon(icon, 16.0, p.tx),
            column![ctx.bold(shown, 14.0), ctx.num(account.pretty_number(), 11.5, Weight::Normal, p.tx)].spacing(0).align_x(ctx.start()).into(),
        ])
        .spacing(10),
    )
    .padding([10, 14])
    .style(move |_| {
        let mut st = theme::fill(p.sf, 12.0);
        st.border = iced::Border { color: p.ac, width: 0.0, radius: iced::border::Radius { top_left: 12.0, top_right: 12.0, bottom_left: 0.0, bottom_right: 0.0 } };
        st
    });
    let preview = column![container(Space::new()).width(Length::Fill).height(3).style(move |_| theme::fill(p.ac, 2.0)), preview].width(Length::Shrink);
    let danger = ctx
        .row(vec![
            container(ctx.muted(s.remove_hint, 12.5)).width(Length::Fill).align_x(ctx.start()).into(),
            button(ctx.text(if f.confirm_remove { s.remove_confirm } else { s.remove }, 13.0, Weight::ExtraBold, p.danger))
                .padding([4, 6])
                .style(theme::ghost(p.danger, p.sf, 9.0))
                .on_press(Message::Modal(Msg::Remove))
                .into(),
        ])
        .spacing(10);
    let body = column![
        column![ctx.row(vec![label(ctx, s.owner_name), ctx.muted(s.from_account, 11.0).into()]), gap(6.0), readout(ctx, owner.clone(), false, false)],
        column![
            label(ctx, s.nickname),
            gap(6.0),
            field(ctx, &owner, &f.draft, |t| Message::Modal(Msg::RenameInput(t)), false).on_submit(Message::Modal(Msg::RenameSave)).id(RENAME_INPUT),
            gap(6.0),
            ctx.muted(s.nick_hint, 12.0)
        ]
        .align_x(ctx.start()),
        column![label(ctx, s.shown_as), gap(6.0), container(preview).width(Length::Fill).align_x(ctx.start())],
        column![hline(p.dvs, 1.0), gap(12.0), danger],
    ]
    .spacing(16);
    let foot = ctx.row(vec![
        ctx.link(s.use_owner, Message::Modal(Msg::RenameUseOwner)).into(),
        super::fill_x(),
        ctx.outline(s.cancel, Some(Message::CloseModal)).into(),
        ctx.primary(s.save, Some(Message::Modal(Msg::RenameSave))).into(),
    ])
    .spacing(8);
    let kicker = format!("{} · {}", i18n::carrier_name(account.carrier, ctx.lang), account.pretty_number());
    dialog(ctx, 520.0, kicker, s.rename_title.to_string(), Message::CloseModal, body.into(), foot.into())
}

// ---------------------------------------------------------------------------------------
// Alerts

fn alerts<'a>(ctx: &Ctx<'a>, f: &'a AlertsForm, account: &'a klue_core::store::Account) -> El<'a> {
    let p = ctx.p;
    let s = ctx.s;
    let ar = ctx.rtl();
    let Some(usage) = &account.usage else { return Space::new().into() };
    let small = |value: &str, on_input: Box<dyn Fn(String) -> Message + 'a>, width: f32, dim: bool| -> El<'a> {
        text_input("", value)
            .on_input(on_input)
            .padding([6, 8])
            .size(14)
            .width(width)
            .font(crate::assets::numbers(Weight::Semibold))
            .style(move |t, st| {
                let mut style = theme::field(p, false)(t, st);
                if dim {
                    style.value = theme::fade(p.tx, 0.4);
                }
                style
            })
            .into()
    };

    let mut quota = column![ctx.caps(s.quota_alerts), gap(6.0), hline(p.dv, 2.0)].align_x(ctx.start());
    for bundle in usage.ordered(account.main_bundle.as_deref()) {
        let key = bundle.key.clone();
        let alert = f.draft.bundles.get(&key).copied().unwrap_or_else(|| BundleAlert::default_for(bundle, false));
        let unit_name = i18n::unit_name(bundle.unit, ctx.lang);
        let sub = format!(
            "{} · {} / {} {}",
            i18n::kind_name(bundle.kind, ctx.lang),
            format::amount(bundle.remaining, bundle.unit),
            format::amount_short(bundle.total, bundle.unit),
            unit_name
        );
        let value = f.values.get(&key).cloned().unwrap_or_default();
        let k = key.clone();
        let field = small(&value, Box::new(move |t| Message::Modal(Msg::AlertValue(k.clone(), t))), 64.0, !alert.enabled);
        let modes: El<'a> = if matches!(bundle.unit, Unit::Gb | Unit::Mb) {
            ctx.segmented(
                vec![
                    (unit_name.to_string(), alert.mode == ThresholdMode::Amount, Some(Message::Modal(Msg::AlertMode(key.clone(), ThresholdMode::Amount)))),
                    ("%".to_string(), alert.mode == ThresholdMode::Percent, Some(Message::Modal(Msg::AlertMode(key.clone(), ThresholdMode::Percent)))),
                ],
                32.0,
                12.5,
            )
        } else {
            ctx.segmented(vec![(unit_name.to_string(), true, None)], 32.0, 12.5)
        };
        let row_el = ctx
            .row(vec![
                container(ctx.toggle(alert.enabled, Message::Modal(Msg::AlertEnabled(key.clone())))).width(40).into(),
                container(column![ctx.semi(bundle.name.clone(), 13.5), ctx.muted(sub, 11.5)].spacing(0).align_x(ctx.start())).width(Length::Fill).align_x(ctx.start()).into(),
                field,
                modes,
            ])
            .spacing(12);
        quota = quota.push(container(row_el).padding([10, 0])).push(hline(p.dvs, 1.0));
    }

    let lead = small(&f.lead, Box::new(|t| Message::Modal(Msg::ReminderValue(t))), 52.0, !f.draft.renewal.enabled);
    let units = ctx.segmented(
        [(LeadUnit::Days, s.days), (LeadUnit::Hours, s.hours)]
            .into_iter()
            .map(|(u, label)| (label.to_string(), f.draft.renewal.unit == u, Some(Message::Modal(Msg::ReminderUnit(u)))))
            .collect(),
        32.0,
        12.5,
    );
    let reminder = ctx
        .row(vec![
            ctx.toggle(f.draft.renewal.enabled, Message::Modal(Msg::ReminderEnabled)),
            ctx.semi(s.remind_me, 13.5).into(),
            lead,
            units,
            ctx.muted(s.before_renewal, 13.5).into(),
        ])
        .spacing(10);
    let renew_day = ctx.row(vec![ctx.toggle(f.draft.renewal_day, Message::Modal(Msg::RenewalDay)), ctx.body(s.renew_day, 13.5).into()]).spacing(10);
    let renewal = column![ctx.caps(s.renewal_reminder), gap(6.0), hline(p.dv, 2.0), container(reminder).padding([10, 0]), hline(p.dvs, 1.0), container(renew_day).padding([10, 0]), hline(p.dvs, 1.0)]
        .align_x(ctx.start());

    let package = usage.package_name.clone().unwrap_or_default();
    let preview_body = account
        .main_bundle()
        .map(|b| {
            let alert = f.draft.bundles.get(&b.key).copied().unwrap_or_else(|| BundleAlert::default_for(b, true));
            let left = format!("{} {}", format::amount_short(alert.threshold(b), b.unit), i18n::unit_name(b.unit, ctx.lang));
            if ar { format!("متبقي {left} فقط من باقة {package}") } else { format!("Only {left} left on {package}") }
        })
        .unwrap_or_default();
    let carrier_name = i18n::carrier_name(account.carrier, ctx.lang);
    let preview = container(
        ctx.row(vec![
            super::logo(16.0),
            column![ctx.bold(format!("{carrier_name} · {}", account.display_name()), 13.5), ctx.muted(preview_body, 12.5)].spacing(2).align_x(ctx.start()).into(),
        ])
        .spacing(12)
        .align_y(Alignment::Start),
    )
    .width(Length::Fill)
    .padding([12, 14])
    .style(theme::bg_line(p.sf, p.dvs, 12.0));
    let body = column![quota, renewal, column![label(ctx, s.preview), gap(6.0), preview]].spacing(16);
    let foot = ctx.row(vec![super::fill_x(), ctx.outline(s.cancel, Some(Message::CloseModal)).into(), ctx.primary(s.save, Some(Message::Modal(Msg::AlertsSave))).into()]).spacing(8);
    let kicker = format!("{carrier_name} · {}", account.pretty_number());
    dialog(ctx, 560.0, kicker, account.display_name(), Message::CloseModal, body.into(), foot.into())
}

// ---------------------------------------------------------------------------------------
// Settings

fn settings<'a>(ctx: &Ctx<'a>) -> El<'a> {
    let p = ctx.p;
    let s = ctx.s;
    let ar = ctx.rtl();
    let settings = &ctx.app.snap.data.settings;
    let cards: Vec<El<'a>> = [(Theme::Light, s.light), (Theme::Dark, s.dark), (Theme::Match, s.match_long)]
        .into_iter()
        .map(|(t, name)| {
            let on = settings.theme == t;
            let swatches = row(theme::swatches(t).into_iter().map(|c| container(Space::new()).width(Length::Fill).height(46).style(move |_| theme::fill(c, 0.0)).into())).width(Length::Fill);
            let card = column![swatches, container(ctx.text(name, 13.0, if on { Weight::ExtraBold } else { Weight::Normal }, p.tx)).padding([8, 10]).width(Length::Fill).align_x(ctx.start())];
            button(card)
                .padding(0)
                .width(Length::FillPortion(1))
                .style(move |_, _| button::Style { border: theme::border(if on { p.ac } else { p.dv }, 2.0, 12.0), ..Default::default() })
                .on_press(Message::SetTheme(t))
                .into()
        })
        .collect();
    let langs = ctx.segmented(vec![
        ("English".to_string(), settings.lang == Lang::En, Some(Message::SetLang(Lang::En))),
        ("العربية".to_string(), settings.lang == Lang::Ar, Some(Message::SetLang(Lang::Ar))),
    ], 36.0, 13.5);
    let every = ctx.segmented(
        [
            (RefreshEvery::Minutes15, if ar { "15 د" } else { "15 min" }),
            (RefreshEvery::Hour1, if ar { "ساعة" } else { "1 hr" }),
            (RefreshEvery::Hours6, if ar { "6 س" } else { "6 hr" }),
        ]
        .into_iter()
        .map(|(r, label)| (label.to_string(), settings.refresh == r, Some(Message::Modal(Msg::RefreshEvery(r)))))
        .collect(),
        36.0,
        13.0,
    );
    let sys = |text: &str, on: bool, msg: Msg| -> El<'a> {
        container(ctx.row(vec![container(ctx.body(text.to_string(), 13.5)).width(Length::Fill).align_x(ctx.start()).into(), ctx.toggle(on, Message::Modal(msg))]).spacing(12))
            .padding([10, 0])
            .into()
    };
    let body = column![
        column![label(ctx, s.theme), gap(8.0), ctx.row(cards).spacing(8), gap(8.0), ctx.muted(s.match_hint, 12.0)].align_x(ctx.start()),
        ctx.row(vec![
            column![label(ctx, s.language), gap(6.0), langs].width(Length::FillPortion(1)).into(),
            column![label(ctx, s.check_every), gap(6.0), every].width(Length::FillPortion(1)).into(),
        ])
        .spacing(12)
        .align_y(Alignment::Start),
        column![
            hline(p.dv, 2.0),
            sys(s.startup, settings.launch_at_startup, Msg::Startup),
            hline(p.dvs, 1.0),
            sys(s.tray, settings.keep_in_tray, Msg::KeepInTray),
            hline(p.dvs, 1.0)
        ],
    ]
    .spacing(16);
    let foot = ctx.row(vec![container(ctx.muted(s.disclaimer, 11.5)).width(Length::Fill).align_x(ctx.start()).into(), ctx.primary(s.done, Some(Message::CloseModal)).into()]).spacing(12);
    dialog(ctx, 540.0, s.app.to_string(), s.settings_title.to_string(), Message::CloseModal, body.into(), foot.into())
}

// ---------------------------------------------------------------------------------------
// Captcha

pub fn captcha<'a>(ctx: &Ctx<'a>, ask: &'a CaptchaAsk) -> El<'a> {
    let p = ctx.p;
    let s = ctx.s;
    let picture = container(image(ask.image.clone()).height(56)).padding(4).style(move |_| theme::outlined(Color::WHITE, p.dv, 9.0));
    let input = text_input(s.captcha_ph, &ask.input)
        .id(CAPTCHA_INPUT)
        .on_input(Message::CaptchaInput)
        .on_submit(Message::CaptchaSubmit)
        .padding([9, 10])
        .size(15)
        .font(crate::assets::numbers(Weight::Semibold))
        .style(theme::field(p, false));
    let body = column![ctx.muted(s.captcha_body, 13.0), picture, input].spacing(14).align_x(ctx.start());
    let foot = ctx.row(vec![super::fill_x(), ctx.outline(s.cancel, Some(Message::CaptchaCancel)).into(), ctx.primary(s.continue_, (!ask.input.is_empty()).then_some(Message::CaptchaSubmit)).into()]).spacing(8);
    dialog(ctx, 420.0, i18n::carrier_name(Carrier::We, ctx.lang).to_string(), s.captcha_title.to_string(), Message::CaptchaCancel, body.into(), foot.into())
}
