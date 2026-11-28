//! Klue desktop app.
//!
//! Flags: `--demo` (or `KLUE_DEMO=1`), `--minimized` (start in the tray), `--quick-view`
//! (open the tray popup), `--debug` (log everything, with a console). `KLUE_DEBUG_DIR=<dir>`
//! saves raw carrier responses with secrets redacted.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod about;
mod app;
mod assets;
mod autostart;
mod console;
mod i18n;
mod install;
mod instance;
mod notify;
mod product;
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
    // --debug logs everything, minus iced/winit window dumps (icon pixels included) and
    // cosmic-text layout timings. RUST_LOG overrides it.
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

    // Started by an update: wait for the old instance to exit.
    if args.iter().any(|a| a == "--after-update") {
        if !instance::wait_until_gone(std::time::Duration::from_secs(30)) {
            tracing::warn!("the old Klue is still running after an update");
        }
        updater::cleanup();
    }

    // A second launch brings the running one forward.
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
    let shared = shared::new(store, demo);
    app::run(shared, minimized || quick_view, quick_view, debug)
}
