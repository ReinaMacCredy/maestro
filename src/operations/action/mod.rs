//! Stage-6 Action submission and replay facade.

mod service;

#[allow(
    unused_imports,
    reason = "the canonical action facade keeps implementation children private"
)]
pub(crate) use service::{
    ActionSubmissionErrorV1, ActionSubmissionServiceV1, GovernedOperationPortV1, OperationKindV1,
    OperationResultReadPortV1, OwnerAdmissionV1, OwnerDurableResultV1, OwnerSubmissionOutcomeV1,
    PreparedOperationV1, semantic_request_hash,
};
