//! Klue core: carrier clients, data model, local storage and alert rules.
//!
//! The UI lives in the `klue-app` crate. Everything here is UI-agnostic so it can be
//! unit-tested and reused from command-line tools (see `examples/`).

pub mod alerts;
pub mod captcha;
pub mod carriers;
pub mod error;
pub mod format;
pub mod history;
mod http;
mod json;
pub mod model;
pub mod store;
pub mod update;

pub use error::FetchError;
pub use model::{Bundle, BundleKind, BundleState, Carrier, LineType, Unit, Usage};
