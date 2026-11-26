//! WE (Telecom Egypt) home internet, through the my.te.eg self-care API.
//!
//! Sign-in is `GenerateCaptcha` (a token, plus an image once WE wants a human) and then
//! `userAuthenticate`. After that: `queryBalance`, `getSubscribedOfferings`, `queryFreeUnit`,
//! and when they work, `queryRechargeLogCz` and `querySubscribers`.
//!
//! WE's edge drops clients that don't look like a browser, hence Chrome's TLS fingerprint.
//! The web app on my.te.eg signs its requests; api-my.te.eg still takes unsigned ones.

use std::time::Duration;

use chrono::Utc;
use serde_json::{Value, json};
use wreq::header::{ACCEPT, ACCEPT_LANGUAGE, HeaderMap, HeaderName, HeaderValue, ORIGIN, REFERER};

use super::FetchContext;
use crate::{
    Bundle, BundleKind, BundleState, FetchError, LineType, Unit, Usage,
    model::{LineInfo, Payment, PaymentKind, Speed, order_bundles},
    captcha::{clean_code, decode_data_uri},
    format::{self, tidy_name},
    http,
    json::{self as j},
    model::assign_keys,
};

const PORTAL: &str = "https://my.te.eg/";
const SITE: &str = "https://my.te.eg";
const API: &str = "https://api-my.te.eg/echannel/service/besapp/base/rest/busiservice";
const CAPTCHA_URL: &str = "https://captcha.te.eg/api/Captcha/GenerateCaptcha";

/// Extra captchas to try when the solver isn't sure. Fetching one is free; a wrong code
/// costs a sign-in attempt.
pub const CAPTCHA_REROLLS: usize = 8;
/// Delay between rerolls.
const REROLL_PAUSE: Duration = Duration::from_millis(600);

/// How long WE blocks captchas, or sign-ins after too many wrong ones.
pub const BLOCK_BACKOFF: Duration = Duration::from_secs(3700);

#[derive(Debug, Clone)]
pub struct CaptchaChallenge {
    pub token: String,
    /// JPEG bytes, when WE wants the code typed.
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

/// One sign-in, with its own cookie jar.
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

    /// Seeds cookies like the website does. Errors don't matter here.
    pub async fn warm_up(&self) {
        if let Err(e) = self.client.get(PORTAL).send().await {
            tracing::debug!("WE portal seed failed: {e}");
        }
        let url = format!("{API}/v1/common/querySysParams");
        if let Err(e) = self.client.post(url).headers(api_headers(None)).json(&json!({})).send().await {
            tracing::debug!("WE querySysParams failed: {e}");
        }
    }

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

    /// `code` is the captcha answer, when the challenge had an image.
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
            // BES status 2 is active; other codes aren't shown.
            status: (j::text(&subscriber["status"]).as_deref() == Some("2")).then(|| "Active".to_string()),
        })
    }

    pub async fn read_usage(&self, auth: &Authenticated, ctx: &FetchContext<'_>) -> Result<Usage, FetchError> {
        let headers = api_headers(Some(&auth.csrf_token));

        // Balance is optional.
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
        let package = main_offering(&offers).ok_or_else(|| missing("offeringList[].mainOfferingId"))?;

        let resp = self
            .client
            .post(format!("{API}/cz/cbs/bb/queryFreeUnit"))
            .headers(headers.clone())
            .json(&json!({ "subscriberId": auth.subscriber_id, "mainOfferId": package.id }))
            .send()
            .await?;
        let units = http::json_body(resp, "WE quota").await?;
        ctx.dump("we-free-units", &units);
        if !ret_ok(&units) {
            return Err(carrier_error(&units, "quota"));
        }
        let mut usage = build_usage(&units, Some(&package), balance_egp, auth.owner_name.clone())?;

        // Optional extras.
        let (plan, since) = self.line_product(auth, &headers, ctx).await.unwrap_or_default();
        usage.info = LineInfo { plan, plan_ar: None, speed: package.speed, payment: auth.payment, status: auth.status.clone(), number: None, since, valid_until: None };
        usage.payments = self.recharges(auth, &headers, ctx).await;
        Ok(usage)
    }

    /// Product ("WE IOE VDSL Primary Prepaid") and activation date.
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

    /// Recharges in the last 90 days (30 if WE refuses 90).
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

/// The channels the website sends to `queryRechargeLogCz`.
const RECHARGE_CHANNELS: [&str; 15] = ["A", "0", "1", "2", "3", "4", "5", "6", "7", "11", "12", "13", "601", "702", "704"];

/// Amounts are in 1/10000 EGP, like `queryBalance`; `rechargeMethod` codes are from WE's
/// `CZ_TRANSACTION_TYPE` dictionary.
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

/// WE amounts are in 1/10000 EGP.
fn egp(amount: f64) -> f64 {
    (amount / 10_000.0 * 100.0).round() / 100.0
}

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

/// Up to 1 + [`CAPTCHA_REROLLS`] captchas go to the solver before the user is asked.
/// Returns the latest challenge, the only one WE accepts.
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

/// `retMsg`, else `errorMsg` or `description`.
fn message(v: &Value) -> Option<String> {
    let header = &v["header"];
    ["retMsg", "errorMsg", "description"].iter().find_map(|k| j::text(&header[*k]))
}

fn carrier_error(v: &Value, what: &str) -> FetchError {
    match message(v) {
        Some(msg) => FetchError::Carrier(msg),
        None => FetchError::Unexpected(format!(
            "WE {what} failed (retCode {})",
            j::text(&v["header"]["retCode"]).unwrap_or_else(|| "?".into())
        )),
    }
}

fn classify_auth_failure(v: &Value, sent_code: bool) -> FetchError {
    let msg = message(v).unwrap_or_default();
    let m = msg.to_lowercase();
    let mentions = |words: &[&str]| words.iter().any(|w| m.contains(w));
    // Too many wrong sign-ins lock the line for an hour, and retrying inside that hour can
    // keep it locked.
    if mentions(&["maximum number", "too many", "try again after", "الحد الأقصى"]) {
        return FetchError::CaptchaBlocked(Utc::now() + chrono::Duration::from_std(BLOCK_BACKOFF).unwrap_or_default());
    }
    if mentions(&["captcha", "verification code", "verify code", "image code", "imgcode", "check code"]) {
        return if sent_code { FetchError::CaptchaRejected } else { FetchError::Carrier(msg) };
    }
    if mentions(&["password", "incorrect", "invalid", "wrong", "not exist", "does not match"]) {
        return FetchError::BadCredentials;
    }
    carrier_error(v, "sign-in")
}

/// WE's field names aren't documented; these are the usual Huawei BES ones. Set
/// `KLUE_DEBUG_DIR` to see what an account returns.
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

/// The package from `getSubscribedOfferings`: the entry flagged `main`. The list also has
/// add-ons, in no particular order.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct MainOffering {
    /// What `queryFreeUnit` wants as `mainOfferId`.
    pub id: String,
    pub name: Option<String>,
    /// `offerArName`, if it's actually Arabic.
    pub name_ar: Option<String>,
    pub speed: Option<Speed>,
    pub subscribed_at: Option<chrono::DateTime<Utc>>,
    pub renews_at: Option<chrono::DateTime<Utc>>,
}

pub(crate) fn main_offering(v: &Value) -> Option<MainOffering> {
    let list = v["body"]["offeringList"].as_array()?;
    let flagged = list.iter().find(|o| j::truthy(&o["main"]));
    let id = flagged
        .and_then(|o| j::text(&o["mainOfferingId"]).or_else(|| j::text(&o["offerId"])))
        .or_else(|| list.iter().find_map(|o| j::text(&o["mainOfferingId"])))?;
    let main = flagged.or_else(|| list.iter().find(|o| j::text(&o["offerId"]).as_deref() == Some(id.as_str())));
    Some(MainOffering {
        name: main.and_then(|o| shallow_text(o, &["offerEnName", "offeringName", "offerArName"])),
        name_ar: main.and_then(|o| shallow_text(o, &["offerArName"])).and_then(|n| format::arabic(&n)),
        speed: main.and_then(offering_speed),
        subscribed_at: main.and_then(|o| j::epoch_ms(&o["subscriptionDate"])),
        renews_at: main.and_then(|o| j::epoch_ms(&o["renewalDate"])),
        id,
    })
}

/// Speeds in kbit/s: `C_O_DOWNLOAD_RATE_NORMAL` (30720 = 30 Mbit/s), `C_O_UPLOAD_RATE_NORMAL`
/// and `C_O_DOWNLOAD_RATE_DEGRADE` (after the quota).
fn offering_speed(offering: &Value) -> Option<Speed> {
    let properties = offering["property"].as_array()?;
    let rate = |id: &str| {
        properties
            .iter()
            .find(|p| j::text(&p["id"]).is_some_and(|i| i.eq_ignore_ascii_case(id)))
            .and_then(|p| j::num(&p["value"]))
            .filter(|v| *v > 0.0 && *v < f64::from(u32::MAX))
            .map(|v| v.round() as u32)
    };
    Some(Speed {
        down_kbps: rate("C_O_DOWNLOAD_RATE_NORMAL")?,
        up_kbps: rate("C_O_UPLOAD_RATE_NORMAL"),
        throttled_kbps: rate("C_O_DOWNLOAD_RATE_DEGRADE"),
    })
}

fn free_unit_details(v: &Value) -> impl Iterator<Item = (&Value, &Value)> {
    v["body"].as_array().into_iter().flatten().flat_map(|group| {
        group["freeUnitBeanDetailList"].as_array().into_iter().flatten().map(move |detail| (group, detail))
    })
}

/// The package quota has `originType: "Main Quota"`; top-ups have `"WE TOPUP"`.
fn is_main_quota(detail: &Value) -> bool {
    match shallow_text(detail, &["originType"]) {
        Some(origin) => origin.eq_ignore_ascii_case("main quota"),
        None => shallow_text(detail, &["itemCode"]).is_some_and(|c| c.contains("_Primary_")),
    }
}

/// Bundles from `queryFreeUnit`, largest with something left first. If WE doesn't mark the
/// package quota, the first bundle is taken as it.
pub(crate) fn parse_free_units(v: &Value) -> Vec<Bundle> {
    let mut out: Vec<Bundle> = Vec::new();
    for (group, detail) in free_unit_details(v) {
        let (Some(total), Some(remaining)) = (j::num(&detail["initialAmount"]), j::num(&detail["currentAmount"])) else {
            continue;
        };
        let unit_label = shallow_text(detail, &["measureUnitName", "measureUnit", "unitName"])
            .or_else(|| shallow_text(group, &["measureUnit"]))
            .unwrap_or_default();
        let (unit, factor) = unit_of(&unit_label);
        // Unnamed top-ups are named after their origin; the group label belongs to the package.
        let name = shallow_text(detail, &["offeringName", "freeUnitTypeName", "freeUnitName"])
            .or_else(|| shallow_text(detail, &["originType"]).filter(|_| !is_main_quota(detail)).map(|o| tidy_label(&o)))
            .or_else(|| shallow_text(group, &["offeringName", "freeUnitTypeName", "freeUnitName", "freeUnitType"]))
            .unwrap_or_else(|| if out.is_empty() { "Home internet".into() } else { "Add-on".into() });
        out.push(Bundle {
            key: String::new(),
            kind: if is_main_quota(detail) { BundleKind::Data } else { BundleKind::Addon },
            name,
            name_ar: None,
            unit,
            total: total * factor,
            remaining: remaining * factor,
            starts_at: j::epoch_ms(&detail["effectiveTime"]),
            expires_at: j::epoch_ms(&detail["expireTime"]),
            state: BundleState::Active,
            settle_by: None,
        });
    }
    order_bundles(&mut out);
    if !out.iter().any(|b| b.kind == BundleKind::Data)
        && let Some(main) = out.first_mut()
    {
        main.kind = BundleKind::Data;
    }
    assign_keys(&mut out);
    out
}

/// "WE TOPUP " -> "WE Topup".
fn tidy_label(raw: &str) -> String {
    raw.split_whitespace()
        .map(|w| {
            if w.chars().count() > 2 && w.chars().any(char::is_alphabetic) && !w.chars().any(char::is_lowercase) {
                let mut cs = w.chars();
                cs.next().map(|f| f.to_string() + &cs.as_str().to_lowercase()).unwrap_or_default()
            } else {
                w.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Unit names, or CBS codes (1109 GB, 1106 MB, 1108 KB).
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

/// Name and cycle dates come from the package quota, not a bigger top-up. Only the package
/// has an Arabic name.
pub(crate) fn build_usage(
    units: &Value,
    package: Option<&MainOffering>,
    balance_egp: Option<f64>,
    owner_name: Option<String>,
) -> Result<Usage, FetchError> {
    let mut bundles = parse_free_units(units);
    let offering = package.cloned().unwrap_or_default();
    let has_quota = free_unit_details(units).any(|(_, d)| is_main_quota(d));
    if has_quota && let Some(quota) = bundles.iter_mut().find(|b| b.kind == BundleKind::Data) {
        quota.name_ar = offering.name_ar.clone();
    }
    let first = bundles.first().ok_or_else(|| FetchError::Unexpected("WE returned no quota for this line".into()))?;
    let quota = has_quota.then(|| bundles.iter().find(|b| b.kind == BundleKind::Data)).flatten();
    Ok(Usage {
        owner_name,
        package_name: quota.map(|q| q.name.clone()).or(offering.name).or_else(|| Some(first.name.clone())),
        package_name_ar: offering.name_ar,
        balance_egp,
        owed_egp: None,
        subscribed_at: quota.and_then(|q| q.starts_at).or(offering.subscribed_at).or(first.starts_at),
        renews_at: quota.and_then(|q| q.expires_at).or(offering.renews_at).or(first.expires_at),
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
        // An older shape that doesn't say which bucket is the package's: the first is.
        let package = MainOffering { id: "1".into(), name: Some("Super 140".into()), ..Default::default() };
        let usage = build_usage(&fixture("we_free_units.json"), Some(&package), Some(12.5), None).unwrap();
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
    fn finds_the_main_offering_anywhere_in_the_list() {
        // The list starts with a top-up here; the package is the entry flagged `main`.
        let offers = fixture("we_offerings.json");
        let p = main_offering(&offers).unwrap();
        assert_eq!((p.id.as_str(), p.name.as_deref()), ("820058", Some("Super speed 3- (250GB)")));
        assert_eq!(p.renews_at.unwrap().to_rfc3339(), "2026-12-12T22:00:00+00:00");
        assert_eq!(p.subscribed_at.unwrap().to_rfc3339(), "2026-11-12T22:00:00+00:00");

        // No flag: use the id every entry gives as `mainOfferingId`.
        let mut unflagged = offers.clone();
        for o in unflagged["body"]["offeringList"].as_array_mut().unwrap() {
            o.as_object_mut().unwrap().remove("main");
        }
        assert_eq!(main_offering(&unflagged).unwrap(), p);
        assert!(main_offering(&json!({"header": {"retCode": "0"}, "body": {"offeringList": []}})).is_none());
    }

    #[test]
    fn package_and_dates_come_from_the_main_quota_not_a_bigger_topup() {
        let package = main_offering(&fixture("we_offerings.json"));
        let usage = build_usage(&fixture("we_free_units_topups.json"), package.as_ref(), None, None).unwrap();
        let rows: Vec<_> = usage.bundles.iter().map(|b| (b.name.as_str(), b.kind, b.key.as_str())).collect();
        // Largest first (the 400 GB top-up); the unnamed top-up is named after its origin.
        assert_eq!(
            rows,
            [
                ("Giga Tank - 400 GB", BundleKind::Addon, "giga-tank-400-gb"),
                ("Super 3 speed (250 GB)", BundleKind::Data, "super-3-speed-250-gb"),
                ("WE Topup", BundleKind::Addon, "we-topup"),
            ]
        );
        assert_eq!(usage.package_name.as_deref(), Some("Super 3 speed (250 GB)"));
        assert_eq!(usage.renews_at.unwrap().to_rfc3339(), "2026-12-12T22:00:00+00:00");
        assert_eq!(usage.subscribed_at.unwrap().to_rfc3339(), "2026-11-13T12:57:00+00:00");

        // No main quota left in the list: name and dates from the subscribed offering.
        let mut topups_only = fixture("we_free_units_topups.json");
        topups_only["body"][0]["freeUnitBeanDetailList"].as_array_mut().unwrap().remove(1);
        let usage = build_usage(&topups_only, package.as_ref(), None, None).unwrap();
        assert_eq!(usage.package_name.as_deref(), Some("Super speed 3- (250GB)"));
        assert_eq!(usage.renews_at.unwrap().to_rfc3339(), "2026-12-12T22:00:00+00:00");
        assert_eq!(usage.bundles[0].kind, BundleKind::Data);
    }

    #[test]
    fn tidies_origin_labels() {
        assert_eq!(tidy_label("WE TOPUP "), "WE Topup");
        assert_eq!(tidy_label("Main Quota"), "Main Quota");
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
        // Messages can come as `errorMsg` instead of `retMsg`.
        let busy = json!({"header": {"retCode": "9999", "errorMsg": "System busy"}});
        assert_eq!(classify_auth_failure(&busy, false), FetchError::Carrier("System busy".into()));
    }

    #[test]
    fn too_many_wrong_sign_ins_pause_them_for_an_hour() {
        // Not a wrong password, though the message says "incorrect".
        match classify_auth_failure(&fixture("we_auth_locked.json"), true) {
            FetchError::CaptchaBlocked(until) => {
                let wait = until - Utc::now();
                assert!(wait > chrono::Duration::minutes(59) && wait < chrono::Duration::minutes(65), "{wait}");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn reads_the_package_speed() {
        // "Super speed 2-(400GB)": 30 Mbit/s down, 3 up, 256 kbit/s after the quota.
        let package = main_offering(&fixture("we_offerings.json")).unwrap();
        assert_eq!(package.speed, Some(Speed { down_kbps: 30720, up_kbps: Some(3072), throttled_kbps: Some(256) }));
        // Packages without the properties have no speed.
        let bare = json!({"body": {"offeringList": [{"main": true, "offerId": "1", "offerEnName": "X", "property": [{"id": "C_O_MONTHLY_FEE", "value": "0"}]}]}});
        assert_eq!(main_offering(&bare).unwrap().speed, None);
    }

    #[test]
    fn the_package_and_its_quota_take_the_arabic_offering_name() {
        let offers = fixture("we_offerings.json");
        let package = main_offering(&offers).unwrap();
        assert_eq!(package.name_ar.as_deref(), Some("سرعة سوبر 3-(250 جيجابايت)"));
        let usage = build_usage(&fixture("we_free_units_topups.json"), Some(&package), None, None).unwrap();
        assert_eq!(usage.package_in(crate::store::Lang::Ar), Some("سرعة سوبر 3-(250 جيجابايت)"));
        assert_eq!(usage.package_in(crate::store::Lang::En), Some("Super 3 speed (250 GB)"));
        let quota = usage.bundles.iter().find(|b| b.kind == BundleKind::Data).unwrap();
        assert_eq!(quota.name_in(crate::store::Lang::Ar), "سرعة سوبر 3-(250 جيجابايت)");
        // WE's "Arabic" names for top-ups are English.
        let topup = usage.bundles.iter().find(|b| b.name == "Giga Tank - 400 GB").unwrap();
        assert_eq!((topup.name_ar.as_deref(), topup.name_in(crate::store::Lang::Ar)), (None, "Giga Tank - 400 GB"));
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

    /// Runs `solve_captcha` on fake captchas 0, 1, 2...; from `plain_from` on they have no image.
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
