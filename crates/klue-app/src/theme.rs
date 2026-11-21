//! Colour tokens, copied from the `T` table in design/project/KlueApp.dc.html.

use klue_core::{
    Carrier,
    store::{Lang, Theme},
};

pub struct Tokens {
    pub bg: &'static str,
    pub sf: &'static str,
    pub tx: &'static str,
    pub mu: &'static str,
    pub dv: &'static str,
    pub dvs: &'static str,
    pub ac: &'static str,
    pub ac_tx: &'static str,
    pub on: &'static str,
    pub ch: &'static str,
    pub cht: &'static str,
    pub chm: &'static str,
    pub chdv: &'static str,
    pub ch_act: &'static str,
    pub ch_bar: &'static str,
}

const DVS: &str = "rgba(32,30,29,.14)";

pub const LIGHT: Tokens = Tokens {
    bg: "#f3f2f2", sf: "#e8e6e6", tx: "#201e1d", mu: "rgba(32,30,29,.64)", dv: "rgba(32,30,29,.4)", dvs: DVS,
    ac: "#ec3013", ac_tx: "#ae1800", on: "#ffffff",
    ch: "#f3f2f2", cht: "#201e1d", chm: "rgba(32,30,29,.64)", chdv: "rgba(32,30,29,.4)", ch_act: "rgba(32,30,29,.07)", ch_bar: "#ec3013",
};

pub const DARK: Tokens = Tokens {
    bg: "#151413", sf: "#24221f", tx: "#f1efee", mu: "rgba(241,239,238,.62)", dv: "rgba(241,239,238,.28)", dvs: "rgba(241,239,238,.11)",
    ac: "#ff563c", ac_tx: "#ff9783", on: "#151413",
    ch: "#0e0d0c", cht: "#f1efee", chm: "rgba(241,239,238,.6)", chdv: "rgba(241,239,238,.2)", ch_act: "rgba(255,255,255,.07)", ch_bar: "#ff563c",
};

const WE: Tokens = Tokens {
    bg: "#f5f2f8", sf: "#e9e2f1", tx: "#1e1430", mu: "rgba(30,20,48,.64)", dv: "rgba(30,20,48,.36)", dvs: "rgba(30,20,48,.13)",
    ac: "#5c2d91", ac_tx: "#5c2d91", on: "#ffffff",
    ch: "#4a2378", cht: "#ffffff", chm: "rgba(255,255,255,.72)", chdv: "rgba(255,255,255,.2)", ch_act: "rgba(255,255,255,.14)", ch_bar: "#ffffff",
};

const VODAFONE: Tokens = Tokens {
    bg: "#f7f4f4", sf: "#efe7e7", tx: "#231a1a", mu: "rgba(35,26,26,.64)", dv: "rgba(35,26,26,.36)", dvs: "rgba(35,26,26,.13)",
    ac: "#e60000", ac_tx: "#b80000", on: "#ffffff",
    ch: "#e60000", cht: "#ffffff", chm: "rgba(255,255,255,.82)", chdv: "rgba(255,255,255,.28)", ch_act: "rgba(0,0,0,.16)", ch_bar: "#ffffff",
};

const ORANGE: Tokens = Tokens {
    bg: "#0b0b0b", sf: "#1c1c1c", tx: "#ffffff", mu: "rgba(255,255,255,.62)", dv: "rgba(255,255,255,.3)", dvs: "rgba(255,255,255,.11)",
    ac: "#ff7900", ac_tx: "#ff7900", on: "#000000",
    ch: "#000000", cht: "#ffffff", chm: "rgba(255,255,255,.62)", chdv: "rgba(255,121,0,.45)", ch_act: "rgba(255,121,0,.16)", ch_bar: "#ff7900",
};

const ETISALAT: Tokens = Tokens {
    bg: "#f4f3f2", sf: "#e8e6e4", tx: "#1a1918", mu: "rgba(26,25,24,.64)", dv: "rgba(26,25,24,.38)", dvs: DVS,
    ac: "#e00800", ac_tx: "#b80700", on: "#ffffff",
    ch: "#1a1918", cht: "#ffffff", chm: "rgba(255,255,255,.66)", chdv: "rgba(255,255,255,.16)", ch_act: "rgba(255,255,255,.1)", ch_bar: "#e00800",
};

/// Tokens for a theme. "Match" follows the selected carrier (the tray widget, which
/// shows every carrier at once, falls back to Light).
pub fn tokens(theme: Theme, carrier: Carrier, tray: bool) -> &'static Tokens {
    match theme {
        Theme::Light => &LIGHT,
        Theme::Dark => &DARK,
        Theme::Match if tray => &LIGHT,
        Theme::Match => match carrier {
            Carrier::We => &WE,
            Carrier::Vodafone => &VODAFONE,
            Carrier::Orange => &ORANGE,
            Carrier::Etisalat => &ETISALAT,
        },
    }
}

pub fn font_stack(lang: Lang) -> &'static str {
    match lang {
        Lang::Ar => "\"Google Sans\", \"DM Sans\", system-ui, sans-serif",
        Lang::En => "\"DM Sans\", \"Google Sans\", system-ui, sans-serif",
    }
}

/// Inline style for the `.rs` root element: CSS variables plus the font.
pub fn root_style(t: &Tokens, lang: Lang) -> String {
    let (ls, lsh) = match lang {
        Lang::Ar => ("0", "0"),
        Lang::En => ("0.1em", "-0.015em"),
    };
    format!(
        "--bg:{bg};--sf:{sf};--tx:{tx};--mu:{mu};--dv:{dv};--dvs:{dvs};--ac:{ac};--acTx:{ac_tx};--on:{on};\
         --ch:{ch};--cht:{cht};--chm:{chm};--chdv:{chdv};--chAct:{ch_act};--chBar:{ch_bar};\
         --errBg:color-mix(in srgb, {ac} 12%, transparent);--strip:{sf};--ls:{ls};--lsh:{lsh};font-family:{font};",
        bg = t.bg, sf = t.sf, tx = t.tx, mu = t.mu, dv = t.dv, dvs = t.dvs, ac = t.ac, ac_tx = t.ac_tx, on = t.on,
        ch = t.ch, cht = t.cht, chm = t.chm, chdv = t.chdv, ch_act = t.ch_act, ch_bar = t.ch_bar, font = font_stack(lang),
    )
}

/// Swatches shown on the theme cards in Settings.
pub fn swatches(theme: Theme) -> [&'static str; 4] {
    match theme {
        Theme::Light => ["#f3f2f2", "#e8e6e6", "#201e1d", "#ec3013"],
        Theme::Dark => ["#151413", "#24221f", "#f1efee", "#ff563c"],
        Theme::Match => ["#5c2d91", "#e60000", "#ff7900", "#1a1918"],
    }
}

/// Window background, so resizing doesn't flash white.
pub fn window_rgba(theme: Theme) -> (u8, u8, u8, u8) {
    match theme {
        Theme::Dark => (0x15, 0x14, 0x13, 0xff),
        _ => (0xf3, 0xf2, 0xf2, 0xff),
    }
}
