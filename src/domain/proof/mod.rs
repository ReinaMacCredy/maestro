//! Proof aggregate facade.

mod events;
mod proof_status;
mod stale;
mod verify_task;

pub use events::managed_event_files;
pub use proof_status::{
    latest_proof_failed_for_task, proof_status, proof_status_for_task, proof_status_kind_for_task,
    render_proof_status, ProofStaleReason, ProofStatus, ProofStatusKind, ProofStatusSource,
};
pub use verify_task::{verify_task, TaskVerification, TaskVerificationStatus};

pub(crate) mod compatibility {
    pub use super::events::{event_files_under, managed_event_files};
    pub(crate) use super::proof_status::{
        legacy_proof_status, render_legacy_proof_status, LegacyProofStatus,
    };
    pub use super::stale::{
        is_fresh, stale_reasons, FreshnessInputs, StaleReason, StoredFreshness,
    };
    pub use super::verify_task::{
        freshness_inputs, freshness_inputs_for_task, load_task_by_id, read_report,
        verification_path, verify_task_report, ClaimCheck, LoadedTask, ProofSource,
        VerificationCommand, VerificationReport, VerificationStatus,
    };
}
