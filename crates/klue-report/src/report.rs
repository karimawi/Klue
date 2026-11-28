//! The PDF report: a page summing up every line, then each line with its graphs. A4, laid
//! out as SVG pages and turned into PDF with real, selectable text.

use klue_core::{
    format,
    store::{Account, Lang},
};
use krilla::{
    Document,
    action::LinkAction,
    annotation::{LinkAnnotation, Target},
    geom::{Rect, Size},
    metadata::Metadata,
    page::PageSettings,
};
use krilla_svg::{SurfaceExt, SvgSettings};
use resvg::usvg;

use crate::{
    Look,
    cards::{self, shown},
    chart::{self, Period},
    fonts, line,
    look::{CARD, DANGER, HAIRLINE, INK, MUTED, PAPER, SURFACE, YELLOW, YELLOW_INK},
    svg::{Align, Svg, area, font},
    words,
};

const W: f32 = 595.0;
const H: f32 = 842.0;
const MARGIN: f32 = 40.0;
const INNER: f32 = W - MARGIN * 2.0;
/// Where a page's content has to stop, above the footer.
const BOTTOM: f32 = H - 58.0;

/// Pages being filled from the top.
struct Pages {
    rtl: bool,
    done: Vec<Svg>,
    page: Svg,
    y: f32,
}

impl Pages {
    fn new(rtl: bool) -> Self {
        Self { rtl, done: Vec::new(), page: Self::blank(rtl), y: MARGIN }
    }

    fn blank(rtl: bool) -> Svg {
        let mut page = Svg::new(W, H, rtl);
        page.rect(0.0, 0.0, W, H, 0.0, PAPER);
        page
    }

    fn new_page(&mut self) {
        let full = std::mem::replace(&mut self.page, Self::blank(self.rtl));
        self.done.push(full);
        self.y = MARGIN;
    }

    /// Draws a block, on a new page if it doesn't fit on this one. `draw` gets the page and
    /// the top of the block, and returns the block's height.
    fn place(&mut self, gap_after: f32, draw: impl Fn(&mut Svg, f32) -> f32) {
        let mut probe = Svg::new(W, H * 4.0, self.rtl);
        let height = draw(&mut probe, MARGIN);
        if self.y + height > BOTTOM && self.y > MARGIN {
            self.new_page();
        }
        let used = draw(&mut self.page, self.y);
        self.y += used + gap_after;
    }

    /// All pages, each with its footer.
    fn finish(mut self, look: &Look) -> Vec<Svg> {
        self.new_page();
        let count = self.done.len();
        for (i, page) in self.done.iter_mut().enumerate() {
            page.line(MARGIN, H - 42.0, W - MARGIN, H - 42.0, HAIRLINE, 1.0);
            cards::branding(page, MARGIN, H - 24.0, look, 8.5);
            let n = if look.lang == Lang::Ar { format!("صفحة {} من {}", i + 1, count) } else { format!("Page {} of {}", i + 1, count) };
            page.text(W - MARGIN, H - 24.0, &n, font(8.5, 400, MUTED), Align::End);
        }
        self.done
    }
}

/// The report on `accounts`, as a PDF.
pub(crate) fn report(accounts: &[&Account], period: Period, look: &Look) -> Option<Vec<u8>> {
    let lang = look.lang;
    let mut pages = Pages::new(lang == Lang::Ar);

    pages.place(18.0, |svg, y| title(svg, y, accounts, period, look));
    if accounts.is_empty() {
        pages.place(0.0, |svg, y| {
            svg.text(MARGIN, y + 20.0, words::pick(lang, "No lines are included in reports.", "لا توجد خطوط مضمنة في التقارير."), font(12.0, 400, MUTED), Align::Start);
            30.0
        });
    } else {
        pages.place(10.0, |svg, y| table_head(svg, y, lang));
        for account in accounts {
            pages.place(0.0, |svg, y| table_row(svg, y, account, look));
        }
    }

    for account in accounts {
        pages.new_page();
        pages.place(12.0, |svg, y| scaled(svg, y, |piece, x, y, w| line::header(piece, x, y, w, account, lang)));
        pages.place(12.0, |svg, y| figures_strip(svg, y, account, look));
        if let Some(main) = shown(account, None).bundle {
            let u = chart::usage(account, main, period, look.now);
            pages.place(12.0, |svg, y| graphs(svg, y, &u, true, look));
        }
        let others: Vec<_> = account.usage.as_ref().map(|u| u.ordered(account.main_bundle.as_deref()).into_iter().skip(1).collect()).unwrap_or_default();
        if !others.is_empty() {
            pages.place(12.0, |svg, y| bundle_table(svg, y, account, &others, look));
        }
        // The other bundles' graphs, two to a row.
        let usages: Vec<_> = others.into_iter().map(|b| chart::usage(account, b, period, look.now)).filter(chart::enough).collect();
        for pair in usages.chunks(2) {
            pages.place(12.0, |svg, y| pair.iter().enumerate().map(|(i, u)| small_graph(svg, y, i, u, look)).fold(0.0, f32::max));
        }
    }

    let pages = pages.finish(look);
    let title = words::pick(lang, "Klue usage report", "تقرير استخدام Klue");
    pdf(pages, title, lang)
}

/// The line's panels are drawn as in the images, 592 wide, and scaled to the page.
const PANEL_SCALE: f32 = INNER / 592.0;

/// Draws a panel made for the images at the page's width. `draw` gets the piece, its
/// position and width in the piece's own units, and returns its height in them.
fn scaled(svg: &mut Svg, y: f32, draw: impl Fn(&mut Svg, f32, f32, f32) -> f32) -> f32 {
    let mut piece = Svg::new(W / PANEL_SCALE, H / PANEL_SCALE, svg.rtl);
    let height = draw(&mut piece, MARGIN / PANEL_SCALE, y / PANEL_SCALE, INNER / PANEL_SCALE);
    svg.embed(piece, PANEL_SCALE);
    height * PANEL_SCALE
}

/// Klue's mark, "Usage report", the period and the server. Returns the height.
fn title(svg: &mut Svg, y: f32, accounts: &[&Account], period: Period, look: &Look) -> f32 {
    let lang = look.lang;
    svg.klue_logo(MARGIN, y, 26.0);
    svg.text(MARGIN + 34.0, y + 19.0, "Klue", font(16.0, 800, INK), Align::Start);
    svg.link(MARGIN, y, 70.0, 26.0, crate::SITE);
    svg.text(W - MARGIN, y + 18.0, &words::stamp(look.now, look.clock, lang), font(9.5, 400, MUTED), Align::End);
    svg.text(MARGIN, y + 78.0, words::pick(lang, "Usage report", "تقرير الاستخدام"), font(30.0, 800, INK), Align::Start);
    let from = match period {
        Period::Cycle => accounts.iter().filter_map(|a| a.usage.as_ref()?.cycle_start()).min().unwrap_or(look.now - chrono::Duration::days(30)),
        Period::Week => look.now - chrono::Duration::days(7),
        Period::Month => look.now - chrono::Duration::days(30),
    };
    let mut sub = vec![words::date_range(from, look.now, lang), cards::lines_count(accounts.len(), lang)];
    if let Some(server) = &look.server {
        sub.push(server.clone());
    }
    svg.text(MARGIN, y + 100.0, &sub.join(" · "), font(11.0, 400, MUTED), Align::Start);
    svg.rect(MARGIN, y + 116.0, 44.0, 4.0, 2.0, YELLOW);
    124.0
}

// Columns of the summary table, from the line's start.
const COL_LEFT: f32 = MARGIN + 205.0;
const COL_SHARE: f32 = MARGIN + 305.0;
const COL_RENEWS: f32 = MARGIN + 395.0;

fn table_head(svg: &mut Svg, y: f32, lang: Lang) -> f32 {
    let caps = font(8.5, 700, MUTED);
    svg.text(MARGIN, y + 12.0, words::pick(lang, "LINE", "الخط"), caps, Align::Start);
    svg.text(COL_LEFT, y + 12.0, words::pick(lang, "LEFT", "المتبقي"), caps, Align::Start);
    svg.text(COL_SHARE, y + 12.0, words::pick(lang, "SHARE LEFT", "النسبة"), caps, Align::Start);
    svg.text(COL_RENEWS, y + 12.0, words::pick(lang, "RENEWS", "التجديد"), caps, Align::Start);
    svg.text(W - MARGIN, y + 12.0, words::pick(lang, "BALANCE", "الرصيد"), caps, Align::End);
    svg.line(MARGIN, y + 20.0, W - MARGIN, y + 20.0, INK, 1.2);
    20.0
}

fn table_row(svg: &mut Svg, y: f32, account: &Account, look: &Look) -> f32 {
    let lang = look.lang;
    let h = 44.0;
    svg.tile(MARGIN, y + 10.0, 24.0, account.carrier);
    let name = font(10.5, 700, INK);
    svg.text(MARGIN + 32.0, y + 21.0, &svg.fit(&account.display_name(), name, COL_LEFT - MARGIN - 44.0), name, Align::Start);
    svg.text(MARGIN + 32.0, y + 34.0, &account.pretty_number(), font(8.5, 400, MUTED), Align::Start);
    let s = shown(account, None);
    match s.bundle {
        Some(b) => {
            let strong = font(11.0, 800, if s.low { DANGER } else { INK });
            let amount = format::amount(b.remaining, b.unit);
            svg.text(COL_LEFT, y + 21.0, &amount, strong, Align::Start);
            svg.text(COL_LEFT + svg.width(&amount, strong) + 3.0, y + 21.0, words::unit_name(b.unit, lang), font(8.5, 600, MUTED), Align::Start);
            let of = if lang == Lang::Ar { format!("من {}", format::amount_short(b.total, b.unit)) } else { format!("of {}", format::amount_short(b.total, b.unit)) };
            svg.text(COL_LEFT, y + 34.0, &of, font(8.5, 400, MUTED), Align::Start);
            svg.bar(area(COL_SHARE, y + 14.0, 60.0, 5.0), b.remaining_pct() / 100.0, if s.low { DANGER } else { YELLOW }, SURFACE);
            svg.text(COL_SHARE, y + 34.0, &format!("{:.0}%", b.remaining_pct()), font(8.5, 600, MUTED), Align::Start);
        }
        None => svg.text(COL_LEFT, y + 25.0, "—", font(11.0, 400, MUTED), Align::Start),
    }
    let usage = account.usage.as_ref();
    match usage.and_then(|u| u.renews_at) {
        Some(at) => {
            svg.text(COL_RENEWS, y + 21.0, &words::date_short(at, lang), font(10.0, 700, INK), Align::Start);
            svg.text(COL_RENEWS, y + 34.0, &words::in_days(at, look.now, lang), font(8.5, 400, YELLOW_INK), Align::Start);
        }
        None => svg.text(COL_RENEWS, y + 25.0, "—", font(10.0, 400, MUTED), Align::Start),
    }
    let balance = usage.and_then(|u| u.balance_egp).map_or_else(|| "—".to_string(), |b| format!("{} {}", format::money(b), words::unit_name(klue_core::Unit::Egp, lang)));
    svg.text(W - MARGIN, y + 25.0, &balance, font(10.0, 600, INK), Align::End);
    svg.line(MARGIN, y + h, W - MARGIN, y + h, HAIRLINE, 1.0);
    h
}

/// The main bundle, its share left, the renewal and the balance, in one strip. Returns its
/// height.
fn figures_strip(svg: &mut Svg, y: f32, account: &Account, look: &Look) -> f32 {
    let lang = look.lang;
    let h = 88.0;
    svg.frame(area(MARGIN, y, INNER, h), 14.0, CARD, HAIRLINE, 1.2);
    let pad = 16.0;
    let caps = font(8.0, 700, MUTED);
    let quiet = font(8.5, 400, MUTED);
    let usage = account.usage.as_ref();
    let s = shown(account, None);
    // Where each column starts: the main bundle takes the most room.
    let cols = [MARGIN, MARGIN + 205.0, MARGIN + 305.0, MARGIN + 405.0];
    for x in &cols[1..] {
        svg.line(*x, y + 16.0, *x, y + h - 16.0, HAIRLINE, 1.0);
    }
    match s.bundle {
        Some(b) => {
            let package = usage.and_then(|u| u.package_in(lang)).unwrap_or(b.name_in(lang)).to_string();
            svg.text(cols[0] + pad, y + 22.0, &svg.fit(&package.to_uppercase(), caps, 205.0 - pad * 2.0), caps, Align::Start);
            let big = font(25.0, 800, if s.low { DANGER } else if s.dim { MUTED } else { INK });
            let amount = format::amount(b.remaining, b.unit);
            svg.text(cols[0] + pad, y + 50.0, &amount, big, Align::Start);
            svg.text(cols[0] + pad + svg.width(&amount, big) + 5.0, y + 50.0, &words::left(b.unit, lang), font(9.5, 700, MUTED), Align::Start);
            svg.bar(area(cols[0] + pad, y + 59.0, 205.0 - pad * 2.0, 5.0), b.remaining_pct() / 100.0, if s.low { DANGER } else { YELLOW }, SURFACE);
            let used = if lang == Lang::Ar { format!("استُخدم {} {}", format::amount(b.used(), b.unit), words::unit_name(b.unit, lang)) } else { format!("{} {} used", format::amount(b.used(), b.unit), words::unit_name(b.unit, lang)) };
            svg.text(cols[0] + pad, y + 76.0, &used, quiet, Align::Start);

            svg.text(cols[1] + pad, y + 22.0, words::pick(lang, "REMAINING", "المتبقي"), caps, Align::Start);
            svg.text(cols[1] + pad, y + 50.0, &format!("{:.1}%", b.remaining_pct()), font(19.0, 800, INK), Align::Start);
            let of = if lang == Lang::Ar { format!("من {} {}", format::amount_short(b.total, b.unit), words::unit_name(b.unit, lang)) } else { format!("of {} {}", format::amount_short(b.total, b.unit), words::unit_name(b.unit, lang)) };
            svg.text(cols[1] + pad, y + 68.0, &of, quiet, Align::Start);
        }
        None => svg.text(cols[0] + pad, y + 50.0, words::pick(lang, "Waiting for the first check", "في انتظار أول تحديث"), font(11.0, 400, MUTED), Align::Start),
    }

    svg.text(cols[2] + pad, y + 22.0, words::pick(lang, "RENEWS", "التجديد"), caps, Align::Start);
    match usage.and_then(|u| u.renews_at) {
        Some(at) => {
            svg.text(cols[2] + pad, y + 48.0, &words::date_short(at, lang), font(15.0, 800, INK), Align::Start);
            svg.text(cols[2] + pad, y + 66.0, &words::in_days(at, look.now, lang), font(8.5, 700, YELLOW_INK), Align::Start);
        }
        None => svg.text(cols[2] + pad, y + 48.0, "—", font(15.0, 800, MUTED), Align::Start),
    }

    svg.text(cols[3] + pad, y + 22.0, words::pick(lang, "BALANCE", "الرصيد"), caps, Align::Start);
    match usage.and_then(|u| u.balance_egp) {
        Some(egp) => {
            let strong = font(15.0, 800, INK);
            let money = format::money(egp);
            svg.text(cols[3] + pad, y + 48.0, &money, strong, Align::Start);
            svg.text(cols[3] + pad + svg.width(&money, strong) + 4.0, y + 48.0, words::unit_name(klue_core::Unit::Egp, lang), font(8.5, 700, MUTED), Align::Start);
            if let Some(owed) = usage.and_then(|u| u.owed_egp).filter(|o| *o > 0.0) {
                let owes = if lang == Lang::Ar { format!("مستحق {}", format::money(owed)) } else { format!("{} owed", format::money(owed)) };
                svg.text(cols[3] + pad, y + 66.0, &owes, font(8.5, 700, DANGER), Align::Start);
            }
        }
        None => svg.text(cols[3] + pad, y + 48.0, "—", font(15.0, 800, MUTED), Align::Start),
    }
    h
}

/// The line's other bundles as a table. Returns its height.
fn bundle_table(svg: &mut Svg, y: f32, account: &Account, bundles: &[&klue_core::Bundle], look: &Look) -> f32 {
    let lang = look.lang;
    let row = 26.0;
    let pad = 16.0;
    let h = 40.0 + row * bundles.len() as f32 + 8.0;
    svg.frame(area(MARGIN, y, INNER, h), 14.0, CARD, HAIRLINE, 1.2);
    let caps = font(8.0, 700, MUTED);
    let (c_kind, c_left, c_share) = (MARGIN + 205.0, MARGIN + 305.0, MARGIN + 395.0);
    svg.text(MARGIN + pad, y + 22.0, words::pick(lang, "OTHER BUNDLES", "باقات أخرى"), caps, Align::Start);
    svg.text(c_kind, y + 22.0, words::pick(lang, "KIND", "النوع"), caps, Align::Start);
    svg.text(c_left, y + 22.0, words::pick(lang, "LEFT", "المتبقي"), caps, Align::Start);
    svg.text(c_share, y + 22.0, words::pick(lang, "SHARE LEFT", "النسبة"), caps, Align::Start);
    for (i, b) in bundles.iter().enumerate() {
        let ry = y + 40.0 + row * i as f32;
        svg.line(MARGIN + pad, ry, W - MARGIN - pad, ry, HAIRLINE, 1.0);
        let low = account.alert_for(b, false).is_triggered(b);
        let name = font(9.5, 700, INK);
        svg.text(MARGIN + pad, ry + 17.0, &svg.fit(b.name_in(lang), name, c_kind - MARGIN - pad - 10.0), name, Align::Start);
        let mut kind = words::kind_name(b.kind, lang).to_string();
        if let Some(state) = words::state_name(b.state, lang) {
            kind = format!("{kind} · {state}");
        }
        svg.text(c_kind, ry + 17.0, &kind, font(8.5, 400, MUTED), Align::Start);
        let strong = font(9.5, 800, if low { DANGER } else { INK });
        let left = format!("{} {}", format::amount(b.remaining, b.unit), words::unit_name(b.unit, lang));
        svg.text(c_left, ry + 17.0, &left, strong, Align::Start);
        let of = if lang == Lang::Ar { format!("من {}", format::amount_short(b.total, b.unit)) } else { format!("of {}", format::amount_short(b.total, b.unit)) };
        svg.text(c_left + svg.width(&left, strong) + 4.0, ry + 17.0, &of, font(8.0, 400, MUTED), Align::Start);
        svg.bar(area(c_share, ry + 11.0, 70.0, 5.0), b.remaining_pct() / 100.0, if low { DANGER } else { YELLOW }, SURFACE);
        svg.text(W - MARGIN - pad, ry + 17.0, &format!("{:.0}%", b.remaining_pct()), font(8.5, 700, MUTED), Align::End);
    }
    h
}

/// A bundle's two graphs in a panel: what was left and what was used each day, with the
/// figures. `main` makes them larger. Returns the height.
fn graphs(svg: &mut Svg, y: f32, u: &chart::Usage, main: bool, look: &Look) -> f32 {
    let lang = look.lang;
    let pad = 18.0;
    let (left_h, day_h) = if main { (120.0, 84.0) } else { (100.0, 0.0) };
    let figures = if main { chart::figures(u, u.bundle.unit, lang) } else { Vec::new() };
    // Too few checks for either graph: a note, and the average if there is one.
    if main && !chart::enough(u) && u.analysis.daily.is_empty() {
        let h = 62.0 + if figures.is_empty() { 0.0 } else { 18.0 };
        svg.frame(area(MARGIN, y, INNER, h), 16.0, CARD, HAIRLINE, 1.2);
        svg.text(MARGIN + pad, y + pad + 12.0, words::pick(lang, "Usage graphs", "رسوم الاستخدام"), font(12.0, 800, INK), Align::Start);
        let note = words::pick(lang, "They appear once Klue has checked this line for a day.", "تظهر بعد أن يتابع Klue هذا الخط ليوم كامل.");
        svg.text(MARGIN + pad, y + pad + 30.0, note, font(10.0, 400, MUTED), Align::Start);
        let mut fx = MARGIN + pad;
        for (text, colour) in &figures {
            let f = font(10.0, 700, colour);
            svg.text(fx, y + h - pad + 2.0, text, f, Align::Start);
            fx += svg.width(text, f) + 18.0;
        }
        return h;
    }
    let h = pad + 22.0 + left_h + if main { 30.0 + day_h } else { 0.0 } + if figures.is_empty() { 0.0 } else { 24.0 } + pad;
    svg.frame(area(MARGIN, y, INNER, h), 16.0, CARD, HAIRLINE, 1.2);
    let name = if main { words::pick(lang, "What was left", "المتبقي على مدار الوقت").to_string() } else { u.bundle.name_in(lang).to_string() };
    svg.text(MARGIN + pad, y + pad + 12.0, &name, font(12.0, 800, INK), Align::Start);
    let since = if lang == Lang::Ar { format!("منذ {}", words::date_short(u.from, lang)) } else { format!("Since {}", words::date_short(u.from, lang)) };
    svg.text(W - MARGIN - pad, y + pad + 12.0, &since, font(9.5, 400, MUTED), Align::End);
    let mut cy = y + pad + 22.0;
    chart::left_over_time(svg, area(MARGIN + pad, cy, INNER - pad * 2.0, left_h), u, lang, look.now);
    cy += left_h;
    if main {
        svg.text(MARGIN + pad, cy + 22.0, words::pick(lang, "Used each day", "الاستخدام اليومي"), font(12.0, 800, INK), Align::Start);
        cy += 30.0;
        chart::per_day(svg, area(MARGIN + pad, cy, INNER - pad * 2.0, day_h), u, 31, lang);
        cy += day_h;
    }
    if !figures.is_empty() {
        let mut fx = MARGIN + pad;
        for (text, colour) in &figures {
            let f = font(10.0, 700, colour);
            svg.text(fx, cy + 18.0, text, f, Align::Start);
            fx += svg.width(text, f) + 18.0;
        }
    }
    h
}

/// A bundle's graph of what was left, in the first or second half of a row. Returns its
/// height.
fn small_graph(svg: &mut Svg, y: f32, column: usize, u: &chart::Usage, look: &Look) -> f32 {
    let lang = look.lang;
    let gap = 12.0;
    let w = (INNER - gap) / 2.0;
    let x = MARGIN + column as f32 * (w + gap);
    let pad = 14.0;
    let chart_h = 76.0;
    let h = pad + 20.0 + chart_h + pad;
    svg.frame(area(x, y, w, h), 14.0, CARD, HAIRLINE, 1.2);
    let name = font(10.5, 800, INK);
    svg.text(x + pad, y + pad + 10.0, &svg.fit(u.bundle.name_in(lang), name, w - pad * 2.0 - 60.0), name, Align::Start);
    let left = format!("{} {}", format::amount(u.bundle.remaining, u.bundle.unit), words::unit_name(u.bundle.unit, lang));
    svg.text(x + w - pad, y + pad + 10.0, &left, font(9.5, 700, MUTED), Align::End);
    chart::left_over_time(svg, area(x + pad, y + pad + 20.0, w - pad * 2.0, chart_h), u, lang, look.now);
    h
}

/// SVG pages as one PDF, with their links.
pub(crate) fn pdf(pages: Vec<Svg>, title: &str, lang: Lang) -> Option<Vec<u8>> {
    let options = usvg::Options { fontdb: fonts::database(), ..Default::default() };
    let mut document = Document::new();
    for svg in pages {
        let (w, h) = (svg.w, svg.h);
        let links = svg.links.clone();
        let tree = usvg::Tree::from_str(&svg.finish(), &options).ok()?;
        let mut page = document.start_page_with(PageSettings::from_wh(w, h)?);
        let mut surface = page.surface();
        surface.draw_svg(&tree, Size::from_wh(w, h)?, SvgSettings::default())?;
        surface.finish();
        for link in links {
            let Some(rect) = Rect::from_xywh(link.x, link.y, link.w, link.h) else { continue };
            page.add_annotation(LinkAnnotation::new(rect, Target::Action(LinkAction::new(link.url).into())).into());
        }
        page.finish();
    }
    let language = if lang == Lang::Ar { "ar" } else { "en" };
    document.set_metadata(Metadata::new().title(title.to_string()).creator("Klue".to_string()).producer(format!("Klue {}", env!("CARGO_PKG_VERSION"))).language(language.to_string()));
    document.finish().ok()
}
