//! Indexed grep/search domain.

pub mod memory;
pub mod query;
pub mod source;
pub mod types;

pub use memory::{MemoryRebuildReport, grep_memory, rebuild_memory};
pub use source::{SourceRebuildReport, grep, grep_source, rebuild_source};
pub use types::{GrepEnvelope, SearchDiagnostic, SearchHit};
