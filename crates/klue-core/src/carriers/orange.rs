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

/// What the app's dashboard reads: `GetDialInfo` for the plan and the credit, then every
/// place bundles show up, since no single one has them all. Units come from the tariff
/// dashboard and the older units service; internet from the activated buckets, the internet
/// dashboard and the older buckets service.
async fn read_mobile(client: &wreq::Client, session: &Session, dial: &str, password: &str, ctx: &FetchContext<'_>) -> Result<Usage, CallError> {
    let token = Some(session.access.as_str());
    // Like the app: with `isCoin` off, the plan, the balance and the validity come back empty.
    let body = object(&[
        ("channel", channel()),
        ("CurrentVersion", json!(APP_VERSION.replace('.', ""))),
        ("dial", json!(dial)),
        ("isCoin", json!(true)),
        ("lang", json!("en")),
        ("password", json!(password)),
        ("isEasyLogin", json!(false)),
    ]);
    let v = post(client, &format!("{BACKEND}/api/V2/Account/GetDialInfo"), body, headers(token, false), "line info").await?;
    ctx.dump("or-dial-info", &v);
    let line = parse_dial_info(&v)?;
    let rp_code = line.rp_code.clone().unwrap_or_default();
    let service_class = line.service_class.clone().unwrap_or_default();

    let body = object(&[
        ("ChannelName", json!(CHANNEL_NAME)),
        ("ChannelPassword", json!(CHANNEL_PASSWORD)),
        ("dial", json!(dial)),
        ("isGoldUser", json!(false)),
        ("language", json!("en")),
        ("isCoin", json!(false)),
        ("rpCode", json!(rp_code)),
        ("serviceClassID", json!(service_class)),
    ]);
    let url = format!("{BACKEND}/APIs/CoreAPIs/Tariffs/api/Dashboard/GetConsumption");
    let tariffs = source(post(client, &url, body, headers(token, true), "tariff units").await, "tariff units")?;
    ctx.dump("or-tariffs", &tariffs);

    // The older units service only answers "RP code is empty" without a rate plan.
    let units = if line.rp_code.is_some() {
        let body = object(&[
            ("channel", channel()),
            ("dial", json!(dial)),
            ("lang", json!("en")),
            ("password", json!(password)),
            ("isEasyLogin", json!(false)),
            ("isCoin", json!(false)),
            ("isDarkMode", json!(false)),
            ("rpCode", json!(rp_code)),
            ("serviceClassId", json!(service_class)),
        ]);
        let url = format!("{SERVICES}/PostpaidInfoService.svc/GetConsumption");
        let v = source(post(client, &url, body, headers(token, false), "units").await, "units")?;
        ctx.dump("or-units", &v);
        v
    } else {
        Value::Null
    };

    let body = object(&[
        ("channel", channel()),
        ("lang", json!("en")),
        ("Dial", json!(dial)),
        ("Password", json!(password)),
        ("IsEasyLogin", json!(false)),
        ("dial", json!(dial)),
        ("rPCode", json!(rp_code)),
        ("serviceClassId", json!(service_class)),
    ]);
    let url = format!("{BACKEND}/APIs/CoreAPIs/Internet/api/InternetConsumption/GetActivatedBuckets");
    let activated = source(post(client, &url, body, headers(token, true), "internet bundles").await, "internet bundles")?;
    ctx.dump("or-activated", &activated);

    let body = object(&[
        ("ChannelName", json!(CHANNEL_NAME)),
        ("ChannelPassword", json!(CHANNEL_PASSWORD)),
        ("dial", json!(dial)),
        ("isGoldUser", json!(false)),
        ("language", json!("en")),
        ("isRoamer", json!(false)),
        ("rpCode", json!(rp_code)),
        ("serviceClassId", json!(service_class)),
    ]);
    let url = format!("{BACKEND}/APIs/CoreAPIs/Internet/api/Dashboard/GetConsumption");
    let internet = source(post(client, &url, body, headers(token, true), "internet usage").await, "internet usage")?;
    ctx.dump("or-internet", &internet);

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
    let buckets = source(post(client, &url, body, headers(token, false), "data buckets").await, "data buckets")?;
    ctx.dump("or-buckets", &buckets);

    let sources = MobileSources { tariffs, units, activated, internet, buckets };
    Ok(mobile_usage(&line, &sources, session.owner_name.clone(), dial, Utc::now())?)
}

/// A source that's merely missing (an error page instead of JSON, say) adds nothing. The
/// firewall, a refused token and the network still fail the refresh, so a line never shows
/// half its bundles.
fn source(result: Result<Value, CallError>, what: &str) -> Result<Value, CallError> {
    match result {
        Err(CallError::Fetch(FetchError::Unexpected(e))) => {
            tracing::info!("Orange {what} unavailable: {e}");
            Ok(Value::Null)
        }
        other => other,
    }
}

/// What `GetDialInfo` says about a mobile line.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct DialInfo {
    /// Rate plan and service class, which the other calls want.
    pub rp_code: Option<String>,
    pub service_class: Option<String>,
    pub plan: Option<String>,
    pub payment: Option<PaymentKind>,
    pub balance: Option<f64>,
    /// When a prepaid line expires unless it's recharged.
    pub valid_until: Option<DateTime<Utc>>,
}

pub(crate) fn parse_dial_info(v: &Value) -> Result<DialInfo, FetchError> {
    match error_code(v) {
        None | Some(0) => {}
        // "None Mobinil": the account's number isn't an Orange mobile line.
        Some(7) => return Err(FetchError::NoLine(LineType::Mobile)),
        Some(_) if v["IsMobinil"] == Value::Bool(false) => return Err(FetchError::NoLine(LineType::Mobile)),
        Some(_) => return Err(carrier_error(v, "line info")),
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
        valid_until: date(&v["ExpiryDate"]),
    })
}

/// The replies `mobile_usage` reads. A call that failed is `Null`, and replies with an error
/// code have no lists, so either way they add nothing.
#[derive(Debug, Default)]
pub(crate) struct MobileSources {
    /// `CoreAPIs/Tariffs/.../GetConsumption`: `units` and `extraUnits`.
    pub tariffs: Value,
    /// `PostpaidInfoService.svc/GetConsumption`: `Units`.
    pub units: Value,
    /// `CoreAPIs/Internet/.../GetActivatedBuckets`: the main bucket and the add-on lists.
    pub activated: Value,
    /// `CoreAPIs/Internet/api/Dashboard/GetConsumption`: internet and roaming buckets.
    pub internet: Value,
    /// `BucketsService.svc/GetCurrentActiveBuckets`: `Buckets`, `GoBucket`, `GoPasses`.
    pub buckets: Value,
}

/// Units and internet from every source, each bundle once, then roaming.
pub(crate) fn mobile_usage(
    line: &DialInfo,
    sources: &MobileSources,
    owner_name: Option<String>,
    dial: &str,
    now: DateTime<Utc>,
) -> Result<Usage, FetchError> {
    let wcf_units = &result(&sources.units, "GetConsumptionResult")["Units"];
    let units = merge(vec![
        dashboard_items(&sources.tariffs["units"], BundleKind::Data),
        wcf_units.as_array().into_iter().flatten().filter_map(wcf_unit).collect(),
        extra_units(&sources.tariffs["extraUnits"], "Extra units"),
    ]);
    let internet = &sources.internet["internetConsumption"];
    let data = merge(vec![
        activated_buckets(&sources.activated),
        dashboard_items(&internet["buckets"], BundleKind::Addon),
        extra_units(&internet["extraUnits"], "Extra internet"),
        wcf_buckets(result(&sources.buckets, "GetCurrentActiveBucketsResult")),
    ]);
    let roaming_data = &sources.internet["roamingConsumption"];
    let roaming = dashboard_items(&roaming_data["buckets"], BundleKind::Addon)
        .into_iter()
        .chain(extra_units(&roaming_data["extraUnits"], "Roaming extra"))
        .filter_map(Reading::bundle);

    let mut bundles: Vec<Bundle> = units.into_iter().chain(data).chain(roaming).collect();
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
            valid_until: line.valid_until,
        },
        payments: None,
    })
}

/// A bundle as one of the APIs puts it, before it's merged with the others.
#[derive(Debug, Clone)]
struct Reading {
    name: String,
    /// The kind when it's data; units, minutes and SMS go by their unit.
    kind: BundleKind,
    remaining: Option<f64>,
    total: Option<f64>,
    unit: Option<String>,
    /// The size's unit when the API gives it separately.
    total_unit: Option<String>,
    used: Option<f64>,
    ends: Option<DateTime<Utc>>,
    inactive: bool,
}

impl Reading {
    fn new(name: String, kind: BundleKind) -> Self {
        Self { name, kind, remaining: None, total: None, unit: None, total_unit: None, used: None, ends: None, inactive: false }
    }

    /// The bundle in Klue's units, kept in the unit of what's left. Its size is the API's,
    /// else what's left plus what was used, else the size in its name ("GO 92 (4250 MBs)").
    /// Without any it shows as full (see [`crate::store::Account::apply_usage`]).
    fn bundle(self) -> Option<Bundle> {
        let named = size_in_name(&self.name);
        let label = self.unit.clone().or_else(|| self.total_unit.clone()).or_else(|| named.as_ref().map(|(_, u)| u.clone()));
        let (unit, factor) = unit_of(label.as_deref().unwrap_or_default());
        let remaining = self.remaining.map(|r| r * factor);
        let used = self.used.map(|u| u * factor);
        let from_api = self.total.filter(|t| *t > 0.0).and_then(|t| {
            let (from, f) = unit_of(self.total_unit.as_deref().or(label.as_deref()).unwrap_or_default());
            convert(t * f, from, unit)
        });
        let from_usage = match (remaining, used) {
            (Some(r), Some(u)) if r >= 0.0 && u >= 0.0 && r + u > 0.0 => Some(r + u),
            _ => None,
        };
        let total = from_api
            .or(from_usage)
            .or_else(|| {
                let (value, label) = named?;
                let (from, f) = unit_of(&label);
                convert(value * f, from, unit)
            });
        let remaining = remaining.unwrap_or(0.0).max(0.0);
        let total = total.unwrap_or(remaining).max(remaining);
        if total <= 0.0 {
            return None;
        }
        Some(Bundle {
            key: String::new(),
            kind: if matches!(unit, Unit::Gb | Unit::Mb) { self.kind } else { kind_of(unit, BundleKind::Other) },
            name: tidy(&self.name),
            name_ar: None,
            unit,
            total,
            remaining,
            starts_at: None,
            expires_at: self.ends,
            state: if self.inactive { BundleState::Inactive } else { BundleState::Active },
            settle_by: None,
        })
    }
}

/// `CoreAPIs/Internet/.../GetActivatedBuckets`: the main bucket, the add-on, extension, bonus
/// and Kart El Kebir lists, any other list of buckets Orange adds, and the out-of-bundle one.
fn activated_buckets(v: &Value) -> Vec<Reading> {
    let one = |x: &Value, kind: BundleKind| {
        let name = j::text(field(x, &["bucketName"]))?;
        Some(Reading {
            remaining: j::num(field(x, &["remainingSize"])),
            total: j::num(field(x, &["bucketSize"])),
            unit: j::text(field(x, &["sizeUnit"])),
            ends: date(field(x, &["expiryDate"])).or_else(|| date(field(x, &["renewalDate"]))),
            ..Reading::new(name, kind)
        })
    };
    let mut out = Vec::new();
    let main = field(v, &["mainBucket"]);
    if let Some(mut reading) = one(main, BundleKind::Data) {
        // A recharge-and-renew bundle with nothing left to report waits for a recharge.
        let renews_on_recharge = j::truthy(field(main, &["isRnR"])) || j::truthy(field(main, &["isRechargeAndRenew"]));
        reading.inactive = renews_on_recharge && field(main, &["remainingSize"]).is_null();
        out.push(reading);
    }
    const LISTS: [&str; 4] = ["addOnBuckets", "extensionBuckets", "bonusBuckets", "kartElKebirBuckets"];
    for key in LISTS {
        out.extend(field(v, &[key]).as_array().into_iter().flatten().filter_map(|x| one(x, BundleKind::Addon)));
    }
    if let Some(map) = v.as_object() {
        let known = |k: &str| k.eq_ignore_ascii_case("mainBucket") || k.eq_ignore_ascii_case("outOfBucket") || LISTS.iter().any(|l| l.eq_ignore_ascii_case(k));
        for (_, list) in map.iter().filter(|(k, _)| !known(k)) {
            let buckets = list.as_array().into_iter().flatten().filter(|x| !field(x, &["remainingSize"]).is_null() || !field(x, &["bucketSize"]).is_null());
            out.extend(buckets.filter_map(|x| one(x, BundleKind::Addon)));
        }
    }
    let outside = field(v, &["outOfBucket"]);
    if let Some(name) = j::text(field(outside, &["bucketName"])) {
        out.push(Reading { remaining: j::num(field(outside, &["remainingSize"])), ..Reading::new(name, BundleKind::Addon) });
    }
    out
}

/// The `units` or `buckets` of a `CoreAPIs` dashboard. `totalSize` is 0 when Orange doesn't
/// give a size, and `consumedSize` then is minus what's left.
fn dashboard_items(items: &Value, kind: BundleKind) -> Vec<Reading> {
    items
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|u| {
            let name = j::text(field(u, &["unitTitle", "bucketName"]))?;
            Some(Reading {
                remaining: j::num(field(u, &["remainingSize"])),
                total: ["bucketSize", "totalSize"].into_iter().filter_map(|k| j::num(field(u, &[k]))).find(|t| *t > 0.0),
                unit: j::text(field(u, &["sizeUnit", "unityType"])),
                used: j::num(field(u, &["consumedSize"])),
                // `renewalDate` is text like "Renews on 01 Dec" here.
                ends: date(field(u, &["expiryDate"])).or_else(|| date(field(u, &["renewalDate"]))),
                ..Reading::new(name, kind)
            })
        })
        .collect()
}

/// `extraUnits`: what's left of extra units bought on top, "Extra units (x3)".
fn extra_units(x: &Value, name: &str) -> Vec<Reading> {
    if field(x, &["remainingSize"]).is_null() {
        return Vec::new();
    }
    let count = j::text(field(x, &["countOfExtraUnits"])).filter(|c| c != "0");
    let name = count.map_or_else(|| name.to_string(), |c| format!("{name} (x{c})"));
    vec![Reading {
        remaining: j::num(field(x, &["remainingSize"])),
        unit: j::text(field(x, &["remainingSizeUnit", "unityType"])),
        ends: date(field(x, &["expiryDate"])),
        ..Reading::new(name, BundleKind::Addon)
    }]
}

/// One of `PostpaidInfoService.svc/GetConsumption`'s `Units`: "Kart El Kebir Bundles",
/// 425 Units until 30/11/2026. `TotalSize` is 0 and `Usage` minus what's left when Orange
/// doesn't give a size.
fn wcf_unit(u: &Value) -> Option<Reading> {
    let name = j::text(field(u, &["Name", "Title"]))?;
    Some(Reading {
        remaining: j::num(field(u, &["Remaining"])),
        total: j::num(field(u, &["TotalSize"])),
        unit: j::text(field(u, &["UOMString"])),
        used: j::num(field(u, &["Usage"])),
        ends: date(field(u, &["ExpiryDate"])),
        ..Reading::new(name, BundleKind::Data)
    })
}

/// `BucketsService.svc/GetCurrentActiveBuckets`: data buckets, the GO bucket and GO passes.
fn wcf_buckets(v: &Value) -> Vec<Reading> {
    let one = |b: &Value, kind: BundleKind| {
        let name = j::text(field(b, &["BucketName", "Name", "BucketCategoryName"]))?;
        Some(Reading {
            remaining: j::num(field(b, &["RemainingValue", "Remaining"])),
            total: j::num(field(b, &["BucketSize", "BucketSizeValue"])),
            unit: j::text(field(b, &["RemainingValueUnit", "RemainingUnit", "BucketSizeUnit"])),
            total_unit: j::text(field(b, &["BucketSizeUnit"])),
            used: j::num(field(b, &["BucketUsage"])),
            ends: ["ExpiryDate", "Validity", "RenewalDate"].into_iter().find_map(|k| date(field(b, &[k]))),
            ..Reading::new(name, kind)
        })
    };
    let listed = field(v, &["Buckets"]).as_array().into_iter().flatten().filter_map(|b| one(b, BundleKind::Data));
    let go = Some(field(v, &["GoBucket"])).filter(|b| b.is_object()).and_then(|b| one(b, BundleKind::Data));
    let passes = field(v, &["GoPasses"]).as_array().into_iter().flatten().filter_map(|b| one(b, BundleKind::Addon));
    listed.chain(go).chain(passes).collect()
}

/// Bundles from several sources, each once. A later source only adds the copies of a name
/// beyond what the earlier ones had: three "Extra 1GB" in one and two in another are three.
fn merge(sources: Vec<Vec<Reading>>) -> Vec<Bundle> {
    let mut merged: Vec<Bundle> = Vec::new();
    for source in sources {
        let mut have: BTreeMap<String, usize> = BTreeMap::new();
        for b in &merged {
            *have.entry(name_key(&b.name)).or_default() += 1;
        }
        let mut seen: BTreeMap<String, usize> = BTreeMap::new();
        for bundle in source.into_iter().filter_map(Reading::bundle) {
            let key = name_key(&bundle.name);
            let count = seen.entry(key.clone()).or_default();
            *count += 1;
            if *count > have.get(&key).copied().unwrap_or(0) {
                merged.push(bundle);
            }
        }
    }
    merged
}

/// A name for matching across sources: lowercase letters and digits (Arabic too), without
/// anything in brackets. "GO 92 (4250 MBs)" and "GO 92" match.
fn name_key(name: &str) -> String {
    let mut depth = 0usize;
    let mut out = String::new();
    for c in name.chars().flat_map(char::to_lowercase) {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if depth > 0 => {}
            c if c.is_ascii_alphanumeric() || ('\u{0600}'..='\u{06FF}').contains(&c) => out.push(c),
            _ => {}
        }
    }
    out
}

/// The last size written in a name: "GO 92 (4250 MBs)" is 4250 MB.
fn size_in_name(name: &str) -> Option<(f64, String)> {
    static SIZE: std::sync::LazyLock<Option<regex::Regex>> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"(?i)(\d[\d,]*(?:\.\d+)?)\s*(GB|MB|KB|Units?|Minutes|Mins?|SMS)s?\b").ok());
    let caps = SIZE.as_ref()?.captures_iter(name).last()?;
    let value = caps[1].replace(',', "").parse().ok()?;
    Some((value, caps[2].to_string()))
}

/// The first of `keys` with a value, whatever their case; `Null` without one.
fn field<'a>(v: &'a Value, keys: &[&str]) -> &'a Value {
    let Some(map) = v.as_object() else { return &Value::Null };
    keys.iter()
        .find_map(|key| map.iter().find(|(k, val)| k.eq_ignore_ascii_case(key) && !val.is_null() && val.as_str() != Some("")).map(|(_, val)| val))
        .unwrap_or(&Value::Null)
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
    let addons = ["Extensions", "Compensations", "DoubleQuota"]
        .into_iter()
        .flat_map(|k| match &v[k] {
            Value::Array(items) => items.iter().collect::<Vec<_>>(),
            item @ Value::Object(_) => vec![item],
            _ => Vec::new(),
        })
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
            valid_until: None,
        },
        payments: None,
    })
}

/// A home internet package or add-on: `Balance` (its size), `Consumed` and `Remaining` in
/// `RemainingValueUnit` (bytes), until `NextRecharge`.
fn quota(p: &Value, kind: BundleKind) -> Option<Bundle> {
    let name = j::text(&p["Name"])?;
    Reading {
        remaining: j::num(&p["Remaining"]),
        total: j::num(&p["Balance"]),
        unit: Some(j::text(&p["RemainingValueUnit"]).unwrap_or_else(|| "bytes".into())),
        used: j::num(&p["Consumed"]),
        ends: date(&p["NextRecharge"]),
        ..Reading::new(name, kind)
    }
    .bundle()
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

/// `amount` in `from`, in `to`: the same unit, or MB and GB. Anything else doesn't convert.
fn convert(amount: f64, from: Unit, to: Unit) -> Option<f64> {
    match (from, to) {
        _ if from == to => Some(amount),
        (Unit::Mb, Unit::Gb) => Some(amount / 1024.0),
        (Unit::Gb, Unit::Mb) => Some(amount * 1024.0),
        _ => None,
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

    fn mobile_sources() -> MobileSources {
        MobileSources {
            tariffs: fixture("or_tariffs.json"),
            units: fixture("or_units.json"),
            activated: fixture("or_activated.json"),
            internet: fixture("or_internet_none.json"),
            buckets: fixture("or_buckets_none.json"),
        }
    }

    #[test]
    fn reads_a_units_line() {
        let line = parse_dial_info(&fixture("or_dial_info.json")).unwrap();
        assert_eq!((line.rp_code.as_deref(), line.service_class.as_deref()), (Some("20"), Some("1062")));
        assert_eq!((line.plan.as_deref(), line.payment, line.balance), (Some("FREEmax 80"), Some(PaymentKind::Prepaid), Some(0.0)));
        assert_eq!(local(line.valid_until), "2027-03-08 23:59:59");

        let usage = mobile_usage(&line, &mobile_sources(), Some("Mostafa Hassan".into()), "01200000000", Utc::now()).unwrap();
        // Kart El Kebir once, though both units services list it, and without a size it's
        // shown full. The GO bundle waits for a recharge; its size is the one in its name.
        let rows: Vec<_> = usage.bundles.iter().map(|b| (b.name.as_str(), b.kind, b.unit, b.remaining, b.total, b.state)).collect();
        assert_eq!(
            rows,
            [
                ("Kart El Kebir Bundles", BundleKind::Other, Unit::Units, 388.0, 388.0, BundleState::Active),
                ("GO 92 (4250 MBs)", BundleKind::Data, Unit::Mb, 0.0, 4250.0, BundleState::Inactive),
            ]
        );
        assert_eq!(local(usage.bundles[0].expires_at), "2026-11-30 23:59:59");
        assert_eq!((usage.package_name.as_deref(), usage.balance_egp), (Some("FREEmax 80"), Some(0.0)));
        assert_eq!(usage.renews_at, usage.bundles[0].expires_at);
        assert_eq!((usage.info.plan.as_deref(), usage.info.payment), (Some("FREEmax 80"), Some(PaymentKind::Prepaid)));
        assert_eq!((usage.info.number.as_deref(), usage.info.valid_until), (Some("01200000000"), line.valid_until));
        assert_eq!(usage.owner_name.as_deref(), Some("Mostafa Hassan"));
    }

    #[test]
    fn an_account_without_an_orange_mobile_line() {
        // "None Mobinil": the account's number isn't an Orange line.
        assert_eq!(parse_dial_info(&fixture("or_dial_info_not_orange.json")), Err(FetchError::NoLine(LineType::Mobile)));
        assert_eq!(parse_dial_info(&json!({"ErrorCode": 12, "IsMobinil": false})), Err(FetchError::NoLine(LineType::Mobile)));
        // Like the app, a line that says it isn't Orange's but reports no error still counts.
        let mut v = fixture("or_dial_info.json");
        v["IsMobinil"] = json!(false);
        assert!(parse_dial_info(&v).is_ok());
        let busy = json!({"ErrorCode": 99, "ErrorDescription": "Service is temporarily unavailable"});
        assert_eq!(parse_dial_info(&busy), Err(FetchError::Carrier("Service is temporarily unavailable".into())));
        assert!(matches!(parse_dial_info(&json!({"ErrorCode": 12, "ErrorDescription": null})), Err(FetchError::Unexpected(_))));
    }

    #[test]
    fn replies_with_errors_add_nothing() {
        let line = parse_dial_info(&fixture("or_dial_info.json")).unwrap();
        let refused = MobileSources {
            tariffs: json!({"errorCode": 7, "internalErrorDescription": "No Units Found", "units": null}),
            units: json!({"GetConsumptionResult": {"ErrorCode": 15, "ErrorDescription": "RP code is empty", "Units": null}}),
            activated: Value::Null,
            internet: fixture("or_internet_none.json"),
            buckets: json!({"GetCurrentActiveBucketsResult": {"ErrorCode": 14, "ErrorDescription": "CheckDataProfileStatus returned Error", "Buckets": null}}),
        };
        assert!(matches!(mobile_usage(&line, &refused, None, "01200000000", Utc::now()), Err(FetchError::Unexpected(_))));
        let only_units = MobileSources { units: fixture("or_units.json"), ..MobileSources::default() };
        assert_eq!(mobile_usage(&line, &only_units, None, "01200000000", Utc::now()).unwrap().bundles.len(), 1);
    }

    #[test]
    fn reads_every_kind_of_internet_bundle() {
        let line = parse_dial_info(&fixture("or_dial_info.json")).unwrap();
        let sources = MobileSources {
            activated: json!({
                "mainBucket": {"isRnR": false, "bucketName": "GO 200", "bucketSize": 12, "remainingSize": 4.5, "sizeUnit": "GB", "expiryDate": "12/12/2026"},
                "addOnBuckets": [{"bucketName": "Extra 1GB", "bucketSize": 1024, "remainingSize": 300, "sizeUnit": "MB"}],
                "socialBuckets": [{"bucketName": "Social 500", "bucketSize": 500, "remainingSize": 120, "sizeUnit": "MB"}, {"bucketName": "Banner"}],
                "outOfBucket": {"bucketName": "Out of bundle", "remainingSize": 0},
                "errorCode": 0
            }),
            internet: json!({"errorCode": 0, "internetConsumption": {
                "buckets": [{"bucketName": "GO 200", "bucketSize": 12, "remainingSize": 4.5, "sizeUnit": "GB"}, {"bucketName": "Extra 1GB", "bucketSize": 1024, "remainingSize": 1000, "sizeUnit": "MB"}],
                "extraUnits": {"remainingSize": 250, "remainingSizeUnit": "MB", "countOfExtraUnits": 2}
            }, "roamingConsumption": {"buckets": [{"bucketName": "Roaming 1GB", "bucketSize": 1, "remainingSize": 1, "sizeUnit": "GB"}]}}),
            buckets: json!({"GetCurrentActiveBucketsResult": {"ErrorCode": 0,
                "Buckets": [{"BucketName": "GO 200", "BucketSize": "12", "BucketSizeUnit": "GB", "RemainingValue": "4608", "RemainingValueUnit": "MB"}],
                "GoBucket": {"BucketName": "Go Pass", "BucketSize": 500, "BucketSizeUnit": "MB", "RemainingValue": 0, "RemainingValueUnit": "MB", "RenewalDate": "/Date(1798761600000+0200)/"},
                "GoPasses": null}}),
            ..MobileSources::default()
        };
        let usage = mobile_usage(&line, &sources, None, "01200000000", Utc::now()).unwrap();
        let rows: Vec<_> = usage.bundles.iter().map(|b| (b.name.as_str(), b.kind, b.unit, b.remaining, b.total)).collect();
        // The GO bundle and its add-on once each, though the sources list them up to three
        // times; the first source's numbers win. The bucket list Orange added is read too,
        // and the GO pass only the older service has. An empty out-of-bundle counter is left
        // out. Largest with something left first.
        assert_eq!(
            rows,
            [
                ("GO 200", BundleKind::Data, Unit::Gb, 4.5, 12.0),
                ("Extra 1GB", BundleKind::Addon, Unit::Mb, 300.0, 1024.0),
                ("Roaming 1GB", BundleKind::Addon, Unit::Gb, 1.0, 1.0),
                ("Social 500", BundleKind::Addon, Unit::Mb, 120.0, 500.0),
                ("Go Pass", BundleKind::Data, Unit::Mb, 0.0, 500.0),
                ("Extra internet (x2)", BundleKind::Addon, Unit::Mb, 250.0, 250.0),
            ]
        );
        assert_eq!(local(usage.bundles[0].expires_at), "2026-12-12 23:59:59");
        assert_eq!(usage.bundles[4].expires_at.unwrap().timestamp(), 1_798_761_600);
        // No plan name here: the package is the main bundle.
        let unnamed = DialInfo { plan: None, ..line };
        assert_eq!(mobile_usage(&unnamed, &sources, None, "01200000000", Utc::now()).unwrap().package_name.as_deref(), Some("GO 200"));
    }

    #[test]
    fn sizes_come_from_the_api_then_usage_then_the_name() {
        let reading = |remaining: Option<f64>, total: Option<f64>, used: Option<f64>, unit: &str| Reading {
            remaining,
            total,
            used,
            unit: Some(unit.into()),
            ..Reading::new("GO 92 (4250 MBs)".into(), BundleKind::Data)
        };
        let sized = |r: Reading| r.bundle().map(|b| (b.unit, b.remaining, b.total));
        assert_eq!(sized(reading(Some(1000.0), Some(2048.0), None, "MB")), Some((Unit::Mb, 1000.0, 2048.0)));
        // Never less than what's left.
        assert_eq!(sized(reading(Some(1000.0), Some(2.0), None, "MB")), Some((Unit::Mb, 1000.0, 1000.0)));
        // A size in another unit converts; a size Klue can't convert is left out.
        assert_eq!(sized(Reading { total_unit: Some("GB".into()), ..reading(Some(1000.0), Some(2.0), None, "MB") }), Some((Unit::Mb, 1000.0, 2048.0)));
        assert_eq!(sized(Reading { total_unit: Some("Units".into()), ..reading(Some(10.0), Some(2.0), None, "Minutes") }), Some((Unit::Minutes, 10.0, 10.0)));
        assert_eq!(sized(reading(Some(1000.0), Some(0.0), Some(3000.0), "MB")), Some((Unit::Mb, 1000.0, 4000.0)));
        // Usage given as minus what's left doesn't count; the name has the size.
        assert_eq!(sized(reading(Some(1000.0), Some(0.0), Some(-1000.0), "MB")), Some((Unit::Mb, 1000.0, 4250.0)));
        // No size anywhere: shown full.
        let plain = Reading { remaining: Some(388.0), unit: Some("Unit".into()), ..Reading::new("Kart El Kebir Bundles".into(), BundleKind::Data) };
        assert_eq!(sized(plain), Some((Unit::Units, 388.0, 388.0)));
        assert_eq!(Reading::new("Nothing".into(), BundleKind::Data).bundle(), None);
        assert_eq!(size_in_name("GO 92 (4,250 MBs)"), Some((4250.0, "MB".into())));
        assert_eq!(size_in_name("Home Internet 660"), None);
    }

    #[test]
    fn merging_keeps_each_bundle_once() {
        let r = |name: &str, left: f64| Reading { remaining: Some(left), total: Some(1024.0), unit: Some("MB".into()), ..Reading::new(name.into(), BundleKind::Addon) };
        let merged = merge(vec![vec![r("Extra 1GB", 1.0), r("Extra 1GB", 2.0)], vec![r("Extra 1GB", 9.0), r("extra 1gb", 9.0), r("Extra 1GB (new)", 3.0), r("Social", 4.0)]]);
        let names: Vec<_> = merged.iter().map(|b| (b.name.as_str(), b.remaining)).collect();
        assert_eq!(names, [("Extra 1GB", 1.0), ("Extra 1GB", 2.0), ("Extra 1GB (new)", 3.0), ("Social", 4.0)]);
        assert_eq!(name_key("GO 92 (4250 MBs)"), name_key("go 92"));
        assert_eq!(name_key("باقة 5 جيجا"), "باقة5جيجا");
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

        // DoubleQuota comes as one object, not a list.
        v["DoubleQuota"] = json!({"Name": "Double Quota", "Balance": "10737418240", "Consumed": "0", "Remaining": "10737418240", "RemainingValueUnit": "bytes", "NextRecharge": "20261216000000"});
        let usage = home_internet_usage(&v, &landline, None, Utc::now()).unwrap();
        assert!(usage.bundles.iter().any(|b| (b.name.as_str(), b.total) == ("Double Quota", 10.0)));

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
