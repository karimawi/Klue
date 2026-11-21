//! Phone number and name normalisation shared by the clients and the UI.

use crate::{FetchError, LineType, Unit};

/// ASCII digits from `input`, converting Arabic-Indic digits (٠–٩, ۰–۹) as well.
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

/// Normalise a user-typed number to the local format the carriers expect
/// (`01xxxxxxxxx` for mobiles, `0` + area code + number for landlines).
pub fn normalize(input: &str, line: LineType) -> Result<String, FetchError> {
    let mut d = digits(input);
    if let Some(rest) = d.strip_prefix("0020") {
        d = format!("0{rest}");
    } else if d.starts_with("20") && (d.len() == 12 || (line == LineType::Landline && d.len() == 11)) {
        d = format!("0{}", &d[2..]);
    } else if !d.starts_with('0') && (d.len() == 10 && d.starts_with('1') || line == LineType::Landline && d.len() == 9) {
        d = format!("0{d}");
    }
    let ok = match line {
        LineType::Mobile => d.len() == 11 && d.starts_with("01"),
        LineType::Landline => (9..=10).contains(&d.len()) && d.starts_with('0') && !d.starts_with("01"),
    };
    if ok { Ok(d) } else { Err(FetchError::InvalidNumber(input.trim().to_string())) }
}

/// Group a normalised number for display: `010 0128 5532`, `02 2358 4417`.
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

/// Clean up a registrant name from a carrier: collapse whitespace and turn SHOUTING
/// Latin names into Title Case. Returns `None` for values that aren't names.
pub fn tidy_name(raw: &str) -> Option<String> {
    let collapsed = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if !collapsed.chars().any(char::is_alphabetic) || collapsed.contains('@') {
        return None;
    }
    let shouting = collapsed.is_ascii() && !collapsed.chars().any(|c| c.is_ascii_lowercase());
    if !shouting {
        return Some(collapsed);
    }
    Some(
        collapsed
            .split(' ')
            .map(|w| {
                let mut cs = w.chars();
                match cs.next() {
                    Some(f) => f.to_ascii_uppercase().to_string() + &cs.as_str().to_ascii_lowercase(),
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
    )
}

/// Amount without its unit: one decimal for GB, whole numbers otherwise.
pub fn amount(v: f64, unit: Unit) -> String {
    match unit {
        Unit::Gb => format!("{:.1}", (v * 10.0).round() / 10.0),
        Unit::Egp => format!("{v:.2}"),
        _ => thousands(v.round() as i64),
    }
}

/// Like [`amount`] but drops a trailing `.0` (used for totals: "of 140 GB").
pub fn amount_short(v: f64, unit: Unit) -> String {
    let s = amount(v, unit);
    s.strip_suffix(".0").map(str::to_string).unwrap_or(s)
}

fn thousands(n: i64) -> String {
    let s = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
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
    fn normalizes_numbers() {
        assert_eq!(normalize("010 0128 5532", LineType::Mobile).unwrap(), "01001285532");
        assert_eq!(normalize("+20 100 128 5532", LineType::Mobile).unwrap(), "01001285532");
        assert_eq!(normalize("١٠٠١٢٨٥٥٣٢", LineType::Mobile).unwrap(), "01001285532");
        assert_eq!(normalize("02-2358-4417", LineType::Landline).unwrap(), "0223584417");
        assert_eq!(normalize("+20 2 2358 4417", LineType::Landline).unwrap(), "0223584417");
        assert!(normalize("0223584417", LineType::Mobile).is_err());
        assert!(normalize("01001285532", LineType::Landline).is_err());
    }

    #[test]
    fn pretty_numbers() {
        assert_eq!(pretty_number("01001285532"), "010 0128 5532");
        assert_eq!(pretty_number("0223584417"), "02 2358 4417");
        assert_eq!(pretty_number("0403456789"), "040 345 6789");
    }

    #[test]
    fn tidies_names() {
        assert_eq!(tidy_name("  MOSTAFA   HASSAN ALI ").as_deref(), Some("Mostafa Hassan Ali"));
        assert_eq!(tidy_name("مصطفى حسن").as_deref(), Some("مصطفى حسن"));
        assert_eq!(tidy_name("0223584417"), None);
    }

    #[test]
    fn amounts() {
        assert_eq!(amount(62.44, Unit::Gb), "62.4");
        assert_eq!(amount_short(140.0, Unit::Gb), "140");
        assert_eq!(amount(1020.0, Unit::Minutes), "1,020");
    }
}
