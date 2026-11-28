//! The computers using the server: which have been seen and when, and which are
//! listening now. Kept in `devices.json`.

use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    sync::{Arc, Mutex},
};

use chrono::{DateTime, Duration, Utc};
use klue_engine::remote::Device;

/// Devices not seen for this long are forgotten.
const FORGET_AFTER: Duration = Duration::days(90);
/// `last_seen` is saved at most this often, since every request updates it.
const SAVE_EVERY: Duration = Duration::minutes(5);

pub struct Devices {
    path: PathBuf,
    known: Mutex<BTreeMap<String, Device>>,
    /// Open event streams per device.
    live: Arc<Mutex<HashMap<String, usize>>>,
    saved_at: Mutex<DateTime<Utc>>,
}

impl Devices {
    pub fn open(path: PathBuf) -> Self {
        let known: BTreeMap<String, Device> = std::fs::read(&path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        Self { path, known: Mutex::new(known), live: Arc::default(), saved_at: Mutex::new(Utc::now()) }
    }

    /// A request from `id`. New devices and new names or versions are saved at once.
    pub fn seen(&self, id: &str, name: &str, version: Option<&str>) {
        let now = Utc::now();
        let save = {
            let Ok(mut known) = self.known.lock() else { return };
            let device = known.entry(id.to_string()).or_insert_with(|| Device { id: id.to_string(), name: String::new(), version: None, first_seen: now, last_seen: now, connected: false });
            let changed = device.name != name || device.version.as_deref() != version;
            device.name = name.to_string();
            device.version = version.map(String::from);
            device.last_seen = now;
            changed || self.saved_at.lock().is_ok_and(|at| now - *at > SAVE_EVERY)
        };
        if save {
            self.save();
        }
    }

    /// Marks `id` as listening until the guard is dropped.
    pub fn listening(&self, id: &str) -> Listening {
        if let Ok(mut live) = self.live.lock() {
            *live.entry(id.to_string()).or_default() += 1;
        }
        Listening { id: id.to_string(), live: self.live.clone() }
    }

    pub fn list(&self) -> Vec<Device> {
        let live = self.live.lock().map(|l| l.clone()).unwrap_or_default();
        let now = Utc::now();
        let mut out: Vec<Device> = self
            .known
            .lock()
            .map(|k| k.values().filter(|d| now - d.last_seen < FORGET_AFTER).cloned().collect())
            .unwrap_or_default();
        for d in &mut out {
            d.connected = live.get(&d.id).is_some_and(|n| *n > 0);
        }
        out.sort_by(|a, b| b.connected.cmp(&a.connected).then(b.last_seen.cmp(&a.last_seen)));
        out
    }

    fn save(&self) {
        let now = Utc::now();
        let Ok(mut known) = self.known.lock() else { return };
        known.retain(|_, d| now - d.last_seen < FORGET_AFTER);
        if let Ok(bytes) = serde_json::to_vec_pretty(&*known)
            && let Err(e) = std::fs::write(&self.path, bytes)
        {
            tracing::warn!("couldn't save {}: {e}", self.path.display());
        }
        if let Ok(mut at) = self.saved_at.lock() {
            *at = now;
        }
    }
}

pub struct Listening {
    id: String,
    live: Arc<Mutex<HashMap<String, usize>>>,
}

impl Drop for Listening {
    fn drop(&mut self) {
        if let Ok(mut live) = self.live.lock()
            && let Some(n) = live.get_mut(&self.id)
        {
            *n = n.saturating_sub(1);
            if *n == 0 {
                live.remove(&self.id);
            }
        }
    }
}
