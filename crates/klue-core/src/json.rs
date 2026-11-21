//! Lenient helpers for poking at carrier JSON, which is loosely typed
//! (numbers arrive as strings, keys vary between API versions).

use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};
use serde_json::Value;

/// A number, or a string holding a number.
pub(crate) fn num(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// A non-empty string, or a number rendered as a string.
pub(crate) fn text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// `true`, `"true"`, `"Y"`, `1`...
pub(crate) fn truthy(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        Value::String(s) => matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "y" | "yes" | "1"),
        _ => false,
    }
}

/// First non-empty string found under any of `keys` (case-insensitive), searching
/// `v` depth-first. Keys earlier in the list win over later ones.
pub(crate) fn find_text(v: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|k| find_key(v, k, 0).and_then(text))
}

fn find_key<'a>(v: &'a Value, key: &str, depth: usize) -> Option<&'a Value> {
    if depth > 6 {
        return None;
    }
    match v {
        Value::Object(map) => {
            for (k, val) in map {
                if k.eq_ignore_ascii_case(key) && text(val).is_some() {
                    return Some(val);
                }
            }
            map.values().find_map(|val| find_key(val, key, depth + 1))
        }
        Value::Array(items) => items.iter().find_map(|val| find_key(val, key, depth + 1)),
        _ => None,
    }
}

/// Milliseconds since the epoch (number or string).
pub(crate) fn epoch_ms(v: &Value) -> Option<DateTime<Utc>> {
    let ms = num(v)? as i64;
    if ms <= 0 {
        return None;
    }
    Utc.timestamp_millis_opt(ms).single()
}

/// RFC 3339 / ISO 8601 date-time, tolerating a missing offset (taken as Cairo time)
/// or a bare date.
pub(crate) fn iso_datetime(v: &Value) -> Option<DateTime<Utc>> {
    let s = v.as_str()?.trim();
    if s.is_empty() {
        return None;
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }
    for fmt in ["%Y-%m-%dT%H:%M:%S%.f%z", "%Y-%m-%dT%H:%M:%S%z"] {
        if let Ok(dt) = DateTime::parse_from_str(s, fmt) {
            return Some(dt.with_timezone(&Utc));
        }
    }
    for fmt in ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%dT%H:%M:%S", "%Y-%m-%d %H:%M:%S"] {
        if let Ok(naive) = NaiveDateTime::parse_from_str(s, fmt) {
            return cairo(naive);
        }
    }
    NaiveDate::parse_from_str(s.get(..10)?, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(23, 59, 59))
        .and_then(cairo)
}

/// Interpret a naive timestamp as Egypt local time (UTC+2 standard, UTC+3 in summer).
/// Using the machine's local zone is the closest we can get without a tz database,
/// and Klue users are in Egypt.
fn cairo(naive: NaiveDateTime) -> Option<DateTime<Utc>> {
    chrono::Local
        .from_local_datetime(&naive)
        .earliest()
        .map(|dt| dt.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn lenient_numbers() {
        assert_eq!(num(&json!("12.5")), Some(12.5));
        assert_eq!(num(&json!(3)), Some(3.0));
        assert_eq!(num(&json!(null)), None);
    }

    #[test]
    fn finds_nested_keys_in_priority_order() {
        let v = json!({"a": {"custName": "  Mostafa  ", "acctName": "x"}, "acctName": ""});
        assert_eq!(find_text(&v, &["custName", "acctName"]), Some("Mostafa".into()));
        assert_eq!(find_text(&v, &["acctName"]), Some("x".into()));
        assert_eq!(find_text(&v, &["missing"]), None);
    }

    #[test]
    fn parses_dates() {
        let d = iso_datetime(&json!("2026-12-08T21:59:59.000Z")).unwrap();
        assert_eq!(d.to_rfc3339(), "2026-12-08T21:59:59+00:00");
        assert!(iso_datetime(&json!("2026-12-08")).is_some());
        assert_eq!(epoch_ms(&json!("1764892800000")).unwrap().timestamp(), 1_764_892_800);
    }
}
