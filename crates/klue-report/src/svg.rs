//! A small SVG writer. Positions are given from the start of the line, left in English and
//! right in Arabic, and mirrored here, so each picture is laid out once for both.

use std::{
    fmt::Write as _,
    sync::atomic::{AtomicUsize, Ordering},
};

use klue_core::{Carrier, LineType};

use crate::{fonts, look};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Align {
    Start,
    Middle,
    End,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Font {
    pub size: f32,
    pub weight: u16,
    pub color: &'static str,
}

pub(crate) const fn font(size: f32, weight: u16, color: &'static str) -> Font {
    Font { size, weight, color }
}

/// A box: its corner from the line's start, and its size.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Area {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

pub(crate) const fn area(x: f32, y: f32, w: f32, h: f32) -> Area {
    Area { x, y, w, h }
}

/// A link to put on the page, in page coordinates.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Link {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub url: String,
}

pub(crate) struct Svg {
    pub w: f32,
    pub h: f32,
    pub rtl: bool,
    out: String,
    defs: String,
    pub links: Vec<Link>,
}

/// Up to two decimals, without trailing zeros.
fn n(v: f32) -> String {
    let s = format!("{:.2}", v);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

pub(crate) fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

impl Svg {
    pub fn new(w: f32, h: f32, rtl: bool) -> Self {
        Self { w, h, rtl, out: String::new(), defs: String::new(), links: Vec::new() }
    }

    /// The left edge of a box `w` wide that starts `x` from the line's start.
    pub fn bx(&self, x: f32, w: f32) -> f32 {
        if self.rtl { self.w - x - w } else { x }
    }

    /// A point `x` from the line's start.
    pub fn px(&self, x: f32) -> f32 {
        if self.rtl { self.w - x } else { x }
    }

    pub fn raw(&mut self, s: &str) {
        self.out.push_str(s);
    }

    /// An id no other picture uses, so pieces can be put in one another.
    fn id(&mut self, prefix: &str) -> String {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        format!("{prefix}{}", NEXT.fetch_add(1, Ordering::Relaxed))
    }

    /// Draws `piece`, laid out at its own size, `scale` times smaller or larger here. Its
    /// width should be this one's divided by `scale`, so mirroring lines up.
    pub fn embed(&mut self, piece: Svg, scale: f32) {
        let _ = write!(self.out, r#"<g transform="scale({})">{}</g>"#, n(scale), piece.out);
        // Clip paths are read in the space of what uses them, which is already scaled.
        self.defs.push_str(&piece.defs);
        self.links.extend(piece.links.into_iter().map(|l| Link { x: l.x * scale, y: l.y * scale, w: l.w * scale, h: l.h * scale, url: l.url }));
    }

    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, fill: &str) {
        let x = self.bx(x, w);
        let _ = write!(self.out, r#"<rect x="{}" y="{}" width="{}" height="{}" rx="{}" fill="{fill}"/>"#, n(x), n(y), n(w), n(h), n(r));
    }

    /// A rounded box with a border inside its edge.
    pub fn frame(&mut self, a: Area, r: f32, fill: &str, stroke: &str, stroke_w: f32) {
        let Area { x, y, w, h } = a;
        let x = self.bx(x, w);
        let half = stroke_w / 2.0;
        let _ = write!(
            self.out,
            r#"<rect x="{}" y="{}" width="{}" height="{}" rx="{}" fill="{fill}" stroke="{stroke}" stroke-width="{}"/>"#,
            n(x + half),
            n(y + half),
            n(w - stroke_w),
            n(h - stroke_w),
            n(r - half),
            n(stroke_w)
        );
    }

    /// A line from the line's start; `x2` past `x1` runs toward the end.
    pub fn line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, stroke: &str, width: f32) {
        let (x1, x2) = (self.px(x1), self.px(x2));
        let _ = write!(self.out, r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{stroke}" stroke-width="{}"/>"#, n(x1), n(y1), n(x2), n(y2), n(width));
    }

    /// Text at `x` from the line's start, its baseline at `y`.
    pub fn text(&mut self, x: f32, y: f32, s: &str, f: Font, align: Align) {
        let anchor = match (align, self.rtl) {
            (Align::Middle, _) => "middle",
            (Align::Start, false) | (Align::End, true) => "start",
            (Align::Start, true) | (Align::End, false) => "end",
        };
        // usvg lays every line out left to right, so Arabic lines are put in a right-to-left
        // embedding: a figure or a Latin name at their start then lands on the right.
        // Text without Arabic (a figure, a phone number, a Latin name) stays left to right.
        let content = if self.rtl && s.chars().any(fonts::is_arabic) { format!("\u{202B}{s}\u{202C}") } else { s.to_string() };
        let _ = write!(
            self.out,
            r#"<text x="{}" y="{}" font-family="{}" font-size="{}" font-weight="{}" fill="{}" text-anchor="{anchor}" xml:space="preserve">{}</text>"#,
            n(self.px(x)),
            n(y),
            fonts::family_for(s),
            n(f.size),
            f.weight,
            f.color,
            escape(&content)
        );
    }

    /// `s` shortened with an ellipsis to fit `max` wide.
    pub fn fit(&self, s: &str, f: Font, max: f32) -> String {
        if fonts::width(s, f.size, f.weight) <= max {
            return s.to_string();
        }
        let mut chars: Vec<char> = s.chars().collect();
        while !chars.is_empty() {
            chars.pop();
            let cut: String = chars.iter().collect::<String>().trim_end().to_string() + "…";
            if fonts::width(&cut, f.size, f.weight) <= max {
                return cut;
            }
        }
        "…".into()
    }

    pub fn width(&self, s: &str, f: Font) -> f32 {
        fonts::width(s, f.size, f.weight)
    }

    /// A bar `fraction` full, filling from the line's start.
    pub fn bar(&mut self, a: Area, fraction: f64, fill: &str, track: &str) {
        let Area { x, y, w, h } = a;
        self.rect(x, y, w, h, h / 2.0, track);
        let filled = (w * fraction.clamp(0.0, 1.0) as f32).max(if fraction > 0.0 { h } else { 0.0 });
        if filled > 0.0 {
            self.rect(x, y, filled, h, h / 2.0, fill);
        }
    }

    /// An embedded image (a data URI) in a box from the line's start.
    pub fn image(&mut self, x: f32, y: f32, w: f32, h: f32, href: &str) {
        let x = self.bx(x, w);
        let _ = write!(self.out, r#"<image x="{}" y="{}" width="{}" height="{}" href="{href}"/>"#, n(x), n(y), n(w), n(h));
    }

    /// A carrier's tile: its logo on its colour, with the app's faint edge.
    pub fn tile(&mut self, x: f32, y: f32, size: f32, carrier: Carrier) {
        let t = look::tile(carrier);
        let radius = size * 0.28;
        let clip = self.id("tile");
        let bx = self.bx(x, size);
        let _ = write!(self.defs, r#"<clipPath id="{clip}"><rect x="{}" y="{}" width="{}" height="{}" rx="{}"/></clipPath>"#, n(bx), n(y), n(size), n(size), n(radius));
        let logo = size * t.scale;
        let _ = write!(self.out, r#"<g clip-path="url(#{clip})">"#);
        self.rect(x, y, size, size, radius, t.background);
        self.image(x + (size - logo) / 2.0, y + (size - logo) / 2.0, logo, logo, &t.logo);
        self.out.push_str("</g>");
        self.frame(area(x, y, size, size), radius, "none", "rgba(0,0,0,0.08)", 1.0);
    }

    pub fn klue_logo(&mut self, x: f32, y: f32, size: f32) {
        self.image(x, y, size, size, &look::LOGO);
    }

    /// A line's icon, `size` square.
    pub fn line_icon(&mut self, x: f32, y: f32, size: f32, line: LineType, color: &str) {
        let x = self.bx(x, size);
        let _ = write!(
            self.out,
            r#"<g transform="translate({} {}) scale({})" fill="none" stroke="{color}" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{}</g>"#,
            n(x),
            n(y),
            n(size / 24.0),
            look::line_icon(line)
        );
    }

    /// A path given in page coordinates, drawn as is.
    pub fn path(&mut self, d: &str, fill: &str, stroke: &str, width: f32) {
        let _ = write!(self.out, r#"<path d="{d}" fill="{fill}" stroke="{stroke}" stroke-width="{}" stroke-linejoin="round" stroke-linecap="round"/>"#, n(width));
    }

    /// Remembers a link over a box from the line's start, for PDFs.
    pub fn link(&mut self, x: f32, y: f32, w: f32, h: f32, url: &str) {
        let x = self.bx(x, w);
        self.links.push(Link { x, y, w, h, url: url.to_string() });
    }

    pub fn finish(self) -> String {
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}"><defs>{}</defs>{}</svg>"#,
            n(self.w),
            n(self.h),
            n(self.w),
            n(self.h),
            self.defs,
            self.out
        )
    }
}
