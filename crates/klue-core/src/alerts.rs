//! Which notifications are due. The app shows them and calls [`Account::mark_fired`] so
//! each fires once per cycle.

use chrono::{DateTime, Local, Utc};

use crate::{Bundle, store::Account};

#[derive(Debug, Clone, PartialEq)]
pub enum AlertEvent {
    /// A bundle dropped to its alert threshold.
    QuotaLow { account_id: String, bundle: Bundle, threshold: f64 },
    /// The package renews within the reminder window.
    RenewalSoon { account_id: String, renews_at: DateTime<Utc> },
    /// The package renews today.
    RenewalToday { account_id: String, renews_at: DateTime<Utc> },
}

impl AlertEvent {
    /// Unique within the billing cycle.
    pub fn key(&self) -> String {
        match self {
            AlertEvent::QuotaLow { bundle, .. } => format!(
                "quota:{}:{}",
                bundle.key,
                bundle.expires_at.map_or_else(|| "-".to_string(), |t| t.timestamp().to_string())
            ),
            AlertEvent::RenewalSoon { renews_at, .. } => format!("renew:{}", renews_at.timestamp()),
            AlertEvent::RenewalToday { renews_at, .. } => format!("renewday:{}", renews_at.timestamp()),
        }
    }

    pub fn account_id(&self) -> &str {
        match self {
            AlertEvent::QuotaLow { account_id, .. }
            | AlertEvent::RenewalSoon { account_id, .. }
            | AlertEvent::RenewalToday { account_id, .. } => account_id,
        }
    }
}

/// Alerts due for `account` at `now` that haven't fired yet.
pub fn due(account: &Account, now: DateTime<Utc>) -> Vec<AlertEvent> {
    let Some(usage) = &account.usage else { return Vec::new() };
    let mut events = Vec::new();

    let main_key = account.main_bundle().map(|b| b.key.clone());
    for bundle in &usage.bundles {
        let alert = account.alert_for(bundle, main_key.as_deref() == Some(bundle.key.as_str()));
        if alert.is_triggered(bundle) {
            events.push(AlertEvent::QuotaLow {
                account_id: account.id.clone(),
                bundle: bundle.clone(),
                threshold: alert.threshold(bundle),
            });
        }
    }

    if let Some(renews_at) = usage.renews_at.filter(|r| *r > now) {
        let reminder = account.alerts.renewal;
        if reminder.enabled && now >= renews_at - reminder.lead() {
            events.push(AlertEvent::RenewalSoon { account_id: account.id.clone(), renews_at });
        }
        if account.alerts.renewal_day
            && now.with_timezone(&Local).date_naive() == renews_at.with_timezone(&Local).date_naive()
        {
            events.push(AlertEvent::RenewalToday { account_id: account.id.clone(), renews_at });
        }
    }

    events.retain(|e| !account.fired.contains(&e.key()));
    events
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BundleKind, Carrier, LineType, Unit, Usage,
        store::{LeadUnit, RenewalReminder},
    };
    use chrono::Duration;

    fn account(remaining: f64, renews_in: Duration) -> Account {
        let now = Utc::now();
        let mut a = Account::new(Carrier::Vodafone, LineType::Mobile, "01001285532".into());
        a.apply_usage(Usage {
            owner_name: None,
            package_name: None,
            package_name_ar: None,
            balance_egp: None,
            owed_egp: None,
            bundles: vec![Bundle {
                key: "mobile-data".into(),
                kind: BundleKind::Data,
                name: "Mobile data".into(),
                name_ar: None,
                unit: Unit::Gb,
                total: 20.0,
                remaining,
                starts_at: None,
                expires_at: Some(now + renews_in),
                state: Default::default(),
                settle_by: None,
            }],
            subscribed_at: None,
            renews_at: Some(now + renews_in),
            fetched_at: now,
            info: Default::default(),
            payments: None,
        }, &Default::default());
        a
    }

    #[test]
    fn quota_alert_fires_once() {
        let mut a = account(1.5, Duration::days(10));
        let events = due(&a, Utc::now());
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], AlertEvent::QuotaLow { threshold, .. } if threshold == 2.0));
        a.mark_fired(events[0].key());
        assert!(due(&a, Utc::now()).is_empty());
    }

    #[test]
    fn renewal_reminders() {
        let mut a = account(15.0, Duration::hours(30));
        assert!(due(&a, Utc::now()).iter().any(|e| matches!(e, AlertEvent::RenewalSoon { .. })));
        a.alerts.renewal = RenewalReminder { enabled: true, value: 12, unit: LeadUnit::Hours };
        assert!(due(&a, Utc::now()).is_empty());
        a.alerts.renewal.enabled = false;
        a.alerts.renewal_day = true;
        let today = account(15.0, Duration::minutes(5));
        let mut today = today;
        today.alerts.renewal.enabled = false;
        today.alerts.renewal_day = true;
        let events = due(&today, Utc::now());
        let same_day = Utc::now().with_timezone(&Local).date_naive()
            == (Utc::now() + Duration::minutes(5)).with_timezone(&Local).date_naive();
        assert_eq!(events.iter().any(|e| matches!(e, AlertEvent::RenewalToday { .. })), same_day);
    }

    #[test]
    fn what_came_due_while_klue_was_off_goes_out_once_it_runs() {
        // Last run a week ago with the renewal 9 days out; the 2-day window opened while it
        // was closed, so it's due now.
        let mut a = account(10.0, Duration::days(1));
        let events = due(&a, Utc::now());
        assert!(events.iter().any(|e| matches!(e, AlertEvent::RenewalSoon { .. })));
        // Fired keys are saved, so it isn't sent again after a restart.
        for e in &events {
            a.mark_fired(e.key());
        }
        let saved: Account = serde_json::from_str(&serde_json::to_string(&a).unwrap()).unwrap();
        assert!(due(&saved, Utc::now()).is_empty());
        // A renewal that already passed while closed isn't announced late.
        let past = account(10.0, -Duration::hours(3));
        assert!(!due(&past, Utc::now()).iter().any(|e| matches!(e, AlertEvent::RenewalSoon { .. })));
    }

}
