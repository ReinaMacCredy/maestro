//! Indexed grep/search domain.

pub mod memory;
mod outline;
pub mod query;
pub mod source;
pub mod types;

pub use memory::{MemoryRebuildReport, grep_memory, rebuild_memory};
pub use source::{
    SourceIndexHealth, SourceRebuildReport, grep, grep_source, rebuild_source, source_index_health,
};
pub use types::{GrepEnvelope, SearchDiagnostic, SearchHit};
