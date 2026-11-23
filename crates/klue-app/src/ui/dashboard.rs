//! A number's dashboard: quota card, renewal, balance and line details, usage graph, other
//! bundles and payments.

use iced::{
    Alignment, Color, Length,
    font::Weight,
    widget::{Space, button, canvas, column, container, scrollable, stack},
};
use klue_core::{
    Bundle, BundleState, Unit, Usage, format,
    history::{self, Analysis},
    model::PaymentKind,
    store::{Account, BundleAlert, Lang, LeadUnit, ThresholdMode},
};

use super::{
    Ctx, El,
    chart::{self, Chart},
    gap, hline, stat, vline,
};
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
        page = page.push(gap(16.0)).push(notice(ctx, Icon::Warning, ctx.p.danger, i18n::error_title(error, ctx.lang), i18n::error_text(error, account.carrier, ctx.lang)));
    }
    if let Some(usage) = &account.usage
        && let Some(state) = state_notice(ctx, account, usage)
    {
        page = page.push(gap(16.0)).push(state);
    }
    match &account.usage {
        Some(usage) => {
            page = page
                .push(overview(ctx, account, usage))
                .push(hline(p.dv, 2.0))
                .push(money_and_line(ctx, usage))
                .push(hline(p.dv, 2.0))
                .push(usage_graph(ctx, account, usage))
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
    super::smooth::smooth(
        scrollable(container(page).padding([22, 28]).width(Length::Fill))
            .id(format!("dash-{}", account.id))
            .style(theme::scroller(p))
            .height(Length::Fill),
    )
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

fn notice<'a>(ctx: &Ctx<'a>, icon: Icon, color: Color, title: impl iced::widget::text::IntoFragment<'a>, body: String) -> El<'a> {
    let text = column![ctx.bold(title, 14.0), ctx.body(body, 13.0)].spacing(2).align_x(ctx.start()).width(Length::Fill);
    container(ctx.row(vec![ctx.icon(icon, 18.0, color), text.into()]).spacing(12).align_y(Alignment::Start))
        .padding([12, 14])
        .style(move |_| theme::fill(theme::fade(color, 0.12), 10.0))
        .into()
}

/// Notice for a borrowed or frozen bundle (frozen first).
fn state_notice<'a>(ctx: &Ctx<'a>, account: &'a Account, usage: &'a Usage) -> Option<El<'a>> {
    let bundle = usage.bundles.iter().filter(|b| b.state != BundleState::Active).max_by_key(|b| b.state == BundleState::Frozen)?;
    let is_main = usage.main_for(account.main_bundle.as_deref()).is_some_and(|m| m.key == bundle.key);
    let what = usage.package_in(ctx.lang).filter(|_| is_main).unwrap_or_else(|| bundle.name_in(ctx.lang)).to_string();
    let (title, body) = i18n::state_notice(bundle.state, account.carrier, &what, usage.owed_egp, bundle.settle_by, ctx.lang)?;
    Some(notice(ctx, Icon::Clock, state_color(ctx, bundle.state), title, body))
}

/// Frozen in the error colour, borrowed in the accent.
fn state_color(ctx: &Ctx<'_>, state: BundleState) -> Color {
    match state {
        BundleState::Frozen => ctx.p.danger,
        BundleState::Borrowed => ctx.p.ac_tx,
        BundleState::Active => ctx.p.tx,
    }
}

/// "Borrowed" / "Frozen" tag.
fn state_pill<'a>(ctx: &Ctx<'a>, state: BundleState) -> Option<El<'a>> {
    let label = i18n::state_name(state, ctx.lang)?;
    let color = state_color(ctx, state);
    Some(container(ctx.text(label, 11.0, Weight::Semibold, color)).padding([3, 10]).style(move |_| theme::fill(theme::fade(color, 0.14), 10.0)).into())
}

/// The quota card's bundle switcher: make-main star and prev/next arrows.
fn bundle_nav<'a>(ctx: &Ctx<'a>, account: &'a Account, shown: &'a Bundle, is_main: bool, index: usize, count: usize) -> El<'a> {
    let p = ctx.p;
    let s = ctx.s;
    let small = |icon: Icon, color: Color, msg: Option<Message>| {
        button(container(ctx.icon(icon, 15.0, color)).center(Length::Fill)).width(30).height(30).padding(0).style(theme::ghost(p.mu, p.sf, 8.0)).on_press_maybe(msg)
    };
    let star = if is_main {
        ctx.tip(small(Icon::StarFilled, p.ac, None), s.main_quota)
    } else {
        ctx.tip(small(Icon::Star, p.mu, Some(Message::SetMain(account.id.clone(), Some(shown.key.clone())))), s.make_main)
    };
    // Arrows follow the bundle order, mirrored in Arabic.
    let (back, ahead) = if ctx.rtl() { (Icon::ChevronRight, Icon::ChevronLeft) } else { (Icon::ChevronLeft, Icon::ChevronRight) };
    let id = account.id.clone();
    ctx.row(vec![
        star,
        ctx.tip(small(back, p.tx, Some(Message::HeroStep(id.clone(), false))), s.prev_bundle),
        ctx.num(format!("{}/{count}", index + 1), 12.0, Weight::Semibold, p.mu).into(),
        ctx.tip(small(ahead, p.tx, Some(Message::HeroStep(id, true))), s.next_bundle),
    ])
    .spacing(2)
    .align_y(Alignment::Center)
    .into()
}

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
    // The main bundle, or whichever the arrows moved to.
    let ordered = usage.ordered(account.main_bundle.as_deref());
    let index = ctx.app.hero.get(&account.id).and_then(|k| ordered.iter().position(|b| &b.key == k)).unwrap_or(0);
    let shown = ordered.get(index).copied();
    let is_main = index == 0;

    // Quota card.
    let package = usage.package_in(ctx.lang).or_else(|| main.map(|m| m.name_in(ctx.lang))).unwrap_or_default().to_string();
    let mut quota = column![].spacing(0);
    let mut quota_foot: El<'a> = Space::new().into();
    let mut top = vec![ctx.caps(if is_main { s.package } else { s.col_bundle }).into(), super::fill_x()];
    top.extend(shown.and_then(|m| state_pill(ctx, m.state)));
    if shown.is_some_and(|m| account.alert_for(m, is_main).is_triggered(m)) {
        top.push(container(ctx.text(s.below_alert, 11.0, Weight::Semibold, p.on)).padding([3, 10]).style(move |_| theme::fill(p.danger, 10.0)).into());
    }
    if let Some(m) = shown.filter(|_| ordered.len() > 1) {
        top.push(bundle_nav(ctx, account, m, is_main, index, ordered.len()));
    }
    quota = quota.push(ctx.row(top).spacing(6).align_y(Alignment::Center));
    let title = match shown {
        Some(m) if !is_main => m.name_in(ctx.lang).to_string(),
        _ => package.clone(),
    };
    quota = quota.push(ctx.bold(title, 21.0));
    if let Some(m) = shown {
        // Under the name: the bundle's own name and the speed, or another bundle's kind.
        let mut line: Vec<El<'a>> = Vec::new();
        let push = |line: &mut Vec<El<'a>>, el: El<'a>| {
            if !line.is_empty() {
                line.push(ctx.muted("·", 13.0).into());
            }
            line.push(el);
        };
        if !is_main {
            push(&mut line, ctx.muted(i18n::kind_name(m.kind, ctx.lang), 13.0).into());
        } else {
            if m.name_in(ctx.lang) != package {
                push(&mut line, ctx.muted(m.name_in(ctx.lang).to_string(), 13.0).into());
            }
            if let Some(speed) = &usage.info.speed {
                push(&mut line, ctx.muted(i18n::up_to(speed.down_kbps, ctx.lang), 13.0).into());
            }
            if picked {
                push(&mut line, ctx.text(s.main_picked, 13.0, Weight::Semibold, p.ac_tx).into());
                line.push(ctx.link(s.main_default, Message::SetMain(account.id.clone(), None)).into());
            }
        }
        if !line.is_empty() {
            quota = quota.push(ctx.row(line).spacing(6));
        }
        let alert = account.alert_for(m, is_main);
        let unit = i18n::unit_name(m.unit, ctx.lang);
        let pct = (m.remaining_pct() * 10.0).round() / 10.0;
        let low = alert.is_triggered(m);
        // Frozen flexes can't be used, so they're greyed out (like Vodafone's Flex page).
        let frozen = m.state == BundleState::Frozen;
        let total = format::amount_short(m.total, m.unit);
        let used = format::amount(m.used(), m.unit);
        let mark = threshold_text(&alert, m.unit, ctx.lang);

        let figure = if frozen { p.mu } else if low { p.danger } else { p.tx };
        let big = ctx.num(format::amount(m.remaining, m.unit), 88.0, Weight::ExtraBold, figure).line_height(0.9);
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
        let meter = ctx.meter(m.remaining_pct() / 100.0, 16.0, if frozen { p.dv } else if low { p.danger } else { p.ac });
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

    // Bodies and footers in separate rows so the footers line up. A Fill spacer won't do: in
    // Arabic the renewal column comes first and would get no height.
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

/// The row under a card: icon or switch, text, and a "Change" link.
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
    let mut balance = column![ctx.row(vec![ctx.icon(Icon::Wallet, 14.0, p.mu), ctx.caps(s.balance).into()]).spacing(6), figure].spacing(8).align_x(ctx.start());
    if let Some(owed) = usage.owed_egp.filter(|o| *o > 0.0) {
        let owed = column![ctx.text(i18n::owed(owed, ctx.lang), 13.5, Weight::Semibold, p.danger), ctx.muted(s.owed_note, 12.0)];
        balance = balance.push(owed.spacing(1).align_x(ctx.start()));
    }

    let info = &usage.info;
    let mut pairs: Vec<(&str, String)> = Vec::new();
    if let Some(plan) = info.plan_in(ctx.lang) {
        pairs.push((s.plan, plan.to_string()));
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
    if let Some(speed) = &info.speed {
        pairs.push((s.speed, i18n::speeds(speed, ctx.lang)));
        if let Some(throttled) = speed.throttled_kbps {
            pairs.push((s.after_quota, i18n::speed(throttled, ctx.lang)));
        }
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

/// Usage graph for the bundle on the quota card, with averages; just estimates while
/// there's too little history.
fn usage_graph<'a>(ctx: &Ctx<'a>, account: &'a Account, usage: &'a Usage) -> El<'a> {
    let p = ctx.p;
    let s = ctx.s;
    let ordered = usage.ordered(account.main_bundle.as_deref());
    let index = ctx.app.hero.get(&account.id).and_then(|k| ordered.iter().position(|b| &b.key == k)).unwrap_or(0);
    let Some(bundle) = ordered.get(index).copied().filter(|b| b.total > 0.0) else { return Space::new().into() };
    // The package quota follows the package's cycle.
    let (since, until) = if index == 0 {
        (bundle.starts_at.or(usage.cycle_start()), bundle.expires_at.or(usage.renews_at))
    } else {
        (bundle.starts_at, bundle.expires_at)
    };
    let a = history::analyse(account.history.samples(&bundle.key), bundle, since, until, ctx.now);
    let hours = !a.hourly.is_empty() && (ctx.app.usage_hours || a.daily.is_empty());

    let name = if index == 0 { usage.package_in(ctx.lang).unwrap_or_else(|| bundle.name_in(ctx.lang)) } else { bundle.name_in(ctx.lang) };
    let mut head: Vec<El<'a>> = vec![
        ctx.icon(Icon::Chart, 16.0, p.tx),
        ctx.bold(s.usage, 16.0).into(),
        ctx.muted("·", 13.0).into(),
        ctx.muted(name.to_string(), 13.0).into(),
        super::fill_x(),
    ];
    if a.enough() {
        let (days_ok, hours_ok) = (!a.daily.is_empty(), !a.hourly.is_empty());
        head.push(ctx.segmented_tips(
            vec![
                (s.usage_days.to_string(), !hours, days_ok.then_some(Message::UsageHours(false)), (!days_ok).then_some(s.usage_days_later)),
                (s.usage_hours.to_string(), hours, hours_ok.then_some(Message::UsageHours(true)), (!hours_ok).then_some(s.usage_hours_later)),
            ],
            30.0,
            12.5,
        ));
    }
    let mut section = column![ctx.row(head).spacing(8)].spacing(16).align_x(ctx.start());
    if !a.enough() {
        let mut body = s.not_enough_body.to_string();
        if let Some(e) = &a.estimate {
            body.push(' ');
            body.push_str(if e.from_samples { s.not_enough_samples } else { s.not_enough_cycle });
        }
        section = section.push(notice(ctx, Icon::Info, p.tx, s.not_enough_title, body));
    }
    if let Some(figures) = usage_figures(ctx, &a, bundle, until) {
        section = section.push(figures);
    }
    if a.enough() {
        let (span, periods) = if hours { (chart::Span::Hours, a.hourly.clone()) } else { (chart::Span::Days, a.daily.clone()) };
        let graph = Chart { span, readings: a.points.clone(), periods, unit: bundle.unit, p, lang: ctx.lang };
        let note = if graph.has_gaps() { format!("{} {}", s.chart_dots, s.chart_gaps) } else { s.chart_dots.to_string() };
        section = section.push(canvas(graph).width(Length::Fill).height(chart::HEIGHT)).push(ctx.muted(note, 12.0));
    }
    container(section).padding(super::pad4(22.0, 0.0, 6.0, 0.0)).into()
}

/// Today, the averages, and when it runs out at this rate.
fn usage_figures<'a>(ctx: &Ctx<'a>, a: &Analysis, bundle: &Bundle, until: Option<chrono::DateTime<chrono::Utc>>) -> Option<El<'a>> {
    let p = ctx.p;
    let s = ctx.s;
    let e = a.estimate?;
    let figure = |label: &str, value: El<'a>, sub: String| -> El<'a> {
        container(column![ctx.caps(label), value, ctx.muted(sub, 12.0)].spacing(3).align_x(ctx.start())).width(Length::FillPortion(1)).align_x(ctx.start()).into()
    };
    let amount = |v: f64| -> El<'a> { ctx.num(i18n::quantity(v, bundle.unit, ctx.lang), 22.0, Weight::ExtraBold, p.tx).into() };
    let mut cells: Vec<El<'a>> = Vec::new();
    if a.enough() {
        cells.push(match a.today {
            Some(today) => figure(s.usage_today, amount(today.used), s.so_far.to_string()),
            None => figure(s.usage_today, ctx.num("—", 22.0, Weight::ExtraBold, p.mu).into(), s.no_refresh_today.to_string()),
        });
    }
    let (over, average) = if e.from_samples { (s.avg_refreshes.to_string(), s.avg_refreshes) } else { (i18n::over_days(e.days, ctx.lang), s.avg_cycle) };
    cells.push(figure(s.per_day, amount(e.per_day), over));
    cells.push(figure(s.per_hour, amount(e.per_hour), average.to_string()));
    let rate = if e.per_day <= 0.0 {
        figure(s.at_this_rate, ctx.text(s.no_usage_yet, 17.0, Weight::ExtraBold, p.tx).into(), String::new())
    } else if let Some(out) = e.runs_out {
        figure(s.at_this_rate, ctx.text(i18n::runs_out(out, ctx.lang), 17.0, Weight::ExtraBold, p.danger).into(), i18n::countdown(out, ctx.now, ctx.lang))
    } else {
        figure(s.at_this_rate, ctx.text(s.lasts_renewal, 17.0, Weight::ExtraBold, p.tx).into(), until.map(|u| i18n::day_label(u, ctx.lang)).unwrap_or_default())
    };
    cells.push(rate);
    Some(ctx.row(cells).spacing(20).align_y(Alignment::Start).into())
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
        let frozen = bundle.state == BundleState::Frozen;
        let kind: El<'a> = match i18n::state_name(bundle.state, ctx.lang) {
            Some(state) => ctx
                .row(vec![
                    ctx.muted(i18n::kind_name(bundle.kind, ctx.lang), 11.5).into(),
                    ctx.muted("·", 11.5).into(),
                    ctx.text(state, 11.5, Weight::Semibold, state_color(ctx, bundle.state)).into(),
                ])
                .spacing(4)
                .into(),
            None => ctx.muted(i18n::kind_name(bundle.kind, ctx.lang), 11.5).into(),
        };
        let name = column![ctx.semi(bundle.name_in(ctx.lang).to_string(), 13.5), kind].spacing(0).align_x(ctx.start());
        let left = ctx
            .row(vec![
                container(super::bar(bundle.remaining_pct() / 100.0, 6.0, if frozen { p.dv } else if low { p.danger } else { p.ac }, p.sf, None, ctx.rtl()))
                    .width(Length::Fill)
                    .into(),
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
            container(ctx.num(format!("{} {unit}", format::amount(bundle.remaining, bundle.unit)), 13.5, Weight::ExtraBold, if frozen { p.mu } else if low { p.danger } else { p.tx }))
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
