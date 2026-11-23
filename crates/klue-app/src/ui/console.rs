//! The debug console window: the live log, with a level filter, search, copy and clear.

use iced::{
    Alignment, Color, Font, Length,
    font::Weight,
    widget::{column, container, row, scrollable, text, text_input},
};
use klue_core::Carrier;
use tracing::Level;

use super::{Ctx, El, hline};
use crate::{
    app::{App, CONSOLE_SCROLL, Message},
    theme,
};

pub fn view(app: &App) -> El<'_> {
    let ctx = Ctx::new(app, Carrier::We, true);
    let p = ctx.p;
    let log = &app.log;
    let levels = ctx.segmented(
        [(Level::TRACE, "All"), (Level::DEBUG, "Debug"), (Level::INFO, "Info"), (Level::WARN, "Warnings"), (Level::ERROR, "Errors")]
            .into_iter()
            .map(|(level, label)| (label.to_string(), log.level == level, Some(Message::ConsoleLevel(level))))
            .collect(),
        32.0,
        12.5,
    );
    let search = text_input("Search", &log.search).on_input(Message::ConsoleSearch).padding([6, 10]).size(13).width(220).style(theme::field(p, false));
    let follow = row![ctx.toggle(log.follow, Message::ConsoleFollow), ctx.body("Follow", 13.0)].spacing(8).align_y(Alignment::Center);
    let bar = row![
        levels,
        search,
        container(ctx.muted(format!("{} lines", log.lines.len()), 12.0)).width(Length::Fill),
        follow,
        ctx.outline("Copy", Some(Message::ConsoleCopy)),
        ctx.outline("Clear", Some(Message::ConsoleClear)),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    let mono = |content: String, size: f32, color: Color| text(content).font(Font::MONOSPACE).size(size).color(color);
    let lines: Vec<El<'_>> = log
        .lines
        .iter()
        .map(|line| {
            let color = match line.level {
                Level::ERROR => p.danger,
                Level::WARN => theme::YELLOW,
                Level::INFO => p.tx,
                _ => p.mu,
            };
            row![
                mono(line.at.format("%H:%M:%S%.3f").to_string(), 12.0, p.mu),
                container(mono(line.level.to_string(), 12.0, color).font(Font { weight: Weight::Bold, ..Font::MONOSPACE })).width(48),
                container(mono(short_target(&line.target), 12.0, p.mu)).width(190).clip(true),
                container(mono(line.text.clone(), 12.0, if line.level <= Level::INFO { p.tx } else { p.mu })).width(Length::Fill),
            ]
            .spacing(10)
            .into()
        })
        .collect();
    let body: El<'_> = if lines.is_empty() {
        container(ctx.muted("Nothing logged yet.", 13.0)).padding(16).into()
    } else {
        super::smooth::smooth(scrollable(container(column(lines).spacing(2)).padding([10, 16])).id(CONSOLE_SCROLL).style(theme::scroller(p)).height(Length::Fill))
    };
    column![container(bar).padding([10, 16]), hline(p.dv, 1.0), container(body).height(Length::Fill)].into()
}

/// `winit::platform_impl::linux::x11` -> `linux::x11`.
fn short_target(target: &str) -> String {
    let parts: Vec<&str> = target.split("::").collect();
    parts[parts.len().saturating_sub(2)..].join("::")
}
