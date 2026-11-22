//! Vodafone Egypt mobile lines, via the web.vodafone.com.eg Keycloak login and DXL API.
//!
//! Flow:
//! 1. open the OpenID Connect auth page and post the number/password to its login form;
//! 2. Keycloak redirects to `/ar/KClogin?code=…`; trade the code for an access token;
//! 3. read `usageConsumptionReport` (aggregated buckets: tariff, flex, data, ...);
//! 4. best-effort extras: the rate plan's name and the payment history.
//!
//! The access token is a JWT whose `userInfo` claim carries the line's profile (number,
//! first and last name, prepaid/postpaid, status, rate plan code). Klue only reads it:
//! it came straight from Vodafone over TLS, so there's no need to check its signature.

use base64::Engine;
use chrono::{DateTime, Utc};
use rand::Rng;
use regex::Regex;
use serde_json::Value;
use wreq::header::{ACCEPT, ACCEPT_LANGUAGE, CONTENT_TYPE, LOCATION, ORIGIN, REFERER};

use super::FetchContext;
use crate::{
    Bundle, BundleKind, FetchError, LineType, Unit, Usage,
    format::tidy_name,
    http, json as j,
    model::{LineInfo, Payment, PaymentKind, assign_keys, order_bundles},
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

    let claims = token_claims(&access_token);
    if let Some(claims) = &claims {
        ctx.dump("vf-claims", claims);
    }
    let mut profile = claims.as_ref().map(profile_from_claims).unwrap_or_default();
    if profile.name.is_none() {
        profile.name = userinfo_name(&client, &access_token).await;
    }

    // 3. Usage.
    let resp = dxl_get(
        &client,
        &format!("{BASE}/services/dxl/usage/usageConsumptionReport?bucket.product.publicIdentifier={msisdn}&@type=aggregated"),
        &access_token,
        &msisdn,
    )
    .send()
    .await?;
    let report = http::json_body(resp, "Vodafone usage").await?;
    ctx.dump("vf-usage", &report);
    // The rate plan's name (best effort) also names a Flex line's package.
    let plan = match &profile.plan_code {
        Some(code) => plan_name(&client, code, ctx).await,
        None => None,
    };
    let mut usage = parse_usage(&report, profile.name.clone(), plan.as_deref(), Utc::now())?;

    // 4. Extras. Not essential, so failures only leave them out.
    usage.info = LineInfo {
        plan: plan.or_else(|| profile.tariff.clone()),
        payment: profile.payment,
        status: profile.status.clone(),
        number: profile.msisdn.clone(),
        since: None,
    };
    let payer = profile.user_id.as_deref().unwrap_or(&msisdn);
    usage.payments = payment_history(&client, &access_token, payer, ctx).await;
    Ok(usage)
}

/// A DXL API request with the headers the web app sends.
fn dxl_get(client: &wreq::Client, url: &str, access_token: &str, msisdn: &str) -> wreq::RequestBuilder {
    client
        .get(url)
        .header("msisdn", msisdn)
        .header("clientId", "WebsiteConsumer")
        .bearer_auth(access_token)
        .header(ACCEPT, "application/json")
        .header(CONTENT_TYPE, "application/json")
        .header(ACCEPT_LANGUAGE, "en-US,en;q=0.9")
        .header(ORIGIN, BASE)
        .header(REFERER, REDIRECT_URI)
}

/// `@type` and `useCase` of the web app's "Recharge & bills history" request
/// (`getPaymentHistory` in its recharge module), which sends `api_id: WEB` alongside.
const PAYMENTS_TYPE: &str = "creditCardHistory";

/// Card top-ups. `payer` is the line as the web app names it: the token's
/// `userInfo.userId`, else its `msisdn` (see [`Profile::user_id`]).
async fn payment_history(client: &wreq::Client, access_token: &str, payer: &str, ctx: &FetchContext<'_>) -> Option<Vec<Payment>> {
    let url = format!("{BASE}/services/dxl/paymentmng/payment?payer.id={payer}&@type={PAYMENTS_TYPE}");
    let resp = dxl_get(client, &url, access_token, payer)
        .header("useCase", PAYMENTS_TYPE)
        .header("api_id", "WEB")
        .send()
        .await
        .ok()?;
    match http::json_body(resp, "Vodafone payments").await {
        Ok(v) => {
            ctx.dump("vf-payments", &v);
            let payments = parse_payments(&v);
            if payments.is_none() {
                tracing::info!("Vodafone payment history isn't a list; leaving it out");
            }
            payments
        }
        Err(e) => {
            tracing::info!("Vodafone payment history unavailable: {e}");
            None
        }
    }
}

/// `paymentmng/payment`: a list of payments, amounts in piastres. Without any in its
/// 90-day window Vodafone answers with an error object instead: `reason: "No Data Found"`.
pub(crate) fn parse_payments(v: &Value) -> Option<Vec<Payment>> {
    if j::text(&v["reason"]).is_some_and(|r| r.eq_ignore_ascii_case("no data found")) {
        return Some(Vec::new());
    }
    let mut out: Vec<Payment> = v
        .as_array()?
        .iter()
        .filter_map(|p| {
            let at = j::iso_datetime(&p["paymentDate"])?;
            let amount = j::num(&p["amount"]["value"])? / 100.0;
            let card = j::text(&p["paymentMethod"]["id"])
                .and_then(|id| {
                    let last4: String = id.chars().rev().take_while(|c| c.is_ascii_digit()).collect::<Vec<_>>().into_iter().rev().collect();
                    (last4.len() >= 4).then(|| format!("Card •••• {}", &last4[last4.len() - 4..]))
                });
            let kind = j::text(&p["paymentItem"][0]["@type"]).map(|t| humanize(&t));
            Some(Payment { at, amount_egp: amount, method: card.or(kind), reference: j::text(&p["id"]), balance_after: None })
        })
        .collect();
    out.sort_by_key(|p| std::cmp::Reverse(p.at));
    Some(out)
}

/// The rate plan's display name (e.g. "Worry Free 14 PT") from Vodafone's content API,
/// keyed by the token's `serviceClassCode`.
async fn plan_name(client: &wreq::Client, code: &str, ctx: &FetchContext<'_>) -> Option<String> {
    let url = format!("{BASE}/o/VodafoneDDLRecordRest/records/get/en/233987/PlanID/{code}");
    let resp = client.get(url).header(ACCEPT, "application/json").header(REFERER, BASE).send().await.ok()?;
    let v = http::json_body(resp, "Vodafone plan").await.ok()?;
    ctx.dump("vf-plan", &v);
    j::text(&v["PlanName"]).or_else(|| j::text(&v["eDesc"]["PlanName"])).filter(|n| n != "N/A")
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

/// The JWT's payload (claims). The signature isn't checked: the token came straight from
/// Vodafone's token endpoint over TLS and Klue only reads it.
fn token_claims(token: &str) -> Option<Value> {
    let payload = token.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload.trim_end_matches('=')).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// What the access token says about the line.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct Profile {
    pub name: Option<String>,
    pub msisdn: Option<String>,
    /// The line as the web app addresses it in DXL calls (`userId`, else `msisdn`).
    pub user_id: Option<String>,
    pub payment: Option<PaymentKind>,
    pub status: Option<String>,
    /// Rate plan code (`serviceClassCode`), for [`plan_name`].
    pub plan_code: Option<String>,
    /// Fallback plan label when the plan's display name can't be fetched.
    pub tariff: Option<String>,
}

pub(crate) fn profile_from_claims(claims: &Value) -> Profile {
    let info = &claims["userInfo"];
    let full = [j::text(&info["firstName"]), j::text(&info["lastName"])].into_iter().flatten().collect::<Vec<_>>().join(" ");
    let kind = j::text(&info["contractType"]).or_else(|| j::text(&info["customerType"])).unwrap_or_default().to_ascii_lowercase();
    Profile {
        name: tidy_name(&full).or_else(|| name_from_claims(claims)),
        msisdn: j::text(&info["msisdn"]),
        user_id: [&info["userId"], &info["msisdn"]]
            .into_iter()
            .filter_map(j::text)
            .find(|id| id.chars().all(|c| c.is_ascii_digit())),
        payment: match kind.as_str() {
            "prepaid" => Some(PaymentKind::Prepaid),
            "postpaid" => Some(PaymentKind::Postpaid),
            _ => None,
        },
        status: j::text(&info["contractStatus"]).map(|s| humanize(&s)),
        plan_code: j::text(&info["serviceClassCode"]),
        tariff: j::text(&info["serviceClassName"]).map(|s| humanize(&s)),
    }
}

/// `SOME_CODE_name` → `Some code name`.
fn humanize(raw: &str) -> String {
    let words = raw.replace('_', " ").split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = if words.chars().any(|c| c.is_ascii_lowercase()) { words } else { words.to_lowercase() };
    let mut cs = lower.chars();
    cs.next().map(|f| f.to_uppercase().collect::<String>() + cs.as_str()).unwrap_or_default()
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

/// `plan` is the rate plan's display name ([`plan_name`]); a Flex line's package is its
/// plan ("Flex 100"), since its flex bucket can be named otherwise ("AFB Freeze Quota"
/// while a borrowed bundle is frozen).
pub(crate) fn parse_usage(report: &Value, owner_name: Option<String>, plan: Option<&str>, now: DateTime<Utc>) -> Result<Usage, FetchError> {
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
            let units = balance["remainingValue"]["units"].as_str().unwrap_or_default();
            let usage_type = bucket["usageType"].as_str().unwrap_or_default().to_ascii_lowercase();
            if is_internal_counter(&kind_raw, &usage_type, units) {
                continue;
            }
            let Some(remaining) = j::num(&balance["remainingValue"]["amount"]) else { continue };
            if remaining <= 0.0 && is_grace_counter(bucket) {
                continue;
            }
            let used = j::num(&bucket["bucketCounter"][0]["value"]["amount"]).unwrap_or(0.0);
            let (unit, factor) = unit_of(units);
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
                _ if matches!(unit, Unit::Gb | Unit::Mb) => BundleKind::Addon,
                _ => BundleKind::Other,
            };
            let valid = if balance["validFor"].is_object() { &balance["validFor"] } else { &bucket["validFor"] };
            bundles.push(Bundle {
                key: String::new(),
                kind,
                name: bucket_name(bucket).unwrap_or_else(|| default_name(kind, &kind_raw)),
                unit,
                total: total * factor,
                remaining: remaining * factor,
                starts_at: j::iso_datetime(&valid["startDateTime"]),
                expires_at: j::iso_datetime(&valid["endDateTime"]),
            });
        }
    }

    order_bundles(&mut bundles);
    assign_keys(&mut bundles);

    let main = bundles.first().ok_or_else(|| FetchError::Unexpected("Vodafone returned no bundles for this line".into()))?;
    let flex = bundles.iter().find(|b| b.kind == BundleKind::Flex);
    let package_name = tariff_name
        .or_else(|| plan.filter(|_| flex.is_some()).map(str::to_string))
        .or_else(|| flex.map(|b| b.name.clone()))
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
        info: LineInfo::default(),
        payments: None,
    })
}

/// Buckets that are internal counters, not quotas: the "monetary" pool counted in
/// piastres (named "ACP/RX", "Stamp Tax", ...), refund caps in money, and flag counters
/// such as "Mared/Trio Flex Flag" in the OTHERS group.
fn is_internal_counter(kind_raw: &str, usage_type: &str, units: &str) -> bool {
    let units = units.trim().to_ascii_uppercase();
    matches!(usage_type, "monetary" | "money")
        || (kind_raw == "others" && usage_type == "units")
        || matches!(units.as_str(), "PT" | "PTS" | "PIASTER" | "PIASTERS" | "PIASTRES" | "MONEY")
}

/// `Grace_Flex` buckets (units `DataFlexes` / `VoiceFlexes`) count the flexes used on
/// credit while a Flex bundle is borrowed or frozen; that bundle carries the quota itself.
fn is_grace_counter(bucket: &Value) -> bool {
    bucket["product"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|p| [j::text(&p["id"]), j::text(&p["name"])])
        .flatten()
        .any(|s| s.eq_ignore_ascii_case("grace_flex"))
}

/// A bucket's display name. A bucket can list several products (a bundle plus its
/// rollover); prefer the bundle, and drop internal suffixes like "(Flex inactive/Mass)".
fn bucket_name(bucket: &Value) -> Option<String> {
    let names: Vec<String> = match &bucket["product"] {
        Value::Array(items) => items.iter().filter_map(|p| j::text(&p["name"])).collect(),
        other => j::text(&other["name"]).or_else(|| j::text(&bucket["name"])).into_iter().collect(),
    };
    let pick = names.iter().find(|n| !n.to_ascii_lowercase().contains("rollover")).or(names.first())?;
    let without_note = Regex::new(r"\s*\([^)]*\)\s*$").map(|re| re.replace(pick, "").into_owned()).unwrap_or_else(|_| pick.clone());
    let spaced = Regex::new(r"\s*-\s*").map(|re| re.replace_all(&without_note, " - ").into_owned()).unwrap_or(without_note);
    let name = if spaced.contains('_') { humanize(&spaced) } else { spaced };
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    (!name.is_empty()).then_some(name)
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
        "MB" | "MBS" | "MEGABYTE" | "MEGABYTES" | "MEGA" => (Unit::Mb, 1.0),
        "KB" | "KBS" => (Unit::Mb, 1.0 / 1024.0),
        "MIN" | "MINS" | "MINUTE" | "MINUTES" => (Unit::Minutes, 1.0),
        "SEC" | "SECS" | "SECONDS" => (Unit::Minutes, 1.0 / 60.0),
        "SMS" | "MESSAGE" | "MESSAGES" => (Unit::Sms, 1.0),
        // "Flex", and Flex 5.0's split pools "DataFlexes" / "VoiceFlexes"
        u if u.contains("FLEX") => (Unit::Flex, 1.0),
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
        let usage = parse_usage(&fixture("vf_usage.json"), Some("Mostafa".into()), None, Utc::now()).unwrap();
        assert_eq!(usage.balance_egp, Some(37.25));
        assert_eq!(usage.package_name.as_deref(), Some("Flex 70"));
        let main = usage.main().unwrap();
        assert_eq!(main.kind, BundleKind::Data);
        // Kept in the unit Vodafone states.
        assert_eq!((main.unit, main.total, main.remaining), (Unit::Mb, 12288.0, 6963.2));
        assert_eq!(usage.renews_at.unwrap().to_rfc3339(), "2026-12-01T21:59:59+00:00");
        assert_eq!(usage.subscribed_at.unwrap().to_rfc3339(), "2026-11-01T00:00:00+00:00");
        let flex = usage.bundles.iter().find(|b| b.kind == BundleKind::Flex).unwrap();
        assert_eq!((flex.remaining, flex.total, flex.unit), (1200.0, 2000.0, Unit::Flex));
        let fakka = usage.bundles.iter().find(|b| b.name.contains("26 LE")).unwrap();
        assert_eq!(fakka.total, 750.0);
    }

    #[test]
    fn keeps_units_and_drops_internal_counters() {
        let usage = parse_usage(&fixture("vf_usage_others.json"), None, Some("Worry Free 14 PT"), Utc::now()).unwrap();
        assert_eq!(usage.balance_egp, Some(12.5));
        // Not a Flex line: the package is the subscribed bundle, not the tariff.
        assert_eq!(usage.package_name.as_deref(), Some("New Plus Bundle 98 LE - 4750Mega"));
        let main = usage.main().unwrap();
        assert_eq!(main.name, "New Plus Bundle 98 LE - 4750Mega");
        assert_eq!((main.unit, main.remaining, main.total), (Unit::Mb, 992.0, 4748.0));
        assert_eq!(main.expires_at.unwrap().to_rfc3339(), "2026-12-12T00:00:00+00:00");
        let names: Vec<_> = usage.bundles.iter().map(|b| b.name.as_str()).collect();
        // ACP/RX (a monetary pool in piastres), the refund cap and the flag counter are gone.
        assert_eq!(names, ["New Plus Bundle 98 LE - 4750Mega", "MI Minutes Conversion Optin"]);
        assert_eq!(usage.bundles[1].unit, Unit::Mb);
    }

    #[test]
    fn frozen_flex_line() {
        // A borrowed Flex bundle, frozen until settled: Vodafone names its bucket "AFB Freeze
        // Quota", counts the flexes used on credit in Grace_Flex buckets and the debt in a
        // "Negative Balance" bucket in piastres.
        let report = fixture("vf_usage_flex_frozen.json");
        let usage = parse_usage(&report, None, Some("Flex 100"), Utc::now()).unwrap();
        assert_eq!(usage.package_name.as_deref(), Some("Flex 100"));
        assert_eq!(usage.balance_egp, Some(0.0));
        let rows: Vec<_> = usage.bundles.iter().map(|b| (b.name.as_str(), b.unit, b.remaining, b.total)).collect();
        assert_eq!(rows, [("AFB Freeze Quota", Unit::Flex, 4200.0, 5000.0), ("Fakka 19 LE", Unit::Units, 120.0, 425.0)]);
        assert_eq!(usage.main().unwrap().kind, BundleKind::Flex);
        assert_eq!(usage.renews_at.unwrap().to_rfc3339(), "2026-12-17T00:00:00+00:00");

        // Without the plan's name, the flex bucket's.
        let usage = parse_usage(&report, None, None, Utc::now()).unwrap();
        assert_eq!(usage.package_name.as_deref(), Some("AFB Freeze Quota"));

        // Grace flexes still to use would be shown, in flexes.
        let mut grace = report.clone();
        grace[3]["bucket"][0]["bucketBalance"][0]["remainingValue"]["amount"] = json!(50);
        let usage = parse_usage(&grace, None, None, Utc::now()).unwrap();
        let g = usage.bundles.iter().find(|b| b.name == "Grace Flex").unwrap();
        assert_eq!((g.unit, g.remaining, g.total), (Unit::Flex, 50.0, 750.0));
    }

    #[test]
    fn no_payments_is_an_empty_history() {
        assert_eq!(parse_payments(&fixture("vf_payments_none.json")), Some(vec![]));
        assert_eq!(parse_payments(&json!([])), Some(vec![]));
    }

    #[test]
    fn parses_payments_newest_first() {
        let payments = parse_payments(&fixture("vf_payments.json")).unwrap();
        assert_eq!(payments.len(), 3);
        assert_eq!(payments[0].at.to_rfc3339(), "2026-11-14T17:51:27+00:00");
        assert_eq!(payments[0].amount_egp, 140.0);
        assert_eq!(payments[0].method.as_deref(), Some("Card •••• 2222"));
        assert_eq!(payments[1].amount_egp, 145.0);
        assert_eq!(payments[2].method.as_deref(), Some("Topup via fawry"));
        assert_eq!(parse_payments(&json!({"code": "2001", "message": "Access Denied"})), None);
    }

    #[test]
    fn reads_the_token_profile() {
        let claims = json!({
            "preferred_username": "1000000000",
            "userInfo": {
                "msisdn": "01000000000",
                "firstName": "سارة",
                "lastName": "أحمد",
                "customerType": "PREPAID",
                "contractType": "prepaid",
                "contractStatus": "ACTIVE",
                "serviceClassCode": 723,
                "serviceClassName": "14pt_Raya7Balak"
            }
        });
        let p = profile_from_claims(&claims);
        assert_eq!(p.name.as_deref(), Some("سارة أحمد"));
        assert_eq!(p.msisdn.as_deref(), Some("01000000000"));
        // No userId in the token: the web app then addresses the line by its msisdn.
        assert_eq!(p.user_id.as_deref(), Some("01000000000"));
        let mut with_id = claims.clone();
        with_id["userInfo"]["userId"] = json!("1000000000");
        assert_eq!(profile_from_claims(&with_id).user_id.as_deref(), Some("1000000000"));
        with_id["userInfo"]["userId"] = json!("someone@example.com");
        assert_eq!(profile_from_claims(&with_id).user_id.as_deref(), Some("01000000000"));
        assert_eq!(p.payment, Some(PaymentKind::Prepaid));
        assert_eq!(p.status.as_deref(), Some("Active"));
        assert_eq!(p.plan_code.as_deref(), Some("723"));
        assert_eq!(p.tariff.as_deref(), Some("14pt Raya7Balak"));
        // No userInfo: standard claims still give the name.
        assert_eq!(profile_from_claims(&json!({"name": "SAMIA FAHMY"})).name.as_deref(), Some("Samia Fahmy"));
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(claims.to_string());
        assert_eq!(token_claims(&format!("h.{payload}.s")).unwrap()["userInfo"]["msisdn"], "01000000000");
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
    }
}
