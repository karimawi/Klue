//! What messages say, the same on every channel: a line's facts, an alert's headline and
//! detail, in the server's language. Each channel lays them out its own way.

use chrono::{DateTime, Utc};
use klue_core::{
    Bundle, Unit,
    alerts::AlertEvent,
    format,
    store::{Account, Lang},
};
use klue_report::{Look, words};

/// "Sent by Klue" in messages; "Klue" links to the site.
pub fn sent_by(lang: Lang) -> &'static str {
    words::pick(lang, "Sent by", "أُرسلت بواسطة")
}

pub struct BundleFacts {
    pub name: String,
    /// "62.4 GB".
    pub left: String,
    /// "140 GB".
    pub total: String,
    pub pct: f64,
    pub low: bool,
}

pub struct LineFacts {
    pub name: String,
    pub carrier_name: &'static str,
    pub number: String,
    pub package: Option<String>,
    pub main: Option<BundleFacts>,
    pub others: Vec<BundleFacts>,
    /// The date in full, and "in 13 days".
    pub renews: Option<(String, String)>,
    /// "214.35 EGP".
    pub balance: Option<String>,
    /// When it was last checked, "09:42".
    pub checked: Option<String>,
    pub failing: bool,
}

fn bundle(account: &Account, b: &Bundle, is_main: bool, lang: Lang) -> BundleFacts {
    BundleFacts {
        name: b.name_in(lang).to_string(),
        left: amount(b.remaining, b.unit, lang),
        total: format!("{} {}", format::amount_short(b.total, b.unit), words::unit_name(b.unit, lang)),
        pct: b.remaining_pct(),
        low: account.alert_for(b, is_main).is_triggered(b),
    }
}

pub fn amount(v: f64, unit: Unit, lang: Lang) -> String {
    format!("{} {}", format::amount(v, unit), words::unit_name(unit, lang))
}

pub fn facts(account: &Account, look: &Look) -> LineFacts {
    let lang = look.lang;
    let usage = account.usage.as_ref();
    let main = account.main_bundle();
    LineFacts {
        name: account.display_name(),
        carrier_name: words::carrier_name(account.carrier, lang),
        number: account.pretty_number(),
        package: usage.and_then(|u| u.package_in(lang)).map(String::from),
        main: main.map(|b| bundle(account, b, true, lang)),
        others: usage.map(|u| u.ordered(account.main_bundle.as_deref()).into_iter().skip(1).map(|b| bundle(account, b, false, lang)).collect()).unwrap_or_default(),
        renews: usage.and_then(|u| u.renews_at).map(|at| (words::date_long(at, lang), words::in_days(at, look.now, lang))),
        balance: usage.and_then(|u| u.balance_egp).map(|b| amount(b, Unit::Egp, lang)),
        checked: account.updated_at().map(|at| checked(at, look)),
        failing: account.last_error.is_some(),
    }
}

/// "Checked 09:42", or with the day when it wasn't today.
pub fn checked(at: DateTime<Utc>, look: &Look) -> String {
    let same_day = at.with_timezone(&chrono::Local).date_naive() == look.now.with_timezone(&chrono::Local).date_naive();
    let when = if same_day { words::time(at, look.clock, look.lang) } else { format!("{} {}", words::date_short(at, look.lang), words::time(at, look.clock, look.lang)) };
    if look.lang == Lang::Ar { format!("آخر تحديث {when}") } else { format!("Checked {when}") }
}

/// A bar of ten blocks, for chats: ▰▰▰▱▱▱▱▱▱▱.
pub fn blocks(pct: f64) -> String {
    let filled = ((pct / 10.0).round() as usize).min(10);
    format!("{}{}", "▰".repeat(filled), "▱".repeat(10 - filled))
}

pub enum Level {
    Warning,
    Reminder,
}

pub struct AlertText {
    /// "WE · Home internet".
    pub title: String,
    /// "Only 9.8 GB left on Super 140".
    pub headline: String,
    /// Under it: "Renews Thu, 10 December 2026 · in 13 days".
    pub detail: Option<String>,
    pub level: Level,
    /// The bundle that ran low, for a bar.
    pub pct: Option<f64>,
}

/// An alert, worded as the app's desktop notification is.
pub fn alert(account: &Account, event: &AlertEvent, look: &Look) -> AlertText {
    let lang = look.lang;
    let ar = lang == Lang::Ar;
    let title = format!("{} · {}", words::carrier_name(account.carrier, lang), account.display_name());
    let usage = account.usage.as_ref();
    let package = usage.and_then(|u| u.package_in(lang)).map_or_else(|| words::pick(lang, "the package", "الباقة").to_string(), str::to_string);
    let renews = usage.and_then(|u| u.renews_at).map(|at| {
        let date = words::date_long(at, lang);
        if ar { format!("تتجدد {date} · {}", words::in_days(at, look.now, lang)) } else { format!("Renews {date} · {}", words::in_days(at, look.now, lang)) }
    });
    match event {
        AlertEvent::QuotaLow { bundle, .. } => {
            let is_main = account.main_bundle().is_some_and(|m| m.key == bundle.key);
            let on = if is_main { package } else { bundle.name_in(lang).to_string() };
            let left = amount(bundle.remaining, bundle.unit, lang);
            AlertText {
                title,
                headline: if ar { format!("متبقي {left} فقط من باقة {on}") } else { format!("Only {left} left on {on}") },
                detail: renews,
                level: Level::Warning,
                pct: Some(bundle.remaining_pct()),
            }
        }
        AlertEvent::RenewalSoon { renews_at, .. } => {
            let date = words::date_long(*renews_at, lang);
            let when = words::in_days(*renews_at, look.now, lang);
            AlertText {
                title,
                headline: if ar { format!("تتجدد باقة {package} {when}") } else { format!("{package} renews {when}") },
                detail: Some(match usage.and_then(|u| u.balance_egp) {
                    Some(b) if ar => format!("{date} · الرصيد {}", amount(b, Unit::Egp, lang)),
                    Some(b) => format!("{date} · balance {}", amount(b, Unit::Egp, lang)),
                    None => date,
                }),
                level: Level::Reminder,
                pct: None,
            }
        }
        AlertEvent::RenewalToday { .. } => AlertText {
            title,
            headline: if ar { format!("تتجدد باقة {package} اليوم") } else { format!("{package} renews today") },
            detail: usage.and_then(|u| u.balance_egp).map(|b| if ar { format!("الرصيد {}", amount(b, Unit::Egp, lang)) } else { format!("Balance {}", amount(b, Unit::Egp, lang)) }),
            level: Level::Reminder,
            pct: None,
        },
    }
}

/// A report's name: "Klue daily report · Sat, 28 November 2026", or "Klue report · …".
pub fn report_title(look: &Look, daily: bool) -> String {
    let date = words::date_long(look.now, look.lang);
    match (look.lang == Lang::Ar, daily) {
        (true, true) => format!("تقرير Klue اليومي · {date}"),
        (true, false) => format!("تقرير Klue · {date}"),
        (false, true) => format!("Klue daily report · {date}"),
        (false, false) => format!("Klue report · {date}"),
    }
}

/// The report's file name.
pub fn report_file(look: &Look) -> String {
    format!("klue-report-{}.pdf", look.now.with_timezone(&chrono::Local).format("%Y-%m-%d"))
}
