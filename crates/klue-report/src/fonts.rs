//! The app's own fonts, so images and reports look like Klue wherever they're made.

use std::sync::{Arc, LazyLock};

use resvg::usvg::fontdb::{Database, Family, Query, Weight};

const FONTS: [&[u8]; 9] = [
    include_bytes!("../../klue-app/assets/fonts/dm-sans-latin-400.ttf"),
    include_bytes!("../../klue-app/assets/fonts/dm-sans-latin-600.ttf"),
    include_bytes!("../../klue-app/assets/fonts/dm-sans-latin-800.ttf"),
    include_bytes!("../../klue-app/assets/fonts/dm-sans-latin-ext-400.ttf"),
    include_bytes!("../../klue-app/assets/fonts/dm-sans-latin-ext-600.ttf"),
    include_bytes!("../../klue-app/assets/fonts/dm-sans-latin-ext-800.ttf"),
    // Google Sans' Latin and Arabic in one file per weight (tools/fonts/merge_for_reports.py):
    // usvg can't mix two fonts in right-to-left text, so Arabic with figures in it needs both.
    include_bytes!("../assets/fonts/google-sans-400.ttf"),
    include_bytes!("../assets/fonts/google-sans-600.ttf"),
    include_bytes!("../assets/fonts/google-sans-700.ttf"),
];

/// Latin text and figures, as in the app.
pub(crate) const DM_SANS: &str = "DM Sans";
/// Anything with Arabic in it.
pub(crate) const GOOGLE_SANS: &str = "Google Sans";

pub(crate) fn database() -> Arc<Database> {
    static DB: LazyLock<Arc<Database>> = LazyLock::new(|| {
        let mut db = Database::new();
        for font in FONTS {
            db.load_font_data(font.to_vec());
        }
        db.set_sans_serif_family(DM_SANS);
        Arc::new(db)
    });
    DB.clone()
}

pub(crate) fn is_arabic(c: char) -> bool {
    matches!(c, '\u{0600}'..='\u{06FF}' | '\u{0750}'..='\u{077F}' | '\u{08A0}'..='\u{08FF}' | '\u{FB50}'..='\u{FDFF}' | '\u{FE70}'..='\u{FEFF}')
}

/// The font a piece of text is drawn in: Google Sans if there's Arabic in it, as the app
/// draws Arabic, else DM Sans.
pub(crate) fn family_for(text: &str) -> &'static str {
    if text.chars().any(is_arabic) { GOOGLE_SANS } else { DM_SANS }
}

/// How wide `text` is at `size` and `weight`, shaped as it will be drawn.
pub(crate) fn width(text: &str, size: f32, weight: u16) -> f32 {
    let db = database();
    let family = family_for(text);
    // Google Sans stops at 700, DM Sans goes to 800.
    let weight = if family == GOOGLE_SANS { weight.min(700) } else { weight };
    let Some(id) = db.query(&Query { families: &[Family::Name(family)], weight: Weight(weight), ..Default::default() }) else { return 0.0 };
    db.with_face_data(id, |data, index| {
        let face = rustybuzz::Face::from_slice(data, index)?;
        let mut buffer = rustybuzz::UnicodeBuffer::new();
        buffer.push_str(text);
        buffer.guess_segment_properties();
        let shaped = rustybuzz::shape(&face, &[], buffer);
        let units: i32 = shaped.glyph_positions().iter().map(|p| p.x_advance).sum();
        Some(units as f32 * size / f32::from(face.units_per_em() as u16))
    })
    .flatten()
    .unwrap_or(0.0)
}
