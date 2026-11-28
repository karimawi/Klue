//! The engine the windows run on: lines, passwords and refreshes (see `klue_engine`).

use std::sync::Arc;

use klue_core::store::{Secrets, Store};
pub use klue_engine::{Engine as Shared, Snapshot, TICK};

/// The engine with passwords in the system's credential store.
pub fn new(store: Store, demo: bool) -> Arc<Shared> {
    Shared::new(store, Box::new(Secrets::new()), demo, crate::about::VERSION)
}
