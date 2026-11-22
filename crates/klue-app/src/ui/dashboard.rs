//! The number dashboard: quota hero, renewal column, balance and line details, the other
//! bundles (any of which can be made the main quota) and the payment history.

use iced::{
    Alignment, Length,
    font::Weight,
    widget::{Space, column, container, scrollable, stack},
};
use klue_core::{
    Bundle, Unit, Usage, format,
    model::PaymentKind,
    store::{Account, BundleAlert, Lang, LeadUnit, ThresholdMode},
};

use super::{Ctx, El, gap, hline, stat, vline};
use crate::{
    app::{Message, ModalKind},
    assets::Icon,
    i18n,
    theme,
};

pub fn view<'a>(ctx: &Ctx<'a>, account: &'a Account) -> El<'a> {
    let p = ctx.p;
    let s = ctx.s;
    let refreshing = ctx.app.snap.is_refreshing(&account.id);

    let mut page = column![header(ctx, account, refreshing), hline(p.dv, 2.0)].spacing(0);
    if let Some(error) = &account.last_error {
        page = page.push(gap(16.0)).push(notice(ctx, i18n::error_title(error, ctx.lang), i18n::error_text(error, account.carrier, ctx.lang)));
    }
    match &account.usage {
        Some(usage) => {
            page = page
                .push(overview(ctx, account, usage))
                .push(hline(p.dv, 2.0))
                .push(money_and_line(ctx, usage))
                .push(hline(p.dv, 2.0))
                .push(bundles(ctx, account, usage))
                .push(payments(ctx, usage));
        }
        None => {
            let mut waiting = column![ctx.muted(s.waiting_first_sync, 14.0)].spacing(10).max_width(360);
            if refreshing {
                waiting = waiting.push(ctx.meter(0.35, 3.0, p.ac));
            }
            page = page.push(container(waiting).padding([22, 0]));
        }
    }
    // One scroll position per number.
    scrollable(container(page).padding([22, 28]).width(Length::Fill))
        .id(format!("dash-{}", account.id))
        .style(theme::scroller(p))
        .height(Length::Fill)
        .into()
}

fn header<'a>(ctx: &Ctx<'a>, account: &'a Account, refreshing: bool) -> El<'a> {
    let p = ctx.p;
    let s = ctx.s;
    let kicker = format!("{} · {}", i18n::line_name(account.line, ctx.lang), i18n::carrier_name(account.carrier, ctx.lang));
    let owner = account.owner_name.clone().unwrap_or_else(|| s.owner_unknown.to_string());
    let reg = ctx
        .row(vec![
            ctx.muted(s.registered_to, 13.5).into(),
            ctx.text(owner, 13.5, Weight::Semibold, p.tx).into(),
            ctx.muted("·", 13.5).into(),
            ctx.num(account.pretty_number(), 13.5, Weight::Normal, p.mu).into(),
        ])
        .spacing(6);
    let who = column![ctx.kicker(&kicker), ctx.bold(account.display_name(), 40.0).line_height(1.15), reg].spacing(4).align_x(ctx.start());
    let id = account.id.clone();
    let tools = ctx
        .row(vec![
            ctx.tool(Icon::Refresh, s.refresh, (!refreshing).then(|| Message::Refresh(id.clone())), refreshing),
            ctx.tool(Icon::Pencil, s.rename, Some(Message::OpenModal(ModalKind::Rename(id.clone()))), false),
            ctx.tool(Icon::Bell, s.alerts, Some(Message::OpenModal(ModalKind::Alerts(id.clone()))), false),
        ])
        .spacing(8);
    container(ctx.row(vec![container(who).width(Length::Fill).align_x(ctx.start()).into(), tools.into()]).align_y(Alignment::End).spacing(24))
        .padding(super::pad4(0.0, 0.0, 18.0, 0.0))
        .into()
}

fn notice<'a>(ctx: &Ctx<'a>, title: &'a str, body: String) -> El<'a> {
    let p = ctx.p;
    let text = column![ctx.bold(title, 14.0), ctx.body(body, 13.0)].spacing(2).align_x(ctx.start()).width(Length::Fill);
    container(ctx.row(vec![ctx.icon(Icon::Warning, 18.0, p.danger), text.into()]).spacing(12).align_y(Alignment::Start))
        .padding([12, 14])
        .style(move |_| theme::fill(theme::fade(p.danger, 0.12), 10.0))
        .into()
}

/// "10 GB" or "20%" for a bundle alert.
fn threshold_text(alert: &BundleAlert, unit: Unit, lang: Lang) -> String {
    match alert.mode {
        ThresholdMode::Percent => format!("{}%", format::amount_short(alert.value, Unit::Gb)),
        ThresholdMode::Amount => format!("{} {}", format::amount_short(alert.value, unit), i18n::unit_name(unit, lang)),
    }
}

fn overview<'a>(ctx: &Ctx<'a>, account: &'a Account, usage: &'a Usage) -> El<'a> {
    let p = ctx.p;
    let s = ctx.s;
    let ar = ctx.rtl();
    let main = usage.main_for(account.main_bundle.as_deref());
    let picked = account.main_bundle.is_some() && main.is_some_and(|m| Some(&m.key) == account.main_bundle.as_ref());

    // Quota hero.
    let package = usage.package_name.clone().or_else(|| main.map(|m| m.name.clone())).unwrap_or_default();
    let mut quota = column![].spacing(0);
    let mut quota_foot: El<'a> = Space::new().into();
    let mut top = vec![ctx.caps(s.package).into(), super::fill_x()];
    if account.main_alert_triggered() {
        top.push(container(ctx.text(s.below_alert, 11.0, Weight::Semibold, p.on)).padding([3, 10]).style(move |_| theme::fill(p.danger, 10.0)).into());
    }
    quota = quota.push(ctx.row(top));
    quota = quota.push(ctx.bold(package.clone(), 21.0));
    if let Some(m) = main {
        if m.name != package || picked {
            let mut line = vec![ctx.muted(m.name.clone(), 13.0).into()];
            if picked {
                line.push(ctx.muted("·", 13.0).into());
                line.push(ctx.text(s.main_picked, 13.0, Weight::Semibold, p.ac_tx).into());
                line.push(ctx.link(s.main_default, Message::SetMain(account.id.clone(), None)).into());
            }
            quota = quota.push(ctx.row(line).spacing(6));
        }
        let alert = account.alert_for(m, true);
        let unit = i18n::unit_name(m.unit, ctx.lang);
        let pct = (m.remaining_pct() * 10.0).round() / 10.0;
        let low = alert.is_triggered(m);
        let total = format::amount_short(m.total, m.unit);
        let used = format::amount(m.used(), m.unit);
        let mark = threshold_text(&alert, m.unit, ctx.lang);

        let big = ctx.num(format::amount(m.remaining, m.unit), 88.0, Weight::ExtraBold, if low { p.danger } else { p.tx }).line_height(0.9);
        let label = column![
            ctx.bold(i18n::left_label(m.unit, ctx.lang), 16.0),
            ctx.muted(if ar { format!("من {total} {unit}") } else { format!("of {total} {unit}") }, 13.0)
        ]
        .spacing(0)
        .align_x(ctx.start());
        let left = ctx.row(vec![big.into(), label.into()]).spacing(12).align_y(Alignment::End);
        let right = column![ctx.num(format!("{pct:.1}%"), 34.0, Weight::ExtraBold, p.tx), ctx.muted(s.remaining, 12.0)].align_x(ctx.end()).spacing(4);
        quota = quota.push(gap(18.0)).push(ctx.row(vec![left.into(), super::fill_x(), right.into()]).align_y(Alignment::End));

        // Meter with the alert threshold marked.
        let meter = ctx.meter(m.remaining_pct() / 100.0, 16.0, if low { p.danger } else { p.ac });
        let meter: El<'a> = if alert.enabled && m.total > 0.0 {
            let at = (alert.threshold(m) / m.total).clamp(0.0, 1.0);
            let before = ((at * 1000.0).round() as u16).max(1);
            let mut parts: Vec<El<'a>> = vec![
                Space::new().width(Length::FillPortion(before)).into(),
                container(Space::new()).width(3).height(28).style(move |_| theme::fill(p.tx, 2.0)).into(),
                Space::new().width(Length::FillPortion((1000 - before).max(1))).into(),
            ];
            if ar {
                parts.reverse();
            }
            stack([container(meter).height(28).align_y(Alignment::Center).into(), iced::widget::Row::with_children(parts).height(28).into()]).into()
        } else {
            container(meter).height(28).align_y(Alignment::Center).into()
        };
        quota = quota.push(gap(14.0)).push(meter);
        let used_text = if ar { format!("تم استخدام {used} {unit}") } else { format!("{used} {unit} used") };
        let mark_text = if !alert.enabled { String::new() } else if ar { format!("حد التنبيه {mark}") } else { format!("Alert at {mark}") };
        quota = quota.push(gap(6.0)).push(ctx.row(vec![ctx.muted(used_text, 12.0).into(), super::fill_x(), ctx.muted(mark_text, 12.0).into()]));

        let alert_text = if !alert.enabled {
            (if ar { "لا يوجد تنبيه للرصيد" } else { "No quota alert set" }).to_string()
        } else if ar {
            format!("نبّهني عندما يتبقى {mark}")
        } else {
            format!("Notify me when {mark} remain")
        };
        quota_foot = column![
            hline(p.dvs, 1.0),
            foot(ctx, ctx.icon(Icon::Bell, 16.0, p.tx), alert_text, Message::OpenModal(ModalKind::Alerts(account.id.clone()))),
        ]
        .into();
    }

    // Renewal column.
    let start = usage.cycle_start();
    let renews = usage.renews_at;
    let renew_long = renews.map(|r| i18n::date_long(r, ctx.lang)).unwrap_or_else(|| "—".into());
    let countdown = renews.map(|r| i18n::countdown(r, ctx.now, ctx.lang)).unwrap_or_default();
    let (cycle_text, cycle) = match (start, renews) {
        (Some(st), Some(r)) if r > st => {
            let day = ((ctx.now - st).num_seconds() as f64 / 86_400.0).round().max(0.0) as i64;
            let days = ((r - st).num_seconds() as f64 / 86_400.0).round() as i64;
            let frac = ((ctx.now - st).num_seconds() as f64 / (r - st).num_seconds() as f64).clamp(0.0, 1.0);
            (if ar { format!("اليوم {day} من {days}") } else { format!("Day {day} of {days}") }, frac)
        }
        _ => (String::new(), 0.0),
    };
    let reminder = account.alerts.renewal;
    let lead = i18n::count(reminder.value as i64, if reminder.unit == LeadUnit::Days { i18n::Span::Day } else { i18n::Span::Hour }, ctx.lang);
    let remind_text = match (reminder.enabled, ar) {
        (true, false) => format!("Remind me {lead} before"),
        (true, true) => format!("ذكّرني قبل التجديد بـ {lead}"),
        (false, false) => "Renewal reminder off".into(),
        (false, true) => "تذكير التجديد غير مفعّل".into(),
    };
    let id = account.id.clone();
    let renew = column![
        ctx.caps(s.next_renewal),
        ctx.bold(renew_long, 30.0),
        ctx.text(countdown, 14.0, Weight::Semibold, p.ac_tx),
        gap(14.0),
        ctx.row(vec![ctx.muted(s.cycle, 12.0).into(), super::fill_x(), ctx.muted(cycle_text, 12.0).into()]),
        super::bar(cycle, 6.0, p.tx, p.sf, None, ar),
        gap(4.0),
        ctx.row(vec![
            stat(ctx, s.subscribed, start.map(|d| i18n::date_short(d, ctx.lang)).unwrap_or_else(|| "—".into()), ctx.start()),
            super::fill_x(),
            stat(ctx, s.renews, renews.map(|d| i18n::date_short(d, ctx.lang)).unwrap_or_else(|| "—".into()), ctx.end()),
        ]),
    ]
    .spacing(6)
    .align_x(ctx.start());
    let renew_foot = column![
        hline(p.dvs, 1.0),
        foot(ctx, ctx.toggle(reminder.enabled, Message::Modal(super::modals::Msg::ToggleReminder(id.clone()))), remind_text, Message::OpenModal(ModalKind::Alerts(id))),
    ];

    // Bodies and footers go in separate rows so the footers line up. (A Fill-height
    // spacer can't push them down: iced sizes a row's children in order, so in Arabic,
    // where the renewal column comes first, it would get no height at all.)
    let body = ctx
        .row(vec![
            container(quota.align_x(ctx.start())).padding(ctx.pad(22.0, 28.0, 16.0, 0.0)).width(Length::FillPortion(165)).into(),
            vline(p.dv, 2.0),
            container(renew).padding(ctx.pad(22.0, 0.0, 16.0, 28.0)).width(Length::FillPortion(100)).into(),
        ])
        .align_y(Alignment::Start);
    let feet = ctx
        .row(vec![
            container(quota_foot).padding(ctx.pad(0.0, 28.0, 22.0, 0.0)).width(Length::FillPortion(165)).into(),
            vline(p.dv, 2.0),
            container(renew_foot).padding(ctx.pad(0.0, 0.0, 22.0, 28.0)).width(Length::FillPortion(100)).into(),
        ])
        .align_y(Alignment::Start);
    column![body, feet].into()
}

/// The row under a card: an icon (or switch), a sentence and a "Change" link.
fn foot<'a>(ctx: &Ctx<'a>, lead: El<'a>, text: String, change: Message) -> El<'a> {
    container(
        ctx.row(vec![lead, container(ctx.body(text, 13.5)).width(Length::Fill).align_x(ctx.start()).into(), ctx.link(ctx.s.change, change).into()])
            .spacing(10),
    )
    .padding(super::pad4(10.0, 0.0, 0.0, 0.0))
    .into()
}

/// Balance and what the carrier says about the line.
fn money_and_line<'a>(ctx: &Ctx<'a>, usage: &'a Usage) -> El<'a> {
    let p = ctx.p;
    let s = ctx.s;
    let figure: El<'a> = match usage.balance_egp {
        Some(v) => ctx
            .row(vec![ctx.num(format::money(v), 34.0, Weight::ExtraBold, p.tx).into(), ctx.text(s.egp, 15.0, Weight::ExtraBold, p.mu).into()])
            .spacing(8)
            .align_y(Alignment::End)
            .into(),
        None => ctx.num("—", 34.0, Weight::ExtraBold, p.mu).into(),
    };
    let balance = column![ctx.row(vec![ctx.icon(Icon::Wallet, 14.0, p.mu), ctx.caps(s.balance).into()]).spacing(6), figure].spacing(8).align_x(ctx.start());

    let info = &usage.info;
    let mut pairs: Vec<(&str, String)> = Vec::new();
    if let Some(plan) = &info.plan {
        pairs.push((s.plan, plan.clone()));
    }
    if let Some(kind) = info.payment {
        pairs.push((s.payment_type, (if kind == PaymentKind::Prepaid { s.prepaid } else { s.postpaid }).to_string()));
    }
    if let Some(status) = &info.status {
        pairs.push((s.status, status.clone()));
    }
    if let Some(since) = info.since {
        pairs.push((s.since, i18n::date_short(since, ctx.lang)));
    }
    let mut details = column![ctx.caps(s.line_info)].spacing(8).align_x(ctx.start());
    if pairs.is_empty() {
        details = details.push(ctx.muted("—", 13.5));
    }
    for chunk in pairs.chunks(2) {
        let cells: Vec<El<'a>> = chunk
            .iter()
            .map(|(k, v)| {
                container(column![ctx.muted(k.to_string(), 12.0), ctx.semi(v.clone(), 13.5)].spacing(1).align_x(ctx.start()))
                    .width(Length::FillPortion(1))
                    .align_x(ctx.start())
                    .into()
            })
            .chain((chunk.len() < 2).then(|| Space::new().width(Length::FillPortion(1)).into()))
            .collect();
        details = details.push(ctx.row(cells).spacing(16));
    }

    let left = container(balance).padding(ctx.pad(20.0, 28.0, 20.0, 0.0)).width(Length::FillPortion(100));
    let right = container(details).padding(ctx.pad(20.0, 0.0, 20.0, 28.0)).width(Length::FillPortion(165));
    ctx.row(vec![left.into(), vline(p.dv, 2.0), right.into()]).align_y(Alignment::Start).into()
}

fn bundles<'a>(ctx: &Ctx<'a>, account: &'a Account, usage: &'a Usage) -> El<'a> {
    let p = ctx.p;
    let s = ctx.s;
    let ordered = usage.ordered(account.main_bundle.as_deref());
    let rest: Vec<&Bundle> = ordered.into_iter().skip(1).collect();
    if rest.is_empty() {
        return Space::new().into();
    }
    let cols = [Length::FillPortion(20), Length::FillPortion(10), Length::FillPortion(10), Length::FillPortion(15), Length::FillPortion(10)];
    let head = ctx
        .row(
            [s.col_bundle, s.col_rem, s.col_total, s.col_left, s.col_alert]
                .iter()
                .zip(cols)
                .map(|(label, w)| container(ctx.caps(label)).width(w).align_x(ctx.start()).into())
                .chain(std::iter::once(Space::new().width(36).into()))
                .collect(),
        )
        .spacing(16);
    let mut table = column![ctx.bold(s.bundles, 16.0), gap(4.0), container(head).padding([8, 0]), hline(p.dv, 2.0)].align_x(ctx.start());
    for bundle in rest {
        let alert = account.alert_for(bundle, false);
        let unit = i18n::unit_name(bundle.unit, ctx.lang);
        let low = alert.is_triggered(bundle);
        let pct = (bundle.remaining_pct() * 10.0).round() / 10.0;
        let alert_text = if alert.enabled { format!("≤ {}", threshold_text(&alert, bundle.unit, ctx.lang)) } else { s.off.to_string() };
        let name = column![ctx.semi(bundle.name.clone(), 13.5), ctx.muted(i18n::kind_name(bundle.kind, ctx.lang), 11.5)].spacing(0).align_x(ctx.start());
        let left = ctx
            .row(vec![
                container(super::bar(bundle.remaining_pct() / 100.0, 6.0, if low { p.danger } else { p.ac }, p.sf, None, ctx.rtl())).width(Length::Fill).into(),
                container(ctx.num(format!("{pct:.1}%"), 12.5, Weight::Normal, p.tx)).width(46).align_x(ctx.end()).into(),
            ])
            .spacing(10);
        let make_main = ctx.tip(
            iced::widget::button(container(ctx.icon(Icon::Star, 15.0, p.mu)).center(Length::Fill))
                .width(30)
                .height(30)
                .padding(0)
                .style(theme::ghost(p.mu, p.sf, 8.0))
                .on_press(Message::SetMain(account.id.clone(), Some(bundle.key.clone()))),
            s.make_main,
        );
        let cells: Vec<El<'a>> = vec![
            container(name).width(cols[0]).align_x(ctx.start()).into(),
            container(ctx.num(format!("{} {unit}", format::amount(bundle.remaining, bundle.unit)), 13.5, Weight::ExtraBold, if low { p.danger } else { p.tx }))
                .width(cols[1])
                .align_x(ctx.start())
                .into(),
            container(ctx.num(format!("{} {unit}", format::amount_short(bundle.total, bundle.unit)), 13.5, Weight::Normal, p.mu)).width(cols[2]).align_x(ctx.start()).into(),
            container(left).width(cols[3]).into(),
            container(ctx.text(alert_text, 12.5, Weight::Normal, if alert.enabled { p.tx } else { p.mu })).width(cols[4]).align_x(ctx.start()).into(),
            container(make_main).width(36).align_x(ctx.end()).into(),
        ];
        table = table.push(container(ctx.row(cells).spacing(16)).padding([9, 0])).push(hline(p.dvs, 1.0));
    }
    container(table).padding(super::pad4(22.0, 0.0, 0.0, 0.0)).into()
}

fn payments<'a>(ctx: &Ctx<'a>, usage: &'a Usage) -> El<'a> {
    let p = ctx.p;
    let s = ctx.s;
    let title = ctx.row(vec![ctx.icon(Icon::Receipt, 16.0, p.tx), ctx.bold(s.payments, 16.0).into()]).spacing(8);
    let mut section = column![title, gap(4.0)].align_x(ctx.start());
    match &usage.payments {
        None => section = section.push(container(ctx.muted(s.payments_unavailable, 13.5)).padding([6, 0])),
        Some(list) if list.is_empty() => section = section.push(container(ctx.muted(s.no_payments, 13.5)).padding([6, 0])),
        Some(list) => {
            let cols = [Length::FillPortion(14), Length::FillPortion(16), Length::FillPortion(10), Length::FillPortion(10)];
            let head = ctx
                .row(
                    [s.col_date, s.col_method, s.col_amount, s.col_balance_after]
                        .iter()
                        .zip(cols)
                        .map(|(label, w)| container(ctx.caps(label)).width(w).align_x(ctx.start()).into())
                        .collect(),
                )
                .spacing(16);
            section = section.push(container(head).padding([8, 0])).push(hline(p.dv, 2.0));
            for payment in list.iter().take(8) {
                let cells: Vec<El<'a>> = vec![
                    container(ctx.body(i18n::date_short(payment.at, ctx.lang), 13.5)).width(cols[0]).align_x(ctx.start()).into(),
                    container(ctx.body(payment.method.clone().unwrap_or_else(|| "—".into()), 13.5)).width(cols[1]).align_x(ctx.start()).into(),
                    container(ctx.num(format!("{} {}", format::money(payment.amount_egp), s.egp), 13.5, Weight::ExtraBold, p.tx)).width(cols[2]).align_x(ctx.start()).into(),
                    container(ctx.num(payment.balance_after.map(|b| format!("{} {}", format::money(b), s.egp)).unwrap_or_else(|| "—".into()), 13.5, Weight::Normal, p.mu))
                        .width(cols[3])
                        .align_x(ctx.start())
                        .into(),
                ];
                section = section.push(container(ctx.row(cells).spacing(16)).padding([9, 0])).push(hline(p.dvs, 1.0));
            }
        }
    }
    container(section).padding(super::pad4(26.0, 0.0, 6.0, 0.0)).into()
}
