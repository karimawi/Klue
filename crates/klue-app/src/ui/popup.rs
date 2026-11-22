//! The tray popup: every number at a glance. It's a fixed panel next to the tray icon:
//! there is no drag handle, and it closes as soon as it loses focus.

use iced::{
    Alignment, Length,
    font::Weight,
    widget::{button, column, container, scrollable},
};
use klue_core::{Carrier, format};

use super::{Ctx, El, hline, tile};
use crate::{app::{App, Message}, assets::Icon, i18n, theme};

pub fn view(app: &App) -> El<'_> {
    let ctx = Ctx::new(app, Carrier::We, true);
    let p = ctx.p;
    let s = ctx.s;
    let data = &app.snap.data;
    let refreshing = !app.snap.refreshing.is_empty();

    let head = ctx
        .row(vec![
            super::logo(16.0),
            ctx.bold("Klue", 15.0).into(),
            container(ctx.muted(i18n::synced(data.last_sync(), ctx.now, ctx.lang), 12.0)).width(Length::Fill).align_x(ctx.start()).into(),
            ctx.tool(Icon::Refresh, s.refresh_all, (!refreshing).then_some(Message::RefreshAll), refreshing),
        ])
        .spacing(10);

    let rows: Vec<El<'_>> = data
        .accounts
        .iter()
        .map(|account| {
            let t = crate::assets::tile(account.carrier);
            let main = account.main_bundle();
            let low = account.main_alert_triggered();
            let renews = account.usage.as_ref().and_then(|u| u.renews_at).map(|r| {
                let days = (r - ctx.now).num_days().max(0);
                if ctx.rtl() { format!("يتجدد خلال {days} يوم") } else { format!("Renews in {days}d") }
            });
            let sub = [renews, Some(account.pretty_number())].into_iter().flatten().collect::<Vec<_>>().join(" · ");
            let text = column![ctx.semi(account.display_name(), 13.5), ctx.muted(sub, 11.5)].spacing(0).align_x(ctx.start()).width(Length::Fill);
            let right: El<'_> = match main {
                Some(m) => column![
                    ctx.num(format!("{} {}", format::amount(m.remaining, m.unit), i18n::unit_name(m.unit, ctx.lang)), 14.0, Weight::ExtraBold, if low { p.danger } else { p.tx }),
                    super::bar(m.remaining_pct() / 100.0, 4.0, if low { p.danger } else { p.ac }, p.sf, None, ctx.rtl()),
                ]
                .spacing(5)
                .align_x(ctx.end())
                .width(96)
                .into(),
                None => container(ctx.muted("—", 14.0)).width(96).align_x(ctx.end()).into(),
            };
            let content = ctx.row(vec![tile(account.carrier, 34.0, t.tray, 10.0, false), text.into(), right]).spacing(12);
            column![
                button(container(content).padding([11, 16])).padding(0).width(Length::Fill).style(theme::plain(p.sf, None, 0.0)).on_press(Message::ShowMain(Some(account.id.clone()))),
                hline(p.dvs, 1.0)
            ]
            .into()
        })
        .collect();
    let list: El<'_> = if rows.is_empty() {
        container(ctx.muted(s.tray_empty, 13.0)).padding([24, 16]).into()
    } else {
        scrollable(column(rows)).style(theme::scroller(p)).height(Length::Fill).into()
    };

    let open = ctx.primary(s.open_app, Some(Message::ShowMain(None))).width(Length::Fill);
    let settings = ctx.tool(Icon::Sliders, s.settings, Some(Message::PopupSettings), false);
    let foot = ctx.row(vec![open.into(), settings]).spacing(8).align_y(Alignment::Center);

    container(column![container(head).padding([12, 16]), hline(p.dv, 2.0), container(list).height(Length::Fill), hline(p.dv, 2.0), container(foot).padding([12, 16])])
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| {
            let mut st = theme::fill(p.bg, 0.0);
            st.border = theme::border(p.dv, 1.0, 0.0);
            st
        })
        .into()
}
