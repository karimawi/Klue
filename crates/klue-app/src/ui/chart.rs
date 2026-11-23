//! The usage graph: what was left at each refresh, over usage per day or per hour.
//! Time runs right to left in Arabic.

use chrono::{DateTime, Datelike, Duration, Local, Timelike, Utc};
use iced::{
    Color, Font, Pixels, Point, Rectangle, Renderer, Size, Theme,
    advanced::{
        graphics::text::Paragraph,
        text::{Alignment, LineHeight, Paragraph as _, Shaping, Wrapping},
    },
    alignment::Vertical,
    border::Radius,
    font::Weight,
    mouse,
    widget::canvas::{self, Event, Frame, Geometry, LineCap, LineDash, LineJoin, Path, Stroke, gradient::Linear},
};
use klue_core::{
    Unit, format,
    history::{Period, Point as Reading},
    store::Lang,
};

use crate::{
    app::Message,
    assets, i18n,
    theme::{self, Palette},
};

/// Graph height, axis labels included.
pub const HEIGHT: f32 = 290.0;

// Layout, top to bottom.
const CAPTION_LINE: f32 = 0.0;
const LINE: (f32, f32) = (26.0, 150.0);
const CAPTION_COLUMNS: f32 = 168.0;
const COLUMNS: (f32, f32) = (194.0, 262.0);
const AXIS: f32 = 270.0;

const LABEL: f32 = 11.0;
const DOT: f32 = 4.0;
/// Max column width.
const COLUMN_MAX: f32 = 24.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Span {
    Days,
    Hours,
}

pub struct Chart {
    pub span: Span,
    /// Readings since the last renewal, oldest first.
    pub readings: Vec<Reading>,
    /// Days or hours to draw, oldest first.
    pub periods: Vec<Period>,
    pub unit: Unit,
    pub p: Palette,
    pub lang: Lang,
}

/// Whether the pointer was over the graph, so leaving it redraws once.
#[derive(Default)]
pub struct State {
    over: bool,
}

impl canvas::Program<Message> for Chart {
    type State = State;

    fn update(&self, state: &mut State, event: &Event, bounds: Rectangle, cursor: mouse::Cursor) -> Option<canvas::Action<Message>> {
        match event {
            Event::Mouse(mouse::Event::CursorMoved { .. } | mouse::Event::CursorLeft) => {
                let over = cursor.is_over(bounds);
                let changed = over || state.over;
                state.over = over;
                changed.then(canvas::Action::request_redraw)
            }
            _ => None,
        }
    }

    fn draw(&self, _state: &State, renderer: &Renderer, _theme: &Theme, bounds: Rectangle, cursor: mouse::Cursor) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        self.paint(&mut frame, cursor.position_in(bounds));
        vec![frame.into_geometry()]
    }
}

/// A point on the line: a reading, or where the line enters at the left edge.
#[derive(Clone, Copy)]
struct Node {
    at: DateTime<Utc>,
    remaining: f64,
    /// Index into `readings`; `None` for the edge point.
    reading: Option<usize>,
    /// A long wait since the previous reading: drawn dashed.
    after_gap: bool,
}

/// Display unit and factor (a GB bundle's small amounts in MB).
#[derive(Clone, Copy)]
struct Scale {
    factor: f64,
    unit: Unit,
    /// Ticks in the display unit, lowest first.
    ticks: [f64; 8],
    count: usize,
    /// Ticks with fractions all get one decimal.
    fractions: bool,
}

impl Scale {
    fn new(lo: f64, hi: f64, unit: Unit, steps: usize) -> Self {
        let (factor, unit) = if unit == Unit::Gb && hi * 1024.0 < 999.5 { (1024.0, Unit::Mb) } else { (1.0, unit) };
        let (lo, hi) = (lo * factor, hi * factor);
        // A flat line still gets a range around it.
        let (lo, hi) = if hi - lo < 1e-9 { ((lo - hi.abs().max(1.0) * 0.1).max(0.0), hi + hi.abs().max(1.0) * 0.1) } else { (lo, hi) };
        let least = if unit == Unit::Gb { 0.1 } else { 1.0 };
        let raw = ((hi - lo) / steps as f64).max(least);
        let magnitude = 10f64.powf(raw.log10().floor());
        let step = [1.0, 2.0, 5.0, 10.0].iter().map(|m| m * magnitude).find(|s| *s >= raw - 1e-12).unwrap_or(10.0 * magnitude).max(least);
        let start = ((lo / step).floor() * step).max(0.0);
        let mut ticks = [0.0; 8];
        let mut count = 0;
        let mut v = start;
        while count < ticks.len() {
            ticks[count] = v;
            count += 1;
            if v >= hi - 1e-9 {
                break;
            }
            v += step;
        }
        Self { factor, unit, ticks, count: count.max(2), fractions: step.fract() > 1e-9 }
    }

    fn ticks(&self) -> &[f64] {
        &self.ticks[..self.count]
    }

    fn lo(&self) -> f64 {
        self.ticks[0]
    }

    fn hi(&self) -> f64 {
        self.ticks[self.count - 1].max(self.ticks[0] + 1e-9)
    }

    /// Y position of `v` (in the bundle's unit).
    fn y(&self, v: f64, (top, bottom): (f32, f32)) -> f32 {
        let f = ((v * self.factor - self.lo()) / (self.hi() - self.lo())) as f32;
        bottom - f * (bottom - top)
    }

    fn label(&self, v: f64) -> String {
        if self.fractions { format::amount(v, self.unit) } else { format::amount_short(v, self.unit) }
    }
}

/// Horizontal extent and time range.
struct Axis {
    left: f32,
    right: f32,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    rtl: bool,
}

impl Axis {
    fn x(&self, t: DateTime<Utc>) -> f32 {
        let f = (t - self.from).num_milliseconds() as f32 / (self.to - self.from).num_milliseconds().max(1) as f32;
        if self.rtl { self.right - f * (self.right - self.left) } else { self.left + f * (self.right - self.left) }
    }

    /// A period's left and right edges.
    fn span(&self, period: &Period) -> (f32, f32) {
        let (a, b) = (self.x(period.start), self.x(period.end));
        (a.min(b), a.max(b))
    }
}

impl Chart {
    /// Waits longer than this are drawn dashed.
    fn long_wait(&self) -> Duration {
        match self.span {
            Span::Days => Duration::hours(3),
            Span::Hours => Duration::minutes(90),
        }
    }

    /// The line's points, starting where it crosses the left edge if it began earlier.
    fn nodes(&self) -> Vec<Node> {
        let (Some(first), Some(last)) = (self.periods.first(), self.periods.last()) else { return Vec::new() };
        let (from, to) = (first.start, last.end);
        let long = self.long_wait();
        let i = self.readings.partition_point(|r| r.at < from);
        let mut nodes = Vec::new();
        if i > 0 && i < self.readings.len() {
            let (a, b) = (self.readings[i - 1], self.readings[i]);
            let f = (from - a.at).num_seconds() as f64 / (b.at - a.at).num_seconds().max(1) as f64;
            nodes.push(Node { at: from, remaining: a.remaining + (b.remaining - a.remaining) * f, reading: None, after_gap: false });
        }
        for (j, r) in self.readings.iter().enumerate().skip(i) {
            if r.at > to {
                break;
            }
            let after_gap = j > 0 && r.at - self.readings[j - 1].at > long;
            nodes.push(Node { at: r.at, remaining: r.remaining, reading: Some(j), after_gap });
        }
        nodes
    }

    /// Whether any part is spread over a long wait (for the note).
    pub fn has_gaps(&self) -> bool {
        self.periods.iter().any(Period::rough) || self.nodes().iter().any(|n| n.after_gap)
    }

    fn paint(&self, frame: &mut Frame, hover: Option<Point>) {
        let p = self.p;
        let rtl = self.lang == Lang::Ar;
        let nodes = self.nodes();
        let (Some(first), Some(last)) = (self.periods.first(), self.periods.last()) else { return };
        if nodes.is_empty() {
            return;
        }
        let mark = mark_color(p);
        let size = frame.size();

        let (lo, hi) = nodes.iter().fold((f64::MAX, f64::MIN), |(lo, hi), n| (lo.min(n.remaining), hi.max(n.remaining)));
        let left = Scale::new(lo, hi, self.unit, 3);
        let most = self.periods.iter().map(|p| p.used).fold(0.0, f64::max);
        let used = Scale::new(0.0, most, self.unit, 2);

        // Value labels go in a gutter on the start side.
        let numbers = assets::numbers(Weight::Normal);
        let gutter = left.ticks().iter().map(|v| left.label(*v)).chain(used.ticks().iter().map(|v| used.label(*v))).map(|l| measure(&l, LABEL, numbers).width).fold(0.0, f32::max) + 10.0;
        let axis = if rtl {
            Axis { left: 4.0, right: size.width - gutter, from: first.start, to: last.end, rtl }
        } else {
            Axis { left: gutter, right: size.width - 4.0, from: first.start, to: last.end, rtl }
        };
        let width = axis.right - axis.left;
        if width < 40.0 {
            return;
        }

        let per = if self.span == Span::Days { i18n::strings(self.lang).chart_per_day } else { i18n::strings(self.lang).chart_per_hour };
        self.caption(frame, &axis, CAPTION_LINE, i18n::strings(self.lang).chart_left, left.unit);
        self.caption(frame, &axis, CAPTION_COLUMNS, per, used.unit);
        for (scale, panel, columns) in [(&left, LINE, false), (&used, COLUMNS, true)] {
            for (i, v) in scale.ticks().iter().enumerate() {
                let y = panel.1 - (v - scale.lo()) as f32 / (scale.hi() - scale.lo()) as f32 * (panel.1 - panel.0);
                // Stronger baseline for the columns.
                let color = if columns && i == 0 { p.dv } else { p.dvs };
                frame.fill_rectangle(Point::new(axis.left, y - 0.5), Size::new(width, 1.0), color);
                let label = scale.label(*v);
                let w = measure(&label, LABEL, numbers).width;
                let x = if rtl { axis.right + 8.0 } else { axis.left - 8.0 - w };
                text(frame, label, Point::new(x, y - LABEL * 0.62), LABEL, numbers, p.mu);
            }
        }

        // Midnight lines in the hours view.
        if self.span == Span::Hours {
            for period in self.periods.iter().skip(1).filter(|q| q.start.with_timezone(&Local).hour() == 0) {
                let x = axis.x(period.start);
                frame.fill_rectangle(Point::new(x - 0.5, LINE.0), Size::new(1.0, COLUMNS.1 - LINE.0), p.dvs);
            }
        }

        // Hovered reading (upper panel) or period (lower panel).
        let hovered = hover.filter(|h| h.x >= axis.left - 12.0 && h.x <= axis.right + 12.0 && h.y <= AXIS + 18.0);
        let split = (LINE.1 + COLUMNS.0) / 2.0;
        let hovered_node = hovered.filter(|h| h.y < split).and_then(|h| {
            nodes.iter().filter(|n| n.reading.is_some()).min_by(|a, b| (axis.x(a.at) - h.x).abs().total_cmp(&(axis.x(b.at) - h.x).abs())).copied()
        });
        let hovered_period = hovered.filter(|h| h.y >= split).and_then(|h| {
            self.periods.iter().enumerate().find(|(_, q)| {
                let (l, r) = axis.span(q);
                h.x >= l && h.x < r
            })
        });

        // Highlight behind the hovered column.
        if let Some((_, period)) = hovered_period {
            let (l, r) = axis.span(period);
            frame.fill(&Path::rounded_rectangle(Point::new(l + 1.0, COLUMNS.0 - 6.0), Size::new((r - l - 2.0).max(1.0), COLUMNS.1 - COLUMNS.0 + 6.0), Radius::from(5.0)), theme::fade(p.tx, 0.06));
        }

        // Line over a fading wash.
        let at = |n: &Node| Point::new(axis.x(n.at), left.y(n.remaining, LINE));
        let area = Path::new(|b| {
            b.move_to(Point::new(axis.x(nodes[0].at), LINE.1));
            for n in &nodes {
                b.line_to(at(n));
            }
            b.line_to(Point::new(axis.x(nodes[nodes.len() - 1].at), LINE.1));
            b.close();
        });
        frame.fill(&area, Linear::new(Point::new(0.0, LINE.0), Point::new(0.0, LINE.1)).add_stop(0.0, theme::fade(mark, 0.24)).add_stop(1.0, theme::fade(mark, 0.02)));
        let solid = Path::new(|b| {
            let mut down = false;
            for w in nodes.windows(2) {
                if w[1].after_gap {
                    down = false;
                    continue;
                }
                if !down {
                    b.move_to(at(&w[0]));
                    down = true;
                }
                b.line_to(at(&w[1]));
            }
        });
        let dashed = Path::new(|b| {
            for w in nodes.windows(2).filter(|w| w[1].after_gap) {
                b.move_to(at(&w[0]));
                b.line_to(at(&w[1]));
            }
        });
        let stroke = |color: Color| Stroke::default().with_color(color).with_width(2.0).with_line_cap(LineCap::Round).with_line_join(LineJoin::Round);
        frame.stroke(&solid, stroke(mark));
        frame.stroke(&dashed, Stroke { line_dash: LineDash { segments: &[2.0, 5.0], offset: 0 }, ..stroke(theme::fade(mark, 0.7)) });

        // A dot per reading, smaller when crowded; only the latest when they'd overlap.
        let dots: Vec<&Node> = nodes.iter().filter(|n| n.reading.is_some()).collect();
        let room = width / dots.len().max(1) as f32;
        for (i, n) in dots.iter().enumerate() {
            if i + 1 == dots.len() || room >= 18.0 {
                dot(frame, at(n), DOT, mark, p.bg);
            } else if room >= 6.0 {
                frame.fill(&Path::circle(at(n), 3.5), p.bg);
                frame.fill(&Path::circle(at(n), 2.25), mark);
            }
        }

        // Columns spread over a long wait are fainter.
        for period in &self.periods {
            let (l, r) = axis.span(period);
            let slot = r - l;
            let w = (slot - (slot * 0.3).max(2.0)).clamp(1.5, COLUMN_MAX);
            let h = (period.used * used.factor - used.lo()) as f32 / (used.hi() - used.lo()) as f32 * (COLUMNS.1 - COLUMNS.0);
            if h < 0.5 {
                continue;
            }
            let color = if period.rough() { theme::fade(mark, 0.42) } else { mark };
            let top = Radius { top_left: 4.0, top_right: 4.0, bottom_right: 0.0, bottom_left: 0.0 };
            frame.fill(&Path::rounded_rectangle(Point::new(l + (slot - w) / 2.0, COLUMNS.1 - h), Size::new(w, h), top), color);
        }

        self.time_labels(frame, &axis);

        // Hovered reading: crosshair, dot, tooltip.
        if let Some(n) = hovered_node
            && let Some(j) = n.reading
        {
            let point = at(&n);
            frame.fill_rectangle(Point::new(point.x - 0.5, LINE.0), Size::new(1.0, LINE.1 - LINE.0), p.dv);
            dot(frame, point, DOT + 1.5, mark, p.bg);
            let reading = self.readings[j];
            let mut lines = vec![
                (i18n::moment(reading.at, self.lang), 12.0, Weight::Normal, false),
                (i18n::left_of(reading.remaining, self.unit, self.lang), 14.0, Weight::ExtraBold, true),
            ];
            if let Some(before) = j.checked_sub(1).map(|k| self.readings[k]) {
                lines.push((i18n::used_since((reading.used - before.used).max(0.0), self.unit, before.at, self.lang), 12.0, Weight::Normal, false));
            }
            self.tooltip(frame, point, &lines);
        }

        // Hovered period tooltip.
        if let Some((i, period)) = hovered_period {
            let (l, r) = axis.span(period);
            let h = (period.used * used.factor - used.lo()) as f32 / (used.hi() - used.lo()) as f32 * (COLUMNS.1 - COLUMNS.0);
            let when = match self.span {
                Span::Days => i18n::day_label(period.start, self.lang),
                Span::Hours => i18n::hour_span(period.start, period.end, self.lang),
            };
            let mut lines = vec![(when, 12.0, Weight::Normal, false), (i18n::used_of(period.used, self.unit, self.lang), 14.0, Weight::ExtraBold, true)];
            let first_at = self.readings.first().map(|r| r.at);
            let last_at = self.readings.last().map(|r| r.at);
            if let Some(gap) = period.gap {
                lines.push((i18n::spread_over(gap, self.lang), 12.0, Weight::Normal, false));
            } else if i + 1 == self.periods.len()
                && let Some(t) = last_at.filter(|t| *t < period.end)
            {
                lines.push((i18n::until_clock(t, self.lang), 12.0, Weight::Normal, false));
            } else if let Some(t) = first_at.filter(|t| *t > period.start && *t < period.end) {
                lines.push((i18n::from_clock(t, self.lang), 12.0, Weight::Normal, false));
            }
            self.tooltip(frame, Point::new((l + r) / 2.0, COLUMNS.1 - h.max(0.0)), &lines);
        }
    }

    /// Panel caption ("WHAT WAS LEFT · GB").
    fn caption(&self, frame: &mut Frame, axis: &Axis, y: f32, title: &str, unit: Unit) {
        let content = format!("{} · {}", title, i18n::unit_name(unit, self.lang));
        let content = if self.lang == Lang::Ar { content } else { content.to_uppercase() };
        let font = assets::font(self.lang, Weight::Semibold);
        let w = measure(&content, LABEL, font).width;
        let x = if axis.rtl { axis.right - w } else { axis.left };
        text(frame, content, Point::new(x, y), LABEL, font, self.p.mu);
    }

    /// Labels under the columns, skipping some so they don't touch; the latest is always
    /// shown. Dates show the month on the first label and on the 1st.
    fn time_labels(&self, frame: &mut Frame, axis: &Axis) {
        let n = self.periods.len();
        if n == 0 {
            return;
        }
        let slot = (axis.right - axis.left) / n as f32;
        let midnight = |q: &Period| q.start.with_timezone(&Local).hour() == 0;
        let full = |q: &Period| match self.span {
            Span::Hours if !midnight(q) => i18n::clock(q.start),
            _ => i18n::axis_day(q.start, true, self.lang),
        };
        let font_for = |s: &str| if s.is_ascii() { assets::numbers(Weight::Normal) } else { assets::font(self.lang, Weight::Normal) };
        let widest = self.periods.iter().map(|q| { let s = full(q); measure(&s, LABEL, font_for(&s)).width }).fold(0.0, f32::max);
        let every = ((widest + 12.0) / slot.max(1.0)).ceil().max(1.0) as usize;
        let every = match self.span {
            Span::Hours => [1, 2, 3, 4, 6, 12, 24].into_iter().find(|k| *k >= every).unwrap_or(24),
            Span::Days => every,
        };
        let mut first = true;
        for (i, period) in self.periods.iter().enumerate() {
            let shown = match self.span {
                Span::Hours => (period.start.with_timezone(&Local).hour() as usize).is_multiple_of(every),
                Span::Days => (n - 1 - i).is_multiple_of(every),
            };
            if !shown {
                continue;
            }
            let content = match self.span {
                Span::Days => i18n::axis_day(period.start, first || period.start.with_timezone(&Local).day() == 1, self.lang),
                Span::Hours => full(period),
            };
            first = false;
            let font = font_for(&content);
            let w = measure(&content, LABEL, font).width;
            let (l, r) = axis.span(period);
            let x = ((l + r) / 2.0 - w / 2.0).clamp(0.0, (frame.size().width - w).max(0.0));
            text(frame, content, Point::new(x, AXIS), LABEL, font, self.p.mu);
        }
    }

    /// Tooltip above `anchor` if there's room, else below.
    fn tooltip(&self, frame: &mut Frame, anchor: Point, lines: &[(String, f32, Weight, bool)]) {
        let (pad_x, pad_y, gap) = (10.0, 8.0, 2.0);
        let fonts: Vec<Font> = lines.iter().map(|(_, _, w, _)| assets::font(self.lang, *w)).collect();
        let sizes: Vec<Size> = lines.iter().zip(&fonts).map(|((t, s, ..), f)| measure(t, *s, *f)).collect();
        let w = sizes.iter().map(|s| s.width).fold(0.0, f32::max) + pad_x * 2.0;
        let h = sizes.iter().map(|s| s.height).sum::<f32>() + gap * (lines.len().saturating_sub(1)) as f32 + pad_y * 2.0;
        let bounds = frame.size();
        let above = anchor.y - 14.0 - h;
        let y = if above >= 2.0 { above } else { (anchor.y + 14.0).min(bounds.height - h - 2.0) };
        let x = (anchor.x - w / 2.0).clamp(2.0, (bounds.width - w - 2.0).max(2.0));
        frame.fill(&Path::rounded_rectangle(Point::new(x, y), Size::new(w, h), Radius::from(7.0)), self.p.tx);
        let mut ty = y + pad_y;
        for (((content, size, _, strong), font), measured) in lines.iter().zip(fonts).zip(&sizes) {
            let tx = if self.lang == Lang::Ar { x + w - pad_x - measured.width } else { x + pad_x };
            let color = if *strong { self.p.bg } else { theme::fade(self.p.bg, 0.72) };
            text(frame, content.clone(), Point::new(tx, ty), *size, font, color);
            ty += measured.height + gap;
        }
    }
}

/// A dot with a ring in the page colour.
fn dot(frame: &mut Frame, at: Point, radius: f32, color: Color, ring: Color) {
    frame.fill(&Path::circle(at, radius + 2.0), ring);
    frame.fill(&Path::circle(at, radius), color);
}

/// The accent, or a darker step of it if it's under 3:1 against the background (the
/// light theme's yellow).
fn mark_color(p: Palette) -> Color {
    (0..=10).map(|i| theme::mix(p.ac, p.ac_tx, i as f32 / 10.0)).find(|c| contrast(*c, p.bg) >= 3.0).unwrap_or(p.ac_tx)
}

/// WCAG contrast ratio of two opaque colours.
fn contrast(a: Color, b: Color) -> f32 {
    let lum = |c: Color| {
        let f = |v: f32| if v <= 0.039_28 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
        0.2126 * f(c.r) + 0.7152 * f(c.g) + 0.0722 * f(c.b)
    };
    let (x, y) = (lum(a), lum(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

fn text(frame: &mut Frame, content: String, position: Point, size: f32, font: Font, color: Color) {
    // A finite width: RTL text is laid out from the right of its box, and an infinite box
    // crashes tiny-skia.
    let width = measure(&content, size, font).width.ceil() + 1.0;
    frame.fill_text(canvas::Text {
        content,
        position,
        max_width: width,
        color,
        size: Pixels(size),
        line_height: LineHeight::Relative(1.2),
        font,
        align_x: Alignment::Left,
        align_y: Vertical::Top,
        shaping: Shaping::Advanced,
    });
}

/// Size of `content` on one line.
fn measure(content: &str, size: f32, font: Font) -> Size {
    Paragraph::with_text(iced::advanced::Text {
        content,
        bounds: Size::INFINITE,
        size: Pixels(size),
        line_height: LineHeight::Relative(1.2),
        font,
        align_x: Alignment::Left,
        align_y: Vertical::Top,
        shaping: Shaping::Advanced,
        wrapping: Wrapping::None,
    })
    .min_bounds()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scales_have_round_ticks_covering_the_values() {
        let s = Scale::new(86.2, 89.7, Unit::Gb, 3);
        assert_eq!(s.ticks(), [86.0, 88.0, 90.0]);
        assert_eq!(s.label(88.0), "88");
        // One axis, one precision.
        let s = Scale::new(3.1, 3.55, Unit::Gb, 3);
        assert_eq!(s.ticks().iter().map(|v| s.label(*v)).collect::<Vec<_>>(), ["3.0", "3.2", "3.4", "3.6"]);
        // A GB bundle's small amounts are shown in MB.
        let s = Scale::new(0.0, 0.3, Unit::Gb, 2);
        assert_eq!((s.factor, s.unit), (1024.0, Unit::Mb));
        assert_eq!(s.ticks(), [0.0, 200.0, 400.0]);
        // Nothing used yet still gives a scale.
        let s = Scale::new(0.0, 0.0, Unit::Minutes, 2);
        assert!(s.ticks().len() >= 2 && s.hi() > s.lo());
        // A flat line too, without going below zero.
        let s = Scale::new(5.0, 5.0, Unit::Gb, 3);
        assert!(s.lo() <= 5.0 && s.hi() >= 5.0 && s.lo() >= 0.0, "{:?}", s.ticks());
    }

    #[test]
    fn marks_stand_out_from_every_page() {
        for p in [theme::LIGHT, theme::DARK] {
            assert!(contrast(mark_color(p), p.bg) >= 3.0);
        }
        // The light theme's yellow is too pale; a darker step is used.
        assert_ne!(mark_color(theme::LIGHT), theme::LIGHT.ac);
        assert_eq!(mark_color(theme::DARK), theme::DARK.ac);
    }
}
