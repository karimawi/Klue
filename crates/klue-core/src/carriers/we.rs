//! WE (Telecom Egypt) home internet, via the my.te.eg self-care API.
//!
//! Flow:
//! 1. seed cookies from the portal and call `querySysParams`, like the website does;
//! 2. `GenerateCaptcha` returns a token, and an image when WE wants proof of a human
//!    (after a few sign-ins). `status: "Blocked"` means it stopped handing out tokens;
//!    Each call replaces the previous captcha, so when the solver isn't sure of an image
//!    Klue asks for another one rather than guess;
//! 3. `userAuthenticate` with the token (and the solved code, if any);
//! 4. `queryBalance`, `getSubscribedOfferings` and `queryFreeUnit` with the CSRF token;
//!    best-effort, `queryRechargeLogCz` (payment history) and `querySubscribers` (plan).
//!
//! All requests go out with Chrome's TLS fingerprint (`curl_cffi`'s `chrome131` in the
//! Python original), since WE's edge rejects obvious non-browser clients.

use std::time::Duration;

use chrono::Utc;
use serde_json::{Value, json};
use wreq::header::{ACCEPT, ACCEPT_LANGUAGE, HeaderMap, HeaderName, HeaderValue, ORIGIN, REFERER};

use super::FetchContext;
use crate::{
    Bundle, BundleKind, FetchError, LineType, Unit, Usage,
    model::{LineInfo, Payment, PaymentKind, order_bundles},
    captcha::{clean_code, decode_data_uri},
    format::tidy_name,
    http,
    json::{self as j},
    model::assign_keys,
};

const PORTAL: &str = "https://my.te.eg/";
const SITE: &str = "https://my.te.eg";
const API: &str = "https://api-my.te.eg/echannel/service/besapp/base/rest/busiservice";
const CAPTCHA_URL: &str = "https://captcha.te.eg/api/Captcha/GenerateCaptcha";

/// How many more captchas to ask WE for when the solver isn't sure of one. Fetching a
/// captcha doesn't count as a sign-in attempt; sending a wrong code does.
pub const CAPTCHA_REROLLS: usize = 8;
/// Pause before each reroll, so they don't arrive as a burst.
const REROLL_PAUSE: Duration = Duration::from_millis(600);

/// How long WE keeps refusing captcha tokens after answering "Blocked"
/// (about an hour).
pub const BLOCK_BACKOFF: Duration = Duration::from_secs(3700);

/// What `GenerateCaptcha` handed out.
#[derive(Debug, Clone)]
pub struct CaptchaChallenge {
    pub token: String,
    /// Raw image bytes (usually JPEG) when WE wants the code typed in.
    pub image: Option<Vec<u8>>,
    pub status: Option<String>,
}

impl CaptchaChallenge {
    pub fn needs_code(&self) -> bool {
        self.image.is_some()
    }
}

/// A signed-in session.
pub struct Authenticated {
    csrf_token: String,
    subscriber_id: String,
    account_id: String,
    pub owner_name: Option<String>,
    payment: Option<PaymentKind>,
    status: Option<String>,
}

/// One sign-in attempt against WE, with its own cookie jar.
pub struct WeSession {
    client: wreq::Client,
    number: String,
    /// `FBB` + the number without its leading zero, e.g. `FBB223584417`.
    fbb_id: String,
}

impl WeSession {
    pub fn new(number: &str) -> Result<Self, FetchError> {
        let number = crate::format::normalize(number, LineType::Landline)?;
        let fbb_id = format!("FBB{}", &number[1..]);
        Ok(Self { client: http::browser_client()?, number, fbb_id })
    }

    /// Load the portal and the system parameters, as the website does before signing in.
    /// Failures are ignored: these calls only seed cookies.
    pub async fn warm_up(&self) {
        if let Err(e) = self.client.get(PORTAL).send().await {
            tracing::debug!("WE portal seed failed: {e}");
        }
        let url = format!("{API}/v1/common/querySysParams");
        if let Err(e) = self.client.post(url).headers(api_headers(None)).json(&json!({})).send().await {
            tracing::debug!("WE querySysParams failed: {e}");
        }
    }

    /// Ask for a captcha token for this number.
    pub async fn captcha(&self, ctx: &FetchContext<'_>) -> Result<CaptchaChallenge, FetchError> {
        let mut headers = cors_headers();
        headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.9"));
        let resp = self
            .client
            .post(CAPTCHA_URL)
            .headers(headers)
            .json(&json!({ "merchantName": "E-Care", "serviceName": "Login", "identifier": self.number }))
            .send()
            .await?;
        let v = http::json_body(resp, "WE captcha").await?;
        ctx.dump("we-captcha", &v);

        let status = j::text(&v["status"]);
        let token = j::text(&v["token"]).unwrap_or_default();
        tracing::info!(
            status = status.as_deref().unwrap_or("-"),
            require_interaction = j::truthy(&v["requireInteraction"]),
            "WE captcha"
        );
        if status.as_deref().is_some_and(|s| s.eq_ignore_ascii_case("blocked")) || token.is_empty() {
            let until = Utc::now() + chrono::Duration::from_std(BLOCK_BACKOFF).unwrap_or_default();
            return Err(FetchError::CaptchaBlocked(until));
        }
        let image = if j::truthy(&v["requireInteraction"]) {
            v["captcha"].as_str().and_then(decode_data_uri)
        } else {
            None
        };
        Ok(CaptchaChallenge { token, image, status })
    }

    /// Sign in. `code` is the solved captcha text when the challenge carried an image.
    pub async fn sign_in(
        &self,
        password: &str,
        challenge: &CaptchaChallenge,
        code: Option<&str>,
        ctx: &FetchContext<'_>,
    ) -> Result<Authenticated, FetchError> {
        let mut payload = json!({
            "acctId": self.fbb_id,
            "password": password,
            "appLocale": "en-US",
            "isSelfcare": "Y",
            "isMobile": "N",
            "imgCacheKey": challenge.token,
        });
        if let Some(code) = code {
            payload["imgCode"] = Value::String(code.to_string());
        }
        let resp = self
            .client
            .post(format!("{API}/v1/auth/userAuthenticate"))
            .headers(api_headers(None))
            .json(&payload)
            .send()
            .await?;
        let v = http::json_body(resp, "WE sign-in").await?;
        ctx.dump("we-auth", &v);
        if !ret_ok(&v) {
            return Err(classify_auth_failure(&v, code.is_some()));
        }
        let body = &v["body"];
        let csrf_token = j::text(&body["token"]).ok_or_else(|| missing("body.token"))?;
        let subscriber_id = j::text(&body["subscriber"]["subscriberId"]).ok_or_else(|| missing("body.subscriber.subscriberId"))?;
        let account_id = j::text(&body["account"]["acctId"]).ok_or_else(|| missing("body.account.acctId"))?;
        let subscriber = &body["subscriber"];
        Ok(Authenticated {
            csrf_token,
            subscriber_id,
            account_id,
            owner_name: owner_name(body, &self.number),
            payment: match j::text(&subscriber["paymentType"]).as_deref() {
                Some("0") => Some(PaymentKind::Prepaid),
                Some("1") => Some(PaymentKind::Postpaid),
                _ => None,
            },
            // Huawei BES subscriber status 2 is "active"; other codes aren't shown.
            status: (j::text(&subscriber["status"]).as_deref() == Some("2")).then(|| "Active".to_string()),
        })
    }

    /// Read balance, package and quotas.
    pub async fn read_usage(&self, auth: &Authenticated, ctx: &FetchContext<'_>) -> Result<Usage, FetchError> {
        let headers = api_headers(Some(&auth.csrf_token));

        // Balance is nice-to-have; don't fail the refresh over it.
        let balance_egp = match self
            .client
            .post(format!("{API}/cbs/ar/queryBalance"))
            .headers(headers.clone())
            .json(&json!({ "acctId": auth.account_id }))
            .send()
            .await
        {
            Ok(resp) => match http::json_body(resp, "WE balance").await {
                Ok(v) => {
                    ctx.dump("we-balance", &v);
                    ret_ok(&v)
                        .then(|| j::num(&v["body"]["balanceInfo"][0]["totalAmount"]))
                        .flatten()
                        .map(egp)
                }
                Err(e) => {
                    tracing::debug!("WE balance unreadable: {e}");
                    None
                }
            },
            Err(e) => {
                tracing::debug!("WE balance failed: {e}");
                None
            }
        };

        let resp = self
            .client
            .post(format!("{API}/cz/v1/auth/getSubscribedOfferings"))
            .headers(headers.clone())
            .json(&json!({ "msisdn": self.fbb_id, "numberServiceType": "FBB" }))
            .send()
            .await?;
        let offers = http::json_body(resp, "WE offerings").await?;
        ctx.dump("we-offerings", &offers);
        if !ret_ok(&offers) {
            return Err(carrier_error(&offers, "offerings"));
        }
        let offering = &offers["body"]["offeringList"][0];
        let main_offer_id = j::text(&offering["mainOfferingId"]).ok_or_else(|| missing("offeringList[0].mainOfferingId"))?;
        let package_name = j::find_text(offering, &["mainOfferingName", "offeringName", "mainOfferingDesc", "offeringDesc"]);

        let resp = self
            .client
            .post(format!("{API}/cz/cbs/bb/queryFreeUnit"))
            .headers(headers.clone())
            .json(&json!({ "subscriberId": auth.subscriber_id, "mainOfferId": main_offer_id }))
            .send()
            .await?;
        let units = http::json_body(resp, "WE quota").await?;
        ctx.dump("we-free-units", &units);
        if !ret_ok(&units) {
            return Err(carrier_error(&units, "quota"));
        }
        let mut usage = build_usage(&units, package_name, balance_egp, auth.owner_name.clone())?;

        // Extras. Neither is essential, so failures only leave them out.
        let (plan, since) = self.line_product(auth, &headers, ctx).await.unwrap_or_default();
        usage.info = LineInfo { plan, payment: auth.payment, status: auth.status.clone(), number: None, since };
        usage.payments = self.recharges(auth, &headers, ctx).await;
        Ok(usage)
    }

    /// The line's product ("WE IOE VDSL Primary Prepaid") and activation date.
    async fn line_product(
        &self,
        auth: &Authenticated,
        headers: &HeaderMap,
        ctx: &FetchContext<'_>,
    ) -> Option<(Option<String>, Option<chrono::DateTime<Utc>>)> {
        let resp = self
            .client
            .post(format!("{API}/cz/v1/customer/querySubscribers"))
            .headers(headers.clone())
            .json(&json!({ "subscriberId": auth.subscriber_id, "startNum": 0, "pageSize": 10 }))
            .send()
            .await
            .ok()?;
        let v = http::json_body(resp, "WE subscribers").await.ok()?;
        ctx.dump("we-subscribers", &v);
        let list = v["body"]["subscriberList"].as_array()?;
        let line = list.iter().find(|s| j::text(&s["subscriberId"]).as_deref() == Some(&auth.subscriber_id)).or(list.first())?;
        Some((j::text(&line["primaryOfferingName"]), j::epoch_ms(&line["activeDate"])))
    }

    /// Recharges over the last 90 days (30 if WE refuses the longer window).
    async fn recharges(&self, auth: &Authenticated, headers: &HeaderMap, ctx: &FetchContext<'_>) -> Option<Vec<Payment>> {
        let now = Utc::now().timestamp_millis();
        for days in [90i64, 30] {
            let body = json!({
                "subscriberId": auth.subscriber_id,
                "startTime": now - days * 86_400_000,
                "endTime": now,
                "startNum": 0,
                "pageSize": 20,
                "channelIds": RECHARGE_CHANNELS,
            });
            let resp = self.client.post(format!("{API}/cbs/ar/queryRechargeLogCz")).headers(headers.clone()).json(&body).send().await.ok()?;
            let v = http::json_body(resp, "WE recharges").await.ok()?;
            ctx.dump("we-recharges", &v);
            if let Some(list) = parse_recharges(&v) {
                return Some(list);
            }
        }
        None
    }
}

/// Channels the website asks `queryRechargeLogCz` about (all of them).
const RECHARGE_CHANNELS: [&str; 15] = ["A", "0", "1", "2", "3", "4", "5", "6", "7", "11", "12", "13", "601", "702", "704"];

/// `queryRechargeLogCz` entries. Amounts use the same 1/10000 EGP scale as `queryBalance`;
/// `rechargeMethod` codes come from WE's `CZ_TRANSACTION_TYPE` dictionary.
pub(crate) fn parse_recharges(v: &Value) -> Option<Vec<Payment>> {
    if !ret_ok(v) {
        return None;
    }
    let mut out: Vec<Payment> = v["body"]["rechargeList"]
        .as_array()?
        .iter()
        .filter_map(|r| {
            let method = j::text(&r["rechargeMethod"]).map(|m| match m.as_str() {
                "0" => "Voucher".to_string(),
                "1" => "Cash".to_string(),
                "2" => "EVC".to_string(),
                "3" => "Cash rollback".to_string(),
                _ => m,
            });
            Some(Payment {
                at: j::epoch_ms(&r["tradeTime"])?,
                amount_egp: egp(j::num(&r["rechargeAmount"])?),
                method,
                reference: j::text(&r["transId"]),
                balance_after: j::num(&r["newBalanceAmount"]).map(egp),
            })
        })
        .collect();
    out.sort_by_key(|p| std::cmp::Reverse(p.at));
    Some(out)
}

/// WE's money amounts are in 1/10000 EGP.
fn egp(amount: f64) -> f64 {
    (amount / 10_000.0 * 100.0).round() / 100.0
}

/// Full refresh: warm up, captcha, sign in, read.
pub async fn fetch(number: &str, password: &str, ctx: &FetchContext<'_>) -> Result<Usage, FetchError> {
    let session = WeSession::new(number)?;
    session.warm_up().await;
    let first = session.captcha(ctx).await?;
    let s = &session;
    let reroll = move || async move {
        tokio::time::sleep(REROLL_PAUSE).await;
        s.captcha(ctx).await
    };
    let (challenge, code) = solve_captcha(first, reroll, ctx).await?;
    let auth = session.sign_in(password, &challenge, code.as_deref(), ctx).await?;
    session.read_usage(&auth, ctx).await
}

/// Get a code for WE's captcha. The solver sees up to 1 + [`CAPTCHA_REROLLS`] captchas and
/// answers the first one it's sure of; a person is asked only if it declines them all.
/// Returns the challenge to sign in with (the latest one: WE only honours that).
async fn solve_captcha<F, Fut>(
    mut challenge: CaptchaChallenge,
    mut reroll: F,
    ctx: &FetchContext<'_>,
) -> Result<(CaptchaChallenge, Option<String>), FetchError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<CaptchaChallenge, FetchError>>,
{
    let answer = |code: Option<String>| code.map(|c| clean_code(&c)).filter(|c| !c.is_empty());
    if let Some(solver) = ctx.solver {
        for attempt in 0..=CAPTCHA_REROLLS {
            if attempt > 0 {
                challenge = reroll().await?;
            }
            let Some(image) = challenge.image.as_deref() else { return Ok((challenge, None)) };
            if let Some(code) = answer(solver.solve(image).await) {
                tracing::info!(solver = solver.name(), rerolls = attempt, "WE captcha read");
                return Ok((challenge, Some(code)));
            }
        }
        tracing::info!(captchas = CAPTCHA_REROLLS + 1, "WE captcha: the solver declined every one");
    }
    let Some(image) = challenge.image.as_deref() else { return Ok((challenge, None)) };
    let ask = ctx.ask.ok_or(FetchError::CaptchaUnsolved)?;
    let code = answer(ask.solve(image).await).ok_or(FetchError::CaptchaUnsolved)?;
    Ok((challenge, Some(code)))
}

fn cors_headers() -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(ACCEPT, HeaderValue::from_static("application/json, text/plain, */*"));
    h.insert(ORIGIN, HeaderValue::from_static(SITE));
    h.insert(REFERER, HeaderValue::from_static(PORTAL));
    h.insert(HeaderName::from_static("sec-fetch-dest"), HeaderValue::from_static("empty"));
    h.insert(HeaderName::from_static("sec-fetch-mode"), HeaderValue::from_static("cors"));
    h.insert(HeaderName::from_static("sec-fetch-site"), HeaderValue::from_static("same-site"));
    h
}

fn api_headers(csrf_token: Option<&str>) -> HeaderMap {
    let mut h = cors_headers();
    h.insert(HeaderName::from_static("channelid"), HeaderValue::from_static("702"));
    h.insert(HeaderName::from_static("isselfcare"), HeaderValue::from_static("true"));
    if let Some(t) = csrf_token.and_then(|t| HeaderValue::from_str(t).ok()) {
        h.insert(HeaderName::from_static("csrftoken"), t);
    }
    h
}

fn ret_ok(v: &Value) -> bool {
    j::text(&v["header"]["retCode"]).as_deref() == Some("0")
}

fn missing(what: &str) -> FetchError {
    FetchError::Unexpected(format!("WE response has no {what}"))
}

fn carrier_error(v: &Value, what: &str) -> FetchError {
    match j::text(&v["header"]["retMsg"]) {
        Some(msg) => FetchError::Carrier(msg),
        None => FetchError::Unexpected(format!(
            "WE {what} failed (retCode {})",
            j::text(&v["header"]["retCode"]).unwrap_or_else(|| "?".into())
        )),
    }
}

/// WE answers failed sign-ins with a non-zero `retCode` and a human message.
fn classify_auth_failure(v: &Value, sent_code: bool) -> FetchError {
    let msg = j::text(&v["header"]["retMsg"]).unwrap_or_default();
    let m = msg.to_lowercase();
    let mentions = |words: &[&str]| words.iter().any(|w| m.contains(w));
    if mentions(&["captcha", "verification code", "verify code", "image code", "imgcode", "check code"]) {
        return if sent_code { FetchError::CaptchaRejected } else { FetchError::Carrier(msg) };
    }
    if mentions(&["password", "incorrect", "invalid", "wrong", "not exist", "does not match"]) {
        return FetchError::BadCredentials;
    }
    carrier_error(v, "sign-in")
}

/// Registrant name from the sign-in response. WE's field names aren't documented; these
/// follow Huawei BES conventions. Run with `KLUE_DEBUG_DIR` to see what your account returns.
fn owner_name(body: &Value, number: &str) -> Option<String> {
    let candidates = [
        j::find_text(&body["customer"], &["custName", "customerName", "name"]),
        {
            let first = j::find_text(body, &["firstName"]);
            let middle = j::find_text(body, &["middleName"]);
            let last = j::find_text(body, &["lastName"]);
            first.map(|f| [Some(f), middle, last].into_iter().flatten().collect::<Vec<_>>().join(" "))
        },
        j::find_text(body, &["custName", "customerName", "acctName", "accountName", "subscriberName", "userName"]),
    ];
    candidates
        .into_iter()
        .flatten()
        .filter(|n| crate::format::digits(n) != crate::format::digits(number) || n.chars().any(char::is_alphabetic))
        .find_map(|n| tidy_name(&n))
}

/// Parse `queryFreeUnit` into bundles. The main quota is the first one with data left;
/// the add-on takes over once the main quota runs out.
pub(crate) fn parse_free_units(v: &Value) -> Vec<Bundle> {
    let mut out: Vec<Bundle> = Vec::new();
    for group in v["body"].as_array().into_iter().flatten() {
        let group_name = shallow_text(group, &["offeringName", "freeUnitTypeName", "freeUnitName", "freeUnitType"]);
        for detail in group["freeUnitBeanDetailList"].as_array().into_iter().flatten() {
            let (Some(total), Some(remaining)) = (j::num(&detail["initialAmount"]), j::num(&detail["currentAmount"])) else {
                continue;
            };
            let unit_label = shallow_text(detail, &["measureUnitName", "measureUnit", "unitName"])
                .or_else(|| shallow_text(group, &["measureUnit"]))
                .unwrap_or_default();
            let (unit, factor) = unit_of(&unit_label);
            let name = shallow_text(detail, &["offeringName", "freeUnitTypeName", "freeUnitName"])
                .or_else(|| group_name.clone())
                .unwrap_or_else(|| if out.is_empty() { "Home internet".into() } else { "Add-on".into() });
            out.push(Bundle {
                key: String::new(),
                kind: BundleKind::Addon,
                name,
                unit,
                total: total * factor,
                remaining: remaining * factor,
                starts_at: j::epoch_ms(&detail["effectiveTime"]),
                expires_at: j::epoch_ms(&detail["expireTime"]),
            });
        }
    }
    order_bundles(&mut out);
    if let Some(main) = out.first_mut() {
        main.kind = BundleKind::Data;
    }
    assign_keys(&mut out);
    out
}

/// WE's measure units, as names or Huawei CBS codes (1109 GB, 1106 MB, 1108 KB).
fn unit_of(label: &str) -> (Unit, f64) {
    match label.trim().to_ascii_uppercase().as_str() {
        "MB" | "1106" => (Unit::Mb, 1.0),
        "KB" | "1108" => (Unit::Mb, 1.0 / 1024.0),
        "MIN" | "MINUTES" => (Unit::Minutes, 1.0),
        "1003" | "SEC" | "SECONDS" => (Unit::Minutes, 1.0 / 60.0),
        _ => (Unit::Gb, 1.0),
    }
}

fn shallow_text(v: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|k| j::text(&v[*k]))
}

pub(crate) fn build_usage(
    units: &Value,
    package_name: Option<String>,
    balance_egp: Option<f64>,
    owner_name: Option<String>,
) -> Result<Usage, FetchError> {
    let bundles = parse_free_units(units);
    let main = bundles.first().ok_or_else(|| FetchError::Unexpected("WE returned no quota for this line".into()))?;
    Ok(Usage {
        owner_name,
        package_name: package_name.or_else(|| Some(main.name.clone())),
        balance_egp,
        subscribed_at: main.starts_at,
        renews_at: main.expires_at,
        bundles,
        fetched_at: Utc::now(),
        info: LineInfo::default(),
        payments: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Value {
        let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn parses_free_units() {
        let usage = build_usage(&fixture("we_free_units.json"), Some("Super 140".into()), Some(12.5), None).unwrap();
        assert_eq!(usage.bundles.len(), 2);
        let main = usage.main().unwrap();
        assert_eq!(main.kind, BundleKind::Data);
        assert_eq!(main.total, 140.0);
        assert!((main.remaining - 62.4).abs() < 1e-9);
        assert_eq!(usage.extras()[0].name, "Extra 25 GB");
        assert_eq!(usage.renews_at.unwrap().timestamp(), 1_764_892_800);
        assert_eq!(usage.subscribed_at.unwrap().timestamp(), 1_762_300_800);
        assert_eq!(usage.package_name.as_deref(), Some("Super 140"));
    }

    #[test]
    fn falls_back_to_addon_when_main_is_empty() {
        let mut v = fixture("we_free_units.json");
        v["body"][0]["freeUnitBeanDetailList"][0]["currentAmount"] = json!(0);
        let bundles = parse_free_units(&v);
        assert_eq!(bundles[0].name, "Extra 25 GB");
        assert_eq!(bundles[0].kind, BundleKind::Data);
    }

    #[test]
    fn parses_recharges_newest_first() {
        let v = json!({"header": {"retCode": "0"}, "body": {"totalRecord": 2, "rechargeList": [
            {"serviceNumber": "FBB200000000", "rechargeAmount": "4000000", "currencyId": "1046", "channelId": "702",
             "rechargeMethod": "1", "tradeTime": 1793000000000i64, "transId": "T1", "newBalanceAmount": "4685963"},
            {"serviceNumber": "FBB200000000", "rechargeAmount": "1000000", "currencyId": "1046", "channelId": "A",
             "rechargeMethod": "0", "tradeTime": 1795000000000i64, "transId": "T2", "newBalanceAmount": "1685963"}
        ]}});
        let list = parse_recharges(&v).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!((list[0].amount_egp, list[0].method.as_deref(), list[0].balance_after), (100.0, Some("Voucher"), Some(168.6)));
        assert_eq!((list[1].amount_egp, list[1].method.as_deref(), list[1].reference.as_deref()), (400.0, Some("Cash"), Some("T1")));
        assert!(parse_recharges(&json!({"header": {"retCode": "1", "retMsg": "x"}})).is_none());
        let empty = json!({"header": {"retCode": "0"}, "body": {"rechargeList": [], "totalRecord": 0}});
        assert_eq!(parse_recharges(&empty), Some(vec![]));
    }

    #[test]
    fn reads_measure_unit_codes() {
        assert_eq!(unit_of("1109"), (Unit::Gb, 1.0));
        assert_eq!(unit_of("1106"), (Unit::Mb, 1.0));
        assert_eq!(unit_of("MB"), (Unit::Mb, 1.0));
        assert_eq!(unit_of(""), (Unit::Gb, 1.0));
    }

    #[test]
    fn classifies_failures() {
        let bad = json!({"header": {"retCode": "1001", "retMsg": "The password is incorrect."}});
        assert_eq!(classify_auth_failure(&bad, false), FetchError::BadCredentials);
        let cap = json!({"header": {"retCode": "1002", "retMsg": "Verification code error"}});
        assert_eq!(classify_auth_failure(&cap, true), FetchError::CaptchaRejected);
        let other = json!({"header": {"retCode": "9", "retMsg": "System busy"}});
        assert_eq!(classify_auth_failure(&other, false), FetchError::Carrier("System busy".into()));
    }

    /// Reads only the captcha whose image is `[sure_of]`.
    struct Picky {
        sure_of: u8,
        seen: std::sync::Mutex<Vec<u8>>,
    }

    impl crate::captcha::CaptchaSolver for Picky {
        fn name(&self) -> &str {
            "picky"
        }
        fn solve<'a>(&'a self, image: &'a [u8]) -> crate::captcha::BoxFuture<'a, Option<String>> {
            self.seen.lock().unwrap().push(image[0]);
            Box::pin(async move { (image[0] == self.sure_of).then(|| format!("code{}", image[0])) })
        }
    }

    fn picky(sure_of: u8) -> Picky {
        Picky { sure_of, seen: Default::default() }
    }

    fn challenge(n: u8, image: bool) -> CaptchaChallenge {
        CaptchaChallenge { token: format!("t{n}"), image: image.then(|| vec![n]), status: None }
    }

    /// Run `solve_captcha` where WE hands out captchas 0, 1, 2, … (`image: false` from `plain_from` on).
    fn run(solver: Option<&Picky>, ask: Option<&Picky>, plain_from: u8) -> (Result<(CaptchaChallenge, Option<String>), FetchError>, u8) {
        let ctx = FetchContext {
            solver: solver.map(|s| s as &dyn crate::captcha::CaptchaSolver),
            ask: ask.map(|s| s as &dyn crate::captcha::CaptchaSolver),
            debug_dir: None,
        };
        let mut next = 0u8;
        let reroll = || {
            next += 1;
            let n = next;
            async move { Ok(challenge(n, n < plain_from)) }
        };
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let result = rt.block_on(solve_captcha(challenge(0, 0 < plain_from), reroll, &ctx));
        (result, next)
    }

    #[test]
    fn answers_the_first_captcha_it_is_sure_of() {
        let model = picky(0);
        let (result, rerolls) = run(Some(&model), None, u8::MAX);
        let (c, code) = result.unwrap();
        assert_eq!((c.token.as_str(), code.as_deref(), rerolls), ("t0", Some("code0"), 0));
    }

    #[test]
    fn rerolls_until_the_solver_is_sure() {
        let model = picky(3);
        let (result, rerolls) = run(Some(&model), None, u8::MAX);
        let (c, code) = result.unwrap();
        // Signs in with the captcha it read, the latest one.
        assert_eq!((c.token.as_str(), code.as_deref(), rerolls), ("t3", Some("code3"), 3));
        assert_eq!(*model.seen.lock().unwrap(), vec![0, 1, 2, 3]);
    }

    #[test]
    fn asks_a_person_only_after_every_reroll() {
        let (model, person) = (picky(99), picky(CAPTCHA_REROLLS as u8));
        let (result, rerolls) = run(Some(&model), Some(&person), u8::MAX);
        let (c, code) = result.unwrap();
        assert_eq!(rerolls as usize, CAPTCHA_REROLLS);
        assert_eq!(model.seen.lock().unwrap().len(), CAPTCHA_REROLLS + 1);
        assert_eq!((c.token, code), (format!("t{CAPTCHA_REROLLS}"), Some(format!("code{CAPTCHA_REROLLS}"))));
    }

    #[test]
    fn gives_up_without_a_person() {
        let model = picky(99);
        assert_eq!(run(Some(&model), None, u8::MAX).0.unwrap_err(), FetchError::CaptchaUnsolved);
        assert_eq!(run(None, None, u8::MAX).0.unwrap_err(), FetchError::CaptchaUnsolved);
    }

    #[test]
    fn signs_in_without_a_code_when_no_captcha_is_shown() {
        let model = picky(99);
        // WE didn't want a code at all.
        let (c, code) = run(Some(&model), None, 0).0.unwrap();
        assert_eq!((c.token.as_str(), code), ("t0", None));
        // Or stopped wanting one after a reroll.
        let (c, code) = run(Some(&model), None, 2).0.unwrap();
        assert_eq!((c.token.as_str(), code), ("t2", None));
    }

    #[test]
    fn finds_owner_name() {
        let body = json!({"customer": {"custName": "MOSTAFA HASSAN ALI"}, "subscriber": {"subscriberId": 1}});
        assert_eq!(owner_name(&body, "0223584417").as_deref(), Some("Mostafa Hassan Ali"));
        let body = json!({"subscriber": {"firstName": "Mostafa", "lastName": "Ali"}});
        assert_eq!(owner_name(&body, "0223584417").as_deref(), Some("Mostafa Ali"));
        assert_eq!(owner_name(&json!({"account": {"acctName": "0223584417"}}), "0223584417"), None);
    }
}
