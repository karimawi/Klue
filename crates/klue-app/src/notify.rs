//! Desktop notifications for quota and renewal alerts.

use std::path::Path;

use chrono::Utc;
use klue_core::{alerts::AlertEvent, format, store::Account, store::Lang};

use crate::{
    i18n::{self, carrier_name},
    sound::{self, Tone},
};

/// The macOS bundle id and the Windows AppUserModelID toasts are filed under.
#[cfg_attr(not(any(windows, target_os = "macos")), allow(dead_code))]
pub const APP_ID: &str = "io.github.karimawi.klue";

/// Title and body, matching the preview in the Alerts dialog.
pub fn describe(account: &Account, event: &AlertEvent, lang: Lang) -> (String, String) {
    let title = format!("{} · {}", carrier_name(account.carrier, lang), account.display_name());
    let package = account
        .usage
        .as_ref()
        .and_then(|u| u.package_in(lang))
        .map_or_else(|| i18n::strings(lang).package.to_string(), str::to_string);
    let ar = lang == Lang::Ar;
    let body = match event {
        AlertEvent::QuotaLow { bundle, .. } => {
            let is_main = account.main_bundle().is_some_and(|m| m.key == bundle.key);
            let on = if is_main { package } else { bundle.name_in(lang).to_string() };
            let left = format!("{} {}", format::amount(bundle.remaining, bundle.unit), i18n::unit_name(bundle.unit, lang));
            if ar { format!("متبقي {left} فقط من باقة {on}") } else { format!("Only {left} left on {on}") }
        }
        AlertEvent::RenewalSoon { renews_at, .. } => {
            let when = i18n::countdown(*renews_at, Utc::now(), lang);
            let date = i18n::date_long(*renews_at, lang);
            if ar { format!("تتجدد باقة {package} {when} ({date})") } else { format!("{package} renews {when} ({date})") }
        }
        AlertEvent::RenewalToday { .. } => {
            if ar { format!("تتجدد باقة {package} اليوم") } else { format!("{package} renews today") }
        }
    };
    (title, body)
}

/// Makes notifications show as Klue's. On Windows `data_dir` gets a copy of the icon.
#[allow(unused_variables)]
pub fn init(data_dir: &Path) {
    // macOS shows notifications under the bundle they name (Finder by default).
    #[cfg(target_os = "macos")]
    {
        let in_bundle = std::env::current_exe().is_ok_and(|p| p.to_string_lossy().contains(".app/Contents/MacOS/"));
        if in_bundle {
            let _ = notify_rust::set_application(APP_ID);
        }
    }
    // Windows shows a toast's sender as an app only if its AppUserModelID is registered (name
    // and icon under HKCU); otherwise toasts come from Windows PowerShell.
    #[cfg(windows)]
    if let Err(e) = windows::register(data_dir) {
        tracing::warn!("couldn't register Klue for notifications: {e}");
    }
}

#[cfg(windows)]
mod windows {
    use std::path::Path;

    use super::APP_ID;

    pub fn register(data_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let icon = data_dir.join("klue-notify.png");
        if std::fs::read(&icon).ok().as_deref() != Some(crate::assets::APP_ICON_PNG) {
            std::fs::write(&icon, crate::assets::APP_ICON_PNG)?;
        }
        let key = windows_registry::CURRENT_USER.create(format!(r"Software\Classes\AppUserModelId\{APP_ID}"))?;
        key.set_string("DisplayName", "Klue")?;
        key.set_string("IconUri", icon.to_string_lossy().as_ref())?;
        key.set_string("IconBackgroundColor", "FFFFBE32")?;
        // Group this process's windows and taskbar button under the same id.
        let wide: Vec<u16> = APP_ID.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID(wide.as_ptr());
        }
        Ok(())
    }
}

/// Shows a notification without sound; [`send`] plays Klue's own.
fn show(title: String, body: String) {
    std::thread::spawn(move || {
        let mut n = notify_rust::Notification::new();
        n.appname("Klue").summary(&title).body(&body);
        #[cfg(windows)]
        n.app_id(APP_ID);
        // No sound name: Windows and macOS stay silent; Linux servers are asked not to play one.
        #[cfg(all(unix, not(target_os = "macos")))]
        n.hint(notify_rust::Hint::SuppressSound(true));
        if let Err(e) = n.show() {
            tracing::warn!("couldn't show a notification: {e}");
        }
    });
}

/// A notification about Klue itself (an update).
pub fn notice(title: String, body: String) {
    show(title, body);
    sound::play(Tone::Notice);
}

/// Low quota is a warning; renewals are notices.
pub fn tone(event: &AlertEvent) -> Tone {
    match event {
        AlertEvent::QuotaLow { .. } => Tone::Warning,
        AlertEvent::RenewalSoon { .. } | AlertEvent::RenewalToday { .. } => Tone::Notice,
    }
}

/// Shows these alerts with one sound for all.
pub fn send(alerts: &[(Account, AlertEvent)], lang: Lang) {
    for (account, event) in alerts {
        let (title, body) = describe(account, event, lang);
        show(title, body);
    }
    if let Some(tone) = sound::for_all(alerts.iter().map(|(_, e)| tone(e))) {
        sound::play(tone);
    }
}
