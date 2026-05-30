//! Run aggregate facade.

mod append;
mod discovery;
mod event;
mod evidence;
mod reader;
mod record;

pub(crate) use append::append_manual_event;
pub use discovery::{managed_event_logs, RunEventLog};
pub use event::{hook_event_contract, HookEventContract};
pub(crate) use event::{
    is_accepted_event, normalized_event_type, run_dir_name, string_field, UNATTRIBUTED_SESSION,
};
pub use evidence::{
    load_run_evidence, write_evidence_for_session, RunEvidenceLoad, RunEvidenceRecord,
};
pub use reader::{visit_managed_events, RunEvent, RunEventRecord};
pub(crate) use record::record_hook_event;
