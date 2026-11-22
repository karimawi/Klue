//! Demo mode (`klue --demo` or `KLUE_DEMO=1`): the numbers from the mockup, kept in
//! memory only, with simulated sign-ins. Useful for trying the UI without accounts.
//! Every name, number, amount and card here is made up.

use std::time::Duration;

use chrono::{DateTime, Local, TimeZone, Utc};
use klue_core::{
    Bundle, BundleKind, BundleState, Carrier, FetchError, LineType, Unit, Usage,
    model::{LineInfo, Payment, PaymentKind},
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
    balance: f64,
    /// The Flex bundle was renewed on credit: settle-by (month, day) and what's owed.
    borrowed: Option<((u32, u32), f64)>,
    plan: &'static str,
    since: (i32, u32, u32),
    /// (month, day, amount, method, balance after)
    payments: Vec<(u32, u32, f64, &'static str, Option<f64>)>,
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
            state: if l.borrowed.is_some() && *kind == BundleKind::Flex { BundleState::Borrowed } else { BundleState::Active },
            settle_by: l.borrowed.filter(|_| *kind == BundleKind::Flex).map(|((m, d), _)| at(m, d)),
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
        balance_egp: Some(l.balance),
        owed_egp: l.borrowed.map(|(_, owed)| owed),
        bundles,
        subscribed_at: Some(sub),
        renews_at: Some(renew),
        fetched_at,
        info: LineInfo {
            plan: Some(l.plan.into()),
            payment: Some(PaymentKind::Prepaid),
            status: Some("Active".into()),
            number: None,
            since: Local.with_ymd_and_hms(l.since.0, l.since.1, l.since.2, 0, 0, 0).single().map(|d| d.with_timezone(&Utc)),
        },
        payments: Some(
            l.payments
                .iter()
                .map(|(m, d, amount, method, after)| Payment {
                    at: at(*m, *d) + chrono::Duration::hours(14),
                    amount_egp: *amount,
                    method: Some((*method).into()),
                    reference: None,
                    balance_after: *after,
                })
                .collect(),
        ),
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
            balance: 214.35, borrowed: None, plan: "WE IOE VDSL Primary Prepaid", since: (2016, 3, 8),
            payments: vec![(11, 4, 410.0, "Cash", Some(624.35)), (10, 5, 410.0, "Voucher", Some(418.2)), (9, 5, 400.0, "Cash", Some(407.95))],
        },
        Line {
            carrier: Carrier::We, line: LineType::Landline, number: "0348712290", owner: mo, nick: None,
            pkg: "Super 25", sub: (11, 12), renew: (12, 12), remind: (12, LeadUnit::Hours),
            bundles: vec![B(K::Data, "Home internet", 25.0, 3.1, U::Gb, Some(5.0)), B(K::Minutes, "All-network minutes", 1000.0, 610.0, U::Minutes, Some(100.0))],
            balance: 12.4, borrowed: None, plan: "WE IOE ADSL Primary Prepaid", since: (2019, 3, 2),
            payments: vec![(11, 11, 165.0, "EVC", Some(177.4))],
        },
        Line {
            carrier: Carrier::Vodafone, line: LineType::Mobile, number: "01001285532", owner: mo, nick: Some("My phone"),
            pkg: "New Plus Bundle 98 LE", sub: (11, 8), renew: (12, 8), remind: (2, LeadUnit::Days),
            bundles: vec![
                B(K::Data, "New Plus Bundle 98 LE - 4750Mega", 4750.0, 1310.0, U::Mb, Some(500.0)),
                B(K::Addon, "MI Minutes Conversion Optin", 500.0, 240.0, U::Mb, None),
                B(K::Minutes, "All-network minutes", 1500.0, 1020.0, U::Minutes, Some(150.0)),
            ],
            balance: 23.75, borrowed: None, plan: "Worry Free 14 PT", since: (2021, 6, 1),
            payments: vec![(11, 9, 120.0, "Card •••• 4242", None), (10, 24, 135.0, "Card •••• 4242", None), (10, 3, 120.0, "Card •••• 1881", None), (9, 12, 100.0, "Topup via fawry", None)],
        },
        Line {
            carrier: Carrier::Vodafone, line: LineType::Mobile, number: "01099214470", owner: "Samia Ibrahim Fahmy", nick: Some("Mom"),
            pkg: "Flex 70", sub: (11, 1), renew: (12, 1), remind: (1, LeadUnit::Days),
            bundles: vec![B(K::Flex, "Flex 70", 2000.0, 1200.0, U::Flex, Some(200.0)), B(K::Data, "Mobile data", 12288.0, 6963.0, U::Mb, Some(1024.0))],
            balance: 0.0, borrowed: Some(((11, 26), 79.8)), plan: "Flex 70", since: (2016, 2, 14),
            payments: vec![(10, 2, 80.0, "Topup via fawry", None)],
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
    let (pkg, total, rem, kind, name, plan) = match carrier {
        Carrier::We => ("Super 200", 200.0, 171.3, BundleKind::Data, "Home internet", "WE IOE VDSL Primary Prepaid"),
        _ => ("Flex 100", 30.0, 21.7, BundleKind::Data, "Mobile data", "Flex 100"),
    };
    Ok(Usage {
        owner_name: Some("Youssef Adel Mansour".into()),
        package_name: Some(pkg.into()),
        balance_egp: Some(42.5),
        owed_egp: None,
        bundles: vec![Bundle {
            key: format!("demo-{number}"),
            kind,
            name: name.into(),
            unit: Unit::Gb,
            total,
            remaining: rem,
            starts_at: Some(sub),
            expires_at: Some(renew),
            state: BundleState::Active,
            settle_by: None,
        }],
        subscribed_at: Some(sub),
        renews_at: Some(renew),
        fetched_at: Utc::now(),
        info: LineInfo {
            plan: Some(plan.into()),
            payment: Some(PaymentKind::Prepaid),
            status: Some("Active".into()),
            number: Some(number.to_string()),
            since: Some(Utc::now() - chrono::Duration::days(400)),
        },
        payments: Some(Vec::new()),
    })
}

/// Simulated refresh: same numbers, new timestamp.
pub async fn refresh(account: &Account) -> Result<Usage, FetchError> {
    tokio::time::sleep(Duration::from_millis(1200)).await;
    let mut usage = account.usage.clone().ok_or(FetchError::Unsupported)?;
    usage.fetched_at = Utc::now();
    Ok(usage)
}
