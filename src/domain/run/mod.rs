//! Run aggregate facade.

mod append;
mod discovery;
mod event;
mod evidence;
mod reader;
mod record;

pub(crate) use append::append_manual_event;
pub use discovery::{managed_event_logs, RunEventLog};
#[cfg(test)]
pub(crate) use event::is_accepted_event;
pub(crate) use event::run_dir_name;
pub use event::{hook_event_contract, HookEventContract};
pub use evidence::{
    load_run_evidence, write_evidence_for_session, RunEvidenceLoad, RunEvidenceRecord,
};
pub use reader::{visit_managed_events, RunEvent, RunEventRecord};
pub(crate) use record::record_hook_event;
