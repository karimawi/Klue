//! Colours and widget styles.
//!
//! Light and dark use the logo's colours: #FFBE32 is the accent in both, with #1D1B1B
//! text on it, and the dark theme is built on #1D1B1B. "Match" recolours the app with
//! the selected carrier's brand (the tray popup, which shows every carrier, uses Light).

use iced::{
    Background, Border, Color, Shadow,
    border::Radius,
    widget::{button, container, scrollable, text_input},
};
use klue_core::{Carrier, store::Theme};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    /// Page background.
    pub bg: Color,
    /// Surfaces: meter tracks, fields, hovered rows.
    pub sf: Color,
    pub tx: Color,
    /// Secondary text.
    pub mu: Color,
    /// Strong dividers and outlines.
    pub dv: Color,
    /// Soft dividers.
    pub dvs: Color,
    /// Accent fill (buttons, meters, selected items).
    pub ac: Color,
    /// Accent as text on the page background (kickers, links).
    pub ac_tx: Color,
    /// Text on the accent fill.
    pub on: Color,
    /// Errors and low quota.
    pub danger: Color,
    /// Chrome (title bar and carrier rail): background, text, muted text, divider,
    /// hovered/selected item and selection bar.
    pub ch: Color,
    pub cht: Color,
    pub chm: Color,
    pub chdv: Color,
    pub ch_act: Color,
    pub ch_bar: Color,
    /// Behind the number tabs.
    pub strip: Color,
}

const fn rgb(hex: u32) -> Color {
    rgba(hex, 1.0)
}

const fn rgba(hex: u32, a: f32) -> Color {
    Color {
        r: ((hex >> 16) & 0xff) as f32 / 255.0,
        g: ((hex >> 8) & 0xff) as f32 / 255.0,
        b: (hex & 0xff) as f32 / 255.0,
        a,
    }
}

/// The logo's colours.
pub const YELLOW: Color = rgb(0xFFBE32);
pub const INK: Color = rgb(0x1D1B1B);

pub const LIGHT: Palette = Palette {
    bg: rgb(0xF5F4F1),
    sf: rgb(0xE9E7E2),
    tx: INK,
    mu: rgba(0x1D1B1B, 0.64),
    dv: rgba(0x1D1B1B, 0.34),
    dvs: rgba(0x1D1B1B, 0.12),
    ac: YELLOW,
    ac_tx: rgb(0x8A5B00),
    on: INK,
    danger: rgb(0xC8321E),
    ch: rgb(0xF5F4F1),
    cht: INK,
    chm: rgba(0x1D1B1B, 0.64),
    chdv: rgba(0x1D1B1B, 0.34),
    ch_act: rgba(0x1D1B1B, 0.07),
    ch_bar: rgb(0xEFA81C),
    strip: rgb(0xE9E7E2),
};

pub const DARK: Palette = Palette {
    bg: INK,
    sf: rgb(0x2B2828),
    tx: rgb(0xF5F1EA),
    mu: rgba(0xF5F1EA, 0.62),
    dv: rgba(0xF5F1EA, 0.24),
    dvs: rgba(0xF5F1EA, 0.10),
    ac: YELLOW,
    ac_tx: YELLOW,
    on: INK,
    danger: rgb(0xFF6B57),
    ch: rgb(0x151313),
    cht: rgb(0xF5F1EA),
    chm: rgba(0xF5F1EA, 0.6),
    chdv: rgba(0xF5F1EA, 0.16),
    ch_act: rgba(0xFFFFFF, 0.07),
    ch_bar: YELLOW,
    strip: rgb(0x242121),
};

const WE: Palette = Palette {
    bg: rgb(0xF5F2F8),
    sf: rgb(0xE9E2F1),
    tx: rgb(0x1E1430),
    mu: rgba(0x1E1430, 0.64),
    dv: rgba(0x1E1430, 0.36),
    dvs: rgba(0x1E1430, 0.13),
    ac: rgb(0x5C2D91),
    ac_tx: rgb(0x5C2D91),
    on: rgb(0xFFFFFF),
    danger: rgb(0xC21F3A),
    ch: rgb(0x4A2378),
    cht: rgb(0xFFFFFF),
    chm: rgba(0xFFFFFF, 0.72),
    chdv: rgba(0xFFFFFF, 0.2),
    ch_act: rgba(0xFFFFFF, 0.14),
    ch_bar: rgb(0xFFFFFF),
    strip: rgb(0xE9E2F1),
};

const VODAFONE: Palette = Palette {
    bg: rgb(0xF7F4F4),
    sf: rgb(0xEFE7E7),
    tx: rgb(0x231A1A),
    mu: rgba(0x231A1A, 0.64),
    dv: rgba(0x231A1A, 0.36),
    dvs: rgba(0x231A1A, 0.13),
    ac: rgb(0xE60000),
    ac_tx: rgb(0xB80000),
    on: rgb(0xFFFFFF),
    danger: rgb(0xB80000),
    ch: rgb(0xE60000),
    cht: rgb(0xFFFFFF),
    chm: rgba(0xFFFFFF, 0.82),
    chdv: rgba(0xFFFFFF, 0.28),
    ch_act: rgba(0x000000, 0.16),
    ch_bar: rgb(0xFFFFFF),
    strip: rgb(0xEFE7E7),
};

const ORANGE: Palette = Palette {
    bg: rgb(0x0B0B0B),
    sf: rgb(0x1C1C1C),
    tx: rgb(0xFFFFFF),
    mu: rgba(0xFFFFFF, 0.62),
    dv: rgba(0xFFFFFF, 0.3),
    dvs: rgba(0xFFFFFF, 0.11),
    ac: rgb(0xFF7900),
    ac_tx: rgb(0xFF7900),
    on: rgb(0x000000),
    danger: rgb(0xFF5A4E),
    ch: rgb(0x000000),
    cht: rgb(0xFFFFFF),
    chm: rgba(0xFFFFFF, 0.62),
    chdv: rgba(0xFF7900, 0.45),
    ch_act: rgba(0xFF7900, 0.16),
    ch_bar: rgb(0xFF7900),
    strip: rgb(0x1C1C1C),
};

const ETISALAT: Palette = Palette {
    bg: rgb(0xF4F3F2),
    sf: rgb(0xE8E6E4),
    tx: rgb(0x1A1918),
    mu: rgba(0x1A1918, 0.64),
    dv: rgba(0x1A1918, 0.38),
    dvs: rgba(0x1A1918, 0.13),
    ac: rgb(0xE00800),
    ac_tx: rgb(0xB80700),
    on: rgb(0xFFFFFF),
    danger: rgb(0xB80700),
    ch: rgb(0x1A1918),
    cht: rgb(0xFFFFFF),
    chm: rgba(0xFFFFFF, 0.66),
    chdv: rgba(0xFFFFFF, 0.16),
    ch_act: rgba(0xFFFFFF, 0.1),
    ch_bar: rgb(0xE00800),
    strip: rgb(0xE8E6E4),
};

pub fn palette(theme: Theme, carrier: Carrier, popup: bool) -> Palette {
    match theme {
        Theme::Light => LIGHT,
        Theme::Dark => DARK,
        Theme::Match if popup => LIGHT,
        Theme::Match => match carrier {
            Carrier::We => WE,
            Carrier::Vodafone => VODAFONE,
            Carrier::Orange => ORANGE,
            Carrier::Etisalat => ETISALAT,
        },
    }
}

/// Swatches on the theme cards in Settings.
pub fn swatches(theme: Theme) -> [Color; 4] {
    match theme {
        Theme::Light => [LIGHT.bg, LIGHT.sf, LIGHT.tx, LIGHT.ac],
        Theme::Dark => [DARK.bg, DARK.sf, DARK.tx, DARK.ac],
        Theme::Match => [WE.ac, VODAFONE.ac, ORANGE.ac, ETISALAT.tx],
    }
}

/// `color` at `alpha` times its opacity.
pub fn fade(color: Color, alpha: f32) -> Color {
    Color { a: color.a * alpha, ..color }
}

/// `a` blended `t` of the way towards `b`.
pub fn mix(a: Color, b: Color, t: f32) -> Color {
    Color { r: a.r + (b.r - a.r) * t, g: a.g + (b.g - a.g) * t, b: a.b + (b.b - a.b) * t, a: a.a + (b.a - a.a) * t }
}

// ---------------------------------------------------------------------------------------
// Containers

pub fn border(color: Color, width: f32, radius: f32) -> Border {
    Border { color, width, radius: Radius::from(radius) }
}

pub fn fill(color: Color, radius: f32) -> container::Style {
    container::Style { background: Some(Background::Color(color)), border: border(Color::TRANSPARENT, 0.0, radius), ..Default::default() }
}

pub fn outlined(background: Color, line: Color, radius: f32) -> container::Style {
    container::Style { background: Some(Background::Color(background)), border: border(line, 1.0, radius), ..Default::default() }
}

/// [`fill`] as a container style.
pub fn bg(color: Color, radius: f32) -> impl Fn(&iced::Theme) -> container::Style {
    move |_| fill(color, radius)
}

/// [`outlined`] as a container style.
pub fn bg_line(background: Color, line: Color, radius: f32) -> impl Fn(&iced::Theme) -> container::Style {
    move |_| outlined(background, line, radius)
}

// No blurred shadows anywhere: the software renderer draws a shadow over its whole extent
// on every partial redraw (it ignores the damage clip), so each blink of a text cursor
// would darken it further; and it computes them pixel by pixel. Edges are lines instead.

/// A floating panel (dialogs).
pub fn panel(p: Palette, radius: f32) -> container::Style {
    container::Style {
        background: Some(Background::Color(p.bg)),
        text_color: Some(p.tx),
        border: border(p.dv, 1.0, radius),
        ..Default::default()
    }
}

pub fn tooltip(p: Palette) -> container::Style {
    container::Style {
        background: Some(Background::Color(p.tx)),
        text_color: Some(p.bg),
        border: border(Color::TRANSPARENT, 0.0, 7.0),
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------------------
// Buttons

fn btn(background: Option<Color>, text: Color, line: Option<(Color, f32)>, radius: f32) -> button::Style {
    button::Style {
        background: background.map(Background::Color),
        text_color: text,
        border: match line {
            Some((c, w)) => border(c, w, radius),
            None => border(Color::TRANSPARENT, 0.0, radius),
        },
        shadow: Shadow::default(),
        snap: true,
    }
}

fn hovered(status: button::Status) -> bool {
    matches!(status, button::Status::Hovered | button::Status::Pressed)
}

/// Filled accent button.
pub fn primary(p: Palette) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_, status| {
        let bg = match status {
            button::Status::Disabled => fade(p.ac, 0.55),
            button::Status::Hovered => mix(p.ac, p.tx, 0.08),
            button::Status::Pressed => mix(p.ac, p.tx, 0.16),
            button::Status::Active => p.ac,
        };
        btn(Some(bg), p.on, None, 9.0)
    }
}

/// Outlined button (also the icon-only tool buttons).
pub fn outline(p: Palette, radius: f32) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_, status| {
        let text = if status == button::Status::Disabled { fade(p.tx, 0.45) } else { p.tx };
        btn(hovered(status).then_some(p.sf), text, Some((p.dv, 1.0)), radius)
    }
}

/// Text-only accent button ("Change", "Make main").
pub fn link(p: Palette) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_, status| btn(hovered(status).then_some(p.sf), p.ac_tx, None, 9.0)
}

/// Borderless button that only shows a background when hovered.
pub fn ghost(text: Color, hover: Color, radius: f32) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_, status| btn(hovered(status).then_some(hover), text, None, radius)
}

/// One option of a segmented control.
pub fn segment(on: bool, fill: Color, on_text: Color, off_text: Color, radius: f32) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_, status| {
        let text = if status == button::Status::Disabled { fade(off_text, 0.45) } else if on { on_text } else { off_text };
        btn(on.then_some(fill), text, None, radius)
    }
}

/// A small button on the title bar's colors: no chrome until pointed at.
pub fn chrome_button(p: Palette) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_, status| btn(hovered(status).then_some(p.ch_act), p.cht, None, 7.0)
}

/// Window buttons in the title bar; `close` turns red when hovered.
pub fn window_button(p: Palette, close: bool) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_, status| match (hovered(status), close) {
        (true, true) => btn(Some(rgb(0xE81123)), rgb(0xFFFFFF), None, 0.0),
        (true, false) => btn(Some(p.ch_act), p.cht, None, 0.0),
        _ => btn(None, p.cht, None, 0.0),
    }
}

/// A clickable row or card: no chrome, `hover` background when pointed at, `on`
/// background when selected.
pub fn plain(hover: Color, on: Option<Color>, radius: f32) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_, status| button::Style {
        background: on.or(hovered(status).then_some(hover)).map(Background::Color),
        text_color: Color::BLACK,
        border: border(Color::TRANSPARENT, 0.0, radius),
        shadow: Shadow::default(),
        snap: true,
    }
}

// ---------------------------------------------------------------------------------------
// Inputs

pub fn field(p: Palette, error: bool) -> impl Fn(&iced::Theme, text_input::Status) -> text_input::Style {
    move |_, status| {
        let focused = matches!(status, text_input::Status::Focused { .. });
        text_input::Style {
            background: Background::Color(p.sf),
            border: if error {
                border(p.danger, 2.0, 9.0)
            } else if focused {
                border(p.ac, 2.0, 9.0)
            } else {
                border(p.dv, 1.0, 9.0)
            },
            icon: p.mu,
            placeholder: p.mu,
            value: p.tx,
            selection: fade(p.ac, 0.45),
        }
    }
}

pub fn scroller(p: Palette) -> impl Fn(&iced::Theme, scrollable::Status) -> scrollable::Style {
    move |theme, status| {
        let mut style = scrollable::default(theme, status);
        let rail = scrollable::Rail {
            background: None,
            border: border(Color::TRANSPARENT, 0.0, 4.0),
            scroller: scrollable::Scroller { background: Background::Color(fade(p.tx, 0.22)), border: border(Color::TRANSPARENT, 0.0, 4.0) },
        };
        style.vertical_rail = rail;
        style.horizontal_rail = rail;
        style.container = container::Style::default();
        style
    }
}
