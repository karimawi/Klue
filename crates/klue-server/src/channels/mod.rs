//! Where alerts and reports go besides the apps: Telegram, email and webhooks.

use std::sync::Arc;

use axum::Router;
use chrono::Utc;
use klue_core::{
    alerts::AlertEvent,
    store::{Account, Lang},
};
use klue_engine::remote::ReportPeriod;
use klue_report::{Look, Period};

use crate::state::State;

/// The channels' own routes (test messages, Telegram links).
pub fn routes() -> Router<Arc<State>> {
    Router::new()
}

/// Starts what runs on its own (the Telegram bot).
pub fn start(_state: &Arc<State>) {}

/// Settings changed: channels that run on their own pick them up.
pub fn restart(_state: &Arc<State>) {}

pub async fn secret_changed(_state: &Arc<State>, _name: &str) {}

pub async fn alert(_state: &Arc<State>, _account: &Account, _event: &AlertEvent) {}

pub async fn daily_report_if_due(_state: &Arc<State>) {}

pub async fn captcha(_state: &Arc<State>, _id: &str, _image: &[u8]) {}

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
