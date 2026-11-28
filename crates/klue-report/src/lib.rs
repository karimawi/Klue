//! Klue's pictures of lines, and its PDF report, drawn as SVG: PNG images for chats and
//! messages, a PDF for reports. They look like the app, in English or Arabic.

mod cards;
mod chart;
mod fonts;
mod line;
mod look;
mod report;
mod svg;
pub mod words;

use chrono::{DateTime, Utc};
use klue_core::store::{Account, Card, Clock, Lang};
use resvg::{tiny_skia, usvg};
use svg::area;

pub use chart::Period;
pub use look::carrier_colour;

/// Where "Klue" links to.
pub const SITE: &str = "https://klue.wkar.im";

/// The language, clock and moment pictures are made in, and the server's name for their
/// footer.
#[derive(Debug, Clone)]
pub struct Look {
    pub lang: Lang,
    pub clock: Clock,
    pub now: DateTime<Utc>,
    pub server: Option<String>,
}

/// Pictures are drawn twice their size, sharp on phones.
const SCALE: f32 = 2.0;

/// A Home tab's cards, as a PNG.
pub fn home_png(title: &str, cards: &[(&Card, &Account)], look: &Look) -> Option<Vec<u8>> {
    png(&cards::home(title, cards, look).finish(), SCALE)
}

/// A line's numbers, renewal, balance and bundles, as a PNG.
pub fn line_png(account: &Account, look: &Look) -> Option<Vec<u8>> {
    png(&line::image(account, look).finish(), SCALE)
}

/// A bundle's graphs (the main one's when `bundle` is `None`), as a PNG. `None` if the
/// line has no bundles yet.
pub fn usage_png(account: &Account, bundle: Option<&str>, look: &Look) -> Option<Vec<u8>> {
    let b = cards::shown(account, bundle).bundle?;
    let u = chart::usage(account, b, Period::Cycle, look.now);
    let w = 640.0;
    let pad = 24.0;
    let inner = w - pad * 2.0;
    let figures = chart::figures(&u, b.unit, look.lang);
    let h = 76.0 + 210.0 + 46.0 + 140.0 + if figures.is_empty() { 0.0 } else { 34.0 } + 56.0;
    let mut svg = svg::Svg::new(w, h, look.lang == Lang::Ar);
    svg.rect(0.0, 0.0, w, h, 0.0, look::PAPER);
    svg.tile(pad, pad, 36.0, account.carrier);
    let strong = svg::font(17.0, 800, look::INK);
    svg.text(pad + 48.0, pad + 16.0, &svg.fit(&account.display_name(), strong, inner - 60.0), strong, svg::Align::Start);
    svg.text(pad + 48.0, pad + 34.0, &format!("{} · {}", b.name_in(look.lang), words::pick(look.lang, "what was left and what was used", "المتبقي والمستخدم")), svg::font(12.5, 400, look::MUTED), svg::Align::Start);
    let mut y = 76.0;
    chart::left_over_time(&mut svg, area(pad, y, inner, 210.0), &u, look.lang, look.now);
    y += 210.0 + 18.0;
    svg.text(pad, y + 14.0, words::pick(look.lang, "Used each day", "الاستخدام اليومي"), svg::font(13.0, 800, look::INK), svg::Align::Start);
    y += 28.0;
    chart::per_day(&mut svg, area(pad, y, inner, 140.0), &u, 14, look.lang);
    y += 140.0;
    if !figures.is_empty() {
        let mut fx = pad;
        for (text, colour) in &figures {
            let f = svg::font(12.5, 700, colour);
            svg.text(fx, y + 26.0, text, f, svg::Align::Start);
            fx += svg.width(text, f) + 18.0;
        }
        y += 34.0;
    }
    svg.line(pad, y + 18.0, w - pad, y + 18.0, look::HAIRLINE, 1.0);
    cards::branding(&mut svg, pad, y + 40.0, look, 11.5);
    png(&svg.finish(), SCALE)
}

/// Klue's logo, `size` pixels square, for email headers.
pub fn logo_png(size: u32) -> Option<Vec<u8>> {
    let mut svg = svg::Svg::new(size as f32, size as f32, false);
    svg.klue_logo(0.0, 0.0, size as f32);
    png(&svg.finish(), 1.0)
}

/// The report on `accounts` over `period`, as a PDF.
pub fn report_pdf(accounts: &[&Account], period: Period, look: &Look) -> Option<Vec<u8>> {
    report::report(accounts, period, look)
}

/// An SVG drawn at `scale` times its size, as PNG.
pub fn png(svg: &str, scale: f32) -> Option<Vec<u8>> {
    let options = usvg::Options { fontdb: fonts::database(), ..Default::default() };
    let tree = usvg::Tree::from_str(svg, &options).ok()?;
    let size = tree.size().to_int_size().scale_by(scale)?;
    let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height())?;
    resvg::render(&tree, tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    pixmap.encode_png().ok()
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use klue_core::store::{Clock, Lang};

    use super::*;

    fn look(lang: Lang) -> Look {
        Look { lang, clock: Clock::H24, now: Utc::now(), server: Some("Home VPS".into()) }
    }

    /// Demo mode's made-up lines, with bundles and history.
    fn lines() -> Vec<Account> {
        klue_engine::demo::data().accounts
    }

    #[test]
    fn pictures_and_reports_are_made_in_both_languages() {
        let accounts = lines();
        let out = std::env::var("KLUE_REPORT_OUT").ok();
        for lang in [Lang::En, Lang::Ar] {
            let look = look(lang);
            let cards: Vec<Card> = accounts.iter().map(|a| Card::main(&a.id)).collect();
            let pairs: Vec<(&Card, &Account)> = cards.iter().zip(accounts.iter()).collect();
            let home = home_png("My lines", &pairs, &look).expect("home");
            let line = line_png(&accounts[0], &look).expect("line");
            let usage = usage_png(&accounts[0], None, &look).expect("usage");
            let refs: Vec<&Account> = accounts.iter().collect();
            let pdf = report_pdf(&refs, Period::Cycle, &look).expect("report");
            assert!(home.starts_with(b"\x89PNG") && line.starts_with(b"\x89PNG") && usage.starts_with(b"\x89PNG"));
            assert!(pdf.starts_with(b"%PDF"));
            if let Some(dir) = &out {
                let tag = if lang == Lang::Ar { "ar" } else { "en" };
                std::fs::write(format!("{dir}/home-{tag}.png"), &home).unwrap();
                std::fs::write(format!("{dir}/line-{tag}.png"), &line).unwrap();
                std::fs::write(format!("{dir}/usage-{tag}.png"), &usage).unwrap();
                std::fs::write(format!("{dir}/report-{tag}.pdf"), &pdf).unwrap();
            }
        }
    }
}
