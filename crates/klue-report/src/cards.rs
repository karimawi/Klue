//! A line at a glance, as on the app's Home: whose, which, what's left and when it renews.

use chrono::{DateTime, Utc};
use klue_core::{
    Bundle, BundleState, format,
    store::{Account, Card, Lang},
};

use crate::{
    Look,
    look::{CARD, DANGER, DIVIDER, GREY_BAR, HAIRLINE, INK, MUTED, PAPER, SURFACE, YELLOW},
    svg::{Align, Svg, area, font},
    words,
};

/// What a card shows of its line.
pub(crate) struct Shown<'a> {
    pub bundle: Option<&'a Bundle>,
    /// A picked bundle the carrier stopped listing.
    pub gone: bool,
    pub low: bool,
    pub dim: bool,
}

/// The bundle `key`, or the line's main one.
pub(crate) fn shown<'a>(account: &'a Account, key: Option<&str>) -> Shown<'a> {
    let usage = account.usage.as_ref();
    let main = account.main_bundle();
    let bundle = match key {
        Some(key) => usage.and_then(|u| u.bundles.iter().find(|b| b.key == key)),
        None => main,
    };
    let is_main = bundle.zip(main).is_some_and(|(b, m)| b.key == m.key);
    Shown {
        bundle,
        gone: key.is_some() && bundle.is_none() && usage.is_some(),
        low: bundle.is_some_and(|b| account.alert_for(b, is_main).is_triggered(b)),
        dim: bundle.is_some_and(|b| matches!(b.state, BundleState::Frozen | BundleState::Inactive)),
    }
}

/// "Super 140 · Renews in 13 days": the package, or a picked bundle and when it ends.
pub(crate) fn foot(account: &Account, card: &Card, shown: &Shown, now: DateTime<Utc>, lang: Lang) -> String {
    let usage = account.usage.as_ref();
    let renews_at = usage.and_then(|u| u.renews_at);
    let renews = renews_at.map(|at| words::renews_in(at, now, lang));
    let parts: Vec<String> = match (&card.bundle, shown.bundle) {
        (Some(_), Some(b)) => {
            let own_end = b.expires_at.filter(|at| renews_at.is_none_or(|r| (r - *at).num_hours().abs() >= 24));
            std::iter::once(b.name_in(lang).to_string()).chain(own_end.map(|at| words::ends_in(at, now, lang)).or(renews)).collect()
        }
        (Some(_), None) => Vec::new(),
        (None, _) => usage.and_then(|u| u.package_in(lang)).map(String::from).into_iter().chain(renews).collect(),
    };
    parts.join(" · ")
}

pub(crate) const CARD_H: f32 = 150.0;

/// One card, `w` wide, its corner at (`x`, `y`) from the line's start.
pub(crate) fn card(svg: &mut Svg, x: f32, y: f32, w: f32, account: &Account, card: &Card, look: &Look) {
    let lang = look.lang;
    let s = shown(account, card.bundle.as_deref());
    svg.frame(area(x, y, w, CARD_H), 16.0, CARD, HAIRLINE, 1.5);
    let pad = 16.0;
    svg.tile(x + pad, y + pad, 36.0, account.carrier);

    let name_x = x + pad + 36.0 + 12.0;
    let name_w = w - (name_x - x) - pad - if account.last_error.is_some() { 16.0 } else { 0.0 };
    let name = font(15.0, 600, INK);
    let shortened = svg.fit(&account.display_name(), name, name_w);
    svg.text(name_x, y + pad + 15.0, &shortened, name, Align::Start);
    svg.line_icon(name_x, y + pad + 22.0, 12.0, account.line, MUTED);
    svg.text(name_x + 17.0, y + pad + 32.5, &account.pretty_number(), font(12.5, 400, MUTED), Align::Start);
    if account.last_error.is_some() {
        let cx = svg.px(x + w - pad - 4.0);
        svg.raw(&format!(r#"<circle cx="{cx}" cy="{}" r="4" fill="{DANGER}"/>"#, y + pad + 8.0));
    }

    let base = y + 96.0;
    match s.bundle {
        Some(b) => {
            let color = if s.dim { MUTED } else if s.low { DANGER } else { INK };
            let big = font(28.0, 800, color);
            let amount = format::amount(b.remaining, b.unit);
            svg.text(x + pad, base, &amount, big, Align::Start);
            let unit_x = x + pad + svg.width(&amount, big) + 7.0;
            svg.text(unit_x, base - 1.0, &words::left(b.unit, lang), font(13.0, 600, MUTED), Align::Start);
            svg.text(x + w - pad, base - 1.0, &format!("{:.0}%", b.remaining_pct()), font(13.0, 600, MUTED), Align::End);
            let fill = if s.dim { GREY_BAR } else if s.low { DANGER } else { YELLOW };
            svg.bar(area(x + pad, base + 10.0, w - pad * 2.0, 6.0), b.remaining_pct() / 100.0, fill, SURFACE);
        }
        None if s.gone => {
            let kept = if lang == Lang::Ar { card.name_ar.as_deref().or(card.name.as_deref()) } else { card.name.as_deref() };
            svg.text(x + pad, base - 14.0, kept.unwrap_or_default(), font(14.0, 600, INK), Align::Start);
            svg.text(x + pad, base + 6.0, words::pick(lang, "Not on this line right now", "غير موجودة على الخط حالياً"), font(13.0, 400, MUTED), Align::Start);
        }
        None => {
            let waiting = if account.usage.is_some() { "—" } else { words::pick(lang, "Waiting for the first check", "في انتظار أول تحديث") };
            svg.text(x + pad, base - 4.0, waiting, font(13.0, 400, MUTED), Align::Start);
        }
    }

    let foot = foot(account, card, &s, look.now, lang);
    let small = font(12.0, 400, MUTED);
    let foot = svg.fit(&foot, small, w - pad * 2.0);
    svg.text(x + pad, y + CARD_H - pad - 2.0, &foot, small, Align::Start);
}

/// "Generated by Klue · Home VPS · 28 November 2026, 09:42" along the bottom, "Klue"
/// linked. Each piece is placed on its own, so mixed Arabic and Latin keep their order.
pub(crate) fn branding(svg: &mut Svg, x: f32, y: f32, look: &Look, size: f32) {
    let (lead, rest) = words::generated(look.server.as_deref(), look.now, look.clock, look.lang);
    let quiet = font(size, 400, MUTED);
    let strong = font(size, 700, INK);
    let gap = size * 0.38;
    let mut at = x;
    let mut put = |svg: &mut Svg, text: &str, f, link: bool| {
        svg.text(at, y, text, f, Align::Start);
        let w = svg.width(text, f);
        if link {
            svg.link(at, y - size, w, size * 1.3, crate::SITE);
        }
        at += w + gap;
    };
    put(svg, lead, quiet, false);
    put(svg, "Klue", strong, true);
    for piece in &rest {
        put(svg, "·", quiet, false);
        put(svg, piece, quiet, false);
    }
}

/// A Home tab as an image: its name, then its cards two by two.
pub(crate) fn home(title: &str, cards: &[(&Card, &Account)], look: &Look) -> Svg {
    let w = 640.0;
    let pad = 24.0;
    let gap = 12.0;
    let card_w = (w - pad * 2.0 - gap) / 2.0;
    let rows = cards.len().div_ceil(2).max(1) as f32;
    let grid_y = 136.0;
    let foot_y = grid_y + rows * CARD_H + (rows - 1.0) * gap + 34.0;
    let h = foot_y + 22.0;
    let mut svg = Svg::new(w, h, look.lang == Lang::Ar);
    svg.rect(0.0, 0.0, w, h, 0.0, PAPER);

    svg.klue_logo(pad, pad, 24.0);
    svg.text(pad + 32.0, pad + 18.0, "Klue", font(17.0, 800, INK), Align::Start);
    svg.text(w - pad, pad + 17.0, &words::date_long(look.now, look.lang), font(12.5, 400, MUTED), Align::End);

    svg.text(pad, 90.0, title, font(28.0, 800, INK), Align::Start);
    let mut lines: Vec<&str> = cards.iter().map(|(c, _)| c.line.as_str()).collect();
    lines.sort_unstable();
    lines.dedup();
    let mut sub = vec![lines_count(lines.len(), look.lang)];
    if let Some(last) = cards.iter().filter_map(|(_, a)| a.updated_at()).max() {
        sub.push(format!("{} {}", words::pick(look.lang, "Checked", "آخر تحديث"), words::time(last, look.clock, look.lang)));
    }
    svg.text(pad, 114.0, &sub.join(" · "), font(13.0, 400, MUTED), Align::Start);

    if cards.is_empty() {
        svg.frame(area(pad, grid_y, w - pad * 2.0, CARD_H), 16.0, "none", DIVIDER, 1.5);
        svg.text(w / 2.0, grid_y + CARD_H / 2.0 + 5.0, words::pick(look.lang, "No lines on this tab yet", "لا توجد خطوط في هذا التبويب بعد"), font(14.0, 400, MUTED), Align::Middle);
    }
    for (i, (c, account)) in cards.iter().enumerate() {
        let (col, row) = ((i % 2) as f32, (i / 2) as f32);
        card(&mut svg, pad + col * (card_w + gap), grid_y + row * (CARD_H + gap), card_w, account, c, look);
    }
    svg.line(pad, foot_y - 20.0, w - pad, foot_y - 20.0, HAIRLINE, 1.0);
    branding(&mut svg, pad, foot_y, look, 11.5);
    svg
}

/// "4 lines".
pub(crate) fn lines_count(n: usize, lang: Lang) -> String {
    match (lang, n) {
        (Lang::En, 0) => "No lines".into(),
        (Lang::En, 1) => "1 line".into(),
        (Lang::En, n) => format!("{n} lines"),
        (Lang::Ar, 0) => "لا خطوط".into(),
        (Lang::Ar, 1) => "خط واحد".into(),
        (Lang::Ar, 2) => "خطان".into(),
        (Lang::Ar, 3..=10) => format!("{n} خطوط"),
        (Lang::Ar, n) => format!("{n} خط"),
    }
}
