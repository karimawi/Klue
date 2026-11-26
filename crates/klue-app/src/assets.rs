//! Fonts, logos and icons, embedded so Klue works offline.

use std::sync::LazyLock;

use iced::{
    Font,
    font::{Family, Weight},
    widget::svg,
};
use klue_core::{Carrier, store::Lang};

/// DM Sans for Latin text and numbers, Google Sans for Arabic (both OFL), static instances
/// made by tools/fonts/make_static.py.
pub const FONTS: [&[u8]; 12] = [
    include_bytes!("../assets/fonts/dm-sans-latin-400.ttf"),
    include_bytes!("../assets/fonts/dm-sans-latin-600.ttf"),
    include_bytes!("../assets/fonts/dm-sans-latin-800.ttf"),
    include_bytes!("../assets/fonts/dm-sans-latin-ext-400.ttf"),
    include_bytes!("../assets/fonts/dm-sans-latin-ext-600.ttf"),
    include_bytes!("../assets/fonts/dm-sans-latin-ext-800.ttf"),
    include_bytes!("../assets/fonts/google-sans-latin-400.ttf"),
    include_bytes!("../assets/fonts/google-sans-latin-600.ttf"),
    include_bytes!("../assets/fonts/google-sans-latin-700.ttf"),
    include_bytes!("../assets/fonts/google-sans-arabic-400.ttf"),
    include_bytes!("../assets/fonts/google-sans-arabic-600.ttf"),
    include_bytes!("../assets/fonts/google-sans-arabic-700.ttf"),
];

const DM_SANS: &str = "DM Sans";
const GOOGLE_SANS: &str = "Google Sans";

/// UI text: DM Sans in English, Google Sans in Arabic.
pub fn font(lang: Lang, weight: Weight) -> Font {
    let family = if lang == Lang::Ar { GOOGLE_SANS } else { DM_SANS };
    Font { family: Family::Name(family), weight, ..Font::DEFAULT }
}

/// Figures stay in DM Sans in both languages.
pub fn numbers(weight: Weight) -> Font {
    Font { family: Family::Name(DM_SANS), weight, ..Font::DEFAULT }
}

pub const APP_ICON_PNG: &[u8] = include_bytes!("../icons/klue-256.png");
pub const TRAY_ICON_PNG: &[u8] = include_bytes!("../icons/klue-64.png");

pub static LOGO: LazyLock<svg::Handle> = LazyLock::new(|| svg::Handle::from_memory(include_bytes!("../icons/klue.svg").as_slice()));

/// How a carrier's logo sits on its icon tile.
pub struct Tile {
    pub logo: svg::Handle,
    /// Tile background (the logos are white, except e&'s).
    pub background: iced::Color,
    /// Logo size in the 46 px rail tile, the 24 px picker tile and the 34 px tray tile.
    pub rail: f32,
    pub small: f32,
    pub tray: f32,
}

static TILES: LazyLock<[Tile; 4]> = LazyLock::new(|| {
    let tile = |svg: &'static [u8], hex: u32, rail, small, tray| Tile {
        logo: svg::Handle::from_memory(svg),
        background: iced::Color::from_rgb8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8),
        rail,
        small,
        tray,
    };
    [
        tile(include_bytes!("../assets/logos/we.svg"), 0x5C2D91, 36.0, 20.0, 27.0),
        tile(include_bytes!("../assets/logos/vf.svg"), 0xE60000, 36.0, 20.0, 27.0),
        // The logo fills its tile, so the tile is Orange's orange where their corners differ.
        tile(include_bytes!("../assets/logos/orange.svg"), 0xFF7900, 46.0, 24.0, 34.0),
        tile(include_bytes!("../assets/logos/eand.svg"), 0xFFFFFF, 30.0, 15.0, 22.0),
    ]
});

pub fn tile(carrier: Carrier) -> &'static Tile {
    &TILES[match carrier {
        Carrier::We => 0,
        Carrier::Vodafone => 1,
        Carrier::Orange => 2,
        Carrier::Etisalat => 3,
    }]
}

/// Line icons (Lucide, ISC), drawn black and tinted where used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Icon {
    Refresh,
    Pencil,
    Bell,
    Plus,
    Sliders,
    Mobile,
    Landline,
    Close,
    Eye,
    EyeOff,
    Warning,
    Check,
    WinMin,
    WinMax,
    WinClose,
    Star,
    StarFilled,
    Wallet,
    Receipt,
    Clock,
    ChevronLeft,
    ChevronRight,
    Info,
    Download,
    Terminal,
    Chart,
    Link,
    Archive,
    Router,
    House,
    Wifi,
    Crown,
    Usb,
}

const fn source(icon: Icon) -> &'static str {
    match icon {
        Icon::Refresh => r##"<svg viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/><path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"/><path d="M8 16H3v5"/></svg>"##,
        Icon::Pencil => r##"<svg viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M17 3a2.85 2.83 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z"/></svg>"##,
        Icon::Bell => r##"<svg viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9"/><path d="M10.3 21a1.94 1.94 0 0 0 3.4 0"/></svg>"##,
        Icon::Plus => r##"<svg viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2.2" stroke-linecap="round"><path d="M5 12h14"/><path d="M12 5v14"/></svg>"##,
        Icon::Sliders => r##"<svg viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 7h-9"/><path d="M14 17H5"/><circle cx="17" cy="17" r="3"/><circle cx="7" cy="7" r="3"/></svg>"##,
        Icon::Mobile => r##"<svg viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round"><rect x="5" y="2" width="14" height="20" rx="2"/><path d="M12 18h.01"/></svg>"##,
        Icon::Landline => r##"<svg viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linejoin="round"><path d="M22 16.92v3a2 2 0 0 1-2.18 2 19.79 19.79 0 0 1-8.63-3.07 19.5 19.5 0 0 1-6-6 19.79 19.79 0 0 1-3.07-8.67A2 2 0 0 1 4.11 2h3a2 2 0 0 1 2 1.72 12.84 12.84 0 0 0 .7 2.81 2 2 0 0 1-.45 2.11L8.09 9.91a16 16 0 0 0 6 6l1.27-1.27a2 2 0 0 1 2.11-.45 12.84 12.84 0 0 0 2.81.7A2 2 0 0 1 22 16.92z"/></svg>"##,
        Icon::Close => r##"<svg viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round"><path d="M18 6 6 18"/><path d="m6 6 12 12"/></svg>"##,
        Icon::Eye => r##"<svg viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2"><path d="M2 12s3-7 10-7 10 7 10 7-3 7-10 7-10-7-10-7Z"/><circle cx="12" cy="12" r="3"/></svg>"##,
        Icon::EyeOff => r##"<svg viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round"><path d="M2 12s3-7 10-7 10 7 10 7-3 7-10 7-10-7-10-7Z"/><circle cx="12" cy="12" r="3"/><path d="m3 3 18 18"/></svg>"##,
        Icon::Warning => r##"<svg viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3Z"/><path d="M12 9v4"/><path d="M12 17h.01"/></svg>"##,
        Icon::Check => r##"<svg viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6 9 17l-5-5"/></svg>"##,
        Icon::WinMin => r##"<svg viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="1.6"><path d="M5 12h14"/></svg>"##,
        Icon::WinMax => r##"<svg viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="1.8"><rect x="4" y="4" width="16" height="16"/></svg>"##,
        Icon::WinClose => r##"<svg viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="1.6"><path d="M18 6 6 18"/><path d="m6 6 12 12"/></svg>"##,
        Icon::Star => r##"<svg viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2.5l2.94 5.96 6.56.95-4.75 4.63 1.12 6.54L12 17.49l-5.87 3.09 1.12-6.54L2.5 9.41l6.56-.95L12 2.5z"/></svg>"##,
        Icon::StarFilled => r##"<svg viewBox="0 0 24 24" fill="#000" stroke="#000" stroke-width="2" stroke-linejoin="round"><path d="M12 2.5l2.94 5.96 6.56.95-4.75 4.63 1.12 6.54L12 17.49l-5.87 3.09 1.12-6.54L2.5 9.41l6.56-.95L12 2.5z"/></svg>"##,
        Icon::Wallet => r##"<svg viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M19 7V4a1 1 0 0 0-1-1H5a2 2 0 0 0 0 4h15a1 1 0 0 1 1 1v4h-3a2 2 0 0 0 0 4h3a1 1 0 0 0 1-1v-2a1 1 0 0 0-1-1"/><path d="M3 5v14a2 2 0 0 0 2 2h15a1 1 0 0 0 1-1v-4"/></svg>"##,
        Icon::Receipt => r##"<svg viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 2v20l2-1 2 1 2-1 2 1 2-1 2 1 2-1 2 1V2l-2 1-2-1-2 1-2-1-2 1-2-1-2 1Z"/><path d="M16 8h-6a2 2 0 1 0 0 4h4a2 2 0 1 1 0 4H8"/><path d="M12 17.5v-11"/></svg>"##,
        Icon::ChevronLeft => r##"<svg viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="m15 18-6-6 6-6"/></svg>"##,
        Icon::ChevronRight => r##"<svg viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="m9 18 6-6-6-6"/></svg>"##,
        Icon::Info => r##"<svg viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="M12 16v-4"/><path d="M12 8h.01"/></svg>"##,
        Icon::Download => r##"<svg viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><path d="m7 10 5 5 5-5"/><path d="M12 15V3"/></svg>"##,
        Icon::Terminal => r##"<svg viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m7 11 2-2-2-2"/><path d="M11 13h4"/><rect width="18" height="18" x="3" y="3" rx="2" ry="2"/></svg>"##,
        Icon::Chart => r##"<svg viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 3v16a2 2 0 0 0 2 2h16"/><path d="m19 9-5 5-4-4-3 3"/></svg>"##,
        Icon::Link => r##"<svg viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M15 3h6v6"/><path d="M10 14 21 3"/><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/></svg>"##,
        Icon::Archive => r##"<svg viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="20" height="5" x="2" y="3" rx="1"/><path d="M4 8v11a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8"/><path d="M10 12h4"/></svg>"##,
        Icon::Router => r##"<svg viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="20" height="8" x="2" y="14" rx="2"/><path d="M6.01 18H6"/><path d="M10.01 18H10"/><path d="M15 10v4"/><path d="M17.84 7.17a4 4 0 0 0-5.66 0"/><path d="M20.66 4.34a8 8 0 0 0-11.31 0"/></svg>"##,
        Icon::House => r##"<svg viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M15 21v-8a1 1 0 0 0-1-1h-4a1 1 0 0 0-1 1v8"/><path d="M3 10a2 2 0 0 1 .709-1.528l7-5.999a2 2 0 0 1 2.582 0l7 5.999A2 2 0 0 1 21 10v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/></svg>"##,
        Icon::Wifi => r##"<svg viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 20h.01"/><path d="M2 8.82a15 15 0 0 1 20 0"/><path d="M5 12.859a10 10 0 0 1 14 0"/><path d="M8.5 16.429a5 5 0 0 1 7 0"/></svg>"##,
        Icon::Crown => r##"<svg viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M11.562 3.266a.5.5 0 0 1 .876 0L15.39 8.87a1 1 0 0 0 1.516.294L21.183 5.5a.5.5 0 0 1 .798.519l-2.834 10.246a1 1 0 0 1-.956.734H5.81a1 1 0 0 1-.957-.734L2.02 6.02a.5.5 0 0 1 .798-.519l4.276 3.664a1 1 0 0 0 1.516-.294z"/><path d="M5 21h14"/></svg>"##,
        Icon::Usb => r##"<svg viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="10" cy="7" r="1"/><circle cx="4" cy="20" r="1"/><path d="M4.7 19.3 19 5"/><path d="m21 3-3 1 2 2Z"/><path d="M9.26 7.68 5 12l2 5"/><path d="m10 14 5 2 3.5-3.5"/><path d="m18 12 1-1 1 1-1 1Z"/></svg>"##,
        Icon::Clock => r##"<svg viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="M12 6v6l4 2"/></svg>"##,
    }
}

const ALL: [Icon; 33] = [
    Icon::Refresh,
    Icon::Pencil,
    Icon::Bell,
    Icon::Plus,
    Icon::Sliders,
    Icon::Mobile,
    Icon::Landline,
    Icon::Close,
    Icon::Eye,
    Icon::EyeOff,
    Icon::Warning,
    Icon::Check,
    Icon::WinMin,
    Icon::WinMax,
    Icon::WinClose,
    Icon::Star,
    Icon::StarFilled,
    Icon::Wallet,
    Icon::Receipt,
    Icon::Clock,
    Icon::ChevronLeft,
    Icon::ChevronRight,
    Icon::Info,
    Icon::Download,
    Icon::Terminal,
    Icon::Chart,
    Icon::Link,
    Icon::Archive,
    Icon::Router,
    Icon::House,
    Icon::Wifi,
    Icon::Crown,
    Icon::Usb,
];

static HANDLES: LazyLock<Vec<(Icon, svg::Handle)>> =
    LazyLock::new(|| ALL.iter().map(|&i| (i, svg::Handle::from_memory(source(i).as_bytes()))).collect());

pub fn icon(icon: Icon) -> svg::Handle {
    HANDLES.iter().find(|(i, _)| *i == icon).map(|(_, h)| h.clone()).expect("every icon has a handle")
}
