//! Klue desktop app.
//!
//! Flags: `--demo` (or `KLUE_DEMO=1`) shows the mockup's sample numbers without touching
//! your data; `--minimized` starts in the tray (used by "Launch at startup").
//! `KLUE_DEBUG_DIR=<dir>` writes raw carrier responses (secrets redacted) for debugging.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod assets;
mod autostart;
mod demo;
mod i18n;
mod notify;
mod state;
mod systray;
mod theme;
mod ui;

use dioxus::desktop::{Config, LogicalSize, WindowBuilder, WindowCloseBehaviour, tao::window::Icon};
use klue_core::store::Store;
use tracing_subscriber::EnvFilter;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("klue=info,klue_core=info")))
        .init();

    let args: Vec<String> = std::env::args().collect();
    let demo = args.iter().any(|a| a == "--demo") || std::env::var_os("KLUE_DEMO").is_some();
    let minimized = args.iter().any(|a| a == "--minimized");

    let store = if demo { Store::open(std::env::temp_dir().join("klue-demo")) } else { Store::open_default() };
    let store = match store {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Klue can't open its data directory: {e}");
            std::process::exit(1);
        }
    };
    let webview_dir = store.dir().join("webview");
    let shared = state::Shared::new(store, demo);
    let settings = shared.settings();

    let icon = Icon::from_rgba(assets::app_icon(64), 64, 64).ok();
    let window = WindowBuilder::new()
        .with_title("Klue")
        .with_decorations(false)
        .with_inner_size(LogicalSize::new(1200.0, 760.0))
        .with_min_inner_size(LogicalSize::new(980.0, 640.0))
        .with_visible(!minimized)
        .with_window_icon(icon.clone());

    let mut cfg = Config::new()
        .with_window(window)
        .with_custom_head(assets::head())
        .with_menu(None)
        .with_disable_context_menu(!cfg!(debug_assertions))
        .with_background_color(theme::window_rgba(settings.theme))
        .with_tray_icon_show_window_on_click(false)
        .with_data_directory(webview_dir)
        .with_close_behaviour(if settings.keep_in_tray { WindowCloseBehaviour::WindowHides } else { WindowCloseBehaviour::WindowCloses });
    if let Some(icon) = icon {
        cfg = cfg.with_icon(icon);
    }

    dioxus::LaunchBuilder::desktop().with_cfg(cfg).with_context(shared).launch(ui::App);
}
