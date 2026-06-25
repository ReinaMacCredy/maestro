//! Indexed grep/search domain.

pub mod memory;
pub mod query;
pub mod types;

pub use memory::{MemoryRebuildReport, grep_memory, rebuild_memory};
pub use types::{GrepEnvelope, SearchDiagnostic, SearchHit};
