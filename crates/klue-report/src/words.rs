//! What images and reports say, in English and Arabic, worded as the app words it.

use chrono::{DateTime, Datelike, Local, Timelike, Utc};
use klue_core::{
    BundleKind, BundleState, Carrier, LineType, Unit,
    store::{Clock, Lang},
};

pub(crate) fn ar(lang: Lang) -> bool {
    lang == Lang::Ar
}

/// Keeps a phone number's groups in order inside Arabic text.
pub fn ltr(s: &str) -> String {
    format!("\u{202A}{s}\u{202C}")
}

/// `(english, arabic)` in the language asked for.
pub(crate) fn pick(lang: Lang, en: &'static str, ar_: &'static str) -> &'static str {
    if ar(lang) { ar_ } else { en }
}

pub fn carrier_name(c: Carrier, lang: Lang) -> &'static str {
    match c {
        Carrier::We => pick(lang, "WE", "وي"),
        Carrier::Vodafone => pick(lang, "Vodafone", "فودافون"),
        Carrier::Orange => pick(lang, "Orange", "أورنج"),
        Carrier::Etisalat => pick(lang, "e&", "إي آند"),
    }
}

pub fn line_name(line: LineType, lang: Lang) -> &'static str {
    match line {
        LineType::Mobile => pick(lang, "Mobile", "موبايل"),
        LineType::Landline => pick(lang, "Home internet", "إنترنت منزلي"),
        LineType::Voice => pick(lang, "Landline", "خط أرضي"),
    }
}

pub fn kind_name(kind: BundleKind, lang: Lang) -> &'static str {
    match kind {
        BundleKind::Data => pick(lang, "Data", "إنترنت"),
        BundleKind::Addon => pick(lang, "Add-on", "إضافة"),
        BundleKind::Minutes => pick(lang, "Minutes", "دقائق"),
        BundleKind::Social => pick(lang, "Social data", "سوشيال"),
        BundleKind::Flex => pick(lang, "Flex", "فليكس"),
        BundleKind::Sms => pick(lang, "SMS", "رسائل"),
        BundleKind::Other => pick(lang, "Bundle", "باقة"),
    }
}

pub fn unit_name(unit: Unit, lang: Lang) -> &'static str {
    match unit {
        Unit::Gb => pick(lang, "GB", "جيجا"),
        Unit::Mb => pick(lang, "MB", "ميجا"),
        Unit::Minutes => pick(lang, "min", "دقيقة"),
        Unit::Sms => pick(lang, "SMS", "رسالة"),
        Unit::Flex => pick(lang, "Flex", "فليكس"),
        Unit::Units => pick(lang, "units", "وحدة"),
        Unit::Egp => pick(lang, "EGP", "جنيه"),
    }
}

/// "GB left" beside a figure.
pub fn left(unit: Unit, lang: Lang) -> String {
    if ar(lang) { format!("{} متبقية", unit_name(unit, lang)) } else { format!("{} left", unit_name(unit, lang)) }
}

/// "Frozen" for a bundle that isn't simply usable.
pub fn state_name(state: BundleState, lang: Lang) -> Option<&'static str> {
    match state {
        BundleState::Active => None,
        BundleState::Borrowed => Some(pick(lang, "Borrowed", "سلفة")),
        BundleState::Frozen => Some(pick(lang, "Frozen", "مجمدة")),
        BundleState::Inactive => Some(pick(lang, "Not active", "غير مفعلة")),
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Span {
    Hour,
    Day,
}

/// "13 days" / "13 يوم", with Arabic's dual and plural forms.
pub(crate) fn count(n: i64, span: Span, lang: Lang) -> String {
    if !ar(lang) {
        return match span {
            Span::Hour => format!("{n} {}", if n == 1 { "hour" } else { "hours" }),
            Span::Day => format!("{n} {}", if n == 1 { "day" } else { "days" }),
        };
    }
    let (one, two, few, many) = match span {
        Span::Hour => ("ساعة", "ساعتين", "ساعات", "ساعة"),
        Span::Day => ("يوم", "يومين", "أيام", "يوم"),
    };
    match n {
        1 => one.into(),
        2 => two.into(),
        3..=10 => format!("{n} {few}"),
        _ => format!("{n} {many}"),
    }
}

pub fn renews_in(at: DateTime<Utc>, now: DateTime<Utc>, lang: Lang) -> String {
    match (at - now).num_days().max(0) {
        0 => pick(lang, "Renews today", "يتجدد اليوم").into(),
        days if ar(lang) => format!("يتجدد خلال {}", count(days, Span::Day, lang)),
        days => format!("Renews in {}", count(days, Span::Day, lang)),
    }
}

pub fn ends_in(at: DateTime<Utc>, now: DateTime<Utc>, lang: Lang) -> String {
    match (at - now).num_days().max(0) {
        0 => pick(lang, "Ends today", "تنتهي اليوم").into(),
        days if ar(lang) => format!("تنتهي خلال {}", count(days, Span::Day, lang)),
        days => format!("Ends in {}", count(days, Span::Day, lang)),
    }
}

/// "in 13 days, 4 hrs" under a date.
pub fn in_days(at: DateTime<Utc>, now: DateTime<Utc>, lang: Lang) -> String {
    let left = at - now;
    let (days, hours) = (left.num_days().max(0), (left.num_hours() % 24).max(0));
    match (ar(lang), days) {
        (false, 0) => format!("in {}", count(hours, Span::Hour, lang)),
        (false, _) => format!("in {}", count(days, Span::Day, lang)),
        (true, 0) => format!("بعد {}", count(hours, Span::Hour, lang)),
        (true, _) => format!("بعد {}", count(days, Span::Day, lang)),
    }
}

const MONTHS_EN: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
const MONTHS_AR: [&str; 12] = ["يناير", "فبراير", "مارس", "أبريل", "مايو", "يونيو", "يوليو", "أغسطس", "سبتمبر", "أكتوبر", "نوفمبر", "ديسمبر"];
const DAYS_EN: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const DAYS_AR: [&str; 7] = ["الاثنين", "الثلاثاء", "الأربعاء", "الخميس", "الجمعة", "السبت", "الأحد"];

/// "Thu, 10 December 2026".
pub fn date_long(at: DateTime<Utc>, lang: Lang) -> String {
    let d = at.with_timezone(&Local);
    let (wd, m) = (d.weekday().num_days_from_monday() as usize, d.month0() as usize);
    if ar(lang) {
        format!("{}، {} {} {}", DAYS_AR[wd], d.day(), MONTHS_AR[m], d.year())
    } else {
        format!("{}, {} {} {}", DAYS_EN[wd], d.day(), MONTHS_EN[m], d.year())
    }
}

/// "10 Dec".
pub fn date_short(at: DateTime<Utc>, lang: Lang) -> String {
    let d = at.with_timezone(&Local);
    let m = d.month0() as usize;
    if ar(lang) { format!("{} {}", d.day(), MONTHS_AR[m]) } else { format!("{} {}", d.day(), &MONTHS_EN[m][..3]) }
}

/// "1 – 28 November 2026", or across months "28 Oct – 28 Nov 2026".
pub fn date_range(from: DateTime<Utc>, to: DateTime<Utc>, lang: Lang) -> String {
    let (a, b) = (from.with_timezone(&Local), to.with_timezone(&Local));
    let months = if ar(lang) { MONTHS_AR } else { MONTHS_EN };
    if a.month() == b.month() && a.year() == b.year() {
        format!("{} – {} {} {}", a.day(), b.day(), months[b.month0() as usize], b.year())
    } else {
        format!("{} – {} {}", date_short(from, lang), date_short(to, lang), b.year())
    }
}

/// "09:42" or "9:42 AM" (ص/م in Arabic).
pub fn time(at: DateTime<Utc>, clock: Clock, lang: Lang) -> String {
    let d = at.with_timezone(&Local);
    match clock {
        Clock::H24 => format!("{:02}:{:02}", d.hour(), d.minute()),
        Clock::H12 => {
            let (pm, h) = d.hour12();
            let mark = match (ar(lang), pm) {
                (false, false) => "AM",
                (false, true) => "PM",
                (true, false) => "ص",
                (true, true) => "م",
            };
            format!("{h}:{:02} {mark}", d.minute())
        }
    }
}

/// "28 November 2026, 09:42".
pub fn stamp(at: DateTime<Utc>, clock: Clock, lang: Lang) -> String {
    let d = at.with_timezone(&Local);
    let months = if ar(lang) { MONTHS_AR } else { MONTHS_EN };
    format!("{} {} {}، {}", d.day(), months[d.month0() as usize], d.year(), time(at, clock, lang)).replace('،', if ar(lang) { "،" } else { "," })
}

/// A line speed: "30 Mbps", "256 Kbps".
pub fn speed(kbps: u32, lang: Lang) -> String {
    let (n, mbps) = klue_core::format::speed(kbps);
    match (ar(lang), mbps) {
        (false, true) => format!("Up to {n} Mbps"),
        (false, false) => format!("Up to {n} Kbps"),
        (true, true) => format!("حتى {n} ميجابت/ث"),
        (true, false) => format!("حتى {n} كيلوبت/ث"),
    }
}

/// "Generated by Klue · Home VPS · 28 November 2026, 09:42", in pieces: what comes before
/// "Klue", then what follows it.
pub fn generated(server: Option<&str>, at: DateTime<Utc>, clock: Clock, lang: Lang) -> (&'static str, Vec<String>) {
    let mut rest: Vec<String> = server.map(String::from).into_iter().collect();
    rest.push(stamp(at, clock, lang));
    (pick(lang, "Generated by", "أُنشئ بواسطة"), rest)
}
