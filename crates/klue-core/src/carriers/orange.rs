//! Orange Egypt, through the API of the My Orange Android app (10.4.3).
//!
//! `SignIn.svc/SignInUser` trades the number and password for a JWT, sent as a `Token`
//! header after that. A mobile line reads `GetDialInfo` on `backend.orange.eg` (rate plan and
//! balance), its units from `PostpaidInfoService.svc/GetConsumption` and its data from
//! `BucketsService.svc/GetCurrentActiveBuckets`. Home internet is a separate API under
//! `APIs/DSL/Account`: `GetProfileInfo` names the landline linked to the account and gives a
//! `DSLToken` for `GetConsumption`, which has the package and its add-ons, in bytes.
//!
//! Orange's F5 firewall answers clients that don't look like the app with a "Request
//! Rejected" page, hence okhttp's TLS and HTTP/2 fingerprint and the app's headers. It also
//! blocks an address that signs in too often, for 15 to 30 minutes. So tokens (good for about
//! 15 minutes) are kept in memory and renewed with their refresh token rather than signing in
//! again, and a rejection pauses every Orange request for [`BLOCK_BACKOFF`].

use std::{collections::BTreeMap, sync::Mutex, time::Duration};

use base64::Engine;
use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};
use serde_json::{Value, json};
use wreq::header::{CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};

use super::FetchContext;
use crate::{
    Bundle, BundleKind, BundleState, FetchError, LineType, Unit, Usage,
    format::{self, tidy_name},
    http, json as j,
    model::{LineInfo, PaymentKind, assign_keys, order_bundles},
};

const SERVICES: &str = "https://services.orange.eg";
const BACKEND: &str = "https://backend.orange.eg";
const APP_VERSION: &str = "10.4.3";
const OS_VERSION: &str = "14";
/// The app's own channel credentials, the same for everyone (not an account's password).
const CHANNEL_NAME: &str = "MobinilAndMe";
const CHANNEL_PASSWORD: &str = "ig3yh*mk5l42@oj7QAR8yF";

/// How long Orange is left alone after its firewall rejects a request. Requests during a
/// block extend it.
pub const BLOCK_BACKOFF: Duration = Duration::from_secs(30 * 60);
/// Tokens are renewed this many seconds before they expire.
const TOKEN_MARGIN: i64 = 60;
const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

/// Signed-in accounts by number, in memory only.
static SESSIONS: Mutex<BTreeMap<String, Session>> = Mutex::new(BTreeMap::new());
/// While Orange's firewall is left alone.
static BLOCKED: Mutex<Option<DateTime<Utc>>> = Mutex::new(None);

/// `number` is the account's My Orange number, for both line types. A home internet line's
/// landline is in [`LineInfo::number`].
pub async fn fetch(line: LineType, number: &str, password: &str, ctx: &FetchContext<'_>) -> Result<Usage, FetchError> {
    let dial = format::normalize(number, LineType::Mobile)?;
    if let Some(until) = blocked_until() {
        tracing::info!("Orange is paused until {until} after a firewall rejection");
        return Err(FetchError::CaptchaBlocked(until));
    }
    let client = http::okhttp_client()?;
    let (mut session, mut kept) = open(&client, &dial, password, ctx).await?;
    loop {
        let read = match line {
            LineType::Mobile => read_mobile(&client, &session, &dial, password, ctx).await,
            LineType::Landline => read_home_internet(&client, &session, &dial, password, ctx).await,
        };
        match read {
            // A kept token can stop working before it expires: sign in once more.
            Err(CallError::Token) if kept => {
                tracing::info!("Orange refused a kept token; signing in again");
                forget(&dial);
                session = sign_in(&client, &dial, password, ctx).await?;
                kept = false;
            }
            other => return other.map_err(CallError::into_fetch_error),
        }
    }
}

// ---------------------------------------------------------------------------------------
// Signing in

/// A signed-in account.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Session {
    /// The password it was opened with; another one has to sign in again.
    password: String,
    access: String,
    refresh: Option<String>,
    /// The token's `exp`, in Unix seconds.
    expires: i64,
    user_id: Option<String>,
    owner_name: Option<String>,
}

/// The kept session, renewed if it's about to expire, else a new sign-in. The flag says the
/// session was kept rather than just signed in.
async fn open(client: &wreq::Client, dial: &str, password: &str, ctx: &FetchContext<'_>) -> Result<(Session, bool), FetchError> {
    let kept = SESSIONS.lock().ok().and_then(|s| s.get(dial).filter(|s| s.password == password).cloned());
    if let Some(kept) = kept {
        if kept.expires - TOKEN_MARGIN > Utc::now().timestamp() {
            tracing::debug!("Orange: reusing the token");
            return Ok((kept, true));
        }
        if let Some(refresh) = kept.refresh.as_deref() {
            match renew(client, dial, password, refresh, &kept, ctx).await {
                Ok(session) => return Ok((session, true)),
                Err(e @ FetchError::CaptchaBlocked(_)) => return Err(e),
                Err(e) => tracing::info!("Orange: couldn't renew the token ({e}); signing in again"),
            }
        }
    }
    Ok((sign_in(client, dial, password, ctx).await?, false))
}

async fn sign_in(client: &wreq::Client, dial: &str, password: &str, ctx: &FetchContext<'_>) -> Result<Session, FetchError> {
    let body = object(&[
        ("dialNumber", json!(dial)),
        ("password", json!(password)),
        ("channel", json!({ "ChannelName": CHANNEL_NAME, "ChannelPassword": CHANNEL_PASSWORD })),
        ("lang", json!("en-US")),
        ("isAndroid", json!(true)),
        ("appVersion", json!(APP_VERSION)),
    ]);
    let url = format!("{SERVICES}/SignIn.svc/SignInUser");
    let v = post(client, &url, body, headers(None, false), "sign-in").await.map_err(CallError::into_fetch_error)?;
    ctx.dump("or-signin", &v);
    let session = parse_sign_in(&v, password)?;
    keep(dial, &session);
    Ok(session)
}

pub(crate) fn parse_sign_in(v: &Value, password: &str) -> Result<Session, FetchError> {
    let r = result(v, "SignInUserResult");
    let access = match (error_code(r), j::text(&r["AccessToken"])) {
        (None | Some(0), Some(access)) => access,
        _ => return Err(sign_in_error(r)),
    };
    let user = &r["UserData"];
    Ok(Session {
        password: password.to_string(),
        expires: token_expiry(&access),
        access,
        refresh: j::text(&r["RefreshToken"]),
        user_id: j::text(&user["UserID"]),
        owner_name: person(user),
    })
}

/// A failed sign-in. A wrong password is code 9, "invalid username and password"; a number
/// Orange doesn't know is code 16, "service down", which is left as Orange's words since it
/// can be an outage too. Other codes aren't documented, so this goes by the message.
fn sign_in_error(r: &Value) -> FetchError {
    let m = j::text(&r["ErrorDescription"]).unwrap_or_default().to_lowercase();
    let mentions = |words: &[&str]| words.iter().any(|w| m.contains(w));
    if mentions(&["too many", "locked", "exceeded"]) {
        return FetchError::CaptchaBlocked(Utc::now() + chrono::Duration::from_std(BLOCK_BACKOFF).unwrap_or_default());
    }
    if error_code(r) == Some(9) || mentions(&["password", "incorrect", "invalid", "wrong", "not registered", "not exist", "doesn't exist", "كلمة"]) {
        return FetchError::BadCredentials;
    }
    carrier_error(r, "sign-in")
}

/// Trades the refresh token for a new access token: one request, like a sign-in, but it
/// isn't one.
async fn renew(
    client: &wreq::Client,
    dial: &str,
    password: &str,
    refresh: &str,
    kept: &Session,
    ctx: &FetchContext<'_>,
) -> Result<Session, FetchError> {
    let body = object(&[
        ("ChannelName", json!(CHANNEL_NAME)),
        ("ChannelPassword", json!(CHANNEL_PASSWORD)),
        ("isBiometric", json!(false)),
        ("channel", channel()),
        ("Dial", json!(dial)),
        ("Password", json!(password)),
        ("lang", json!("en")),
        ("IsEasyLogin", json!(false)),
    ]);
    let mut headers = headers(None, false);
    let value = HeaderValue::from_str(refresh).map_err(|_| FetchError::Unexpected("Orange's refresh token isn't a header value".into()))?;
    headers.insert(HeaderName::from_static("refreshtoken"), value);
    let url = format!("{SERVICES}/APIs/Profile/api/Authentications/RefreshToken");
    let v = post(client, &url, body, headers, "token renewal").await.map_err(CallError::into_fetch_error)?;
    ctx.dump("or-renew", &v);
    let session = parse_renewal(&v, kept).ok_or_else(|| FetchError::Unexpected("Orange didn't renew the token".into()))?;
    keep(dial, &session);
    tracing::debug!("Orange: renewed the token");
    Ok(session)
}

pub(crate) fn parse_renewal(v: &Value, kept: &Session) -> Option<Session> {
    let access = j::text(&v["AccessToken"])?;
    let profile = &v["ProfileInfo"];
    Some(Session {
        password: kept.password.clone(),
        expires: token_expiry(&access),
        access,
        refresh: j::text(&v["RefreshToken"]).or_else(|| kept.refresh.clone()),
        user_id: j::text(&profile["UserID"]).or_else(|| kept.user_id.clone()),
        owner_name: person(profile).or_else(|| kept.owner_name.clone()),
    })
}

/// The JWT's `exp`. Its signature isn't checked: it comes straight from Orange over TLS and
/// only says when to renew it. Without one, about 15 minutes, like Orange's tokens.
fn token_expiry(token: &str) -> i64 {
    token
        .split('.')
        .nth(1)
        .and_then(|p| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(p.trim_end_matches('=')).ok())
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .and_then(|claims| j::num(&claims["exp"]))
        .map(|exp| exp as i64)
        .unwrap_or_else(|| Utc::now().timestamp() + 14 * 60)
}

/// "First Last" from `UserData` or `ProfileInfo`.
fn person(v: &Value) -> Option<String> {
    let full = [j::text(&v["FirstName"]), j::text(&v["LastName"])].into_iter().flatten().collect::<Vec<_>>().join(" ");
    tidy_name(&full)
}

fn keep(dial: &str, session: &Session) {
    if let Ok(mut sessions) = SESSIONS.lock() {
        sessions.insert(dial.to_string(), session.clone());
    }
}

fn forget(dial: &str) {
    if let Ok(mut sessions) = SESSIONS.lock() {
        sessions.remove(dial);
    }
}

fn blocked_until() -> Option<DateTime<Utc>> {
    BLOCKED.lock().ok().and_then(|b| *b).filter(|until| *until > Utc::now())
}

// ---------------------------------------------------------------------------------------
// Requests

/// A failed call: Orange refused the token, or anything else.
enum CallError {
    Token,
    Fetch(FetchError),
}

impl From<FetchError> for CallError {
    fn from(e: FetchError) -> Self {
        CallError::Fetch(e)
    }
}

impl From<wreq::Error> for CallError {
    fn from(e: wreq::Error) -> Self {
        CallError::Fetch(e.into())
    }
}

impl CallError {
    fn into_fetch_error(self) -> FetchError {
        match self {
            CallError::Token => FetchError::Unexpected("Orange refused its own sign-in token".into()),
            CallError::Fetch(e) => e,
        }
    }
}

/// The app's headers. Home internet calls also want `lang` as a header.
fn headers(token: Option<&str>, lang_header: bool) -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(CONTENT_TYPE, HeaderValue::from_static("application/json; charset=UTF-8"));
    h.insert(HeaderName::from_static("appversion"), HeaderValue::from_static(APP_VERSION));
    h.insert(HeaderName::from_static("osversion"), HeaderValue::from_static(OS_VERSION));
    h.insert(HeaderName::from_static("isandroid"), HeaderValue::from_static("true"));
    h.insert(HeaderName::from_static("iseasylogin"), HeaderValue::from_static("false"));
    if let Some(token) = token.and_then(|t| HeaderValue::from_str(t).ok()) {
        h.insert(HeaderName::from_static("token"), token);
    }
    if lang_header {
        h.insert(HeaderName::from_static("lang"), HeaderValue::from_static("en"));
    }
    h
}

/// `channel` in the data calls: the same credentials, with `Password` for `ChannelPassword`.
fn channel() -> Value {
    json!({ "ChannelName": CHANNEL_NAME, "Password": CHANNEL_PASSWORD })
}

/// A JSON object with its keys in the order the app sends them (serde_json sorts them).
fn object(fields: &[(&str, Value)]) -> String {
    let members: Vec<String> = fields.iter().map(|(k, v)| format!("{}:{v}", Value::from(*k))).collect();
    format!("{{{}}}", members.join(","))
}

async fn post(client: &wreq::Client, url: &str, body: String, headers: HeaderMap, what: &str) -> Result<Value, CallError> {
    let resp = client.post(url).headers(headers).body(body).send().await?;
    let status = resp.status().as_u16();
    let text = resp.text().await?;
    if is_firewall_page(&text) {
        let until = Utc::now() + chrono::Duration::from_std(BLOCK_BACKOFF).unwrap_or_default();
        if let Ok(mut blocked) = BLOCKED.lock() {
            *blocked = Some(until);
        }
        tracing::warn!("Orange's firewall rejected the {what} request; leaving Orange alone until {until}");
        return Err(FetchError::CaptchaBlocked(until).into());
    }
    if status == 401 {
        return Err(CallError::Token);
    }
    serde_json::from_str(&text).map_err(|_| {
        let snippet: String = text.chars().take(120).collect();
        FetchError::Unexpected(format!("Orange {what}: HTTP {status} with a non-JSON body: {snippet}")).into()
    })
}

/// F5's "Request Rejected" page, for a client it doesn't trust or an address that sent
/// too much.
fn is_firewall_page(text: &str) -> bool {
    text.contains("<title>Request Rejected</title>") || text.contains("The requested URL was rejected")
}

/// The payload of a WCF reply, which wraps it in `<Method>Result`.
fn result<'a>(v: &'a Value, key: &str) -> &'a Value {
    match &v[key] {
        Value::Null => v,
        inner => inner,
    }
}

fn error_code(v: &Value) -> Option<i64> {
    j::num(&v["ErrorCode"]).map(|c| c as i64)
}

/// Orange's message for a failed call, else its code.
fn carrier_error(v: &Value, what: &str) -> FetchError {
    match j::text(&v["ErrorDescription"]).filter(|d| !d.eq_ignore_ascii_case("success")) {
        Some(message) => FetchError::Carrier(message),
        None => FetchError::Unexpected(format!(
            "Orange {what} failed (ErrorCode {})",
            j::text(&v["ErrorCode"]).unwrap_or_else(|| "?".into())
        )),
    }
}

// ---------------------------------------------------------------------------------------
// Mobile lines

async fn read_mobile(client: &wreq::Client, session: &Session, dial: &str, password: &str, ctx: &FetchContext<'_>) -> Result<Usage, CallError> {
    let token = Some(session.access.as_str());
    let body = object(&[
        ("channel", channel()),
        ("CurrentVersion", json!(APP_VERSION)),
        ("dial", json!(dial)),
        ("isCoin", json!(false)),
        ("lang", json!("en")),
        ("password", json!(password)),
        ("isEasyLogin", json!(false)),
    ]);
    let v = post(client, &format!("{BACKEND}/api/V2/Account/GetDialInfo"), body, headers(token, false), "line info").await?;
    ctx.dump("or-dial-info", &v);
    let line = parse_dial_info(&v)?;

    // Units need the rate plan; without one Orange only says "RP code is empty".
    let units = match (&line.rp_code, &line.service_class) {
        (Some(rp_code), service_class) => {
            let body = object(&[
                ("channel", channel()),
                ("dial", json!(dial)),
                ("lang", json!("en")),
                ("password", json!(password)),
                ("isEasyLogin", json!(false)),
                ("isCoin", json!(false)),
                ("isDarkMode", json!(false)),
                ("rpCode", json!(rp_code)),
                ("serviceClassId", json!(service_class.clone().unwrap_or_default())),
            ]);
            let url = format!("{SERVICES}/PostpaidInfoService.svc/GetConsumption");
            let v = post(client, &url, body, headers(token, false), "units").await?;
            ctx.dump("or-units", &v);
            v
        }
        (None, _) => Value::Null,
    };

    let body = object(&[
        ("channel", channel()),
        ("dial", json!(dial)),
        ("lang", json!("en")),
        ("password", json!(password)),
        ("isEasyLogin", json!(false)),
        ("userID", json!(session.user_id)),
        ("hasBonusBucket", json!(true)),
        ("roamingBuckets", json!(false)),
    ]);
    let url = format!("{SERVICES}/BucketsService.svc/GetCurrentActiveBuckets");
    let buckets = post(client, &url, body, headers(token, false), "data buckets").await?;
    ctx.dump("or-buckets", &buckets);

    Ok(mobile_usage(&line, &units, &buckets, session.owner_name.clone(), dial, Utc::now())?)
}

/// What `GetDialInfo` says about a mobile line.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct DialInfo {
    /// Rate plan and service class, for the units call.
    pub rp_code: Option<String>,
    pub service_class: Option<String>,
    pub plan: Option<String>,
    pub payment: Option<PaymentKind>,
    pub balance: Option<f64>,
}

pub(crate) fn parse_dial_info(v: &Value) -> Result<DialInfo, FetchError> {
    match error_code(v) {
        None | Some(0) => {}
        // "None Mobinil": the account's number isn't an Orange mobile line.
        Some(7) => return Err(FetchError::NoLine(LineType::Mobile)),
        Some(_) => return Err(carrier_error(v, "line info")),
    }
    if v["IsMobinil"] == Value::Bool(false) {
        return Err(FetchError::NoLine(LineType::Mobile));
    }
    Ok(DialInfo {
        rp_code: j::text(&v["RpCode"]),
        service_class: j::text(&v["ServiceClassID"]),
        plan: j::text(&v["PlanName"]),
        payment: match j::text(&v["DialPackage"]).map(|p| p.to_ascii_lowercase()).as_deref() {
            Some("prepaid") => Some(PaymentKind::Prepaid),
            Some("postpaid") => Some(PaymentKind::Postpaid),
            _ => None,
        },
        balance: j::num(&v["Balance"]).map(|b| (b * 100.0).round() / 100.0),
    })
}

/// Units (Kart El Kebir, minutes...) and data buckets. A call that failed adds nothing; a
/// units-only line has `Buckets: null`.
pub(crate) fn mobile_usage(
    line: &DialInfo,
    units: &Value,
    buckets: &Value,
    owner_name: Option<String>,
    dial: &str,
    now: DateTime<Utc>,
) -> Result<Usage, FetchError> {
    let mut bundles: Vec<Bundle> = Vec::new();
    let units = result(units, "GetConsumptionResult");
    match error_code(units) {
        None | Some(0) => bundles.extend(units["Units"].as_array().into_iter().flatten().filter_map(unit_bundle)),
        Some(code) => tracing::info!(code, "Orange units unavailable: {}", j::text(&units["ErrorDescription"]).unwrap_or_default()),
    }
    let buckets = result(buckets, "GetCurrentActiveBucketsResult");
    match error_code(buckets) {
        None | Some(0) => {
            let listed = ["Buckets", "GoPasses"].into_iter().flat_map(|k| buckets[k].as_array().into_iter().flatten());
            let go = Some(&buckets["GoBucket"]).filter(|b| b.is_object());
            bundles.extend(listed.chain(go).filter_map(data_bundle));
        }
        Some(code) => tracing::info!(code, "Orange data buckets unavailable: {}", j::text(&buckets["ErrorDescription"]).unwrap_or_default()),
    }
    order_bundles(&mut bundles);
    assign_keys(&mut bundles);

    let main = bundles.first().ok_or_else(|| FetchError::Unexpected("Orange returned no bundles for this line".into()))?;
    let (main_name, main_expiry, main_start) = (main.name.clone(), main.expires_at, main.starts_at);
    let renews_at = main_expiry.or_else(|| bundles.iter().find_map(|b| b.expires_at));
    Ok(Usage {
        owner_name,
        package_name: line.plan.clone().or(Some(main_name)),
        package_name_ar: None,
        balance_egp: line.balance,
        owed_egp: None,
        bundles,
        subscribed_at: main_start,
        renews_at,
        fetched_at: now,
        info: LineInfo {
            plan: line.plan.clone(),
            plan_ar: None,
            speed: None,
            payment: line.payment,
            status: None,
            number: Some(dial.to_string()),
            since: None,
        },
        payments: None,
    })
}

/// One of `GetConsumption`'s `Units`, e.g. "Kart El Kebir Bundles", 425 Units until
/// 30/11/2026. `TotalSize` is 0 when Orange doesn't say how big a bundle is: it's shown as
/// full then, and [`crate::store::Account::apply_usage`] keeps the most it had this cycle.
fn unit_bundle(u: &Value) -> Option<Bundle> {
    let name = j::text(&u["Name"]).or_else(|| j::text(&u["Title"]))?;
    let remaining = j::num(&u["Remaining"])?.max(0.0);
    let label = j::text(&u["UOMString"]).or_else(|| j::text(&u["ConsumptionText"])).unwrap_or_default();
    let (unit, factor) = unit_of(&label);
    let total = j::num(&u["TotalSize"]).unwrap_or(0.0).max(remaining);
    if total <= 0.0 {
        return None;
    }
    Some(Bundle {
        key: String::new(),
        kind: kind_of(unit, BundleKind::Other),
        name: tidy(&name),
        name_ar: None,
        unit,
        total: total * factor,
        remaining: remaining * factor,
        starts_at: None,
        expires_at: date(&u["ExpiryDate"]),
        state: BundleState::Active,
        settle_by: None,
    })
}

/// An active data bucket or Go pass. No line Klue has read has one yet, so this goes by the
/// fields the app reads: `BucketName`, `RemainingValue` in `RemainingValueUnit`, `BucketSize`
/// in `BucketSizeUnit`, `BucketUsage`, and `ExpiryDate` or `RenewalDate`. Kept in the size's
/// unit.
fn data_bundle(b: &Value) -> Option<Bundle> {
    let name = first_text(b, &["BucketName", "Name", "BucketCategoryName"])?;
    let remaining = first_num(b, &["RemainingValue", "Remaining"])?.max(0.0);
    let remaining_label = first_text(b, &["RemainingValueUnit", "RemainingUnit"]);
    let size_label = first_text(b, &["BucketSizeUnit"]).or_else(|| remaining_label.clone()).unwrap_or_else(|| "MB".into());
    let (unit, size_factor) = unit_of(&size_label);
    let (remaining_unit, remaining_factor) = unit_of(remaining_label.as_deref().unwrap_or(&size_label));
    let remaining = convert(remaining * remaining_factor, remaining_unit, unit);
    let total = match first_num(b, &["BucketSize", "BucketSizeValue"]) {
        Some(size) if size > 0.0 => size * size_factor,
        _ => remaining + first_num(b, &["BucketUsage"]).map(|u| convert(u * remaining_factor, remaining_unit, unit)).unwrap_or(0.0),
    }
    .max(remaining);
    if total <= 0.0 {
        return None;
    }
    Some(Bundle {
        key: String::new(),
        kind: kind_of(unit, BundleKind::Data),
        name: tidy(&name),
        name_ar: None,
        unit,
        total,
        remaining,
        starts_at: None,
        expires_at: ["ExpiryDate", "RenewalDate", "Validity"].into_iter().find_map(|k| date(&b[k])),
        state: BundleState::Active,
        settle_by: None,
    })
}

fn first_text(v: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|k| j::text(&v[*k]))
}

fn first_num(v: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|k| j::num(&v[*k]))
}

// ---------------------------------------------------------------------------------------
// Home internet

async fn read_home_internet(
    client: &wreq::Client,
    session: &Session,
    dial: &str,
    password: &str,
    ctx: &FetchContext<'_>,
) -> Result<Usage, CallError> {
    let token = Some(session.access.as_str());
    let fields = |dsl_token: Option<&str>| {
        let mut fields = vec![
            ("ChannelName", json!(CHANNEL_NAME)),
            ("ChannelPassword", json!(CHANNEL_PASSWORD)),
            ("Dial", json!(dial)),
            ("lang", json!("en")),
            ("Password", json!(password)),
        ];
        fields.extend(dsl_token.map(|t| ("DSLToken", json!(t))));
        object(&fields)
    };
    let url = format!("{SERVICES}/APIs/DSL/Account/api/Profile/GetProfileInfo");
    let v = post(client, &url, fields(None), headers(token, true), "home internet profile").await?;
    ctx.dump("or-dsl-profile", &v);
    let landline = parse_landline(&v)?;

    let url = format!("{SERVICES}/APIs/DSL/Account/api/Consumption/GetConsumption");
    let v = post(client, &url, fields(Some(&landline.token)), headers(token, true), "home internet usage").await?;
    ctx.dump("or-dsl-usage", &v);
    Ok(home_internet_usage(&v, &landline, session.owner_name.clone(), Utc::now())?)
}

/// What `GetProfileInfo` says about the landline linked to the account.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct LinkedLandline {
    /// For `GetConsumption`.
    pub token: String,
    pub number: String,
    pub owner_name: Option<String>,
    pub status: Option<String>,
}

pub(crate) fn parse_landline(v: &Value) -> Result<LinkedLandline, FetchError> {
    if error_code(v).is_some_and(|c| c != 0) {
        return Err(carrier_error(v, "home internet profile"));
    }
    let token = j::text(&v["DSLToken"]).filter(|_| j::truthy(&v["IsLandlineLinked"]));
    let (Some(token), Some(number)) = (token, j::text(&v["DSLNumber"])) else {
        return Err(FetchError::NoLine(LineType::Landline));
    };
    Ok(LinkedLandline {
        token,
        number: format::normalize(&number, LineType::Landline).unwrap_or(number),
        owner_name: j::text(&v["CustomerName"]).and_then(|n| tidy_name(&n)),
        status: j::text(&v["DSLUserStatus"]),
    })
}

/// The package and the add-ons with something left: Orange keeps used-up add-ons in the list
/// until they expire.
pub(crate) fn home_internet_usage(
    v: &Value,
    landline: &LinkedLandline,
    account_name: Option<String>,
    now: DateTime<Utc>,
) -> Result<Usage, FetchError> {
    if error_code(v).is_some_and(|c| c != 0) {
        return Err(carrier_error(v, "home internet usage"));
    }
    let package = quota(&v["MainPackage"], BundleKind::Data);
    let addons = ["Extensions", "Compensations"]
        .into_iter()
        .flat_map(|k| v[k].as_array().into_iter().flatten())
        .filter_map(|a| quota(a, BundleKind::Addon))
        .filter(|a| a.remaining > 0.0);
    let mut bundles: Vec<Bundle> = package.clone().into_iter().chain(addons).collect();
    order_bundles(&mut bundles);
    assign_keys(&mut bundles);

    let first = bundles.first().ok_or_else(|| FetchError::Unexpected("Orange returned no quota for this landline".into()))?;
    let package_name = package.as_ref().map(|p| p.name.clone()).unwrap_or_else(|| first.name.clone());
    let renews_at = package.as_ref().and_then(|p| p.expires_at).or(first.expires_at);
    Ok(Usage {
        owner_name: landline.owner_name.clone().or(account_name),
        package_name: Some(package_name),
        package_name_ar: None,
        balance_egp: None,
        owed_egp: None,
        bundles,
        subscribed_at: None,
        renews_at,
        fetched_at: now,
        info: LineInfo {
            plan: package.map(|p| p.name),
            plan_ar: None,
            speed: None,
            payment: None,
            status: landline.status.clone(),
            number: Some(landline.number.clone()),
            since: None,
        },
        payments: None,
    })
}

/// A home internet package or add-on: `Balance` (its size), `Consumed` and `Remaining` in
/// `RemainingValueUnit` (bytes), until `NextRecharge`.
fn quota(p: &Value, kind: BundleKind) -> Option<Bundle> {
    let name = j::text(&p["Name"])?;
    let remaining = j::num(&p["Remaining"])?.max(0.0);
    let total = j::num(&p["Balance"]).filter(|b| *b > 0.0).or_else(|| j::num(&p["Consumed"]).map(|c| c + remaining))?;
    if total <= 0.0 {
        return None;
    }
    let (unit, factor) = unit_of(&j::text(&p["RemainingValueUnit"]).unwrap_or_else(|| "bytes".into()));
    Some(Bundle {
        key: String::new(),
        kind,
        name: tidy(&name),
        name_ar: None,
        unit,
        total: total * factor,
        remaining: remaining * factor,
        starts_at: None,
        expires_at: date(&p["NextRecharge"]),
        state: BundleState::Active,
        settle_by: None,
    })
}

// ---------------------------------------------------------------------------------------
// Units and dates

/// Orange's unit names, with the factor to Klue's unit. Home internet counts bytes, which
/// Klue shows in GB (2^30 bytes, as Orange sizes its packages).
fn unit_of(label: &str) -> (Unit, f64) {
    match label.trim().to_ascii_lowercase().as_str() {
        "bytes" | "byte" | "b" => (Unit::Gb, 1.0 / GIB),
        "kb" | "kbs" | "kilobyte" | "kilobytes" => (Unit::Mb, 1.0 / 1024.0),
        "mb" | "mbs" | "mega" | "megabyte" | "megabytes" => (Unit::Mb, 1.0),
        "gb" | "gbs" | "giga" | "gigabyte" | "gigabytes" => (Unit::Gb, 1.0),
        "min" | "mins" | "minute" | "minutes" => (Unit::Minutes, 1.0),
        "sec" | "secs" | "second" | "seconds" => (Unit::Minutes, 1.0 / 60.0),
        "sms" | "message" | "messages" => (Unit::Sms, 1.0),
        "egp" | "le" | "l.e" | "pound" | "pounds" => (Unit::Egp, 1.0),
        _ => (Unit::Units, 1.0),
    }
}

/// `amount` in `from`, in `to`. Only MB and GB convert.
fn convert(amount: f64, from: Unit, to: Unit) -> f64 {
    match (from, to) {
        (Unit::Mb, Unit::Gb) => amount / 1024.0,
        (Unit::Gb, Unit::Mb) => amount * 1024.0,
        _ => amount,
    }
}

fn kind_of(unit: Unit, otherwise: BundleKind) -> BundleKind {
    match unit {
        Unit::Minutes => BundleKind::Minutes,
        Unit::Sms => BundleKind::Sms,
        Unit::Gb | Unit::Mb => BundleKind::Data,
        _ => otherwise,
    }
}

fn tidy(name: &str) -> String {
    name.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Orange's dates: `20261216000000` (home internet), `30/11/2026` (units: the end of that
/// day), .NET's `/Date(1764892800000+0200)/`, or ISO 8601. Times without a zone are Cairo
/// time.
pub(crate) fn date(v: &Value) -> Option<DateTime<Utc>> {
    let s = j::text(v)?;
    let end_of = |d: NaiveDate| d.and_hms_opt(23, 59, 59).and_then(j::cairo);
    if let Some(inner) = s.strip_prefix("/Date(").and_then(|rest| rest.strip_suffix(")/")) {
        let digits = inner.find(['+', '-']).filter(|i| *i > 0).map_or(inner, |i| &inner[..i]);
        return digits.parse::<i64>().ok().filter(|ms| *ms > 0).and_then(|ms| Utc.timestamp_millis_opt(ms).single());
    }
    if s.chars().all(|c| c.is_ascii_digit()) {
        return match s.len() {
            14 => NaiveDateTime::parse_from_str(&s, "%Y%m%d%H%M%S").ok().and_then(j::cairo),
            8 => NaiveDate::parse_from_str(&s, "%Y%m%d").ok().and_then(end_of),
            _ => None,
        };
    }
    if s.contains('/') {
        for fmt in ["%d/%m/%Y %H:%M:%S", "%d/%m/%Y %H:%M", "%m/%d/%Y %I:%M:%S %p"] {
            if let Ok(dt) = NaiveDateTime::parse_from_str(&s, fmt) {
                return j::cairo(dt);
            }
        }
        let day = s.split_whitespace().next()?;
        return NaiveDate::parse_from_str(day, "%d/%m/%Y").or_else(|_| NaiveDate::parse_from_str(day, "%m/%d/%Y")).ok().and_then(end_of);
    }
    j::iso_datetime(&Value::String(s))
}

#[cfg(test)]
mod tests {
    use chrono::Local;

    use super::*;

    fn fixture(name: &str) -> Value {
        let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    /// Wall-clock time in Cairo (the machine's zone, as Klue reads Orange's dates).
    fn local(d: Option<DateTime<Utc>>) -> String {
        d.map(|d| d.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S").to_string()).unwrap_or_default()
    }

    fn jwt(claims: Value) -> String {
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(claims.to_string());
        format!("eyJhbGciOiJIUzI1NiJ9.{payload}.c2ln")
    }

    #[test]
    fn reads_a_units_line() {
        let line = parse_dial_info(&fixture("or_dial_info.json")).unwrap();
        assert_eq!(line.rp_code.as_deref(), Some("20"));
        assert_eq!(line.service_class.as_deref(), Some("1062"));
        assert_eq!((line.plan.as_deref(), line.payment, line.balance), (None, Some(PaymentKind::Prepaid), None));

        let usage = mobile_usage(&line, &fixture("or_units.json"), &fixture("or_buckets_none.json"), Some("Mostafa Hassan".into()), "01200000000", Utc::now()).unwrap();
        // Kart El Kebir doesn't say how big it is: shown as full.
        let rows: Vec<_> = usage.bundles.iter().map(|b| (b.name.as_str(), b.kind, b.unit, b.remaining, b.total, b.key.as_str())).collect();
        assert_eq!(rows, [("Kart El Kebir Bundles", BundleKind::Other, Unit::Units, 425.0, 425.0, "kart-el-kebir-bundles")]);
        assert_eq!(local(usage.bundles[0].expires_at), "2026-11-30 23:59:59");
        // No plan name: the package is the main bundle.
        assert_eq!(usage.package_name.as_deref(), Some("Kart El Kebir Bundles"));
        assert_eq!(usage.renews_at, usage.bundles[0].expires_at);
        assert_eq!((usage.info.payment, usage.info.number.as_deref()), (Some(PaymentKind::Prepaid), Some("01200000000")));
        assert_eq!(usage.owner_name.as_deref(), Some("Mostafa Hassan"));
    }

    #[test]
    fn a_plan_name_and_balance_win() {
        let mut v = fixture("or_dial_info.json");
        v["PlanName"] = json!("Alo Plus");
        v["Balance"] = json!("37.256");
        v["DialPackage"] = json!("PostPaid");
        let line = parse_dial_info(&v).unwrap();
        let usage = mobile_usage(&line, &fixture("or_units.json"), &Value::Null, None, "01200000000", Utc::now()).unwrap();
        assert_eq!((usage.package_name.as_deref(), usage.balance_egp), (Some("Alo Plus"), Some(37.26)));
        assert_eq!((usage.info.plan.as_deref(), usage.info.payment), (Some("Alo Plus"), Some(PaymentKind::Postpaid)));
    }

    #[test]
    fn an_account_without_an_orange_mobile_line() {
        // "None Mobinil": the account's number isn't an Orange line.
        assert_eq!(parse_dial_info(&fixture("or_dial_info_not_orange.json")), Err(FetchError::NoLine(LineType::Mobile)));
        let mut v = fixture("or_dial_info.json");
        v["IsMobinil"] = json!(false);
        assert_eq!(parse_dial_info(&v), Err(FetchError::NoLine(LineType::Mobile)));
        let busy = json!({"ErrorCode": 99, "ErrorDescription": "Service is temporarily unavailable"});
        assert_eq!(parse_dial_info(&busy), Err(FetchError::Carrier("Service is temporarily unavailable".into())));
        assert!(matches!(parse_dial_info(&json!({"ErrorCode": 12, "ErrorDescription": null})), Err(FetchError::Unexpected(_))));
    }

    #[test]
    fn calls_that_failed_add_nothing() {
        let line = parse_dial_info(&fixture("or_dial_info.json")).unwrap();
        // Units refused ("RP code is empty"), buckets refused (a home internet number).
        let units = json!({"GetConsumptionResult": {"ErrorCode": 15, "ErrorDescription": "RP code is empty", "Units": null}});
        let buckets = json!({"GetCurrentActiveBucketsResult": {"ErrorCode": 14, "ErrorDescription": "CheckDataProfileStatus returned Error", "Buckets": null}});
        assert!(matches!(mobile_usage(&line, &units, &buckets, None, "01200000000", Utc::now()), Err(FetchError::Unexpected(_))));
        let usage = mobile_usage(&line, &fixture("or_units.json"), &buckets, None, "01200000000", Utc::now()).unwrap();
        assert_eq!(usage.bundles.len(), 1);
    }

    #[test]
    fn reads_data_buckets() {
        // Laid out like the app reads them; no line Klue has read has one yet.
        let line = parse_dial_info(&fixture("or_dial_info.json")).unwrap();
        let buckets = json!({"GetCurrentActiveBucketsResult": {
            "ErrorCode": 0,
            "Buckets": [
                {"BucketName": "Go Internet  5 GB", "BucketSize": "5", "BucketSizeUnit": "GB", "RemainingValue": "1536", "RemainingValueUnit": "MB", "ExpiryDate": "12/12/2026"},
                {"BucketName": "Social", "RemainingValue": 300, "RemainingValueUnit": "MB", "BucketUsage": 700}
            ],
            "GoBucket": {"BucketName": "Go Pass", "BucketSize": 500, "BucketSizeUnit": "MB", "RemainingValue": 0, "RemainingValueUnit": "MB", "RenewalDate": "/Date(1798761600000+0200)/"},
            "GoPasses": null
        }});
        let usage = mobile_usage(&line, &fixture("or_units.json"), &buckets, None, "01200000000", Utc::now()).unwrap();
        let rows: Vec<_> = usage.bundles.iter().map(|b| (b.name.as_str(), b.kind, b.unit, b.remaining, b.total)).collect();
        // Largest with something left first; each kept in its size's unit.
        assert_eq!(
            rows,
            [
                ("Go Internet 5 GB", BundleKind::Data, Unit::Gb, 1.5, 5.0),
                ("Social", BundleKind::Data, Unit::Mb, 300.0, 1000.0),
                ("Go Pass", BundleKind::Data, Unit::Mb, 0.0, 500.0),
                ("Kart El Kebir Bundles", BundleKind::Other, Unit::Units, 425.0, 425.0),
            ]
        );
        assert_eq!(local(usage.bundles[0].expires_at), "2026-12-12 23:59:59");
        assert_eq!(usage.bundles[2].expires_at.unwrap().timestamp(), 1_798_761_600);
        assert_eq!(usage.package_name.as_deref(), Some("Go Internet 5 GB"));
    }

    #[test]
    fn reads_home_internet() {
        let landline = parse_landline(&fixture("or_dsl_profile.json")).unwrap();
        assert_eq!(landline.number, "0233456789");
        assert_eq!(landline.token, "00000000-0000-4000-8000-000000000000");
        assert_eq!((landline.owner_name.as_deref(), landline.status.as_deref()), (Some("Mostafa Hassan Ali"), Some("Active")));

        let usage = home_internet_usage(&fixture("or_dsl_usage.json"), &landline, Some("Account Holder".into()), Utc::now()).unwrap();
        // Bytes in GB of 2^30 bytes: the 500 GB package and the one add-on with something
        // left. The six used-up add-ons are dropped.
        let rows: Vec<_> = usage.bundles.iter().map(|b| (b.name.as_str(), b.kind, b.unit, (b.remaining * 100.0).round() / 100.0, b.total, b.key.as_str())).collect();
        assert_eq!(
            rows,
            [
                ("Home Internet 660", BundleKind::Data, Unit::Gb, 237.17, 500.0, "home-internet-660"),
                ("Extra Quota 20 GB", BundleKind::Addon, Unit::Gb, 15.66, 20.0, "extra-quota-20-gb"),
            ]
        );
        assert_eq!(local(usage.bundles[0].expires_at), "2026-12-16 00:00:00");
        assert_eq!(local(usage.bundles[1].expires_at), "2026-12-15 00:00:00");
        assert_eq!(usage.renews_at, usage.bundles[0].expires_at);
        assert_eq!(usage.package_name.as_deref(), Some("Home Internet 660"));
        // The landline's customer name wins over the account's.
        assert_eq!(usage.owner_name.as_deref(), Some("Mostafa Hassan Ali"));
        assert_eq!((usage.info.plan.as_deref(), usage.info.status.as_deref(), usage.info.number.as_deref()), (Some("Home Internet 660"), Some("Active"), Some("0233456789")));
        assert_eq!(usage.balance_egp, None);
    }

    #[test]
    fn an_empty_package_still_names_the_line() {
        let landline = parse_landline(&fixture("or_dsl_profile.json")).unwrap();
        let mut v = fixture("or_dsl_usage.json");
        v["MainPackage"]["Remaining"] = json!("0");
        let usage = home_internet_usage(&v, &landline, None, Utc::now()).unwrap();
        // The add-on with something left leads; the package keeps the name and the date.
        assert_eq!(usage.bundles[0].name, "Extra Quota 20 GB");
        assert_eq!((usage.bundles[1].name.as_str(), usage.bundles[1].remaining), ("Home Internet 660", 0.0));
        assert_eq!(usage.package_name.as_deref(), Some("Home Internet 660"));
        assert_eq!(local(usage.renews_at), "2026-12-16 00:00:00");

        let failed = json!({"ErrorCode": 3, "ErrorDescription": "Invalid DSL token"});
        assert_eq!(home_internet_usage(&failed, &landline, None, Utc::now()), Err(FetchError::Carrier("Invalid DSL token".into())));
    }

    #[test]
    fn an_account_without_a_linked_landline() {
        assert_eq!(parse_landline(&fixture("or_dsl_profile_unlinked.json")), Err(FetchError::NoLine(LineType::Landline)));
        let mut v = fixture("or_dsl_profile.json");
        v["IsLandlineLinked"] = json!(false);
        assert_eq!(parse_landline(&v), Err(FetchError::NoLine(LineType::Landline)));
    }

    #[test]
    fn signs_in() {
        let token = jwt(json!({"exp": 1_893_456_000, "unique_name": "01200000000"}));
        let reply = json!({"SignInUserResult": {
            "AccessToken": token, "RefreshToken": "r1", "ErrorCode": 0, "ErrorDescription": null,
            "UserData": {"Dial": "01200000000", "UserID": "u1", "Email": "x@example.com", "FirstName": "Mostafa", "LastName": "Hassan"}
        }});
        let s = parse_sign_in(&reply, "pw").unwrap();
        assert_eq!((s.expires, s.refresh.as_deref(), s.user_id.as_deref(), s.owner_name.as_deref()), (1_893_456_000, Some("r1"), Some("u1"), Some("Mostafa Hassan")));
        assert_eq!(s.password, "pw");

        // A renewal keeps what it doesn't send again.
        let renewed = parse_renewal(&json!({"AccessToken": jwt(json!({"exp": 1_893_457_000})), "ProfileInfo": {}}), &s).unwrap();
        assert_eq!((renewed.expires, renewed.refresh.as_deref(), renewed.user_id.as_deref()), (1_893_457_000, Some("r1"), Some("u1")));
        assert_eq!(parse_renewal(&json!({"ErrorCode": 401}), &s), None);
        // No `exp`: about 15 minutes.
        let soon = token_expiry("not-a-jwt") - Utc::now().timestamp();
        assert!((13 * 60..=15 * 60).contains(&soon), "{soon}");
    }

    #[test]
    fn classifies_sign_in_failures() {
        let failed = |code: i64, message: &str| json!({"SignInUserResult": {"ErrorCode": code, "ErrorDescription": message, "AccessToken": null}});
        // What Orange says for a wrong password, by its code whatever the wording.
        assert_eq!(parse_sign_in(&failed(9, "invalid username and password"), "pw"), Err(FetchError::BadCredentials));
        assert_eq!(parse_sign_in(&failed(9, "اسم المستخدم غير صحيح"), "pw"), Err(FetchError::BadCredentials));
        assert_eq!(parse_sign_in(&failed(1, "Invalid dial or password"), "pw"), Err(FetchError::BadCredentials));
        // A number Orange doesn't know. It reads like an outage, so it stays Orange's words.
        assert_eq!(parse_sign_in(&failed(16, "service down"), "pw"), Err(FetchError::Carrier("service down".into())));
        assert!(matches!(parse_sign_in(&failed(11, "Too many attempts, try again later"), "pw"), Err(FetchError::CaptchaBlocked(_))));
        assert_eq!(parse_sign_in(&failed(8, "Service is temporarily unavailable"), "pw"), Err(FetchError::Carrier("Service is temporarily unavailable".into())));
        assert_eq!(parse_sign_in(&failed(5, "System is under maintenance"), "pw"), Err(FetchError::Carrier("System is under maintenance".into())));
        // A success code without a token isn't a sign-in.
        assert!(matches!(parse_sign_in(&json!({"SignInUserResult": {"ErrorCode": 0}}), "pw"), Err(FetchError::Unexpected(_))));
    }

    #[test]
    fn reads_orange_dates() {
        assert_eq!(local(date(&json!("20261216000000"))), "2026-12-16 00:00:00");
        assert_eq!(local(date(&json!("20261207235900"))), "2026-12-07 23:59:00");
        assert_eq!(local(date(&json!("30/11/2026"))), "2026-11-30 23:59:59");
        assert_eq!(local(date(&json!("30/11/2026 14:05:00"))), "2026-11-30 14:05:00");
        assert_eq!(date(&json!("/Date(1764892800000+0200)/")).unwrap().timestamp(), 1_764_892_800);
        assert_eq!(date(&json!("/Date(-62135596800000)/")), None);
        assert_eq!(date(&json!("2026-12-08T21:59:59Z")).unwrap().to_rfc3339(), "2026-12-08T21:59:59+00:00");
        assert_eq!(date(&json!("")), None);
        assert_eq!(date(&Value::Null), None);
    }

    #[test]
    fn keeps_the_apps_key_order() {
        let body = object(&[("dialNumber", json!("012")), ("channel", channel()), ("isAndroid", json!(true))]);
        assert_eq!(body, r#"{"dialNumber":"012","channel":{"ChannelName":"MobinilAndMe","Password":"ig3yh*mk5l42@oj7QAR8yF"},"isAndroid":true}"#);
        assert!(serde_json::from_str::<Value>(&body).is_ok());
    }

    #[test]
    fn knows_the_firewall_page() {
        let page = "<html><head><title>Request Rejected</title></head><body>The requested URL was rejected. Please consult with your administrator.<br><br>Your support ID is: 1</body></html>";
        assert!(is_firewall_page(page));
        assert!(!is_firewall_page(r#"{"ErrorCode":0,"ErrorDescription":"Success"}"#));
    }
}
