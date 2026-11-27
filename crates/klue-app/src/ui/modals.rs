//! Dialogs: add a number, rename, alerts, settings, archived numbers, and the captcha
//! prompt.

use std::collections::BTreeMap;

use iced::{
    Alignment, Color, Element, Length, Task,
    font::Weight,
    widget::{Space, button, column, container, image, row, scrollable, text_input},
};
use klue_core::{
    Carrier, FetchError, Unit, format,
    store::{AlertPrefs, BundleAlert, Clock, Lang, LeadUnit, RefreshEvery, Theme, ThresholdMode},
};

use super::{Ctx, El, gap, hline, window::field};
use crate::{
    app::{self, App, CaptchaAsk, Message, ModalKind},
    assets::Icon,
    i18n,
    shared::Snapshot,
    theme,
};

/// Fields focused when their dialog opens.
pub const RENAME_INPUT: &str = "rename-nickname";
/// The scrolling body of every dialog.
const BODY: &str = "dialog-body";
pub const CAPTCHA_INPUT: &str = "captcha-input";

pub enum Modal {
    None,
    Add(super::add::Wizard),
    Rename(RenameForm),
    Alerts(AlertsForm),
    Settings(SettingsForm),
    Archived(ArchivedList),
    Export(super::transfer::ExportForm),
    Import(super::transfer::ImportForm),
    Group(super::home::GroupForm),
    GroupLines(super::home::LinesPicker),
}

pub struct ArchivedList {
    /// The number whose Remove was pressed once.
    confirm_remove: Option<String>,
}

pub struct SettingsForm {
    /// Text in the all-numbers threshold and lead-time fields.
    percent: String,
    lead: String,
    /// "Apply to all numbers" was pressed and nothing changed since.
    applied: bool,
}

pub struct RenameForm {
    id: String,
    draft: String,
    /// The number's own refresh interval; `None` follows Settings.
    refresh: Option<RefreshEvery>,
    confirm_remove: bool,
}

pub struct AlertsForm {
    id: String,
    draft: AlertPrefs,
    /// Text in each threshold field, by bundle key.
    values: BTreeMap<String, String>,
    lead: String,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Add(super::add::Msg),
    Transfer(super::transfer::Msg),
    Home(super::home::Msg),

    RenameInput(String),
    RenameRefresh(Option<RefreshEvery>),
    RenameUseOwner,
    RenameSave,
    Remove,
    Archive,
    /// Archived numbers: bring one back, or remove it (pressed twice).
    Restore(String),
    RemoveArchived(String),

    AlertEnabled(String),
    AlertValue(String, String),
    AlertMode(String, ThresholdMode),
    ReminderEnabled,
    ReminderValue(String),
    ReminderUnit(LeadUnit),
    RenewalDay,
    AlertsSave,
    /// The dashboard switch; saved right away.
    ToggleReminder(String),

    Startup,
    KeepInTray,
    RefreshEvery(RefreshEvery),
    Clock(Clock),
    /// Alerts for all numbers: saved as they change, applied to every number on AllApply.
    AllMain,
    AllOthers,
    AllPercent(String),
    AllReminder,
    AllLead(String),
    AllLeadUnit(LeadUnit),
    AllRenewalDay,
    AllApply,
}

impl Modal {
    pub fn open(kind: ModalKind, snap: &Snapshot) -> Self {
        match kind {
            ModalKind::Add(carrier) => Modal::Add(super::add::Wizard::new(carrier)),
            ModalKind::Rename(id) => match snap.data.account(&id) {
                Some(a) => Modal::Rename(RenameForm { id, draft: a.nickname.clone().unwrap_or_default(), refresh: a.refresh, confirm_remove: false }),
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
            ModalKind::Archived => Modal::Archived(ArchivedList { confirm_remove: None }),
            ModalKind::Export(ids) => Modal::Export(super::transfer::ExportForm::new(ids)),
            ModalKind::Group(id) => Modal::Group(super::home::GroupForm::new(id, &snap.data)),
            ModalKind::GroupLines(id) => Modal::GroupLines(super::home::LinesPicker::new(id, &snap.data)),
            ModalKind::Settings => {
                let alerts = snap.data.settings.alerts;
                Modal::Settings(SettingsForm {
                    percent: format::amount_short(alerts.percent_or_default(), Unit::Gb),
                    lead: alerts.renewal.value.to_string(),
                    applied: false,
                })
            }
        }
    }
}

pub fn update(app: &mut App, msg: Msg) -> Task<Message> {
    let msg = match msg {
        Msg::Add(m) => return super::add::update(app, m),
        Msg::Transfer(m) => return super::transfer::update(app, m),
        Msg::Home(m) => return super::home::update(app, m),
        other => other,
    };
    let shared = app.shared.clone();
    match (&mut app.modal, msg) {
        (Modal::Rename(f), Msg::RenameInput(t)) => f.draft = t,
        (Modal::Rename(f), Msg::RenameRefresh(r)) => f.refresh = r,
        (Modal::Rename(f), Msg::RenameUseOwner) => f.draft.clear(),
        (Modal::Rename(f), Msg::RenameSave) => {
            shared.rename(&f.id, &f.draft);
            shared.set_refresh(&f.id, f.refresh);
            app.modal = Modal::None;
        }
        (Modal::Rename(f), Msg::Archive) => {
            shared.set_archived(&f.id, true);
            app.modal = Modal::None;
        }
        (_, Msg::Restore(id)) => {
            shared.set_archived(&id, false);
            app.snap = shared.snapshot();
            if app.snap.data.archived().next().is_none() {
                app.modal = Modal::None;
            }
            // It shows behind the list, where it was.
            return Task::done(Message::SelectAccount(id));
        }
        (Modal::Archived(f), Msg::RemoveArchived(id)) => {
            if f.confirm_remove.as_deref() == Some(id.as_str()) {
                shared.remove_account(&id);
                app.snap = shared.snapshot();
                f.confirm_remove = None;
                if app.snap.data.archived().next().is_none() {
                    app.modal = Modal::None;
                }
            } else {
                f.confirm_remove = Some(id);
            }
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
                    // New thresholds re-arm alerts that already fired.
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
        (_, Msg::Clock(c)) => shared.update(|d| d.settings.clock = c),

        (Modal::Settings(f), msg @ (Msg::AllMain | Msg::AllOthers | Msg::AllPercent(_) | Msg::AllReminder | Msg::AllLead(_) | Msg::AllLeadUnit(_) | Msg::AllRenewalDay)) => {
            f.applied = false;
            let mut alerts = shared.settings().alerts;
            match msg {
                Msg::AllMain => alerts.main_quota = !alerts.main_quota,
                Msg::AllOthers => alerts.other_bundles = !alerts.other_bundles,
                Msg::AllPercent(text) => {
                    let text: String = text.chars().filter(|c| c.is_ascii_digit() || *c == '.').take(5).collect();
                    if let Ok(n) = text.parse::<f64>() {
                        alerts.percent = Some(n.clamp(0.0, 100.0));
                    }
                    f.percent = text;
                }
                Msg::AllReminder => alerts.renewal.enabled = !alerts.renewal.enabled,
                Msg::AllLead(text) => {
                    let text: String = text.chars().filter(char::is_ascii_digit).take(3).collect();
                    if let Ok(n) = text.parse::<u32>() {
                        alerts.renewal.value = n.max(1);
                    }
                    f.lead = text;
                }
                Msg::AllLeadUnit(u) => alerts.renewal.unit = u,
                Msg::AllRenewalDay => alerts.renewal_day = !alerts.renewal_day,
                _ => {}
            }
            shared.update(|d| d.settings.alerts = alerts);
        }
        (Modal::Settings(f), Msg::AllApply) => {
            shared.update(|d| {
                // Every number gets what the field shows.
                d.settings.alerts.percent = Some(d.settings.alerts.percent_or_default());
                d.apply_alert_defaults();
            });
            f.applied = true;
        }
        _ => {}
    }
    Task::none()
}

pub fn view<'a>(ctx: &Ctx<'a>) -> Option<El<'a>> {
    match &ctx.app.modal {
        Modal::None => None,
        Modal::Add(w) => Some(super::add::view(ctx, w)),
        Modal::Rename(f) => ctx.app.snap.data.account(&f.id).map(|a| rename(ctx, f, a)),
        Modal::Alerts(f) => ctx.app.snap.data.account(&f.id).map(|a| alerts(ctx, f, a)),
        Modal::Settings(f) => Some(settings(ctx, f)),
        Modal::Archived(f) => Some(archived(ctx, f)),
        Modal::Export(f) => Some(super::transfer::export_view(ctx, f)),
        Modal::Import(f) => Some(super::transfer::import_view(ctx, f)),
        Modal::Group(f) => Some(super::home::group_view(ctx, f)),
        Modal::GroupLines(f) => Some(super::home::lines_view(ctx, f)),
    }
}

// ---------------------------------------------------------------------------------------
// Frame

pub(super) fn dialog<'a>(ctx: &Ctx<'a>, width: f32, kicker: String, title: String, close: Option<Message>, body: El<'a>, foot: El<'a>) -> El<'a> {
    let p = ctx.p;
    let close = close.map(|close| {
        Element::from(button(container(ctx.icon(Icon::Close, 16.0, p.tx)).center(Length::Fill)).width(32).height(32).padding(0).style(theme::ghost(p.tx, p.sf, 9.0)).on_press(close))
    });
    let head = ctx
        .row(
            std::iter::once(container(column![ctx.kicker(&kicker), ctx.bold(title, 21.0)].spacing(2).align_x(ctx.start())).width(Length::Fill).align_x(ctx.start()).into())
                .chain(close)
                .collect(),
        )
        .align_y(Alignment::Start)
        .spacing(16);
    // The body scrolls so the footer stays visible.
    let room = (ctx.app.main_size.height - 48.0 - 40.0 - 150.0).max(160.0);
    let body = container(super::smooth::smooth(scrollable(container(body).padding([18, 22])).id(BODY).style(theme::scroller(p)).height(Length::Shrink))).max_height(room);
    container(column![container(head).padding(super::pad4(18.0, 22.0, 14.0, 22.0)), hline(p.dv, 2.0), body, hline(p.dv, 2.0), container(foot).padding([14, 22])])
        .width(width)
        .max_height(720)
        .style(move |_| theme::panel(p, 16.0))
        .clip(true)
        .into()
}

/// Scrolls a dialog back to its top. iced keeps a scroll position for whatever dialog takes
/// the same place, so one that opens or moves to its next step starts where the last one was.
pub fn top() -> Task<Message> {
    iced::widget::operation::snap_to(BODY, iced::widget::operation::RelativeOffset::START)
}

pub(super) fn label<'a>(ctx: &Ctx<'a>, text: &str) -> El<'a> {
    container(ctx.muted(text.to_string(), 12.0)).width(Length::Fill).align_x(ctx.start()).into()
}

pub(super) fn notice<'a>(ctx: &Ctx<'a>, e: &FetchError, carrier: Carrier) -> El<'a> {
    let p = ctx.p;
    let text = column![ctx.bold(i18n::error_title(e, ctx.lang), 14.0), ctx.body(i18n::error_text(e, carrier, ctx.clock, ctx.lang), 13.0)].spacing(2).align_x(ctx.start()).width(Length::Fill);
    container(ctx.row(vec![ctx.icon(Icon::Warning, 18.0, p.danger), text.into()]).spacing(12).align_y(Alignment::Start))
        .padding([12, 14])
        .style(move |_| theme::fill(theme::fade(p.danger, 0.12), 10.0))
        .into()
}

/// A read-only box shaped like a field (owner name).
pub(super) fn readout<'a>(ctx: &Ctx<'a>, text: String, highlight: bool, check: bool) -> El<'a> {
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
// Rename

fn rename<'a>(ctx: &Ctx<'a>, f: &'a RenameForm, account: &'a klue_core::store::Account) -> El<'a> {
    let p = ctx.p;
    let s = ctx.s;
    let owner = account.owner_name.clone().unwrap_or_else(|| s.owner_unknown.into());
    let fallback = account.owner_name.clone().unwrap_or_else(|| account.pretty_number());
    let shown = if f.draft.trim().is_empty() { fallback } else { f.draft.clone() };
    let icon = super::line_icon(account.line);
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
    let global = ctx.app.snap.data.settings.refresh;
    let every = ctx.segmented(
        std::iter::once((None, format!("{} ({})", s.follow_settings, i18n::every_label(global, ctx.lang))))
            .chain(RefreshEvery::ALL.into_iter().map(|r| (Some(r), i18n::every_label(r, ctx.lang).to_string())))
            .map(|(r, label)| (label, f.refresh == r, Some(Message::Modal(Msg::RenameRefresh(r)))))
            .collect(),
        34.0,
        12.5,
    );
    let export = ctx
        .row(vec![
            container(ctx.muted(s.export_hint, 12.5)).width(Length::Fill).align_x(ctx.start()).into(),
            button(ctx.text(s.export_number, 13.0, Weight::ExtraBold, p.tx))
                .padding([4, 6])
                .style(theme::ghost(p.tx, p.sf, 9.0))
                .on_press(Message::OpenModal(ModalKind::Export(vec![f.id.clone()])))
                .into(),
        ])
        .spacing(10);
    let archive = ctx
        .row(vec![
            container(ctx.muted(s.archive_hint, 12.5)).width(Length::Fill).align_x(ctx.start()).into(),
            button(ctx.text(s.archive, 13.0, Weight::ExtraBold, p.tx)).padding([4, 6]).style(theme::ghost(p.tx, p.sf, 9.0)).on_press(Message::Modal(Msg::Archive)).into(),
        ])
        .spacing(10);
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
        column![label(ctx, s.check_number_every), gap(6.0), container(every).width(Length::Shrink), gap(6.0), ctx.muted(s.check_number_hint, 12.0)].align_x(ctx.start()),
        column![hline(p.dvs, 1.0), gap(12.0), export, gap(12.0), archive, gap(12.0), danger],
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
    dialog(ctx, 520.0, kicker, s.rename_title.to_string(), Some(Message::CloseModal), body.into(), foot.into())
}

// ---------------------------------------------------------------------------------------
// Alerts

/// A small number field, faded while its setting is off.
fn number_field<'a>(ctx: &Ctx<'a>, value: &str, on_input: impl Fn(String) -> Message + 'a, width: f32, dim: bool) -> El<'a> {
    let p = ctx.p;
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
}

fn alerts<'a>(ctx: &Ctx<'a>, f: &'a AlertsForm, account: &'a klue_core::store::Account) -> El<'a> {
    let p = ctx.p;
    let s = ctx.s;
    let ar = ctx.rtl();
    let Some(usage) = &account.usage else { return Space::new().into() };
    let small = |value: &str, on_input: Box<dyn Fn(String) -> Message + 'a>, width: f32, dim: bool| number_field(ctx, value, on_input, width, dim);

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
                container(column![ctx.semi(bundle.name_in(ctx.lang).to_string(), 13.5), ctx.muted(sub, 11.5)].spacing(0).align_x(ctx.start())).width(Length::Fill).align_x(ctx.start()).into(),
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

    let package = usage.package_in(ctx.lang).unwrap_or_default().to_string();
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
    dialog(ctx, 560.0, kicker, account.display_name(), Some(Message::CloseModal), body.into(), foot.into())
}

// ---------------------------------------------------------------------------------------
// Settings

fn settings<'a>(ctx: &Ctx<'a>, f: &'a SettingsForm) -> El<'a> {
    let p = ctx.p;
    let s = ctx.s;
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
        RefreshEvery::ALL.into_iter().map(|r| (i18n::every_label(r, ctx.lang).to_string(), settings.refresh == r, Some(Message::Modal(Msg::RefreshEvery(r))))).collect(),
        36.0,
        13.0,
    );
    // Each format shown with the time now.
    let clocks = ctx.segmented(
        [Clock::H24, Clock::H12]
            .into_iter()
            .map(|c| (i18n::clock(ctx.now, c, ctx.lang), settings.clock == c, Some(Message::Modal(Msg::Clock(c)))))
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
            column![label(ctx, s.time_format), gap(6.0), clocks].width(Length::FillPortion(1)).into(),
        ])
        .spacing(12)
        .align_y(Alignment::Start),
        column![label(ctx, s.check_every), gap(6.0), ctx.row(vec![every, ctx.muted(s.check_every_hint, 12.0).into()]).spacing(12)].align_x(ctx.start()),
        column![
            hline(p.dv, 2.0),
            sys(s.startup, settings.launch_at_startup, Msg::Startup),
            hline(p.dvs, 1.0),
            sys(s.tray, settings.keep_in_tray, Msg::KeepInTray),
            hline(p.dvs, 1.0)
        ],
        all_alerts(ctx, f),
        super::transfer::settings_section(ctx),
        about(ctx),
    ]
    .spacing(16);
    let foot = ctx.row(vec![container(ctx.muted(s.disclaimer, 11.5)).width(Length::Fill).align_x(ctx.start()).into(), ctx.primary(s.done, Some(Message::CloseModal)).into()]).spacing(12);
    dialog(ctx, 540.0, s.app.to_string(), s.settings_title.to_string(), Some(Message::CloseModal), body.into(), foot.into())
}

/// Alerts for all numbers. Saved as they change; Apply gives them to every number.
fn all_alerts<'a>(ctx: &Ctx<'a>, f: &'a SettingsForm) -> El<'a> {
    let p = ctx.p;
    let s = ctx.s;
    let a = ctx.app.snap.data.settings.alerts;
    let line = |items: Vec<El<'a>>| -> El<'a> { container(ctx.row(items).spacing(10).align_y(Alignment::Center)).padding([10, 0]).into() };
    let percent = number_field(ctx, &f.percent, |t| Message::Modal(Msg::AllPercent(t)), 52.0, !(a.main_quota || a.other_bundles));
    let lead = number_field(ctx, &f.lead, |t| Message::Modal(Msg::AllLead(t)), 52.0, !a.renewal.enabled);
    let units = ctx.segmented(
        [(LeadUnit::Days, s.days), (LeadUnit::Hours, s.hours)]
            .into_iter()
            .map(|(u, label)| (label.to_string(), a.renewal.unit == u, Some(Message::Modal(Msg::AllLeadUnit(u)))))
            .collect(),
        32.0,
        12.5,
    );
    let apply = ctx.row(vec![
        container(ctx.muted(if f.applied { s.applied_all } else { s.all_alerts_hint }, 12.0)).width(Length::Fill).align_x(ctx.start()).into(),
        ctx.outline(s.apply_all, (!ctx.app.snap.data.accounts.is_empty()).then_some(Message::Modal(Msg::AllApply))).into(),
    ])
    .spacing(12)
    .align_y(Alignment::Center);
    column![
        ctx.caps(s.all_alerts),
        gap(6.0),
        hline(p.dv, 2.0),
        line(vec![ctx.toggle(a.main_quota, Message::Modal(Msg::AllMain)), ctx.semi(s.all_low_quota, 13.5).into(), percent, ctx.muted(s.percent_left, 13.5).into()]),
        hline(p.dvs, 1.0),
        line(vec![ctx.toggle(a.other_bundles, Message::Modal(Msg::AllOthers)), ctx.body(s.all_other_bundles, 13.5).into()]),
        hline(p.dvs, 1.0),
        line(vec![ctx.toggle(a.renewal.enabled, Message::Modal(Msg::AllReminder)), ctx.semi(s.remind_me, 13.5).into(), lead, units, ctx.muted(s.before_renewal, 13.5).into()]),
        hline(p.dvs, 1.0),
        line(vec![ctx.toggle(a.renewal_day, Message::Modal(Msg::AllRenewalDay)), ctx.body(s.renew_day, 13.5).into()]),
        hline(p.dvs, 1.0),
        gap(10.0),
        apply,
    ]
    .align_x(ctx.start())
    .into()
}

/// Version, author, links and the update check.
fn about<'a>(ctx: &Ctx<'a>) -> El<'a> {
    let p = ctx.p;
    let s = ctx.s;
    let link = |label: &'a str, url: &'static str| -> El<'a> {
        button(ctx.row(vec![ctx.text(label, 13.0, Weight::Semibold, p.ac_tx).into(), ctx.icon(Icon::Link, 13.0, p.ac_tx)]).spacing(6).align_y(Alignment::Center))
            .padding([6, 10])
            .style(theme::ghost(p.ac_tx, p.sf, 8.0))
            .on_press(Message::OpenUrl(url))
            .into()
    };
    let (status, action): (String, El<'a>) = match &ctx.app.update {
        app::UpdateState::Checking => (s.checking_updates.into(), ctx.outline(s.check_updates, None).into()),
        app::UpdateState::UpToDate => (s.up_to_date.into(), ctx.outline(s.check_updates, Some(Message::CheckUpdates { manual: true })).into()),
        app::UpdateState::CheckFailed => (s.update_check_failed.into(), ctx.outline(s.check_updates, Some(Message::CheckUpdates { manual: true })).into()),
        update @ (app::UpdateState::Available { .. } | app::UpdateState::Failed { .. } | app::UpdateState::Opened { .. }) => (
            update.release().map(|r| i18n::update_available(&r.version.to_string(), ctx.lang)).unwrap_or_default(),
            ctx.primary(s.view_update, Some(Message::ShowUpdate)).into(),
        ),
        app::UpdateState::Downloading { .. } | app::UpdateState::Installing { .. } => (s.update_downloading.into(), ctx.outline(s.check_updates, None).into()),
        app::UpdateState::Idle => (String::new(), ctx.outline(s.check_updates, Some(Message::CheckUpdates { manual: true })).into()),
    };
    let who = column![
        ctx.row(vec![super::logo(18.0), ctx.bold("Klue", 16.0).into(), ctx.muted(i18n::version_label(crate::about::VERSION, ctx.lang), 12.5).into()]).spacing(8).align_y(Alignment::Center),
        ctx.row(vec![ctx.muted(s.made_by, 13.0).into(), ctx.semi(crate::about::AUTHOR, 13.0).into()]).spacing(5),
    ]
    .spacing(4)
    .align_x(ctx.start());
    column![
        ctx.caps(s.about),
        gap(6.0),
        hline(p.dv, 2.0),
        gap(10.0),
        ctx.row(vec![container(who).width(Length::Fill).align_x(ctx.start()).into(), link(s.repo_link, crate::about::REPO), link(s.website_link, crate::about::WEBSITE)]).spacing(6).align_y(Alignment::Center),
        gap(10.0),
        ctx.row(vec![container(ctx.muted(status, 12.5)).width(Length::Fill).align_x(ctx.start()).into(), action]).spacing(12).align_y(Alignment::Center),
    ]
    .align_x(ctx.start())
    .into()
}

// ---------------------------------------------------------------------------------------
// Archived numbers

/// The numbers put away with Archive, each with Restore and Remove.
fn archived<'a>(ctx: &Ctx<'a>, f: &'a ArchivedList) -> El<'a> {
    let p = ctx.p;
    let s = ctx.s;
    let rows: Vec<El<'a>> = ctx
        .app
        .snap
        .data
        .archived()
        .map(|a| {
            let t = crate::assets::tile(a.carrier);
            let what = format!("{} · {}", i18n::carrier_name(a.carrier, ctx.lang), i18n::line_name(a.line, ctx.lang));
            let who = column![
                ctx.semi(a.display_name(), 14.0),
                ctx.num(a.pretty_number(), 12.0, Weight::Normal, p.mu),
                ctx.muted(format!("{what} · {}", i18n::synced(a.updated_at(), ctx.now, ctx.lang)), 11.5),
            ]
            .spacing(1)
            .align_x(ctx.start());
            // A bin beside Restore; the first click turns it red and the second removes.
            let confirming = f.confirm_remove.as_deref() == Some(a.id.as_str());
            let bin = button(container(ctx.icon(Icon::Trash, 16.0, if confirming { Color::WHITE } else { p.danger })).center(Length::Fill))
                .width(38)
                .height(38)
                .padding(0)
                .style(move |_, status| {
                    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
                    let fill = if confirming { p.danger } else if hovered { theme::fade(p.danger, 0.12) } else { Color::TRANSPARENT };
                    button::Style {
                        background: Some(iced::Background::Color(fill)),
                        border: theme::border(if confirming || hovered { p.danger } else { p.dv }, 2.0, 9.0),
                        ..Default::default()
                    }
                })
                .on_press(Message::Modal(Msg::RemoveArchived(a.id.clone())));
            let actions = ctx
                .row(vec![ctx.outline(s.restore, Some(Message::Modal(Msg::Restore(a.id.clone())))).into(), ctx.tip(bin, if confirming { s.remove_confirm } else { s.remove })])
                .spacing(8);
            let row = ctx
                .row(vec![super::tile(a.carrier, 40.0, t.small * 1.35, 11.0, true), container(who).width(Length::Fill).align_x(ctx.start()).into(), actions.into()])
                .spacing(14);
            column![container(row).padding([12, 0]), hline(p.dvs, 1.0)].into()
        })
        .collect();
    let body = column![ctx.muted(s.archived_hint, 13.0), column![hline(p.dvs, 1.0), column(rows)]].spacing(10).align_x(ctx.start());
    let foot = ctx.row(vec![super::fill_x(), ctx.primary(s.done, Some(Message::CloseModal)).into()]);
    dialog(ctx, 520.0, s.app.to_string(), s.archived_title.to_string(), Some(Message::CloseModal), body.into(), foot.into())
}

// ---------------------------------------------------------------------------------------
// Disclaimer

/// Accepted once per version, before any sign-in.
pub fn disclaimer<'a>(ctx: &Ctx<'a>) -> El<'a> {
    let p = ctx.p;
    let s = ctx.s;
    let point = |icon: Icon, title: &'a str, text: &'a str| -> El<'a> {
        ctx.row(vec![
            container(ctx.icon(icon, 18.0, p.ac_tx)).padding(super::pad4(2.0, 0.0, 0.0, 0.0)).into(),
            column![ctx.bold(title, 14.5), ctx.body(text, 13.0).line_height(1.45)].spacing(3).align_x(ctx.start()).width(Length::Fill).into(),
        ])
        .spacing(12)
        .align_y(Alignment::Start)
        .into()
    };
    let body = column![
        point(Icon::Check, s.disc_passwords_title, s.disc_passwords),
        point(Icon::Info, s.disc_unofficial_title, s.disc_unofficial),
        point(Icon::Warning, s.disc_risk_title, s.disc_risk),
    ]
    .spacing(18);
    // Settings isn't reachable yet, so it has its own language switch.
    let lang = ctx.app.snap.data.settings.lang;
    let langs = ctx.segmented(
        vec![
            ("English".to_string(), lang == Lang::En, Some(Message::SetLang(Lang::En))),
            ("العربية".to_string(), lang == Lang::Ar, Some(Message::SetLang(Lang::Ar))),
        ],
        34.0,
        13.0,
    );
    let foot = ctx.row(vec![
        langs,
        super::fill_x(),
        ctx.outline(s.quit, Some(Message::Quit)).into(),
        ctx.primary(s.disc_accept, Some(Message::AcceptDisclaimer)).into(),
    ])
    .spacing(8)
    .align_y(Alignment::Center);
    // No close button: Quit or accept.
    dialog(ctx, 560.0, s.disc_kicker.to_string(), s.disc_title.to_string(), None, body.into(), foot.into())
}

// ---------------------------------------------------------------------------------------
// Update

/// What's new, and the install button.
pub fn update_dialog<'a>(ctx: &Ctx<'a>) -> Option<El<'a>> {
    let p = ctx.p;
    let s = ctx.s;
    let state = &ctx.app.update;
    let release = state.release()?;
    let version = release.version.to_string();
    let mut body = column![ctx.muted(i18n::you_have(crate::about::VERSION, ctx.lang), 13.0)].spacing(12).align_x(ctx.start());
    if !release.notes.trim().is_empty() {
        body = body.push(column![label(ctx, s.whats_new), gap(6.0), notes(ctx, &release.notes)].align_x(ctx.start()));
    }
    let foot_status: El<'a> = match state {
        app::UpdateState::Downloading { got, total, .. } => {
            let fraction = total.map_or(0.35, |t| *got as f64 / t.max(1) as f64);
            column![ctx.muted(format!("{} {}", s.update_downloading, i18n::downloaded(*got, *total, ctx.lang)), 12.5), ctx.meter(fraction, 4.0, p.ac)].spacing(6).width(Length::Fill).into()
        }
        app::UpdateState::Installing { .. } => ctx.muted(s.update_installing, 12.5).into(),
        app::UpdateState::Opened { .. } => ctx.muted(s.update_opened, 12.5).into(),
        app::UpdateState::Failed { message, .. } => column![ctx.text(s.update_failed, 12.5, Weight::Semibold, p.danger), ctx.muted(message.clone(), 11.5)].spacing(2).into(),
        _ => Space::new().into(),
    };
    let busy = matches!(state, app::UpdateState::Downloading { .. } | app::UpdateState::Installing { .. });
    // Disk images and packages go to the system installer; the rest installs itself.
    let installs_itself = release.asset(crate::updater::suffixes()).is_some_and(|a| a.name.ends_with(".zip") || a.name.ends_with(".tar.gz"));
    let mut foot = vec![container(foot_status).width(Length::Fill).align_x(ctx.start()).into()];
    if !busy {
        foot.push(ctx.link(s.update_skip, Message::UpdateSkip).into());
        foot.push(ctx.outline(s.update_later, Some(Message::UpdateLater)).into());
    }
    let page = release.page.clone();
    foot.push(match state {
        app::UpdateState::Opened { .. } => ctx.primary(s.done, Some(Message::UpdateLater)).into(),
        app::UpdateState::Failed { .. } if release.asset(crate::updater::suffixes()).is_none() || !page.is_empty() => ctx.primary(s.release_page, Some(Message::OpenUrlOwned(page))).into(),
        _ => ctx.primary(if installs_itself { s.update_install } else { s.update_download }, (!busy).then_some(Message::UpdateInstall)).into(),
    });
    let foot = ctx.row(foot).spacing(10).align_y(Alignment::Center);
    Some(dialog(ctx, 560.0, s.update_kicker.to_string(), i18n::update_available(&version, ctx.lang), Some(Message::UpdateLater), body.into(), foot.into()))
}

/// Minimal Markdown: bold headings, bulleted list items, plain text; up to 40 lines.
fn notes<'a>(ctx: &Ctx<'a>, markdown: &str) -> El<'a> {
    let clean = |t: &str| t.replace("**", "").replace("__", "").replace('`', "");
    let mut col = column![].spacing(4).align_x(ctx.start());
    // An HTML comment ends the part shown here (download tables and such are for GitHub).
    for line in markdown.lines().map(str::trim).take_while(|l| !l.starts_with("<!--")).filter(|l| !l.is_empty()).take(40) {
        col = col.push(if let Some(heading) = line.strip_prefix('#') {
            Element::from(ctx.bold(clean(heading.trim_start_matches('#').trim()), 14.0))
        } else if let Some(item) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
            ctx.row(vec![ctx.muted("•", 13.0).into(), ctx.body(clean(item), 13.0).into()]).spacing(8).align_y(Alignment::Start).into()
        } else {
            ctx.body(clean(line), 13.0).into()
        });
    }
    container(col).padding([12, 14]).width(Length::Fill).style(theme::bg_line(ctx.p.sf, ctx.p.dvs, 12.0)).into()
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
    dialog(ctx, 420.0, i18n::carrier_name(Carrier::We, ctx.lang).to_string(), s.captcha_title.to_string(), Some(Message::CaptchaCancel), body.into(), foot.into())
}
