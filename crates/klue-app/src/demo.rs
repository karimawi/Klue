//! Demo mode (`klue --demo` or `KLUE_DEMO=1`): made-up numbers in memory and simulated
//! sign-ins. All names, numbers and amounts are invented.

use std::time::Duration;

use chrono::{DateTime, Local, TimeZone, Timelike, Utc};
use klue_core::{
    Bundle, BundleKind, BundleState, Carrier, FetchError, LineType, Unit, Usage,
    model::{LineInfo, Payment, PaymentKind, Speed},
    store::{Account, AppData, BundleAlert, LeadUnit, RenewalReminder, ThresholdMode},
};

/// Demo dates relative to now, as if today were 21 Nov 2026 10:00.
fn at(m: u32, d: u32) -> DateTime<Utc> {
    let mock_now = Local.with_ymd_and_hms(2026, 11, 21, 10, 0, 0).unwrap();
    let date = Local.with_ymd_and_hms(2026, m, d, 0, 0, 0).unwrap();
    Utc::now() + (date - mock_now)
}

/// kind, name, total, remaining, unit, alert threshold.
struct B(BundleKind, &'static str, f64, f64, Unit, Option<f64>);

/// How much fake history a demo number has.
enum Kept {
    /// The whole cycle so far.
    Cycle,
    /// Added this many hours ago.
    Hours(i64),
    /// Just the latest refresh.
    Nothing,
}

struct Line {
    carrier: Carrier,
    line: LineType,
    number: &'static str,
    owner: &'static str,
    nick: Option<&'static str>,
    pkg: &'static str,
    /// Arabic package name, as the carrier gives it. The package quota uses it too.
    pkg_ar: Option<&'static str>,
    sub: (u32, u32),
    renew: (u32, u32),
    remind: (u32, LeadUnit),
    bundles: Vec<B>,
    balance: f64,
    /// Borrowed Flex bundle: settle-by (month, day) and the amount owed.
    borrowed: Option<((u32, u32), f64)>,
    plan: &'static str,
    plan_ar: Option<&'static str>,
    since: (i32, u32, u32),
    /// (month, day, amount, method, balance after)
    payments: Vec<(u32, u32, f64, &'static str, Option<f64>)>,
    kept: Kept,
}

fn build(l: Line) -> Account {
    let (sub, renew) = (at(l.sub.0, l.sub.1), at(l.renew.0, l.renew.1));
    let mut a = Account::new(l.carrier, l.line, l.number.into());
    a.nickname = l.nick.map(String::from);
    let bundles: Vec<Bundle> = l
        .bundles
        .iter()
        .enumerate()
        .map(|(i, B(kind, name, total, rem, unit, _))| Bundle {
            key: name.to_lowercase().replace(' ', "-"),
            kind: *kind,
            name: (*name).into(),
            name_ar: bundle_ar(i, *kind, name, l.pkg_ar),
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
    let seed = l.number.bytes().map(u64::from).sum();
    match l.kept {
        Kept::Cycle => history(&mut a, &bundles, sub, sub, fetched_at, true, seed),
        Kept::Hours(h) => history(&mut a, &bundles, sub, fetched_at - chrono::Duration::hours(h), fetched_at, false, seed),
        Kept::Nothing => {}
    }
    a.apply_usage(Usage {
        owner_name: Some(l.owner.into()),
        package_name: Some(l.pkg.into()),
        package_name_ar: l.pkg_ar.map(String::from),
        balance_egp: Some(l.balance),
        owed_egp: l.borrowed.map(|(_, owed)| owed),
        bundles,
        subscribed_at: Some(sub),
        renews_at: Some(renew),
        fetched_at,
        info: LineInfo {
            plan: Some(l.plan.into()),
            plan_ar: l.plan_ar.map(String::from),
            // WE's "Super" packages: 30 Mbit/s down, 3 up, 256 kbit/s after the quota.
            speed: (l.carrier == Carrier::We).then_some(Speed { down_kbps: 30720, up_kbps: Some(3072), throttled_kbps: Some(256) }),
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
    }, &Default::default());
    a.last_attempt = Some(fetched_at);
    a
}

/// Fake refreshes between `from` and `to`, every 20-50 minutes while the computer is on.
/// If it `sleeps`, there are none at night (01:00 to after 08:00) or on one day it's off.
/// Usage is light at night and heaviest in the evening.
#[allow(clippy::too_many_arguments)]
fn history(a: &mut Account, bundles: &[Bundle], cycle: DateTime<Utc>, from: DateTime<Utc>, to: DateTime<Utc>, sleeps: bool, seed: u64) {
    use chrono::Duration as D;
    let mut state = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407) | 1;
    let mut random = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state % 10_000) as f64 / 10_000.0
    };
    // Hourly usage weight: by time of day, times a random factor per day.
    let hours = ((to - cycle).num_minutes() as f64 / 60.0).ceil().max(1.0) as usize;
    let moods: Vec<f64> = (0..hours / 24 + 2).map(|_| 0.55 + random()).collect();
    let weight = |i: usize| {
        let h = (cycle + D::hours(i as i64)).with_timezone(&Local).hour();
        let by_hour = match h {
            1..=7 => 0.12,
            8..=11 => 0.5,
            12..=16 => 0.75,
            17..=22 => 1.45,
            _ => 1.0,
        };
        by_hour * moods[i / 24]
    };
    let mut cumulative = vec![0.0; hours + 1];
    for i in 0..hours {
        cumulative[i + 1] = cumulative[i] + weight(i);
    }
    let share = |t: DateTime<Utc>| {
        let x = ((t - cycle).num_seconds() as f64 / 3600.0).clamp(0.0, hours as f64);
        let i = (x.floor() as usize).min(hours - 1);
        (cumulative[i] + (cumulative[i + 1] - cumulative[i]) * (x - i as f64)) / cumulative[hours]
    };
    let off_day = (to - from).num_days() / 2;
    let mut refreshes: Vec<(DateTime<Utc>, Vec<Bundle>)> = Vec::new();
    let mut t = from + D::minutes(25);
    while t < to - D::minutes(15) {
        let local = t.with_timezone(&Local);
        let day = (t - from).num_days();
        if sleeps && ((1..8).contains(&local.hour()) || (off_day >= 3 && day == off_day)) {
            // Asleep or off: resume after 08:00 the next morning that isn't off.
            let mut wake = local.date_naive().and_hms_opt(8, 0, 0).and_then(|n| Local.from_local_datetime(&n).earliest()).map_or(t, |w| w.with_timezone(&Utc));
            while wake <= t || (off_day >= 3 && (wake - from).num_days() == off_day) {
                wake += D::days(1);
            }
            t = wake + D::minutes((random() * 80.0) as i64);
            continue;
        }
        let now: Vec<Bundle> = bundles
            .iter()
            .map(|b| {
                let used = b.used() * share(t) / share(to);
                let left = b.total - used;
                let left = if b.unit == Unit::Gb { (left * 100.0).round() / 100.0 } else { left.round() };
                Bundle { remaining: left, ..b.clone() }
            })
            .collect();
        refreshes.push((t, now));
        t += D::minutes(20 + (random() * 30.0) as i64);
    }
    a.history.record_all(refreshes.iter().map(|(t, b)| (*t, b.as_slice())));
}

/// Arabic names like the carriers give them: the package quota has the package's name,
/// Vodafone's unnamed data is "موبايل انترنت", named bundles have none.
fn bundle_ar(index: usize, kind: BundleKind, name: &str, pkg_ar: Option<&str>) -> Option<String> {
    match (index, kind) {
        (0, BundleKind::Data | BundleKind::Flex) => pkg_ar.map(String::from),
        _ if name == "Mobile data" => Some("موبايل انترنت".into()),
        _ => None,
    }
}

pub fn data() -> AppData {
    use BundleKind as K;
    use Unit as U;
    let mo = "Mostafa Hassan Ali";
    let lines = vec![
        Line {
            carrier: Carrier::We, line: LineType::Landline, number: "0223584417", owner: mo, nick: Some("Home internet"),
            pkg: "Super 140", pkg_ar: Some("سرعة سوبر 140"), sub: (11, 5), renew: (12, 5), remind: (2, LeadUnit::Days),
            bundles: vec![B(K::Data, "Home internet", 140.0, 62.4, U::Gb, Some(10.0)), B(K::Addon, "Extra 25 GB", 25.0, 18.5, U::Gb, None)],
            balance: 214.35, borrowed: None, plan: "WE IOE VDSL Primary Prepaid", plan_ar: None, since: (2016, 3, 8),
            payments: vec![(11, 4, 410.0, "Cash", Some(624.35)), (10, 5, 410.0, "Voucher", Some(418.2)), (9, 5, 400.0, "Cash", Some(407.95))],
            kept: Kept::Cycle,
        },
        Line {
            carrier: Carrier::We, line: LineType::Landline, number: "0348712290", owner: mo, nick: None,
            pkg: "Super 25", pkg_ar: Some("سرعة سوبر 25"), sub: (11, 12), renew: (12, 12), remind: (12, LeadUnit::Hours),
            bundles: vec![B(K::Data, "Home internet", 25.0, 3.1, U::Gb, Some(5.0)), B(K::Minutes, "All-network minutes", 1000.0, 610.0, U::Minutes, Some(100.0))],
            balance: 12.4, borrowed: None, plan: "WE IOE ADSL Primary Prepaid", plan_ar: None, since: (2019, 3, 2),
            payments: vec![(11, 11, 165.0, "EVC", Some(177.4))],
            kept: Kept::Hours(7),
        },
        Line {
            carrier: Carrier::Vodafone, line: LineType::Mobile, number: "01001285532", owner: mo, nick: Some("My phone"),
            pkg: "New Plus Bundle 98 LE", pkg_ar: None, sub: (11, 8), renew: (12, 8), remind: (2, LeadUnit::Days),
            bundles: vec![
                B(K::Data, "New Plus Bundle 98 LE - 4750Mega", 4750.0, 1310.0, U::Mb, Some(500.0)),
                B(K::Addon, "MI Minutes Conversion Optin", 500.0, 240.0, U::Mb, None),
                B(K::Minutes, "All-network minutes", 1500.0, 1020.0, U::Minutes, Some(150.0)),
            ],
            balance: 23.75, borrowed: None, plan: "Worry Free 14 PT", plan_ar: Some("ريح بالك ١٤ قرش"), since: (2021, 6, 1),
            payments: vec![(11, 9, 120.0, "Card •••• 4242", None), (10, 24, 135.0, "Card •••• 4242", None), (10, 3, 120.0, "Card •••• 1881", None), (9, 12, 100.0, "Topup via fawry", None)],
            kept: Kept::Cycle,
        },
        Line {
            carrier: Carrier::Vodafone, line: LineType::Mobile, number: "01099214470", owner: "Samia Ibrahim Fahmy", nick: Some("Mom"),
            pkg: "Flex 70", pkg_ar: Some("فليكس 70"), sub: (11, 1), renew: (12, 1), remind: (1, LeadUnit::Days),
            bundles: vec![B(K::Flex, "Flex 70", 2000.0, 1200.0, U::Flex, Some(200.0)), B(K::Data, "Mobile data", 12288.0, 6963.0, U::Mb, Some(1024.0))],
            balance: 0.0, borrowed: Some(((11, 26), 79.8)), plan: "Flex 70", plan_ar: Some("فليكس 70"), since: (2016, 2, 14),
            payments: vec![(10, 2, 80.0, "Topup via fawry", None)],
            kept: Kept::Nothing,
        },
    ];
    AppData { accounts: lines.into_iter().map(build).collect(), ..AppData::default() }
}

/// Fake sign-in. A password containing "wrong" fails.
pub async fn verify(carrier: Carrier, number: &str, password: &str) -> Result<Usage, FetchError> {
    tokio::time::sleep(Duration::from_millis(1700)).await;
    if password.to_lowercase().contains("wrong") {
        return Err(FetchError::BadCredentials);
    }
    let (sub, renew) = (Utc::now() - chrono::Duration::days(6), Utc::now() + chrono::Duration::days(24));
    let (pkg, pkg_ar, total, rem, kind, name, plan, plan_ar) = match carrier {
        Carrier::We => ("Super 200", "سرعة سوبر 200", 200.0, 171.3, BundleKind::Data, "Home internet", "WE IOE VDSL Primary Prepaid", None),
        _ => ("Flex 100", "فليكس 100", 30.0, 21.7, BundleKind::Data, "Mobile data", "Flex 100", Some("فليكس 100")),
    };
    Ok(Usage {
        owner_name: Some("Youssef Adel Mansour".into()),
        package_name: Some(pkg.into()),
        package_name_ar: Some(pkg_ar.into()),
        balance_egp: Some(42.5),
        owed_egp: None,
        bundles: vec![Bundle {
            key: format!("demo-{number}"),
            kind,
            name: name.into(),
            name_ar: bundle_ar(0, kind, name, Some(pkg_ar)),
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
            plan_ar: plan_ar.map(String::from),
            speed: (carrier == Carrier::We).then_some(Speed { down_kbps: 30720, up_kbps: Some(3072), throttled_kbps: Some(256) }),
            payment: Some(PaymentKind::Prepaid),
            status: Some("Active".into()),
            number: Some(number.to_string()),
            since: Some(Utc::now() - chrono::Duration::days(400)),
        },
        payments: Some(Vec::new()),
    })
}

/// Fake refresh: same numbers, new time.
pub async fn refresh(account: &Account) -> Result<Usage, FetchError> {
    tokio::time::sleep(Duration::from_millis(1200)).await;
    let mut usage = account.usage.clone().ok_or(FetchError::Unsupported)?;
    usage.fetched_at = Utc::now();
    Ok(usage)
}

/// A fake update for "Check for updates" in demo mode.
pub fn update() -> klue_core::update::Release {
    klue_core::update::Release {
        version: klue_core::update::Version { major: 9, minor: 9, patch: 0, pre: None },
        tag: "v9.9.0".into(),
        title: "Klue v9.9.0".into(),
        notes: "## New\n- Usage graphs for every bundle\n- The package speed on WE lines\n## Fixed\n- Smoother scrolling".into(),
        page: crate::about::REPO.to_string(),
        published: Some(Utc::now()),
        prerelease: false,
        assets: Vec::new(),
        target: "main".into(),
    }
}
