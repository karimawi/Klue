//! Desktop notifications for quota and renewal alerts.

use chrono::Utc;
use klue_core::{alerts::AlertEvent, format, store::Account, store::Lang};

use crate::i18n::{self, carrier_name};

/// Notification title and body, like the preview card in the Alerts dialog.
pub fn describe(account: &Account, event: &AlertEvent, lang: Lang) -> (String, String) {
    let title = format!("{} · {}", carrier_name(account.carrier, lang), account.display_name());
    let package = account
        .usage
        .as_ref()
        .and_then(|u| u.package_name.clone())
        .unwrap_or_else(|| i18n::strings(lang).package.to_string());
    let ar = lang == Lang::Ar;
    let body = match event {
        AlertEvent::QuotaLow { bundle, .. } => {
            let is_main = account.usage.as_ref().and_then(|u| u.main()).is_some_and(|m| m.key == bundle.key);
            let on = if is_main { package } else { bundle.name.clone() };
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

/// macOS shows a notification under whichever app bundle it's sent as, and that's
/// Finder unless we name one. Name Klue's own bundle when running from Klue.app.
pub fn init() {
    #[cfg(target_os = "macos")]
    {
        let in_bundle = std::env::current_exe().is_ok_and(|p| p.to_string_lossy().contains(".app/Contents/MacOS/"));
        if in_bundle {
            let _ = notify_rust::set_application("io.github.karimawi.klue");
        }
    }
}

pub fn show(title: String, body: String) {
    std::thread::spawn(move || {
        if let Err(e) = notify_rust::Notification::new().appname("Klue").summary(&title).body(&body).show() {
            tracing::warn!("couldn't show a notification: {e}");
        }
    });
}

pub fn alert(account: &Account, event: &AlertEvent, lang: Lang) {
    let (title, body) = describe(account, event, lang);
    show(title, body);
}
