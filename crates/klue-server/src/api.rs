//! The HTTPS API the app talks to. Everything but `/v1/ping` needs the key.

use std::{convert::Infallible, sync::Arc, time::Duration};

use axum::{
    Json, Router,
    extract::{Path, Query, Request, State as Axum},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{
        IntoResponse, Response,
        sse::{Event as Sse, KeepAlive, Sse as SseBody},
    },
    routing::{get, post, put},
};
use chrono::Utc;
use futures_util::{Stream, StreamExt, stream};
use klue_core::{Carrier, LineType, Usage, store::Lang};
use klue_engine::{
    Op,
    remote::{
        AddRequest, Added, CaptchaAnswer, Copy, DEVICE_ID, DEVICE_NAME, APP_VERSION, Event, Failure, Health, OthersRequest, Problem, ProxySettings, ProxyTest, Reading,
        RefreshRequest, ReportPeriod, SecretUpdate, ServerSettings, VerifyRequest, secret,
    },
};
use serde::Deserialize;
use tokio_stream::wrappers::BroadcastStream;
use tower_http::compression::CompressionLayer;

use crate::state::{self, State};

type Shared = Axum<Arc<State>>;

pub fn router(state: Arc<State>) -> Router {
    let keyed = Router::new()
        .route("/v1/health", get(health))
        .route("/v1/sync", get(sync))
        .route("/v1/events", get(events))
        .route("/v1/ops", post(ops))
        .route("/v1/refresh", post(refresh))
        .route("/v1/lines/verify", post(verify))
        .route("/v1/lines/others", post(others))
        .route("/v1/lines", post(add_line))
        .route("/v1/copy", get(copy_out).post(copy_in))
        .route("/v1/readings", post(readings))
        .route("/v1/erase", post(erase))
        .route("/v1/settings", get(get_settings).put(put_settings))
        .route("/v1/secrets", put(put_secret))
        .route("/v1/proxy/test", post(test_proxy))
        .route("/v1/captcha/{id}", post(captcha))
        .route("/v1/report", get(report))
        .merge(crate::channels::routes())
        .layer(middleware::from_fn_with_state(state.clone(), key));
    Router::new().route("/v1/ping", get(|| async { "ok" })).merge(keyed).layer(CompressionLayer::new()).with_state(state)
}

/// Lets in requests with the key, and notes which device sent them.
async fn key(Axum(state): Shared, request: Request, next: Next) -> Response {
    let headers = request.headers();
    let given = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("Bearer ")).unwrap_or_default();
    let secret = state.secret.read().map(|s| s.clone()).unwrap_or_default();
    if secret.is_empty() || !same(given.as_bytes(), secret.as_bytes()) {
        return (StatusCode::UNAUTHORIZED, Json(Failure::BadRequest { detail: "wrong key".into() })).into_response();
    }
    if let Some(id) = text(headers, DEVICE_ID) {
        state.devices.seen(&id, &text(headers, DEVICE_NAME).unwrap_or_else(|| "Klue".into()), text(headers, APP_VERSION).as_deref());
    }
    next.run(request).await
}

fn text(headers: &HeaderMap, name: &str) -> Option<String> {
    headers.get(name).and_then(|v| v.to_str().ok()).map(|s| s.chars().take(80).collect::<String>()).filter(|s| !s.trim().is_empty())
}

/// Compares in constant time, so the key can't be guessed from how long a refusal takes.
fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn failure(status: StatusCode, failure: Failure) -> Response {
    (status, Json(failure)).into_response()
}

fn fetch_failure(e: klue_core::FetchError) -> Response {
    failure(StatusCode::UNPROCESSABLE_ENTITY, Failure::Fetch { detail: e })
}

async fn health(Axum(state): Shared) -> Json<Health> {
    let snap = state.engine.snapshot();
    let failing: Vec<String> = snap.data.accounts.iter().filter(|a| !a.archived && a.last_error.is_some()).map(|a| a.id.clone()).collect();
    let disk_free = fs4::available_space(&state.data_dir).ok();
    let mut problems = Vec::new();
    if !failing.is_empty() {
        problems.push(Problem::LinesFailing { count: failing.len() });
    }
    if let Some(free) = disk_free.filter(|f| *f < 200 * 1024 * 1024) {
        problems.push(Problem::DiskLow { free });
    }
    if let Ok(errors) = state.channel_errors.lock() {
        problems.extend(errors.iter().map(|(channel, error)| Problem::Channel { channel: channel.clone(), error: error.clone() }));
    }
    Json(Health {
        name: state.settings().name,
        version: crate::VERSION.into(),
        started_at: state.started_at,
        now: Utc::now(),
        lines: snap.data.accounts.iter().filter(|a| !a.archived).count(),
        failing,
        last_refresh: snap.data.last_sync(),
        devices: state.devices.list(),
        disk_free,
        problems,
    })
}

#[derive(Deserialize)]
struct Since {
    epoch: Option<String>,
    rev: Option<u64>,
}

async fn sync(Axum(state): Shared, Query(since): Query<Since>) -> Response {
    let data = state.engine.snapshot().data;
    match state.tracker.lock() {
        Ok(t) => Json(t.since(&data, since.epoch.as_deref(), since.rev.unwrap_or(0))).into_response(),
        Err(_) => failure(StatusCode::INTERNAL_SERVER_ERROR, Failure::Internal { detail: "tracker".into() }),
    }
}

/// The event stream: where the server is first, then whatever happens.
async fn events(Axum(state): Shared, headers: HeaderMap) -> SseBody<impl Stream<Item = Result<Sse, Infallible>>> {
    let listening = text(&headers, DEVICE_ID).map(|id| state.devices.listening(&id));
    let (epoch, rev) = state.tracker.lock().map(|t| (t.epoch().to_string(), t.rev())).unwrap_or_default();
    let snap = state.engine.snapshot();
    let first = stream::iter([Event::Rev { epoch, rev }, Event::Refreshing { ids: snap.refreshing, all: snap.refreshing_all }]);
    let rest = BroadcastStream::new(state.events.subscribe()).filter_map(|e| async move { e.ok() });
    let body = first.chain(rest).map(move |event| {
        // Kept alive with the stream, so the device counts as listening until it closes.
        let _listening = &listening;
        Ok(Sse::default().json_data(&event).unwrap_or_default())
    });
    SseBody::new(body).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

async fn ops(Axum(state): Shared, Json(ops): Json<Vec<Op>>) -> StatusCode {
    for op in ops {
        state.engine.apply(op);
    }
    StatusCode::NO_CONTENT
}

async fn refresh(Axum(state): Shared, Json(r): Json<RefreshRequest>) -> StatusCode {
    tokio::spawn(async move {
        match r.ids {
            None => state.engine.refresh_all().await,
            Some(ids) => {
                for id in ids {
                    state.engine.refresh(&id, true).await;
                }
            }
        }
    });
    StatusCode::ACCEPTED
}

async fn verify(Axum(state): Shared, Json(r): Json<VerifyRequest>) -> Response {
    match state.engine.verify(r.carrier, r.line, &r.number, &r.password).await {
        Ok(usage) => Json(usage).into_response(),
        Err(e) => fetch_failure(e),
    }
}

async fn others(Axum(state): Shared, Json(r): Json<OthersRequest>) -> Json<Vec<(LineType, Usage)>> {
    Json(state.engine.other_lines(r.carrier, r.line, &r.number, &r.own, &r.password).await)
}

async fn add_line(Axum(state): Shared, Json(r): Json<AddRequest>) -> Response {
    match state.engine.add_account(r.carrier, r.line, &r.number, &r.password, r.nickname, r.usage) {
        Ok(id) => Json(Added { id }).into_response(),
        Err(e) => fetch_failure(e),
    }
}

async fn copy_out(Axum(state): Shared) -> Json<Copy> {
    Json(state.engine.copy_out())
}

async fn copy_in(Axum(state): Shared, Json(copy): Json<Copy>) -> Response {
    Json(state.engine.take_copy(copy)).into_response()
}

async fn readings(Axum(state): Shared, Json(readings): Json<Vec<Reading>>) -> StatusCode {
    state.engine.take_readings(readings);
    StatusCode::NO_CONTENT
}

/// Deletes the lines, passwords and the channels' settings and secrets. The name stays.
async fn erase(Axum(state): Shared) -> StatusCode {
    state.engine.erase();
    for name in state.vault.names() {
        state.vault.set(&name, None);
    }
    let name = state.settings().name;
    state.set_settings(ServerSettings { name, ..ServerSettings::default() });
    crate::channels::restart(&state);
    StatusCode::NO_CONTENT
}

async fn get_settings(Axum(state): Shared) -> Json<ServerSettings> {
    Json(state.settings())
}

async fn put_settings(Axum(state): Shared, Json(settings): Json<ServerSettings>) -> Json<ServerSettings> {
    state.set_settings(settings);
    crate::channels::restart(&state);
    Json(state.settings())
}

/// Secrets are named after what they're for; anything else is refused.
fn known_secret(name: &str) -> bool {
    [secret::TELEGRAM_TOKEN, secret::SMTP_PASSWORD, secret::PROXY_PASSWORD].contains(&name)
        || name.strip_prefix("webhook.").is_some_and(|rest| rest.ends_with(".url") || rest.ends_with(".signing"))
}

async fn put_secret(Axum(state): Shared, Json(update): Json<SecretUpdate>) -> Response {
    if !known_secret(&update.name) {
        return failure(StatusCode::BAD_REQUEST, Failure::BadRequest { detail: format!("unknown secret {}", update.name) });
    }
    let value = update.value.as_deref().map(str::trim).filter(|v| !v.is_empty());
    if !state.vault.set(&update.name, value) {
        return failure(StatusCode::INTERNAL_SERVER_ERROR, Failure::Internal { detail: "couldn't save the vault".into() });
    }
    if update.name == secret::PROXY_PASSWORD {
        state.apply_proxy();
    }
    crate::channels::secret_changed(&state, &update.name).await;
    Json(state.settings()).into_response()
}

/// Reaches each carrier through the proxy given (with the saved password).
async fn test_proxy(Axum(state): Shared, Json(proxy): Json<ProxySettings>) -> Json<Vec<ProxyTest>> {
    let url = state::proxy_url(&proxy.url, &proxy.username, state.vault.get(secret::PROXY_PASSWORD).as_deref());
    let carriers = if proxy.carriers.is_empty() { Carrier::ALL.to_vec() } else { proxy.carriers };
    let tests = carriers.into_iter().map(|carrier| {
        let url = url.clone();
        async move {
            match klue_core::try_proxy(&url, carrier).await {
                Ok(took) => ProxyTest { carrier, millis: Some(took.as_millis() as u64), error: None },
                Err(e) => ProxyTest { carrier, millis: None, error: Some(e.to_string()) },
            }
        }
    });
    Json(futures_util::future::join_all(tests).await)
}

async fn captcha(Axum(state): Shared, Path(id): Path<String>, Json(answer): Json<CaptchaAnswer>) -> StatusCode {
    if state::answer_captcha(&state, &id, answer.code) { StatusCode::NO_CONTENT } else { StatusCode::NOT_FOUND }
}

#[derive(Deserialize)]
struct ReportQuery {
    lang: Option<Lang>,
    period: Option<ReportPeriod>,
}

async fn report(Axum(state): Shared, Query(q): Query<ReportQuery>) -> Response {
    match crate::channels::report_pdf(&state, q.lang, q.period).await {
        Some(pdf) => ([(header::CONTENT_TYPE, "application/pdf"), (header::CONTENT_DISPOSITION, "attachment; filename=\"klue-report.pdf\"")], pdf).into_response(),
        None => failure(StatusCode::INTERNAL_SERVER_ERROR, Failure::Internal { detail: "couldn't make the report".into() }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_known_secrets_are_kept() {
        assert!(known_secret("telegram.token") && known_secret("webhook.abc.url") && known_secret("webhook.abc.signing"));
        assert!(!known_secret("line:we:0223584417") && !known_secret("webhook.abc") && !known_secret("anything"));
        assert!(same(b"abc", b"abc") && !same(b"abc", b"abd") && !same(b"abc", b"ab"));
    }
}
