//! Klue desktop app.
//!
//! Flags: `--demo` (or `KLUE_DEMO=1`) shows sample numbers without touching your data;
//! `--minimized` starts in the tray (used by "Launch at startup"); `--quick-view` opens
//! the tray popup.
//! `KLUE_DEBUG_DIR=<dir>` writes raw carrier responses (secrets redacted) for debugging.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod assets;
mod autostart;
mod demo;
mod i18n;
mod instance;
mod notify;
mod shared;
mod sound;
mod systray;
mod theme;
mod ui;

use klue_core::store::Store;
use tracing_subscriber::EnvFilter;

fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("klue=info,klue_core=info")))
        .init();

    let args: Vec<String> = std::env::args().collect();
    let demo = args.iter().any(|a| a == "--demo") || std::env::var_os("KLUE_DEMO").is_some();
    let minimized = args.iter().any(|a| a == "--minimized");
    let quick_view = args.iter().any(|a| a == "--quick-view");

    // A second launch just brings the running Klue forward.
    if !demo && matches!(instance::claim(quick_view), instance::Start::Second) {
        tracing::info!("Klue is already running; showing it");
        return Ok(());
    }

    let store = if demo { Store::open(std::env::temp_dir().join("klue-demo")) } else { Store::open_default() };
    let store = match store {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("can't open the data directory: {e}");
            std::process::exit(1);
        }
    };
    notify::init(store.dir());
    let shared = shared::Shared::new(store, demo);
    app::run(shared, minimized || quick_view, quick_view)
}
