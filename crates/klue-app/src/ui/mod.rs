//! Views, drawn from [`App`] through a [`Ctx`] (palette, language, clock, RTL helpers).

pub mod add;
pub mod chart;
pub mod console;
pub mod dashboard;
pub mod empty;
pub mod min_height;
pub mod modals;
pub mod popup;
pub mod smooth;
pub mod transfer;
pub mod window;

use chrono::{DateTime, Utc};
use iced::{
    Alignment, Color, Element, Length, Padding,
    alignment::Horizontal,
    font::Weight,
    widget::{Row, Space, button, column, container, svg, text, tooltip},
};
use klue_core::{
    Carrier, LineType,
    store::{Clock, Lang},
};

use crate::{
    app::{App, Message},
    assets::{self, Icon},
    i18n::{self, Strings},
    theme::{self, Palette},
};

pub type El<'a> = Element<'a, Message>;

/// Palette, language, clock format, strings and the time.
#[derive(Clone, Copy)]
pub struct Ctx<'a> {
    pub app: &'a App,
    pub p: Palette,
    pub lang: Lang,
    pub clock: Clock,
    pub s: &'static Strings,
    pub now: DateTime<Utc>,
}

impl<'a> Ctx<'a> {
    pub fn new(app: &'a App, carrier: Carrier, popup: bool) -> Self {
        let settings = &app.snap.data.settings;
        Self {
            app,
            p: theme::palette(settings.theme, carrier, popup),
            lang: settings.lang,
            clock: settings.clock,
            s: i18n::strings(settings.lang),
            now: app.now,
        }
    }

    pub fn rtl(&self) -> bool {
        self.lang == Lang::Ar
    }

    /// Text in the UI font.
    pub fn text(&self, content: impl text::IntoFragment<'a>, size: f32, weight: Weight, color: Color) -> text::Text<'a> {
        text(content).size(size).font(assets::font(self.lang, weight)).color(color)
    }

    pub fn body(&self, content: impl text::IntoFragment<'a>, size: f32) -> text::Text<'a> {
        self.text(content, size, Weight::Normal, self.p.tx)
    }

    pub fn semi(&self, content: impl text::IntoFragment<'a>, size: f32) -> text::Text<'a> {
        self.text(content, size, Weight::Semibold, self.p.tx)
    }

    pub fn bold(&self, content: impl text::IntoFragment<'a>, size: f32) -> text::Text<'a> {
        self.text(content, size, Weight::ExtraBold, self.p.tx)
    }

    pub fn muted(&self, content: impl text::IntoFragment<'a>, size: f32) -> text::Text<'a> {
        self.text(content, size, Weight::Normal, self.p.mu)
    }

    /// Small uppercase label.
    pub fn caps(&self, content: &str) -> text::Text<'a> {
        self.text(self.upper(content), 11.0, Weight::Semibold, self.p.mu)
    }

    /// Small uppercase label in the accent colour.
    pub fn kicker(&self, content: &str) -> text::Text<'a> {
        self.text(self.upper(content), 11.0, Weight::Semibold, self.p.ac_tx)
    }

    fn upper(&self, s: &str) -> String {
        if self.rtl() { s.to_string() } else { s.to_uppercase() }
    }

    /// Figures in DM Sans in both languages.
    pub fn num(&self, content: impl text::IntoFragment<'a>, size: f32, weight: Weight, color: Color) -> text::Text<'a> {
        text(content).size(size).font(assets::numbers(weight)).color(color)
    }

    /// Start/end of a line in reading order.
    pub fn start(&self) -> Horizontal {
        if self.rtl() { Horizontal::Right } else { Horizontal::Left }
    }

    pub fn end(&self) -> Horizontal {
        if self.rtl() { Horizontal::Left } else { Horizontal::Right }
    }

    /// A row in reading order (mirrored in Arabic).
    pub fn row(&self, mut items: Vec<El<'a>>) -> Row<'a, Message> {
        if self.rtl() {
            items.reverse();
        }
        Row::with_children(items).align_y(Alignment::Center)
    }

    /// Padding given as (top, end, bottom, start) in reading order.
    pub fn pad(&self, top: f32, end: f32, bottom: f32, start: f32) -> Padding {
        let (left, right) = if self.rtl() { (end, start) } else { (start, end) };
        Padding { top, right, bottom, left }
    }

    // -----------------------------------------------------------------------------------
    // Widgets

    pub fn icon(&self, icon: Icon, size: f32, color: Color) -> El<'a> {
        tint(svg(assets::icon(icon)).width(size).height(size), color).into()
    }

    /// Icon-only tool button with a tooltip.
    pub fn tool(&self, icon: Icon, label: &'a str, on_press: Option<Message>, spinning: bool) -> El<'a> {
        let glyph = if spinning { self.icon(Icon::Refresh, 15.0, self.p.mu) } else { self.icon(icon, 15.0, self.p.tx) };
        let b = button(container(glyph).center(Length::Fill))
            .width(36)
            .height(36)
            .padding(0)
            .style(theme::outline(self.p, 9.0))
            .on_press_maybe(on_press);
        self.tip(b, label)
    }

    pub fn tip(&self, content: impl Into<El<'a>>, label: &'a str) -> El<'a> {
        let p = self.p;
        tooltip(
            content,
            container(self.text(label, 12.0, Weight::Semibold, self.p.bg)).padding([4, 8]).style(move |_| theme::tooltip(p)),
            tooltip::Position::Bottom,
        )
        .gap(6)
        .into()
    }

    pub fn primary(&self, label: impl text::IntoFragment<'a>, on_press: Option<Message>) -> button::Button<'a, Message> {
        button(self.text(label, 14.0, Weight::ExtraBold, self.p.on))
            .padding([9, 14])
            .style(theme::primary(self.p))
            .on_press_maybe(on_press)
    }

    pub fn outline(&self, label: impl text::IntoFragment<'a>, on_press: Option<Message>) -> button::Button<'a, Message> {
        button(self.text(label, 14.0, Weight::ExtraBold, self.p.tx))
            .padding([9, 14])
            .style(theme::outline(self.p, 9.0))
            .on_press_maybe(on_press)
    }

    pub fn link(&self, label: impl text::IntoFragment<'a>, on_press: Message) -> button::Button<'a, Message> {
        button(self.text(label, 13.0, Weight::ExtraBold, self.p.ac_tx)).padding([4, 6]).style(theme::link(self.p)).on_press(on_press)
    }

    /// A switch.
    pub fn toggle(&self, on: bool, on_press: Message) -> El<'a> {
        let p = self.p;
        let knob = container(Space::new()).width(12).height(12).style(theme::bg(if on { self.p.on } else { self.p.mu }, 6.0));
        let start_side = on == self.rtl();
        let track = container(knob)
            .width(36)
            .height(20)
            .padding(2)
            .align_x(if start_side { Horizontal::Left } else { Horizontal::Right })
            .align_y(Alignment::Center)
            .style(move |_| {
                let mut s = theme::fill(if on { p.ac } else { Color::TRANSPARENT }, 10.0);
                s.border = theme::border(if on { p.ac } else { p.dv }, 2.0, 10.0);
                s
            });
        button(track).padding(0).style(theme::ghost(self.p.tx, Color::TRANSPARENT, 10.0)).on_press(on_press).into()
    }

    /// Segmented control: (label, selected, message).
    pub fn segmented(&self, options: Vec<(String, bool, Option<Message>)>, height: f32, size: f32) -> El<'a> {
        self.segmented_tips(options.into_iter().map(|(label, on, msg)| (label, on, msg, None)).collect(), height, size)
    }

    /// [`segmented`](Self::segmented) with optional tooltips. Options without a message are
    /// dimmed.
    pub fn segmented_tips(&self, options: Vec<(String, bool, Option<Message>, Option<&'a str>)>, height: f32, size: f32) -> El<'a> {
        let p = self.p;
        let buttons = options
            .into_iter()
            .map(|(label, on, msg, tip)| {
                let weight = if on { Weight::Semibold } else { Weight::Normal };
                let color = if on { p.on } else if msg.is_none() { theme::fade(p.tx, 0.45) } else { p.tx };
                let b = button(container(self.text(label, size, weight, color)).center_y(Length::Fill))
                    .padding([0, 12])
                    .height(Length::Fill)
                    .style(theme::segment(on, p.ac, p.on, p.tx, 6.0))
                    .on_press_maybe(msg);
                match tip {
                    Some(tip) => self.tip(b, tip),
                    None => b.into(),
                }
            })
            .collect();
        container(self.row(buttons).spacing(2).height(Length::Fill))
            .height(height)
            .padding(2)
            .style(theme::bg_line(Color::TRANSPARENT, p.dv, 9.0))
            .into()
    }

    /// A horizontal meter: `fraction` filled with `color` on a rounded track.
    pub fn meter(&self, fraction: f64, height: f32, color: Color) -> El<'a> {
        bar(fraction, height, color, self.p.sf, Some(self.p.dvs), self.rtl())
    }
}

/// Recolour a monochrome SVG.
pub fn tint<'a>(s: svg::Svg<'a>, color: Color) -> svg::Svg<'a> {
    s.style(move |_, _| svg::Style { color: Some(color) })
}

/// Filled bar; mirrored for right-to-left.
pub fn bar<'a>(fraction: f64, height: f32, color: Color, track: Color, outline: Option<Color>, rtl: bool) -> El<'a> {
    let filled = (fraction.clamp(0.0, 1.0) * 1000.0).round() as u16;
    let radius = height / 2.0;
    let fill_part: El<'a> = container(Space::new()).width(Length::FillPortion(filled.max(1))).height(height).style(theme::bg(color, radius)).into();
    let rest: El<'a> = Space::new().width(Length::FillPortion((1000 - filled).max(1))).into();
    let mut parts = match filled {
        0 => vec![rest],
        1000 => vec![fill_part],
        _ => vec![fill_part, rest],
    };
    if rtl {
        parts.reverse();
    }
    container(Row::with_children(parts).height(height))
        .width(Length::Fill)
        .height(height)
        .style(move |_| {
            let mut s = theme::fill(track, radius);
            if let Some(line) = outline {
                s.border = theme::border(line, 1.0, radius);
            }
            s
        })
        .into()
}

pub fn hline<'a>(color: Color, thickness: f32) -> El<'a> {
    container(Space::new()).width(Length::Fill).height(thickness).style(theme::bg(color, 0.0)).into()
}

pub fn vline<'a>(color: Color, thickness: f32) -> El<'a> {
    container(Space::new()).width(thickness).height(Length::Fill).style(theme::bg(color, 0.0)).into()
}

/// A carrier's logo on its tile.
pub fn tile<'a>(carrier: Carrier, size: f32, logo_size: f32, radius: f32, grey: bool) -> El<'a> {
    let t = assets::tile(carrier);
    let logo = svg(t.logo.clone()).width(logo_size).height(logo_size).opacity(if grey { 0.55_f32 } else { 1.0_f32 });
    let background = if grey { theme::mix(t.background, Color::from_rgb8(0x9A, 0x9A, 0x9A), 0.75) } else { t.background };
    container(logo)
        .center(size)
        .style(move |_| {
            let mut s = theme::fill(background, radius);
            s.border = theme::border(Color::from_rgba8(0, 0, 0, 0.08), 1.0, radius);
            s
        })
        .into()
}

/// A line's icon: a phone, a router for home internet, or a handset for a landline.
pub fn line_icon(line: LineType) -> Icon {
    match line {
        LineType::Mobile => Icon::Mobile,
        LineType::Landline => Icon::Router,
        LineType::Voice => Icon::Landline,
    }
}

/// Klue's logo mark.
pub fn logo<'a>(size: f32) -> El<'a> {
    svg(assets::LOGO.clone()).width(size).height(size).into()
}

/// Label over value.
///
/// Placed by the column, not `Text::align_x`: iced 0.14 draws one-line Arabic with an
/// explicit alignment shifted.
pub fn stat<'a>(ctx: &Ctx<'a>, label: &str, value: impl text::IntoFragment<'a>, align: Horizontal) -> El<'a> {
    column![ctx.muted(label.to_string(), 12.0), ctx.semi(value, 12.0)].align_x(align).spacing(1).into()
}

/// Padding from explicit sides.
pub fn pad4(top: f32, right: f32, bottom: f32, left: f32) -> Padding {
    Padding { top, right, bottom, left }
}

pub fn gap<'a>(size: f32) -> El<'a> {
    Space::new().width(size).height(size).into()
}

pub fn fill_x<'a>() -> El<'a> {
    Space::new().width(Length::Fill).into()
}
