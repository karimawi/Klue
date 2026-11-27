//! Number and name formatting shared by the clients and the UI.

use crate::{FetchError, LineType, Unit};

/// ASCII digits from `input`, including Arabic-Indic ones.
pub fn digits(input: &str) -> String {
    input
        .chars()
        .filter_map(|c| match c {
            '0'..='9' => Some(c),
            '٠'..='٩' => char::from_digit(c as u32 - '٠' as u32, 10),
            '۰'..='۹' => char::from_digit(c as u32 - '۰' as u32, 10),
            _ => None,
        })
        .collect()
}

/// A typed number in the local format carriers expect (`01xxxxxxxxx`, or `0` + area code
/// + number for landlines).
pub fn normalize(input: &str, line: LineType) -> Result<String, FetchError> {
    let fixed = line != LineType::Mobile;
    let mut d = digits(input);
    if let Some(rest) = d.strip_prefix("0020") {
        d = format!("0{rest}");
    } else if d.starts_with("20") && (d.len() == 12 || (fixed && d.len() == 11)) {
        d = format!("0{}", &d[2..]);
    } else if !d.starts_with('0') && (d.len() == 10 && d.starts_with('1') || fixed && d.len() == 9) {
        d = format!("0{d}");
    }
    let ok = match line {
        LineType::Mobile => d.len() == 11 && d.starts_with("01"),
        LineType::Landline | LineType::Voice => (9..=10).contains(&d.len()) && d.starts_with('0') && !d.starts_with("01"),
    };
    if ok { Ok(d) } else { Err(FetchError::InvalidNumber(input.trim().to_string())) }
}

/// `010 0128 5532`, `02 2358 4417`.
pub fn pretty_number(number: &str) -> String {
    let d = digits(number);
    let groups: &[usize] = match d.len() {
        11 => &[3, 4, 4],
        10 if d.starts_with("02") || d.starts_with("03") => &[2, 4, 4],
        10 => &[3, 3, 4],
        9 => &[3, 3, 3],
        _ => return number.trim().to_string(),
    };
    let mut out = String::new();
    let mut i = 0;
    for g in groups {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&d[i..i + g]);
        i += g;
    }
    out
}

/// Collapses whitespace and title-cases SHOUTING Latin words ("MOSTAFA HASSAN",
/// "Samia NAGUIB") and names written all in lowercase ("mostafa hassan"). `None` if it isn't
/// a name.
pub fn tidy_name(raw: &str) -> Option<String> {
    let collapsed = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if !collapsed.chars().any(char::is_alphabetic) || collapsed.contains('@') {
        return None;
    }
    let whispering = collapsed.chars().any(|c| c.is_ascii_lowercase()) && !collapsed.chars().any(char::is_uppercase);
    let shouting = |w: &str| {
        let letters = w.chars().filter(char::is_ascii_alphabetic).count();
        w.is_ascii() && if whispering { letters >= 1 } else { letters >= 2 && !w.chars().any(|c| c.is_ascii_lowercase()) }
    };
    // "EL-SAYED" -> "El-Sayed".
    let title = |w: &str| {
        let mut out = String::with_capacity(w.len());
        let mut start = true;
        for c in w.chars() {
            out.push(if start { c.to_ascii_uppercase() } else { c.to_ascii_lowercase() });
            start = matches!(c, '-' | '\'');
        }
        out
    };
    Some(collapsed.split(' ').map(|w| if shouting(w) { title(w) } else { w.to_string() }).collect::<Vec<_>>().join(" "))
}

/// A speed as a number, and whether it's Mbit/s. WE uses 1024 kbit/s per Mbit/s.
pub fn speed(kbps: u32) -> (String, bool) {
    if kbps >= 1024 {
        let mbps = f64::from(kbps) / 1024.0;
        let text = if (mbps - mbps.round()).abs() < 0.05 { format!("{}", mbps.round()) } else { format!("{mbps:.1}") };
        (text, true)
    } else {
        (kbps.to_string(), false)
    }
}

/// `name`, tidied, if it's actually Arabic. Carriers return English for names they
/// haven't translated.
pub fn arabic(name: &str) -> Option<String> {
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    name.chars().any(|c| matches!(c, '\u{0600}'..='\u{06FF}' | '\u{0750}'..='\u{077F}' | '\u{FB50}'..='\u{FDFF}' | '\u{FE70}'..='\u{FEFF}')).then_some(name)
}

/// Without the unit: one decimal for GB, two for money, whole numbers otherwise.
pub fn amount(v: f64, unit: Unit) -> String {
    match unit {
        Unit::Gb => grouped(v, 1),
        Unit::Egp => grouped(v, 2),
        _ => thousands(v.round() as i64),
    }
}

/// "1,685.96".
pub fn money(v: f64) -> String {
    grouped(v, 2)
}

fn grouped(v: f64, decimals: usize) -> String {
    let fixed = format!("{:.*}", decimals, v.abs());
    let (int, frac) = fixed.split_once('.').unwrap_or((&fixed, ""));
    let int: i64 = int.parse().unwrap_or(0);
    let sign = if v < 0.0 && fixed.chars().any(|c| c.is_ascii_digit() && c != '0') { "-" } else { "" };
    if frac.is_empty() { format!("{sign}{}", thousands(int)) } else { format!("{sign}{}.{frac}", thousands(int)) }
}

/// [`amount`] without a trailing `.0` ("of 140 GB").
pub fn amount_short(v: f64, unit: Unit) -> String {
    let s = amount(v, unit);
    s.strip_suffix(".0").map(str::to_string).unwrap_or(s)
}

fn thousands(n: i64) -> String {
    let s = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_amounts_in_their_unit() {
        assert_eq!(amount(992.0, Unit::Mb), "992");
        assert_eq!(amount(4748.0, Unit::Mb), "4,748");
        assert_eq!(amount(305.66, Unit::Gb), "305.7");
        assert_eq!(amount(1234.5, Unit::Gb), "1,234.5");
        assert_eq!(money(1685.963), "1,685.96");
        assert_eq!(money(0.0), "0.00");
        assert_eq!(money(-12.5), "-12.50");
        assert_eq!(amount_short(140.0, Unit::Gb), "140");
    }

    #[test]
    fn normalizes_numbers() {
        assert_eq!(normalize("010 0128 5532", LineType::Mobile).unwrap(), "01001285532");
        assert_eq!(normalize("+20 100 128 5532", LineType::Mobile).unwrap(), "01001285532");
        assert_eq!(normalize("١٠٠١٢٨٥٥٣٢", LineType::Mobile).unwrap(), "01001285532");
        assert_eq!(normalize("02-2358-4417", LineType::Landline).unwrap(), "0223584417");
        assert_eq!(normalize("+20 2 2358 4417", LineType::Landline).unwrap(), "0223584417");
        assert!(normalize("0223584417", LineType::Mobile).is_err());
        assert!(normalize("01001285532", LineType::Landline).is_err());
        // A phone line's number is written like home internet's.
        assert_eq!(normalize("223584417", LineType::Voice).unwrap(), "0223584417");
        assert!(normalize("01501234567", LineType::Voice).is_err());
    }

    #[test]
    fn pretty_numbers() {
        assert_eq!(pretty_number("01001285532"), "010 0128 5532");
        assert_eq!(pretty_number("0223584417"), "02 2358 4417");
        assert_eq!(pretty_number("0403456789"), "040 345 6789");
    }

    #[test]
    fn speeds_in_mbps_from_1024_kbps() {
        assert_eq!(speed(30720), ("30".to_string(), true));
        assert_eq!(speed(2560), ("2.5".to_string(), true));
        assert_eq!(speed(3072), ("3".to_string(), true));
        assert_eq!(speed(256), ("256".to_string(), false));
    }

    #[test]
    fn keeps_only_names_written_in_arabic() {
        assert_eq!(arabic("  فليكس   115 ").as_deref(), Some("فليكس 115"));
        assert_eq!(arabic("سرعة سوبر 3-(250 جيجابايت)").as_deref(), Some("سرعة سوبر 3-(250 جيجابايت)"));
        assert_eq!(arabic("Top-up 400 GB – 30 days"), None);
        assert_eq!(arabic(""), None);
    }

    #[test]
    fn tidies_names() {
        assert_eq!(tidy_name("  MOSTAFA   HASSAN ALI ").as_deref(), Some("Mostafa Hassan Ali"));
        assert_eq!(tidy_name("مصطفى حسن").as_deref(), Some("مصطفى حسن"));
        assert_eq!(tidy_name("0223584417"), None);
        // One shouting word in an otherwise cased name, hyphens and initials.
        assert_eq!(tidy_name("Samia Fahmy NAGUIB").as_deref(), Some("Samia Fahmy Naguib"));
        assert_eq!(tidy_name("MOSTAFA A EL-SAYED").as_deref(), Some("Mostafa A El-Sayed"));
        assert_eq!(tidy_name("Samir Al-Masri").as_deref(), Some("Samir Al-Masri"));
        // All lowercase, as Orange has some names; lowercase in a cased name stays.
        assert_eq!(tidy_name("mostafa  a el-sayed").as_deref(), Some("Mostafa A El-Sayed"));
        assert_eq!(tidy_name("Mostafa van der Berg").as_deref(), Some("Mostafa van der Berg"));
        assert_eq!(tidy_name("مصطفى hassan").as_deref(), Some("مصطفى Hassan"));
    }

    #[test]
    fn amounts() {
        assert_eq!(amount(62.44, Unit::Gb), "62.4");
        assert_eq!(amount_short(140.0, Unit::Gb), "140");
        assert_eq!(amount(1020.0, Unit::Minutes), "1,020");
    }
}
