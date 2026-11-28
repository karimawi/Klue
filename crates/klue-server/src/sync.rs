//! What changed, for the devices: a revision for each change to what they share, and what
//! changed since any revision they hold.
//!
//! The engine doesn't say what it changed, so each of its revisions is compared with the
//! last: lines, their history, the Home tabs and the shared settings. History is sent as the
//! samples added since a device's revision, since it's most of the data and grows with
//! every check.

use std::collections::{BTreeSet, HashMap, VecDeque};

use chrono::{DateTime, Utc};
use klue_core::{
    history::History,
    store::{Account, AppData},
};
use klue_engine::remote::{SharedSettings, Sync};

/// How many history changes to remember per line, and removed lines overall.
const KEEP: usize = 256;

pub struct Tracker {
    epoch: String,
    rev: u64,
    previous: Option<AppData>,
    /// When each line last changed, and when it came.
    changed: HashMap<String, u64>,
    born: HashMap<String, u64>,
    /// When each line's history gained samples, with the oldest new one.
    added: HashMap<String, VecDeque<(u64, DateTime<Utc>)>>,
    /// Lines whose history changes before this revision were forgotten.
    trimmed: HashMap<String, u64>,
    removed: VecDeque<(String, u64)>,
    groups: u64,
    settings: u64,
    order: u64,
    /// Devices behind this revision sync in full: what came before was forgotten.
    horizon: u64,
    refreshing: (BTreeSet<String>, bool),
}

impl Tracker {
    pub fn new() -> Self {
        Self {
            epoch: uuid::Uuid::new_v4().simple().to_string(),
            rev: 1,
            previous: None,
            changed: HashMap::new(),
            born: HashMap::new(),
            added: HashMap::new(),
            trimmed: HashMap::new(),
            removed: VecDeque::new(),
            groups: 1,
            settings: 1,
            order: 1,
            horizon: 1,
            refreshing: (BTreeSet::new(), false),
        }
    }

    pub fn epoch(&self) -> &str {
        &self.epoch
    }

    pub fn rev(&self) -> u64 {
        self.rev
    }

    /// Compares the data with what it was. Returns whether that's a new revision.
    pub fn observe(&mut self, data: &AppData) -> bool {
        let Some(prev) = self.previous.take() else {
            for a in &data.accounts {
                self.changed.insert(a.id.clone(), self.rev);
                self.born.insert(a.id.clone(), self.rev);
            }
            self.previous = Some(data.clone());
            return false;
        };
        let next = self.rev + 1;
        let mut changed = false;
        if SharedSettings::of(&prev.settings) != SharedSettings::of(&data.settings) {
            self.settings = next;
            changed = true;
        }
        if prev.groups != data.groups {
            self.groups = next;
            changed = true;
        }
        if !prev.accounts.iter().map(|a| &a.id).eq(data.accounts.iter().map(|a| &a.id)) {
            self.order = next;
            changed = true;
        }
        let before: HashMap<&str, &Account> = prev.accounts.iter().map(|a| (a.id.as_str(), a)).collect();
        for a in &data.accounts {
            match before.get(a.id.as_str()) {
                None => {
                    self.changed.insert(a.id.clone(), next);
                    self.born.insert(a.id.clone(), next);
                    changed = true;
                }
                Some(b) => {
                    if bare(a) != bare(b) {
                        self.changed.insert(a.id.clone(), next);
                        changed = true;
                    }
                    // Samples only thinned out aren't news: devices thin theirs the same way.
                    if a.history != b.history
                        && let Some(oldest) = new_samples(&b.history, &a.history)
                    {
                        let list = self.added.entry(a.id.clone()).or_default();
                        list.push_back((next, oldest));
                        if list.len() > KEEP
                            && let Some((rev, _)) = list.pop_front()
                        {
                            self.trimmed.insert(a.id.clone(), rev);
                        }
                        self.changed.insert(a.id.clone(), next);
                        changed = true;
                    }
                }
            }
        }
        let now: BTreeSet<&str> = data.accounts.iter().map(|a| a.id.as_str()).collect();
        for b in prev.accounts.iter().filter(|b| !now.contains(b.id.as_str())) {
            for map in [&mut self.changed, &mut self.born, &mut self.trimmed] {
                map.remove(&b.id);
            }
            self.added.remove(&b.id);
            self.removed.push_back((b.id.clone(), next));
            if self.removed.len() > KEEP
                && let Some((_, rev)) = self.removed.pop_front()
            {
                self.horizon = rev;
            }
            changed = true;
        }
        if changed {
            self.rev = next;
        }
        self.previous = Some(data.clone());
        changed
    }

    /// Notes which lines are refreshing. Returns whether that changed.
    pub fn refreshing(&mut self, ids: &BTreeSet<String>, all: bool) -> bool {
        if self.refreshing.0 == *ids && self.refreshing.1 == all {
            return false;
        }
        self.refreshing = (ids.clone(), all);
        true
    }

    /// What a device at revision `since` of `epoch` needs, from `data`.
    pub fn since(&self, data: &AppData, epoch: Option<&str>, since: u64) -> Sync {
        let full = epoch != Some(self.epoch.as_str()) || since < self.horizon || since > self.rev;
        let mut out = Sync {
            epoch: self.epoch.clone(),
            rev: self.rev,
            full,
            settings: None,
            groups: None,
            order: None,
            accounts: Vec::new(),
            history_full: BTreeSet::new(),
            removed: Vec::new(),
            refreshing: self.refreshing.0.clone(),
            refreshing_all: self.refreshing.1,
        };
        if full {
            out.settings = Some(SharedSettings::of(&data.settings));
            out.groups = Some(data.groups.clone());
            out.order = Some(data.accounts.iter().map(|a| a.id.clone()).collect());
            out.accounts = data.accounts.clone();
            out.history_full = data.accounts.iter().map(|a| a.id.clone()).collect();
            return out;
        }
        if self.settings > since {
            out.settings = Some(SharedSettings::of(&data.settings));
        }
        if self.groups > since {
            out.groups = Some(data.groups.clone());
        }
        if self.order > since {
            out.order = Some(data.accounts.iter().map(|a| a.id.clone()).collect());
        }
        for a in &data.accounts {
            if self.changed.get(&a.id).copied().unwrap_or(0) <= since {
                continue;
            }
            let new_line = self.born.get(&a.id).copied().unwrap_or(0) > since;
            let forgotten = self.trimmed.get(&a.id).is_some_and(|rev| *rev > since);
            if new_line || forgotten {
                out.history_full.insert(a.id.clone());
                out.accounts.push(a.clone());
                continue;
            }
            let oldest = self.added.get(&a.id).into_iter().flatten().filter(|(rev, _)| *rev > since).map(|(_, at)| *at).min();
            let mut line = bare(a);
            if let Some(oldest) = oldest {
                for (key, samples) in &a.history.bundles {
                    let recent: Vec<_> = samples.iter().filter(|s| s.at >= oldest).copied().collect();
                    if !recent.is_empty() {
                        line.history.bundles.insert(key.clone(), recent);
                    }
                }
            }
            out.accounts.push(line);
        }
        out.removed = self.removed.iter().filter(|(_, rev)| *rev > since).map(|(id, _)| id.clone()).collect();
        out
    }
}

/// A line without its history. Every field is named, so a new one isn't forgotten.
fn bare(a: &Account) -> Account {
    let Account {
        id,
        carrier,
        line,
        number,
        login,
        nickname,
        owner_name,
        refresh,
        archived,
        alerts,
        usage,
        last_error,
        last_attempt,
        blocked_until,
        fired,
        created_at,
        main_bundle,
        history: _,
    } = a;
    Account {
        id: id.clone(),
        carrier: *carrier,
        line: *line,
        number: number.clone(),
        login: login.clone(),
        nickname: nickname.clone(),
        owner_name: owner_name.clone(),
        refresh: *refresh,
        archived: *archived,
        alerts: alerts.clone(),
        usage: usage.clone(),
        last_error: last_error.clone(),
        last_attempt: *last_attempt,
        blocked_until: *blocked_until,
        fired: fired.clone(),
        created_at: *created_at,
        main_bundle: main_bundle.clone(),
        history: History::default(),
    }
}

/// The time of the oldest sample in `new` that `old` doesn't have, if any.
fn new_samples(old: &History, new: &History) -> Option<DateTime<Utc>> {
    let mut oldest: Option<DateTime<Utc>> = None;
    for (key, samples) in &new.bundles {
        let had = old.samples(key);
        for s in samples {
            if had.binary_search_by(|h| h.at.cmp(&s.at)).is_err() {
                oldest = Some(oldest.map_or(s.at, |o| o.min(s.at)));
            }
        }
    }
    oldest
}

#[cfg(test)]
mod tests {
    use chrono::Duration;
    use klue_core::store::Card;

    use super::*;

    #[test]
    fn devices_catch_up_on_what_changed_and_end_up_alike() {
        let mut server = klue_engine::demo::data();
        let mut t = Tracker::new();
        t.observe(&server);

        // A device syncs in full.
        let first = t.since(&server, None, 0);
        assert!(first.full);
        let mut device = AppData::default();
        first.apply_to(&mut device);
        assert_eq!(device.accounts, server.accounts);

        // A refresh adds a sample to one line; a tab changes; a line goes.
        let at = Utc::now() + Duration::minutes(5);
        let bundles = server.accounts[0].usage.as_ref().unwrap().bundles.clone();
        server.accounts[0].history.record(at, &bundles);
        server.accounts[0].last_attempt = Some(at);
        server.set_card(klue_core::store::Group::FIRST, Card::main(&server.accounts[1].id), false);
        let gone = server.accounts.remove(3).id;
        server.accounts.swap(0, 1);
        assert!(t.observe(&server));
        assert!(!t.observe(&server), "nothing new the second time");

        let delta = t.since(&server, Some(t.epoch()), first.rev);
        assert!(!delta.full);
        assert_eq!(delta.accounts.len(), 1);
        let sent: usize = delta.accounts[0].history.bundles.values().map(Vec::len).sum();
        assert!(sent <= bundles.len(), "only the new samples: {sent}");
        assert_eq!(delta.removed, [gone]);
        assert!(delta.groups.is_some() && delta.order.is_some() && delta.settings.is_none());

        delta.apply_to(&mut device);
        assert_eq!(device.accounts, server.accounts);
        assert_eq!(device.groups, server.groups);

        // Another start of the server, or an unknown revision, means a full sync.
        assert!(t.since(&server, Some("other"), first.rev).full);
        assert!(t.since(&server, Some(t.epoch()), t.rev() + 5).full);
    }
}
