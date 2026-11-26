//! Carrier clients. Each signs in with the account's own credentials and returns a
//! [`Usage`].

pub mod orange;
pub mod vodafone;
pub mod we;

use std::path::PathBuf;

use serde_json::Value;

use crate::{Carrier, FetchError, LineType, Usage, captcha::CaptchaSolver, format, store::Account};

#[derive(Default)]
pub struct FetchContext<'a> {
    /// Reads WE captchas. It can decline one it isn't sure of; WE is then asked for another
    /// (see [`we::CAPTCHA_REROLLS`]).
    pub solver: Option<&'a dyn CaptchaSolver>,
    /// Asks the user, only after `solver` declined every captcha.
    pub ask: Option<&'a dyn CaptchaSolver>,
    /// Raw responses (secrets redacted) are saved here when set. `KLUE_DEBUG_DIR`.
    pub debug_dir: Option<PathBuf>,
}

impl FetchContext<'_> {
    pub(crate) fn dump(&self, name: &str, value: &Value) {
        let Some(dir) = &self.debug_dir else { return };
        let mut v = value.clone();
        redact(&mut v);
        let file = dir.join(format!("{}-{name}.json", chrono::Utc::now().format("%Y%m%dT%H%M%S%.3f")));
        let _ = std::fs::create_dir_all(dir);
        if let Err(e) = std::fs::write(&file, serde_json::to_vec_pretty(&v).unwrap_or_default()) {
            tracing::warn!("couldn't write debug dump {}: {e}", file.display());
        }
    }
}

/// Blanks secrets before a response is saved: keys containing one of `SECRET`, and personal
/// ids that are never read (SIM serial, national ID, email, birthday).
fn redact(v: &mut Value) {
    const SECRET: [&str; 6] = ["password", "pwd", "token", "secret", "captcha", "imgcode"];
    const PERSONAL: [&str; 6] = ["sim", "idnumber", "iccid", "imsi", "email", "birthday"];
    match v {
        Value::Object(map) => {
            for (k, val) in map.iter_mut() {
                let key = k.to_ascii_lowercase();
                if SECRET.iter().any(|s| key.contains(s)) || PERSONAL.contains(&key.as_str()) {
                    *val = Value::String("<redacted>".into());
                } else {
                    redact(val);
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(redact),
        _ => {}
    }
}

/// `number` is the one the line signs in with (see [`Carrier::sign_in_line`]).
pub async fn fetch(
    carrier: Carrier,
    line: LineType,
    number: &str,
    password: &str,
    ctx: &FetchContext<'_>,
) -> Result<Usage, FetchError> {
    if !carrier.supports_line(line) {
        return Err(FetchError::Unsupported);
    }
    match carrier {
        Carrier::We => we::fetch(number, password, ctx).await,
        Carrier::Vodafone => vodafone::fetch(number, password, ctx).await,
        Carrier::Orange => orange::fetch(line, number, password, ctx).await,
        Carrier::Etisalat => Err(FetchError::Unsupported),
    }
}

/// Reads a saved line, and checks the carrier still names it for its sign-in number.
pub async fn fetch_account(account: &Account, password: &str, ctx: &FetchContext<'_>) -> Result<Usage, FetchError> {
    let sign_in = account.sign_in_number();
    let usage = fetch(account.carrier, account.line, sign_in, password, ctx).await?;
    if line_number(account.carrier, account.line, sign_in, &usage)? != account.number {
        return Err(FetchError::NoLine(account.line));
    }
    Ok(usage)
}

/// The account's lines besides `line`, read right after signing in to it: an Orange account
/// can hold a mobile line and home internet. Orange keeps the session from that sign-in, so
/// this doesn't sign in again. Nothing for carriers whose accounts hold one line.
pub async fn other_lines(carrier: Carrier, line: LineType, sign_in: &str, password: &str, ctx: &FetchContext<'_>) -> Vec<(LineType, Usage)> {
    if carrier != Carrier::Orange {
        return Vec::new();
    }
    let other = match line {
        LineType::Mobile => LineType::Landline,
        LineType::Landline => LineType::Mobile,
    };
    match fetch(carrier, other, sign_in, password, ctx).await {
        Ok(usage) if worth_adding(other, &usage) => vec![(other, usage)],
        Ok(_) => Vec::new(),
        Err(e) => {
            tracing::info!("no {other:?} line on this {carrier:?} account: {e}");
            Vec::new()
        }
    }
}

/// A mobile line found next to home internet only counts with a plan or a bundle; a number
/// that only signs in to My Orange has neither.
fn worth_adding(line: LineType, usage: &Usage) -> bool {
    line == LineType::Landline || usage.package_name.is_some() || !usage.bundles.is_empty()
}

/// The line's own number: `sign_in` itself, or for a line that signs in with another number
/// (Orange home internet), the landline the carrier names in [`crate::model::LineInfo::number`].
pub fn line_number(carrier: Carrier, line: LineType, sign_in: &str, usage: &Usage) -> Result<String, FetchError> {
    if carrier.sign_in_line(line) == line {
        return Ok(sign_in.to_string());
    }
    let named = usage.info.number.as_deref().ok_or_else(|| FetchError::Unexpected("the carrier didn't say which line this is".into()))?;
    format::normalize(named, line)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_mobile_line_needs_a_plan_or_a_bundle() {
        let mut usage: Usage = serde_json::from_value(json!({"bundles": [], "fetched_at": "2026-11-26T08:00:00Z"})).unwrap();
        assert!(!worth_adding(LineType::Mobile, &usage));
        assert!(worth_adding(LineType::Landline, &usage));
        usage.package_name = Some("FREEmax 80".into());
        assert!(worth_adding(LineType::Mobile, &usage));
    }

    #[test]
    fn redacts_secrets() {
        let mut v = json!({"body": {"token": "abc", "uToken": "u", "subscriber": {"name": "x", "SIM": "89"}}, "list": [{"password": "p"}]});
        redact(&mut v);
        assert_eq!(v["body"]["token"], "<redacted>");
        assert_eq!(v["body"]["uToken"], "<redacted>");
        assert_eq!(v["body"]["subscriber"]["SIM"], "<redacted>");
        assert_eq!(v["list"][0]["password"], "<redacted>");
        assert_eq!(v["body"]["subscriber"]["name"], "x");
    }
}
