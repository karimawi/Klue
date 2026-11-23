//! Klue desktop app.
//!
//! Flags: `--demo` (or `KLUE_DEMO=1`) shows sample numbers without touching your data;
//! `--minimized` starts in the tray (used by "Launch at startup"); `--quick-view` opens
//! the tray popup; `--debug` logs everything and adds a debug console to the title bar.
//! `KLUE_DEBUG_DIR=<dir>` writes raw carrier responses (secrets redacted) for debugging.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod about;
mod app;
mod assets;
mod autostart;
mod console;
mod demo;
mod i18n;
mod install;
mod instance;
mod notify;
mod shared;
mod sound;
mod systray;
mod theme;
mod ui;
mod updater;

use klue_core::store::Store;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

fn main() -> iced::Result {
    let args: Vec<String> = std::env::args().collect();
    // --debug: log everything (Klue's own code in full detail) and keep it for the
    // debug console. A few sources are held back: iced and winit print each new window's
    // settings, icon pixels and all (a quarter of a million lines per window), and
    // cosmic-text times every text layout. RUST_LOG overrides all of it.
    let debug = args.iter().any(|a| a == "--debug");
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new(if debug { "debug,klue=trace,klue_core=trace,iced_winit=warn,winit=warn,cosmic_text=info" } else { "klue=info,klue_core=info" })
    });
    tracing_subscriber::registry().with(filter).with(tracing_subscriber::fmt::layer()).with(debug.then_some(console::Capture)).init();
    if debug {
        tracing::info!("Klue {} started with --debug", about::VERSION);
    }

    let demo = args.iter().any(|a| a == "--demo") || std::env::var_os("KLUE_DEMO").is_some();
    let minimized = args.iter().any(|a| a == "--minimized");
    let quick_view = args.iter().any(|a| a == "--quick-view");

    // Started by an update: the old Klue is quitting; take over once it has gone.
    if args.iter().any(|a| a == "--after-update") {
        if !instance::wait_until_gone(std::time::Duration::from_secs(30)) {
            tracing::warn!("the old Klue is still running after an update");
        }
        updater::cleanup();
    }

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
    if !demo {
        install::register();
    }
    let shared = shared::Shared::new(store, demo);
    app::run(shared, minimized || quick_view, quick_view, debug)
}
