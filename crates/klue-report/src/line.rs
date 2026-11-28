//! One line in full, as on its page in the app: its main bundle, renewal, balance and the
//! other bundles. Also the top of each line's page in the report.

use klue_core::{
    Unit, format,
    store::{Account, Lang},
};

use crate::{
    Look,
    cards::{self, shown},
    look::{CARD, DANGER, HAIRLINE, INK, MUTED, PAPER, SURFACE, YELLOW, YELLOW_INK, GREY_BAR},
    svg::{Align, Svg, area, font},
    words,
};

/// Name, number and kind of line, `w` wide from (`x`, `y`). Returns the height used.
pub(crate) fn header(svg: &mut Svg, x: f32, y: f32, w: f32, account: &Account, lang: Lang) -> f32 {
    svg.tile(x, y, 52.0, account.carrier);
    let tx = x + 52.0 + 14.0;
    let name = font(23.0, 800, INK);
    let title = svg.fit(&account.display_name(), name, w - (tx - x) - 120.0);
    svg.text(tx, y + 24.0, &title, name, Align::Start);
    svg.line_icon(tx, y + 32.0, 14.0, account.line, MUTED);
    let mut sub = vec![words::ltr(&account.pretty_number()), words::carrier_name(account.carrier, lang).to_string(), words::line_name(account.line, lang).to_string()];
    if let Some(owner) = account.owner_name.as_ref().filter(|o| Some(o.as_str()) != account.nickname.as_deref()) {
        sub.push(owner.clone());
    }
    let quiet = font(13.5, 400, MUTED);
    let sub = svg.fit(&sub.join(" · "), quiet, w - (tx - x) - 20.0);
    svg.text(tx + 20.0, y + 44.5, &sub, quiet, Align::Start);

    let s = shown(account, None);
    let pill = if s.low {
        Some((words::pick(lang, "Below alert", "أقل من التنبيه"), DANGER))
    } else if let Some(state) = s.bundle.and_then(|b| words::state_name(b.state, lang)) {
        Some((state, MUTED))
    } else if account.last_error.is_some() {
        Some((words::pick(lang, "Last check failed", "فشل آخر تحديث"), DANGER))
    } else {
        None
    };
    if let Some((label, color)) = pill {
        let f = font(11.5, 700, "#FFFFFF");
        let pw = svg.width(label, f) + 22.0;
        svg.rect(x + w - pw, y + 8.0, pw, 24.0, 12.0, color);
        svg.text(x + w - pw / 2.0, y + 24.5, label, f, Align::Middle);
    }
    60.0
}

/// The main bundle's panel. Returns its height.
pub(crate) fn main_panel(svg: &mut Svg, x: f32, y: f32, w: f32, account: &Account, look: &Look) -> f32 {
    let lang = look.lang;
    let h = 210.0;
    svg.frame(area(x, y, w, h), 18.0, CARD, HAIRLINE, 1.5);
    let pad = 22.0;
    let usage = account.usage.as_ref();
    let s = shown(account, None);
    let Some(b) = s.bundle else {
        let note = if usage.is_some() { "—" } else { words::pick(lang, "Waiting for the first check", "في انتظار أول تحديث") };
        svg.text(x + pad, y + 50.0, note, font(15.0, 400, MUTED), Align::Start);
        return h;
    };
    let caps = font(11.0, 700, MUTED);
    svg.text(x + pad, y + 30.0, words::pick(lang, "PACKAGE", "الباقة"), caps, Align::Start);
    let package = usage.and_then(|u| u.package_in(lang)).unwrap_or(b.name_in(lang)).to_string();
    svg.text(x + pad, y + 54.0, &svg.fit(&package, font(19.0, 800, INK), w * 0.6), font(19.0, 800, INK), Align::Start);
    let mut under = Vec::new();
    if b.name_in(lang) != package {
        under.push(b.name_in(lang).to_string());
    }
    if let Some(speed) = usage.and_then(|u| u.info.speed.as_ref()) {
        under.push(words::speed(speed.down_kbps, lang));
    }
    if !under.is_empty() {
        svg.text(x + pad, y + 74.0, &under.join(" · "), font(13.0, 400, MUTED), Align::Start);
    }

    let color = if s.dim { MUTED } else if s.low { DANGER } else { INK };
    let big = font(64.0, 800, color);
    let amount = format::amount(b.remaining, b.unit);
    let base = y + 142.0;
    svg.text(x + pad, base, &amount, big, Align::Start);
    let ux = x + pad + svg.width(&amount, big) + 10.0;
    svg.text(ux, base - 22.0, &words::left(b.unit, lang), font(16.0, 700, INK), Align::Start);
    let of = if lang == Lang::Ar { format!("من {} {}", format::amount_short(b.total, b.unit), words::unit_name(b.unit, lang)) } else { format!("of {} {}", format::amount_short(b.total, b.unit), words::unit_name(b.unit, lang)) };
    svg.text(ux, base - 3.0, &of, font(13.0, 400, MUTED), Align::Start);
    svg.text(x + w - pad, base - 20.0, &format!("{:.1}%", b.remaining_pct()), font(32.0, 800, INK), Align::End);
    svg.text(x + w - pad, base - 2.0, words::pick(lang, "remaining", "متبقية"), font(12.0, 400, MUTED), Align::End);

    let bar_y = base + 16.0;
    let bar_w = w - pad * 2.0;
    let fill = if s.dim { GREY_BAR } else if s.low { DANGER } else { YELLOW };
    svg.bar(area(x + pad, bar_y, bar_w, 10.0), b.remaining_pct() / 100.0, fill, SURFACE);
    let alert = account.alert_for(b, true);
    if alert.enabled && b.total > 0.0 {
        let at = (alert.threshold(b) / b.total).clamp(0.0, 1.0) as f32;
        let mx = x + pad + bar_w * at;
        svg.rect(mx - 1.5, bar_y - 4.0, 3.0, 18.0, 1.5, INK);
    }
    let small = font(12.0, 400, MUTED);
    let used = if lang == Lang::Ar { format!("استُخدم {} {}", format::amount(b.used(), b.unit), words::unit_name(b.unit, lang)) } else { format!("{} {} used", format::amount(b.used(), b.unit), words::unit_name(b.unit, lang)) };
    svg.text(x + pad, bar_y + 30.0, &used, small, Align::Start);
    if alert.enabled {
        let at = format!("{} {}", format::amount_short(alert.threshold(b), b.unit), words::unit_name(b.unit, lang));
        svg.text(x + w - pad, bar_y + 30.0, &if lang == Lang::Ar { format!("تنبيه عند {at}") } else { format!("Alert at {at}") }, small, Align::End);
    }
    h
}

/// Renewal and balance side by side. Returns the height.
pub(crate) fn facts(svg: &mut Svg, x: f32, y: f32, w: f32, account: &Account, look: &Look) -> f32 {
    let lang = look.lang;
    let h = 92.0;
    let gap = 12.0;
    let half = (w - gap) / 2.0;
    let usage = account.usage.as_ref();
    let caps = font(11.0, 700, MUTED);
    let pad = 20.0;

    svg.frame(area(x, y, half, h), 18.0, CARD, HAIRLINE, 1.5);
    svg.text(x + pad, y + 28.0, words::pick(lang, "NEXT RENEWAL", "التجديد القادم"), caps, Align::Start);
    match usage.and_then(|u| u.renews_at) {
        Some(at) => {
            svg.text(x + pad, y + 54.0, &svg.fit(&words::date_long(at, lang), font(16.5, 800, INK), half - pad * 2.0), font(16.5, 800, INK), Align::Start);
            svg.text(x + pad, y + 75.0, &words::in_days(at, look.now, lang), font(13.0, 600, YELLOW_INK), Align::Start);
        }
        None => svg.text(x + pad, y + 56.0, "—", font(16.0, 800, MUTED), Align::Start),
    }

    let bx = x + half + gap;
    svg.frame(area(bx, y, half, h), 18.0, CARD, HAIRLINE, 1.5);
    svg.text(bx + pad, y + 28.0, words::pick(lang, "BALANCE", "الرصيد"), caps, Align::Start);
    match usage.and_then(|u| u.balance_egp) {
        Some(egp) => {
            let big = font(26.0, 800, INK);
            let money = format::money(egp);
            svg.text(bx + pad, y + 62.0, &money, big, Align::Start);
            svg.text(bx + pad + svg.width(&money, big) + 6.0, y + 62.0, words::unit_name(Unit::Egp, lang), font(13.0, 700, MUTED), Align::Start);
            if let Some(owed) = usage.and_then(|u| u.owed_egp).filter(|o| *o > 0.0) {
                let owes = if lang == Lang::Ar { format!("مستحق {} جنيه", format::money(owed)) } else { format!("{} EGP owed", format::money(owed)) };
                svg.text(bx + pad, y + 80.0, &owes, font(12.0, 600, DANGER), Align::Start);
            }
        }
        None => svg.text(bx + pad, y + 56.0, "—", font(16.0, 800, MUTED), Align::Start),
    }
    h
}

/// The other bundles, each with what's left. Returns the height, 0 if there are none.
pub(crate) fn others(svg: &mut Svg, x: f32, y: f32, w: f32, account: &Account, look: &Look) -> f32 {
    let lang = look.lang;
    let Some(usage) = account.usage.as_ref() else { return 0.0 };
    let rest: Vec<_> = usage.ordered(account.main_bundle.as_deref()).into_iter().skip(1).collect();
    if rest.is_empty() {
        return 0.0;
    }
    let row = 50.0;
    let h = 46.0 + row * rest.len() as f32 + 8.0;
    svg.frame(area(x, y, w, h), 18.0, CARD, HAIRLINE, 1.5);
    let pad = 22.0;
    svg.text(x + pad, y + 30.0, words::pick(lang, "OTHER BUNDLES", "باقات أخرى"), font(11.0, 700, MUTED), Align::Start);
    for (i, b) in rest.iter().enumerate() {
        let ry = y + 46.0 + row * i as f32;
        if i > 0 {
            svg.line(x + pad, ry, x + w - pad, ry, HAIRLINE, 1.0);
        }
        let low = account.alert_for(b, false).is_triggered(b);
        let name = font(14.0, 700, INK);
        svg.text(x + pad, ry + 21.0, &svg.fit(b.name_in(lang), name, w * 0.5), name, Align::Start);
        let mut kind = words::kind_name(b.kind, lang).to_string();
        if let Some(state) = words::state_name(b.state, lang) {
            kind = format!("{kind} · {state}");
        }
        svg.text(x + pad, ry + 38.0, &kind, font(11.5, 400, MUTED), Align::Start);
        let left = format!("{} {}", format::amount(b.remaining, b.unit), words::unit_name(b.unit, lang));
        let of = if lang == Lang::Ar { format!(" من {}", format::amount_short(b.total, b.unit)) } else { format!(" of {}", format::amount_short(b.total, b.unit)) };
        let strong = font(14.0, 800, if low { DANGER } else { INK });
        let quiet = font(12.0, 400, MUTED);
        let of_w = svg.width(&of, quiet);
        svg.text(x + w - pad, ry + 21.0, &of, quiet, Align::End);
        svg.text(x + w - pad - of_w, ry + 21.0, &left, strong, Align::End);
        svg.bar(area(x + w - pad - 150.0, ry + 31.0, 150.0, 5.0), b.remaining_pct() / 100.0, if low { DANGER } else { YELLOW }, SURFACE);
    }
    h
}

/// A line as an image.
pub(crate) fn image(account: &Account, look: &Look) -> Svg {
    let w = 640.0;
    let pad = 24.0;
    let inner = w - pad * 2.0;
    // Laid out once to measure, then for real.
    let mut probe = Svg::new(w, 2000.0, look.lang == Lang::Ar);
    let height = layout(&mut probe, account, look, pad, inner);
    let mut svg = Svg::new(w, height, look.lang == Lang::Ar);
    svg.rect(0.0, 0.0, w, height, 0.0, PAPER);
    layout(&mut svg, account, look, pad, inner);
    svg
}

fn layout(svg: &mut Svg, account: &Account, look: &Look, pad: f32, inner: f32) -> f32 {
    let mut y = pad;
    y += header(svg, pad, y, inner, account, look.lang) + 14.0;
    y += main_panel(svg, pad, y, inner, account, look) + 12.0;
    y += facts(svg, pad, y, inner, account, look) + 12.0;
    let o = others(svg, pad, y, inner, account, look);
    if o > 0.0 {
        y += o + 12.0;
    }
    y += 22.0;
    svg.line(pad, y - 22.0, pad + inner, y - 22.0, HAIRLINE, 1.0);
    cards::branding(svg, pad, y, look, 11.5);
    y + 20.0
}
