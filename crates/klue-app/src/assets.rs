//! Fonts, logos, icons and the stylesheet, embedded in the binary so Klue works offline.

use std::sync::OnceLock;

use base64::Engine;
use klue_core::Carrier;

const CSS: &str = include_str!("../assets/klue.css");

const FONTS: [(&str, &[u8], &str, &str); 4] = [
    // DM Sans and Google Sans are variable fonts from Google Fonts (SIL OFL 1.1).
    ("DM Sans", include_bytes!("../assets/fonts/dm-sans-latin.woff2"), "400 800",
     "U+0000-00FF, U+0131, U+0152-0153, U+02BB-02BC, U+02C6, U+02DA, U+02DC, U+0304, U+0308, U+0329, U+2000-206F, U+20AC, U+2122, U+2191, U+2193, U+2212, U+2215, U+FEFF, U+FFFD"),
    ("DM Sans", include_bytes!("../assets/fonts/dm-sans-latin-ext.woff2"), "400 800",
     "U+0100-02BA, U+02BD-02C5, U+02C7-02CC, U+02CE-02D7, U+02DD-02FF, U+0304, U+0308, U+0329, U+1D00-1DBF, U+1E00-1E9F, U+1EF2-1EFF, U+2020, U+20A0-20AB, U+20AD-20C0, U+2113, U+2C60-2C7F, U+A720-A7FF"),
    ("Google Sans", include_bytes!("../assets/fonts/google-sans-arabic.woff2"), "400 700",
     "U+0600-06FF, U+0750-077F, U+0870-0891, U+0897-08E1, U+08E3-08FF, U+200C-200E, U+2010-2011, U+204F, U+2E41, U+FB50-FDFF, U+FE70-FE74, U+FE76-FEFC"),
    ("Google Sans", include_bytes!("../assets/fonts/google-sans-latin.woff2"), "400 700",
     "U+0000-00FF, U+0131, U+0152-0153, U+02BB-02BC, U+02C6, U+02DA, U+02DC, U+0304, U+0308, U+0329, U+2000-206F, U+20AC, U+2122, U+2191, U+2193, U+2212, U+2215, U+FEFF, U+FFFD"),
];

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// `<style>` block injected into the webview's `<head>`.
pub fn head() -> String {
    let mut out = String::from("<style>");
    for (family, bytes, weight, range) in FONTS {
        out.push_str(&format!(
            "@font-face{{font-family:'{family}';font-style:normal;font-weight:{weight};font-display:block;\
             src:url(data:font/woff2;base64,{}) format('woff2');unicode-range:{range};}}",
            b64(bytes)
        ));
    }
    out.push_str(CSS);
    out.push_str("</style>");
    out
}

/// How a carrier's logo sits on its icon tile.
pub struct Logo {
    pub uri: &'static str,
    /// Tile background (the logo files are white artwork, except e&'s).
    pub tile: &'static str,
    /// Logo size inside the 46 px rail tile, the 24 px picker tile and the 34 px tray tile.
    pub rail: &'static str,
    pub small: &'static str,
    pub tray: &'static str,
}

pub fn logo(carrier: Carrier) -> &'static Logo {
    static LOGOS: OnceLock<[Logo; 4]> = OnceLock::new();
    let uri = |svg: &[u8]| -> &'static str { Box::leak(format!("data:image/svg+xml;base64,{}", b64(svg)).into_boxed_str()) };
    let all = LOGOS.get_or_init(|| {
        [
            Logo { uri: uri(include_bytes!("../assets/logos/we.svg")), tile: "#5c2d91", rail: "36px", small: "20px", tray: "27px" },
            Logo { uri: uri(include_bytes!("../assets/logos/vf.svg")), tile: "#e60000", rail: "36px", small: "20px", tray: "27px" },
            Logo { uri: uri(include_bytes!("../assets/logos/orange.svg")), tile: "#000", rail: "46px", small: "24px", tray: "34px" },
            Logo { uri: uri(include_bytes!("../assets/logos/eand.svg")), tile: "#ffffff", rail: "30px", small: "15px", tray: "22px" },
        ]
    });
    match carrier {
        Carrier::We => &all[0],
        Carrier::Vodafone => &all[1],
        Carrier::Orange => &all[2],
        Carrier::Etisalat => &all[3],
    }
}

/// The brand mark (a rounded accent square, as in the title bar) as RGBA pixels,
/// for the window and tray icons.
pub fn app_icon(size: u32) -> Vec<u8> {
    let (r, g, b) = (0xec_u8, 0x30_u8, 0x13_u8);
    let radius = size as f32 * 0.3;
    let mut px = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            // Distance outside the rounded rectangle, for a 1 px anti-aliased edge.
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let cx = fx.clamp(radius, size as f32 - radius);
            let cy = fy.clamp(radius, size as f32 - radius);
            let d = ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt() - radius;
            let alpha = (0.5 - d).clamp(0.0, 1.0);
            px.extend_from_slice(&[r, g, b, (alpha * 255.0) as u8]);
        }
    }
    px
}

/// Inline SVG icons from the mockup (Lucide shapes).
pub mod icons {
    pub const REFRESH: &str = r#"<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/><path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"/><path d="M8 16H3v5"/></svg>"#;
    pub const PENCIL: &str = r#"<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M17 3a2.85 2.83 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z"/></svg>"#;
    pub const BELL_15: &str = r#"<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9"/><path d="M10.3 21a1.94 1.94 0 0 0 3.4 0"/></svg>"#;
    pub const BELL_16: &str = r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9"/><path d="M10.3 21a1.94 1.94 0 0 0 3.4 0"/></svg>"#;
    pub const PLUS_16: &str = r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M5 12h14"/><path d="M12 5v14"/></svg>"#;
    pub const PLUS_16_BOLD: &str = r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round"><path d="M5 12h14"/><path d="M12 5v14"/></svg>"#;
    pub const PLUS_18: &str = r#"<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M5 12h14"/><path d="M12 5v14"/></svg>"#;
    pub const SLIDERS_18: &str = r#"<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M20 7h-9"/><path d="M14 17H5"/><circle cx="17" cy="17" r="3"/><circle cx="7" cy="7" r="3"/></svg>"#;
    pub const SLIDERS_16: &str = r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 7h-9"/><path d="M14 17H5"/><circle cx="17" cy="17" r="3"/><circle cx="7" cy="7" r="3"/></svg>"#;
    pub const MOBILE: &str = r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><rect x="5" y="2" width="14" height="20"/><path d="M12 18h.01"/></svg>"#;
    pub const LANDLINE: &str = r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round"><path d="M22 16.92v3a2 2 0 0 1-2.18 2 19.79 19.79 0 0 1-8.63-3.07 19.5 19.5 0 0 1-6-6 19.79 19.79 0 0 1-3.07-8.67A2 2 0 0 1 4.11 2h3a2 2 0 0 1 2 1.72 12.84 12.84 0 0 0 .7 2.81 2 2 0 0 1-.45 2.11L8.09 9.91a16 16 0 0 0 6 6l1.27-1.27a2 2 0 0 1 2.11-.45 12.84 12.84 0 0 0 2.81.7A2 2 0 0 1 22 16.92z"/></svg>"#;
    pub const CLOSE_16: &str = r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M18 6 6 18"/><path d="m6 6 12 12"/></svg>"#;
    pub const EYE: &str = r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M2 12s3-7 10-7 10 7 10 7-3 7-10 7-10-7-10-7Z"/><circle cx="12" cy="12" r="3"/></svg>"#;
    pub const EYE_OFF: &str = r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M2 12s3-7 10-7 10 7 10 7-3 7-10 7-10-7-10-7Z"/><circle cx="12" cy="12" r="3"/><path d="m3 3 18 18"/></svg>"#;
    pub const WARNING: &str = r#"<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3Z"/><path d="M12 9v4"/><path d="M12 17h.01"/></svg>"#;
    pub const CHECK: &str = r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6 9 17l-5-5"/></svg>"#;
    pub const WIN_MIN: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M5 12h14"/></svg>"#;
    pub const WIN_MAX: &str = r#"<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4"><rect x="3" y="3" width="18" height="18"/></svg>"#;
    pub const WIN_CLOSE: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M18 6 6 18"/><path d="m6 6 12 12"/></svg>"#;
}
