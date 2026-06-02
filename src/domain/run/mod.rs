//! Run aggregate facade.

mod append;
mod discovery;
mod event;
mod evidence;
mod reader;
mod record;

pub(crate) use append::append_manual_event;
pub use discovery::{RunEventLog, managed_event_logs};
#[cfg(test)]
pub(crate) use event::is_accepted_event;
pub(crate) use event::run_dir_name;
pub use event::{HookEventContract, hook_event_contract};
pub use evidence::{
    RunEvidenceLoad, RunEvidenceRecord, load_run_evidence, write_evidence_for_session,
};
pub use reader::{RunEvent, RunEventRecord, visit_managed_events};
pub(crate) use record::{RecordOutcome, record_hook_event};
