use chrono::{DateTime, Months, Utc};
use serde::{Deserialize, Serialize};

/// The four Egyptian carriers Klue knows about.
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

    /// Short stable id, also used for theme lookup.
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

    /// Line types with a working client. WE is home internet (landline) only and
    /// Vodafone is mobile only.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Unit {
    Gb,
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

/// One quota bucket: the main internet quota, an add-on, minutes, flexes...
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bundle {
    /// Stable identifier used to remember alert settings across refreshes.
    pub key: String,
    pub kind: BundleKind,
    /// Name as reported by the carrier.
    pub name: String,
    pub unit: Unit,
    pub total: f64,
    pub remaining: f64,
    #[serde(default)]
    pub starts_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
}

impl Bundle {
    pub fn used(&self) -> f64 {
        (self.total - self.remaining).max(0.0)
    }

    /// Remaining share of the bundle, 0–100.
    pub fn remaining_pct(&self) -> f64 {
        if self.total <= 0.0 {
            return 0.0;
        }
        (self.remaining / self.total * 100.0).clamp(0.0, 100.0)
    }
}

/// A snapshot of one line's account, as read from the carrier.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    pub owner_name: Option<String>,
    pub package_name: Option<String>,
    #[serde(default)]
    pub balance_egp: Option<f64>,
    /// The first bundle is the main quota shown in the dashboard hero.
    pub bundles: Vec<Bundle>,
    pub subscribed_at: Option<DateTime<Utc>>,
    pub renews_at: Option<DateTime<Utc>>,
    pub fetched_at: DateTime<Utc>,
}

impl Usage {
    pub fn main(&self) -> Option<&Bundle> {
        self.bundles.first()
    }

    pub fn extras(&self) -> &[Bundle] {
        self.bundles.get(1..).unwrap_or(&[])
    }

    /// Start of the current billing cycle. Falls back to one month before renewal
    /// when the carrier doesn't report it.
    pub fn cycle_start(&self) -> Option<DateTime<Utc>> {
        self.subscribed_at
            .or_else(|| self.renews_at.and_then(|r| r.checked_sub_months(Months::new(1))))
    }
}

/// Turn a carrier-provided name into a key fragment: lowercase ASCII alphanumerics.
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

/// Give every bundle a unique key based on its name (falling back to its kind), so
/// alert settings survive refreshes even when the carrier reorders bundles.
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

    #[test]
    fn keys_are_unique() {
        let b = |name: &str| Bundle {
            key: String::new(),
            kind: BundleKind::Addon,
            name: name.into(),
            unit: Unit::Gb,
            total: 1.0,
            remaining: 1.0,
            starts_at: None,
            expires_at: None,
        };
        let mut v = vec![b("Extra 25 GB"), b("Extra 25 GB"), b("سوشيال")];
        assign_keys(&mut v);
        assert_eq!(v[0].key, "extra-25-gb");
        assert_eq!(v[1].key, "extra-25-gb#2");
        assert_eq!(v[2].key, "addon");
    }
}
