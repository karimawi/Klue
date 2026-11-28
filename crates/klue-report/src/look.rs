//! Klue's light look, the carriers' tiles and the few icons images use.

use std::sync::LazyLock;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use klue_core::{Carrier, LineType};

pub(crate) const PAPER: &str = "#F5F4F1";
pub(crate) const CARD: &str = "#FFFFFF";
pub(crate) const SURFACE: &str = "#ECEAE6";
pub(crate) const INK: &str = "#1D1B1B";
/// Ink at 64% on paper.
pub(crate) const MUTED: &str = "#6B6968";
/// Ink at 34% on paper.
pub(crate) const DIVIDER: &str = "#ACAAA8";
pub(crate) const HAIRLINE: &str = "#E2DFDA";
pub(crate) const YELLOW: &str = "#FFBE32";
/// Readable on paper where yellow isn't.
pub(crate) const YELLOW_INK: &str = "#8A5B00";
pub(crate) const DANGER: &str = "#C8321E";
pub(crate) const GREY_BAR: &str = "#C9C6C1";

/// A carrier's tile: its logo on its colour.
pub(crate) struct Tile {
    pub logo: String,
    pub background: &'static str,
    /// The logo's share of the tile.
    pub scale: f32,
}

static TILES: LazyLock<[Tile; 4]> = LazyLock::new(|| {
    let tile = |svg: &[u8], background, scale| Tile { logo: data_uri(svg), background, scale };
    [
        tile(include_bytes!("../../klue-app/assets/logos/we.svg"), "#5C2D91", 36.0 / 46.0),
        tile(include_bytes!("../../klue-app/assets/logos/vf.svg"), "#E60000", 36.0 / 46.0),
        // The logo fills its tile.
        tile(include_bytes!("../../klue-app/assets/logos/orange.svg"), "#FF7900", 1.0),
        tile(include_bytes!("../../klue-app/assets/logos/eand.svg"), "#FFFFFF", 30.0 / 46.0),
    ]
});

pub(crate) fn tile(carrier: Carrier) -> &'static Tile {
    &TILES[match carrier {
        Carrier::We => 0,
        Carrier::Vodafone => 1,
        Carrier::Orange => 2,
        Carrier::Etisalat => 3,
    }]
}

/// The carrier's colour, for accents (e&'s tile is white, so it gets its red).
pub fn carrier_colour(carrier: Carrier) -> &'static str {
    match carrier {
        Carrier::We => "#5C2D91",
        Carrier::Vodafone => "#E60000",
        Carrier::Orange => "#FF7900",
        Carrier::Etisalat => "#E00800",
    }
}

pub(crate) static LOGO: LazyLock<String> = LazyLock::new(|| data_uri(include_bytes!("../../klue-app/icons/klue.svg")));

fn data_uri(svg: &[u8]) -> String {
    format!("data:image/svg+xml;base64,{}", STANDARD.encode(svg))
}

/// A line's icon (Lucide, ISC) drawn on a 24-unit grid: a phone, a router for home
/// internet, a handset for a landline.
pub(crate) fn line_icon(line: LineType) -> &'static str {
    match line {
        LineType::Mobile => r#"<rect x="5" y="2" width="14" height="20" rx="2"/><path d="M12 18h.01"/>"#,
        LineType::Landline => {
            r#"<rect width="20" height="8" x="2" y="14" rx="2"/><path d="M6.01 18H6"/><path d="M10.01 18H10"/><path d="M15 10v4"/><path d="M17.84 7.17a4 4 0 0 0-5.66 0"/><path d="M20.66 4.34a8 8 0 0 0-11.31 0"/>"#
        }
        LineType::Voice => {
            r#"<path d="M22 16.92v3a2 2 0 0 1-2.18 2 19.79 19.79 0 0 1-8.63-3.07 19.5 19.5 0 0 1-6-6 19.79 19.79 0 0 1-3.07-8.67A2 2 0 0 1 4.11 2h3a2 2 0 0 1 2 1.72 12.84 12.84 0 0 0 .7 2.81 2 2 0 0 1-.45 2.11L8.09 9.91a16 16 0 0 0 6 6l1.27-1.27a2 2 0 0 1 2.11-.45 12.84 12.84 0 0 0 2.81.7A2 2 0 0 1 22 16.92z"/>"#
        }
    }
}
