//! The system tray icon, its menu, and the quick-view popup (mockup frames 1s–1u).

use std::{
    cell::Cell,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use dioxus::{
    desktop::{
        Config, DesktopContext, LogicalSize, WindowBuilder, WindowCloseBehaviour,
        tao::dpi::PhysicalPosition,
        trayicon::{
            Icon, MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent,
            menu::{Menu, MenuItem, PredefinedMenuItem},
        },
        use_tray_icon_event_handler, use_tray_menu_event_handler, use_window,
    },
    prelude::*,
};

use crate::{
    assets, i18n,
    state::{Shared, UiRequest},
    theme,
};

/// Whether a tray icon exists. Without one, closing the window must quit the app.
pub static TRAY_OK: AtomicBool = AtomicBool::new(false);

thread_local! {
    static POPUP_OPEN: Cell<bool> = const { Cell::new(false) };
}

/// Handle to the main window, handed to the popup so it can bring it back.
#[derive(Clone)]
pub struct MainWindow(pub DesktopContext);

pub fn use_systray(shared: Arc<Shared>) {
    let desktop = use_window();
    let lang = shared.settings().lang;

    use_hook(move || {
        let s = i18n::strings(lang);
        let menu = Menu::new();
        let items = [
            MenuItem::with_id("open", s.open_app, true, None),
            MenuItem::with_id("quick", s.quick_view, true, None),
            MenuItem::with_id("refresh", s.refresh_all, true, None),
            MenuItem::with_id("quit", s.quit, true, None),
        ];
        let sep = PredefinedMenuItem::separator();
        let _ = menu.append_items(&[&items[0], &items[1], &sep, &items[2], &sep, &items[3]]);
        let mut builder = TrayIconBuilder::new().with_menu(Box::new(menu)).with_menu_on_left_click(false).with_tooltip("Klue");
        if let Ok(icon) = Icon::from_rgba(assets::app_icon(32), 32, 32) {
            builder = builder.with_icon(icon);
        }
        match builder.build() {
            Ok(tray) => {
                TRAY_OK.store(true, Ordering::Relaxed);
                Some(Rc::new(tray))
            }
            Err(e) => {
                tracing::warn!("no tray icon: {e}");
                None
            }
        }
    });

    let (menu_shared, menu_desktop) = (shared.clone(), desktop.clone());
    use_tray_menu_event_handler(move |event| match event.id.0.as_str() {
        "open" => show(&menu_desktop),
        "quick" => open_popup(menu_shared.clone(), menu_desktop.clone(), None),
        "refresh" => {
            let shared = menu_shared.clone();
            spawn(async move { shared.refresh_all().await });
        }
        "quit" => std::process::exit(0),
        _ => {}
    });

    use_tray_icon_event_handler(move |event| match event {
        TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, rect, .. } => {
            let anchor = (rect.position.x, rect.position.y, rect.size.width as f64, rect.size.height as f64);
            open_popup(shared.clone(), desktop.clone(), Some(anchor));
        }
        TrayIconEvent::DoubleClick { button: MouseButton::Left, .. } => show(&desktop),
        _ => {}
    });
}

pub fn show(desktop: &DesktopContext) {
    desktop.set_visible(true);
    desktop.set_minimized(false);
    desktop.set_focus();
}

/// Open the quick-view popup next to the tray icon (`anchor` = x, y, width, height in
/// physical pixels), or near the bottom-right of the screen.
pub fn open_popup(shared: Arc<Shared>, main: DesktopContext, anchor: Option<(f64, f64, f64, f64)>) {
    if POPUP_OPEN.with(Cell::get) {
        return;
    }
    POPUP_OPEN.with(|p| p.set(true));

    let scale = main.scale_factor();
    let (w, h) = (360.0 * scale, 560.0 * scale);
    let monitor = main.current_monitor().or_else(|| main.primary_monitor());
    let (mx, my, mw, mh) = monitor
        .map(|m| (m.position().x as f64, m.position().y as f64, m.size().width as f64, m.size().height as f64))
        .unwrap_or((0.0, 0.0, 1920.0 * scale, 1080.0 * scale));
    let (x, y) = match anchor {
        Some((ax, ay, aw, ah)) => {
            let x = (ax + aw / 2.0 - w / 2.0).clamp(mx + 8.0, mx + mw - w - 8.0);
            let y = if ay > my + mh / 2.0 { ay - h - 8.0 } else { ay + ah + 8.0 };
            (x, y)
        }
        None => (mx + mw - w - 16.0 * scale, my + mh - h - 64.0 * scale),
    };

    let settings = shared.settings();
    let dom = VirtualDom::new(crate::ui::tray::TrayApp).with_root_context(shared).with_root_context(MainWindow(main.clone()));
    let window = WindowBuilder::new()
        .with_title("Klue")
        .with_decorations(false)
        .with_resizable(false)
        .with_always_on_top(true)
        .with_inner_size(LogicalSize::new(360.0, 560.0))
        .with_position(PhysicalPosition::new(x, y));
    let cfg = Config::new()
        .with_window(window)
        .with_custom_head(assets::head())
        .with_menu(None)
        .with_disable_context_menu(true)
        .with_background_color(theme::window_rgba(settings.theme))
        .with_close_behaviour(WindowCloseBehaviour::WindowCloses);
    main.new_window(dom, cfg);
}

pub fn popup_closed() {
    POPUP_OPEN.with(|p| p.set(false));
}

/// Ask the main window to come forward, optionally on an account or the settings.
pub fn bring_main(shared: &Shared, main: &MainWindow, request: UiRequest) {
    show(&main.0);
    shared.request_ui(request);
}
