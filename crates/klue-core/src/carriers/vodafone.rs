//! Vodafone Egypt mobile lines, via the web.vodafone.com.eg Keycloak login and DXL API.
//!
//! Flow:
//! 1. open the OpenID Connect auth page and post the number/password to its login form;
//! 2. Keycloak redirects to `/ar/KClogin?code=…`; trade the code for an access token;
//! 3. read `usageConsumptionReport` (aggregated buckets: tariff, flex, data, ...).
//!
//! The registrant name comes from the access token's OpenID claims, falling back to the
//! standard `userinfo` endpoint.

use base64::Engine;
use chrono::{DateTime, Utc};
use rand::Rng;
use regex::Regex;
use serde_json::Value;
use wreq::header::{ACCEPT, ACCEPT_LANGUAGE, CONTENT_TYPE, LOCATION, ORIGIN, REFERER};

use super::FetchContext;
use crate::{
    Bundle, BundleKind, FetchError, LineType, Unit, Usage, format::tidy_name, http, json as j, model::assign_keys,
};

const BASE: &str = "https://web.vodafone.com.eg";
const OIDC: &str = "https://web.vodafone.com.eg/auth/realms/vf-realm/protocol/openid-connect";
const REDIRECT_URI: &str = "https://web.vodafone.com.eg/ar/KClogin";
const HTML_ACCEPT: &str = "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,image/apng,*/*;q=0.8";

pub async fn fetch(number: &str, password: &str, ctx: &FetchContext<'_>) -> Result<Usage, FetchError> {
    let msisdn = crate::format::normalize(number, LineType::Mobile)?;
    let client = http::browser_client()?;

    // 1. Login page.
    let nonce: String = (0..10).map(|_| rand::rng().random_range(b'a'..=b'z') as char).collect();
    let auth_url = format!(
        "{OIDC}/auth?client_id=website&redirect_uri={}&state={}&response_mode=query&response_type=code&scope=openid&nonce={nonce}&kc_locale=en",
        url::form_urlencoded::byte_serialize(REDIRECT_URI.as_bytes()).collect::<String>(),
        uuid::Uuid::new_v4(),
    );
    let page = get_following_redirects(&client, &auth_url).await?;
    let action = login_form_action(&page)
        .ok_or_else(|| FetchError::Unexpected("Vodafone's login page has no login form".into()))?;

    // 2. Credentials. Success is a redirect to KClogin carrying the authorization code.
    let resp = client
        .post(&action)
        .header(ACCEPT, HTML_ACCEPT)
        .header(ORIGIN, BASE)
        .header(REFERER, &auth_url)
        .form(&[("username", msisdn.as_str()), ("password", password)])
        .send()
        .await?;
    let location = resp.headers().get(LOCATION).and_then(|l| l.to_str().ok()).map(str::to_string);
    let code = match location.as_deref().and_then(|l| query_param(l, "code")) {
        Some(code) => code,
        None => return Err(login_error(&resp.text().await.unwrap_or_default())),
    };

    let resp = client
        .post(format!("{OIDC}/token"))
        .header(ACCEPT, "*/*")
        .header(ORIGIN, BASE)
        .header(REFERER, REDIRECT_URI)
        .form(&[
            ("code", code.as_str()),
            ("grant_type", "authorization_code"),
            ("client_id", "website"),
            ("redirect_uri", REDIRECT_URI),
        ])
        .send()
        .await?;
    let token = http::json_body(resp, "Vodafone token").await?;
    let access_token = j::text(&token["access_token"])
        .ok_or_else(|| FetchError::Unexpected("Vodafone didn't return an access token".into()))?;

    let owner_name = match owner_from_jwt(&access_token) {
        Some(name) => Some(name),
        None => userinfo_name(&client, &access_token).await,
    };

    // 3. Usage.
    let resp = client
        .get(format!(
            "{BASE}/services/dxl/usage/usageConsumptionReport?bucket.product.publicIdentifier={msisdn}&@type=aggregated"
        ))
        .header("msisdn", msisdn.as_str())
        .bearer_auth(&access_token)
        .header(ACCEPT, "application/json")
        .header(CONTENT_TYPE, "application/json")
        .header(ACCEPT_LANGUAGE, "en-US,en;q=0.9")
        .header(ORIGIN, BASE)
        .header(REFERER, REDIRECT_URI)
        .send()
        .await?;
    let report = http::json_body(resp, "Vodafone usage").await?;
    ctx.dump("vf-usage", &report);
    parse_usage(&report, owner_name, Utc::now())
}

async fn get_following_redirects(client: &wreq::Client, url: &str) -> Result<String, FetchError> {
    let mut url = url.to_string();
    for _ in 0..5 {
        let resp = client.get(&url).header(ACCEPT, HTML_ACCEPT).send().await?;
        if resp.status().is_redirection() {
            let next = resp.headers().get(LOCATION).and_then(|l| l.to_str().ok()).unwrap_or_default();
            url = url::Url::parse(&url)
                .and_then(|base| base.join(next))
                .map_err(|e| FetchError::Unexpected(format!("bad redirect: {e}")))?
                .to_string();
            continue;
        }
        return Ok(resp.text().await?);
    }
    Err(FetchError::Unexpected("too many redirects on Vodafone's login page".into()))
}

/// The `action` of Keycloak's `<form id="kc-form-login">`.
fn login_form_action(html: &str) -> Option<String> {
    let form = Regex::new(r#"(?is)<form\b[^>]*\bid\s*=\s*["']kc-form-login["'][^>]*>"#).ok()?.find(html)?;
    let action = Regex::new(r#"(?is)\baction\s*=\s*["']([^"']+)["']"#).ok()?.captures(form.as_str())?;
    Some(action[1].replace("&amp;", "&"))
}

fn query_param(url: &str, name: &str) -> Option<String> {
    let parsed = url::Url::parse(url).or_else(|_| url::Url::parse(BASE).and_then(|b| b.join(url))).ok()?;
    parsed.query_pairs().find(|(k, _)| k == name).map(|(_, v)| v.into_owned())
}

/// Keycloak re-renders the login page with a message when sign-in fails.
fn login_error(html: &str) -> FetchError {
    let message = Regex::new(r#"(?is)<span[^>]*(?:id\s*=\s*["']input-error["']|class\s*=\s*["'][^"']*kc-feedback-text[^"']*["'])[^>]*>(.*?)</span>"#)
        .ok()
        .and_then(|re| re.captures(html).map(|c| c[1].trim().to_string()))
        .map(|m| Regex::new(r"<[^>]+>").map(|tags| tags.replace_all(&m, "").trim().to_string()).unwrap_or(m))
        .filter(|m| !m.is_empty());
    match message {
        Some(m) if m.to_lowercase().contains("invalid") || m.to_lowercase().contains("password") => FetchError::BadCredentials,
        Some(m) => FetchError::Carrier(m),
        None if html.contains("kc-form-login") => FetchError::BadCredentials,
        None => FetchError::Unexpected("Vodafone didn't redirect after sign-in".into()),
    }
}

fn owner_from_jwt(token: &str) -> Option<String> {
    let payload = token.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload.trim_end_matches('=')).ok()?;
    name_from_claims(&serde_json::from_slice(&bytes).ok()?)
}

async fn userinfo_name(client: &wreq::Client, access_token: &str) -> Option<String> {
    let resp = client.get(format!("{OIDC}/userinfo")).bearer_auth(access_token).header(ACCEPT, "application/json").send().await.ok()?;
    name_from_claims(&http::json_body(resp, "Vodafone userinfo").await.ok()?)
}

fn name_from_claims(claims: &Value) -> Option<String> {
    let joined = [j::text(&claims["given_name"]), j::text(&claims["family_name"])]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ");
    j::text(&claims["name"])
        .and_then(|n| tidy_name(&n))
        .or_else(|| tidy_name(&joined))
}

/// Fakka/Mared cards don't report their size: EGP → (units, days).
const FAKKA_CARDS: [(&str, f64, u32); 4] = [("26", 750.0, 10), ("19.5", 550.0, 7), ("16.5", 425.0, 6), ("13", 300.0, 2)];

pub(crate) fn parse_usage(report: &Value, owner_name: Option<String>, now: DateTime<Utc>) -> Result<Usage, FetchError> {
    let items = report
        .as_array()
        .ok_or_else(|| FetchError::Unexpected("Vodafone's usage report isn't a list".into()))?;
    let mut balance_egp = None;
    let mut tariff_name = None;
    let mut bundles = Vec::new();

    for item in items {
        let kind_raw = item["@type"].as_str().unwrap_or_default().to_ascii_lowercase();
        for bucket in item["bucket"].as_array().into_iter().flatten() {
            let product = product_name(bucket);
            if kind_raw == "tariff" {
                tariff_name = tariff_name.or(product);
                for bal in bucket["bucketBalance"].as_array().into_iter().flatten() {
                    if bal["@type"].as_str().is_some_and(|t| t.eq_ignore_ascii_case("remaining")) {
                        balance_egp = j::num(&bal["remainingValue"]["amount"]).map(|a| (a * 100.0).round() / 100.0);
                    }
                }
                continue;
            }
            let balance = &bucket["bucketBalance"][0];
            let Some(remaining) = j::num(&balance["remainingValue"]["amount"]) else { continue };
            let used = j::num(&bucket["bucketCounter"][0]["value"]["amount"]).unwrap_or(0.0);
            let (unit, factor) = unit_of(balance["remainingValue"]["units"].as_str().unwrap_or_default());
            let mut total = remaining + used;
            if kind_raw == "fakka_and_mared"
                && let Some(card_units) = product.as_deref().and_then(fakka_card_units) {
                    total = total.max(card_units);
                }
            if total <= 0.0 {
                continue;
            }
            let kind = match kind_raw.as_str() {
                "data" => BundleKind::Data,
                "flex" => BundleKind::Flex,
                "voice" | "minutes" | "mins" | "onnet" | "offnet" => BundleKind::Minutes,
                "sms" => BundleKind::Sms,
                k if k.contains("social") => BundleKind::Social,
                _ if unit == Unit::Gb => BundleKind::Addon,
                _ => BundleKind::Other,
            };
            let valid = if balance["validFor"].is_object() { &balance["validFor"] } else { &bucket["validFor"] };
            bundles.push(Bundle {
                key: String::new(),
                kind,
                name: product.unwrap_or_else(|| default_name(kind, &kind_raw)),
                unit,
                total: total * factor,
                remaining: remaining * factor,
                starts_at: j::iso_datetime(&valid["startDateTime"]),
                expires_at: j::iso_datetime(&valid["endDateTime"]),
            });
        }
    }

    // Main quota: the biggest data bucket, else flexes, else whatever came first.
    let main = bundles
        .iter()
        .enumerate()
        .filter(|(_, b)| b.kind == BundleKind::Data)
        .max_by(|a, b| a.1.total.total_cmp(&b.1.total))
        .map(|(i, _)| i)
        .or_else(|| bundles.iter().position(|b| b.kind == BundleKind::Flex));
    if let Some(i) = main {
        let b = bundles.remove(i);
        bundles.insert(0, b);
    }
    assign_keys(&mut bundles);

    let main = bundles.first().ok_or_else(|| FetchError::Unexpected("Vodafone returned no bundles for this line".into()))?;
    let package_name = tariff_name
        .or_else(|| bundles.iter().find(|b| b.kind == BundleKind::Flex).map(|b| b.name.clone()))
        .or_else(|| Some(main.name.clone()));
    let renews_at = main
        .expires_at
        .or_else(|| bundles.iter().find_map(|b| b.expires_at));
    Ok(Usage {
        owner_name,
        package_name,
        balance_egp,
        subscribed_at: main.starts_at,
        renews_at,
        bundles,
        fetched_at: now,
    })
}

fn product_name(bucket: &Value) -> Option<String> {
    j::text(&bucket["product"][0]["name"])
        .or_else(|| j::text(&bucket["product"]["name"]))
        .or_else(|| j::text(&bucket["name"]))
}

fn fakka_card_units(name: &str) -> Option<f64> {
    let price = Regex::new(r"(\d+(?:\.\d+)?)\s*LE").ok()?.captures(name)?[1].to_string();
    FAKKA_CARDS.iter().find(|(p, ..)| *p == price).map(|(_, units, _)| *units)
}

fn default_name(kind: BundleKind, raw: &str) -> String {
    match kind {
        BundleKind::Data => "Mobile data".into(),
        BundleKind::Flex => "Flex".into(),
        BundleKind::Minutes => "Minutes".into(),
        BundleKind::Sms => "SMS".into(),
        _ if raw == "fakka_and_mared" => "Fakka".into(),
        _ => {
            let mut s = raw.replace('_', " ");
            if let Some(f) = s.get_mut(0..1) {
                f.make_ascii_uppercase();
            }
            s
        }
    }
}

fn unit_of(units: &str) -> (Unit, f64) {
    match units.trim().to_ascii_uppercase().as_str() {
        "GB" | "GBS" | "GIGABYTE" | "GIGABYTES" => (Unit::Gb, 1.0),
        "MB" | "MBS" | "MEGABYTE" | "MEGABYTES" => (Unit::Gb, 1.0 / 1024.0),
        "KB" | "KBS" => (Unit::Gb, 1.0 / (1024.0 * 1024.0)),
        "MIN" | "MINS" | "MINUTE" | "MINUTES" => (Unit::Minutes, 1.0),
        "SEC" | "SECS" | "SECONDS" => (Unit::Minutes, 1.0 / 60.0),
        "SMS" | "MESSAGE" | "MESSAGES" => (Unit::Sms, 1.0),
        "FLEX" | "FLEXES" => (Unit::Flex, 1.0),
        "EGP" | "LE" | "L.E" => (Unit::Egp, 1.0),
        _ => (Unit::Units, 1.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture(name: &str) -> Value {
        let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn parses_usage_report() {
        let usage = parse_usage(&fixture("vf_usage.json"), Some("Mostafa".into()), Utc::now()).unwrap();
        assert_eq!(usage.balance_egp, Some(37.25));
        assert_eq!(usage.package_name.as_deref(), Some("Flex 70"));
        let main = usage.main().unwrap();
        assert_eq!(main.kind, BundleKind::Data);
        assert_eq!(main.unit, Unit::Gb);
        assert!((main.total - 12.0).abs() < 1e-9, "MB are converted to GB: {}", main.total);
        assert!((main.remaining - 6.8).abs() < 1e-9);
        assert_eq!(usage.renews_at.unwrap().to_rfc3339(), "2026-12-01T21:59:59+00:00");
        assert_eq!(usage.subscribed_at.unwrap().to_rfc3339(), "2026-11-01T00:00:00+00:00");
        let flex = usage.bundles.iter().find(|b| b.kind == BundleKind::Flex).unwrap();
        assert_eq!((flex.remaining, flex.total, flex.unit), (1200.0, 2000.0, Unit::Flex));
        let fakka = usage.bundles.iter().find(|b| b.name.contains("26 LE")).unwrap();
        assert_eq!(fakka.total, 750.0);
    }

    #[test]
    fn finds_login_form() {
        let html = r#"<div><form id="kc-form-login" onsubmit="x" action="https://web.vodafone.com.eg/auth/realms/vf-realm/login-actions/authenticate?session_code=abc&amp;execution=e1&amp;client_id=website" method="post"></form>"#;
        assert_eq!(
            login_form_action(html).unwrap(),
            "https://web.vodafone.com.eg/auth/realms/vf-realm/login-actions/authenticate?session_code=abc&execution=e1&client_id=website"
        );
        assert!(login_form_action("<form id=\"other\" action=\"x\">").is_none());
    }

    #[test]
    fn reads_code_and_errors() {
        assert_eq!(
            query_param("https://web.vodafone.com.eg/ar/KClogin?state=s&session_state=t&code=c0de.1", "code").as_deref(),
            Some("c0de.1")
        );
        let page = r#"<form id="kc-form-login" action="x"></form><span id="input-error" class="pf-c">
            Invalid username or password.</span>"#;
        assert_eq!(login_error(page), FetchError::BadCredentials);
        let locked = r#"<span class="kc-feedback-text">Account is temporarily disabled; contact your administrator.</span>"#;
        assert!(matches!(login_error(locked), FetchError::Carrier(_)));
    }

    #[test]
    fn reads_name_claims() {
        assert_eq!(name_from_claims(&json!({"name": "SAMIA IBRAHIM FAHMY"})).as_deref(), Some("Samia Ibrahim Fahmy"));
        assert_eq!(name_from_claims(&json!({"given_name": "Samia", "family_name": "Fahmy"})).as_deref(), Some("Samia Fahmy"));
        assert_eq!(name_from_claims(&json!({"preferred_username": "01099214470"})), None);
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(r#"{"name":"Karim Adel"}"#);
        assert_eq!(owner_from_jwt(&format!("h.{payload}.s")).as_deref(), Some("Karim Adel"));
    }
}
