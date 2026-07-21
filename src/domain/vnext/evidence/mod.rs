//! Immutable Evidence-owned Claim records and their Submission associations.

mod assessment;
mod claim;
mod erasure;
mod identity;
mod observation;
mod store;
pub mod submission_claim;

pub use assessment::{
    ASSESSMENT_RECORD_DOMAIN_V1, ASSESSMENT_RECORD_VERSION_V1, AssessmentApplicabilityV1,
    AssessmentBasisV1, AssessmentError, AssessmentInputRefV1, AssessmentInvalidationReasonV1,
    AssessmentInvalidationV1, AssessmentScopeV1, AssessmentTimeBasisV1, AssessmentV1,
    AuthorizationAssessmentInputV1, ClaimAssessmentInputV1, ClosedLeafGateEvaluatorV1,
    DerivedGateSatisfactionV1, EvidenceCutV1, EvidenceMutationAuthorityV1,
    GateAssessmentResolutionV1, LeafGateEvaluationContextV1, LeafGateEvaluationOutputV1,
    ObservationAssessmentInputV1, PinnedLeafGateEvaluatorV1, resolve_gate_assessments,
};
pub use claim::{
    CLAIM_RECORD_DOMAIN_V1, CLAIM_RECORD_VERSION_V1, ClaimError, ClaimSubjectV1, ClaimV1,
    EvidenceClaimPublicationV1, SubmissionRefKindV1, SubmissionRefV1,
};
pub(crate) use erasure::SecurityErasureFinalizationV1;
pub use erasure::{
    SecurityErasureError, SecurityErasureIntentV1, SecurityErasurePublicationV1,
    SecurityErasureReceiptV1,
};
pub use identity::{
    AssessmentIdV1, AssessmentInvalidationIdV1, CLAIM_ID_DOMAIN_V1, ClaimIdV1,
    EvidenceIdentityError, ObservationRecordIdV1, SecurityErasureIntentIdV1,
    SecurityErasureReceiptIdV1,
};
pub use observation::{
    EvidencePayloadManifestV1, EvidenceRedactionPolicyV1, EvidenceRetentionClassV1,
    EvidenceRetentionPolicyV1, EvidenceSecretScanReceiptV1, NominalObservationPayloadV1,
    OBSERVATION_RECORD_DOMAIN_V1, OBSERVATION_RECORD_VERSION_V1, ObservationAcquisitionV1,
    ObservationDraftV1, ObservationError, ObservationKindContractV1, ObservationKindV1,
    ObservationPayloadCommonV1, ObservationPayloadDetailV1, ObservationPayloadFieldSpecV1,
    ObservationPayloadFieldTypeV1, ObservationPayloadFieldV1, ObservationPayloadV1,
    ObservationPublicationRouteV1, ObservationSubjectKindV1, ObservationSubjectV1, ObservationV1,
};
pub use store::{
    AssessmentInvalidationDraftV1, AssessmentInvalidationOutcomeV1, AssessmentPublicationOutcomeV1,
    AuthorizedAssessmentInvalidationV1, AuthorizedAssessmentPublicationV1,
    AuthorizedObservationPublicationV1, AuthorizedSecurityErasureV1,
    CanonicalEvidenceActionRequestV1, EvidenceStoreErrorV1, EvidenceStoreFacadeV1,
    EvidenceStoreStateBindingV1, ObservationPublicationOutcomeV1, SecurityErasureOutcomeV1,
};
pub(crate) use store::{
    ValidatedWorkCompletionEvidenceV1, resolve_current_observation_objects,
    validate_work_completion_evidence,
};
pub use submission_claim::{
    ClaimEntryV1, SUBMISSION_CLAIM_SET_DOMAIN_V1, SubmissionClaimSetError, SubmissionClaimSetV1,
    submission_claim_set_schema_v1,
};
