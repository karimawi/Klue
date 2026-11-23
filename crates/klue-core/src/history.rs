//! Usage history: what each bundle had left at each refresh, and what can be read from
//! it. Refreshes come at uneven times (the computer sleeps, Klue is closed, a refresh
//! fails or is started by hand), so everything here works from whatever samples there
//! are: usage between two samples is spread evenly over the time between them.

use std::collections::BTreeMap;

use chrono::{DateTime, Duration, Local, NaiveDate, TimeZone, Timelike, Utc};
use serde::{Deserialize, Serialize};

use crate::Bundle;

/// What a bundle had left at one refresh. Saved as `[unix seconds, remaining, total]`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(from = "(i64, f64, f64)", into = "(i64, f64, f64)")]
pub struct Sample {
    pub at: DateTime<Utc>,
    pub remaining: f64,
    pub total: f64,
}

impl From<(i64, f64, f64)> for Sample {
    fn from((at, remaining, total): (i64, f64, f64)) -> Self {
        Self { at: Utc.timestamp_opt(at, 0).single().unwrap_or_default(), remaining, total }
    }
}

impl From<Sample> for (i64, f64, f64) {
    fn from(s: Sample) -> Self {
        (s.at.timestamp(), s.remaining, s.total)
    }
}

/// Every bundle's samples, by [`Bundle::key`], oldest first.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct History {
    pub bundles: BTreeMap<String, Vec<Sample>>,
}

/// Samples older than this are dropped.
const KEEP: i64 = 62;

impl History {
    /// Record a refresh's bundles at `at`, in time order whenever it came.
    pub fn record(&mut self, at: DateTime<Utc>, bundles: &[Bundle]) {
        self.insert(at, bundles);
        self.thin(at);
    }

    /// Record many refreshes at once, in any order: quicker than one at a time.
    pub fn record_all<'a>(&mut self, refreshes: impl IntoIterator<Item = (DateTime<Utc>, &'a [Bundle])>) {
        let mut latest = None;
        for (at, bundles) in refreshes {
            self.insert(at, bundles);
            latest = latest.max(Some(at));
        }
        if let Some(now) = latest {
            self.thin(now);
        }
    }

    fn insert(&mut self, at: DateTime<Utc>, bundles: &[Bundle]) {
        for b in bundles {
            let samples = self.bundles.entry(b.key.clone()).or_default();
            let sample = Sample { at, remaining: b.remaining, total: b.total };
            let i = samples.partition_point(|s| s.at <= at);
            // The same reading twice within a minute adds nothing.
            if i > 0 && (at - samples[i - 1].at).num_seconds().abs() < 60 && samples[i - 1].remaining == sample.remaining {
                continue;
            }
            samples.insert(i, sample);
        }
    }

    pub fn samples(&self, key: &str) -> &[Sample] {
        self.bundles.get(key).map_or(&[], Vec::as_slice)
    }

    /// Keep the file small: every sample from the last 2 days, one an hour for the two
    /// weeks before, one every 6 hours before that, nothing older than [`KEEP`] days. The
    /// samples on either side of each local midnight stay, so every day's usage stays
    /// exact. Bundles with no samples left are forgotten.
    fn thin(&mut self, now: DateTime<Utc>) {
        for samples in self.bundles.values_mut() {
            // Only samples older than 2 days are thinned: their local dates (and the next
            // one's) tell where the midnights are.
            let old = samples.partition_point(|s| now - s.at > Duration::days(2));
            let days: Vec<NaiveDate> = samples[..(old + 1).min(samples.len())].iter().map(|s| s.at.with_timezone(&Local).date_naive()).collect();
            let edge = |i: usize| (i > 0 && days[i - 1] != days[i]) || days.get(i + 1).is_some_and(|d| *d != days[i]);
            let mut kept: Vec<(usize, Sample)> = Vec::with_capacity(samples.len());
            for (i, s) in samples.iter().enumerate() {
                let age = now - s.at;
                let slot = if age > Duration::days(KEEP) {
                    continue;
                } else if age > Duration::days(14) {
                    Some(6 * 3600)
                } else if age > Duration::days(2) {
                    Some(3600)
                } else {
                    None
                };
                // Within a slot keep the latest sample: it replaces the one before it.
                if let (Some(slot), Some(&(j, last))) = (slot, kept.last())
                    && now - last.at > Duration::days(2)
                    && last.at.timestamp().div_euclid(slot) == s.at.timestamp().div_euclid(slot)
                    && !edge(j)
                {
                    kept.pop();
                }
                kept.push((i, *s));
            }
            *samples = kept.into_iter().map(|(_, s)| s).collect();
        }
        self.bundles.retain(|_, s| !s.is_empty());
    }
}

/// A sample read for the chart: when, what was left, and how much the stretch had used.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub at: DateTime<Utc>,
    pub remaining: f64,
    /// Used since the stretch began.
    pub used: f64,
}

/// Usage over a day or an hour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Period {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub used: f64,
    /// The share of the period the samples cover (the first and last are partial).
    pub covered: f64,
    /// Much of the period's usage came in a long wait between two refreshes on either side
    /// of one of its ends, spread evenly over that wait: a guess, not a measure. The
    /// wait is the longest such one.
    pub gap: Option<Duration>,
}

impl Period {
    pub fn rough(&self) -> bool {
        self.gap.is_some()
    }
}

/// Averages from what the bundle has used, for the figures beside the chart (and in its
/// place when there are too few samples).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Estimate {
    pub used: f64,
    /// Days the usage is spread over (at least an hour's worth).
    pub days: f64,
    pub per_day: f64,
    pub per_hour: f64,
    /// When the rest runs out at this rate, if that's before the bundle renews or expires.
    pub runs_out: Option<DateTime<Utc>>,
    /// Read from the samples (the cycle's start isn't known), not from the whole cycle.
    pub from_samples: bool,
}

/// What a bundle's history says.
#[derive(Debug, Clone, PartialEq)]
pub struct Analysis {
    /// The samples since the bundle last renewed or was topped up, oldest first.
    pub points: Vec<Point>,
    /// Usage per local day of that stretch, once it spans a day.
    pub daily: Vec<Period>,
    /// Usage per hour over the last two days, when enough of those hours were measured.
    pub hourly: Vec<Period>,
    /// What today has used so far, when today has samples.
    pub today: Option<Period>,
    pub estimate: Option<Estimate>,
}

impl Analysis {
    /// Whether there's anything to draw.
    pub fn enough(&self) -> bool {
        !self.daily.is_empty() || !self.hourly.is_empty()
    }
}

/// A bundle's history read against what it has now. `since` and `until` are the bundle's
/// cycle, as far as the carrier says.
pub fn analyse(samples: &[Sample], bundle: &Bundle, since: Option<DateTime<Utc>>, until: Option<DateTime<Utc>>, now: DateTime<Utc>) -> Analysis {
    let samples = without_glitches(samples);
    let stretch = current_stretch(&samples);
    let base = stretch.first().map_or(0.0, |s| s.remaining);
    let points: Vec<Point> = stretch.iter().map(|s| Point { at: s.at, remaining: s.remaining, used: (base - s.remaining).max(0.0) }).collect();
    let span = points.first().zip(points.last()).map_or(Duration::zero(), |(f, l)| l.at - f.at);
    let mut daily = boundaries(&points, Boundary::Day);
    let today = daily.last().copied().filter(|p| p.start == floor(now, Boundary::Day));
    // Days are worth a chart once there's a day's worth of samples across two of them.
    if daily.len() < 2 || span < Duration::hours(24) {
        daily.clear();
    }
    let window = (now - Duration::hours(48)).max(points.first().map_or(now, |p| p.at));
    let mut hourly = boundaries_from(&points, Boundary::Hour, window);
    // Hours are worth showing once a few of them are measured, not guessed.
    if hourly.iter().filter(|p| !p.rough() && p.covered > 0.5).count() < 4 {
        hourly.clear();
    }
    Analysis { estimate: estimate(bundle, &points, since, until, now), points, daily, hourly, today }
}

/// The tolerance for comparing what's left: carriers round, and some report a little
/// more after a little less.
fn tolerance(total: f64) -> f64 {
    (total * 0.002).max(1e-6)
}

/// Readings that dip and come straight back (a carrier hiccup: a zero while its system
/// was busy, say) aren't usage. They're left out before anything is read.
fn without_glitches(samples: &[Sample]) -> Vec<Sample> {
    let mut out: Vec<Sample> = Vec::with_capacity(samples.len());
    for (i, s) in samples.iter().enumerate() {
        if let (Some(before), Some(after)) = (out.last(), samples.get(i + 1)) {
            let slack = tolerance(before.total);
            let dipped = s.remaining < before.remaining - slack.max(before.total * 0.05);
            let back = (after.remaining - before.remaining).abs() <= slack && after.total == before.total;
            if dipped && back {
                continue;
            }
        }
        out.push(*s);
    }
    out
}

/// The samples since the last renewal or top-up: a jump up in what's left (beyond
/// rounding) or a new total starts a new stretch.
fn current_stretch(samples: &[Sample]) -> &[Sample] {
    let start = samples
        .windows(2)
        .rposition(|w| {
            let slack = tolerance(w[0].total);
            w[1].remaining > w[0].remaining + slack || (w[1].total - w[0].total).abs() > slack
        })
        .map_or(0, |i| i + 1);
    &samples[start..]
}

#[derive(Clone, Copy)]
enum Boundary {
    Day,
    Hour,
}

/// The local day or hour starting at or before `t`.
fn floor(t: DateTime<Utc>, unit: Boundary) -> DateTime<Utc> {
    let local = t.with_timezone(&Local);
    let Some(floored) = (match unit {
        Boundary::Day => local.date_naive().and_hms_opt(0, 0, 0),
        Boundary::Hour => local.date_naive().and_hms_opt(local.hour(), 0, 0),
    }) else {
        return t;
    };
    // Where the clocks skip that time (summer time can start at midnight), the period
    // starts when they resume.
    let resolved = Local.from_local_datetime(&floored).earliest().or_else(|| Local.from_local_datetime(&(floored + Duration::hours(1))).earliest());
    resolved.map(|l| l.with_timezone(&Utc)).filter(|f| *f <= t).unwrap_or(t)
}

/// The start of the next local day or hour. A local day can be 23 or 25 hours long, and
/// where the clocks go back an hour repeats; the result is always later than `t`.
fn next(t: DateTime<Utc>, unit: Boundary) -> DateTime<Utc> {
    let (ahead, least) = match unit {
        Boundary::Day => (Duration::hours(26), Duration::hours(22)),
        Boundary::Hour => (Duration::minutes(61), Duration::minutes(30)),
    };
    let found = floor(t + ahead, unit);
    if found - t >= least { found } else { t + least.max(Duration::hours(1)) }
}

/// What the stretch had used by `t`, between the samples around it.
fn used_at(points: &[Point], t: DateTime<Utc>) -> f64 {
    let i = points.partition_point(|p| p.at <= t);
    match (i.checked_sub(1).map(|j| points[j]), points.get(i)) {
        (Some(a), Some(b)) => {
            let span = (b.at - a.at).num_milliseconds() as f64;
            if span <= 0.0 { b.used } else { a.used + (b.used - a.used) * ((t - a.at).num_milliseconds() as f64 / span) }
        }
        (Some(a), None) => a.used,
        (None, Some(b)) => b.used,
        (None, None) => 0.0,
    }
}

/// Usage per day or hour across the samples.
fn boundaries(points: &[Point], unit: Boundary) -> Vec<Period> {
    boundaries_from(points, unit, points.first().map_or_else(Utc::now, |p| p.at))
}

/// Usage per day or hour across the samples, from the period holding `from` on.
fn boundaries_from(points: &[Point], unit: Boundary, from: DateTime<Utc>) -> Vec<Period> {
    let (Some(first), Some(last)) = (points.first(), points.last()) else { return Vec::new() };
    // How long a wait between refreshes across a period's end can be before the split of
    // its usage between the two sides is a guess. Hourly refreshes (the default) measure
    // hours.
    let slack = match unit {
        Boundary::Day => Duration::hours(2),
        Boundary::Hour => Duration::minutes(90),
    };
    let mut out = Vec::new();
    let mut start = floor(from.max(first.at), unit);
    while start < last.at {
        let end = next(start, unit);
        let (lo, hi) = (start.max(first.at), end.min(last.at));
        if hi > lo {
            let used = (used_at(points, hi) - used_at(points, lo)).max(0.0);
            let covered = (hi - lo).num_seconds() as f64 / (end - start).num_seconds().max(1) as f64;
            // The usage in long waits across either end was split by time, not measured.
            let (mut unsure, mut gap) = (0.0, None::<Duration>);
            for edge in [start, end] {
                let i = points.partition_point(|p| p.at <= edge);
                if i == 0 || i == points.len() {
                    continue;
                }
                let (a, b) = (points[i - 1], points[i]);
                if a.at < edge && b.at - a.at > slack {
                    unsure += b.used - a.used;
                    gap = gap.max(Some(b.at - a.at));
                }
            }
            let gap = gap.filter(|_| unsure > 0.0 && unsure > used * 0.25);
            out.push(Period { start, end, used, covered, gap });
        }
        start = end;
    }
    out
}

/// Averages from what the bundle has used since its cycle started or, when that isn't
/// known, over the samples.
fn estimate(bundle: &Bundle, points: &[Point], since: Option<DateTime<Utc>>, until: Option<DateTime<Utc>>, now: DateTime<Utc>) -> Option<Estimate> {
    let (used, minutes, from_samples) = match since.filter(|s| *s < now) {
        Some(since) => (bundle.used(), (now - since).num_minutes(), false),
        None => {
            let (first, last) = (points.first()?, points.last()?);
            let minutes = (last.at - first.at).num_minutes();
            if minutes < 60 {
                return None;
            }
            (last.used - first.used, minutes, true)
        }
    };
    let days = (minutes as f64 / 1440.0).max(1.0 / 24.0);
    let per_day = used / days;
    let ends = until.or(bundle.expires_at);
    let runs_out = (per_day > 0.0)
        .then(|| now + Duration::minutes((bundle.remaining / per_day * 1440.0).min(1e7) as i64))
        .filter(|t| ends.is_none_or(|e| *t < e));
    Some(Estimate { used, days, per_day, per_hour: per_day / 24.0, runs_out, from_samples })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BundleKind, Unit};

    fn bundle(total: f64, remaining: f64) -> Bundle {
        Bundle {
            key: "data".into(),
            kind: BundleKind::Data,
            name: "Data".into(),
            name_ar: None,
            unit: Unit::Gb,
            total,
            remaining,
            starts_at: None,
            expires_at: None,
            state: Default::default(),
            settle_by: None,
        }
    }

    /// `h` hours after a local midnight (in November: no clock changes nearby in the zones
    /// the tests are run in).
    fn at(h: i64) -> DateTime<Utc> {
        floor(Utc.with_ymd_and_hms(2026, 11, 20, 12, 0, 0).unwrap(), Boundary::Day) + Duration::hours(h)
    }

    #[test]
    fn records_in_time_order_whatever_order_refreshes_come_in() {
        let mut h = History::default();
        h.record(at(5), &[bundle(100.0, 90.0)]);
        h.record(at(1), &[bundle(100.0, 99.0)]);
        h.record(at(3), &[bundle(100.0, 95.0)]);
        // The same reading again a few seconds later adds nothing.
        h.record(at(3) + Duration::seconds(20), &[bundle(100.0, 95.0)]);
        let s: Vec<(DateTime<Utc>, f64)> = h.samples("data").iter().map(|s| (s.at, s.remaining)).collect();
        assert_eq!(s, [(at(1), 99.0), (at(3), 95.0), (at(5), 90.0)]);
        // Saved compactly, read back the same.
        let json = serde_json::to_string(&h).unwrap();
        assert!(json.contains(&format!("[{},99.0,100.0]", at(1).timestamp())), "{json}");
        assert_eq!(serde_json::from_str::<History>(&json).unwrap(), h);
    }

    #[test]
    fn thins_old_samples_and_forgets_very_old_ones() {
        let mut h = History::default();
        let end = at(24 * 70);
        let (mut t, mut now) = (at(0), at(0));
        let mut left = 100.0;
        while t < end {
            h.record(t, &[bundle(100.0, left)]);
            now = t;
            left -= 0.001;
            t += Duration::minutes(15);
        }
        let s = h.samples("data");
        assert!(s.iter().all(|x| now - x.at <= Duration::days(KEEP)));
        // Everything from the last 2 days, hourly to 14 days, 6-hourly after, and the
        // samples around each midnight.
        let recent = s.iter().filter(|x| now - x.at <= Duration::days(2)).count();
        assert!(recent >= 190, "{recent}");
        assert!(s.len() < recent + 12 * 24 + 48 * 4 + 2 * 62 + 10, "{}", s.len());
        let old_midnight = at(24 * 20);
        assert!(s.iter().any(|x| x.at == old_midnight) && s.iter().any(|x| x.at == old_midnight - Duration::minutes(15)));
    }

    #[test]
    fn recording_at_once_keeps_what_recording_one_by_one_keeps() {
        let refreshes: Vec<(DateTime<Utc>, Vec<Bundle>)> = (0..24 * 20).map(|i| (at(0) + Duration::minutes(25 * i), vec![bundle(100.0, 100.0 - i as f64 * 0.01)])).collect();
        let mut one = History::default();
        for (t, b) in &refreshes {
            one.record(*t, b);
        }
        let mut all = History::default();
        all.record_all(refreshes.iter().rev().map(|(t, b)| (*t, b.as_slice())));
        // Thinning as it goes can only have dropped what the last pass would drop too.
        let (a, b) = (one.samples("data"), all.samples("data"));
        assert!(a.iter().all(|s| b.contains(s)) && b.len() - a.len() < 30, "{} vs {}", a.len(), b.len());
    }

    #[test]
    fn days_stay_exact_after_their_samples_are_thinned() {
        // Refreshes every 15 minutes for 20 days, 0.01 GB used between each.
        let mut h = History::default();
        let mut left = 100.0;
        for i in 0..20 * 96 {
            h.record(at(0) + Duration::minutes(15 * i), &[bundle(100.0, left)]);
            left -= 0.01;
        }
        let now = at(20 * 24) - Duration::minutes(15);
        let a = analyse(h.samples("data"), &bundle(100.0, left + 0.01), None, None, now);
        assert!(a.daily.len() >= 19);
        // The first and last days are partial; every whole day in between is exact.
        for day in &a.daily[1..a.daily.len() - 1] {
            assert!(!day.rough() && (day.used - 0.96).abs() < 0.02, "{day:?}");
        }
    }

    #[test]
    fn reads_daily_and_hourly_usage_from_uneven_samples() {
        // Hourly samples for two days, using 1 GB an hour, with a 5-hour gap (asleep).
        let mut samples = Vec::new();
        let mut left = 100.0;
        for h in 0..48 {
            if (20..25).contains(&h) {
                left -= 1.0;
                continue;
            }
            samples.push(Sample { at: at(h), remaining: left, total: 100.0 });
            left -= 1.0;
        }
        let now = at(47);
        let a = analyse(&samples, &bundle(100.0, left + 1.0), Some(at(0)), None, now);
        assert!(a.enough());
        assert_eq!(a.points.first().unwrap().used, 0.0);
        let total: f64 = a.daily.iter().map(|p| p.used).sum();
        assert!((total - 47.0).abs() < 1e-6, "{total}");
        // Hours across the gap are spread evenly, and marked as rough.
        let gap: Vec<&Period> = a.hourly.iter().filter(|p| p.start >= at(20) && p.start < at(24)).collect();
        assert!(!gap.is_empty() && gap.iter().all(|p| p.rough() && (p.used - 1.0).abs() < 1e-6));
        assert_eq!(gap[0].gap, Some(Duration::hours(6)));
        assert!(a.hourly.iter().filter(|p| !p.rough()).count() > 10);
        // Today so far: the samples after midnight of the last day.
        assert_eq!(a.today.map(|p| p.start), Some(at(24)));
    }

    #[test]
    fn a_renewal_starts_the_chart_over() {
        let samples = [
            Sample { at: at(0), remaining: 20.0, total: 100.0 },
            Sample { at: at(2), remaining: 10.0, total: 100.0 },
            // Renewed: back to 100.
            Sample { at: at(4), remaining: 100.0, total: 100.0 },
            Sample { at: at(6), remaining: 97.0, total: 100.0 },
        ];
        let a = analyse(&samples, &bundle(100.0, 97.0), None, None, at(6));
        assert_eq!(a.points.iter().map(|p| p.used).collect::<Vec<_>>(), [0.0, 3.0]);
    }

    #[test]
    fn too_few_samples_still_give_an_estimate() {
        let now = at(24 * 4);
        let mut b = bundle(140.0, 100.0);
        b.expires_at = Some(now + Duration::days(26));
        let a = analyse(&[Sample { at: now, remaining: 100.0, total: 140.0 }], &b, Some(at(0)), None, now);
        assert!(!a.enough() && a.daily.is_empty());
        let e = a.estimate.unwrap();
        // 40 GB in 4 days: 10 a day; 100 left lasts 10 days, before the renewal.
        assert!((e.per_day - 10.0).abs() < 1e-9 && (e.per_hour - 10.0 / 24.0).abs() < 1e-9);
        assert_eq!(e.runs_out.map(|t| (t - now).num_days()), Some(10));
        assert!(!e.from_samples);
        // Renewing first: no running out.
        let a = analyse(&[], &b, Some(at(0)), Some(now + Duration::days(5)), now);
        assert_eq!(a.estimate.unwrap().runs_out, None);
    }

    #[test]
    fn without_a_cycle_start_the_estimate_comes_from_the_samples() {
        let samples = [Sample { at: at(0), remaining: 50.0, total: 100.0 }, Sample { at: at(12), remaining: 47.0, total: 100.0 }];
        let e = analyse(&samples, &bundle(100.0, 47.0), None, None, at(12)).estimate.unwrap();
        assert!((e.per_day - 6.0).abs() < 1e-9 && e.from_samples, "{e:?}");
        // One sample says nothing about a rate.
        assert_eq!(analyse(&samples[..1], &bundle(100.0, 50.0), None, None, at(12)).estimate, None);
    }

    #[test]
    fn a_reading_that_dips_and_comes_back_is_ignored() {
        let samples = [
            Sample { at: at(0), remaining: 80.0, total: 100.0 },
            Sample { at: at(1), remaining: 79.0, total: 100.0 },
            // The carrier said 0 once.
            Sample { at: at(2), remaining: 0.0, total: 100.0 },
            Sample { at: at(3), remaining: 78.9, total: 100.0 },
            Sample { at: at(4), remaining: 78.0, total: 100.0 },
        ];
        let a = analyse(&samples, &bundle(100.0, 78.0), None, None, at(4));
        assert_eq!(a.points.iter().map(|p| p.remaining).collect::<Vec<_>>(), [80.0, 79.0, 78.9, 78.0], "not a renewal either");
        // A real drop that stays isn't a glitch.
        let mut real = samples;
        real[2].remaining = 2.0;
        real[3].remaining = 1.0;
        real[4].remaining = 0.5;
        assert_eq!(analyse(&real, &bundle(100.0, 0.5), None, None, at(4)).points.len(), 5);
    }

    #[test]
    fn a_night_without_refreshes_only_makes_a_day_a_guess_when_it_used_a_lot() {
        // Refreshes every half hour from 07:00 to 23:00 for three days; the computer
        // sleeps at night.
        let build = |night_use: f64| {
            let mut samples = Vec::new();
            let mut left = 100.0;
            for day in 0..3 {
                for half in 14..=46 {
                    samples.push(Sample { at: at(day * 24) + Duration::minutes(half * 30), remaining: left, total: 100.0 });
                    left -= 0.05;
                }
                left -= night_use;
            }
            samples
        };
        let quiet = analyse(&build(0.1), &bundle(100.0, 90.0), None, None, at(71));
        assert!(quiet.daily.len() >= 3 && quiet.daily.iter().all(|p| !p.rough()), "{:?}", quiet.daily);
        let busy = analyse(&build(8.0), &bundle(100.0, 70.0), None, None, at(71));
        let middle = busy.daily.iter().find(|p| p.start == at(24)).unwrap();
        assert!(middle.rough() && middle.gap == Some(Duration::hours(8)), "{middle:?}");
    }

    #[test]
    fn hourly_refreshes_measure_hours_and_six_hourly_ones_do_not() {
        let build = |every: i64| -> Vec<Sample> {
            (0..48 * 60 / every).map(|i| Sample { at: at(0) + Duration::minutes(17 + i * every), remaining: 100.0 - (i * every) as f64 * 0.01, total: 100.0 }).collect()
        };
        let hourly = analyse(&build(60), &bundle(100.0, 72.0), None, None, at(48));
        assert!(hourly.hourly.iter().filter(|p| !p.rough()).count() > 40, "{:?}", hourly.hourly);
        let sparse = analyse(&build(360), &bundle(100.0, 72.0), None, None, at(48));
        assert!(sparse.hourly.is_empty() && !sparse.daily.is_empty());
    }

    #[test]
    fn periods_always_move_forward() {
        let mut t = at(0);
        for _ in 0..24 * 400 {
            let (h, d) = (next(t, Boundary::Hour), next(t, Boundary::Day));
            assert!(h > t && d > t && h - t <= Duration::hours(2) && d - t <= Duration::hours(26));
            t = h;
        }
    }
}
