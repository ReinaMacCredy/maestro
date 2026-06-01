//! Proof aggregate facade.

mod attempts;
mod claims;
mod commands;
mod events;
mod proof_status;
mod restore_journal;
mod stale;
mod verify_task;

pub use events::{managed_event_files, record_claim};
pub use proof_status::{
    ProofStaleReason, ProofStatus, ProofStatusKind, ProofStatusSource,
    needs_verification_proof_status_kind_for_task, proof_status, proof_status_for_task,
    proof_status_kind_for_task, render_proof_status,
};
pub(crate) use proof_status::{VerificationCommandRead, verification_command_read_for_task};
pub(crate) use restore_journal::replace_task_report_preserving_previous;
pub use verify_task::{TaskVerification, TaskVerificationStatus};
pub(crate) use verify_task::{
    VerificationReport, evaluate_and_write_task_report_attempt, verification_outcome_for_report,
};
