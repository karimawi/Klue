//! Where alerts and reports go besides the apps: Telegram, email and webhooks.

pub mod email;
pub mod telegram;
pub mod text;
pub mod webhooks;

use std::{path::Path, sync::Arc};

use axum::{
    Json, Router,
    extract::{Path as UrlPath, State as Axum},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
};
use chrono::{Local, NaiveDate, NaiveTime, Utc};
use klue_core::{
    alerts::AlertEvent,
    store::{Account, Lang},
};
use klue_engine::remote::{Failure, ReportPeriod, Sent, secret};
use klue_report::{Look, Period};
use serde::{Deserialize, Serialize};

use crate::state::State;

/// The channels' own routes: test messages, and links to add Telegram chats.
pub fn routes() -> Router<Arc<State>> {
    Router::new().route("/v1/test/{channel}", post(test)).route("/v1/telegram/link", post(telegram_link))
}

/// Starts what runs on its own: the Telegram bot.
pub fn start(state: &Arc<State>) {
    tokio::spawn(telegram::run(state.clone()));
}

/// Settings changed: the bot picks them up.
pub fn restart(state: &Arc<State>) {
    state.telegram.restart.notify_one();
}

pub async fn secret_changed(state: &Arc<State>, name: &str) {
    if name == secret::TELEGRAM_TOKEN {
        if state.vault.get(secret::TELEGRAM_TOKEN).is_none() {
            state.update_settings(|s| {
                s.telegram.bot = None;
                s.telegram.chats.clear();
            });
        }
        restart(state);
    }
}

/// An alert, to each channel set up for its kind. Sent in the background, so a slow mail
/// server doesn't hold up the next check.
pub async fn alert(state: &Arc<State>, account: &Account, event: &AlertEvent) {
    let settings = state.settings();
    let (state, account, event) = (state.clone(), account.clone(), event.clone());
    tokio::spawn(async move {
        let look = look(&state, None);
        if settings.telegram.alerts.wants(&event) && !settings.telegram.chats.is_empty() {
            let result = telegram::alert(&state, &account, &event).await;
            state.channel_result("Telegram", result);
        }
        if settings.email.enabled && settings.email.alerts.wants(&event) {
            let result = email::send(&state, email::alert_mail(&account, &event, &look)).await;
            state.channel_result("Email", result);
        }
        for hook in settings.webhooks.iter().filter(|h| h.alerts.wants(&event)) {
            let content = webhooks::alert_content(&account, &event, &look);
            let result = webhooks::send(&state, hook, &content, None).await;
            state.channel_result(&format!("Webhook {}", hook.name), result);
        }
    });
}

/// When the daily report last went out, kept so a restart doesn't send it twice.
#[derive(Default, Serialize, Deserialize)]
struct Schedule {
    daily: Option<NaiveDate>,
}

const SCHEDULE_FILE: &str = "schedule.json";

fn schedule(dir: &Path) -> Schedule {
    std::fs::read(dir.join(SCHEDULE_FILE)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// Sends the daily report once a day, at its time in the server's time zone, to the
/// channels it's turned on for.
pub async fn daily_report_if_due(state: &Arc<State>) {
    let settings = state.settings();
    let wanted = (settings.email.enabled && settings.email.daily_report)
        || (settings.telegram.daily_report && !settings.telegram.chats.is_empty())
        || settings.webhooks.iter().any(|h| h.daily_report);
    let at = NaiveTime::parse_from_str(settings.report.daily_at.trim(), "%H:%M").unwrap_or(NaiveTime::from_hms_opt(21, 30, 0).expect("a time"));
    let now = Local::now();
    let today = now.date_naive();
    if !wanted || now.time() < at || schedule(&state.data_dir).daily == Some(today) {
        return;
    }
    if let Ok(bytes) = serde_json::to_vec(&Schedule { daily: Some(today) }) {
        let _ = std::fs::write(state.data_dir.join(SCHEDULE_FILE), bytes);
    }
    let lines = report_lines(state);
    if lines.is_empty() {
        return;
    }
    let Some(pdf) = report_pdf(state, None, None).await else {
        state.channel_result("Daily report", Err("couldn't make the report".into()));
        return;
    };
    let look = look(state, None);
    tracing::info!("sending the daily report");
    if settings.email.enabled && settings.email.daily_report {
        let result = email::send(state, email::report_mail(&lines, &look, pdf.clone())).await;
        state.channel_result("Email", result);
    }
    if settings.telegram.daily_report && !settings.telegram.chats.is_empty() {
        let result = telegram::daily(state, &pdf, &lines).await;
        state.channel_result("Telegram", result);
    }
    let file = text::report_file(&look);
    for hook in settings.webhooks.iter().filter(|h| h.daily_report) {
        let content = webhooks::report_content(&lines, &look);
        let result = webhooks::send(state, hook, &content, Some((&file, &pdf))).await;
        state.channel_result(&format!("Webhook {}", hook.name), result);
    }
}

pub async fn captcha(state: &Arc<State>, id: &str, image: &[u8]) {
    telegram::captcha(state, id, image).await;
}

/// How messages and pictures look: the server's language and clock, now, its name.
pub fn look(state: &State, lang: Option<Lang>) -> Look {
    let s = state.settings();
    Look { lang: lang.unwrap_or(s.lang), clock: s.clock, now: Utc::now(), server: Some(s.name) }
}

/// The lines reports cover: the ones picked in Settings → Reports, or every line.
pub fn report_lines(state: &State) -> Vec<Account> {
    let picked = state.settings().report.lines;
    let data = state.engine.snapshot().data;
    data.accounts.into_iter().filter(|a| !a.archived && picked.as_ref().is_none_or(|ids| ids.contains(&a.id))).collect()
}

/// The PDF report, made off the async threads since drawing it takes a moment.
pub async fn report_pdf(state: &Arc<State>, lang: Option<Lang>, period: Option<ReportPeriod>) -> Option<Vec<u8>> {
    let look = look(state, lang);
    let period = match period.unwrap_or(state.settings().report.period) {
        ReportPeriod::Cycle => Period::Cycle,
        ReportPeriod::Week => Period::Week,
        ReportPeriod::Month => Period::Month,
    };
    let lines = report_lines(state);
    tokio::task::spawn_blocking(move || {
        let refs: Vec<&Account> = lines.iter().collect();
        klue_report::report_pdf(&refs, period, &look)
    })
    .await
    .ok()
    .flatten()
}

/// Settings → Send a test: `telegram`, `email`, `webhook-<id>` or `report` (the daily
/// report, now, to every channel it's on for).
async fn test(Axum(state): Axum<Arc<State>>, UrlPath(channel): UrlPath<String>) -> Json<Sent> {
    let look = look(&state, None);
    let result = match channel.as_str() {
        "telegram" => telegram::test(&state).await,
        "email" => email::send(&state, email::test_mail(&look)).await,
        "report" => {
            let path = state.data_dir.join(SCHEDULE_FILE);
            let _ = std::fs::remove_file(&path);
            let at = state.settings().report.daily_at.clone();
            state.update_settings(|s| s.report.daily_at = "00:00".into());
            daily_report_if_due(&state).await;
            state.update_settings(|s| s.report.daily_at = at);
            Ok(())
        }
        other => match other.strip_prefix("webhook-").and_then(|id| state.settings().webhooks.into_iter().find(|h| h.id == id)) {
            Some(hook) => webhooks::send(&state, &hook, &webhooks::test_content(&look), None).await,
            None => Err(format!("no channel {other}")),
        },
    };
    let name = match channel.as_str() {
        "telegram" => "Telegram".to_string(),
        "email" => "Email".to_string(),
        other => other.to_string(),
    };
    state.channel_result(&name, result.clone());
    Json(match result {
        Ok(()) => Sent { ok: true, error: None },
        Err(e) => Sent { ok: false, error: Some(e) },
    })
}

async fn telegram_link(Axum(state): Axum<Arc<State>>) -> Response {
    match telegram::link(&state) {
        Some(link) => Json(link).into_response(),
        None => (StatusCode::CONFLICT, Json(Failure::BadRequest { detail: "set the bot's token first".into() })).into_response(),
    }
}
