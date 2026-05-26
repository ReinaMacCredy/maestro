//! Transitional operations module root for multi-domain workflows.
//!
//! Existing operation-like modules are re-exported here. Empty roots reserve
//! target homes for workflows that still live in command adapters today.

pub use crate::improver;
pub use crate::metrics;
pub use crate::migrate;
pub use crate::update;
