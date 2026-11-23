use chrono::{DateTime, Months, Utc};
use serde::{Deserialize, Serialize};

use crate::store::Lang;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Carrier {
    We,
    Vodafone,
    Orange,
    Etisalat,
}

impl Carrier {
    pub const ALL: [Carrier; 4] = [Carrier::We, Carrier::Vodafone, Carrier::Orange, Carrier::Etisalat];

    /// Short id, also used to pick the theme.
    pub fn id(self) -> &'static str {
        match self {
            Carrier::We => "we",
            Carrier::Vodafone => "vf",
            Carrier::Orange => "or",
            Carrier::Etisalat => "ea",
        }
    }

    /// Whether Klue can sign in to this carrier yet.
    pub fn is_supported(self) -> bool {
        matches!(self, Carrier::We | Carrier::Vodafone)
    }

    /// Line types with a working client: WE home internet and Vodafone mobile.
    pub fn supports_line(self, line: LineType) -> bool {
        matches!((self, line), (Carrier::We, LineType::Landline) | (Carrier::Vodafone, LineType::Mobile))
    }

    pub fn default_line(self) -> LineType {
        match self {
            Carrier::We => LineType::Landline,
            _ => LineType::Mobile,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LineType {
    Mobile,
    Landline,
}

/// The unit the carrier uses for a bucket. Amounts stay in it, so "992 MB" doesn't turn
/// into "1.0 GB".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Unit {
    Gb,
    Mb,
    Minutes,
    Sms,
    Flex,
    Units,
    Egp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BundleKind {
    Data,
    Addon,
    Minutes,
    Social,
    Flex,
    Sms,
    Other,
}

/// Vodafone can renew a Flex bundle on credit ("borrowed"). It works until a settle-by
/// date, then it's frozen until paid. The next recharge pays it first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BundleState {
    #[default]
    Active,
    /// Renewed on credit, usable until [`Bundle::settle_by`].
    Borrowed,
    /// Not paid in time; unusable until it is.
    Frozen,
}

/// One quota bucket: internet, an add-on, minutes, flexes...
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bundle {
    /// Stable id, so alert settings survive refreshes.
    pub key: String,
    pub kind: BundleKind,
    pub name: String,
    /// Arabic name, if the carrier has one.
    #[serde(default)]
    pub name_ar: Option<String>,
    pub unit: Unit,
    pub total: f64,
    pub remaining: f64,
    #[serde(default)]
    pub starts_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub state: BundleState,
    /// For a borrowed bundle: when it freezes unless paid.
    #[serde(default)]
    pub settle_by: Option<DateTime<Utc>>,
}

impl Bundle {
    /// The Arabic name in Arabic, when there is one.
    pub fn name_in(&self, lang: Lang) -> &str {
        self.name_ar.as_deref().filter(|_| lang == Lang::Ar).unwrap_or(&self.name)
    }

    pub fn used(&self) -> f64 {
        (self.total - self.remaining).max(0.0)
    }

    /// Remaining share, 0-100.
    pub fn remaining_pct(&self) -> f64 {
        if self.total <= 0.0 {
            return 0.0;
        }
        (self.remaining / self.total * 100.0).clamp(0.0, 100.0)
    }

    /// For ranking bundles: data in MB, everything else in its own unit.
    pub fn size(&self) -> f64 {
        match self.unit {
            Unit::Gb => self.total * 1024.0,
            _ => self.total,
        }
    }
}

/// The largest bundle with something left first, then the rest by size.
pub(crate) fn order_bundles(bundles: &mut [Bundle]) {
    bundles.sort_by(|a, b| b.size().total_cmp(&a.size()));
    if let Some(pos) = bundles.iter().position(|b| b.remaining > 0.0) {
        bundles[..=pos].rotate_right(1);
    }
}

/// Prepaid (topped up) or postpaid (billed monthly).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PaymentKind {
    Prepaid,
    Postpaid,
}

/// DSL speeds, in kbit/s.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Speed {
    pub down_kbps: u32,
    #[serde(default)]
    pub up_kbps: Option<u32>,
    /// Download speed after the quota runs out.
    #[serde(default)]
    pub throttled_kbps: Option<u32>,
}

/// What the carrier says about the line itself.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LineInfo {
    /// Rate plan, e.g. "Worry Free 14 PT" or "WE IOE VDSL Primary Prepaid".
    #[serde(default)]
    pub plan: Option<String>,
    /// Arabic plan name, if any.
    #[serde(default)]
    pub plan_ar: Option<String>,
    /// Package speed (WE home internet).
    #[serde(default)]
    pub speed: Option<Speed>,
    #[serde(default)]
    pub payment: Option<PaymentKind>,
    /// Line status in the carrier's words, e.g. "Active".
    #[serde(default)]
    pub status: Option<String>,
    /// The number on file with the carrier.
    #[serde(default)]
    pub number: Option<String>,
    #[serde(default)]
    pub since: Option<DateTime<Utc>>,
}

impl LineInfo {
    pub fn plan_in(&self, lang: Lang) -> Option<&str> {
        self.plan_ar.as_deref().filter(|_| lang == Lang::Ar).or(self.plan.as_deref())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Payment {
    pub at: DateTime<Utc>,
    pub amount_egp: f64,
    /// e.g. "Card •••• 0485" or "Cash".
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub reference: Option<String>,
    /// Balance after the payment, if the carrier says.
    #[serde(default)]
    pub balance_after: Option<f64>,
}

/// One line's account, as read from the carrier.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    pub owner_name: Option<String>,
    pub package_name: Option<String>,
    /// Arabic package name, if any.
    #[serde(default)]
    pub package_name_ar: Option<String>,
    #[serde(default)]
    pub balance_egp: Option<f64>,
    /// Money owed, e.g. for a borrowed bundle. The next recharge pays it first.
    #[serde(default)]
    pub owed_egp: Option<f64>,
    /// In display order: the first is the main quota.
    pub bundles: Vec<Bundle>,
    pub subscribed_at: Option<DateTime<Utc>>,
    pub renews_at: Option<DateTime<Utc>>,
    pub fetched_at: DateTime<Utc>,
    #[serde(default)]
    pub info: LineInfo,
    /// Newest first. `None` if the history couldn't be read.
    #[serde(default)]
    pub payments: Option<Vec<Payment>>,
}

impl Usage {
    pub fn package_in(&self, lang: Lang) -> Option<&str> {
        self.package_name_ar.as_deref().filter(|_| lang == Lang::Ar).or(self.package_name.as_deref())
    }

    /// Default main quota: the largest bundle with something left.
    pub fn main(&self) -> Option<&Bundle> {
        self.bundles.first()
    }

    pub fn extras(&self) -> &[Bundle] {
        self.bundles.get(1..).unwrap_or(&[])
    }

    /// The main quota, preferring the user's pick while it exists.
    pub fn main_for(&self, preferred: Option<&str>) -> Option<&Bundle> {
        preferred
            .and_then(|key| self.bundles.iter().find(|b| b.key == key))
            .or_else(|| self.main())
    }

    /// All bundles, main quota first.
    pub fn ordered(&self, preferred: Option<&str>) -> Vec<&Bundle> {
        let main = self.main_for(preferred);
        main.into_iter()
            .chain(self.bundles.iter().filter(|b| main.is_none_or(|m| !std::ptr::eq(*b, m))))
            .collect()
    }

    /// Start of the billing cycle, or a month before renewal if the carrier doesn't say.
    pub fn cycle_start(&self) -> Option<DateTime<Utc>> {
        self.subscribed_at
            .or_else(|| self.renews_at.and_then(|r| r.checked_sub_months(Months::new(1))))
    }
}

/// Lowercase ASCII alphanumerics, for keys.
pub(crate) fn slug(s: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// Gives each bundle a unique key from its name (or kind), so alerts survive the carrier
/// reordering bundles.
pub(crate) fn assign_keys(bundles: &mut [Bundle]) {
    let mut seen = std::collections::HashMap::<String, usize>::new();
    for b in bundles.iter_mut() {
        let name = slug(&b.name);
        let base = if name.is_empty() { format!("{:?}", b.kind).to_lowercase() } else { name };
        let n = seen.entry(base.clone()).or_insert(0);
        *n += 1;
        b.key = if *n == 1 { base } else { format!("{base}#{n}") };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_is_stable() {
        assert_eq!(slug("Super 140 GB!"), "super-140-gb");
        assert_eq!(slug("  إنترنت "), "");
    }

    fn bundle(name: &str, unit: Unit, total: f64, remaining: f64) -> Bundle {
        Bundle { key: slug(name), kind: BundleKind::Other, name: name.into(), name_ar: None, unit, total, remaining, starts_at: None, expires_at: None, state: BundleState::Active, settle_by: None }
    }

    #[test]
    fn largest_bundle_with_something_left_comes_first() {
        let mut v = vec![
            bundle("minutes", Unit::Minutes, 500.0, 120.0),
            bundle("used up", Unit::Gb, 140.0, 0.0),
            bundle("data", Unit::Mb, 4748.0, 992.0),
            bundle("social", Unit::Gb, 1.0, 0.5),
        ];
        order_bundles(&mut v);
        // 140 GB is the largest but empty, so 4748 MB leads; the rest stay largest first.
        let names: Vec<_> = v.iter().map(|b| b.name.as_str()).collect();
        assert_eq!(names, ["data", "used up", "social", "minutes"]);

        let mut all_empty = vec![bundle("a", Unit::Gb, 1.0, 0.0), bundle("b", Unit::Gb, 2.0, 0.0)];
        order_bundles(&mut all_empty);
        assert_eq!(all_empty[0].name, "b");
    }

    #[test]
    fn user_can_pick_the_main_bundle() {
        let usage = Usage {
            owner_name: None,
            package_name: None,
            package_name_ar: None,
            balance_egp: None,
            owed_egp: None,
            bundles: vec![bundle("data", Unit::Mb, 4748.0, 992.0), bundle("minutes", Unit::Minutes, 500.0, 120.0)],
            subscribed_at: None,
            renews_at: None,
            fetched_at: Utc::now(),
            info: LineInfo::default(),
            payments: None,
        };
        assert_eq!(usage.main_for(None).unwrap().name, "data");
        assert_eq!(usage.main_for(Some("minutes")).unwrap().name, "minutes");
        assert_eq!(usage.main_for(Some("gone")).unwrap().name, "data");
        let order: Vec<_> = usage.ordered(Some("minutes")).iter().map(|b| b.name.as_str()).collect();
        assert_eq!(order, ["minutes", "data"]);
    }

    #[test]
    fn keys_are_unique() {
        let b = |name: &str| Bundle {
            key: String::new(),
            kind: BundleKind::Addon,
            name: name.into(),
            name_ar: None,
            unit: Unit::Gb,
            total: 1.0,
            remaining: 1.0,
            starts_at: None,
            expires_at: None,
            state: BundleState::Active,
            settle_by: None,
        };
        let mut v = vec![b("Extra 25 GB"), b("Extra 25 GB"), b("سوشيال")];
        assign_keys(&mut v);
        assert_eq!(v[0].key, "extra-25-gb");
        assert_eq!(v[1].key, "extra-25-gb#2");
        assert_eq!(v[2].key, "addon");
    }
}
