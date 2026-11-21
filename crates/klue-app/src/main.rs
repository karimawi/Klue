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
    notify::init();

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

    let icon: Option<Icon> = dioxus::desktop::icon_from_memory(include_bytes!("../icons/klue-256.png")).ok();
    let window = WindowBuilder::new()
        .with_title("Klue")
        .with_inner_size(LogicalSize::new(1200.0, 760.0))
        .with_min_inner_size(LogicalSize::new(980.0, 640.0))
        .with_visible(!minimized)
        .with_window_icon(icon.clone());
    let window = without_native_title_bar(window);

    let mut cfg = Config::new()
        .with_window(window)
        .with_custom_head(assets::head())
        .with_disable_context_menu(!cfg!(debug_assertions))
        .with_background_color(theme::window_rgba(settings.theme))
        .with_tray_icon_show_window_on_click(false)
        .with_data_directory(webview_dir)
        .with_close_behaviour(if settings.keep_in_tray { WindowCloseBehaviour::WindowHides } else { WindowCloseBehaviour::WindowCloses });
    // macOS keeps its (screen-top) menu bar: without an Edit menu, ⌘C/⌘V don't work in text fields.
    if !cfg!(target_os = "macos") {
        cfg = cfg.with_menu(None);
    }
    if let Some(icon) = icon {
        cfg = cfg.with_icon(icon);
    }

    dioxus::LaunchBuilder::desktop().with_cfg(cfg).with_context(shared).launch(ui::App);
}

/// Klue draws its own title bar (theme and language switches, window buttons), so the
/// native one is hidden everywhere.
fn without_native_title_bar(window: WindowBuilder) -> WindowBuilder {
    // macOS: keep a real titled window (rounded corners, shadow, resizable edges) but make
    // the title bar transparent, hide its title and traffic lights, and let the content
    // extend underneath it.
    #[cfg(target_os = "macos")]
    {
        use dioxus::desktop::tao::platform::macos::WindowBuilderExtMacOS;
        window
            .with_decorations(true)
            .with_titlebar_transparent(true)
            .with_title_hidden(true)
            .with_titlebar_buttons_hidden(true)
            .with_fullsize_content_view(true)
    }
    // Windows: frameless, but keep the drop shadow.
    #[cfg(target_os = "windows")]
    {
        use dioxus::desktop::tao::platform::windows::WindowBuilderExtWindows;
        window.with_decorations(false).with_undecorated_shadow(true)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        window.with_decorations(false)
    }
}
