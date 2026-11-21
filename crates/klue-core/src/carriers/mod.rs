//! Carrier clients. Each one signs in with the same credentials the carrier's own app
//! uses and returns a normalised [`Usage`] snapshot.

pub mod vodafone;
pub mod we;

use std::path::PathBuf;

use serde_json::Value;

use crate::{Carrier, FetchError, LineType, Usage, captcha::CaptchaSolver};

/// Everything a client needs besides the credentials.
#[derive(Default)]
pub struct FetchContext<'a> {
    /// Used when WE asks for a captcha. Without one, such sign-ins fail with
    /// [`FetchError::CaptchaUnsolved`].
    pub solver: Option<&'a dyn CaptchaSolver>,
    /// When set, raw carrier responses (with secrets redacted) are written here so
    /// parsers can be checked against real data. Enabled with `KLUE_DEBUG_DIR`.
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

/// Blank out anything secret-looking before a response is written to disk.
fn redact(v: &mut Value) {
    const SECRET: [&str; 8] = ["password", "token", "csrftoken", "access_token", "refresh_token", "id_token", "captcha", "imgcode"];
    match v {
        Value::Object(map) => {
            for (k, val) in map.iter_mut() {
                if SECRET.iter().any(|s| k.eq_ignore_ascii_case(s)) {
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

/// Sign in and read the account.
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
        Carrier::Orange | Carrier::Etisalat => Err(FetchError::Unsupported),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn redacts_secrets() {
        let mut v = json!({"body": {"token": "abc", "subscriber": {"name": "x"}}, "list": [{"password": "p"}]});
        redact(&mut v);
        assert_eq!(v["body"]["token"], "<redacted>");
        assert_eq!(v["list"][0]["password"], "<redacted>");
        assert_eq!(v["body"]["subscriber"]["name"], "x");
    }
}
