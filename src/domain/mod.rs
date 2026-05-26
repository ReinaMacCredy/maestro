//! Transitional domain module root for Maestro's durable concepts.
//!
//! Existing implementation modules stay at their legacy crate-root paths during
//! the folder migration. This root gives future moves a stable target without
//! duplicating behavior.

pub use crate::decisions;
pub use crate::feature;
pub use crate::harness;
pub use crate::install;
pub use crate::skills;
pub use crate::task;
pub use crate::verification as proof;
