use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::vnext::authority::{
    ActionRequestIdV1, AdmittedRepositoryActionV1, ContinuedRepositoryActionV1,
    EffectReferenceIdV1, ExecutionAuthorityV1, IdempotencyKeyIdV1,
    PersistedEvidenceMutationAuthorityExpectationV1, RepositoryActionAdmissionInputV1,
    RepositoryActionLeafV1, RepositoryAuthorityAdmissionErrorV1, RepositoryAuthorityArtifactsV1,
    admit_repository_action, continue_durably_admitted_repository_action_attempt,
    current_authorization_receipt_is_persisted, validate_persisted_evidence_mutation_authority,
};
use crate::domain::vnext::gate::{GateError, GateEvaluationResultV1, GateScopeV1, GateSnapshotV1};
use crate::domain::vnext::identity::{
    IdentityError, SchemaIdV1, StoreGenerationIdV1, StoreHeadIdV1, StoreObjectIdV1, derive_identity,
};
use crate::domain::vnext::persistence::{
    AtomicGenerationPublicationV1, AtomicPublicationError, GenerationError, LogicalTombstoneV1,
    PreparedPublicationError, RetentionError, StoreError, StoreGenerationV1, StoreHeadV1,
    StoreIdempotencyProbeV1, StoreIdempotencyV1, StoreObjectError, StoreObjectV1,
    StorePublicationOutcomeV1, StorePublicationViewV1, StoreStateV1, StoreV1,
    VerifiedCollectionAbsenceV1, VerifiedControlledCopyAbsenceV1,
};
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::identity::domain_hash;
use super::{
    AssessmentError, AssessmentIdV1, AssessmentInputRefV1, AssessmentInvalidationIdV1,
    AssessmentInvalidationReasonV1, AssessmentInvalidationV1, AssessmentScopeV1, AssessmentV1,
    ClaimAssessmentInputV1, ClaimSubjectV1, ClaimV1, EvidenceClaimPublicationV1, EvidenceCutV1,
    EvidenceMutationAuthorityV1, GateAssessmentResolutionV1, ObservationAssessmentInputV1,
    ObservationError, ObservationPayloadV1, ObservationRecordIdV1, ObservationSubjectKindV1,
    ObservationV1, SecurityErasureError, SecurityErasureFinalizationV1, SecurityErasureIntentIdV1,
    SecurityErasureIntentV1, SecurityErasurePublicationV1, SecurityErasureReceiptIdV1,
    SecurityErasureReceiptV1, SubmissionRefV1,
};

const EVIDENCE_INDEX_SCHEMA_V1: &str = "maestro.vnext.evidence-index-schema.v1";
const EVIDENCE_INDEX_DOMAIN_V1: &str = "maestro.vnext.evidence-index.v1";
const EVIDENCE_OBSERVATION_SCHEMA_V1: &str = "maestro.vnext.evidence-observation-schema.v1";
const EVIDENCE_CLAIM_SCHEMA_V1: &str = "maestro.vnext.evidence-claim-schema.v1";
const EVIDENCE_ASSESSMENT_SCHEMA_V1: &str = "maestro.vnext.evidence-assessment-schema.v1";
const EVIDENCE_GATE_SNAPSHOT_SCHEMA_V1: &str = "maestro.vnext.evidence-gate-snapshot-schema.v1";
const EVIDENCE_INVALIDATION_SCHEMA_V1: &str = "maestro.vnext.evidence-invalidation-schema.v1";
const EVIDENCE_ERASURE_INTENT_SCHEMA_V1: &str = "maestro.vnext.evidence-erasure-intent-schema.v1";
const EVIDENCE_ERASURE_RECEIPT_SCHEMA_V1: &str = "maestro.vnext.evidence-erasure-receipt-schema.v1";
const EVIDENCE_ACTION_REQUEST_SCHEMA_V1: &str = "maestro.vnext.evidence-action-request-schema.v1";
const OBSERVATION_PUBLICATION_NAMESPACE_V1: &str =
    "maestro.vnext.evidence-observation-authorized-publication.v1";
const ASSESSMENT_PUBLICATION_NAMESPACE_V1: &str =
    "maestro.vnext.evidence-assessment-authorized-publication.v1";
const ASSESSMENT_INVALIDATION_NAMESPACE_V1: &str =
    "maestro.vnext.evidence-assessment-invalidation-authorized-publication.v1";
const SECURITY_ERASURE_BEGIN_NAMESPACE_V1: &str =
    "maestro.vnext.evidence-security-erasure-begin.v1";
const SECURITY_ERASURE_FINALIZE_NAMESPACE_V1: &str =
    "maestro.vnext.evidence-security-erasure-finalize.v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EvidenceStoreStateBindingV1 {
    store_head_id: StoreHeadIdV1,
    store_generation_id: StoreGenerationIdV1,
    evidence_index_object_id: Option<StoreObjectIdV1>,
}

impl EvidenceStoreStateBindingV1 {
    pub const fn store_head_id(self) -> StoreHeadIdV1 {
        self.store_head_id
    }

    pub const fn store_generation_id(self) -> StoreGenerationIdV1 {
        self.store_generation_id
    }

    pub const fn evidence_index_object_id(self) -> Option<StoreObjectIdV1> {
        self.evidence_index_object_id
    }

    pub fn canonical_value(self) -> Result<CborValue, CborError> {
        Ok(CborValue::Array(vec![
            CborValue::text("maestro.vnext.evidence-store-state-binding.v1")?,
            bytes(self.store_head_id.as_bytes()),
            bytes(self.store_generation_id.as_bytes()),
            CborValue::optional(
                self.evidence_index_object_id
                    .map(|object_id| bytes(object_id.as_bytes())),
            ),
        ]))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalEvidenceActionRequestV1 {
    request_id: ActionRequestIdV1,
    idempotency_key_id: IdempotencyKeyIdV1,
    action: RepositoryActionLeafV1,
    subject_commitment: [u8; 32],
    expected_state_commitment: [u8; 32],
    payload_commitment: [u8; 32],
}

impl CanonicalEvidenceActionRequestV1 {
    fn for_observation(
        state: EvidenceStoreStateBindingV1,
        observation: &ObservationV1,
        payload_object: &StoreObjectV1,
        idempotency_key_id: IdempotencyKeyIdV1,
    ) -> Result<Self, EvidenceStoreErrorV1> {
        let action = observation_action(observation)?;
        Self::for_values(
            state,
            action,
            observation_subject_value(observation)?,
            observation_publication_payload_value(observation, payload_object)?,
            idempotency_key_id,
        )
    }

    fn for_assessment(
        state: EvidenceStoreStateBindingV1,
        assessment: &AssessmentV1,
        idempotency_key_id: IdempotencyKeyIdV1,
    ) -> Result<Self, EvidenceStoreErrorV1> {
        Self::for_values(
            state,
            RepositoryActionLeafV1::PublishAssessment,
            assessment.authorization_subject_value()?,
            CborValue::Array(vec![
                CborValue::text("maestro.vnext.evidence-assessment-publication-payload.v1")?,
                CborValue::Bytes(assessment.canonical_bytes()?),
                bytes(assessment.complete_input_cut_hash()),
            ]),
            idempotency_key_id,
        )
    }

    fn for_invalidation(
        state: EvidenceStoreStateBindingV1,
        draft: &AssessmentInvalidationDraftV1,
        complete_cut_hash: [u8; 32],
        idempotency_key_id: IdempotencyKeyIdV1,
    ) -> Result<Self, EvidenceStoreErrorV1> {
        Self::for_values(
            state,
            RepositoryActionLeafV1::InvalidateAssessment,
            CborValue::Array(vec![
                CborValue::text("maestro.vnext.evidence-invalidation-subject.v1")?,
                bytes(draft.assessment_id.as_bytes()),
                bytes(&complete_cut_hash),
            ]),
            draft.canonical_value()?,
            idempotency_key_id,
        )
    }

    fn for_security_erasure(
        state: EvidenceStoreStateBindingV1,
        payload_object_id: StoreObjectIdV1,
        complete_cut_hash: [u8; 32],
        idempotency_key_id: IdempotencyKeyIdV1,
    ) -> Result<Self, EvidenceStoreErrorV1> {
        Self::for_values(
            state,
            RepositoryActionLeafV1::SecurityEraseEvidencePayload,
            CborValue::Array(vec![
                CborValue::text("maestro.vnext.evidence-security-erasure-subject.v1")?,
                bytes(payload_object_id.as_bytes()),
            ]),
            CborValue::Array(vec![
                CborValue::text("maestro.vnext.evidence-security-erasure-payload.v1")?,
                bytes(payload_object_id.as_bytes()),
                bytes(&complete_cut_hash),
            ]),
            idempotency_key_id,
        )
    }

    fn for_values(
        state: EvidenceStoreStateBindingV1,
        action: RepositoryActionLeafV1,
        subject: CborValue,
        payload: CborValue,
        idempotency_key_id: IdempotencyKeyIdV1,
    ) -> Result<Self, EvidenceStoreErrorV1> {
        let subject_commitment = hash(&subject)?;
        let expected_state_commitment = hash(&state.canonical_value()?)?;
        let payload_commitment = hash(&payload)?;
        let request_id = ActionRequestIdV1::from_digest(hash(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.evidence-action-request-id.v1")?,
            CborValue::Unsigned(action.global_tag()),
            bytes(&subject_commitment),
            bytes(&expected_state_commitment),
            bytes(&payload_commitment),
            bytes(idempotency_key_id.as_bytes()),
        ]))?);
        Ok(Self {
            request_id,
            idempotency_key_id,
            action,
            subject_commitment,
            expected_state_commitment,
            payload_commitment,
        })
    }

    pub const fn request_id(&self) -> ActionRequestIdV1 {
        self.request_id
    }

    pub const fn idempotency_key_id(&self) -> IdempotencyKeyIdV1 {
        self.idempotency_key_id
    }

    pub const fn action(&self) -> RepositoryActionLeafV1 {
        self.action
    }

    pub const fn subject_commitment(&self) -> [u8; 32] {
        self.subject_commitment
    }

    pub const fn expected_state_commitment(&self) -> [u8; 32] {
        self.expected_state_commitment
    }

    pub const fn payload_commitment(&self) -> [u8; 32] {
        self.payload_commitment
    }

    pub fn canonical_value(&self) -> Result<CborValue, CborError> {
        Ok(CborValue::Array(vec![
            CborValue::text("maestro.vnext.evidence-action-request.v1")?,
            bytes(self.request_id.as_bytes()),
            bytes(self.idempotency_key_id.as_bytes()),
            CborValue::Unsigned(self.action.global_tag()),
            bytes(&self.subject_commitment),
            bytes(&self.expected_state_commitment),
            bytes(&self.payload_commitment),
        ]))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorizedObservationPublicationV1 {
    state: EvidenceStoreStateBindingV1,
    request: CanonicalEvidenceActionRequestV1,
    authority: ExecutionAuthorityV1,
    observation: ObservationV1,
    payload_object: StoreObjectV1,
}

impl AuthorizedObservationPublicationV1 {
    pub fn new(
        state: EvidenceStoreStateBindingV1,
        request: CanonicalEvidenceActionRequestV1,
        authority: impl Into<ExecutionAuthorityV1>,
        observation: ObservationV1,
        payload_object: StoreObjectV1,
    ) -> Result<Self, EvidenceStoreErrorV1> {
        let authority = authority.into();
        validate_payload_object(&observation, &payload_object)?;
        let expected = CanonicalEvidenceActionRequestV1::for_observation(
            state,
            &observation,
            &payload_object,
            request.idempotency_key_id(),
        )?;
        if request != expected
            || authority.action() != request.action()
            || authority.subject_commitment() != request.subject_commitment()
            || authority.current_state_commitment() != request.expected_state_commitment()
            || authority.exact_payload_commitment() != request.payload_commitment()
            || authority.producer() != observation.producer()
            || authority.producer() != observation.payload().secret_scan_receipt().scanner()
        {
            return Err(EvidenceStoreErrorV1::PublicationBindingMismatch);
        }
        Ok(Self {
            state,
            request,
            authority,
            observation,
            payload_object,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservationPublicationOutcomeV1 {
    store_head: StoreHeadV1,
    observation: ObservationV1,
    replayed: bool,
}

impl ObservationPublicationOutcomeV1 {
    pub const fn store_head(&self) -> &StoreHeadV1 {
        &self.store_head
    }

    pub const fn observation(&self) -> &ObservationV1 {
        &self.observation
    }

    pub const fn replayed(&self) -> bool {
        self.replayed
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorizedAssessmentPublicationV1 {
    state: EvidenceStoreStateBindingV1,
    request: CanonicalEvidenceActionRequestV1,
    authority: ExecutionAuthorityV1,
    gate_snapshot: GateSnapshotV1,
    assessment: AssessmentV1,
}

impl AuthorizedAssessmentPublicationV1 {
    pub fn new(
        state: EvidenceStoreStateBindingV1,
        request: CanonicalEvidenceActionRequestV1,
        authority: impl Into<ExecutionAuthorityV1>,
        gate_snapshot: GateSnapshotV1,
        assessment: AssessmentV1,
    ) -> Result<Self, EvidenceStoreErrorV1> {
        let authority = authority.into();
        let expected = CanonicalEvidenceActionRequestV1::for_assessment(
            state,
            &assessment,
            request.idempotency_key_id(),
        )?;
        if request != expected
            || authority.action() != request.action()
            || authority.subject_commitment() != request.subject_commitment()
            || authority.current_state_commitment() != request.expected_state_commitment()
            || authority.exact_payload_commitment() != request.payload_commitment()
            || !assessment.binds_gate_snapshot(&gate_snapshot)
        {
            return Err(EvidenceStoreErrorV1::PublicationBindingMismatch);
        }
        Ok(Self {
            state,
            request,
            authority,
            gate_snapshot,
            assessment,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssessmentPublicationOutcomeV1 {
    store_head: StoreHeadV1,
    assessment: AssessmentV1,
    replayed: bool,
}

impl AssessmentPublicationOutcomeV1 {
    pub const fn store_head(&self) -> &StoreHeadV1 {
        &self.store_head
    }

    pub const fn assessment(&self) -> &AssessmentV1 {
        &self.assessment
    }

    pub const fn replayed(&self) -> bool {
        self.replayed
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssessmentInvalidationDraftV1 {
    assessment_id: AssessmentIdV1,
    reason: AssessmentInvalidationReasonV1,
    source_revision_hash: [u8; 32],
    replacement_assessment_id: Option<AssessmentIdV1>,
}

impl AssessmentInvalidationDraftV1 {
    pub fn new(
        assessment_id: AssessmentIdV1,
        reason: AssessmentInvalidationReasonV1,
        source_revision_hash: [u8; 32],
        replacement_assessment_id: Option<AssessmentIdV1>,
    ) -> Result<Self, EvidenceStoreErrorV1> {
        if source_revision_hash == [0; 32] || replacement_assessment_id == Some(assessment_id) {
            return Err(EvidenceStoreErrorV1::InvalidInvalidationDraft);
        }
        Ok(Self {
            assessment_id,
            reason,
            source_revision_hash,
            replacement_assessment_id,
        })
    }

    fn canonical_value(&self) -> Result<CborValue, CborError> {
        Ok(CborValue::Array(vec![
            CborValue::text("maestro.vnext.evidence-assessment-invalidation-draft.v1")?,
            bytes(self.assessment_id.as_bytes()),
            CborValue::Unsigned(self.reason.tag()),
            bytes(&self.source_revision_hash),
            CborValue::optional(
                self.replacement_assessment_id
                    .map(|id| bytes(id.as_bytes())),
            ),
        ]))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorizedAssessmentInvalidationV1 {
    state: EvidenceStoreStateBindingV1,
    request: CanonicalEvidenceActionRequestV1,
    authority: ExecutionAuthorityV1,
    complete_cut_hash: [u8; 32],
    draft: AssessmentInvalidationDraftV1,
}

impl AuthorizedAssessmentInvalidationV1 {
    pub fn new(
        state: EvidenceStoreStateBindingV1,
        request: CanonicalEvidenceActionRequestV1,
        authority: impl Into<ExecutionAuthorityV1>,
        complete_cut_hash: [u8; 32],
        draft: AssessmentInvalidationDraftV1,
    ) -> Result<Self, EvidenceStoreErrorV1> {
        let authority = authority.into();
        let expected = CanonicalEvidenceActionRequestV1::for_invalidation(
            state,
            &draft,
            complete_cut_hash,
            request.idempotency_key_id(),
        )?;
        if request != expected
            || authority.action() != request.action()
            || authority.subject_commitment() != request.subject_commitment()
            || authority.current_state_commitment() != request.expected_state_commitment()
            || authority.exact_payload_commitment() != request.payload_commitment()
        {
            return Err(EvidenceStoreErrorV1::PublicationBindingMismatch);
        }
        Ok(Self {
            state,
            request,
            authority,
            complete_cut_hash,
            draft,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssessmentInvalidationOutcomeV1 {
    store_head: StoreHeadV1,
    invalidation: AssessmentInvalidationV1,
    replayed: bool,
}

impl AssessmentInvalidationOutcomeV1 {
    pub const fn invalidation(&self) -> &AssessmentInvalidationV1 {
        &self.invalidation
    }

    pub const fn replayed(&self) -> bool {
        self.replayed
    }

    pub const fn store_head(&self) -> &StoreHeadV1 {
        &self.store_head
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorizedSecurityErasureV1 {
    state: EvidenceStoreStateBindingV1,
    request: CanonicalEvidenceActionRequestV1,
    authority: ExecutionAuthorityV1,
    payload_object_id: StoreObjectIdV1,
    complete_cut_hash: [u8; 32],
}

impl AuthorizedSecurityErasureV1 {
    pub fn new(
        state: EvidenceStoreStateBindingV1,
        request: CanonicalEvidenceActionRequestV1,
        authority: impl Into<ExecutionAuthorityV1>,
        payload_object_id: StoreObjectIdV1,
        complete_cut_hash: [u8; 32],
    ) -> Result<Self, EvidenceStoreErrorV1> {
        let authority = authority.into();
        let expected = CanonicalEvidenceActionRequestV1::for_security_erasure(
            state,
            payload_object_id,
            complete_cut_hash,
            request.idempotency_key_id(),
        )?;
        if request != expected
            || authority.action() != request.action()
            || authority.subject_commitment() != request.subject_commitment()
            || authority.current_state_commitment() != request.expected_state_commitment()
            || authority.exact_payload_commitment() != request.payload_commitment()
        {
            return Err(EvidenceStoreErrorV1::PublicationBindingMismatch);
        }
        Ok(Self {
            state,
            request,
            authority,
            payload_object_id,
            complete_cut_hash,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecurityErasureOutcomeV1 {
    store_head: StoreHeadV1,
    receipt: SecurityErasureReceiptV1,
    replayed: bool,
}

impl SecurityErasureOutcomeV1 {
    pub const fn store_head(&self) -> &StoreHeadV1 {
        &self.store_head
    }

    pub const fn receipt(&self) -> &SecurityErasureReceiptV1 {
        &self.receipt
    }

    pub const fn replayed(&self) -> bool {
        self.replayed
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SecurityErasureCheckpointV1 {
    IntentPublished,
    TombstoneDurable,
    ControlledCopiesErased,
    CollectionCommitted,
    AbsenceVerified,
    FinalPublicationCommitted,
}

pub struct EvidenceStoreFacadeV1<'store> {
    store: &'store mut StoreV1,
}

impl<'store> EvidenceStoreFacadeV1<'store> {
    pub fn new(store: &'store mut StoreV1) -> Self {
        Self { store }
    }

    pub fn current_state_binding(
        &self,
    ) -> Result<EvidenceStoreStateBindingV1, EvidenceStoreErrorV1> {
        let (state, head, generation, objects) = self.store.coherent_publication_snapshot()?;
        if state != StoreStateV1::Active {
            return Err(EvidenceStoreErrorV1::InactiveStore);
        }
        let index = load_optional_evidence_index(&objects)?;
        Ok(EvidenceStoreStateBindingV1 {
            store_head_id: head.id(),
            store_generation_id: generation.id(),
            evidence_index_object_id: index.as_ref().map(|index| index.object.id()),
        })
    }

    pub fn current_evidence_cut(&self) -> Result<EvidenceCutV1, EvidenceStoreErrorV1> {
        let (state, head, generation, objects) = self.store.coherent_publication_snapshot()?;
        if state != StoreStateV1::Active {
            return Err(EvidenceStoreErrorV1::InactiveStore);
        }
        let index = load_optional_evidence_index(&objects)?
            .ok_or(EvidenceStoreErrorV1::MissingEvidenceIndex)?;
        load_validated_complete_evidence_cut(self.store, &head, &generation, &objects, &index)
    }

    pub fn current_observations(&self) -> Result<Vec<ObservationV1>, EvidenceStoreErrorV1> {
        let (state, _, _, objects) = self.store.coherent_publication_snapshot()?;
        if state != StoreStateV1::Active {
            return Err(EvidenceStoreErrorV1::InactiveStore);
        }
        let index = load_optional_evidence_index(&objects)?
            .ok_or(EvidenceStoreErrorV1::MissingEvidenceIndex)?;
        load_current_observations(&objects, &index)
    }

    pub fn canonical_observation_request(
        &self,
        state: EvidenceStoreStateBindingV1,
        observation: &ObservationV1,
        payload_object: &StoreObjectV1,
        idempotency_key_id: IdempotencyKeyIdV1,
    ) -> Result<CanonicalEvidenceActionRequestV1, EvidenceStoreErrorV1> {
        if state != self.current_state_binding()? {
            return Err(EvidenceStoreErrorV1::StaleExpectedStoreState);
        }
        validate_payload_object(observation, payload_object)?;
        CanonicalEvidenceActionRequestV1::for_observation(
            state,
            observation,
            payload_object,
            idempotency_key_id,
        )
    }

    pub fn canonical_assessment_request(
        &self,
        state: EvidenceStoreStateBindingV1,
        assessment: &AssessmentV1,
        idempotency_key_id: IdempotencyKeyIdV1,
    ) -> Result<CanonicalEvidenceActionRequestV1, EvidenceStoreErrorV1> {
        if state != self.current_state_binding()? {
            return Err(EvidenceStoreErrorV1::StaleExpectedStoreState);
        }
        let cut = self.current_evidence_cut()?;
        if assessment.input_store_generation_id() != cut.store_generation_id()
            || assessment.evidence_input_cut_hash() != cut.evidence_input_cut_hash()
            || assessment.complete_input_cut_hash() != cut.complete_cut_hash()
        {
            return Err(EvidenceStoreErrorV1::StaleEvidenceCut);
        }
        CanonicalEvidenceActionRequestV1::for_assessment(state, assessment, idempotency_key_id)
    }

    pub fn canonical_invalidation_request(
        &self,
        state: EvidenceStoreStateBindingV1,
        complete_cut_hash: [u8; 32],
        draft: &AssessmentInvalidationDraftV1,
        idempotency_key_id: IdempotencyKeyIdV1,
    ) -> Result<CanonicalEvidenceActionRequestV1, EvidenceStoreErrorV1> {
        if state != self.current_state_binding()?
            || self.current_evidence_cut()?.complete_cut_hash() != &complete_cut_hash
        {
            return Err(EvidenceStoreErrorV1::StaleEvidenceCut);
        }
        CanonicalEvidenceActionRequestV1::for_invalidation(
            state,
            draft,
            complete_cut_hash,
            idempotency_key_id,
        )
    }

    pub fn canonical_security_erasure_request(
        &self,
        state: EvidenceStoreStateBindingV1,
        payload_object_id: StoreObjectIdV1,
        complete_cut_hash: [u8; 32],
        idempotency_key_id: IdempotencyKeyIdV1,
    ) -> Result<CanonicalEvidenceActionRequestV1, EvidenceStoreErrorV1> {
        if state != self.current_state_binding()?
            || self.current_evidence_cut()?.complete_cut_hash() != &complete_cut_hash
        {
            return Err(EvidenceStoreErrorV1::StaleEvidenceCut);
        }
        CanonicalEvidenceActionRequestV1::for_security_erasure(
            state,
            payload_object_id,
            complete_cut_hash,
            idempotency_key_id,
        )
    }

    pub fn publish_observation(
        &mut self,
        plan: AuthorizedObservationPublicationV1,
    ) -> Result<ObservationPublicationOutcomeV1, EvidenceStoreErrorV1> {
        if let Some((run_id, owner)) = plan.observation.acquisition().run_binding()
            && !crate::domain::vnext::execution::store::current_run_binding_is_persisted(
                self.store, run_id, owner,
            )
            .map_err(|_| EvidenceStoreErrorV1::RunProvenanceNotCurrent)?
        {
            return Err(EvidenceStoreErrorV1::RunProvenanceNotCurrent);
        }
        let meaning_digest = hash(&CborValue::Array(vec![
            CborValue::text(OBSERVATION_PUBLICATION_NAMESPACE_V1)?,
            plan.request.canonical_value()?,
            plan.observation.canonical_value()?,
            CborValue::Bytes(plan.payload_object.canonical_bytes().to_vec()),
        ]))?;
        let probe = StoreIdempotencyProbeV1::new(
            OBSERVATION_PUBLICATION_NAMESPACE_V1,
            *plan.request.idempotency_key_id().as_bytes(),
            meaning_digest,
        )?;
        let expected = plan.clone();
        let publication = self
            .store
            .publish_generation_atomically_with_prepare(&probe, |view| {
                let head = view
                    .active_head()?
                    .ok_or(EvidenceStoreErrorV1::InactiveStore)?;
                let generation = view
                    .active_generation()?
                    .ok_or(EvidenceStoreErrorV1::InactiveStore)?;
                let objects = view.active_generation_objects()?;
                let index = load_optional_evidence_index(&objects)?;
                if head.id() != expected.state.store_head_id()
                    || generation.id() != expected.state.store_generation_id()
                    || index.as_ref().map(|index| index.object.id())
                        != expected.state.evidence_index_object_id()
                    || expected.observation.store_domain_id() != generation.domain().id()
                    || expected.observation.recorded_at() == 0
                    || expected.authority.producer() != expected.observation.producer()
                {
                    return Err(EvidenceStoreErrorV1::StaleExpectedStoreState);
                }
                validate_payload_object(&expected.observation, &expected.payload_object)?;
                let request_object = evidence_action_request_object(&expected.request)?;
                let admission = admit_repository_action(
                    view,
                    &generation,
                    RepositoryActionAdmissionInputV1::new(
                        expected.request.request_id(),
                        expected.authority.clone(),
                    ),
                )?;
                if admission.action() != expected.request.action()
                    || admission.accepted_h_time() != expected.observation.recorded_at()
                {
                    return Err(EvidenceStoreErrorV1::UntrustedMutationTime);
                }
                let observation_object = evidence_observation_object(&expected.observation)?;
                let next_index = build_next_observation_index(
                    index.as_ref(),
                    &expected.observation,
                    observation_object.id(),
                    expected.payload_object.id(),
                )?;
                let produced = vec![
                    observation_object.clone(),
                    expected.payload_object.clone(),
                    next_index.clone(),
                ];
                let artifacts = admission.issue_committed_evidence_artifacts(
                    &request_object,
                    &produced,
                    std::slice::from_ref(&observation_object),
                )?;
                build_authorized_evidence_publication(
                    view.domain().clone(),
                    &head,
                    &generation,
                    &objects,
                    index.as_ref(),
                    &admission,
                    &artifacts,
                    &expected.request,
                    request_object,
                    observation_object,
                    expected.payload_object.clone(),
                    next_index,
                    meaning_digest,
                )
            });
        let publication = match publication {
            Ok(publication) => publication,
            Err(PreparedPublicationError::Store(error)) => return Err(map_store_error(error)),
            Err(PreparedPublicationError::Prepare(error)) => return Err(error),
        };
        validate_observation_result(
            self.store,
            publication.result(),
            &plan.observation,
            &plan.payload_object,
        )?;
        Ok(ObservationPublicationOutcomeV1 {
            store_head: publication.head().clone(),
            observation: plan.observation,
            replayed: matches!(publication, StorePublicationOutcomeV1::Replayed { .. }),
        })
    }

    pub fn publish_assessment(
        &mut self,
        plan: AuthorizedAssessmentPublicationV1,
    ) -> Result<AssessmentPublicationOutcomeV1, EvidenceStoreErrorV1> {
        let meaning_digest = hash(&CborValue::Array(vec![
            CborValue::text(ASSESSMENT_PUBLICATION_NAMESPACE_V1)?,
            plan.request.canonical_value()?,
            CborValue::Bytes(plan.assessment.canonical_bytes()?),
        ]))?;
        let probe = StoreIdempotencyProbeV1::new(
            ASSESSMENT_PUBLICATION_NAMESPACE_V1,
            *plan.request.idempotency_key_id().as_bytes(),
            meaning_digest,
        )?;
        let expected = plan.clone();
        let publication = self
            .store
            .publish_generation_atomically_with_prepare(&probe, |view| {
                let head = view
                    .active_head()?
                    .ok_or(EvidenceStoreErrorV1::InactiveStore)?;
                let generation = view
                    .active_generation()?
                    .ok_or(EvidenceStoreErrorV1::InactiveStore)?;
                let objects = view.active_generation_objects()?;
                let index = load_optional_evidence_index(&objects)?
                    .ok_or(EvidenceStoreErrorV1::MissingEvidenceIndex)?;
                let input_cut = load_validated_complete_evidence_cut(
                    view,
                    &head,
                    &generation,
                    &objects,
                    &index,
                )?;
                if head.id() != expected.state.store_head_id()
                    || generation.id() != expected.state.store_generation_id()
                    || Some(index.object.id()) != expected.state.evidence_index_object_id()
                    || expected.assessment.store_domain_id() != generation.domain().id()
                    || expected.assessment.input_store_generation_id() != generation.id()
                    || expected.assessment.evidence_input_cut_hash()
                        != &evidence_input_cut_hash(&index.observations)?
                    || expected.assessment.complete_input_cut_hash()
                        != input_cut.complete_cut_hash()
                {
                    return Err(EvidenceStoreErrorV1::StaleEvidenceCut);
                }
                expected
                    .assessment
                    .validate_recomputed(&expected.gate_snapshot, &input_cut)?;
                let request_object = evidence_action_request_object(&expected.request)?;
                let admission = admit_repository_action(
                    view,
                    &generation,
                    RepositoryActionAdmissionInputV1::new(
                        expected.request.request_id(),
                        expected.authority.clone(),
                    ),
                )?;
                if admission.action() != RepositoryActionLeafV1::PublishAssessment
                    || admission.accepted_h_time() != expected.assessment.evaluated_at()
                {
                    return Err(EvidenceStoreErrorV1::UntrustedMutationTime);
                }
                validate_assessment_input_references(&index, &objects, &expected.assessment)?;
                let gate_snapshot_object = evidence_gate_snapshot_object(&expected.gate_snapshot)?;
                let assessment_object =
                    evidence_assessment_object(&expected.assessment, gate_snapshot_object.id())?;
                let next_index = build_next_assessment_index(
                    &index,
                    expected.assessment.id(),
                    assessment_object.id(),
                )?;
                let produced = vec![
                    gate_snapshot_object,
                    assessment_object.clone(),
                    next_index.clone(),
                ];
                let artifacts = admission.issue_committed_evidence_artifacts(
                    &request_object,
                    &produced,
                    std::slice::from_ref(&assessment_object),
                )?;
                build_authorized_evidence_record_publication(
                    view.domain().clone(),
                    &head,
                    &generation,
                    &objects,
                    &index,
                    &admission,
                    &artifacts,
                    &expected.request,
                    request_object,
                    produced,
                    next_index,
                    meaning_digest,
                    ASSESSMENT_PUBLICATION_NAMESPACE_V1,
                )
            });
        let publication = match publication {
            Ok(publication) => publication,
            Err(PreparedPublicationError::Store(error)) => return Err(map_store_error(error)),
            Err(PreparedPublicationError::Prepare(error)) => return Err(error),
        };
        Ok(AssessmentPublicationOutcomeV1 {
            store_head: publication.head().clone(),
            assessment: plan.assessment,
            replayed: matches!(publication, StorePublicationOutcomeV1::Replayed { .. }),
        })
    }

    pub fn invalidate_assessment(
        &mut self,
        plan: AuthorizedAssessmentInvalidationV1,
    ) -> Result<AssessmentInvalidationOutcomeV1, EvidenceStoreErrorV1> {
        let meaning_digest = hash(&CborValue::Array(vec![
            CborValue::text(ASSESSMENT_INVALIDATION_NAMESPACE_V1)?,
            plan.request.canonical_value()?,
            plan.draft.canonical_value()?,
            bytes(&plan.complete_cut_hash),
        ]))?;
        let probe = StoreIdempotencyProbeV1::new(
            ASSESSMENT_INVALIDATION_NAMESPACE_V1,
            *plan.request.idempotency_key_id().as_bytes(),
            meaning_digest,
        )?;
        let expected = plan.clone();
        let publication = self
            .store
            .publish_generation_atomically_with_prepare(&probe, |view| {
                let head = view
                    .active_head()?
                    .ok_or(EvidenceStoreErrorV1::InactiveStore)?;
                let generation = view
                    .active_generation()?
                    .ok_or(EvidenceStoreErrorV1::InactiveStore)?;
                let objects = view.active_generation_objects()?;
                let index = load_optional_evidence_index(&objects)?
                    .ok_or(EvidenceStoreErrorV1::MissingEvidenceIndex)?;
                if head.id() != expected.state.store_head_id()
                    || generation.id() != expected.state.store_generation_id()
                    || Some(index.object.id()) != expected.state.evidence_index_object_id()
                {
                    return Err(EvidenceStoreErrorV1::StaleExpectedStoreState);
                }
                let cut = load_validated_complete_evidence_cut(
                    view,
                    &head,
                    &generation,
                    &objects,
                    &index,
                )?;
                if cut.complete_cut_hash() != &expected.complete_cut_hash {
                    return Err(EvidenceStoreErrorV1::StaleEvidenceCut);
                }
                let assessment = cut
                    .assessment(expected.draft.assessment_id)
                    .ok_or(EvidenceStoreErrorV1::UnknownAssessment)?;
                if let Some(replacement) = expected.draft.replacement_assessment_id
                    && cut.assessment(replacement).is_none()
                {
                    return Err(EvidenceStoreErrorV1::UnknownAssessment);
                }
                let request_object = evidence_action_request_object(&expected.request)?;
                let admission = admit_repository_action(
                    view,
                    &generation,
                    RepositoryActionAdmissionInputV1::new(
                        expected.request.request_id(),
                        expected.authority.clone(),
                    ),
                )?;
                let mutation_authority = EvidenceMutationAuthorityV1::from_admitted_action(
                    &admission,
                    &request_object,
                    expected.complete_cut_hash,
                )?;
                let invalidation = AssessmentInvalidationV1::authorized(
                    assessment,
                    expected.draft.reason,
                    expected.draft.source_revision_hash,
                    &mutation_authority,
                    expected.complete_cut_hash,
                    expected.draft.replacement_assessment_id,
                )?;
                let assessment_object_id = index
                    .assessments
                    .iter()
                    .find(|entry| entry.assessment_id == assessment.id())
                    .map(|entry| entry.assessment_object_id)
                    .ok_or(EvidenceStoreErrorV1::InvalidEvidenceIndex)?;
                let invalidation_object =
                    evidence_invalidation_object(&invalidation, assessment_object_id)?;
                let next_index = build_next_invalidation_index(
                    &index,
                    invalidation.id(),
                    invalidation_object.id(),
                    invalidation.assessment_id(),
                )?;
                let produced = vec![invalidation_object.clone(), next_index.clone()];
                let artifacts = admission.issue_committed_evidence_artifacts(
                    &request_object,
                    &produced,
                    std::slice::from_ref(&invalidation_object),
                )?;
                build_authorized_evidence_record_publication(
                    view.domain().clone(),
                    &head,
                    &generation,
                    &objects,
                    &index,
                    &admission,
                    &artifacts,
                    &expected.request,
                    request_object,
                    produced,
                    next_index,
                    meaning_digest,
                    ASSESSMENT_INVALIDATION_NAMESPACE_V1,
                )
            });
        let publication = match publication {
            Ok(publication) => publication,
            Err(PreparedPublicationError::Store(error)) => return Err(map_store_error(error)),
            Err(PreparedPublicationError::Prepare(error)) => return Err(error),
        };
        let invalidation = decode_result_invalidation(self.store, publication.result())?;
        Ok(AssessmentInvalidationOutcomeV1 {
            store_head: publication.head().clone(),
            invalidation,
            replayed: matches!(publication, StorePublicationOutcomeV1::Replayed { .. }),
        })
    }

    pub fn security_erase_payload(
        &mut self,
        plan: AuthorizedSecurityErasureV1,
    ) -> Result<SecurityErasureOutcomeV1, EvidenceStoreErrorV1> {
        self.security_erase_payload_with_checkpoint(plan, |_| Ok(()))
    }

    fn security_erase_payload_with_checkpoint(
        &mut self,
        plan: AuthorizedSecurityErasureV1,
        mut checkpoint: impl FnMut(SecurityErasureCheckpointV1) -> Result<(), EvidenceStoreErrorV1>,
    ) -> Result<SecurityErasureOutcomeV1, EvidenceStoreErrorV1> {
        let (begin_replayed, head, intent, existing_receipt) =
            begin_security_erasure(self.store, &plan)?;
        checkpoint(SecurityErasureCheckpointV1::IntentPublished)?;
        if let Some(receipt) = existing_receipt {
            verify_security_erasure_absence(self.store, &intent, &receipt)?;
            self.store.finish_controlled_copy_erasure(
                intent.controlled_copy_plan(),
                receipt.controlled_copy_absence_receipt_hash(),
            )?;
            return Ok(SecurityErasureOutcomeV1 {
                store_head: head,
                receipt,
                replayed: true,
            });
        }

        let tombstone = match self.store.load_tombstone(plan.payload_object_id)? {
            Some(tombstone) => {
                if tombstone.reason_digest() != intent.dependency_closure_hash()
                    || tombstone.invalidation_digest() != intent.dependency_closure_hash()
                {
                    return Err(EvidenceStoreErrorV1::ErasureRecoveryMismatch);
                }
                tombstone
            }
            None => LogicalTombstoneV1::new(
                head.id(),
                plan.payload_object_id,
                *intent.dependency_closure_hash(),
                *intent.dependency_closure_hash(),
            )?,
        };
        self.store.ensure_tombstone(&tombstone)?;
        checkpoint(SecurityErasureCheckpointV1::TombstoneDurable)?;
        let controlled_copy_absence = self
            .store
            .erase_controlled_copies(intent.controlled_copy_plan())?;
        checkpoint(SecurityErasureCheckpointV1::ControlledCopiesErased)?;
        let snapshot = self.store.snapshot_reachability()?;
        let collection = self
            .store
            .plan_exact_collection(&snapshot, plan.payload_object_id)?;
        self.store.collect(&collection)?;
        checkpoint(SecurityErasureCheckpointV1::CollectionCommitted)?;
        let absence = self
            .store
            .verify_collected_object_absence(&tombstone, &collection)?;
        checkpoint(SecurityErasureCheckpointV1::AbsenceVerified)?;
        let receipt = finalize_erasure_receipt(&intent, absence, controlled_copy_absence)?;
        verify_security_erasure_absence(self.store, &intent, &receipt)?;
        let final_meaning_digest = hash(&CborValue::Array(vec![
            CborValue::text(SECURITY_ERASURE_FINALIZE_NAMESPACE_V1)?,
            plan.request.canonical_value()?,
            receipt.canonical_value(),
        ]))?;
        let final_probe = StoreIdempotencyProbeV1::new(
            SECURITY_ERASURE_FINALIZE_NAMESPACE_V1,
            *plan.request.idempotency_key_id().as_bytes(),
            final_meaning_digest,
        )?;
        let expected_intent = intent.clone();
        let expected_receipt = receipt.clone();
        let final_idempotency_key = plan.request.idempotency_key_id();
        let final_publication =
            self.store
                .publish_generation_atomically_with_prepare(&final_probe, |view| {
                    let current_head = view
                        .active_head()?
                        .ok_or(EvidenceStoreErrorV1::InactiveStore)?;
                    let generation = view
                        .active_generation()?
                        .ok_or(EvidenceStoreErrorV1::InactiveStore)?;
                    let objects = view.active_generation_objects()?;
                    let index = load_optional_evidence_index(&objects)?
                        .ok_or(EvidenceStoreErrorV1::MissingEvidenceIndex)?;
                    let loaded = load_erasure_from_index(&objects, &index, expected_intent.id())?;
                    if loaded.0 != expected_intent || loaded.1.is_some() {
                        return Err(EvidenceStoreErrorV1::ErasureRecoveryMismatch);
                    }
                    let continuation = continue_durably_admitted_repository_action_attempt(
                        view,
                        &generation,
                        StoreObjectIdV1::from_digest(*expected_intent.action_request_hash()),
                        expected_intent.authority_basis_object_id(),
                        expected_intent.authorization_receipt(),
                        EffectReferenceIdV1::from_digest(*expected_intent.id().as_bytes()),
                        loaded.2,
                        expected_intent.authority_epoch(),
                        expected_intent.authority_epoch_commitment(),
                    )?;
                    let erasure_receipt_object =
                        evidence_erasure_receipt_object(&expected_receipt, loaded.2)?;
                    let next_index = build_final_erasure_index(
                        &index,
                        expected_intent.id(),
                        expected_receipt.id(),
                        erasure_receipt_object.id(),
                    )?;
                    let produced = vec![erasure_receipt_object, next_index.clone()];
                    build_security_erasure_final_publication(
                        view.domain().clone(),
                        &current_head,
                        &generation,
                        &objects,
                        &index,
                        &continuation,
                        produced,
                        next_index,
                        final_idempotency_key,
                        final_meaning_digest,
                    )
                });
        let final_publication = match final_publication {
            Ok(publication) => publication,
            Err(PreparedPublicationError::Store(error)) => return Err(map_store_error(error)),
            Err(PreparedPublicationError::Prepare(error)) => return Err(error),
        };
        checkpoint(SecurityErasureCheckpointV1::FinalPublicationCommitted)?;
        let (_, _, stored_receipt) = load_current_erasure(
            self.store,
            plan.request.request_id(),
            plan.payload_object_id,
        )?;
        let stored_receipt = stored_receipt.ok_or(EvidenceStoreErrorV1::ErasureRecoveryMismatch)?;
        if stored_receipt != receipt {
            return Err(EvidenceStoreErrorV1::ErasureRecoveryMismatch);
        }
        verify_security_erasure_absence(self.store, &intent, &stored_receipt)?;
        self.store.finish_controlled_copy_erasure(
            intent.controlled_copy_plan(),
            stored_receipt.controlled_copy_absence_receipt_hash(),
        )?;
        Ok(SecurityErasureOutcomeV1 {
            store_head: final_publication.head().clone(),
            receipt,
            replayed: begin_replayed
                || matches!(
                    final_publication,
                    StorePublicationOutcomeV1::Replayed { .. }
                ),
        })
    }
}

fn verify_security_erasure_absence(
    store: &StoreV1,
    intent: &SecurityErasureIntentV1,
    receipt: &SecurityErasureReceiptV1,
) -> Result<(), EvidenceStoreErrorV1> {
    store.verify_recorded_collection_absence(
        receipt.payload_object_id(),
        receipt.tombstone_id(),
        receipt.collection_plan_id(),
        *receipt.destroyed_payload_hash(),
        *receipt.physical_absence_receipt_hash(),
    )?;
    if receipt.intent_id() != intent.id()
        || receipt.controlled_copy_plan_id() != intent.controlled_copy_plan().plan_id()
    {
        return Err(EvidenceStoreErrorV1::ErasureRecoveryMismatch);
    }
    let copy_absence =
        store.verify_controlled_copy_erasure_absence(intent.controlled_copy_plan())?;
    if copy_absence.plan_id() != receipt.controlled_copy_plan_id()
        || copy_absence.receipt_hash() != receipt.controlled_copy_absence_receipt_hash()
    {
        return Err(EvidenceStoreErrorV1::ErasureRecoveryMismatch);
    }
    Ok(())
}

fn begin_security_erasure(
    store: &mut StoreV1,
    plan: &AuthorizedSecurityErasureV1,
) -> Result<
    (
        bool,
        StoreHeadV1,
        SecurityErasureIntentV1,
        Option<SecurityErasureReceiptV1>,
    ),
    EvidenceStoreErrorV1,
> {
    if let Some((head, intent, receipt)) =
        find_current_erasure(store, plan.request.request_id(), plan.payload_object_id)?
    {
        return Ok((true, head, intent, receipt));
    }
    let barrier_identity = hash(&CborValue::Array(vec![
        CborValue::text("maestro.vnext.evidence-security-erasure-copy-barrier.v1")?,
        plan.request.canonical_value()?,
        plan.authority.producer().canonical_value(),
    ]))?;
    let expected_for_plan = plan.clone();
    let controlled_copy_plan = store
        .prepare_controlled_copy_erasure_plan(plan.payload_object_id, barrier_identity, |view| {
            validate_security_erasure_preparation(view, &expected_for_plan)
        })
        .map_err(|error| match error {
            PreparedPublicationError::Store(error) => map_store_error(error),
            PreparedPublicationError::Prepare(error) => error,
        })?;
    let meaning_digest = hash(&CborValue::Array(vec![
        CborValue::text(SECURITY_ERASURE_BEGIN_NAMESPACE_V1)?,
        plan.request.canonical_value()?,
        bytes(plan.payload_object_id.as_bytes()),
        bytes(&plan.complete_cut_hash),
    ]))?;
    let probe = StoreIdempotencyProbeV1::new(
        SECURITY_ERASURE_BEGIN_NAMESPACE_V1,
        *plan.request.idempotency_key_id().as_bytes(),
        meaning_digest,
    )?;
    let expected = plan.clone();
    let recovery_copy_plan = controlled_copy_plan.clone();
    let expected_copy_plan = controlled_copy_plan;
    let begin_publication = store.publish_generation_atomically_with_prepare(&probe, |view| {
        let head = view
            .active_head()?
            .ok_or(EvidenceStoreErrorV1::InactiveStore)?;
        let generation = view
            .active_generation()?
            .ok_or(EvidenceStoreErrorV1::InactiveStore)?;
        let objects = view.active_generation_objects()?;
        let index = load_optional_evidence_index(&objects)?
            .ok_or(EvidenceStoreErrorV1::MissingEvidenceIndex)?;
        if head.id() != expected.state.store_head_id()
            || generation.id() != expected.state.store_generation_id()
            || Some(index.object.id()) != expected.state.evidence_index_object_id()
            || !index
                .observations
                .iter()
                .any(|entry| entry.payload_object_id == Some(expected.payload_object_id))
        {
            return Err(EvidenceStoreErrorV1::StaleExpectedStoreState);
        }
        let cut = load_validated_complete_evidence_cut(view, &head, &generation, &objects, &index)?;
        if cut.complete_cut_hash() != &expected.complete_cut_hash {
            return Err(EvidenceStoreErrorV1::StaleEvidenceCut);
        }
        let request_object = evidence_action_request_object(&expected.request)?;
        let admission = admit_repository_action(
            view,
            &generation,
            RepositoryActionAdmissionInputV1::new(
                expected.request.request_id(),
                expected.authority.clone(),
            ),
        )?;
        let mutation_authority = EvidenceMutationAuthorityV1::from_admitted_action(
            &admission,
            &request_object,
            expected.complete_cut_hash,
        )?;
        let erasure = SecurityErasurePublicationV1::begin(
            expected.payload_object_id,
            &mutation_authority,
            cut.assessments().iter().collect(),
            expected_copy_plan,
        )?;
        let (intent_object, invalidation_objects, next_index) =
            build_next_erasure_begin_index(&index, &erasure)?;
        let mut produced = invalidation_objects;
        produced.extend([intent_object, next_index.clone()]);
        let durable_result_references = produced[..produced.len() - 1].to_vec();
        let artifacts = admission.issue_in_doubt_evidence_artifacts(
            &request_object,
            &produced,
            &durable_result_references,
            EffectReferenceIdV1::from_digest(*erasure.intent().id().as_bytes()),
        )?;
        build_security_erasure_begin_publication(
            view.domain().clone(),
            &head,
            &generation,
            &objects,
            &index,
            &admission,
            &artifacts,
            &expected.request,
            request_object,
            produced,
            next_index,
            expected.payload_object_id,
            meaning_digest,
        )
    });
    let begin_publication = match begin_publication {
        Ok(publication) => publication,
        Err(PreparedPublicationError::Store(error)) => {
            if !matches!(&error, StoreError::PublicationRecoveryRequired { .. }) {
                store.abort_controlled_copy_erasure_plan(&recovery_copy_plan)?;
            }
            return Err(map_store_error(error));
        }
        Err(PreparedPublicationError::Prepare(error)) => {
            store.abort_controlled_copy_erasure_plan(&recovery_copy_plan)?;
            return Err(error);
        }
    };
    let replayed = matches!(
        begin_publication,
        StorePublicationOutcomeV1::Replayed { .. }
    );
    let (head, intent, receipt) =
        load_current_erasure(store, plan.request.request_id(), plan.payload_object_id)?;
    Ok((replayed, head, intent, receipt))
}

fn validate_security_erasure_preparation(
    view: &crate::domain::vnext::persistence::StorePublicationViewV1<'_>,
    expected: &AuthorizedSecurityErasureV1,
) -> Result<(), EvidenceStoreErrorV1> {
    let head = view
        .active_head()?
        .ok_or(EvidenceStoreErrorV1::InactiveStore)?;
    let generation = view
        .active_generation()?
        .ok_or(EvidenceStoreErrorV1::InactiveStore)?;
    let objects = view.active_generation_objects()?;
    let index = load_optional_evidence_index(&objects)?
        .ok_or(EvidenceStoreErrorV1::MissingEvidenceIndex)?;
    if head.id() != expected.state.store_head_id()
        || generation.id() != expected.state.store_generation_id()
        || Some(index.object.id()) != expected.state.evidence_index_object_id()
        || !index
            .observations
            .iter()
            .any(|entry| entry.payload_object_id == Some(expected.payload_object_id))
    {
        return Err(EvidenceStoreErrorV1::StaleExpectedStoreState);
    }
    let cut = load_validated_complete_evidence_cut(view, &head, &generation, &objects, &index)?;
    if cut.complete_cut_hash() != &expected.complete_cut_hash {
        return Err(EvidenceStoreErrorV1::StaleEvidenceCut);
    }
    let request_object = evidence_action_request_object(&expected.request)?;
    let admission = admit_repository_action(
        view,
        &generation,
        RepositoryActionAdmissionInputV1::new(
            expected.request.request_id(),
            expected.authority.clone(),
        ),
    )?;
    EvidenceMutationAuthorityV1::from_admitted_action(
        &admission,
        &request_object,
        expected.complete_cut_hash,
    )?;
    Ok(())
}

fn load_complete_evidence_cut(
    head: &StoreHeadV1,
    generation: &StoreGenerationV1,
    objects: &[StoreObjectV1],
    index: &ActiveEvidenceIndexV1,
) -> Result<EvidenceCutV1, EvidenceStoreErrorV1> {
    load_current_observations(objects, index)?;
    load_evidence_cut_without_observation_payloads(head, generation, objects, index)
}

fn load_evidence_cut_without_observation_payloads(
    head: &StoreHeadV1,
    generation: &StoreGenerationV1,
    objects: &[StoreObjectV1],
    index: &ActiveEvidenceIndexV1,
) -> Result<EvidenceCutV1, EvidenceStoreErrorV1> {
    let assessments_with_snapshots = load_current_assessments(objects, index)?;
    let assessments = assessments_with_snapshots
        .into_iter()
        .map(|(assessment, _)| assessment)
        .collect();
    let by_id = objects
        .iter()
        .map(|object| (object.id(), object))
        .collect::<std::collections::BTreeMap<_, _>>();
    let invalidation_schema = evidence_schema_id(EVIDENCE_INVALIDATION_SCHEMA_V1)?;
    let mut invalidations = Vec::with_capacity(index.invalidations.len());
    for entry in &index.invalidations {
        let object = by_id
            .get(&entry.invalidation_object_id)
            .filter(|object| object.schema_id() == invalidation_schema)
            .ok_or(EvidenceStoreErrorV1::InvalidEvidenceIndex)?;
        let CborValue::Bytes(value) = object.value() else {
            return Err(EvidenceStoreErrorV1::InvalidEvidenceIndex);
        };
        let invalidation = AssessmentInvalidationV1::from_canonical_bytes(value)?;
        if invalidation.id() != entry.invalidation_id
            || invalidation.assessment_id() != entry.assessment_id
        {
            return Err(EvidenceStoreErrorV1::InvalidEvidenceIndex);
        }
        validate_stored_invalidation_authority(generation, objects, &invalidation)?;
        invalidations.push(invalidation);
    }
    Ok(EvidenceCutV1::from_current_index(
        generation.domain().id(),
        head.id(),
        generation.id(),
        index.object.id(),
        evidence_input_cut_hash(&index.observations)?,
        assessments,
        invalidations,
    )?)
}

fn load_current_assessments(
    objects: &[StoreObjectV1],
    index: &ActiveEvidenceIndexV1,
) -> Result<Vec<(AssessmentV1, GateSnapshotV1)>, EvidenceStoreErrorV1> {
    let by_id = objects
        .iter()
        .map(|object| (object.id(), object))
        .collect::<std::collections::BTreeMap<_, _>>();
    let assessment_schema = evidence_schema_id(EVIDENCE_ASSESSMENT_SCHEMA_V1)?;
    let gate_snapshot_schema = evidence_schema_id(EVIDENCE_GATE_SNAPSHOT_SCHEMA_V1)?;
    let mut assessments = Vec::with_capacity(index.assessments.len());
    for entry in &index.assessments {
        let object = by_id
            .get(&entry.assessment_object_id)
            .filter(|object| object.schema_id() == assessment_schema)
            .ok_or(EvidenceStoreErrorV1::InvalidEvidenceIndex)?;
        let CborValue::Bytes(value) = object.value() else {
            return Err(EvidenceStoreErrorV1::InvalidEvidenceIndex);
        };
        let assessment = AssessmentV1::from_canonical_bytes(value)?;
        let [gate_snapshot_object_id] = object.references() else {
            return Err(EvidenceStoreErrorV1::InvalidEvidenceIndex);
        };
        let gate_snapshot_object = by_id
            .get(gate_snapshot_object_id)
            .filter(|object| object.schema_id() == gate_snapshot_schema)
            .ok_or(EvidenceStoreErrorV1::InvalidEvidenceIndex)?;
        let CborValue::Bytes(gate_snapshot_bytes) = gate_snapshot_object.value() else {
            return Err(EvidenceStoreErrorV1::InvalidEvidenceIndex);
        };
        let gate_snapshot = GateSnapshotV1::from_canonical_bytes(gate_snapshot_bytes)?;
        if assessment.id() != entry.assessment_id
            || **object != evidence_assessment_object(&assessment, gate_snapshot_object.id())?
            || **gate_snapshot_object != evidence_gate_snapshot_object(&gate_snapshot)?
            || !assessment.binds_gate_snapshot(&gate_snapshot)
        {
            return Err(EvidenceStoreErrorV1::InvalidEvidenceIndex);
        }
        assessment.validate_recomputed_from_persisted_snapshot(&gate_snapshot)?;
        assessments.push((assessment, gate_snapshot));
    }
    Ok(assessments)
}

fn validate_stored_invalidation_authority(
    generation: &StoreGenerationV1,
    objects: &[StoreObjectV1],
    invalidation: &AssessmentInvalidationV1,
) -> Result<(), EvidenceStoreErrorV1> {
    let invalidation_schema = evidence_schema_id(EVIDENCE_INVALIDATION_SCHEMA_V1)?;
    let invalidation_objects = objects
        .iter()
        .filter(|object| object.schema_id() == invalidation_schema)
        .filter(|object| {
            let CborValue::Bytes(value) = object.value() else {
                return false;
            };
            AssessmentInvalidationV1::from_canonical_bytes(value).as_ref() == Ok(invalidation)
        })
        .collect::<Vec<_>>();
    let [invalidation_object] = invalidation_objects.as_slice() else {
        return Err(EvidenceStoreErrorV1::InvalidInvalidationAuthority);
    };
    let request_object_id = StoreObjectIdV1::from_digest(*invalidation.action_request_hash());
    let request_schema = evidence_schema_id(EVIDENCE_ACTION_REQUEST_SCHEMA_V1)?;
    let request_objects = objects
        .iter()
        .filter(|object| object.id() == request_object_id && object.schema_id() == request_schema)
        .collect::<Vec<_>>();
    let [request_object] = request_objects.as_slice() else {
        return Err(EvidenceStoreErrorV1::InvalidInvalidationAuthority);
    };
    let CborValue::Array(request_fields) = request_object.value() else {
        return Err(EvidenceStoreErrorV1::InvalidInvalidationAuthority);
    };
    let [
        CborValue::Text(domain),
        request_id,
        idempotency_key_id,
        CborValue::Unsigned(action_tag),
        subject_commitment,
        expected_state_commitment,
        payload_commitment,
    ] = request_fields.as_slice()
    else {
        return Err(EvidenceStoreErrorV1::InvalidInvalidationAuthority);
    };
    let request_id_bytes = exact_digest(request_id)?;
    let idempotency_key_id = exact_digest(idempotency_key_id)?;
    let subject_commitment = exact_digest(subject_commitment)?;
    let expected_state_commitment = exact_digest(expected_state_commitment)?;
    let payload_commitment = exact_digest(payload_commitment)?;
    let recomputed_request_id = hash(&CborValue::Array(vec![
        CborValue::text("maestro.vnext.evidence-action-request-id.v1")?,
        CborValue::Unsigned(*action_tag),
        bytes(&subject_commitment),
        bytes(&expected_state_commitment),
        bytes(&payload_commitment),
        bytes(&idempotency_key_id),
    ]))?;
    if domain != "maestro.vnext.evidence-action-request.v1"
        || request_id_bytes != *invalidation.action_request_id().as_bytes()
        || recomputed_request_id != request_id_bytes
        || !matches!(*action_tag, 41 | 42)
        || subject_commitment == [0; 32]
        || expected_state_commitment == [0; 32]
        || payload_commitment == [0; 32]
    {
        return Err(EvidenceStoreErrorV1::InvalidInvalidationAuthority);
    }
    let (expected_subject_commitment, expected_payload_commitment, expected_effect) =
        match *action_tag {
            41 => {
                let draft = AssessmentInvalidationDraftV1::new(
                    invalidation.assessment_id(),
                    invalidation.reason(),
                    *invalidation.source_revision_hash(),
                    invalidation.replacement_assessment_id(),
                )?;
                (
                    hash(&CborValue::Array(vec![
                        CborValue::text("maestro.vnext.evidence-invalidation-subject.v1")?,
                        bytes(invalidation.assessment_id().as_bytes()),
                        bytes(invalidation.evidence_cut_hash()),
                    ]))?,
                    hash(&draft.canonical_value()?)?,
                    None,
                )
            }
            42 => {
                let intent_schema = evidence_schema_id(EVIDENCE_ERASURE_INTENT_SCHEMA_V1)?;
                let matching = objects
                    .iter()
                    .filter(|object| object.schema_id() == intent_schema)
                    .filter_map(|object| {
                        SecurityErasureIntentV1::from_canonical_value(object.value())
                            .ok()
                            .map(|intent| (object, intent))
                    })
                    .filter(|(_, intent)| {
                        intent.action_request_id() == invalidation.action_request_id()
                            && intent.action_request_hash() == invalidation.action_request_hash()
                            && intent.authorization_receipt().id()
                                == invalidation.authority_receipt_id()
                            && intent.accepted_h_time() == invalidation.invalidated_at()
                            && intent
                                .affected_assessments()
                                .contains(&invalidation.assessment_id())
                    })
                    .collect::<Vec<_>>();
                let [(object, intent)] = matching.as_slice() else {
                    return Err(EvidenceStoreErrorV1::InvalidInvalidationAuthority);
                };
                if invalidation.reason() != AssessmentInvalidationReasonV1::InputTombstoned
                    || invalidation.source_revision_hash() != intent.dependency_closure_hash()
                    || invalidation.replacement_assessment_id().is_some()
                    || invalidation.evidence_cut_hash() != intent.evidence_cut_hash()
                {
                    return Err(EvidenceStoreErrorV1::InvalidInvalidationAuthority);
                }
                (
                    hash(&CborValue::Array(vec![
                        CborValue::text("maestro.vnext.evidence-security-erasure-subject.v1")?,
                        bytes(intent.payload_object_id().as_bytes()),
                    ]))?,
                    hash(&CborValue::Array(vec![
                        CborValue::text("maestro.vnext.evidence-security-erasure-payload.v1")?,
                        bytes(intent.payload_object_id().as_bytes()),
                        bytes(intent.evidence_cut_hash()),
                    ]))?,
                    Some((
                        EffectReferenceIdV1::from_digest(*intent.id().as_bytes()),
                        object.id(),
                    )),
                )
            }
            _ => return Err(EvidenceStoreErrorV1::InvalidInvalidationAuthority),
        };
    if subject_commitment != expected_subject_commitment
        || payload_commitment != expected_payload_commitment
    {
        return Err(EvidenceStoreErrorV1::InvalidInvalidationAuthority);
    }
    let receipt = validate_persisted_evidence_mutation_authority(
        generation,
        objects,
        request_object,
        PersistedEvidenceMutationAuthorityExpectationV1::new(
            invalidation.authority_receipt_id(),
            invalidation.action_request_id(),
            invalidation.invalidated_at(),
            invalidation_object.id(),
            expected_effect,
        ),
    )
    .map_err(|_| EvidenceStoreErrorV1::InvalidInvalidationAuthority)?;
    let receipt_hash = domain_hash(
        "maestro.vnext.evidence.invalidation-authority-receipt.v1",
        &CborValue::Bytes(receipt.canonical_bytes()?),
    )
    .map_err(|_| EvidenceStoreErrorV1::InvalidInvalidationAuthority)?;
    if receipt_hash != *invalidation.authority_receipt_hash() {
        return Err(EvidenceStoreErrorV1::InvalidInvalidationAuthority);
    }
    Ok(())
}

trait EvidenceGenerationSnapshotSourceV1 {
    fn evidence_generation_snapshot(
        &self,
        generation_id: StoreGenerationIdV1,
    ) -> Result<(StoreHeadV1, StoreGenerationV1, Vec<StoreObjectV1>), EvidenceStoreErrorV1>;
}

impl EvidenceGenerationSnapshotSourceV1 for StoreV1 {
    fn evidence_generation_snapshot(
        &self,
        generation_id: StoreGenerationIdV1,
    ) -> Result<(StoreHeadV1, StoreGenerationV1, Vec<StoreObjectV1>), EvidenceStoreErrorV1> {
        Ok(self.coherent_generation_snapshot(generation_id)?)
    }
}

impl EvidenceGenerationSnapshotSourceV1 for StorePublicationViewV1<'_> {
    fn evidence_generation_snapshot(
        &self,
        generation_id: StoreGenerationIdV1,
    ) -> Result<(StoreHeadV1, StoreGenerationV1, Vec<StoreObjectV1>), EvidenceStoreErrorV1> {
        Ok(self.coherent_generation_snapshot(generation_id)?)
    }
}

fn load_validated_complete_evidence_cut(
    source: &impl EvidenceGenerationSnapshotSourceV1,
    head: &StoreHeadV1,
    generation: &StoreGenerationV1,
    objects: &[StoreObjectV1],
    index: &ActiveEvidenceIndexV1,
) -> Result<EvidenceCutV1, EvidenceStoreErrorV1> {
    let cut = load_complete_evidence_cut(head, generation, objects, index)?;
    let assessments = load_current_assessments(objects, index)?;
    if assessments.is_empty() {
        return Ok(cut);
    }
    let current_assessment_ids = assessments
        .iter()
        .map(|(assessment, _)| assessment.id())
        .collect::<std::collections::BTreeSet<_>>();
    let mut ancestor_ids = std::collections::BTreeSet::new();
    let mut previous = generation.previous();
    let mut preceding_ordinal = generation.ordinal();
    while let Some(generation_id) = previous {
        if !ancestor_ids.insert(generation_id) {
            return Err(EvidenceStoreErrorV1::InvalidEvidenceIndex);
        }
        let (_, ancestor, _) = source.evidence_generation_snapshot(generation_id)?;
        if ancestor.ordinal() >= preceding_ordinal {
            return Err(EvidenceStoreErrorV1::InvalidEvidenceIndex);
        }
        preceding_ordinal = ancestor.ordinal();
        previous = ancestor.previous();
    }
    for (assessment, gate_snapshot) in assessments {
        if !ancestor_ids.contains(&assessment.input_store_generation_id()) {
            return Err(EvidenceStoreErrorV1::AssessmentInputNotCurrent);
        }
        let (input_head, input_generation, input_objects) =
            source.evidence_generation_snapshot(assessment.input_store_generation_id())?;
        let input_index = load_optional_evidence_index(&input_objects)?
            .ok_or(EvidenceStoreErrorV1::MissingEvidenceIndex)?;
        let input_cut = load_evidence_cut_without_observation_payloads(
            &input_head,
            &input_generation,
            &input_objects,
            &input_index,
        )?;
        if input_cut
            .assessments()
            .iter()
            .any(|input| !current_assessment_ids.contains(&input.id()))
        {
            return Err(EvidenceStoreErrorV1::InvalidEvidenceIndex);
        }
        validate_assessment_input_references(&input_index, &input_objects, &assessment)?;
        assessment.validate_recomputed(&gate_snapshot, &input_cut)?;
    }
    Ok(cut)
}

fn load_current_observations(
    objects: &[StoreObjectV1],
    index: &ActiveEvidenceIndexV1,
) -> Result<Vec<ObservationV1>, EvidenceStoreErrorV1> {
    let by_id = objects
        .iter()
        .map(|object| (object.id(), object))
        .collect::<std::collections::BTreeMap<_, _>>();
    let observation_schema = evidence_schema_id(EVIDENCE_OBSERVATION_SCHEMA_V1)?;
    let mut observations = Vec::with_capacity(index.observations.len());
    for entry in &index.observations {
        let object = by_id
            .get(&entry.observation_object_id)
            .filter(|object| object.schema_id() == observation_schema)
            .ok_or(EvidenceStoreErrorV1::InvalidEvidenceIndex)?;
        let CborValue::Bytes(value) = object.value() else {
            return Err(EvidenceStoreErrorV1::InvalidEvidenceIndex);
        };
        let observation = ObservationV1::from_canonical_bytes(value)?;
        if observation.id() != entry.observation_id
            || observation.acquisition_id().copied() != entry.acquisition_id
            || **object != evidence_observation_object(&observation)?
        {
            return Err(EvidenceStoreErrorV1::InvalidEvidenceIndex);
        }
        match entry.payload_object_id {
            Some(payload_id) => {
                let payload = by_id
                    .get(&payload_id)
                    .ok_or(EvidenceStoreErrorV1::InvalidEvidenceIndex)?;
                if observation.payload().object_id() != payload_id {
                    return Err(EvidenceStoreErrorV1::InvalidEvidenceIndex);
                }
                validate_payload_object(&observation, payload)?;
            }
            None => {
                if by_id.contains_key(&observation.payload().object_id()) {
                    return Err(EvidenceStoreErrorV1::InvalidEvidenceIndex);
                }
            }
        }
        observations.push(observation);
    }
    let current_ids = observations
        .iter()
        .map(ObservationV1::id)
        .collect::<std::collections::BTreeSet<_>>();
    if observations
        .iter()
        .flat_map(|observation| observation.lineage())
        .any(|source| !current_ids.contains(source))
    {
        return Err(EvidenceStoreErrorV1::UnresolvedObservationLineage);
    }
    Ok(observations)
}

fn evidence_input_cut_hash(
    observations: &[EvidenceObservationIndexEntryV1],
) -> Result<[u8; 32], EvidenceStoreErrorV1> {
    Ok(hash(&CborValue::Array(vec![
        CborValue::text("maestro.vnext.evidence-input-cut.v1")?,
        CborValue::Array(
            observations
                .iter()
                .map(|entry| {
                    CborValue::Array(vec![
                        bytes(entry.observation_id.as_bytes()),
                        bytes(entry.observation_object_id.as_bytes()),
                        CborValue::optional(
                            entry
                                .payload_object_id
                                .map(|payload| bytes(payload.as_bytes())),
                        ),
                        CborValue::optional(entry.acquisition_id.map(|id| bytes(&id))),
                    ])
                })
                .collect(),
        ),
    ]))?)
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct EvidenceObservationIndexEntryV1 {
    observation_id: ObservationRecordIdV1,
    observation_object_id: StoreObjectIdV1,
    payload_object_id: Option<StoreObjectIdV1>,
    acquisition_id: Option<[u8; 32]>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct EvidenceAssessmentIndexEntryV1 {
    assessment_id: AssessmentIdV1,
    assessment_object_id: StoreObjectIdV1,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct EvidenceInvalidationIndexEntryV1 {
    invalidation_id: AssessmentInvalidationIdV1,
    invalidation_object_id: StoreObjectIdV1,
    assessment_id: AssessmentIdV1,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct EvidenceErasureIndexEntryV1 {
    intent_id: SecurityErasureIntentIdV1,
    intent_object_id: StoreObjectIdV1,
    receipt_id: Option<SecurityErasureReceiptIdV1>,
    receipt_object_id: Option<StoreObjectIdV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ActiveEvidenceIndexV1 {
    object: StoreObjectV1,
    previous_index_object_id: Option<StoreObjectIdV1>,
    observations: Vec<EvidenceObservationIndexEntryV1>,
    assessments: Vec<EvidenceAssessmentIndexEntryV1>,
    invalidations: Vec<EvidenceInvalidationIndexEntryV1>,
    erasures: Vec<EvidenceErasureIndexEntryV1>,
}

pub(crate) fn resolve_current_observation_objects(
    objects: &[StoreObjectV1],
    observations: &[ObservationV1],
) -> Result<Vec<StoreObjectV1>, EvidenceStoreErrorV1> {
    let index =
        load_optional_evidence_index(objects)?.ok_or(EvidenceStoreErrorV1::MissingEvidenceIndex)?;
    let by_id = objects
        .iter()
        .map(|object| (object.id(), object))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut resolved = Vec::with_capacity(observations.len());
    for observation in observations {
        let entry = index
            .observations
            .iter()
            .find(|entry| entry.observation_id == observation.id())
            .ok_or(EvidenceStoreErrorV1::ObservationNotCurrent)?;
        if entry.payload_object_id != Some(observation.payload().object_id())
            || entry.acquisition_id != observation.acquisition_id().copied()
        {
            return Err(EvidenceStoreErrorV1::ObservationNotCurrent);
        }
        let object = by_id
            .get(&entry.observation_object_id)
            .ok_or(EvidenceStoreErrorV1::ObservationNotCurrent)?;
        if **object != evidence_observation_object(observation)?
            || !entry
                .payload_object_id
                .is_some_and(|payload| by_id.contains_key(&payload))
        {
            return Err(EvidenceStoreErrorV1::ObservationNotCurrent);
        }
        resolved.push((*object).clone());
    }
    resolved.sort_by_key(StoreObjectV1::id);
    if resolved.windows(2).any(|pair| pair[0].id() == pair[1].id()) {
        return Err(EvidenceStoreErrorV1::DuplicateObservationOrAcquisition);
    }
    Ok(resolved)
}

#[derive(Clone, Debug)]
pub(crate) struct ValidatedWorkCompletionEvidenceV1 {
    observation_objects: Vec<StoreObjectV1>,
    assessment_objects: Vec<StoreObjectV1>,
    gate_snapshot_object: StoreObjectV1,
    evidence_index_object: StoreObjectV1,
    complete_cut_hash: [u8; 32],
}

impl ValidatedWorkCompletionEvidenceV1 {
    pub(crate) fn observation_objects(&self) -> &[StoreObjectV1] {
        &self.observation_objects
    }

    pub(crate) fn assessment_objects(&self) -> &[StoreObjectV1] {
        &self.assessment_objects
    }

    pub(crate) const fn gate_snapshot_object(&self) -> &StoreObjectV1 {
        &self.gate_snapshot_object
    }

    pub(crate) const fn evidence_index_object(&self) -> &StoreObjectV1 {
        &self.evidence_index_object
    }

    pub(crate) const fn complete_cut_hash(&self) -> &[u8; 32] {
        &self.complete_cut_hash
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "Work completion validates one exact Store, Contract, Evidence, Gate, and time snapshot"
)]
pub(crate) fn validate_work_completion_evidence(
    source: &StorePublicationViewV1<'_>,
    head: &StoreHeadV1,
    generation: &StoreGenerationV1,
    objects: &[StoreObjectV1],
    work_id: crate::domain::vnext::work::WorkIdV1,
    contract_generation_id: crate::domain::vnext::contract::runtime::ContractGenerationIdV1,
    contract_root_id: crate::domain::vnext::identity::ContractRootIdV1,
    gate_snapshot: &GateSnapshotV1,
    resolutions: &[GateAssessmentResolutionV1],
    evidence: &EvidenceClaimPublicationV1,
    as_of: u64,
) -> Result<ValidatedWorkCompletionEvidenceV1, EvidenceStoreErrorV1> {
    if as_of == 0
        || generation.domain().id() != source.domain().id()
        || gate_snapshot.work_id() != work_id
        || gate_snapshot.contract_generation_id() != contract_generation_id
        || gate_snapshot.contract_root_id() != contract_root_id
    {
        return Err(EvidenceStoreErrorV1::WorkCompletionEvidenceMismatch);
    }
    let index =
        load_optional_evidence_index(objects)?.ok_or(EvidenceStoreErrorV1::MissingEvidenceIndex)?;
    let cut = load_validated_complete_evidence_cut(source, head, generation, objects, &index)?;
    let current_assessments = load_current_assessments(objects, &index)?;
    let current_by_id = current_assessments
        .iter()
        .map(|(assessment, snapshot)| (assessment.id(), (assessment, snapshot)))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut required_gate_ids = gate_snapshot
        .roots()
        .iter()
        .copied()
        .filter(|gate_id| {
            gate_snapshot
                .node(*gate_id)
                .is_some_and(|node| node.scope() == GateScopeV1::Work)
        })
        .collect::<Vec<_>>();
    required_gate_ids.sort_unstable();
    let mut supplied_resolutions = resolutions.iter().collect::<Vec<_>>();
    supplied_resolutions.sort_unstable_by_key(|resolution| resolution.gate_id());
    if required_gate_ids.is_empty()
        || supplied_resolutions
            .iter()
            .map(|resolution| resolution.gate_id())
            .collect::<Vec<_>>()
            != required_gate_ids
    {
        return Err(EvidenceStoreErrorV1::WorkCompletionGateNotPassed);
    }
    let mut required_assessment_ids = std::collections::BTreeSet::new();
    for resolution in supplied_resolutions {
        resolution.validate_recomputed(&cut)?;
        if resolution.snapshot_id() != gate_snapshot.id()
            || resolution.scope() != AssessmentScopeV1::Work
            || resolution.as_of() != as_of
            || resolution.result() != GateEvaluationResultV1::Pass
            || resolution.applicable_assessment_ids().is_empty()
        {
            return Err(EvidenceStoreErrorV1::WorkCompletionGateNotPassed);
        }
        for assessment_id in resolution.applicable_assessment_ids() {
            let Some((assessment, snapshot)) = current_by_id.get(assessment_id) else {
                return Err(EvidenceStoreErrorV1::WorkCompletionGateNotPassed);
            };
            if *snapshot != gate_snapshot
                || assessment.result() != GateEvaluationResultV1::Pass
                || !required_assessment_ids.insert(*assessment_id)
            {
                return Err(EvidenceStoreErrorV1::WorkCompletionGateNotPassed);
            }
        }
    }

    let observation_objects =
        resolve_current_observation_objects(objects, evidence.observations())?;
    for observation in evidence.observations() {
        let matching_work_subjects = observation
            .subjects()
            .iter()
            .filter(|subject| subject.kind() == ObservationSubjectKindV1::Work)
            .filter(|subject| {
                subject.subject_id() == work_id.as_bytes()
                    && subject.contract_generation_id() == Some(contract_generation_id)
                    && subject.revision_id() == contract_root_id.as_bytes()
            })
            .count();
        if observation.store_domain_id() != generation.domain().id()
            || observation.observed_at() > as_of
            || observation.recorded_at() > as_of
            || matching_work_subjects != 1
        {
            return Err(EvidenceStoreErrorV1::WorkCompletionEvidenceMismatch);
        }
    }
    if evidence.claims().iter().any(|claim| {
        !matches!(claim.submission(), SubmissionRefV1::Work(_))
            || !matches!(
                claim.subject(),
                ClaimSubjectV1::Work {
                    work_id: claim_work_id,
                    contract_root_id: claim_root_id,
                    ..
                } if *claim_work_id == work_id && *claim_root_id == contract_root_id
            )
    }) {
        return Err(EvidenceStoreErrorV1::WorkCompletionEvidenceMismatch);
    }

    let by_id = objects
        .iter()
        .map(|object| (object.id(), object))
        .collect::<std::collections::BTreeMap<_, _>>();
    let assessment_objects = required_assessment_ids
        .iter()
        .map(|assessment_id| {
            let entry = index
                .assessments
                .iter()
                .find(|entry| entry.assessment_id == *assessment_id)
                .ok_or(EvidenceStoreErrorV1::WorkCompletionGateNotPassed)?;
            by_id
                .get(&entry.assessment_object_id)
                .map(|object| (*object).clone())
                .ok_or(EvidenceStoreErrorV1::WorkCompletionGateNotPassed)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let gate_snapshot_value = CborValue::Bytes(gate_snapshot.canonical_bytes()?);
    let gate_snapshot_schema = evidence_schema_id(EVIDENCE_GATE_SNAPSHOT_SCHEMA_V1)?;
    let gate_snapshot_objects = objects
        .iter()
        .filter(|object| {
            object.schema_id() == gate_snapshot_schema && object.value() == &gate_snapshot_value
        })
        .collect::<Vec<_>>();
    let [gate_snapshot_object] = gate_snapshot_objects.as_slice() else {
        return Err(EvidenceStoreErrorV1::WorkCompletionGateNotPassed);
    };
    if assessment_objects
        .iter()
        .any(|assessment| !assessment.references().contains(&gate_snapshot_object.id()))
    {
        return Err(EvidenceStoreErrorV1::WorkCompletionGateNotPassed);
    }
    Ok(ValidatedWorkCompletionEvidenceV1 {
        observation_objects,
        assessment_objects,
        gate_snapshot_object: (*gate_snapshot_object).clone(),
        evidence_index_object: index.object,
        complete_cut_hash: *cut.complete_cut_hash(),
    })
}

pub(crate) fn load_optional_evidence_index(
    objects: &[StoreObjectV1],
) -> Result<Option<ActiveEvidenceIndexV1>, EvidenceStoreErrorV1> {
    let schema = evidence_schema_id(EVIDENCE_INDEX_SCHEMA_V1)?;
    let matches = objects
        .iter()
        .filter(|object| object.schema_id() == schema)
        .collect::<Vec<_>>();
    if matches.is_empty() {
        return Ok(None);
    }
    let mut referenced_indices = std::collections::BTreeSet::new();
    for candidate in &matches {
        let CborValue::Array(fields) = candidate.value() else {
            return Err(EvidenceStoreErrorV1::InvalidEvidenceIndex);
        };
        let Some(previous) = fields.get(1) else {
            return Err(EvidenceStoreErrorV1::InvalidEvidenceIndex);
        };
        if let Some(previous) = optional_object_id(previous)? {
            referenced_indices.insert(previous);
        }
    }
    let tips = matches
        .iter()
        .copied()
        .filter(|candidate| !referenced_indices.contains(&candidate.id()))
        .collect::<Vec<_>>();
    let [object] = tips.as_slice() else {
        return Err(EvidenceStoreErrorV1::AmbiguousEvidenceIndex);
    };
    let CborValue::Array(fields) = object.value() else {
        return Err(EvidenceStoreErrorV1::InvalidEvidenceIndex);
    };
    let [
        CborValue::Text(domain),
        previous_index_object_id,
        CborValue::Array(observation_rows),
        CborValue::Array(assessment_rows),
        CborValue::Array(invalidation_rows),
        CborValue::Array(erasure_rows),
    ] = fields.as_slice()
    else {
        return Err(EvidenceStoreErrorV1::InvalidEvidenceIndex);
    };
    if domain != EVIDENCE_INDEX_DOMAIN_V1 {
        return Err(EvidenceStoreErrorV1::InvalidEvidenceIndex);
    }
    let previous_index_object_id = optional_object_id(previous_index_object_id)?;
    if previous_index_object_id.is_some_and(|previous| {
        !matches.iter().any(|candidate| candidate.id() == previous)
            || !object.references().contains(&previous)
    }) {
        return Err(EvidenceStoreErrorV1::InvalidEvidenceIndex);
    }
    let mut observations = Vec::with_capacity(observation_rows.len());
    for row in observation_rows {
        let CborValue::Array(fields) = row else {
            return Err(EvidenceStoreErrorV1::InvalidEvidenceIndex);
        };
        let [observation, object_id, payload, acquisition] = fields.as_slice() else {
            return Err(EvidenceStoreErrorV1::InvalidEvidenceIndex);
        };
        observations.push(EvidenceObservationIndexEntryV1 {
            observation_id: ObservationRecordIdV1::from_bytes(exact_digest(observation)?)
                .map_err(|_| EvidenceStoreErrorV1::InvalidEvidenceIndex)?,
            observation_object_id: StoreObjectIdV1::from_digest(exact_digest(object_id)?),
            payload_object_id: optional_object_id(payload)?,
            acquisition_id: optional_digest(acquisition)?,
        });
    }
    if observations.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(EvidenceStoreErrorV1::InvalidEvidenceIndex);
    }
    let mut assessments = Vec::with_capacity(assessment_rows.len());
    for row in assessment_rows {
        let CborValue::Array(fields) = row else {
            return Err(EvidenceStoreErrorV1::InvalidEvidenceIndex);
        };
        let [assessment_id, assessment_object_id] = fields.as_slice() else {
            return Err(EvidenceStoreErrorV1::InvalidEvidenceIndex);
        };
        assessments.push(EvidenceAssessmentIndexEntryV1 {
            assessment_id: AssessmentIdV1::from_bytes(exact_digest(assessment_id)?)
                .map_err(|_| EvidenceStoreErrorV1::InvalidEvidenceIndex)?,
            assessment_object_id: StoreObjectIdV1::from_digest(exact_digest(assessment_object_id)?),
        });
    }
    if assessments.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(EvidenceStoreErrorV1::InvalidEvidenceIndex);
    }
    let assessment_ids = assessments
        .iter()
        .map(|entry| entry.assessment_id)
        .collect::<std::collections::BTreeSet<_>>();
    let mut invalidations = Vec::with_capacity(invalidation_rows.len());
    for row in invalidation_rows {
        let CborValue::Array(fields) = row else {
            return Err(EvidenceStoreErrorV1::InvalidEvidenceIndex);
        };
        let [invalidation_id, invalidation_object_id, assessment_id] = fields.as_slice() else {
            return Err(EvidenceStoreErrorV1::InvalidEvidenceIndex);
        };
        let assessment_id = AssessmentIdV1::from_bytes(exact_digest(assessment_id)?)
            .map_err(|_| EvidenceStoreErrorV1::InvalidEvidenceIndex)?;
        if !assessment_ids.contains(&assessment_id) {
            return Err(EvidenceStoreErrorV1::InvalidEvidenceIndex);
        }
        invalidations.push(EvidenceInvalidationIndexEntryV1 {
            invalidation_id: AssessmentInvalidationIdV1::from_bytes(exact_digest(invalidation_id)?)
                .map_err(|_| EvidenceStoreErrorV1::InvalidEvidenceIndex)?,
            invalidation_object_id: StoreObjectIdV1::from_digest(exact_digest(
                invalidation_object_id,
            )?),
            assessment_id,
        });
    }
    if invalidations.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(EvidenceStoreErrorV1::InvalidEvidenceIndex);
    }
    let mut erasures = Vec::with_capacity(erasure_rows.len());
    for row in erasure_rows {
        let CborValue::Array(fields) = row else {
            return Err(EvidenceStoreErrorV1::InvalidEvidenceIndex);
        };
        let [intent_id, intent_object_id, receipt_id, receipt_object_id] = fields.as_slice() else {
            return Err(EvidenceStoreErrorV1::InvalidEvidenceIndex);
        };
        let receipt_id = optional_digest(receipt_id)?
            .map(SecurityErasureReceiptIdV1::from_bytes)
            .transpose()
            .map_err(|_| EvidenceStoreErrorV1::InvalidEvidenceIndex)?;
        let receipt_object_id = optional_object_id(receipt_object_id)?;
        if receipt_id.is_some() != receipt_object_id.is_some() {
            return Err(EvidenceStoreErrorV1::InvalidEvidenceIndex);
        }
        erasures.push(EvidenceErasureIndexEntryV1 {
            intent_id: SecurityErasureIntentIdV1::from_bytes(exact_digest(intent_id)?)
                .map_err(|_| EvidenceStoreErrorV1::InvalidEvidenceIndex)?,
            intent_object_id: StoreObjectIdV1::from_digest(exact_digest(intent_object_id)?),
            receipt_id,
            receipt_object_id,
        });
    }
    if erasures.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(EvidenceStoreErrorV1::InvalidEvidenceIndex);
    }
    let mut references = observations
        .iter()
        .flat_map(|entry| {
            std::iter::once(entry.observation_object_id).chain(entry.payload_object_id)
        })
        .chain(assessments.iter().map(|entry| entry.assessment_object_id))
        .chain(
            invalidations
                .iter()
                .map(|entry| entry.invalidation_object_id),
        )
        .chain(erasures.iter().flat_map(|entry| {
            std::iter::once(entry.intent_object_id).chain(entry.receipt_object_id)
        }))
        .chain(previous_index_object_id)
        .collect::<Vec<_>>();
    references.sort_unstable();
    references.dedup();
    if object.references() != references {
        return Err(EvidenceStoreErrorV1::InvalidEvidenceIndex);
    }
    Ok(Some(ActiveEvidenceIndexV1 {
        object: (*object).clone(),
        previous_index_object_id,
        observations,
        assessments,
        invalidations,
        erasures,
    }))
}

fn build_next_observation_index(
    current: Option<&ActiveEvidenceIndexV1>,
    observation: &ObservationV1,
    observation_object_id: StoreObjectIdV1,
    payload_object_id: StoreObjectIdV1,
) -> Result<StoreObjectV1, EvidenceStoreErrorV1> {
    let mut entries = current
        .map(|index| index.observations.clone())
        .unwrap_or_default();
    let candidate = EvidenceObservationIndexEntryV1 {
        observation_id: observation.id(),
        observation_object_id,
        payload_object_id: Some(payload_object_id),
        acquisition_id: observation.acquisition_id().copied(),
    };
    if entries.iter().any(|entry| {
        entry.observation_id == candidate.observation_id
            || entry.observation_object_id == candidate.observation_object_id
            || entry
                .acquisition_id
                .zip(candidate.acquisition_id)
                .is_some_and(|(left, right)| left == right)
    }) {
        return Err(EvidenceStoreErrorV1::DuplicateObservationOrAcquisition);
    }
    for source in observation.lineage() {
        if !entries.iter().any(|entry| entry.observation_id == *source) {
            return Err(EvidenceStoreErrorV1::UnresolvedObservationLineage);
        }
    }
    entries.push(candidate);
    entries.sort_unstable();
    build_evidence_index(
        current.map(|index| index.object.id()),
        entries,
        current
            .map(|index| index.assessments.clone())
            .unwrap_or_default(),
        current
            .map(|index| index.invalidations.clone())
            .unwrap_or_default(),
        current
            .map(|index| index.erasures.clone())
            .unwrap_or_default(),
    )
}

fn build_evidence_index(
    previous_index_object_id: Option<StoreObjectIdV1>,
    observations: Vec<EvidenceObservationIndexEntryV1>,
    assessments: Vec<EvidenceAssessmentIndexEntryV1>,
    invalidations: Vec<EvidenceInvalidationIndexEntryV1>,
    erasures: Vec<EvidenceErasureIndexEntryV1>,
) -> Result<StoreObjectV1, EvidenceStoreErrorV1> {
    let mut references = observations
        .iter()
        .flat_map(|entry| {
            std::iter::once(entry.observation_object_id).chain(entry.payload_object_id)
        })
        .chain(assessments.iter().map(|entry| entry.assessment_object_id))
        .chain(
            invalidations
                .iter()
                .map(|entry| entry.invalidation_object_id),
        )
        .chain(erasures.iter().flat_map(|entry| {
            std::iter::once(entry.intent_object_id).chain(entry.receipt_object_id)
        }))
        .chain(previous_index_object_id)
        .collect::<Vec<_>>();
    references.sort_unstable();
    references.dedup();
    StoreObjectV1::new(
        evidence_schema_id(EVIDENCE_INDEX_SCHEMA_V1)?,
        CborValue::Array(vec![
            CborValue::text(EVIDENCE_INDEX_DOMAIN_V1)?,
            CborValue::optional(previous_index_object_id.map(|id| bytes(id.as_bytes()))),
            CborValue::Array(
                observations
                    .iter()
                    .map(|entry| {
                        CborValue::Array(vec![
                            bytes(entry.observation_id.as_bytes()),
                            bytes(entry.observation_object_id.as_bytes()),
                            CborValue::optional(
                                entry
                                    .payload_object_id
                                    .map(|payload| bytes(payload.as_bytes())),
                            ),
                            CborValue::optional(entry.acquisition_id.map(|id| bytes(&id))),
                        ])
                    })
                    .collect(),
            ),
            CborValue::Array(
                assessments
                    .iter()
                    .map(|entry| {
                        CborValue::Array(vec![
                            bytes(entry.assessment_id.as_bytes()),
                            bytes(entry.assessment_object_id.as_bytes()),
                        ])
                    })
                    .collect(),
            ),
            CborValue::Array(
                invalidations
                    .iter()
                    .map(|entry| {
                        CborValue::Array(vec![
                            bytes(entry.invalidation_id.as_bytes()),
                            bytes(entry.invalidation_object_id.as_bytes()),
                            bytes(entry.assessment_id.as_bytes()),
                        ])
                    })
                    .collect(),
            ),
            CborValue::Array(
                erasures
                    .iter()
                    .map(|entry| {
                        CborValue::Array(vec![
                            bytes(entry.intent_id.as_bytes()),
                            bytes(entry.intent_object_id.as_bytes()),
                            CborValue::optional(entry.receipt_id.map(|id| bytes(id.as_bytes()))),
                            CborValue::optional(
                                entry.receipt_object_id.map(|id| bytes(id.as_bytes())),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ]),
        references,
    )
    .map_err(Into::into)
}

fn build_next_assessment_index(
    current: &ActiveEvidenceIndexV1,
    assessment_id: AssessmentIdV1,
    assessment_object_id: StoreObjectIdV1,
) -> Result<StoreObjectV1, EvidenceStoreErrorV1> {
    let mut assessments = current.assessments.clone();
    if assessments.iter().any(|entry| {
        entry.assessment_id == assessment_id || entry.assessment_object_id == assessment_object_id
    }) {
        return Err(EvidenceStoreErrorV1::DuplicateAssessment);
    }
    assessments.push(EvidenceAssessmentIndexEntryV1 {
        assessment_id,
        assessment_object_id,
    });
    assessments.sort_unstable();
    build_evidence_index(
        Some(current.object.id()),
        current.observations.clone(),
        assessments,
        current.invalidations.clone(),
        current.erasures.clone(),
    )
}

fn build_next_invalidation_index(
    current: &ActiveEvidenceIndexV1,
    invalidation_id: AssessmentInvalidationIdV1,
    invalidation_object_id: StoreObjectIdV1,
    assessment_id: AssessmentIdV1,
) -> Result<StoreObjectV1, EvidenceStoreErrorV1> {
    let mut invalidations = current.invalidations.clone();
    if invalidations.iter().any(|entry| {
        entry.invalidation_id == invalidation_id
            || entry.invalidation_object_id == invalidation_object_id
    }) {
        return Err(EvidenceStoreErrorV1::DuplicateInvalidation);
    }
    invalidations.push(EvidenceInvalidationIndexEntryV1 {
        invalidation_id,
        invalidation_object_id,
        assessment_id,
    });
    invalidations.sort_unstable();
    build_evidence_index(
        Some(current.object.id()),
        current.observations.clone(),
        current.assessments.clone(),
        invalidations,
        current.erasures.clone(),
    )
}

fn validate_assessment_input_references(
    index: &ActiveEvidenceIndexV1,
    objects: &[StoreObjectV1],
    assessment: &AssessmentV1,
) -> Result<(), EvidenceStoreErrorV1> {
    let by_object = objects
        .iter()
        .map(|object| (object.id(), object))
        .collect::<std::collections::BTreeMap<_, _>>();
    let observation_schema = evidence_schema_id(EVIDENCE_OBSERVATION_SCHEMA_V1)?;
    let erasure_intent_schema = evidence_schema_id(EVIDENCE_ERASURE_INTENT_SCHEMA_V1)?;
    let tombstoned_payloads = index
        .erasures
        .iter()
        .map(|entry| {
            let object = by_object
                .get(&entry.intent_object_id)
                .filter(|object| object.schema_id() == erasure_intent_schema)
                .ok_or(EvidenceStoreErrorV1::InvalidEvidenceIndex)?;
            SecurityErasureIntentV1::from_canonical_value(object.value())
                .map(|intent| intent.payload_object_id())
                .map_err(EvidenceStoreErrorV1::from)
        })
        .collect::<Result<std::collections::BTreeSet<_>, _>>()?;
    let mut observations = std::collections::BTreeMap::new();
    for entry in &index.observations {
        let object = by_object
            .get(&entry.observation_object_id)
            .filter(|object| object.schema_id() == observation_schema)
            .ok_or(EvidenceStoreErrorV1::InvalidEvidenceIndex)?;
        let CborValue::Bytes(bytes) = object.value() else {
            return Err(EvidenceStoreErrorV1::InvalidEvidenceIndex);
        };
        let observation = ObservationV1::from_canonical_bytes(bytes)?;
        if observation.id() != entry.observation_id {
            return Err(EvidenceStoreErrorV1::InvalidEvidenceIndex);
        }
        match entry.payload_object_id {
            Some(payload) if payload == observation.payload().object_id() => {
                if observations.insert(observation.id(), observation).is_some() {
                    return Err(EvidenceStoreErrorV1::InvalidEvidenceIndex);
                }
            }
            None if tombstoned_payloads.contains(&observation.payload().object_id()) => {}
            _ => return Err(EvidenceStoreErrorV1::InvalidEvidenceIndex),
        }
    }

    let claim_schema = evidence_schema_id(EVIDENCE_CLAIM_SCHEMA_V1)?;
    let mut claims = std::collections::BTreeMap::new();
    for object in objects
        .iter()
        .filter(|object| object.schema_id() == claim_schema)
    {
        let claim = ClaimV1::from_canonical_value(object.value())?;
        if claims.insert(claim.claim_id(), claim).is_some() {
            return Err(EvidenceStoreErrorV1::InvalidEvidenceIndex);
        }
    }

    let current_assessments = index
        .assessments
        .iter()
        .map(|entry| entry.assessment_id)
        .collect::<std::collections::BTreeSet<_>>();
    for input in assessment.inputs() {
        match input {
            AssessmentInputRefV1::Observation(expected) => {
                let current = observations
                    .get(&expected.observation_id())
                    .ok_or(EvidenceStoreErrorV1::ObservationNotCurrent)?;
                if &ObservationAssessmentInputV1::from_observation(current)? != expected {
                    return Err(EvidenceStoreErrorV1::ObservationNotCurrent);
                }
            }
            AssessmentInputRefV1::Claim(expected) => {
                let claim = claims
                    .get(&expected.claim_id())
                    .ok_or(EvidenceStoreErrorV1::AssessmentInputNotCurrent)?;
                let resolved = claim
                    .observation_refs()
                    .iter()
                    .map(|id| {
                        observations
                            .get(id)
                            .ok_or(EvidenceStoreErrorV1::ObservationNotCurrent)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                if &ClaimAssessmentInputV1::from_claim(claim, &resolved)? != expected {
                    return Err(EvidenceStoreErrorV1::AssessmentInputNotCurrent);
                }
            }
            AssessmentInputRefV1::ChildResolution(expected) => {
                if expected
                    .assessment_ids()
                    .iter()
                    .any(|id| !current_assessments.contains(id))
                {
                    return Err(EvidenceStoreErrorV1::AssessmentInputNotCurrent);
                }
            }
            AssessmentInputRefV1::AuthorizationReceipt(expected) => {
                let receipt = expected.exact_receipt()?;
                if !current_authorization_receipt_is_persisted(objects, &receipt)? {
                    return Err(EvidenceStoreErrorV1::AssessmentInputNotCurrent);
                }
            }
        }
    }
    Ok(())
}

fn evidence_gate_snapshot_object(
    gate_snapshot: &GateSnapshotV1,
) -> Result<StoreObjectV1, EvidenceStoreErrorV1> {
    Ok(StoreObjectV1::new(
        evidence_schema_id(EVIDENCE_GATE_SNAPSHOT_SCHEMA_V1)?,
        CborValue::Bytes(gate_snapshot.canonical_bytes()?),
        vec![],
    )?)
}

fn evidence_assessment_object(
    assessment: &AssessmentV1,
    gate_snapshot_object_id: StoreObjectIdV1,
) -> Result<StoreObjectV1, EvidenceStoreErrorV1> {
    Ok(StoreObjectV1::new(
        evidence_schema_id(EVIDENCE_ASSESSMENT_SCHEMA_V1)?,
        CborValue::Bytes(assessment.canonical_bytes()?),
        vec![gate_snapshot_object_id],
    )?)
}

fn evidence_invalidation_object(
    invalidation: &AssessmentInvalidationV1,
    assessment_object_id: StoreObjectIdV1,
) -> Result<StoreObjectV1, EvidenceStoreErrorV1> {
    Ok(StoreObjectV1::new(
        evidence_schema_id(EVIDENCE_INVALIDATION_SCHEMA_V1)?,
        CborValue::Bytes(invalidation.canonical_bytes()?),
        vec![assessment_object_id],
    )?)
}

fn decode_result_invalidation(
    store: &StoreV1,
    result: &StoreObjectV1,
) -> Result<AssessmentInvalidationV1, EvidenceStoreErrorV1> {
    let schema = evidence_schema_id(EVIDENCE_INVALIDATION_SCHEMA_V1)?;
    let mut records = result
        .references()
        .iter()
        .map(|id| store.read_object(*id))
        .filter_map(|result| match result {
            Ok(object) if object.schema_id() == schema => Some(Ok(object)),
            Ok(_) => None,
            Err(error) => Some(Err(error)),
        })
        .collect::<Result<Vec<_>, _>>()?;
    if records.len() != 1 {
        return Err(EvidenceStoreErrorV1::InvalidPublicationResult);
    }
    let CborValue::Bytes(value) = records
        .pop()
        .expect("invariant: exact one invalidation result")
        .value()
        .clone()
    else {
        return Err(EvidenceStoreErrorV1::InvalidPublicationResult);
    };
    Ok(AssessmentInvalidationV1::from_canonical_bytes(&value)?)
}

fn build_next_erasure_begin_index(
    current: &ActiveEvidenceIndexV1,
    erasure: &SecurityErasurePublicationV1,
) -> Result<(StoreObjectV1, Vec<StoreObjectV1>, StoreObjectV1), EvidenceStoreErrorV1> {
    if current
        .erasures
        .iter()
        .any(|entry| entry.intent_id == erasure.intent().id())
    {
        return Err(EvidenceStoreErrorV1::DuplicateErasure);
    }
    let mut observations = current.observations.clone();
    let mut removed = 0_usize;
    for entry in &mut observations {
        if entry.payload_object_id == Some(erasure.intent().payload_object_id()) {
            entry.payload_object_id = None;
            removed += 1;
        }
    }
    if removed == 0 {
        return Err(EvidenceStoreErrorV1::ObservationNotCurrent);
    }
    let mut invalidations = current.invalidations.clone();
    let mut invalidation_objects = Vec::with_capacity(erasure.invalidations().len());
    for invalidation in erasure.invalidations() {
        if invalidations
            .iter()
            .any(|entry| entry.invalidation_id == invalidation.id())
        {
            return Err(EvidenceStoreErrorV1::DuplicateInvalidation);
        }
        let assessment_object_id = current
            .assessments
            .iter()
            .find(|entry| entry.assessment_id == invalidation.assessment_id())
            .map(|entry| entry.assessment_object_id)
            .ok_or(EvidenceStoreErrorV1::UnknownAssessment)?;
        let object = evidence_invalidation_object(invalidation, assessment_object_id)?;
        invalidations.push(EvidenceInvalidationIndexEntryV1 {
            invalidation_id: invalidation.id(),
            invalidation_object_id: object.id(),
            assessment_id: invalidation.assessment_id(),
        });
        invalidation_objects.push(object);
    }
    invalidations.sort_unstable();
    let intent_object = evidence_erasure_intent_object(
        erasure.intent(),
        invalidation_objects.iter().map(StoreObjectV1::id).collect(),
    )?;
    let mut erasures = current.erasures.clone();
    erasures.push(EvidenceErasureIndexEntryV1 {
        intent_id: erasure.intent().id(),
        intent_object_id: intent_object.id(),
        receipt_id: None,
        receipt_object_id: None,
    });
    erasures.sort_unstable();
    let next_index = build_evidence_index(
        None,
        observations,
        current.assessments.clone(),
        invalidations,
        erasures,
    )?;
    Ok((intent_object, invalidation_objects, next_index))
}

fn evidence_erasure_intent_object(
    intent: &super::SecurityErasureIntentV1,
    references: Vec<StoreObjectIdV1>,
) -> Result<StoreObjectV1, EvidenceStoreErrorV1> {
    Ok(StoreObjectV1::new(
        evidence_schema_id(EVIDENCE_ERASURE_INTENT_SCHEMA_V1)?,
        intent.canonical_value()?,
        references,
    )?)
}

fn evidence_erasure_receipt_object(
    receipt: &SecurityErasureReceiptV1,
    intent_object_id: StoreObjectIdV1,
) -> Result<StoreObjectV1, EvidenceStoreErrorV1> {
    Ok(StoreObjectV1::new(
        evidence_schema_id(EVIDENCE_ERASURE_RECEIPT_SCHEMA_V1)?,
        receipt.canonical_value(),
        vec![intent_object_id],
    )?)
}

fn load_erasure_from_index(
    objects: &[StoreObjectV1],
    index: &ActiveEvidenceIndexV1,
    intent_id: SecurityErasureIntentIdV1,
) -> Result<
    (
        super::SecurityErasureIntentV1,
        Option<SecurityErasureReceiptV1>,
        StoreObjectIdV1,
    ),
    EvidenceStoreErrorV1,
> {
    let entry = index
        .erasures
        .iter()
        .find(|entry| entry.intent_id == intent_id)
        .ok_or(EvidenceStoreErrorV1::ErasureRecoveryMismatch)?;
    let by_id = objects
        .iter()
        .map(|object| (object.id(), object))
        .collect::<std::collections::BTreeMap<_, _>>();
    let intent_object = by_id
        .get(&entry.intent_object_id)
        .filter(|object| {
            object.schema_id()
                == evidence_schema_id(EVIDENCE_ERASURE_INTENT_SCHEMA_V1)
                    .expect("invariant: static Evidence schema domain")
        })
        .ok_or(EvidenceStoreErrorV1::ErasureRecoveryMismatch)?;
    let intent = super::SecurityErasureIntentV1::from_canonical_value(intent_object.value())?;
    if intent.id() != intent_id {
        return Err(EvidenceStoreErrorV1::ErasureRecoveryMismatch);
    }
    let receipt = match (entry.receipt_id, entry.receipt_object_id) {
        (None, None) => None,
        (Some(receipt_id), Some(object_id)) => {
            let object = by_id
                .get(&object_id)
                .filter(|object| {
                    object.schema_id()
                        == evidence_schema_id(EVIDENCE_ERASURE_RECEIPT_SCHEMA_V1)
                            .expect("invariant: static Evidence schema domain")
                })
                .ok_or(EvidenceStoreErrorV1::ErasureRecoveryMismatch)?;
            let receipt = SecurityErasureReceiptV1::from_canonical_value(object.value())?;
            if receipt.id() != receipt_id
                || receipt.intent_id() != intent.id()
                || receipt.payload_object_id() != intent.payload_object_id()
                || receipt.dependency_closure_hash() != intent.dependency_closure_hash()
                || receipt.affected_assessments() != intent.affected_assessments()
                || receipt.controlled_copy_plan_id() != intent.controlled_copy_plan().plan_id()
            {
                return Err(EvidenceStoreErrorV1::ErasureRecoveryMismatch);
            }
            Some(receipt)
        }
        _ => return Err(EvidenceStoreErrorV1::ErasureRecoveryMismatch),
    };
    Ok((intent, receipt, intent_object.id()))
}

fn load_current_erasure(
    store: &StoreV1,
    request_id: ActionRequestIdV1,
    payload_object_id: StoreObjectIdV1,
) -> Result<
    (
        StoreHeadV1,
        super::SecurityErasureIntentV1,
        Option<SecurityErasureReceiptV1>,
    ),
    EvidenceStoreErrorV1,
> {
    find_current_erasure(store, request_id, payload_object_id)?
        .ok_or(EvidenceStoreErrorV1::ErasureRecoveryMismatch)
}

fn find_current_erasure(
    store: &StoreV1,
    request_id: ActionRequestIdV1,
    payload_object_id: StoreObjectIdV1,
) -> Result<
    Option<(
        StoreHeadV1,
        super::SecurityErasureIntentV1,
        Option<SecurityErasureReceiptV1>,
    )>,
    EvidenceStoreErrorV1,
> {
    let (state, head, _, objects) = store.coherent_publication_snapshot()?;
    if state != StoreStateV1::Active {
        return Err(EvidenceStoreErrorV1::InactiveStore);
    }
    let index = load_optional_evidence_index(&objects)?
        .ok_or(EvidenceStoreErrorV1::MissingEvidenceIndex)?;
    let mut matches = index
        .erasures
        .iter()
        .map(|entry| load_erasure_from_index(&objects, &index, entry.intent_id))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|(intent, _, _)| {
            intent.action_request_id() == request_id
                && intent.payload_object_id() == payload_object_id
        });
    let Some((intent, receipt, _)) = matches.next() else {
        return Ok(None);
    };
    if matches.next().is_some() {
        return Err(EvidenceStoreErrorV1::ErasureRecoveryMismatch);
    }
    Ok(Some((head, intent, receipt)))
}

fn finalize_erasure_receipt(
    intent: &super::SecurityErasureIntentV1,
    absence: VerifiedCollectionAbsenceV1,
    controlled_copy_absence: VerifiedControlledCopyAbsenceV1,
) -> Result<SecurityErasureReceiptV1, EvidenceStoreErrorV1> {
    Ok(intent.finalize(SecurityErasureFinalizationV1 {
        tombstone_id: absence.tombstone_id(),
        collection_plan_id: absence.collection_plan_id(),
        destroyed_payload_hash: absence.destroyed_object_hash(),
        physical_absence_receipt_hash: absence.receipt_hash(),
        controlled_copy_plan_id: controlled_copy_absence.plan_id(),
        controlled_copy_absence_receipt_hash: controlled_copy_absence.receipt_hash(),
        finalized_at: intent.accepted_h_time(),
    })?)
}

fn build_final_erasure_index(
    current: &ActiveEvidenceIndexV1,
    intent_id: SecurityErasureIntentIdV1,
    receipt_id: SecurityErasureReceiptIdV1,
    receipt_object_id: StoreObjectIdV1,
) -> Result<StoreObjectV1, EvidenceStoreErrorV1> {
    let mut erasures = current.erasures.clone();
    let entry = erasures
        .iter_mut()
        .find(|entry| entry.intent_id == intent_id)
        .ok_or(EvidenceStoreErrorV1::ErasureRecoveryMismatch)?;
    if entry.receipt_id.is_some() || entry.receipt_object_id.is_some() {
        return Err(EvidenceStoreErrorV1::ErasureRecoveryMismatch);
    }
    entry.receipt_id = Some(receipt_id);
    entry.receipt_object_id = Some(receipt_object_id);
    erasures.sort_unstable();
    build_evidence_index(
        Some(current.object.id()),
        current.observations.clone(),
        current.assessments.clone(),
        current.invalidations.clone(),
        erasures,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "the Evidence transaction names each independently owned Store carrier"
)]
fn build_authorized_evidence_record_publication(
    domain: crate::domain::vnext::persistence::StoreDomainV1,
    current_head: &StoreHeadV1,
    current_generation: &StoreGenerationV1,
    active_objects: &[StoreObjectV1],
    current_index: &ActiveEvidenceIndexV1,
    admission: &AdmittedRepositoryActionV1,
    artifacts: &RepositoryAuthorityArtifactsV1,
    request: &CanonicalEvidenceActionRequestV1,
    request_object: StoreObjectV1,
    produced: Vec<StoreObjectV1>,
    next_index: StoreObjectV1,
    meaning_digest: [u8; 32],
    namespace: &'static str,
) -> Result<AtomicGenerationPublicationV1, EvidenceStoreErrorV1> {
    let mut roots = current_generation.roots().to_vec();
    replace_required_root(&mut roots, current_index.object.id(), next_index.id())?;
    replace_required_root(
        &mut roots,
        admission.current_snapshot_id(),
        admission.successor_snapshot().id(),
    )?;
    replace_root_if_present(
        &mut roots,
        admission.current_capacity_root_id(),
        admission.successor_capacity_root().id(),
    );
    roots.push(artifacts.result_object().id());
    roots.sort_unstable();
    roots.dedup();
    let generation = StoreGenerationV1::new(
        domain,
        current_generation
            .ordinal()
            .checked_add(1)
            .ok_or(EvidenceStoreErrorV1::GenerationOverflow)?,
        Some(current_generation.id()),
        current_generation.contract_root_id(),
        current_generation.compatibility().clone(),
        roots,
    )?;
    let idempotency = StoreIdempotencyV1::new(
        namespace,
        *request.idempotency_key_id().as_bytes(),
        meaning_digest,
        artifacts.result_object().id(),
    )?;
    let mut objects = active_objects.to_vec();
    objects.extend([
        request_object,
        admission.basis_object().clone(),
        admission.successor_snapshot().clone(),
        admission.successor_capacity_root().clone(),
        admission.capacity_debit().clone(),
        artifacts.receipt_object().clone(),
        artifacts.result_object().clone(),
    ]);
    objects.extend(produced);
    objects.extend(artifacts.leaf_authority_objects().iter().cloned());
    objects.sort_by_key(StoreObjectV1::id);
    objects.dedup_by_key(|object| object.id());
    let objects = exact_generation_objects(&generation, objects)?;
    Ok(AtomicGenerationPublicationV1::new_from_object_superset(
        generation,
        Some(current_head.id()),
        objects,
        idempotency,
    )?)
}

#[expect(
    clippy::too_many_arguments,
    reason = "the erasure begin transaction names every authority, Store, and recovery carrier"
)]
fn build_security_erasure_begin_publication(
    domain: crate::domain::vnext::persistence::StoreDomainV1,
    current_head: &StoreHeadV1,
    current_generation: &StoreGenerationV1,
    active_objects: &[StoreObjectV1],
    current_index: &ActiveEvidenceIndexV1,
    admission: &AdmittedRepositoryActionV1,
    artifacts: &RepositoryAuthorityArtifactsV1,
    request: &CanonicalEvidenceActionRequestV1,
    request_object: StoreObjectV1,
    produced: Vec<StoreObjectV1>,
    next_index: StoreObjectV1,
    payload_object_id: StoreObjectIdV1,
    meaning_digest: [u8; 32],
) -> Result<AtomicGenerationPublicationV1, EvidenceStoreErrorV1> {
    let mut roots = current_generation
        .roots()
        .iter()
        .copied()
        .filter_map(
            |root| match root_reaches_target(root, payload_object_id, active_objects) {
                Ok(true) => None,
                Ok(false) => Some(Ok(root)),
                Err(error) => Some(Err(error)),
            },
        )
        .collect::<Result<Vec<_>, _>>()?;
    roots.retain(|root| *root != current_index.object.id());
    roots.push(next_index.id());
    replace_required_root(
        &mut roots,
        admission.current_snapshot_id(),
        admission.successor_snapshot().id(),
    )?;
    replace_root_if_present(
        &mut roots,
        admission.current_capacity_root_id(),
        admission.successor_capacity_root().id(),
    );
    roots.push(artifacts.result_object().id());
    roots.sort_unstable();
    roots.dedup();
    let generation = StoreGenerationV1::new(
        domain,
        current_generation
            .ordinal()
            .checked_add(1)
            .ok_or(EvidenceStoreErrorV1::GenerationOverflow)?,
        Some(current_generation.id()),
        current_generation.contract_root_id(),
        current_generation.compatibility().clone(),
        roots,
    )?;
    let idempotency = StoreIdempotencyV1::new(
        SECURITY_ERASURE_BEGIN_NAMESPACE_V1,
        *request.idempotency_key_id().as_bytes(),
        meaning_digest,
        artifacts.result_object().id(),
    )?;
    let mut objects = active_objects.to_vec();
    objects.extend([
        request_object,
        admission.basis_object().clone(),
        admission.successor_snapshot().clone(),
        admission.successor_capacity_root().clone(),
        admission.capacity_debit().clone(),
        artifacts.receipt_object().clone(),
        artifacts.result_object().clone(),
    ]);
    objects.extend(produced);
    objects.extend(artifacts.leaf_authority_objects().iter().cloned());
    objects.sort_by_key(StoreObjectV1::id);
    objects.dedup_by_key(|object| object.id());
    let objects = exact_generation_objects(&generation, objects)?;
    Ok(AtomicGenerationPublicationV1::new_from_object_superset(
        generation,
        Some(current_head.id()),
        objects,
        idempotency,
    )?)
}

fn root_reaches_target(
    root: StoreObjectIdV1,
    target: StoreObjectIdV1,
    objects: &[StoreObjectV1],
) -> Result<bool, EvidenceStoreErrorV1> {
    let by_id = objects
        .iter()
        .map(|object| (object.id(), object))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut pending = vec![root];
    let mut visited = std::collections::BTreeSet::new();
    while let Some(candidate) = pending.pop() {
        if candidate == target {
            return Ok(true);
        }
        if !visited.insert(candidate) {
            continue;
        }
        let object = by_id
            .get(&candidate)
            .ok_or(EvidenceStoreErrorV1::InvalidEvidenceIndex)?;
        pending.extend(object.references());
    }
    Ok(false)
}

#[expect(
    clippy::too_many_arguments,
    reason = "the erasure final transaction names every immutable recovery carrier explicitly"
)]
fn build_security_erasure_final_publication(
    domain: crate::domain::vnext::persistence::StoreDomainV1,
    current_head: &StoreHeadV1,
    current_generation: &StoreGenerationV1,
    active_objects: &[StoreObjectV1],
    current_index: &ActiveEvidenceIndexV1,
    continuation: &ContinuedRepositoryActionV1,
    produced: Vec<StoreObjectV1>,
    next_index: StoreObjectV1,
    idempotency_key: IdempotencyKeyIdV1,
    meaning_digest: [u8; 32],
) -> Result<AtomicGenerationPublicationV1, EvidenceStoreErrorV1> {
    let mut roots = current_generation.roots().to_vec();
    replace_required_root(&mut roots, current_index.object.id(), next_index.id())?;
    replace_required_root(
        &mut roots,
        continuation.current_snapshot_id(),
        continuation.successor_snapshot().id(),
    )?;
    roots.sort_unstable();
    roots.dedup();
    let generation = StoreGenerationV1::new(
        domain,
        current_generation
            .ordinal()
            .checked_add(1)
            .ok_or(EvidenceStoreErrorV1::GenerationOverflow)?,
        Some(current_generation.id()),
        current_generation.contract_root_id(),
        current_generation.compatibility().clone(),
        roots,
    )?;
    let idempotency = StoreIdempotencyV1::new(
        SECURITY_ERASURE_FINALIZE_NAMESPACE_V1,
        *idempotency_key.as_bytes(),
        meaning_digest,
        produced
            .first()
            .ok_or(EvidenceStoreErrorV1::ErasureRecoveryMismatch)?
            .id(),
    )?;
    let mut objects = active_objects.to_vec();
    objects.extend(produced);
    objects.push(continuation.successor_snapshot().clone());
    objects.sort_by_key(StoreObjectV1::id);
    objects.dedup_by_key(|object| object.id());
    let objects = exact_generation_objects(&generation, objects)?;
    Ok(AtomicGenerationPublicationV1::new_from_object_superset(
        generation,
        Some(current_head.id()),
        objects,
        idempotency,
    )?)
}

#[expect(
    clippy::too_many_arguments,
    reason = "the Evidence transaction names each independently owned Store carrier"
)]
fn build_authorized_evidence_publication(
    domain: crate::domain::vnext::persistence::StoreDomainV1,
    current_head: &StoreHeadV1,
    current_generation: &StoreGenerationV1,
    active_objects: &[StoreObjectV1],
    current_index: Option<&ActiveEvidenceIndexV1>,
    admission: &AdmittedRepositoryActionV1,
    artifacts: &RepositoryAuthorityArtifactsV1,
    request: &CanonicalEvidenceActionRequestV1,
    request_object: StoreObjectV1,
    observation_object: StoreObjectV1,
    payload_object: StoreObjectV1,
    next_index: StoreObjectV1,
    meaning_digest: [u8; 32],
) -> Result<AtomicGenerationPublicationV1, EvidenceStoreErrorV1> {
    let mut roots = current_generation.roots().to_vec();
    if let Some(index) = current_index {
        replace_required_root(&mut roots, index.object.id(), next_index.id())?;
    } else {
        roots.push(next_index.id());
    }
    replace_required_root(
        &mut roots,
        admission.current_snapshot_id(),
        admission.successor_snapshot().id(),
    )?;
    replace_root_if_present(
        &mut roots,
        admission.current_capacity_root_id(),
        admission.successor_capacity_root().id(),
    );
    roots.push(artifacts.result_object().id());
    roots.sort_unstable();
    roots.dedup();
    let generation = StoreGenerationV1::new(
        domain,
        current_generation
            .ordinal()
            .checked_add(1)
            .ok_or(EvidenceStoreErrorV1::GenerationOverflow)?,
        Some(current_generation.id()),
        current_generation.contract_root_id(),
        current_generation.compatibility().clone(),
        roots,
    )?;
    let idempotency = StoreIdempotencyV1::new(
        OBSERVATION_PUBLICATION_NAMESPACE_V1,
        *request.idempotency_key_id().as_bytes(),
        meaning_digest,
        artifacts.result_object().id(),
    )?;
    let mut objects = active_objects.to_vec();
    objects.extend([
        request_object,
        admission.basis_object().clone(),
        admission.successor_snapshot().clone(),
        admission.successor_capacity_root().clone(),
        admission.capacity_debit().clone(),
        artifacts.receipt_object().clone(),
        artifacts.result_object().clone(),
        observation_object,
        payload_object,
        next_index,
    ]);
    objects.extend(artifacts.leaf_authority_objects().iter().cloned());
    objects.sort_by_key(StoreObjectV1::id);
    objects.dedup_by_key(|object| object.id());
    let objects = exact_generation_objects(&generation, objects)?;
    Ok(AtomicGenerationPublicationV1::new_from_object_superset(
        generation,
        Some(current_head.id()),
        objects,
        idempotency,
    )?)
}

fn exact_generation_objects(
    generation: &StoreGenerationV1,
    objects: Vec<StoreObjectV1>,
) -> Result<Vec<StoreObjectV1>, EvidenceStoreErrorV1> {
    let by_id = objects
        .into_iter()
        .map(|object| (object.id(), object))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut pending = generation.roots().to_vec();
    let mut reachable = std::collections::BTreeSet::new();
    while let Some(object_id) = pending.pop() {
        if !reachable.insert(object_id) {
            continue;
        }
        let object = by_id
            .get(&object_id)
            .ok_or(EvidenceStoreErrorV1::InvalidEvidenceIndex)?;
        pending.extend(object.references());
    }
    Ok(reachable
        .into_iter()
        .map(|object_id| {
            by_id
                .get(&object_id)
                .expect("invariant: exact Generation closure was resolved")
                .clone()
        })
        .collect())
}

fn validate_payload_object(
    observation: &ObservationV1,
    payload: &StoreObjectV1,
) -> Result<(), EvidenceStoreErrorV1> {
    let CborValue::Bytes(payload_bytes) = payload.value() else {
        return Err(EvidenceStoreErrorV1::InvalidPayloadObject);
    };
    let typed_payload = ObservationPayloadV1::from_canonical_bytes(payload_bytes)?;
    if payload.id() != observation.payload().object_id()
        || payload.schema_id() != observation.payload().schema_id()
        || !payload.references().is_empty()
        || payload_bytes.len() as u64 != observation.payload().byte_length()
        || typed_payload.semantic_hash() != observation.payload().semantic_hash()
        || !typed_payload.matches_observation(observation)?
        || !observation
            .payload()
            .validates_exact_secret_scan(&typed_payload)?
    {
        return Err(EvidenceStoreErrorV1::InvalidPayloadObject);
    }
    Ok(())
}

fn evidence_observation_object(
    observation: &ObservationV1,
) -> Result<StoreObjectV1, EvidenceStoreErrorV1> {
    Ok(StoreObjectV1::new(
        evidence_schema_id(EVIDENCE_OBSERVATION_SCHEMA_V1)?,
        CborValue::Bytes(observation.canonical_bytes()?),
        vec![],
    )?)
}

fn evidence_action_request_object(
    request: &CanonicalEvidenceActionRequestV1,
) -> Result<StoreObjectV1, EvidenceStoreErrorV1> {
    Ok(StoreObjectV1::new(
        evidence_schema_id(EVIDENCE_ACTION_REQUEST_SCHEMA_V1)?,
        request.canonical_value()?,
        vec![],
    )?)
}

fn validate_observation_result(
    store: &StoreV1,
    result: &StoreObjectV1,
    expected: &ObservationV1,
    payload: &StoreObjectV1,
) -> Result<(), EvidenceStoreErrorV1> {
    let observation = evidence_observation_object(expected)?;
    let index_schema = evidence_schema_id(EVIDENCE_INDEX_SCHEMA_V1)?;
    let CborValue::Array(fields) = result.value() else {
        return Err(EvidenceStoreErrorV1::InvalidPublicationResult);
    };
    let Some(CborValue::Array(produced)) = fields.get(7) else {
        return Err(EvidenceStoreErrorV1::InvalidPublicationResult);
    };
    let produced = produced
        .iter()
        .map(|value| exact_digest(value).map(StoreObjectIdV1::from_digest))
        .collect::<Result<Vec<_>, _>>()?;
    if produced.len() != 3
        || !produced.contains(&observation.id())
        || !produced.contains(&payload.id())
        || !result.references().contains(&observation.id())
        || result.references().contains(&payload.id())
    {
        return Err(EvidenceStoreErrorV1::InvalidPublicationResult);
    }
    let index_ids = produced
        .iter()
        .filter_map(|id| store.read_object(*id).ok())
        .filter(|object| object.schema_id() == index_schema)
        .map(|object| object.id())
        .collect::<Vec<_>>();
    if index_ids.len() != 1 || result.references().contains(&index_ids[0]) {
        return Err(EvidenceStoreErrorV1::InvalidPublicationResult);
    }
    Ok(())
}

fn observation_action(
    observation: &ObservationV1,
) -> Result<RepositoryActionLeafV1, EvidenceStoreErrorV1> {
    Ok(
        match observation.publication_route().producer_action_tag() {
            39 => RepositoryActionLeafV1::PublishObservation,
            43 => RepositoryActionLeafV1::PublishBootstrapMandatePresentationObservation,
            44 => RepositoryActionLeafV1::PublishBootstrapMandateResponseObservation,
            45 => RepositoryActionLeafV1::PublishContinuityMaintenanceObservation,
            _ => return Err(EvidenceStoreErrorV1::PublicationBindingMismatch),
        },
    )
}

fn observation_subject_value(observation: &ObservationV1) -> Result<CborValue, ObservationError> {
    Ok(CborValue::Array(vec![
        CborValue::text("maestro.vnext.evidence-observation-subject.v1")?,
        CborValue::Unsigned(observation.kind().tag()),
        bytes(observation.id().as_bytes()),
        CborValue::Array(
            observation
                .subjects()
                .iter()
                .map(|subject| subject.canonical_value())
                .collect(),
        ),
    ]))
}

fn observation_publication_payload_value(
    observation: &ObservationV1,
    payload: &StoreObjectV1,
) -> Result<CborValue, ObservationError> {
    Ok(CborValue::Array(vec![
        CborValue::text("maestro.vnext.evidence-observation-publication-payload.v1")?,
        observation.canonical_value()?,
        CborValue::Bytes(payload.canonical_bytes().to_vec()),
    ]))
}

fn evidence_schema_id(domain: &str) -> Result<SchemaIdV1, IdentityError> {
    derive_identity(&CborValue::Text(domain.to_owned()))
}

fn replace_required_root(
    roots: &mut [StoreObjectIdV1],
    old: StoreObjectIdV1,
    new: StoreObjectIdV1,
) -> Result<(), EvidenceStoreErrorV1> {
    let Some(root) = roots.iter_mut().find(|root| **root == old) else {
        return Err(EvidenceStoreErrorV1::StaleExpectedStoreState);
    };
    *root = new;
    Ok(())
}

fn replace_root_if_present(
    roots: &mut [StoreObjectIdV1],
    old: StoreObjectIdV1,
    new: StoreObjectIdV1,
) {
    if let Some(root) = roots.iter_mut().find(|root| **root == old) {
        *root = new;
    }
}

fn exact_digest(value: &CborValue) -> Result<[u8; 32], EvidenceStoreErrorV1> {
    let CborValue::Bytes(bytes) = value else {
        return Err(EvidenceStoreErrorV1::InvalidEvidenceIndex);
    };
    bytes
        .as_slice()
        .try_into()
        .map_err(|_| EvidenceStoreErrorV1::InvalidEvidenceIndex)
}

fn optional_digest(value: &CborValue) -> Result<Option<[u8; 32]>, EvidenceStoreErrorV1> {
    let CborValue::Array(fields) = value else {
        return Err(EvidenceStoreErrorV1::InvalidEvidenceIndex);
    };
    match fields.as_slice() {
        [CborValue::Unsigned(0)] => Ok(None),
        [CborValue::Unsigned(1), value] => Ok(Some(exact_digest(value)?)),
        _ => Err(EvidenceStoreErrorV1::InvalidEvidenceIndex),
    }
}

fn optional_object_id(value: &CborValue) -> Result<Option<StoreObjectIdV1>, EvidenceStoreErrorV1> {
    optional_digest(value).map(|value| value.map(StoreObjectIdV1::from_digest))
}

fn hash(value: &CborValue) -> Result<[u8; 32], CborError> {
    Ok(Sha256::digest(deterministic_cbor::encode(value)?).into())
}

fn bytes(value: &[u8]) -> CborValue {
    CborValue::Bytes(value.to_vec())
}

fn map_store_error(error: StoreError) -> EvidenceStoreErrorV1 {
    match error {
        StoreError::HeadCasMismatch => EvidenceStoreErrorV1::StaleExpectedStoreState,
        error => EvidenceStoreErrorV1::Store(error),
    }
}

#[derive(Debug, Error)]
pub enum EvidenceStoreErrorV1 {
    #[error("Evidence Store is not active")]
    InactiveStore,
    #[error("Evidence publication expected-old Store state is stale")]
    StaleExpectedStoreState,
    #[error("Evidence request does not bind the exact subject, current state, and payload")]
    PublicationBindingMismatch,
    #[error("Evidence mutation time is not the admitted current Authority H_time")]
    UntrustedMutationTime,
    #[error("Evidence payload Object does not match its typed Observation manifest")]
    InvalidPayloadObject,
    #[error("active Store contains more than one Evidence index")]
    AmbiguousEvidenceIndex,
    #[error("active Store has no Evidence index")]
    MissingEvidenceIndex,
    #[error("active Evidence index is malformed or has invalid references")]
    InvalidEvidenceIndex,
    #[error("Observation identity or acquisition identity already exists")]
    DuplicateObservationOrAcquisition,
    #[error("Assessment identity or exact carrier already exists")]
    DuplicateAssessment,
    #[error("Assessment already has a current invalidation")]
    DuplicateInvalidation,
    #[error("Evidence Action names an unknown Assessment")]
    UnknownAssessment,
    #[error("Assessment invalidation draft is malformed")]
    InvalidInvalidationDraft,
    #[error("persisted Assessment invalidation no longer joins one exact authorized Action")]
    InvalidInvalidationAuthority,
    #[error("Evidence Action does not bind the complete current Evidence cut")]
    StaleEvidenceCut,
    #[error("Observation is absent, erased, or not exact in the complete current Evidence index")]
    ObservationNotCurrent,
    #[error("Assessment input is absent or not exact in the complete current Store generation")]
    AssessmentInputNotCurrent,
    #[error(
        "Work completion Evidence does not bind the exact current Store, Work, Contract, and Submission"
    )]
    WorkCompletionEvidenceMismatch,
    #[error("one or more required Work completion Gates lack one exact applicable Pass resolution")]
    WorkCompletionGateNotPassed,
    #[error("derived Observation lineage is absent from the complete current Evidence index")]
    UnresolvedObservationLineage,
    #[error("run-mediated Observation does not bind one exact current persisted Run owner")]
    RunProvenanceNotCurrent,
    #[error("Evidence publication result is malformed")]
    InvalidPublicationResult,
    #[error("Evidence Store Generation ordinal overflowed")]
    GenerationOverflow,
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    AtomicPublication(#[from] AtomicPublicationError),
    #[error(transparent)]
    Generation(#[from] GenerationError),
    #[error(transparent)]
    Object(#[from] StoreObjectError),
    #[error(transparent)]
    Retention(#[from] RetentionError),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(transparent)]
    Observation(#[from] ObservationError),
    #[error(transparent)]
    Assessment(#[from] AssessmentError),
    #[error(transparent)]
    Gate(#[from] GateError),
    #[error(transparent)]
    Claim(#[from] super::ClaimError),
    #[error(transparent)]
    SecurityErasure(#[from] SecurityErasureError),
    #[error("security-erasure recovery state does not match its durable Intent")]
    ErasureRecoveryMismatch,
    #[error("security-erasure Intent already exists")]
    DuplicateErasure,
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error("Evidence Action failed current Repository Authority admission")]
    AuthorityAdmissionFailed,
}

impl From<RepositoryAuthorityAdmissionErrorV1> for EvidenceStoreErrorV1 {
    fn from(_: RepositoryAuthorityAdmissionErrorV1) -> Self {
        Self::AuthorityAdmissionFailed
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use rusqlite::Connection;

    use super::*;
    use crate::domain::vnext::authority::{
        GenericExecutionAuthorityV1, PrincipalIdV1, SessionIdV1,
        test_support::{AuthorityFixtureModeV1, repository_authority_fixture_at},
    };
    use crate::domain::vnext::contract::runtime::ContractGenerationIdV1;
    use crate::domain::vnext::evidence::identity;
    use crate::domain::vnext::gate::{
        GateEvaluationResultV1, GateEvaluatorContractV1, GateInputClassV1, GateLeafRuleV1,
        GateNodeV1, GateOperatorV1, GateScopeV1, GateSnapshotV1,
    };
    use crate::domain::vnext::identity::ContractRootIdV1;
    use crate::domain::vnext::persistence::{StoreCompatibilityV1, StoreDomainV1, StoreRoleV1};
    use crate::domain::vnext::work::{WorkIdV1, WorkSubmissionIdV1};
    use crate::domain::vnext::{
        authority::TrustedTimeV1,
        evidence::{
            AssessmentApplicabilityV1, AssessmentBasisV1, AssessmentInputRefV1, AssessmentScopeV1,
            AssessmentTimeBasisV1, ClaimSubjectV1, ClosedLeafGateEvaluatorV1,
            EvidencePayloadManifestV1, EvidenceRedactionPolicyV1, EvidenceRetentionClassV1,
            EvidenceRetentionPolicyV1, EvidenceSecretScanReceiptV1, NominalObservationPayloadV1,
            ObservationAcquisitionV1, ObservationAssessmentInputV1, ObservationDraftV1,
            ObservationKindV1, ObservationPayloadCommonV1, ObservationPayloadDetailV1,
            ObservationPayloadFieldTypeV1, ObservationPayloadFieldV1, ObservationPayloadV1,
            ObservationPublicationRouteV1, ObservationSubjectKindV1, ObservationSubjectV1,
            SubmissionRefV1, resolve_gate_assessments,
        },
        execution::{
            DispatchAttemptV1, EffectIntentIdV1, ExecutionAttemptV1, RunReservationV1, RunSetV1,
        },
    };

    fn digest(byte: u8) -> [u8; 32] {
        Sha256::digest([byte]).into()
    }

    fn rendered(byte: u8) -> String {
        format!("sha256:{}", format!("{byte:02x}").repeat(32))
    }

    fn invalidation_mutant(
        invalidation: &AssessmentInvalidationV1,
        mutate: impl FnOnce(&mut Vec<CborValue>),
    ) -> AssessmentInvalidationV1 {
        let mut value = deterministic_cbor::decode(&invalidation.canonical_bytes().unwrap())
            .expect("test fixture");
        let CborValue::Array(fields) = &mut value else {
            unreachable!("Assessment invalidation is a record array");
        };
        mutate(fields);
        fields[0] = CborValue::Bytes(
            identity::domain_hash(
                "maestro.vnext.evidence.assessment-invalidation.v1",
                &CborValue::Array(fields[1..].to_vec()),
            )
            .expect("test fixture")
            .to_vec(),
        );
        AssessmentInvalidationV1::from_canonical_bytes(
            &deterministic_cbor::encode(&value).expect("test fixture"),
        )
        .expect("test fixture")
    }

    fn nominal_fields(kind: ObservationKindV1, seed: u8) -> Vec<ObservationPayloadFieldV1> {
        kind.contract()
            .unwrap()
            .payload_fields()
            .into_iter()
            .enumerate()
            .map(|(index, field)| match field.field_type() {
                ObservationPayloadFieldTypeV1::Digest => {
                    ObservationPayloadFieldV1::Digest(digest(seed.wrapping_add(index as u8)))
                }
                ObservationPayloadFieldTypeV1::Count => {
                    ObservationPayloadFieldV1::Count(index as u64 + 1)
                }
                ObservationPayloadFieldTypeV1::Timestamp => {
                    ObservationPayloadFieldV1::Timestamp(1_000 + index as u64)
                }
                ObservationPayloadFieldTypeV1::Tag => {
                    ObservationPayloadFieldV1::Tag(index as u64 + 1)
                }
                ObservationPayloadFieldTypeV1::Boolean => {
                    ObservationPayloadFieldV1::Boolean(index % 2 == 0)
                }
            })
            .collect()
    }

    fn actor_producer() -> crate::domain::vnext::authority::ExecutionProducerV1 {
        crate::domain::vnext::authority::ExecutionProducerV1::SessionBound {
            principal_id: PrincipalIdV1::derive("stage3-actor-principal").unwrap(),
            session_id: SessionIdV1::derive("stage3-actor-session").unwrap(),
        }
    }

    fn payload_manifest(
        kind: ObservationKindV1,
        object_id: StoreObjectIdV1,
        payload: &ObservationPayloadV1,
        recorded_at: u64,
        _seed: u8,
    ) -> EvidencePayloadManifestV1 {
        let redaction = EvidenceRedactionPolicyV1::prohibit_secrets_v1(1_048_576).unwrap();
        let scan = EvidenceSecretScanReceiptV1::scan(
            object_id,
            payload,
            redaction,
            actor_producer(),
            recorded_at,
        )
        .unwrap();
        let retention = EvidenceRetentionPolicyV1::new(
            EvidenceRetentionClassV1::ExplicitSecurityErasureEligible,
            recorded_at + 1_000,
        )
        .unwrap();
        EvidencePayloadManifestV1::new(
            kind,
            object_id,
            payload,
            "application/cbor",
            redaction,
            scan,
            retention,
        )
        .unwrap()
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "the test fixture binds the complete typed Observation provenance"
    )]
    fn typed_payload_object(
        kind: ObservationKindV1,
        subjects: &[ObservationSubjectV1],
        procedure_hash: [u8; 32],
        environment_hash: [u8; 32],
        toolchain_hash: [u8; 32],
        observed_at: u64,
        recorded_at: u64,
        clock_basis_hash: [u8; 32],
        seed: u8,
    ) -> (ObservationPayloadV1, StoreObjectV1) {
        let detail = if kind == ObservationKindV1::DeterministicProcedure {
            ObservationPayloadDetailV1::Deterministic {
                executable_bytes_hash: digest(seed.wrapping_add(1)),
                executable_version_hash: digest(seed.wrapping_add(2)),
                arguments_hash: digest(seed.wrapping_add(3)),
                working_directory_hash: digest(seed.wrapping_add(4)),
                relevant_environment_hash: digest(seed.wrapping_add(5)),
                subject_revision_hash: digest(seed.wrapping_add(6)),
                dirty_state_hash: digest(seed.wrapping_add(7)),
                exit_status_hash: digest(seed.wrapping_add(8)),
                stdout_hash: digest(seed.wrapping_add(9)),
                stderr_hash: digest(seed.wrapping_add(10)),
            }
        } else {
            ObservationPayloadDetailV1::Nominal(
                NominalObservationPayloadV1::new(kind, nominal_fields(kind, seed)).unwrap(),
            )
        };
        let payload = ObservationPayloadV1::new(
            kind,
            ObservationPayloadCommonV1::new(
                subjects,
                procedure_hash,
                environment_hash,
                toolchain_hash,
                observed_at,
                recorded_at,
                clock_basis_hash,
            )
            .unwrap(),
            detail,
        )
        .unwrap();
        let object = StoreObjectV1::new(
            kind.contract().unwrap().payload_schema_id(),
            CborValue::Bytes(payload.canonical_bytes().unwrap()),
            vec![],
        )
        .unwrap();
        (payload, object)
    }

    fn test_root() -> std::path::PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let path = std::env::temp_dir().join(format!(
            "maestro-stage5-evidence-store-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir(&path).unwrap();
        std::fs::canonicalize(path).unwrap()
    }

    fn put_objects_in_reference_order(store: &mut StoreV1, objects: Vec<StoreObjectV1>) {
        let mut pending = objects;
        let mut inserted = std::collections::BTreeSet::new();
        while !pending.is_empty() {
            let index = pending
                .iter()
                .position(|object| {
                    object
                        .references()
                        .iter()
                        .all(|reference| inserted.contains(reference))
                })
                .expect("fixture Store objects form a closed DAG");
            let object = pending.remove(index);
            store.put_object(&object).unwrap();
            inserted.insert(object.id());
        }
    }

    fn active_objects(store: &mut StoreV1) -> Vec<StoreObjectV1> {
        store
            .with_serialized_active_view(|view| {
                view.active_generation_objects()
                    .map_err(EvidenceStoreErrorV1::Store)
            })
            .unwrap()
    }

    fn authority_for(
        selection: crate::domain::vnext::authority::RepositoryAuthoritySelectionV1,
        request: &CanonicalEvidenceActionRequestV1,
        actor: PrincipalIdV1,
    ) -> GenericExecutionAuthorityV1 {
        GenericExecutionAuthorityV1::new(
            selection,
            request.action(),
            request.subject_commitment(),
            request.expected_state_commitment(),
            request.payload_commitment(),
            actor,
        )
        .unwrap()
    }

    #[test]
    fn authorized_store_cut_and_security_erasure_are_restart_safe() {
        let domain =
            StoreDomainV1::derive(StoreRoleV1::Repository, b"stage5-evidence-store").unwrap();
        let contract_root = ContractRootIdV1::parse(&rendered(11)).unwrap();
        let contract_generation_id = ContractGenerationIdV1::parse(&rendered(24)).unwrap();
        let work_id = WorkIdV1::derive("stage5-evidence-work").unwrap();
        let kind = ObservationKindV1::DeterministicProcedure;
        let observation_subjects = vec![
            ObservationSubjectV1::for_work(
                *work_id.as_bytes(),
                contract_generation_id,
                *contract_root.as_bytes(),
            )
            .unwrap(),
            ObservationSubjectV1::new(
                ObservationSubjectKindV1::Repository,
                *domain.id().as_bytes(),
                *contract_generation_id.as_bytes(),
            )
            .unwrap(),
        ];
        let (typed_payload, payload) = typed_payload_object(
            kind,
            &observation_subjects,
            digest(13),
            digest(14),
            digest(15),
            119,
            120,
            digest(16),
            60,
        );
        let observation = ObservationV1::new(ObservationDraftV1 {
            kind,
            store_domain_id: domain.id(),
            subjects: observation_subjects,
            producer: actor_producer(),
            procedure_hash: digest(13),
            environment_hash: digest(14),
            toolchain_hash: digest(15),
            observed_at: 119,
            recorded_at: 120,
            clock_basis_hash: digest(16),
            lineage: vec![],
            payload: payload_manifest(kind, payload.id(), &typed_payload, 120, 17),
            acquisition: ObservationAcquisitionV1::effect_free(digest(20), digest(21)).unwrap(),
            publication_route: ObservationPublicationRouteV1::new(kind, 39, None, None).unwrap(),
        })
        .unwrap();
        let raw_payload = StoreObjectV1::new(
            kind.contract().unwrap().payload_schema_id(),
            CborValue::Bytes(br#"{"forged":true}"#.to_vec()),
            vec![],
        )
        .unwrap();
        assert!(validate_payload_object(&observation, &raw_payload).is_err());
        let interleaving_subjects = vec![
            ObservationSubjectV1::for_work(
                *work_id.as_bytes(),
                contract_generation_id,
                *contract_root.as_bytes(),
            )
            .unwrap(),
            ObservationSubjectV1::new(
                ObservationSubjectKindV1::Repository,
                *domain.id().as_bytes(),
                *contract_generation_id.as_bytes(),
            )
            .unwrap(),
        ];
        let (typed_interleaving_payload, interleaving_payload) = typed_payload_object(
            kind,
            &interleaving_subjects,
            digest(33),
            digest(34),
            digest(35),
            119,
            120,
            digest(36),
            80,
        );
        let interleaving_observation = ObservationV1::new(ObservationDraftV1 {
            kind,
            store_domain_id: domain.id(),
            subjects: interleaving_subjects,
            producer: actor_producer(),
            procedure_hash: digest(33),
            environment_hash: digest(34),
            toolchain_hash: digest(35),
            observed_at: 119,
            recorded_at: 120,
            clock_basis_hash: digest(36),
            lineage: vec![],
            payload: payload_manifest(
                kind,
                interleaving_payload.id(),
                &typed_interleaving_payload,
                120,
                37,
            ),
            acquisition: ObservationAcquisitionV1::effect_free(digest(40), digest(41)).unwrap(),
            publication_route: ObservationPublicationRouteV1::new(kind, 39, None, None).unwrap(),
        })
        .unwrap();
        let dispatch_attempt = DispatchAttemptV1::new(
            EffectIntentIdV1::derive("stage5-unpersisted-run-intent").unwrap(),
            1,
            digest(42),
            None,
        )
        .unwrap();
        let execution_attempt = ExecutionAttemptV1::Dispatch(dispatch_attempt);
        let run_set = RunSetV1::reserve_non_step_run_at_revision(
            &execution_attempt,
            RunReservationV1 {
                semantic_operation_hash: digest(43),
                inputs_commitment: digest(44),
                environment_commitment: digest(45),
                target_commitment: digest(46),
                execution_boundary_commitment: digest(47),
                deadline: 130,
                launch_ordinal: 1,
                current_step_term: None,
            },
            1,
        )
        .unwrap();
        let unpersisted_run = &run_set.runs()[0];
        let run_kind = ObservationKindV1::ProcessLiveness;
        let run_subjects = vec![
            ObservationSubjectV1::new(
                ObservationSubjectKindV1::Run,
                *unpersisted_run.id().as_bytes(),
                digest(48),
            )
            .unwrap(),
        ];
        let (typed_run_payload, run_payload) = typed_payload_object(
            run_kind,
            &run_subjects,
            digest(49),
            digest(50),
            digest(51),
            119,
            120,
            digest(52),
            100,
        );
        let run_observation = ObservationV1::new(ObservationDraftV1 {
            kind: run_kind,
            store_domain_id: domain.id(),
            subjects: run_subjects,
            producer: actor_producer(),
            procedure_hash: digest(49),
            environment_hash: digest(50),
            toolchain_hash: digest(51),
            observed_at: 119,
            recorded_at: 120,
            clock_basis_hash: digest(52),
            lineage: vec![],
            payload: payload_manifest(run_kind, run_payload.id(), &typed_run_payload, 120, 53),
            acquisition: ObservationAcquisitionV1::run_mediated(digest(56), unpersisted_run)
                .unwrap(),
            publication_route: ObservationPublicationRouteV1::new(run_kind, 39, None, None)
                .unwrap(),
        })
        .unwrap();
        assert_eq!(
            ObservationV1::from_canonical_bytes(&observation.canonical_bytes().unwrap()).unwrap(),
            observation
        );
        let mut identity_mutant = observation.canonical_value().unwrap();
        let CborValue::Array(record) = &mut identity_mutant else {
            unreachable!("fixture Observation is a record array");
        };
        let CborValue::Bytes(id) = &mut record[1] else {
            unreachable!("fixture Observation has a byte identity");
        };
        id[0] ^= 1;
        assert!(matches!(
            ObservationV1::from_canonical_bytes(
                &deterministic_cbor::encode(&identity_mutant).unwrap()
            ),
            Err(ObservationError::InvalidStoredObservation)
        ));
        let gate = GateNodeV1::new(
            GateScopeV1::Work,
            GateInputClassV1::Evidence,
            GateOperatorV1::Leaf,
            GateEvaluatorContractV1::leaf(GateLeafRuleV1::EvidenceSetPresent, digest(22)).unwrap(),
            digest(23),
            None,
            vec![],
        )
        .unwrap();
        let missing_gate = GateNodeV1::new(
            GateScopeV1::Work,
            GateInputClassV1::Evidence,
            GateOperatorV1::Leaf,
            GateEvaluatorContractV1::leaf(GateLeafRuleV1::EvidenceSetPresent, digest(64)).unwrap(),
            digest(65),
            None,
            vec![],
        )
        .unwrap();
        let missing_composite = GateNodeV1::new(
            GateScopeV1::Work,
            GateInputClassV1::Composite,
            GateOperatorV1::All,
            GateEvaluatorContractV1::composite(GateOperatorV1::All, digest(66)).unwrap(),
            digest(67),
            None,
            vec![missing_gate.id()],
        )
        .unwrap();
        let gate_snapshot = GateSnapshotV1::new(
            work_id,
            contract_generation_id,
            contract_root,
            crate::domain::vnext::identity::ContractComponentIdV1::parse(&rendered(24)).unwrap(),
            digest(25),
            digest(26),
            vec![gate.id(), missing_composite.id()],
            vec![
                gate.clone(),
                missing_gate.clone(),
                missing_composite.clone(),
            ],
        )
        .unwrap();
        let evaluator = ClosedLeafGateEvaluatorV1::new(gate.evaluator().clone()).unwrap();
        let substitute_observation = ObservationV1::new(ObservationDraftV1 {
            kind,
            store_domain_id: domain.id(),
            subjects: observation.subjects().to_vec(),
            producer: actor_producer(),
            procedure_hash: digest(57),
            environment_hash: digest(58),
            toolchain_hash: digest(59),
            observed_at: 119,
            recorded_at: 120,
            clock_basis_hash: digest(60),
            lineage: vec![],
            payload: observation.payload().clone(),
            acquisition: ObservationAcquisitionV1::effect_free(digest(61), digest(62)).unwrap(),
            publication_route: ObservationPublicationRouteV1::new(kind, 39, None, None).unwrap(),
        })
        .unwrap();
        let re_root_observation = ObservationV1::new(ObservationDraftV1 {
            kind,
            store_domain_id: domain.id(),
            subjects: observation.subjects().to_vec(),
            producer: actor_producer(),
            procedure_hash: digest(13),
            environment_hash: digest(14),
            toolchain_hash: digest(15),
            observed_at: 119,
            recorded_at: 120,
            clock_basis_hash: digest(16),
            lineage: vec![],
            payload: observation.payload().clone(),
            acquisition: ObservationAcquisitionV1::effect_free(digest(62), digest(63)).unwrap(),
            publication_route: ObservationPublicationRouteV1::new(kind, 39, None, None).unwrap(),
        })
        .unwrap();
        let submission = SubmissionRefV1::for_work(
            WorkSubmissionIdV1::derive("stage5-unpublished-claim-submission").unwrap(),
        )
        .unwrap();
        let unpublished_claim = ClaimV1::new(
            submission,
            ClaimSubjectV1::for_work(work_id, contract_root, vec![]).unwrap(),
            digest(63),
            vec![observation.id()],
        )
        .unwrap();
        let observation_subject = hash(&observation_subject_value(&observation).unwrap()).unwrap();
        let interleaving_observation_subject =
            hash(&observation_subject_value(&interleaving_observation).unwrap()).unwrap();
        let run_observation_subject =
            hash(&observation_subject_value(&run_observation).unwrap()).unwrap();
        let re_root_observation_subject =
            hash(&observation_subject_value(&re_root_observation).unwrap()).unwrap();
        let assessment_subject = hash(
            &super::super::assessment::assessment_authorization_subject_value(
                domain.id(),
                gate_snapshot.id(),
                gate.id(),
                work_id,
                contract_generation_id,
                AssessmentScopeV1::Work,
            )
            .unwrap(),
        )
        .unwrap();
        let missing_composite_subject = hash(
            &super::super::assessment::assessment_authorization_subject_value(
                domain.id(),
                gate_snapshot.id(),
                missing_composite.id(),
                work_id,
                contract_generation_id,
                AssessmentScopeV1::Work,
            )
            .unwrap(),
        )
        .unwrap();
        let erasure_subject = hash(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.evidence-security-erasure-subject.v1").unwrap(),
            bytes(payload.id().as_bytes()),
        ]))
        .unwrap();
        let fixture = repository_authority_fixture_at(
            vec![
                (
                    RepositoryActionLeafV1::PublishObservation.literal(),
                    observation_subject,
                ),
                (
                    RepositoryActionLeafV1::PublishObservation.literal(),
                    interleaving_observation_subject,
                ),
                (
                    RepositoryActionLeafV1::PublishObservation.literal(),
                    run_observation_subject,
                ),
                (
                    RepositoryActionLeafV1::PublishObservation.literal(),
                    re_root_observation_subject,
                ),
                (
                    RepositoryActionLeafV1::PublishAssessment.literal(),
                    assessment_subject,
                ),
                (
                    RepositoryActionLeafV1::PublishAssessment.literal(),
                    missing_composite_subject,
                ),
                (
                    RepositoryActionLeafV1::SecurityEraseEvidencePayload.literal(),
                    erasure_subject,
                ),
            ],
            AuthorityFixtureModeV1::Valid,
            120,
            130,
        );
        let root = test_root();
        let mut store = StoreV1::create(&root, domain.clone()).unwrap();
        put_objects_in_reference_order(&mut store, fixture.objects.clone());
        let generation = StoreGenerationV1::new(
            domain.clone(),
            1,
            None,
            contract_root,
            StoreCompatibilityV1::stage0_successor().unwrap(),
            vec![fixture.authority_root_id],
        )
        .unwrap();
        store.publish_generation(&generation, None).unwrap();
        Connection::open(root.join("store.sqlite3"))
            .unwrap()
            .execute(
                "UPDATE store_state SET state = 'active', state_revision = state_revision + 1 WHERE singleton = 1",
                [],
            )
            .unwrap();

        let forged_session_observation = ObservationV1::new(ObservationDraftV1 {
            kind,
            store_domain_id: domain.id(),
            subjects: observation.subjects().to_vec(),
            producer: crate::domain::vnext::authority::ExecutionProducerV1::SessionBound {
                principal_id: PrincipalIdV1::derive("stage3-actor-principal").unwrap(),
                session_id: SessionIdV1::derive("stage5-forged-producer-session").unwrap(),
            },
            procedure_hash: digest(13),
            environment_hash: digest(14),
            toolchain_hash: digest(15),
            observed_at: 119,
            recorded_at: 120,
            clock_basis_hash: digest(16),
            lineage: vec![],
            payload: observation.payload().clone(),
            acquisition: ObservationAcquisitionV1::effect_free(digest(20), digest(21)).unwrap(),
            publication_route: ObservationPublicationRouteV1::new(kind, 39, None, None).unwrap(),
        });
        assert_eq!(
            forged_session_observation.unwrap_err(),
            ObservationError::KindSemanticMismatch
        );

        let run_state = EvidenceStoreFacadeV1::new(&mut store)
            .current_state_binding()
            .unwrap();
        let run_request = EvidenceStoreFacadeV1::new(&mut store)
            .canonical_observation_request(
                run_state,
                &run_observation,
                &run_payload,
                IdempotencyKeyIdV1::derive("stage5-unpersisted-run-observation").unwrap(),
            )
            .unwrap();
        let run_authority = authority_for(fixture.selection, &run_request, fixture.actor_principal);
        assert!(matches!(
            EvidenceStoreFacadeV1::new(&mut store)
                .publish_observation(
                    AuthorizedObservationPublicationV1::new(
                        run_state,
                        run_request,
                        run_authority,
                        run_observation,
                        run_payload,
                    )
                    .unwrap(),
                )
                .unwrap_err(),
            EvidenceStoreErrorV1::RunProvenanceNotCurrent
        ));

        let state = EvidenceStoreFacadeV1::new(&mut store)
            .current_state_binding()
            .unwrap();
        let request = EvidenceStoreFacadeV1::new(&mut store)
            .canonical_observation_request(
                state,
                &observation,
                &payload,
                IdempotencyKeyIdV1::derive("stage5-publish-observation").unwrap(),
            )
            .unwrap();
        let authority = authority_for(fixture.selection, &request, fixture.actor_principal);
        let publication = AuthorizedObservationPublicationV1::new(
            state,
            request,
            authority,
            observation.clone(),
            payload.clone(),
        )
        .unwrap();
        EvidenceStoreFacadeV1::new(&mut store)
            .publish_observation(publication)
            .unwrap();
        assert_eq!(
            EvidenceStoreFacadeV1::new(&mut store)
                .current_observations()
                .unwrap(),
            vec![observation.clone()]
        );

        let assessment_input_cut = EvidenceStoreFacadeV1::new(&mut store)
            .current_evidence_cut()
            .unwrap();
        let assessment_time = AssessmentTimeBasisV1::from_evidence_cut(
            &assessment_input_cut,
            TrustedTimeV1::Verified {
                lower_bound: 120,
                upper_bound: 120,
            },
            digest(27),
        )
        .unwrap();
        let assessment = AssessmentV1::evaluate_leaf(
            &gate_snapshot,
            gate.id(),
            AssessmentBasisV1 {
                store_domain_id: domain.id(),
                scope: AssessmentScopeV1::Work,
                inputs: vec![AssessmentInputRefV1::Observation(
                    ObservationAssessmentInputV1::from_observation(&observation).unwrap(),
                )],
                time: assessment_time,
            },
            &evaluator,
        )
        .unwrap();
        assert_eq!(assessment.result(), GateEvaluationResultV1::Indeterminate);
        let substitute_assessment = AssessmentV1::evaluate_leaf(
            &gate_snapshot,
            gate.id(),
            AssessmentBasisV1 {
                store_domain_id: domain.id(),
                scope: AssessmentScopeV1::Work,
                inputs: vec![AssessmentInputRefV1::Observation(
                    ObservationAssessmentInputV1::from_observation(&substitute_observation)
                        .unwrap(),
                )],
                time: assessment_time,
            },
            &evaluator,
        )
        .unwrap();
        let claim_assessment = AssessmentV1::evaluate_leaf(
            &gate_snapshot,
            gate.id(),
            AssessmentBasisV1 {
                store_domain_id: domain.id(),
                scope: AssessmentScopeV1::Work,
                inputs: vec![AssessmentInputRefV1::Claim(
                    ClaimAssessmentInputV1::from_claim(&unpublished_claim, &[&observation])
                        .unwrap(),
                )],
                time: assessment_time,
            },
            &evaluator,
        )
        .unwrap();

        let state = EvidenceStoreFacadeV1::new(&mut store)
            .current_state_binding()
            .unwrap();
        let request = EvidenceStoreFacadeV1::new(&mut store)
            .canonical_assessment_request(
                state,
                &substitute_assessment,
                IdempotencyKeyIdV1::derive("stage5-substitute-observation-assessment").unwrap(),
            )
            .unwrap();
        let authority = authority_for(fixture.selection, &request, fixture.actor_principal);
        assert!(matches!(
            EvidenceStoreFacadeV1::new(&mut store)
                .publish_assessment(
                    AuthorizedAssessmentPublicationV1::new(
                        state,
                        request,
                        authority,
                        gate_snapshot.clone(),
                        substitute_assessment,
                    )
                    .unwrap(),
                )
                .unwrap_err(),
            EvidenceStoreErrorV1::ObservationNotCurrent
        ));

        let state = EvidenceStoreFacadeV1::new(&mut store)
            .current_state_binding()
            .unwrap();
        let request = EvidenceStoreFacadeV1::new(&mut store)
            .canonical_assessment_request(
                state,
                &claim_assessment,
                IdempotencyKeyIdV1::derive("stage5-unpublished-claim-assessment").unwrap(),
            )
            .unwrap();
        let authority = authority_for(fixture.selection, &request, fixture.actor_principal);
        assert!(matches!(
            EvidenceStoreFacadeV1::new(&mut store)
                .publish_assessment(
                    AuthorizedAssessmentPublicationV1::new(
                        state,
                        request,
                        authority,
                        gate_snapshot.clone(),
                        claim_assessment,
                    )
                    .unwrap(),
                )
                .unwrap_err(),
            EvidenceStoreErrorV1::AssessmentInputNotCurrent
        ));

        let state = EvidenceStoreFacadeV1::new(&mut store)
            .current_state_binding()
            .unwrap();
        let request = EvidenceStoreFacadeV1::new(&mut store)
            .canonical_assessment_request(
                state,
                &assessment,
                IdempotencyKeyIdV1::derive("stage5-publish-assessment").unwrap(),
            )
            .unwrap();
        let authority = authority_for(fixture.selection, &request, fixture.actor_principal);
        EvidenceStoreFacadeV1::new(&mut store)
            .publish_assessment(
                AuthorizedAssessmentPublicationV1::new(
                    state,
                    request,
                    authority,
                    gate_snapshot.clone(),
                    assessment.clone(),
                )
                .unwrap(),
            )
            .unwrap();
        let cut = EvidenceStoreFacadeV1::new(&mut store)
            .current_evidence_cut()
            .unwrap();
        assert_eq!(cut.assessments(), std::slice::from_ref(&assessment));
        assert_ne!(
            assessment.input_store_generation_id(),
            cut.store_generation_id()
        );
        let applicability = AssessmentApplicabilityV1::new(
            domain.id(),
            cut.store_generation_id(),
            &gate_snapshot,
            AssessmentScopeV1::Work,
            TrustedTimeV1::Verified {
                lower_bound: 120,
                upper_bound: 120,
            },
            assessment.time_basis(),
        )
        .unwrap();
        let resolution = resolve_gate_assessments(gate.id(), &applicability, &cut).unwrap();
        assert_eq!(resolution.result(), GateEvaluationResultV1::Indeterminate);
        assert_eq!(resolution.applicable_assessment_ids(), &[assessment.id()]);
        let missing_time = AssessmentTimeBasisV1::from_evidence_cut(
            &cut,
            TrustedTimeV1::Verified {
                lower_bound: 120,
                upper_bound: 120,
            },
            digest(27),
        )
        .unwrap();
        let missing_applicability = AssessmentApplicabilityV1::new(
            domain.id(),
            cut.store_generation_id(),
            &gate_snapshot,
            AssessmentScopeV1::Work,
            TrustedTimeV1::Verified {
                lower_bound: 120,
                upper_bound: 120,
            },
            missing_time,
        )
        .unwrap();
        let missing_resolution =
            resolve_gate_assessments(missing_gate.id(), &missing_applicability, &cut).unwrap();
        assert!(missing_resolution.applicable_assessment_ids().is_empty());
        let missing_composite_assessment = AssessmentV1::evaluate_composite(
            &gate_snapshot,
            missing_composite.id(),
            domain.id(),
            AssessmentScopeV1::Work,
            missing_time,
            vec![missing_resolution],
        )
        .unwrap();
        assert_eq!(
            missing_composite_assessment.result(),
            GateEvaluationResultV1::Indeterminate
        );
        let state = EvidenceStoreFacadeV1::new(&mut store)
            .current_state_binding()
            .unwrap();
        let request = EvidenceStoreFacadeV1::new(&mut store)
            .canonical_assessment_request(
                state,
                &missing_composite_assessment,
                IdempotencyKeyIdV1::derive("stage5-publish-missing-composite-assessment").unwrap(),
            )
            .unwrap();
        let authority = authority_for(fixture.selection, &request, fixture.actor_principal);
        EvidenceStoreFacadeV1::new(&mut store)
            .publish_assessment(
                AuthorizedAssessmentPublicationV1::new(
                    state,
                    request,
                    authority,
                    gate_snapshot.clone(),
                    missing_composite_assessment.clone(),
                )
                .unwrap(),
            )
            .unwrap();
        drop(store);
        let mut store = StoreV1::open(&root, domain.clone()).unwrap();
        let cut = EvidenceStoreFacadeV1::new(&mut store)
            .current_evidence_cut()
            .unwrap();
        assert!(cut.assessments().contains(&missing_composite_assessment));
        let pre_erasure_nonpayload_ids = active_objects(&mut store)
            .into_iter()
            .map(|object| object.id())
            .filter(|object_id| *object_id != payload.id())
            .collect::<Vec<_>>();
        let retained_backup = store.seal_export().unwrap();
        let retained_export_id = retained_backup.export().id();
        assert!(retained_backup.export().entries().iter().any(|entry| {
            matches!(entry, crate::domain::vnext::persistence::SealedExportEntryV1::Available(object)
                if object.id() == payload.id())
        }));

        let state = EvidenceStoreFacadeV1::new(&mut store)
            .current_state_binding()
            .unwrap();
        let request = EvidenceStoreFacadeV1::new(&mut store)
            .canonical_security_erasure_request(
                state,
                payload.id(),
                *cut.complete_cut_hash(),
                IdempotencyKeyIdV1::derive("stage5-security-erasure").unwrap(),
            )
            .unwrap();
        let authority = authority_for(fixture.selection, &request, fixture.actor_principal);
        let plan = AuthorizedSecurityErasureV1::new(
            state,
            request,
            authority,
            payload.id(),
            *cut.complete_cut_hash(),
        )
        .unwrap();
        let (_, _, intent, receipt_before_collection) =
            begin_security_erasure(&mut store, &plan).unwrap();
        assert_eq!(intent.payload_object_id(), payload.id());
        assert!(receipt_before_collection.is_none());
        for field_index in [6_usize, 9_usize] {
            let mut mutant = intent.canonical_value().unwrap();
            let CborValue::Array(fields) = &mut mutant else {
                unreachable!("security-erasure Intent is a record array");
            };
            let CborValue::Bytes(value) = &mut fields[field_index] else {
                unreachable!("security-erasure digest field is bytes");
            };
            value[0] ^= 1;
            let identity_value = CborValue::Array(fields[1..].to_vec());
            fields[0] = CborValue::Bytes(
                identity::domain_hash(
                    "maestro.vnext.evidence.security-erasure-intent.v1",
                    &identity_value,
                )
                .unwrap()
                .to_vec(),
            );
            assert_eq!(
                SecurityErasureIntentV1::from_canonical_value(&mutant).unwrap_err(),
                SecurityErasureError::InvalidStoredErasure
            );
        }
        assert_eq!(store.read_object(payload.id()).unwrap(), payload);
        let re_root_state = EvidenceStoreFacadeV1::new(&mut store)
            .current_state_binding()
            .unwrap();
        let re_root_request = EvidenceStoreFacadeV1::new(&mut store)
            .canonical_observation_request(
                re_root_state,
                &re_root_observation,
                &payload,
                IdempotencyKeyIdV1::derive("stage5-erasure-re-root-observation").unwrap(),
            )
            .unwrap();
        let re_root_authority =
            authority_for(fixture.selection, &re_root_request, fixture.actor_principal);
        let re_root_result = EvidenceStoreFacadeV1::new(&mut store).publish_observation(
            AuthorizedObservationPublicationV1::new(
                re_root_state,
                re_root_request,
                re_root_authority,
                re_root_observation,
                payload.clone(),
            )
            .unwrap(),
        );
        assert!(
            matches!(
                &re_root_result,
                Err(EvidenceStoreErrorV1::Store(StoreError::ControlledCopyErasureInProgress(object_id)))
                    if *object_id == payload.id()
            ),
            "re-root publication returned {re_root_result:?}"
        );
        let interleaving_state = EvidenceStoreFacadeV1::new(&mut store)
            .current_state_binding()
            .unwrap();
        let interleaving_request = EvidenceStoreFacadeV1::new(&mut store)
            .canonical_observation_request(
                interleaving_state,
                &interleaving_observation,
                &interleaving_payload,
                IdempotencyKeyIdV1::derive("stage5-erasure-interleaving-observation").unwrap(),
            )
            .unwrap();
        let interleaving_authority = authority_for(
            fixture.selection,
            &interleaving_request,
            fixture.actor_principal,
        );
        EvidenceStoreFacadeV1::new(&mut store)
            .publish_observation(
                AuthorizedObservationPublicationV1::new(
                    interleaving_state,
                    interleaving_request,
                    interleaving_authority,
                    interleaving_observation.clone(),
                    interleaving_payload,
                )
                .unwrap(),
            )
            .unwrap();
        let debit_schema = SchemaIdV1::parse(
            "sha256:00307bd5d7de8e473f3bd06c4153c39b9e3789a9cad5b0bfca004a9ecd368c8a",
        )
        .unwrap();
        let debits_after_authority_advance = active_objects(&mut store)
            .iter()
            .filter(|object| object.schema_id() == debit_schema)
            .count();
        drop(store);
        let mut store = StoreV1::open(&root, domain.clone()).unwrap();
        for boundary in [
            SecurityErasureCheckpointV1::TombstoneDurable,
            SecurityErasureCheckpointV1::ControlledCopiesErased,
            SecurityErasureCheckpointV1::CollectionCommitted,
            SecurityErasureCheckpointV1::AbsenceVerified,
            SecurityErasureCheckpointV1::FinalPublicationCommitted,
        ] {
            let interrupted = EvidenceStoreFacadeV1::new(&mut store)
                .security_erase_payload_with_checkpoint(plan.clone(), |observed| {
                    if observed == boundary {
                        Err(EvidenceStoreErrorV1::ErasureRecoveryMismatch)
                    } else {
                        Ok(())
                    }
                });
            assert!(
                matches!(
                    interrupted,
                    Err(EvidenceStoreErrorV1::ErasureRecoveryMismatch)
                ),
                "restart boundary {boundary:?} returned {interrupted:?}"
            );
            if boundary == SecurityErasureCheckpointV1::TombstoneDurable {
                assert!(matches!(
                    store.seal_export().unwrap_err(),
                    StoreError::ControlledCopyErasureInProgress(object_id)
                        if object_id == payload.id()
                ));
            }
            drop(store);
            store = StoreV1::open(&root, domain.clone()).unwrap();
        }
        let outcome = EvidenceStoreFacadeV1::new(&mut store)
            .security_erase_payload(plan.clone())
            .unwrap();
        assert!(outcome.replayed());
        assert_eq!(
            active_objects(&mut store)
                .iter()
                .filter(|object| object.schema_id() == debit_schema)
                .count(),
            debits_after_authority_advance,
            "durably admitted erasure recovery must not debit Authority a second time"
        );
        assert_eq!(outcome.receipt().affected_assessments().len(), 1);
        assert!(!outcome.receipt().is_general_garbage_collection());
        assert!(
            store
                .recover_sealed_export_publication(retained_export_id)
                .is_err()
        );
        let post_erasure_backup = store.seal_export().unwrap();
        assert!(post_erasure_backup.export().entries().iter().any(|entry| {
            matches!(entry, crate::domain::vnext::persistence::SealedExportEntryV1::Tombstoned(object)
                if object.object_id() == payload.id())
        }));
        let immutable_delete = Connection::open(root.join("store.sqlite3"))
            .unwrap()
            .execute(
                "DELETE FROM store_sealed_exports WHERE export_id = ?1",
                rusqlite::params![post_erasure_backup.export().id().as_bytes()],
            )
            .unwrap_err();
        assert!(
            immutable_delete
                .to_string()
                .contains("store_sealed_exports is insert-only"),
            "the narrow security-erasure override must restore ordinary insert-only law"
        );
        let mut receipt_mutant = outcome.receipt().canonical_value();
        let CborValue::Array(receipt_fields) = &mut receipt_mutant else {
            unreachable!("security-erasure Receipt is a record array");
        };
        receipt_fields[1] = CborValue::Bytes(digest(99).to_vec());
        let receipt_identity_value = CborValue::Array(receipt_fields[1..].to_vec());
        receipt_fields[0] = CborValue::Bytes(
            identity::domain_hash(
                "maestro.vnext.evidence.security-erasure-receipt.v1",
                &receipt_identity_value,
            )
            .unwrap()
            .to_vec(),
        );
        let receipt_mutant =
            SecurityErasureReceiptV1::from_canonical_value(&receipt_mutant).unwrap();
        let mut objects = active_objects(&mut store);
        let mut index = load_optional_evidence_index(&objects).unwrap().unwrap();
        let entry = index
            .erasures
            .iter()
            .find(|entry| entry.intent_id == intent.id())
            .copied()
            .unwrap();
        let receipt_object =
            evidence_erasure_receipt_object(&receipt_mutant, entry.intent_object_id).unwrap();
        objects.retain(|object| Some(object.id()) != entry.receipt_object_id);
        objects.push(receipt_object.clone());
        let entry = index
            .erasures
            .iter_mut()
            .find(|entry| entry.intent_id == intent.id())
            .unwrap();
        entry.receipt_id = Some(receipt_mutant.id());
        entry.receipt_object_id = Some(receipt_object.id());
        assert!(matches!(
            load_erasure_from_index(&objects, &index, intent.id()),
            Err(EvidenceStoreErrorV1::ErasureRecoveryMismatch)
        ));
        assert!(matches!(
            store.read_object(payload.id()),
            Err(StoreError::CollectedObject(_) | StoreError::TombstonedObject(_))
        ));
        for object_id in &pre_erasure_nonpayload_ids {
            store.read_object(*object_id).unwrap_or_else(|error| {
                panic!("erasure removed nonpayload Object {object_id}: {error}")
            });
        }
        drop(store);
        let mut store = StoreV1::open(&root, domain.clone()).unwrap();
        let payload_hex = payload
            .id()
            .render()
            .strip_prefix("sha256:")
            .unwrap()
            .to_owned();
        let resurrected_payload_path = root
            .join("objects")
            .join(&payload_hex[..2])
            .join(format!("{payload_hex}.cbor"));
        std::fs::create_dir_all(resurrected_payload_path.parent().unwrap()).unwrap();
        std::fs::write(&resurrected_payload_path, payload.canonical_bytes()).unwrap();
        assert!(matches!(
            EvidenceStoreFacadeV1::new(&mut store).security_erase_payload(plan.clone()),
            Err(EvidenceStoreErrorV1::Store(
                StoreError::CollectedObjectStillPresent(object_id)
            )) if object_id == payload.id()
        ));
        std::fs::remove_file(&resurrected_payload_path).unwrap();
        let replay = EvidenceStoreFacadeV1::new(&mut store)
            .security_erase_payload(plan)
            .unwrap();
        assert!(replay.replayed());
        assert_eq!(replay.receipt(), outcome.receipt());
        let cut = EvidenceStoreFacadeV1::new(&mut store)
            .current_evidence_cut()
            .unwrap();
        assert_ne!(cut.complete_cut_hash(), &[0; 32]);
        let (_, _, current_generation, active_objects) =
            store.coherent_publication_snapshot().unwrap();
        let current_index = load_optional_evidence_index(&active_objects)
            .unwrap()
            .unwrap();
        let assessment_id = outcome.receipt().affected_assessments()[0];
        let invalidation_entry = current_index
            .invalidations
            .iter()
            .find(|entry| entry.assessment_id == assessment_id)
            .unwrap();
        let invalidation_object = active_objects
            .iter()
            .find(|object| object.id() == invalidation_entry.invalidation_object_id)
            .unwrap();
        let CborValue::Bytes(invalidation_bytes) = invalidation_object.value() else {
            unreachable!("Assessment invalidation Object contains canonical bytes");
        };
        let stored_invalidation =
            AssessmentInvalidationV1::from_canonical_bytes(invalidation_bytes).unwrap();
        validate_stored_invalidation_authority(
            &current_generation,
            &active_objects,
            &stored_invalidation,
        )
        .unwrap();
        for (field, replacement) in [
            (4_usize, bytes(&digest(90))),
            (5_usize, bytes(&digest(91))),
            (6_usize, bytes(&digest(92))),
            (7_usize, bytes(&digest(93))),
            (8_usize, CborValue::Unsigned(121)),
        ] {
            let mutant = invalidation_mutant(&stored_invalidation, |fields| {
                fields[field] = replacement;
            });
            assert!(matches!(
                validate_stored_invalidation_authority(
                    &current_generation,
                    &active_objects,
                    &mutant,
                ),
                Err(EvidenceStoreErrorV1::InvalidInvalidationAuthority)
            ));
        }

        let original_request_id =
            StoreObjectIdV1::from_digest(*stored_invalidation.action_request_hash());
        let original_request = active_objects
            .iter()
            .find(|object| object.id() == original_request_id)
            .unwrap();
        let CborValue::Array(mut forged_request_fields) = original_request.value().clone() else {
            unreachable!("Evidence Action Request is an array");
        };
        forged_request_fields[3] = CborValue::Unsigned(41);
        let forged_request_id = hash(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.evidence-action-request-id.v1").unwrap(),
            forged_request_fields[3].clone(),
            forged_request_fields[4].clone(),
            forged_request_fields[5].clone(),
            forged_request_fields[6].clone(),
            forged_request_fields[2].clone(),
        ]))
        .unwrap();
        forged_request_fields[1] = bytes(&forged_request_id);
        let forged_request = StoreObjectV1::new(
            original_request.schema_id(),
            CborValue::Array(forged_request_fields),
            vec![],
        )
        .unwrap();
        let forged_action_invalidation = invalidation_mutant(&stored_invalidation, |fields| {
            fields[6] = bytes(&forged_request_id);
            fields[7] = bytes(forged_request.id().as_bytes());
        });
        let mut forged_objects = active_objects.clone();
        forged_objects.push(forged_request);
        assert!(matches!(
            validate_stored_invalidation_authority(
                &current_generation,
                &forged_objects,
                &forged_action_invalidation,
            ),
            Err(EvidenceStoreErrorV1::InvalidInvalidationAuthority)
        ));

        let result_object = active_objects
            .iter()
            .find(|object| {
                matches!(object.value(), CborValue::Array(fields)
                    if fields.len() == 11
                        && fields.get(1).and_then(|value| exact_digest(value).ok())
                            == Some(*stored_invalidation.action_request_id().as_bytes())
                        && fields.get(3) == Some(&CborValue::Unsigned(1)))
            })
            .unwrap();
        let result_schema = result_object.schema_id();
        let CborValue::Array(result_fields) = result_object.value() else {
            unreachable!("Action Result is an array");
        };
        let mut missing_produced_fields = result_fields.clone();
        missing_produced_fields[7] = CborValue::Array(vec![bytes(&digest(94))]);
        let missing_produced_result = StoreObjectV1::new(
            result_schema,
            CborValue::Array(missing_produced_fields),
            result_object.references().to_vec(),
        )
        .unwrap();
        let mut missing_produced_objects = active_objects
            .iter()
            .filter(|object| object.id() != result_object.id())
            .cloned()
            .collect::<Vec<_>>();
        missing_produced_objects.push(missing_produced_result);
        assert!(matches!(
            validate_stored_invalidation_authority(
                &current_generation,
                &missing_produced_objects,
                &stored_invalidation,
            ),
            Err(EvidenceStoreErrorV1::InvalidInvalidationAuthority)
        ));

        let missing_reference_result = StoreObjectV1::new(
            result_schema,
            CborValue::Array(result_fields.clone()),
            result_object
                .references()
                .iter()
                .copied()
                .filter(|reference| *reference != invalidation_object.id())
                .collect(),
        )
        .unwrap();
        let mut missing_reference_objects = active_objects
            .iter()
            .filter(|object| object.id() != result_object.id())
            .cloned()
            .collect::<Vec<_>>();
        missing_reference_objects.push(missing_reference_result);
        assert!(matches!(
            validate_stored_invalidation_authority(
                &current_generation,
                &missing_reference_objects,
                &stored_invalidation,
            ),
            Err(EvidenceStoreErrorV1::InvalidInvalidationAuthority)
        ));
        let mut expected_observations = vec![observation, interleaving_observation.clone()];
        expected_observations.sort_by_key(ObservationV1::id);
        assert_eq!(
            EvidenceStoreFacadeV1::new(&mut store)
                .current_observations()
                .unwrap(),
            expected_observations
        );
        let post_erasure_input_cut = *cut.evidence_input_cut_hash();
        let post_erasure_assessment = AssessmentV1::evaluate_leaf(
            &gate_snapshot,
            gate.id(),
            AssessmentBasisV1 {
                store_domain_id: domain.id(),
                scope: AssessmentScopeV1::Work,
                inputs: vec![AssessmentInputRefV1::Observation(
                    ObservationAssessmentInputV1::from_observation(&interleaving_observation)
                        .unwrap(),
                )],
                time: AssessmentTimeBasisV1::from_evidence_cut(
                    &cut,
                    TrustedTimeV1::Verified {
                        lower_bound: 120,
                        upper_bound: 120,
                    },
                    digest(27),
                )
                .unwrap(),
            },
            &evaluator,
        )
        .unwrap();
        assert_eq!(cut.evidence_input_cut_hash(), &post_erasure_input_cut);
        let state = EvidenceStoreFacadeV1::new(&mut store)
            .current_state_binding()
            .unwrap();
        let request = EvidenceStoreFacadeV1::new(&mut store)
            .canonical_assessment_request(
                state,
                &post_erasure_assessment,
                IdempotencyKeyIdV1::derive("stage5-post-erasure-assessment").unwrap(),
            )
            .unwrap();
        let authority = authority_for(fixture.selection, &request, fixture.actor_principal);
        EvidenceStoreFacadeV1::new(&mut store)
            .publish_assessment(
                AuthorizedAssessmentPublicationV1::new(
                    state,
                    request,
                    authority,
                    gate_snapshot.clone(),
                    post_erasure_assessment.clone(),
                )
                .unwrap(),
            )
            .unwrap();
        assert!(
            EvidenceStoreFacadeV1::new(&mut store)
                .current_evidence_cut()
                .unwrap()
                .assessments()
                .contains(&post_erasure_assessment)
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
