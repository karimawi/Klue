//! Demo mode (`klue --demo` or `KLUE_DEMO=1`): the numbers from the mockup, kept in
//! memory only, with simulated sign-ins. Useful for trying the UI without accounts.

use std::time::Duration;

use chrono::{DateTime, Local, TimeZone, Utc};
use klue_core::{
    Bundle, BundleKind, Carrier, FetchError, LineType, Unit, Usage,
    store::{Account, AppData, BundleAlert, LeadUnit, RenewalReminder, ThresholdMode},
};

/// The mockup is drawn at 2026-11-21 10:00; demo dates keep the same distance from now.
fn at(m: u32, d: u32) -> DateTime<Utc> {
    let mock_now = Local.with_ymd_and_hms(2026, 11, 21, 10, 0, 0).unwrap();
    let date = Local.with_ymd_and_hms(2026, m, d, 0, 0, 0).unwrap();
    Utc::now() + (date - mock_now)
}

/// kind, name, total, remaining, unit, alert threshold.
struct B(BundleKind, &'static str, f64, f64, Unit, Option<f64>);

struct Line {
    carrier: Carrier,
    line: LineType,
    number: &'static str,
    owner: &'static str,
    nick: Option<&'static str>,
    pkg: &'static str,
    sub: (u32, u32),
    renew: (u32, u32),
    remind: (u32, LeadUnit),
    bundles: Vec<B>,
}

fn build(l: Line) -> Account {
    let (sub, renew) = (at(l.sub.0, l.sub.1), at(l.renew.0, l.renew.1));
    let mut a = Account::new(l.carrier, l.line, l.number.into());
    a.nickname = l.nick.map(String::from);
    let bundles: Vec<Bundle> = l
        .bundles
        .iter()
        .map(|B(kind, name, total, rem, unit, _)| Bundle {
            key: name.to_lowercase().replace(' ', "-"),
            kind: *kind,
            name: (*name).into(),
            unit: *unit,
            total: *total,
            remaining: *rem,
            starts_at: Some(sub),
            expires_at: Some(renew),
        })
        .collect();
    for (bundle, B(.., alert)) in bundles.iter().zip(&l.bundles) {
        let pref = match alert {
            Some(v) => BundleAlert { enabled: true, value: *v, mode: ThresholdMode::Amount },
            None => BundleAlert { enabled: false, ..BundleAlert::default_for(bundle, false) },
        };
        a.alerts.bundles.insert(bundle.key.clone(), pref);
    }
    a.alerts.renewal = RenewalReminder { enabled: true, value: l.remind.0, unit: l.remind.1 };
    let fetched_at = Utc::now() - chrono::Duration::minutes(2);
    a.apply_usage(Usage {
        owner_name: Some(l.owner.into()),
        package_name: Some(l.pkg.into()),
        balance_egp: None,
        bundles,
        subscribed_at: Some(sub),
        renews_at: Some(renew),
        fetched_at,
        info: Default::default(),
        payments: None,
    });
    a.last_attempt = Some(fetched_at);
    a
}

pub fn data() -> AppData {
    use BundleKind as K;
    use Unit as U;
    let mo = "Mostafa Hassan Ali";
    let lines = vec![
        Line {
            carrier: Carrier::We, line: LineType::Landline, number: "0223584417", owner: mo, nick: Some("Home internet"),
            pkg: "Super 140", sub: (11, 5), renew: (12, 5), remind: (2, LeadUnit::Days),
            bundles: vec![B(K::Data, "Home internet", 140.0, 62.4, U::Gb, Some(10.0)), B(K::Addon, "Extra 25 GB", 25.0, 18.5, U::Gb, None)],
        },
        Line {
            carrier: Carrier::We, line: LineType::Landline, number: "0348712290", owner: mo, nick: None,
            pkg: "Super 25", sub: (11, 12), renew: (12, 12), remind: (12, LeadUnit::Hours),
            bundles: vec![B(K::Data, "Home internet", 25.0, 3.1, U::Gb, Some(5.0)), B(K::Minutes, "All-network minutes", 1000.0, 610.0, U::Minutes, Some(100.0))],
        },
        Line {
            carrier: Carrier::Vodafone, line: LineType::Mobile, number: "01001285532", owner: mo, nick: Some("My phone"),
            pkg: "Red Essential", sub: (11, 8), renew: (12, 8), remind: (2, LeadUnit::Days),
            bundles: vec![
                B(K::Data, "Mobile data", 20.0, 11.2, U::Gb, Some(2.0)),
                B(K::Social, "Social 5 GB", 5.0, 4.1, U::Gb, None),
                B(K::Minutes, "All-network minutes", 1500.0, 1020.0, U::Minutes, Some(150.0)),
            ],
        },
        Line {
            carrier: Carrier::Vodafone, line: LineType::Mobile, number: "01099214470", owner: "Samia Ibrahim Fahmy", nick: Some("Mom"),
            pkg: "Flex 70", sub: (11, 1), renew: (12, 1), remind: (1, LeadUnit::Days),
            bundles: vec![B(K::Data, "Mobile data", 12.0, 6.8, U::Gb, Some(1.0)), B(K::Minutes, "All-network minutes", 500.0, 388.0, U::Minutes, None)],
        },
    ];
    AppData { accounts: lines.into_iter().map(build).collect(), ..AppData::default() }
}

/// Simulated sign-in, like the mockup's Verify button. A password containing "wrong" fails.
pub async fn verify(carrier: Carrier, number: &str, password: &str) -> Result<Usage, FetchError> {
    tokio::time::sleep(Duration::from_millis(1700)).await;
    if password.to_lowercase().contains("wrong") {
        return Err(FetchError::BadCredentials);
    }
    let (sub, renew) = (Utc::now() - chrono::Duration::days(6), Utc::now() + chrono::Duration::days(24));
    let (pkg, total, rem, kind, name) = match carrier {
        Carrier::We => ("Super 200", 200.0, 171.3, BundleKind::Data, "Home internet"),
        _ => ("Flex 100", 30.0, 21.7, BundleKind::Data, "Mobile data"),
    };
    Ok(Usage {
        owner_name: Some("Karim Adel Mansour".into()),
        package_name: Some(pkg.into()),
        balance_egp: None,
        bundles: vec![Bundle {
            key: format!("demo-{number}"),
            kind,
            name: name.into(),
            unit: Unit::Gb,
            total,
            remaining: rem,
            starts_at: Some(sub),
            expires_at: Some(renew),
        }],
        subscribed_at: Some(sub),
        renews_at: Some(renew),
        fetched_at: Utc::now(),
        info: Default::default(),
        payments: None,
    })
}

/// Simulated refresh: same numbers, new timestamp.
pub async fn refresh(account: &Account) -> Result<Usage, FetchError> {
    tokio::time::sleep(Duration::from_millis(1200)).await;
    let mut usage = account.usage.clone().ok_or(FetchError::Unsupported)?;
    usage.fetched_at = Utc::now();
    Ok(usage)
}
