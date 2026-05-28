//! Proof aggregate facade.

mod events;
mod proof_status;
mod stale;
mod verify_task;

pub use events::managed_event_files;
pub use proof_status::{
    needs_verification_proof_status_kind_for_task, proof_status, proof_status_for_task,
    proof_status_kind_for_task, render_proof_status, ProofStaleReason, ProofStatus,
    ProofStatusKind, ProofStatusSource,
};
pub(crate) use proof_status::{verification_command_read_for_task, VerificationCommandRead};
pub(crate) use verify_task::{
    evaluate_and_write_task_report_attempt, replace_task_report_preserving_previous,
    verification_outcome_for_report, VerificationReport,
};
pub use verify_task::{TaskVerification, TaskVerificationStatus};
