//! Pure Work identity, lifecycle, Submission, and closed relation contracts.

mod identity;
mod lifecycle;
mod relation;
mod submission;

pub use identity::{
    WorkIdV1, WorkIdentityError, WorkRelationIdV1, WorkRequirementIdV1, WorkSubmissionIdV1,
};
pub use lifecycle::{
    MAX_WORK_HISTORY_FACTS_V1, MAX_WORK_TRANSITION_REASON_BYTES_V1, WORK_LIFECYCLE_VERSION_V1,
    WorkHistoryFactV1, WorkLifecycleError, WorkLifecycleStateV1, WorkRecordV1, WorkRevisionV1,
    WorkTransitionKindV1, WorkTransitionReasonV1, WorkTransitionV1,
};
pub use relation::{
    ExactStepRevisionRefV1, MAX_IDEMPOTENCY_KEY_BYTES_V1, MAX_PUBLISHED_ROOTS_PER_WORK_V1,
    MAX_RELATION_TEXT_BYTES_V1, MAX_STEP_REFERENCE_BYTES_V1, MAX_WORK_GRAPH_ENDPOINTS_V1,
    MAX_WORK_RELATIONS_V1, MAX_WORK_REQUIREMENTS_V1, WORK_RELATION_VERSION_V1,
    WorkRelationAdmissionV1, WorkRelationEndpointV1, WorkRelationError, WorkRelationGraphV1,
    WorkRelationKindV1, WorkRelationRecordV1, WorkRequirementScopeV1, WorkRequirementV1,
    WorkSnapshotV1,
};
pub use submission::{
    WORK_SUBMISSION_VERSION_V1, WorkRecordWriterV1, WorkSubmissionError, WorkSubmissionSubjectV1,
    WorkSubmissionV1,
};
