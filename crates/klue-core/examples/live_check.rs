//! Signs in to the accounts in an env file and prints what Klue would show.
//!
//! ```text
//! cargo run --release -p klue-core --example live_check -- accounts.env [--only vf2]
//! ```
//!
//! Keys: `TENUMBER`/`TEPASSWORD` (WE) and `VFNUMBER`/`VFPWD` (Vodafone), with suffixes 2, 3...
//! for more accounts (`we1`, `we2`, `vf1`... for `--only`). Passwords aren't printed.
//! `KLUE_DEBUG_DIR` also saves the raw responses. WE counts failed sign-ins, so don't loop it.

use std::{collections::BTreeMap, path::PathBuf, time::Instant};

use klue_core::{
    BundleState, Carrier, LineType, Usage,
    captcha::default_solver,
    carriers::{self, FetchContext},
    format,
};

struct Account {
    carrier: Carrier,
    line: LineType,
    label: String,
    number: String,
    password: String,
}

fn parse_env(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (key, value) = line.split_once('=')?;
            let value = value.trim();
            let unquoted = [('"', '"'), ('\'', '\'')].iter().find_map(|(a, b)| value.strip_prefix(*a)?.strip_suffix(*b));
            Some((key.trim().trim_start_matches("export ").trim().to_string(), unquoted.unwrap_or(value).to_string()))
        })
        .collect()
}

fn accounts(vars: &BTreeMap<String, String>) -> Vec<Account> {
    let kinds = [
        (Carrier::We, LineType::Landline, "TENUMBER", "TEPASSWORD", "we"),
        (Carrier::Vodafone, LineType::Mobile, "VFNUMBER", "VFPWD", "vf"),
    ];
    let mut out = Vec::new();
    for (carrier, line, number_key, password_key, tag) in kinds {
        for i in 1..10 {
            let suffix = if i == 1 { String::new() } else { i.to_string() };
            let (Some(number), Some(password)) = (vars.get(&format!("{number_key}{suffix}")), vars.get(&format!("{password_key}{suffix}"))) else {
                continue;
            };
            out.push(Account { carrier, line, label: format!("{tag}{i}"), number: number.clone(), password: password.clone() });
        }
    }
    out
}

/// "022•••835": enough to tell the accounts apart.
fn short(number: &str) -> String {
    let digits: Vec<char> = number.chars().filter(char::is_ascii_digit).collect();
    if digits.len() < 7 {
        return "•••".into();
    }
    let head: String = digits[..3].iter().collect();
    let tail: String = digits[digits.len() - 3..].iter().collect();
    format!("{head}•••{tail}")
}

fn date(d: Option<chrono::DateTime<chrono::Utc>>) -> String {
    d.map(|d| d.format("%Y-%m-%d %H:%M").to_string()).unwrap_or_else(|| "—".into())
}

fn report(u: &Usage) {
    let or_dash = |s: &Option<String>| s.clone().unwrap_or_else(|| "—".into());
    println!("  owner:      {}", or_dash(&u.owner_name));
    println!("  package:    {}   ar: {}", or_dash(&u.package_name), or_dash(&u.package_name_ar));
    println!("  subscribed: {}   renews: {}", date(u.subscribed_at), date(u.renews_at));
    println!("  balance:    {}", u.balance_egp.map(|b| format!("{} EGP", format::money(b))).unwrap_or_else(|| "—".into()));
    if let Some(owed) = u.owed_egp {
        println!("  owed:       {} EGP", format::money(owed));
    }
    let info = &u.info;
    println!(
        "  line:       plan {} (ar: {}) · {} · status {} · number {} · since {}",
        or_dash(&info.plan),
        or_dash(&info.plan_ar),
        info.payment.map(|p| format!("{p:?}")).unwrap_or_else(|| "—".into()),
        or_dash(&info.status),
        info.number.as_deref().map(short).unwrap_or_else(|| "—".into()),
        date(info.since),
    );
    println!("  buckets ({}), main first:", u.bundles.len());
    for (i, b) in u.ordered(None).into_iter().enumerate() {
        println!(
            "    {} {:<44} {:>10} / {:<10} {:?} {:?}  key={}  ends {}{}\n        ar: {}",
            if i == 0 { "*" } else { " " },
            b.name,
            format::amount(b.remaining, b.unit),
            format::amount_short(b.total, b.unit),
            b.unit,
            b.kind,
            b.key,
            date(b.expires_at),
            match b.state {
                BundleState::Active => String::new(),
                state => format!("  {state:?}, settle by {}", date(b.settle_by)),
            },
            or_dash(&b.name_ar),
        );
    }
    match &u.payments {
        None => println!("  payments:   unavailable"),
        Some(list) if list.is_empty() => println!("  payments:   none"),
        Some(list) => {
            println!("  payments ({}):", list.len());
            for p in list.iter().take(6) {
                println!(
                    "    {}  {:>10} EGP  {:<24} after {}",
                    p.at.format("%Y-%m-%d %H:%M"),
                    format::money(p.amount_egp),
                    p.method.clone().unwrap_or_else(|| "—".into()),
                    p.balance_after.map(format::money).unwrap_or_else(|| "—".into()),
                );
            }
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "klue_core=info".into()))
        .with_writer(std::io::stderr)
        .init();

    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: live_check <accounts.env> [--only <label>]...");
        std::process::exit(2);
    };
    let mut only = Vec::new();
    while let Some(a) = args.next() {
        if a == "--only" {
            only.extend(args.next());
        }
    }
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        eprintln!("can't read {path}: {e}");
        std::process::exit(2);
    });
    let list: Vec<Account> = accounts(&parse_env(&text)).into_iter().filter(|a| only.is_empty() || only.contains(&a.label)).collect();
    if list.is_empty() {
        eprintln!("no accounts found in {path}");
        std::process::exit(2);
    }

    let solver = default_solver();
    let debug_root = std::env::var_os("KLUE_DEBUG_DIR").map(PathBuf::from);
    let mut failures = 0;
    for account in &list {
        println!("\n== {} ({:?} {}) ==", account.label, account.carrier, short(&account.number));
        let ctx = FetchContext {
            solver: solver.as_deref(),
            ask: None,
            debug_dir: debug_root.as_ref().map(|d| d.join(&account.label)),
        };
        let started = Instant::now();
        match carriers::fetch(account.carrier, account.line, &account.number, &account.password, &ctx).await {
            Ok(usage) => {
                println!("  ok in {:.1}s", started.elapsed().as_secs_f32());
                report(&usage);
            }
            Err(e) => {
                failures += 1;
                println!("  FAILED after {:.1}s: {e} ({e:?})", started.elapsed().as_secs_f32());
            }
        }
    }
    println!("\n{} of {} accounts read", list.len() - failures, list.len());
    if failures > 0 {
        std::process::exit(1);
    }
}
