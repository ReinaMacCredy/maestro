//! Run aggregate facade.

mod active;
mod append;
mod discovery;
mod event;
mod evidence;
mod reader;
mod record;
mod trace;

pub(crate) use active::{DeclaredScopeOverlap, declared_scope_overlaps_union};
pub use active::{
    FileOverlap, Presence, SessionActivity, WarmEditor, active_sessions, active_sessions_union,
    current_bound_card, union_session_id, warm_file_overlaps,
};
pub(crate) use append::{append_jsonl_line, append_manual_event, open_managed_appendable};
pub use discovery::{RunEventLog, managed_event_logs};
#[cfg(test)]
pub(crate) use event::is_accepted_event;
pub(crate) use event::run_dir_name;
pub use event::{HookEventContract, hook_event_contract};
pub use evidence::{
    RunEvidenceLoad, RunEvidenceRecord, load_run_evidence, write_evidence_for_session,
};
pub use reader::{RunEvent, RunEventRecord, visit_managed_event_logs, visit_managed_events};
pub(crate) use record::{RecordOutcome, record_hook_event};
pub use trace::{RunStatus, RunTrace, TraceEntry, assemble_trace};
