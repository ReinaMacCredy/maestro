//! Decision aggregate facade.

pub(crate) mod create;
pub mod query;
pub mod template;

pub use create::create;
pub use query::{decision_entries, decision_id, resolve_decision_path};
