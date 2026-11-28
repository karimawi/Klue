//! Usage graphs, drawn as the app draws them: what was left over time, and what was used
//! each day. Time runs from the line's start, right to left in Arabic.

use std::fmt::Write as _;

use chrono::{DateTime, Duration, Local, Utc};
use klue_core::{
    Bundle, Unit, format,
    history::{self, Analysis},
    store::{Account, Lang},
};

use crate::{
    look::{DANGER, HAIRLINE, INK, MUTED, YELLOW, YELLOW_INK},
    svg::{Align, Area, Svg, font},
    words,
};

/// How far back a report looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Period {
    /// Since the line last renewed.
    #[default]
    Cycle,
    Week,
    Month,
}

/// A bundle's samples over the period, analysed, with where its graph starts and ends.
pub(crate) struct Usage<'a> {
    pub bundle: &'a Bundle,
    pub analysis: Analysis,
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
}

pub(crate) fn usage<'a>(account: &'a Account, bundle: &'a Bundle, period: Period, now: DateTime<Utc>) -> Usage<'a> {
    let u = account.usage.as_ref();
    let cycle = u.and_then(|u| u.cycle_start());
    let until = bundle.expires_at.or(u.and_then(|u| u.renews_at));
    let window = match period {
        Period::Cycle => cycle.unwrap_or(now - Duration::days(30)),
        Period::Week => now - Duration::days(7),
        Period::Month => now - Duration::days(30),
    };
    let samples: Vec<_> = account.history.samples(&bundle.key).iter().filter(|s| s.at >= window).copied().collect();
    let analysis = history::analyse(&samples, bundle, bundle.starts_at.or(cycle), until, now);
    let from = analysis.points.first().map_or(window, |p| p.at).max(window);
    // Through to the renewal when it's close, so the graph shows the time left too.
    let to = match until {
        Some(until) if period == Period::Cycle && until > now && until - from < Duration::days(45) => until,
        _ => now,
    };
    Usage { bundle, analysis, from, to }
}

/// Whether there are enough checks for a graph of what was left: a day's worth.
pub(crate) fn enough(u: &Usage) -> bool {
    let span = u.analysis.points.first().zip(u.analysis.points.last()).map_or(Duration::zero(), |(a, b)| b.at - a.at);
    u.analysis.points.len() >= 2 && span >= Duration::hours(24)
}

/// What was left, from `u.from` to `u.to`, in the box at (`x`, `y`).
pub(crate) fn left_over_time(svg: &mut Svg, a: Area, u: &Usage, lang: Lang, now: DateTime<Utc>) {
    let Area { x, y, w, h } = a;
    let label_w = 46.0;
    let (plot_x, plot_w, plot_h) = (x + label_w, w - label_w, h - 22.0);
    let total = u.analysis.points.iter().map(|p| p.remaining).fold(u.bundle.total, f64::max).max(f64::EPSILON);
    let span = (u.to - u.from).num_seconds().max(1) as f64;
    let at_x = |t: DateTime<Utc>| plot_x + (((t - u.from).num_seconds() as f64 / span).clamp(0.0, 1.0) as f32) * plot_w;
    let at_y = |v: f64| y + plot_h - ((v / total).clamp(0.0, 1.0) as f32) * plot_h;

    // A day of checks at least, or the line is too short to read.
    if !enough(u) {
        svg.line(plot_x, y + plot_h, plot_x + plot_w, y + plot_h, HAIRLINE, 1.0);
        svg.text(plot_x + plot_w / 2.0, y + plot_h / 2.0, words::pick(lang, "Not enough checks yet for a graph", "لا توجد تحديثات كافية للرسم بعد"), font(12.0, 400, MUTED), Align::Middle);
        return;
    }

    // Gridlines at nothing, half and all, labelled at the start.
    let small = font(10.5, 400, MUTED);
    for share in [0.0, 0.5, 1.0] {
        let gy = at_y(total * share);
        svg.line(plot_x, gy, plot_x + plot_w, gy, HAIRLINE, 1.0);
        svg.text(x + label_w - 8.0, gy + 3.5, &format!("{} {}", format::amount_short(total * share, u.bundle.unit), words::unit_name(u.bundle.unit, lang)), small, Align::End);
    }
    // Dates along the bottom: the start, the middle and the end.
    for (i, t) in [u.from, u.from + (u.to - u.from) / 2, u.to].into_iter().enumerate() {
        let align = [Align::Start, Align::Middle, Align::End][i];
        svg.text(at_x(t), y + h - 4.0, &words::date_short(t, lang), small, align);
    }
    // Now, when the graph runs on to the renewal.
    if u.to > now + Duration::hours(12) {
        let nx = at_x(now);
        svg.line(nx, y, nx, y + plot_h, "#D9D5CE", 1.0);
    }

    let points: Vec<(f32, f32)> = u.analysis.points.iter().map(|p| (svg.px(at_x(p.at)), at_y(p.remaining))).collect();
    let mut line = String::new();
    for (i, (px, py)) in points.iter().enumerate() {
        let _ = write!(line, "{}{px:.1} {py:.1} ", if i == 0 { "M" } else { "L" });
    }
    let base = y + plot_h;
    let (first, last) = (points[0], points[points.len() - 1]);
    let area = format!("{line}L{:.1} {base:.1} L{:.1} {base:.1} Z", last.0, first.0);
    svg.path(&area, "rgba(255,190,50,0.22)", "none", 0.0);
    svg.path(&line, "none", YELLOW, 2.5);
    svg.raw(&format!(r#"<circle cx="{:.1}" cy="{:.1}" r="4.5" fill="{YELLOW}" stroke="white" stroke-width="2"/>"#, last.0, last.1));
}

/// Usage per day, the latest `days` of them, with the average as a dashed line.
pub(crate) fn per_day(svg: &mut Svg, a: Area, u: &Usage, days: usize, lang: Lang) {
    let Area { x, y, w, h } = a;
    let label_w = 46.0;
    let (plot_x, plot_w, plot_h) = (x + label_w, w - label_w, h - 22.0);
    let daily: Vec<_> = u.analysis.daily.iter().rev().take(days).rev().collect();
    let small = font(10.5, 400, MUTED);
    if daily.is_empty() {
        svg.line(plot_x, y + plot_h, plot_x + plot_w, y + plot_h, HAIRLINE, 1.0);
        svg.text(plot_x + plot_w / 2.0, y + plot_h / 2.0, words::pick(lang, "A day of checks is needed first", "يلزم يوم من التحديثات أولاً"), font(12.0, 400, MUTED), Align::Middle);
        return;
    }
    let most = daily.iter().map(|d| d.used).fold(0.0, f64::max).max(f64::EPSILON);
    let at_y = |v: f64| y + plot_h - ((v / most).clamp(0.0, 1.0) as f32) * plot_h;
    for share in [0.0, 1.0] {
        let gy = at_y(most * share);
        svg.line(plot_x, gy, plot_x + plot_w, gy, HAIRLINE, 1.0);
        svg.text(x + label_w - 8.0, gy + 3.5, &format!("{} {}", format::amount_short(most * share, u.bundle.unit), words::unit_name(u.bundle.unit, lang)), small, Align::End);
    }
    let slot = plot_w / daily.len() as f32;
    let bar_w = (slot * 0.62).min(26.0);
    let every = daily.len().div_ceil(10).max(1);
    for (i, d) in daily.iter().enumerate() {
        let bx = plot_x + slot * i as f32 + (slot - bar_w) / 2.0;
        let top = at_y(d.used);
        let fill = if d.rough() { "#F3DDA8" } else { YELLOW };
        svg.rect(bx, top, bar_w, (y + plot_h - top).max(1.5), 3.0_f32.min(bar_w / 2.0), fill);
        if i % every == 0 || i == daily.len() - 1 {
            let day = d.start.with_timezone(&Local);
            svg.text(bx + bar_w / 2.0, y + h - 4.0, &day.format("%-d").to_string(), small, Align::Middle);
        }
    }
    if let Some(e) = u.analysis.estimate {
        let ay = at_y(e.per_day);
        let (x1, x2) = (svg.px(plot_x), svg.px(plot_x + plot_w));
        svg.raw(&format!(r#"<line x1="{x1:.1}" y1="{ay:.1}" x2="{x2:.1}" y2="{ay:.1}" stroke="{INK}" stroke-width="1.2" stroke-dasharray="4 4"/>"#));
    }
}

/// "4.7 GB a day on average · runs out Sun 6 Dec": the figures under the graphs.
pub(crate) fn figures(u: &Usage, unit: Unit, lang: Lang) -> Vec<(String, &'static str)> {
    let mut out = Vec::new();
    if let Some(e) = u.analysis.estimate {
        let per_day = format!("{} {}", format::amount(e.per_day, unit), words::unit_name(unit, lang));
        out.push((if lang == Lang::Ar { format!("{per_day} في اليوم في المتوسط") } else { format!("{per_day} a day on average") }, INK));
        if let Some(out_at) = e.runs_out {
            let when = words::date_short(out_at, lang);
            out.push((if lang == Lang::Ar { format!("ينفد في {when} بهذا المعدل") } else { format!("Runs out {when} at this rate") }, DANGER));
        }
    }
    if let Some(today) = u.analysis.today {
        let used = format!("{} {}", format::amount(today.used, unit), words::unit_name(unit, lang));
        out.push((if lang == Lang::Ar { format!("{used} اليوم حتى الآن") } else { format!("{used} today so far") }, YELLOW_INK));
    }
    out
}
