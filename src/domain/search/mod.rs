//! Indexed grep/search domain.

mod intent;
mod lock;
pub mod memory;
mod outline;
pub mod query;
pub mod source;
pub mod types;

pub use lock::{SearchWriterLock, acquire_writer};
pub(crate) use memory::rebuild_memory_unlocked;
pub use memory::{MemoryRebuildReport, card_list_grep_candidates, grep_memory, rebuild_memory};
pub(crate) use source::rebuild_source_unlocked;
pub use source::{
    SourceIndexHealth, SourceRebuildReport, grep, grep_source, rebuild_source, source_index_health,
};
pub use types::{GrepEnvelope, SearchDiagnostic, SearchHit};
