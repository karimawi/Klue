//! "Launch at startup": registers Klue with the OS (Run key on Windows, LaunchAgent on
//! macOS, XDG autostart on Linux) to start minimised in the tray.

pub fn apply(enabled: bool) {
    let Some(exe) = std::env::current_exe().ok().and_then(|p| p.to_str().map(String::from)) else { return };
    let launcher = auto_launch::AutoLaunchBuilder::new()
        .set_app_name("Klue")
        .set_app_path(&exe)
        .set_args(&["--minimized"])
        .build();
    let result = launcher.and_then(|l| if enabled { l.enable() } else { l.disable() });
    if let Err(e) = result {
        tracing::warn!("couldn't {} launch at startup: {e}", if enabled { "enable" } else { "disable" });
    }
}
