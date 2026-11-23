//! The debug console's log (`--debug`): every tracing event, kept in memory, oldest
//! dropped first.

use std::{
    collections::VecDeque,
    fmt::Write as _,
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use chrono::{DateTime, Local};
use tracing::{Level, field::Field};
use tracing_subscriber::layer::Context;

/// Lines kept.
const CAP: usize = 20_000;
/// Longer messages are cut.
const MAX_TEXT: usize = 4_000;

static LINES: Mutex<VecDeque<Line>> = Mutex::new(VecDeque::new());
/// Bumped on every change, so the console knows when to re-read.
static REVISION: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub at: DateTime<Local>,
    pub level: Level,
    pub target: String,
    pub text: String,
}

impl Line {
    /// The line as copied to the clipboard.
    pub fn plain(&self) -> String {
        format!("{} {:>5} {}: {}", self.at.format("%H:%M:%S%.3f"), self.level, self.target, self.text)
    }
}

/// A tracing layer that records every event.
pub struct Capture;

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for Capture {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let meta = event.metadata();
        let mut text = Text::default();
        event.record(&mut text);
        // Events from the `log` crate name their real source in a field.
        let target = text.log_target.take().unwrap_or_else(|| meta.target().to_string());
        let mut text = text.finish();
        if text.len() > MAX_TEXT {
            let cut = (0..=MAX_TEXT).rev().find(|i| text.is_char_boundary(*i)).unwrap_or(0);
            let more = text.len() - cut;
            text.truncate(cut);
            let _ = write!(text, "… ({more} more bytes)");
        }
        push(Line { at: Local::now(), level: *meta.level(), target, text });
    }
}

fn push(line: Line) {
    if let Ok(mut lines) = LINES.lock() {
        if lines.len() >= CAP {
            lines.pop_front();
        }
        lines.push_back(line);
    }
    REVISION.fetch_add(1, Ordering::Relaxed);
}

pub fn revision() -> u64 {
    REVISION.load(Ordering::Relaxed)
}

/// The last `limit` lines that pass `keep`, oldest first.
pub fn lines(limit: usize, keep: impl Fn(&Line) -> bool) -> Vec<Line> {
    let Ok(lines) = LINES.lock() else { return Vec::new() };
    let mut picked: Vec<Line> = lines.iter().rev().filter(|l| keep(l)).take(limit).cloned().collect();
    picked.reverse();
    picked
}

pub fn clear() {
    if let Ok(mut lines) = LINES.lock() {
        lines.clear();
    }
    REVISION.fetch_add(1, Ordering::Relaxed);
}

/// An event's message followed by its other fields as `name=value`.
#[derive(Default)]
struct Text {
    message: String,
    fields: String,
    log_target: Option<String>,
}

impl Text {
    fn finish(self) -> String {
        if self.fields.is_empty() { self.message } else { format!("{}{}", self.message, self.fields) }
    }
}

impl tracing::field::Visit for Text {
    fn record_str(&mut self, field: &Field, value: &str) {
        match field.name() {
            "message" => self.message.push_str(value),
            "log.target" => self.log_target = Some(value.to_string()),
            "log.module_path" | "log.file" | "log.line" => {}
            name => {
                let _ = write!(self.fields, " {name}={value}");
            }
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        match field.name() {
            "message" => {
                let _ = write!(self.message, "{value:?}");
            }
            "log.target" => self.log_target = Some(format!("{value:?}").trim_matches('"').to_string()),
            "log.module_path" | "log.file" | "log.line" => {}
            name => {
                let _ = write!(self.fields, " {name}={value:?}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use tracing_subscriber::layer::SubscriberExt;

    use super::*;

    #[test]
    fn records_events_with_their_fields_and_keeps_the_newest() {
        let subscriber = tracing_subscriber::registry().with(Capture);
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(count = 3, "background refresh");
            tracing::warn!("couldn't play {}", "negative.wav");
        });
        let mine = lines(10, |l| l.target.contains("console"));
        let texts: Vec<_> = mine.iter().map(|l| (l.level, l.text.as_str())).collect();
        assert!(texts.contains(&(Level::INFO, "background refresh count=3")));
        assert!(texts.contains(&(Level::WARN, "couldn't play negative.wav")));
        // The other test shares the log and may add its line after these.
        let warning = mine.iter().find(|l| l.text.starts_with("couldn't play")).unwrap();
        assert!(warning.plain().contains(" WARN "));
    }

    #[test]
    fn cuts_huge_messages() {
        let subscriber = tracing_subscriber::registry().with(Capture);
        tracing::subscriber::with_default(subscriber, || tracing::info!("huge {}", "é".repeat(10_000)));
        let line = lines(1, |l| l.text.starts_with("huge ")).pop().unwrap();
        assert!(line.text.len() < MAX_TEXT + 40 && line.text.ends_with("more bytes)"), "{}", line.text.len());
    }
}
