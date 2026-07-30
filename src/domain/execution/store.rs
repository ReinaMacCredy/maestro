use sha2::{Digest, Sha256};
use thiserror::Error;

use super::control_head::{
    EffectIntentControlErrorV1, EffectIntentControlHeadV1, EffectIntentControlHealthV1,
    EffectIntentControlPublicationCommitmentsV1, EffectIntentControlRevisionV1,
    EffectIntentControlTokenV1, EffectIntentControlWriterTermV1, SameHomeWriterFencingReceiptV1,
};
use super::effect_home::HomeTokenV1;
use super::effect_home::{
    ActiveStoreHomeV1, ActiveStoreOriginationFenceV1, ActiveStoreUseFenceV1,
    EffectIntentDomainKindV1, EffectIntentHomeV1, EffectIntentOriginationFenceV1,
    EffectIntentUseFenceV1,
};
use super::effects::{
    EffectControlTransitionNeedV1, EffectCredentialRequirementsV1, EffectDispatchAttemptV1,
    EffectDispatchBindingInputsV1, EffectDispatchOutcomePayloadV1, EffectDispatchSealCandidateV1,
    EffectDispatchTerminalCandidateV1, EffectIntentDraftV1, EffectIntentV1, EffectMaterialInputsV1,
    EffectOriginKindV1, EffectOriginV1, EffectReconciliationAttemptV1,
    EffectReconciliationOutcomeV1, EffectReconciliationPreparationV1,
    EffectReconciliationReadPlanV1, EffectReconciliationReadUsageV1, EffectRuntimeErrorV1,
    EffectSemanticUseV1, EffectWithdrawalCurrentCarrierV1, EffectWithdrawalV1,
    PreparedEffectDispatchV1,
};
use super::runtime::{
    AuthorizedExecutionActionV1, CanonicalExecutionActionRequestV1, EffectIntentIdV1,
    ExecutionActionV1, ExecutionRuntimeErrorV1, LeaseTermIdV1, RunIdV1, RunNoStartReceiptV1,
    RunReservationV1, RunSegmentAppendV1, RunStateV1, StepAttemptStateV1, StepAttemptTerminalV1,
    StepExecutionAcquisitionV1, StepExecutionCarrierV1, TakeoverSafetyV1,
};
use crate::domain::authority::{
    ActionOutcomeV1, ActionRequestIdV1, ActionResultV1, AdmittedRepositoryActionV1,
    BootstrapControlG0AuthorityBasisV1, BootstrapExecutionAuthorityV1, CmaBranchIdV1,
    CmaEffectWithdrawalSlotFamilyV1, CmaObservationPublicationPurposeV1,
    ContinuedRepositoryActionV1, ContinuityMaintenanceAuthorityBasisV1,
    ContinuityMaintenanceExecutionAuthorityV1, ExecutionAuthorityV1, ExecutorAssertionIdV1,
    GenericExecutionAuthorityV1, GenesisGrantIdV1, GrantIdV1, IdempotencyKeyIdV1,
    PrincipalBindingIdV1, PrincipalIdV1, RepositoryActionAdmissionInputV1, RepositoryActionLeafV1,
    RepositoryAuthorityAdmissionErrorV1, RepositoryAuthorityArtifactsV1,
    RepositoryAuthoritySelectionV1, SessionIdV1, SlotIdV1, StateTokenIdV1, SubmitStepAuthorityV1,
    admit_repository_action, continue_repository_action_attempt, current_repository_authority_time,
    validate_persisted_repository_action_basis,
};
use crate::domain::evidence::{
    ClaimError, ClaimV1, EvidenceClaimPublicationV1, EvidenceStoreErrorV1,
    ObservationSubjectKindV1, SubmissionRefV1, resolve_current_observation_objects,
};
use crate::domain::identity::{
    IdentityError, SchemaIdV1, StoreGenerationIdV1, StoreHeadIdV1, StoreObjectIdV1, derive_identity,
};
use crate::domain::persistence::{
    AtomicGenerationPublicationV1, AtomicPublicationError, GenerationError,
    PreparedPublicationError, StoreError, StoreGenerationV1, StoreHeadV1, StoreIdempotencyProbeV1,
    StoreIdempotencyV1, StoreObjectError, StoreObjectV1, StorePublicationOutcomeV1, StoreRoleV1,
    StoreStateV1, StoreV1,
};
use crate::domain::repository::RepositoryStoreSchemaV1;
use crate::domain::step::{
    CanonicalStepSubmissionActionRequestV1, StepBindingV1, StepLifecycleError, StepLifecycleV1,
    StepOpenBasisV1, StepStateV1, StepSubmissionErrorV1, StepSubmissionIdV1, StepSubmissionV1,
};
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

const EFFECT_ORIGINATION_IDEMPOTENCY_NAMESPACE_V1: &str =
    "maestro.vnext.effect-origination-authorized-publication.v1";
const EFFECT_REDISPATCH_IDEMPOTENCY_NAMESPACE_V1: &str =
    "maestro.vnext.effect-redispatch-authorized-publication.v1";
const EFFECT_DISPATCH_SEAL_IDEMPOTENCY_NAMESPACE_V1: &str =
    "maestro.vnext.effect-dispatch-seal-authorized-publication.v1";
const EFFECT_DISPATCH_TERMINAL_IDEMPOTENCY_NAMESPACE_V1: &str =
    "maestro.vnext.effect-dispatch-terminal-authorized-publication.v1";
const EFFECT_RECOVER_RESERVED_IDEMPOTENCY_NAMESPACE_V1: &str =
    "maestro.vnext.effect-recover-reserved-authorized-publication.v1";
const EFFECT_DISPATCH_AUTHORIZED_CARRIER_DOMAIN_V1: &str =
    "maestro.vnext.effect-dispatch-authorized-store-carrier.v1";
const EFFECT_RECONCILIATION_BEGIN_IDEMPOTENCY_NAMESPACE_V1: &str =
    "maestro.vnext.effect-reconciliation-begin-authorized-publication.v1";
const EFFECT_RECONCILIATION_READ_IDEMPOTENCY_NAMESPACE_V1: &str =
    "maestro.vnext.effect-reconciliation-read-authorized-publication.v1";
const EFFECT_RECONCILIATION_TERMINAL_IDEMPOTENCY_NAMESPACE_V1: &str =
    "maestro.vnext.effect-reconciliation-terminal-authorized-publication.v1";
const EFFECT_RECONCILIATION_AUTHORIZED_CARRIER_DOMAIN_V1: &str =
    "maestro.vnext.effect-reconciliation-authorized-store-carrier.v1";
const EFFECT_WITHDRAWAL_IDEMPOTENCY_NAMESPACE_V1: &str =
    "maestro.vnext.effect-withdrawal-authorized-publication.v1";
const EFFECT_WITHDRAWAL_AUTHORIZED_CARRIER_DOMAIN_V1: &str =
    "maestro.vnext.effect-withdrawal-authorized-store-carrier.v1";
const EFFECT_WRITER_HANDOFF_IDEMPOTENCY_NAMESPACE_V1: &str =
    "maestro.vnext.effect-writer-handoff-authorized-publication.v1";
const EFFECT_HEALTH_IDEMPOTENCY_NAMESPACE_V1: &str =
    "maestro.vnext.effect-health-authorized-publication.v1";
const EFFECT_HEALTH_AUTHORIZED_CARRIER_DOMAIN_V1: &str =
    "maestro.vnext.effect-health-authorized-store-carrier.v1";
const EFFECT_WRITER_HANDOFF_AUTHORIZED_CARRIER_DOMAIN_V1: &str =
    "maestro.vnext.effect-writer-handoff-authorized-store-carrier.v1";
const STEP_SUBMISSION_IDEMPOTENCY_NAMESPACE_V1: &str =
    "maestro.vnext.step-submission-authorized-publication.v1";
const STEP_SUBMISSION_SCHEMA_V1: &str = "maestro.vnext.step-submission-schema.v1";
const STEP_SUBMISSION_CLAIM_SET_SCHEMA_V1: &str =
    "maestro.vnext.step-submission-claim-set-schema.v1";
const STEP_SUBMISSION_CLAIM_SCHEMA_V1: &str = "maestro.vnext.evidence-claim-schema.v1";
const STEP_SUBMISSION_OBSERVATION_SCHEMA_V1: &str = "maestro.vnext.evidence-observation-schema.v1";
const STEP_SUBMISSION_ACTION_REQUEST_SCHEMA_V1: &str =
    "maestro.vnext.step-submit-action-request-schema.v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionStoreStateBindingV1 {
    store_head_id: StoreHeadIdV1,
    store_generation_id: StoreGenerationIdV1,
    control_head: Option<EffectIntentControlTokenV1>,
    control_index_object_id: Option<StoreObjectIdV1>,
}

impl ExecutionStoreStateBindingV1 {
    pub const fn store_head_id(&self) -> StoreHeadIdV1 {
        self.store_head_id
    }

    pub const fn store_generation_id(&self) -> StoreGenerationIdV1 {
        self.store_generation_id
    }

    pub const fn control_head(&self) -> Option<EffectIntentControlTokenV1> {
        self.control_head
    }

    const fn control_index_object_id(&self) -> Option<StoreObjectIdV1> {
        self.control_index_object_id
    }

    pub fn canonical_value(&self) -> Result<CborValue, CborError> {
        Ok(CborValue::Array(vec![
            CborValue::text("maestro.vnext.execution-store-state-binding.v1")?,
            bytes(self.store_head_id.as_bytes()),
            bytes(self.store_generation_id.as_bytes()),
            CborValue::optional(self.control_head.map(|head| bytes(head.as_bytes()))),
            CborValue::optional(
                self.control_index_object_id
                    .map(|object_id| bytes(object_id.as_bytes())),
            ),
        ]))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveStoreEffectOriginationDraftV1 {
    pub domain_kind: EffectIntentDomainKindV1,
    pub stable_domain_id: HomeTokenV1,
    pub realm: HomeTokenV1,
    pub semantic_namespace: HomeTokenV1,
    pub uniqueness_namespace: HomeTokenV1,
    pub origin: EffectOriginV1,
    pub semantic_use: EffectSemanticUseV1,
    pub material_inputs: EffectMaterialInputsV1,
    pub credential_requirements: EffectCredentialRequirementsV1,
    pub dispatch: EffectDispatchBindingInputsV1,
}

impl ActiveStoreEffectOriginationDraftV1 {
    fn subject_value(&self) -> Result<CborValue, ExecutionStoreErrorV1> {
        Ok(CborValue::Array(vec![
            CborValue::text("maestro.vnext.active-store-effect-origination-subject.v1")?,
            CborValue::Unsigned(match self.domain_kind {
                EffectIntentDomainKindV1::RepositoryDomain => 1,
                EffectIntentDomainKindV1::InstallationDomain => 2,
            }),
            bytes(self.stable_domain_id.as_bytes()),
            bytes(self.realm.as_bytes()),
            bytes(self.semantic_namespace.as_bytes()),
            bytes(self.uniqueness_namespace.as_bytes()),
            CborValue::Unsigned(u64::from(self.origin.kind().tag())),
            bytes(&self.origin.commitment()?),
            bytes(self.semantic_use.as_bytes()),
            bytes(self.material_inputs.as_bytes()),
            bytes(self.credential_requirements.as_bytes()),
        ]))
    }

    fn payload_value(&self) -> Result<CborValue, ExecutionStoreErrorV1> {
        Ok(CborValue::Array(vec![
            CborValue::text("maestro.vnext.active-store-effect-origination-payload.v1")?,
            self.dispatch.canonical_value()?,
        ]))
    }

    fn request_payload_value(&self) -> Result<CborValue, ExecutionStoreErrorV1> {
        Ok(CborValue::Array(vec![
            CborValue::text("maestro.vnext.active-store-effect-origination-request-payload.v1")?,
            self.subject_value()?,
            self.payload_value()?,
        ]))
    }

    pub fn authority_subject_value(&self) -> Result<CborValue, CborError> {
        effect_authority_subject_value(
            self.domain_kind,
            self.stable_domain_id,
            self.realm,
            self.semantic_namespace,
            self.uniqueness_namespace,
            self.semantic_use,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveStoreEffectOriginationPublicationV1 {
    request: CanonicalExecutionActionRequestV1,
    authority: ExecutionAuthorityV1,
    state_binding: ExecutionStoreStateBindingV1,
    draft: ActiveStoreEffectOriginationDraftV1,
}

impl ActiveStoreEffectOriginationPublicationV1 {
    pub fn new(
        request: CanonicalExecutionActionRequestV1,
        authority: impl Into<ExecutionAuthorityV1>,
        state_binding: ExecutionStoreStateBindingV1,
        draft: ActiveStoreEffectOriginationDraftV1,
    ) -> Result<Self, ExecutionStoreErrorV1> {
        let authority = authority.into();
        if state_binding.control_head().is_some()
            || draft.stable_domain_id.as_bytes() == &[0; 32]
            || request.action() != draft.origin.reservation_action()?
            || draft.realm.as_bytes() == &[0; 32]
            || draft.semantic_namespace.as_bytes() == &[0; 32]
            || draft.uniqueness_namespace.as_bytes() == &[0; 32]
        {
            return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
        }
        validate_request_values(
            &request,
            &draft.authority_subject_value()?,
            &state_binding.canonical_value()?,
            &draft.request_payload_value()?,
        )?;
        validate_execution_authority_binding(&request, &authority)?;
        validate_effect_authority_origin(&authority, draft.origin.kind())?;
        Ok(Self {
            request,
            authority,
            state_binding,
            draft,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveStoreEffectRedispatchDraftV1 {
    dispatch: EffectDispatchBindingInputsV1,
}

impl ActiveStoreEffectRedispatchDraftV1 {
    pub const fn new(dispatch: EffectDispatchBindingInputsV1) -> Self {
        Self { dispatch }
    }

    pub const fn dispatch(&self) -> &EffectDispatchBindingInputsV1 {
        &self.dispatch
    }

    fn payload_value(&self) -> Result<CborValue, ExecutionStoreErrorV1> {
        Ok(CborValue::Array(vec![
            CborValue::text("maestro.vnext.active-store-effect-redispatch-payload.v1")?,
            self.dispatch.canonical_value()?,
        ]))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveStoreEffectRedispatchPublicationV1 {
    request: CanonicalExecutionActionRequestV1,
    authority: ExecutionAuthorityV1,
    snapshot: ActiveStoreEffectSnapshotV1,
    draft: ActiveStoreEffectRedispatchDraftV1,
}

impl ActiveStoreEffectRedispatchPublicationV1 {
    pub fn new(
        request: CanonicalExecutionActionRequestV1,
        authority: impl Into<ExecutionAuthorityV1>,
        snapshot: ActiveStoreEffectSnapshotV1,
        draft: ActiveStoreEffectRedispatchDraftV1,
    ) -> Result<Self, ExecutionStoreErrorV1> {
        let authority = authority.into();
        if request.action() != snapshot.intent.origin().reservation_action()? {
            return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
        }
        validate_request_values(
            &request,
            &effect_intent_subject_value(&snapshot.intent)?,
            &snapshot.state_binding.canonical_value()?,
            &draft.payload_value()?,
        )?;
        validate_execution_authority_binding(&request, &authority)?;
        validate_effect_authority_origin(&authority, snapshot.intent.origin().kind())?;
        Ok(Self {
            request,
            authority,
            snapshot,
            draft,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveStoreEffectRedispatchOutcomeV1 {
    store_head: StoreHeadV1,
    intent: EffectIntentIdV1,
    control_head: EffectIntentControlTokenV1,
    control_revision: EffectIntentControlTokenV1,
    dispatch_attempt: super::runtime::DispatchAttemptIdV1,
    replayed: bool,
}

impl ActiveStoreEffectRedispatchOutcomeV1 {
    pub const fn store_head(&self) -> &StoreHeadV1 {
        &self.store_head
    }

    pub const fn intent(&self) -> EffectIntentIdV1 {
        self.intent
    }

    pub const fn control_head(&self) -> EffectIntentControlTokenV1 {
        self.control_head
    }

    pub const fn control_revision(&self) -> EffectIntentControlTokenV1 {
        self.control_revision
    }

    pub const fn dispatch_attempt(&self) -> super::runtime::DispatchAttemptIdV1 {
        self.dispatch_attempt
    }

    pub const fn replayed(&self) -> bool {
        self.replayed
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ProviderApplicationReleaseV1 {
    intent: EffectIntentIdV1,
    dispatch_attempt: super::runtime::DispatchAttemptIdV1,
    sealed_control_head: EffectIntentControlTokenV1,
    operation: SealedProviderOperationV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProviderOperationBindingV1 {
    run_id: RunIdV1,
    execution_boundary_commitment: [u8; 32],
    deadline: u64,
    application_envelope_commitment: [u8; 32],
    provider_operation_contract_commitment: [u8; 32],
    provider_scope_commitment: [u8; 32],
    provider_key_commitment: [u8; 32],
    credential_commitment: [u8; 32],
    semantic_operation_commitment: [u8; 32],
    payload_commitment: [u8; 32],
    target_commitment: [u8; 32],
}

#[derive(Debug, Eq, PartialEq)]
pub struct SealedProviderOperationV1 {
    binding: ProviderOperationBindingV1,
}

impl SealedProviderOperationV1 {
    pub const fn run_id(&self) -> RunIdV1 {
        self.binding.run_id
    }

    pub const fn execution_boundary_commitment(&self) -> [u8; 32] {
        self.binding.execution_boundary_commitment
    }

    pub const fn deadline(&self) -> u64 {
        self.binding.deadline
    }

    pub const fn application_envelope_commitment(&self) -> [u8; 32] {
        self.binding.application_envelope_commitment
    }

    pub const fn provider_operation_contract_commitment(&self) -> [u8; 32] {
        self.binding.provider_operation_contract_commitment
    }

    pub const fn provider_scope_commitment(&self) -> [u8; 32] {
        self.binding.provider_scope_commitment
    }

    pub const fn provider_key_commitment(&self) -> [u8; 32] {
        self.binding.provider_key_commitment
    }

    pub const fn credential_commitment(&self) -> [u8; 32] {
        self.binding.credential_commitment
    }

    pub const fn semantic_operation_commitment(&self) -> [u8; 32] {
        self.binding.semantic_operation_commitment
    }

    pub const fn payload_commitment(&self) -> [u8; 32] {
        self.binding.payload_commitment
    }

    pub const fn target_commitment(&self) -> [u8; 32] {
        self.binding.target_commitment
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct RunExecutionTimeReceiptV1 {
    run_id: RunIdV1,
    execution_boundary_commitment: [u8; 32],
    deadline: u64,
    accepted_h_time: u64,
    authority_acceptance_commitment: [u8; 32],
    receipt_commitment: [u8; 32],
}

impl RunExecutionTimeReceiptV1 {
    fn from_binding(
        run_id: RunIdV1,
        execution_boundary_commitment: [u8; 32],
        deadline: u64,
        accepted_h_time: u64,
        authority_acceptance_commitment: [u8; 32],
    ) -> Result<Self, ExecutionStoreErrorV1> {
        if execution_boundary_commitment == [0; 32]
            || deadline == 0
            || authority_acceptance_commitment == [0; 32]
        {
            return Err(ExecutionStoreErrorV1::InvalidRunExecutionTimeReceipt);
        }
        let mut receipt = Self {
            run_id,
            execution_boundary_commitment,
            deadline,
            accepted_h_time,
            authority_acceptance_commitment,
            receipt_commitment: [0; 32],
        };
        receipt.receipt_commitment = receipt.compute_commitment()?;
        Ok(receipt)
    }

    fn validate(
        &self,
        run_id: RunIdV1,
        execution_boundary_commitment: [u8; 32],
        deadline: u64,
    ) -> Result<(), ExecutionStoreErrorV1> {
        if self.run_id != run_id
            || self.execution_boundary_commitment != execution_boundary_commitment
            || self.deadline != deadline
            || self.authority_acceptance_commitment == [0; 32]
            || self.receipt_commitment != self.compute_commitment()?
        {
            return Err(ExecutionStoreErrorV1::InvalidRunExecutionTimeReceipt);
        }
        if self.accepted_h_time >= self.deadline {
            return Err(ExecutionStoreErrorV1::RunDeadlineExpired);
        }
        Ok(())
    }

    fn compute_commitment(&self) -> Result<[u8; 32], CborError> {
        hash(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.run-execution-time-receipt.v1")?,
            bytes(self.run_id.as_bytes()),
            bytes(&self.execution_boundary_commitment),
            CborValue::Unsigned(self.deadline),
            CborValue::Unsigned(self.accepted_h_time),
            bytes(&self.authority_acceptance_commitment),
        ]))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RunNoStartObservationChallengeV1 {
    run_id: RunIdV1,
    execution_boundary_commitment: [u8; 32],
    observed_at: u64,
    authority_acceptance_commitment: [u8; 32],
}

impl RunNoStartObservationChallengeV1 {
    pub const fn run_id(self) -> RunIdV1 {
        self.run_id
    }

    pub const fn execution_boundary_commitment(self) -> [u8; 32] {
        self.execution_boundary_commitment
    }

    pub const fn observed_at(self) -> u64 {
        self.observed_at
    }

    pub const fn authority_acceptance_commitment(self) -> [u8; 32] {
        self.authority_acceptance_commitment
    }
}

pub trait PinnedExecutionBoundaryObserverV1 {
    fn execution_boundary_commitment(&self) -> [u8; 32];
    fn observer_commitment(&self) -> [u8; 32];
    fn observe_definitely_not_started(
        &mut self,
        challenge: RunNoStartObservationChallengeV1,
    ) -> Option<[u8; 32]>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderApplicationFactV1 {
    Applied,
    NotApplied,
    Pending,
    Unknown,
    PartiallyApplied,
    Conflicted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderTransportObservationV1 {
    DefinitelyNotSent {
        authenticated_evidence_commitment: [u8; 32],
    },
    ResponseReceived {
        authenticated_evidence_commitment: [u8; 32],
        application_fact: ProviderApplicationFactV1,
    },
    AmbiguousTransport {
        authenticated_evidence_commitment: [u8; 32],
    },
}

pub trait PinnedProviderExecutorV1 {
    fn provider_operation_contract_commitment(&self) -> [u8; 32];
    fn provider_scope_commitment(&self) -> [u8; 32];
    fn provider_key_commitment(&self) -> [u8; 32];
    fn credential_commitment(&self) -> [u8; 32];
    fn hidden_retries_disabled(&self) -> bool;
    fn execute_once(
        &mut self,
        operation: &SealedProviderOperationV1,
    ) -> ProviderTransportObservationV1;
}

impl ProviderApplicationReleaseV1 {
    pub const fn intent(&self) -> EffectIntentIdV1 {
        self.intent
    }

    pub const fn dispatch_attempt(&self) -> super::runtime::DispatchAttemptIdV1 {
        self.dispatch_attempt
    }

    pub const fn sealed_control_head(&self) -> EffectIntentControlTokenV1 {
        self.sealed_control_head
    }

    #[cfg(test)]
    pub(crate) fn execution_time_receipt(
        &self,
        accepted_h_time: u64,
        authority_acceptance_commitment: [u8; 32],
    ) -> Result<RunExecutionTimeReceiptV1, ExecutionStoreErrorV1> {
        RunExecutionTimeReceiptV1::from_binding(
            self.operation.binding.run_id,
            self.operation.binding.execution_boundary_commitment,
            self.operation.binding.deadline,
            accepted_h_time,
            authority_acceptance_commitment,
        )
    }

    fn execute_once(
        self,
        execution_time: RunExecutionTimeReceiptV1,
        executor: &mut impl PinnedProviderExecutorV1,
    ) -> Result<ActiveStoreEffectTerminalDraftV1, ExecutionStoreErrorV1> {
        execution_time.validate(
            self.operation.binding.run_id,
            self.operation.binding.execution_boundary_commitment,
            self.operation.binding.deadline,
        )?;
        if executor.provider_operation_contract_commitment()
            != self
                .operation
                .binding
                .provider_operation_contract_commitment
            || executor.provider_scope_commitment()
                != self.operation.binding.provider_scope_commitment
            || executor.provider_key_commitment() != self.operation.binding.provider_key_commitment
            || executor.credential_commitment() != self.operation.binding.credential_commitment
            || !executor.hidden_retries_disabled()
        {
            return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
        }
        let observation = executor.execute_once(&self.operation);
        let outcome = match observation {
            ProviderTransportObservationV1::DefinitelyNotSent {
                authenticated_evidence_commitment,
            } => EffectDispatchOutcomePayloadV1::DefinitelyNotSent {
                evidence_commitment: authenticated_evidence_commitment,
            },
            ProviderTransportObservationV1::ResponseReceived {
                authenticated_evidence_commitment,
                application_fact,
            } => EffectDispatchOutcomePayloadV1::ResponseReceived {
                evidence_commitment: authenticated_evidence_commitment,
                classification: match application_fact {
                    ProviderApplicationFactV1::Applied => {
                        super::withdrawal::RemoteClassificationV1::ConfirmedApplied
                    }
                    ProviderApplicationFactV1::NotApplied => {
                        super::withdrawal::RemoteClassificationV1::ConfirmedNotApplied
                    }
                    ProviderApplicationFactV1::Pending => {
                        super::withdrawal::RemoteClassificationV1::Pending
                    }
                    ProviderApplicationFactV1::Unknown => {
                        super::withdrawal::RemoteClassificationV1::InDoubt
                    }
                    ProviderApplicationFactV1::PartiallyApplied => {
                        super::withdrawal::RemoteClassificationV1::PartiallyApplied
                    }
                    ProviderApplicationFactV1::Conflicted => {
                        super::withdrawal::RemoteClassificationV1::Conflicted
                    }
                },
            },
            ProviderTransportObservationV1::AmbiguousTransport {
                authenticated_evidence_commitment,
            } => EffectDispatchOutcomePayloadV1::AmbiguousTransport {
                evidence_commitment: authenticated_evidence_commitment,
            },
        };
        if effect_dispatch_outcome_evidence_commitment(outcome) == [0; 32] {
            return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
        }
        let commitment = hash(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.provider-dispatch-outcome-proof.v1")?,
            bytes(self.intent.as_bytes()),
            bytes(self.dispatch_attempt.as_bytes()),
            bytes(self.sealed_control_head.as_bytes()),
            provider_operation_binding_value(&self.operation.binding)?,
            effect_dispatch_outcome_value(outcome),
        ]))?;
        let operation = self.operation.binding;
        Ok(ActiveStoreEffectTerminalDraftV1 {
            outcome,
            provider_proof: Some(ProviderDispatchOutcomeProofV1 {
                intent: self.intent,
                dispatch_attempt: self.dispatch_attempt,
                sealed_control_head: self.sealed_control_head,
                operation,
                outcome_commitment: commitment,
            }),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProviderDispatchOutcomeProofV1 {
    intent: EffectIntentIdV1,
    dispatch_attempt: super::runtime::DispatchAttemptIdV1,
    sealed_control_head: EffectIntentControlTokenV1,
    operation: ProviderOperationBindingV1,
    outcome_commitment: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveStoreEffectOriginationOutcomeV1 {
    store_head: StoreHeadV1,
    intent: EffectIntentIdV1,
    control_head: EffectIntentControlTokenV1,
    control_revision: EffectIntentControlTokenV1,
    writer_term: EffectIntentControlTokenV1,
    dispatch_attempt: super::runtime::DispatchAttemptIdV1,
    replayed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveStoreEffectSnapshotV1 {
    state_binding: ExecutionStoreStateBindingV1,
    intent: EffectIntentV1,
    control_revision: EffectIntentControlRevisionV1,
    writer_term: EffectIntentControlWriterTermV1,
    control_head: EffectIntentControlHeadV1,
    dispatch: EffectDispatchAttemptV1,
    reconciliation: Option<EffectReconciliationAttemptV1>,
    reconciliation_evaluated_classification: Option<super::withdrawal::RemoteClassificationV1>,
    reconciliation_authorization: Option<AuthorizedExecutionActionV1>,
    reconciliation_execution_authority: Option<ExecutionAuthorityV1>,
}

impl ActiveStoreEffectSnapshotV1 {
    pub const fn state_binding(&self) -> &ExecutionStoreStateBindingV1 {
        &self.state_binding
    }

    pub const fn intent(&self) -> &EffectIntentV1 {
        &self.intent
    }

    pub const fn control_revision(&self) -> &EffectIntentControlRevisionV1 {
        &self.control_revision
    }

    pub const fn writer_term(&self) -> EffectIntentControlWriterTermV1 {
        self.writer_term
    }

    pub const fn control_head(&self) -> &EffectIntentControlHeadV1 {
        &self.control_head
    }

    pub const fn dispatch(&self) -> &EffectDispatchAttemptV1 {
        &self.dispatch
    }

    pub const fn reconciliation(&self) -> Option<&EffectReconciliationAttemptV1> {
        self.reconciliation.as_ref()
    }

    pub fn reconciliation_terminal_draft(
        &self,
    ) -> Result<ActiveStoreEffectReconciliationTerminalDraftV1, ExecutionStoreErrorV1> {
        let classification = self
            .reconciliation_evaluated_classification
            .ok_or(ExecutionStoreErrorV1::PublicationBindingMismatch)?;
        if self
            .reconciliation
            .as_ref()
            .and_then(EffectReconciliationAttemptV1::read_usage)
            .is_none()
        {
            return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
        }
        Ok(ActiveStoreEffectReconciliationTerminalDraftV1::new(
            classification,
        ))
    }

    fn reconciliation_authorization(&self) -> Option<&AuthorizedExecutionActionV1> {
        self.reconciliation_authorization.as_ref()
    }

    fn reconciliation_execution_authority(&self) -> Option<&ExecutionAuthorityV1> {
        self.reconciliation_execution_authority.as_ref()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActiveStoreEffectSealDraftV1 {
    seal_commitment: [u8; 32],
}

impl ActiveStoreEffectSealDraftV1 {
    pub fn new(seal_commitment: [u8; 32]) -> Result<Self, ExecutionStoreErrorV1> {
        if seal_commitment == [0; 32] {
            return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
        }
        Ok(Self { seal_commitment })
    }

    pub const fn seal_commitment(self) -> [u8; 32] {
        self.seal_commitment
    }

    fn payload_value(self) -> Result<CborValue, CborError> {
        Ok(CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-dispatch-seal-payload.v1")?,
            bytes(&self.seal_commitment),
        ]))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveStoreEffectSealPublicationV1 {
    request: CanonicalExecutionActionRequestV1,
    authority: ExecutionAuthorityV1,
    snapshot: ActiveStoreEffectSnapshotV1,
    draft: ActiveStoreEffectSealDraftV1,
}

impl ActiveStoreEffectSealPublicationV1 {
    pub fn new(
        request: CanonicalExecutionActionRequestV1,
        authority: impl Into<ExecutionAuthorityV1>,
        snapshot: ActiveStoreEffectSnapshotV1,
        draft: ActiveStoreEffectSealDraftV1,
    ) -> Result<Self, ExecutionStoreErrorV1> {
        let authority = authority.into();
        if request.action() != snapshot.intent.origin().outcome_action()? {
            return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
        }
        validate_request_values(
            &request,
            &effect_intent_subject_value(&snapshot.intent)?,
            &snapshot.state_binding.canonical_value()?,
            &draft.payload_value()?,
        )?;
        validate_execution_authority_binding(&request, &authority)?;
        validate_effect_authority_origin(&authority, snapshot.intent.origin().kind())?;
        Ok(Self {
            request,
            authority,
            snapshot,
            draft,
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ActiveStoreEffectSealOutcomeV1 {
    store_head: StoreHeadV1,
    intent: EffectIntentIdV1,
    control_head: EffectIntentControlTokenV1,
    control_revision: EffectIntentControlTokenV1,
    replayed: bool,
    provider_release: Option<ProviderApplicationReleaseV1>,
}

impl ActiveStoreEffectSealOutcomeV1 {
    pub const fn store_head(&self) -> &StoreHeadV1 {
        &self.store_head
    }

    pub const fn intent(&self) -> EffectIntentIdV1 {
        self.intent
    }

    pub const fn control_head(&self) -> EffectIntentControlTokenV1 {
        self.control_head
    }

    pub const fn control_revision(&self) -> EffectIntentControlTokenV1 {
        self.control_revision
    }

    pub const fn replayed(&self) -> bool {
        self.replayed
    }

    pub fn take_provider_release(&mut self) -> Option<ProviderApplicationReleaseV1> {
        self.provider_release.take()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveStoreEffectTerminalDraftV1 {
    outcome: EffectDispatchOutcomePayloadV1,
    provider_proof: Option<ProviderDispatchOutcomeProofV1>,
}

impl ActiveStoreEffectTerminalDraftV1 {
    pub fn new(outcome: EffectDispatchOutcomePayloadV1) -> Result<Self, ExecutionStoreErrorV1> {
        if !matches!(
            outcome,
            EffectDispatchOutcomePayloadV1::LocallyRejected { .. }
        ) {
            return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
        }
        Ok(Self {
            outcome,
            provider_proof: None,
        })
    }

    pub const fn outcome(&self) -> EffectDispatchOutcomePayloadV1 {
        self.outcome
    }

    fn payload_value(&self) -> Result<CborValue, CborError> {
        let provider_proof = self
            .provider_proof
            .as_ref()
            .map(|proof| {
                Ok(CborValue::Array(vec![
                    bytes(proof.intent.as_bytes()),
                    bytes(proof.dispatch_attempt.as_bytes()),
                    bytes(proof.sealed_control_head.as_bytes()),
                    provider_operation_binding_value(&proof.operation)?,
                    bytes(&proof.outcome_commitment),
                ]))
            })
            .transpose()?;
        Ok(CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-dispatch-terminal-payload.v1")?,
            effect_dispatch_outcome_value(self.outcome),
            CborValue::optional(provider_proof),
        ]))
    }

    fn validate_for_snapshot(
        &self,
        snapshot: &ActiveStoreEffectSnapshotV1,
    ) -> Result<(), ExecutionStoreErrorV1> {
        match &self.provider_proof {
            None if matches!(
                self.outcome,
                EffectDispatchOutcomePayloadV1::LocallyRejected { .. }
            ) =>
            {
                Ok(())
            }
            Some(proof)
                if !matches!(
                    self.outcome,
                    EffectDispatchOutcomePayloadV1::LocallyRejected { .. }
                ) && proof.intent == snapshot.intent.id()
                    && proof.dispatch_attempt == snapshot.dispatch.attempt().id()
                    && proof.sealed_control_head == snapshot.control_head.id()
                    && proof.operation
                        == sealed_provider_operation_from_snapshot(snapshot)?.binding
                    && proof.outcome_commitment
                        == hash(&CborValue::Array(vec![
                            CborValue::text("maestro.vnext.provider-dispatch-outcome-proof.v1")?,
                            bytes(proof.intent.as_bytes()),
                            bytes(proof.dispatch_attempt.as_bytes()),
                            bytes(proof.sealed_control_head.as_bytes()),
                            provider_operation_binding_value(&proof.operation)?,
                            effect_dispatch_outcome_value(self.outcome),
                        ]))? =>
            {
                Ok(())
            }
            _ => Err(ExecutionStoreErrorV1::PublicationBindingMismatch),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveStoreEffectTerminalPublicationV1 {
    request: CanonicalExecutionActionRequestV1,
    authority: ExecutionAuthorityV1,
    snapshot: ActiveStoreEffectSnapshotV1,
    draft: ActiveStoreEffectTerminalDraftV1,
}

impl ActiveStoreEffectTerminalPublicationV1 {
    pub fn new(
        request: CanonicalExecutionActionRequestV1,
        authority: impl Into<ExecutionAuthorityV1>,
        snapshot: ActiveStoreEffectSnapshotV1,
        draft: ActiveStoreEffectTerminalDraftV1,
    ) -> Result<Self, ExecutionStoreErrorV1> {
        let authority = authority.into();
        if request.action() != snapshot.intent.origin().outcome_action()? {
            return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
        }
        draft.validate_for_snapshot(&snapshot)?;
        validate_request_values(
            &request,
            &effect_intent_subject_value(&snapshot.intent)?,
            &snapshot.state_binding.canonical_value()?,
            &draft.payload_value()?,
        )?;
        validate_execution_authority_binding(&request, &authority)?;
        validate_effect_authority_origin(&authority, snapshot.intent.origin().kind())?;
        Ok(Self {
            request,
            authority,
            snapshot,
            draft,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveStoreEffectTerminalOutcomeV1 {
    store_head: StoreHeadV1,
    intent: EffectIntentIdV1,
    control_head: EffectIntentControlTokenV1,
    control_revision: EffectIntentControlTokenV1,
    classification: super::withdrawal::RemoteClassificationV1,
    replayed: bool,
}

impl ActiveStoreEffectTerminalOutcomeV1 {
    pub const fn store_head(&self) -> &StoreHeadV1 {
        &self.store_head
    }

    pub const fn intent(&self) -> EffectIntentIdV1 {
        self.intent
    }

    pub const fn control_head(&self) -> EffectIntentControlTokenV1 {
        self.control_head
    }

    pub const fn control_revision(&self) -> EffectIntentControlTokenV1 {
        self.control_revision
    }

    pub const fn classification(&self) -> super::withdrawal::RemoteClassificationV1 {
        self.classification
    }

    pub const fn replayed(&self) -> bool {
        self.replayed
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[expect(
    clippy::large_enum_variant,
    reason = "the closed recovery draft keeps the exact seal-or-terminal payload inline as one publication input"
)]
pub enum ActiveStoreEffectRecoverReservedDraftV1 {
    Seal(ActiveStoreEffectSealDraftV1),
    Reject(ActiveStoreEffectTerminalDraftV1),
}

impl ActiveStoreEffectRecoverReservedDraftV1 {
    pub const fn seal(draft: ActiveStoreEffectSealDraftV1) -> Self {
        Self::Seal(draft)
    }

    pub fn reject(draft: ActiveStoreEffectTerminalDraftV1) -> Result<Self, ExecutionStoreErrorV1> {
        if !matches!(
            draft.outcome(),
            EffectDispatchOutcomePayloadV1::LocallyRejected { .. }
        ) {
            return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
        }
        Ok(Self::Reject(draft))
    }

    fn payload_value(&self) -> Result<CborValue, CborError> {
        match self {
            Self::Seal(draft) => draft.payload_value(),
            Self::Reject(draft) => draft.payload_value(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveStoreEffectRecoverReservedPublicationV1 {
    request: CanonicalExecutionActionRequestV1,
    authority: ExecutionAuthorityV1,
    snapshot: ActiveStoreEffectSnapshotV1,
    draft: ActiveStoreEffectRecoverReservedDraftV1,
}

impl ActiveStoreEffectRecoverReservedPublicationV1 {
    pub fn new(
        request: CanonicalExecutionActionRequestV1,
        authority: impl Into<ExecutionAuthorityV1>,
        snapshot: ActiveStoreEffectSnapshotV1,
        draft: ActiveStoreEffectRecoverReservedDraftV1,
    ) -> Result<Self, ExecutionStoreErrorV1> {
        let authority = authority.into();
        if request.action() != snapshot.intent.origin().reservation_action()? {
            return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
        }
        validate_request_values(
            &request,
            &effect_intent_subject_value(&snapshot.intent)?,
            &snapshot.state_binding.canonical_value()?,
            &draft.payload_value()?,
        )?;
        validate_execution_authority_binding(&request, &authority)?;
        validate_effect_authority_origin(&authority, snapshot.intent.origin().kind())?;
        Ok(Self {
            request,
            authority,
            snapshot,
            draft,
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
#[expect(
    clippy::large_enum_variant,
    reason = "the closed recovery outcome owns exactly one complete seal-or-terminal publication result"
)]
pub enum ActiveStoreEffectRecoverReservedOutcomeV1 {
    Sealed(ActiveStoreEffectSealOutcomeV1),
    Rejected(ActiveStoreEffectTerminalOutcomeV1),
}

impl ActiveStoreEffectRecoverReservedOutcomeV1 {
    pub const fn replayed(&self) -> bool {
        match self {
            Self::Sealed(outcome) => outcome.replayed(),
            Self::Rejected(outcome) => outcome.replayed(),
        }
    }

    pub fn take_provider_release(&mut self) -> Option<ProviderApplicationReleaseV1> {
        match self {
            Self::Sealed(outcome) => outcome.take_provider_release(),
            Self::Rejected(_) => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveStoreEffectReconciliationBeginDraftV1 {
    read_plan: EffectReconciliationReadPlanV1,
    read_run: RunReservationV1,
}

impl ActiveStoreEffectReconciliationBeginDraftV1 {
    pub const fn new(
        read_plan: EffectReconciliationReadPlanV1,
        read_run: RunReservationV1,
    ) -> Self {
        Self {
            read_plan,
            read_run,
        }
    }

    pub const fn read_plan(&self) -> EffectReconciliationReadPlanV1 {
        self.read_plan
    }

    pub const fn read_run(&self) -> &RunReservationV1 {
        &self.read_run
    }

    fn payload_value(&self) -> Result<CborValue, ExecutionStoreErrorV1> {
        Ok(CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-reconciliation-begin-payload.v1")?,
            self.read_plan.canonical_value()?,
            run_reservation_store_value(&self.read_run),
        ]))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveStoreEffectReconciliationBeginPublicationV1 {
    request: CanonicalExecutionActionRequestV1,
    authority: ExecutionAuthorityV1,
    snapshot: ActiveStoreEffectSnapshotV1,
    draft: ActiveStoreEffectReconciliationBeginDraftV1,
}

impl ActiveStoreEffectReconciliationBeginPublicationV1 {
    pub fn new(
        request: CanonicalExecutionActionRequestV1,
        authority: impl Into<ExecutionAuthorityV1>,
        snapshot: ActiveStoreEffectSnapshotV1,
        draft: ActiveStoreEffectReconciliationBeginDraftV1,
    ) -> Result<Self, ExecutionStoreErrorV1> {
        let authority = authority.into();
        if request.action() != snapshot.intent.origin().reconciliation_action()? {
            return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
        }
        validate_request_values(
            &request,
            &effect_intent_subject_value(&snapshot.intent)?,
            &snapshot.state_binding.canonical_value()?,
            &draft.payload_value()?,
        )?;
        validate_execution_authority_binding(&request, &authority)?;
        validate_effect_authority_origin(&authority, snapshot.intent.origin().kind())?;
        Ok(Self {
            request,
            authority,
            snapshot,
            draft,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ReconciliationReadBindingV1 {
    run_id: RunIdV1,
    execution_boundary_commitment: [u8; 32],
    deadline: u64,
    intent: EffectIntentIdV1,
    control_head: EffectIntentControlTokenV1,
    read_plan: EffectReconciliationReadPlanV1,
}

#[derive(Debug, Eq, PartialEq)]
pub struct SealedReconciliationReadV1 {
    binding: ReconciliationReadBindingV1,
}

impl SealedReconciliationReadV1 {
    pub const fn run_id(&self) -> RunIdV1 {
        self.binding.run_id
    }

    pub const fn execution_boundary_commitment(&self) -> [u8; 32] {
        self.binding.execution_boundary_commitment
    }

    pub const fn deadline(&self) -> u64 {
        self.binding.deadline
    }

    pub const fn intent(&self) -> EffectIntentIdV1 {
        self.binding.intent
    }

    pub const fn control_head(&self) -> EffectIntentControlTokenV1 {
        self.binding.control_head
    }

    pub const fn read_plan(&self) -> EffectReconciliationReadPlanV1 {
        self.binding.read_plan
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReconciliationReadObservationV1 {
    pub usage: EffectReconciliationReadUsageV1,
    pub application_fact: ProviderApplicationFactV1,
}

pub trait PinnedReconciliationReaderV1 {
    fn provider_commitment(&self) -> [u8; 32];
    fn account_commitment(&self) -> [u8; 32];
    fn target_commitment(&self) -> [u8; 32];
    fn correlation_commitment(&self) -> [u8; 32];
    fn credential_commitment(&self) -> [u8; 32];
    fn visibility_commitment(&self) -> [u8; 32];
    fn query_commitment(&self) -> [u8; 32];
    fn evaluator_commitment(&self) -> [u8; 32];
    fn hidden_retries_disabled(&self) -> bool;
    fn read_once(&mut self, read: &SealedReconciliationReadV1) -> ReconciliationReadObservationV1;
}

#[derive(Debug, Eq, PartialEq)]
pub struct ReconciliationReadReleaseV1 {
    read: SealedReconciliationReadV1,
}

impl ReconciliationReadReleaseV1 {
    #[cfg(test)]
    pub(crate) fn execution_time_receipt(
        &self,
        accepted_h_time: u64,
        authority_acceptance_commitment: [u8; 32],
    ) -> Result<RunExecutionTimeReceiptV1, ExecutionStoreErrorV1> {
        RunExecutionTimeReceiptV1::from_binding(
            self.read.binding.run_id,
            self.read.binding.execution_boundary_commitment,
            self.read.binding.deadline,
            accepted_h_time,
            authority_acceptance_commitment,
        )
    }

    fn execute_once(
        self,
        execution_time: RunExecutionTimeReceiptV1,
        reader: &mut impl PinnedReconciliationReaderV1,
    ) -> Result<ActiveStoreEffectReconciliationReadDraftV1, ExecutionStoreErrorV1> {
        execution_time.validate(
            self.read.binding.run_id,
            self.read.binding.execution_boundary_commitment,
            self.read.binding.deadline,
        )?;
        let plan = self.read.binding.read_plan;
        if reader.provider_commitment() != plan.provider_commitment()
            || reader.account_commitment() != plan.account_commitment()
            || reader.target_commitment() != plan.target_commitment()
            || reader.correlation_commitment() != plan.correlation_commitment()
            || reader.credential_commitment() != plan.credential_commitment()
            || reader.visibility_commitment() != plan.visibility_commitment()
            || reader.query_commitment() != plan.query_commitment()
            || reader.evaluator_commitment() != plan.evaluator_commitment()
            || !reader.hidden_retries_disabled()
        {
            return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
        }
        let observation = reader.read_once(&self.read);
        validate_reconciliation_read_observation(plan, observation)?;
        let classification = provider_application_fact_classification(observation.application_fact);
        let read = self.read.binding;
        let proof_commitment =
            reconciliation_read_proof_commitment(&read, observation.usage, classification)?;
        Ok(ActiveStoreEffectReconciliationReadDraftV1 {
            usage: observation.usage,
            classification,
            proof: ReconciliationReadProofV1 {
                read,
                proof_commitment,
            },
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ReconciliationReadProofV1 {
    read: ReconciliationReadBindingV1,
    proof_commitment: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActiveStoreEffectReconciliationReadDraftV1 {
    usage: EffectReconciliationReadUsageV1,
    classification: super::withdrawal::RemoteClassificationV1,
    proof: ReconciliationReadProofV1,
}

impl ActiveStoreEffectReconciliationReadDraftV1 {
    pub const fn usage(self) -> EffectReconciliationReadUsageV1 {
        self.usage
    }

    pub const fn classification(self) -> super::withdrawal::RemoteClassificationV1 {
        self.classification
    }

    fn payload_value(self) -> Result<CborValue, CborError> {
        Ok(CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-reconciliation-read-payload.v1")?,
            reconciliation_read_usage_store_value(self.usage),
            CborValue::Unsigned(remote_classification_store_tag(self.classification)),
            bytes(&self.proof.proof_commitment),
        ]))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveStoreEffectReconciliationReadPublicationV1 {
    snapshot: ActiveStoreEffectSnapshotV1,
    draft: ActiveStoreEffectReconciliationReadDraftV1,
}

impl ActiveStoreEffectReconciliationReadPublicationV1 {
    pub fn new(
        snapshot: ActiveStoreEffectSnapshotV1,
        draft: ActiveStoreEffectReconciliationReadDraftV1,
    ) -> Result<Self, ExecutionStoreErrorV1> {
        if snapshot.reconciliation.is_none()
            || snapshot.reconciliation_authorization().is_none()
            || snapshot.reconciliation_execution_authority().is_none()
        {
            return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
        }
        validate_reconciliation_read_draft_for_snapshot(&snapshot, draft)?;
        Ok(Self { snapshot, draft })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActiveStoreEffectReconciliationTerminalDraftV1 {
    classification: super::withdrawal::RemoteClassificationV1,
}

impl ActiveStoreEffectReconciliationTerminalDraftV1 {
    const fn new(classification: super::withdrawal::RemoteClassificationV1) -> Self {
        Self { classification }
    }

    pub const fn classification(self) -> super::withdrawal::RemoteClassificationV1 {
        self.classification
    }

    fn payload_value(self, read_result_commitment: [u8; 32]) -> Result<CborValue, CborError> {
        Ok(CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-reconciliation-terminal-payload.v1")?,
            CborValue::Unsigned(remote_classification_store_tag(self.classification)),
            bytes(&read_result_commitment),
        ]))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveStoreEffectReconciliationTerminalPublicationV1 {
    snapshot: ActiveStoreEffectSnapshotV1,
    draft: ActiveStoreEffectReconciliationTerminalDraftV1,
}

impl ActiveStoreEffectReconciliationTerminalPublicationV1 {
    pub fn new(
        snapshot: ActiveStoreEffectSnapshotV1,
        draft: ActiveStoreEffectReconciliationTerminalDraftV1,
    ) -> Result<Self, ExecutionStoreErrorV1> {
        let read_result_commitment = snapshot
            .reconciliation
            .as_ref()
            .and_then(EffectReconciliationAttemptV1::read_usage)
            .map(|usage| usage.result_commitment)
            .ok_or(ExecutionStoreErrorV1::PublicationBindingMismatch)?;
        if snapshot.reconciliation_authorization().is_none()
            || snapshot.reconciliation_execution_authority().is_none()
            || snapshot.reconciliation_evaluated_classification != Some(draft.classification())
        {
            return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
        }
        let _ = draft.payload_value(read_result_commitment)?;
        Ok(Self { snapshot, draft })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActiveStoreEffectWithdrawalDraftV1;

impl ActiveStoreEffectWithdrawalDraftV1 {
    pub const fn new() -> Self {
        Self
    }

    fn payload_value(self) -> Result<CborValue, CborError> {
        CborValue::text("maestro.vnext.effect-withdrawal-payload.v1")
    }
}

impl Default for ActiveStoreEffectWithdrawalDraftV1 {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveStoreEffectWithdrawalPublicationV1 {
    request: CanonicalExecutionActionRequestV1,
    authority: ExecutionAuthorityV1,
    snapshot: ActiveStoreEffectSnapshotV1,
    draft: ActiveStoreEffectWithdrawalDraftV1,
}

impl ActiveStoreEffectWithdrawalPublicationV1 {
    pub fn new(
        request: CanonicalExecutionActionRequestV1,
        authority: impl Into<ExecutionAuthorityV1>,
        snapshot: ActiveStoreEffectSnapshotV1,
        draft: ActiveStoreEffectWithdrawalDraftV1,
    ) -> Result<Self, ExecutionStoreErrorV1> {
        let authority = authority.into();
        if request.action() != snapshot.intent.origin().withdrawal_action()? {
            return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
        }
        validate_request_values(
            &request,
            &effect_intent_subject_value(&snapshot.intent)?,
            &snapshot.state_binding.canonical_value()?,
            &draft.payload_value()?,
        )?;
        validate_execution_authority_binding(&request, &authority)?;
        validate_effect_authority_origin(&authority, snapshot.intent.origin().kind())?;
        Ok(Self {
            request,
            authority,
            snapshot,
            draft,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveStoreEffectWithdrawalOutcomeV1 {
    store_head: StoreHeadV1,
    intent: EffectIntentIdV1,
    control_head: EffectIntentControlTokenV1,
    control_revision: EffectIntentControlTokenV1,
    replayed: bool,
}

impl ActiveStoreEffectWithdrawalOutcomeV1 {
    pub const fn store_head(&self) -> &StoreHeadV1 {
        &self.store_head
    }

    pub const fn intent(&self) -> EffectIntentIdV1 {
        self.intent
    }

    pub const fn control_head(&self) -> EffectIntentControlTokenV1 {
        self.control_head
    }

    pub const fn control_revision(&self) -> EffectIntentControlTokenV1 {
        self.control_revision
    }

    pub const fn replayed(&self) -> bool {
        self.replayed
    }

    pub const fn provider_io_operations(&self) -> u8 {
        0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActiveStoreEffectHealthDraftV1 {
    RecoverSealedInDoubt,
    MarkRecoveryRequired,
    MarkIntegrityBlocked,
}

impl ActiveStoreEffectHealthDraftV1 {
    fn payload_value(self) -> Result<CborValue, CborError> {
        Ok(CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-health-payload.v1")?,
            CborValue::Unsigned(match self {
                Self::RecoverSealedInDoubt => 1,
                Self::MarkRecoveryRequired => 2,
                Self::MarkIntegrityBlocked => 3,
            }),
        ]))
    }

    fn control_need(
        self,
        action_request_id: ActionRequestIdV1,
    ) -> Result<EffectControlTransitionNeedV1, ExecutionStoreErrorV1> {
        Ok(match self {
            Self::RecoverSealedInDoubt => {
                return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
            }
            Self::MarkRecoveryRequired => {
                EffectControlTransitionNeedV1::MarkRecoveryRequired { action_request_id }
            }
            Self::MarkIntegrityBlocked => {
                EffectControlTransitionNeedV1::MarkIntegrityBlocked { action_request_id }
            }
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveStoreEffectHealthPublicationV1 {
    request: CanonicalExecutionActionRequestV1,
    authority: ExecutionAuthorityV1,
    snapshot: ActiveStoreEffectSnapshotV1,
    draft: ActiveStoreEffectHealthDraftV1,
}

impl ActiveStoreEffectHealthPublicationV1 {
    pub fn new(
        request: CanonicalExecutionActionRequestV1,
        authority: impl Into<ExecutionAuthorityV1>,
        snapshot: ActiveStoreEffectSnapshotV1,
        draft: ActiveStoreEffectHealthDraftV1,
    ) -> Result<Self, ExecutionStoreErrorV1> {
        let authority = authority.into();
        if request.action() != snapshot.intent.origin().reconciliation_action()? {
            return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
        }
        validate_request_values(
            &request,
            &effect_intent_subject_value(&snapshot.intent)?,
            &snapshot.state_binding.canonical_value()?,
            &draft.payload_value()?,
        )?;
        validate_execution_authority_binding(&request, &authority)?;
        validate_effect_authority_origin(&authority, snapshot.intent.origin().kind())?;
        if draft == ActiveStoreEffectHealthDraftV1::RecoverSealedInDoubt {
            if snapshot.control_revision.live_attempt().is_none()
                || snapshot.control_revision.live_dispatch()
                    != super::withdrawal::EffectIntentLiveDispatchV1::Sealed
                || snapshot.control_revision.classification()
                    != super::withdrawal::RemoteClassificationV1::InDoubt
                || snapshot.control_revision.runs_closed()
            {
                return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
            }
        } else {
            let _ = draft.control_need(request.request_id())?;
        }
        Ok(Self {
            request,
            authority,
            snapshot,
            draft,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveStoreEffectHealthOutcomeV1 {
    store_head: StoreHeadV1,
    intent: EffectIntentIdV1,
    control_head: EffectIntentControlTokenV1,
    control_revision: EffectIntentControlTokenV1,
    health: EffectIntentControlHealthV1,
    replayed: bool,
}

impl ActiveStoreEffectHealthOutcomeV1 {
    pub const fn store_head(&self) -> &StoreHeadV1 {
        &self.store_head
    }

    pub const fn intent(&self) -> EffectIntentIdV1 {
        self.intent
    }

    pub const fn control_head(&self) -> EffectIntentControlTokenV1 {
        self.control_head
    }

    pub const fn control_revision(&self) -> EffectIntentControlTokenV1 {
        self.control_revision
    }

    pub const fn health(&self) -> EffectIntentControlHealthV1 {
        self.health
    }

    pub const fn replayed(&self) -> bool {
        self.replayed
    }

    pub const fn provider_io_operations(&self) -> u8 {
        0
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ActiveStoreEffectWriterHandoffDraftV1;

impl ActiveStoreEffectWriterHandoffDraftV1 {
    pub const fn new() -> Self {
        Self
    }

    fn payload_value(self) -> Result<CborValue, CborError> {
        CborValue::text("maestro.vnext.effect-writer-handoff-payload.v1")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveStoreEffectWriterHandoffPublicationV1 {
    request: CanonicalExecutionActionRequestV1,
    authority: ExecutionAuthorityV1,
    snapshot: ActiveStoreEffectSnapshotV1,
    draft: ActiveStoreEffectWriterHandoffDraftV1,
}

impl ActiveStoreEffectWriterHandoffPublicationV1 {
    pub fn new(
        request: CanonicalExecutionActionRequestV1,
        authority: impl Into<ExecutionAuthorityV1>,
        snapshot: ActiveStoreEffectSnapshotV1,
        draft: ActiveStoreEffectWriterHandoffDraftV1,
    ) -> Result<Self, ExecutionStoreErrorV1> {
        let authority = authority.into();
        if request.action() != effect_writer_handoff_action(&snapshot)? {
            return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
        }
        validate_request_values(
            &request,
            &effect_intent_subject_value(&snapshot.intent)?,
            &snapshot.state_binding.canonical_value()?,
            &draft.payload_value()?,
        )?;
        validate_execution_authority_binding(&request, &authority)?;
        validate_effect_authority_origin(&authority, snapshot.intent.origin().kind())?;
        Ok(Self {
            request,
            authority,
            snapshot,
            draft,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveStoreEffectWriterHandoffOutcomeV1 {
    store_head: StoreHeadV1,
    intent: EffectIntentIdV1,
    control_head: EffectIntentControlTokenV1,
    control_revision: EffectIntentControlTokenV1,
    writer_term: EffectIntentControlTokenV1,
    fencing_receipt: EffectIntentControlTokenV1,
    replayed: bool,
}

impl ActiveStoreEffectWriterHandoffOutcomeV1 {
    pub const fn store_head(&self) -> &StoreHeadV1 {
        &self.store_head
    }

    pub const fn intent(&self) -> EffectIntentIdV1 {
        self.intent
    }

    pub const fn control_head(&self) -> EffectIntentControlTokenV1 {
        self.control_head
    }

    pub const fn control_revision(&self) -> EffectIntentControlTokenV1 {
        self.control_revision
    }

    pub const fn writer_term(&self) -> EffectIntentControlTokenV1 {
        self.writer_term
    }

    pub const fn fencing_receipt(&self) -> EffectIntentControlTokenV1 {
        self.fencing_receipt
    }

    pub const fn replayed(&self) -> bool {
        self.replayed
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ActiveStoreEffectReconciliationOutcomeV1 {
    store_head: StoreHeadV1,
    intent: EffectIntentIdV1,
    control_head: EffectIntentControlTokenV1,
    control_revision: EffectIntentControlTokenV1,
    classification: super::withdrawal::RemoteClassificationV1,
    replayed: bool,
    read_release: Option<ReconciliationReadReleaseV1>,
}

impl ActiveStoreEffectReconciliationOutcomeV1 {
    pub const fn store_head(&self) -> &StoreHeadV1 {
        &self.store_head
    }

    pub const fn intent(&self) -> EffectIntentIdV1 {
        self.intent
    }

    pub const fn control_head(&self) -> EffectIntentControlTokenV1 {
        self.control_head
    }

    pub const fn control_revision(&self) -> EffectIntentControlTokenV1 {
        self.control_revision
    }

    pub const fn classification(&self) -> super::withdrawal::RemoteClassificationV1 {
        self.classification
    }

    pub const fn replayed(&self) -> bool {
        self.replayed
    }

    pub fn take_read_release(&mut self) -> Option<ReconciliationReadReleaseV1> {
        self.read_release.take()
    }
}

impl ActiveStoreEffectOriginationOutcomeV1 {
    pub const fn store_head(&self) -> &StoreHeadV1 {
        &self.store_head
    }

    pub const fn intent(&self) -> EffectIntentIdV1 {
        self.intent
    }

    pub const fn control_head(&self) -> EffectIntentControlTokenV1 {
        self.control_head
    }

    pub const fn control_revision(&self) -> EffectIntentControlTokenV1 {
        self.control_revision
    }

    pub const fn writer_term(&self) -> EffectIntentControlTokenV1 {
        self.writer_term
    }

    pub const fn dispatch_attempt(&self) -> super::runtime::DispatchAttemptIdV1 {
        self.dispatch_attempt
    }

    pub const fn replayed(&self) -> bool {
        self.replayed
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StepExecutionStoreStateBindingV1 {
    store_head_id: StoreHeadIdV1,
    store_generation_id: StoreGenerationIdV1,
    step_binding_commitment: [u8; 32],
    step_index_object_id: Option<StoreObjectIdV1>,
    carrier_object_id: Option<StoreObjectIdV1>,
    fence_high_water: u64,
}

impl StepExecutionStoreStateBindingV1 {
    pub const fn store_head_id(&self) -> StoreHeadIdV1 {
        self.store_head_id
    }

    pub const fn store_generation_id(&self) -> StoreGenerationIdV1 {
        self.store_generation_id
    }

    pub const fn fence_high_water(&self) -> u64 {
        self.fence_high_water
    }

    pub fn canonical_value(&self) -> Result<CborValue, CborError> {
        Ok(CborValue::Array(vec![
            CborValue::text("maestro.vnext.step-execution-store-state-binding.v1")?,
            bytes(self.store_head_id.as_bytes()),
            bytes(self.store_generation_id.as_bytes()),
            bytes(&self.step_binding_commitment),
            CborValue::optional(
                self.step_index_object_id
                    .map(|object_id| bytes(object_id.as_bytes())),
            ),
            CborValue::optional(
                self.carrier_object_id
                    .map(|object_id| bytes(object_id.as_bytes())),
            ),
            CborValue::Unsigned(self.fence_high_water),
        ]))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StepExecutionSnapshotV1 {
    binding: StepBindingV1,
    state_binding: StepExecutionStoreStateBindingV1,
    carrier: Option<StepExecutionCarrierV1>,
}

impl StepExecutionSnapshotV1 {
    pub const fn binding(&self) -> StepBindingV1 {
        self.binding
    }

    pub const fn state_binding(&self) -> &StepExecutionStoreStateBindingV1 {
        &self.state_binding
    }

    pub const fn carrier(&self) -> Option<&StepExecutionCarrierV1> {
        self.carrier.as_ref()
    }

    pub fn next_fence(&self) -> Result<u64, ExecutionStoreErrorV1> {
        self.state_binding
            .fence_high_water
            .checked_add(1)
            .ok_or(ExecutionStoreErrorV1::FenceOverflow)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthorizedStepExecutionMutationV1 {
    Acquire {
        executor: PrincipalIdV1,
        fixed_envelope_commitment: [u8; 32],
        run_limit: u32,
        issued_at: u64,
        expires_at: u64,
        hard_deadline: u64,
        takeover_safety: Option<Box<TakeoverSafetyV1>>,
    },
    Renew {
        expected_term_id: LeaseTermIdV1,
        issued_at: u64,
        expires_at: u64,
        lease_mutation: Option<Box<StepLeaseMutationV1>>,
    },
    Abandon {
        terminal: StepAttemptTerminalV1,
        expected_term_id: LeaseTermIdV1,
        as_of: u64,
        expected_run_set_revision: u64,
    },
}

impl AuthorizedStepExecutionMutationV1 {
    pub const fn action(&self) -> ExecutionActionV1 {
        match self {
            Self::Acquire { .. } => ExecutionActionV1::AcquireStepExecution,
            Self::Renew { .. } => ExecutionActionV1::RenewStepLeaseTerm,
            Self::Abandon { .. } => ExecutionActionV1::AbandonStepAttempt,
        }
    }

    pub fn canonical_value(&self) -> Result<CborValue, CborError> {
        Ok(match self {
            Self::Acquire {
                executor,
                fixed_envelope_commitment,
                run_limit,
                issued_at,
                expires_at,
                hard_deadline,
                takeover_safety,
            } => CborValue::Array(vec![
                CborValue::Unsigned(1),
                bytes(executor.as_bytes()),
                bytes(fixed_envelope_commitment),
                CborValue::Unsigned(u64::from(*run_limit)),
                CborValue::Unsigned(*issued_at),
                CborValue::Unsigned(*expires_at),
                CborValue::Unsigned(*hard_deadline),
                CborValue::optional(
                    takeover_safety
                        .as_deref()
                        .map(TakeoverSafetyV1::canonical_value),
                ),
            ]),
            Self::Renew {
                expected_term_id,
                issued_at,
                expires_at,
                lease_mutation,
            } => CborValue::Array(vec![
                CborValue::Unsigned(2),
                bytes(expected_term_id.as_bytes()),
                CborValue::Unsigned(*issued_at),
                CborValue::Unsigned(*expires_at),
                CborValue::optional(
                    lease_mutation
                        .as_deref()
                        .map(StepLeaseMutationV1::canonical_value)
                        .transpose()?,
                ),
            ]),
            Self::Abandon {
                terminal,
                expected_term_id,
                as_of,
                expected_run_set_revision,
            } => CborValue::Array(vec![
                CborValue::Unsigned(3),
                CborValue::Unsigned(step_terminal_tag(*terminal)),
                bytes(expected_term_id.as_bytes()),
                CborValue::Unsigned(*as_of),
                CborValue::Unsigned(*expected_run_set_revision),
            ]),
        })
    }

    fn executor(
        &self,
        snapshot: &StepExecutionSnapshotV1,
    ) -> Result<PrincipalIdV1, ExecutionStoreErrorV1> {
        match self {
            Self::Acquire { executor, .. } => Ok(*executor),
            Self::Renew { .. } | Self::Abandon { .. } => snapshot
                .carrier()
                .map(|carrier| carrier.tenure().attempt().executor())
                .ok_or(ExecutionStoreErrorV1::MissingStepExecutionCarrier),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StepExecutionPublicationV1 {
    request: CanonicalExecutionActionRequestV1,
    authority: GenericExecutionAuthorityV1,
    snapshot: StepExecutionSnapshotV1,
    mutation: AuthorizedStepExecutionMutationV1,
}

impl StepExecutionPublicationV1 {
    pub fn new(
        request: CanonicalExecutionActionRequestV1,
        authority: GenericExecutionAuthorityV1,
        snapshot: StepExecutionSnapshotV1,
        mutation: AuthorizedStepExecutionMutationV1,
    ) -> Result<Self, ExecutionStoreErrorV1> {
        let subject = step_binding_store_value(snapshot.binding);
        let expected_state = snapshot.state_binding.canonical_value()?;
        let payload = mutation.canonical_value()?;
        if request.action() != mutation.action()
            || authority.executor_principal_id() != mutation.executor(&snapshot)?
        {
            return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
        }
        validate_request_values(&request, &subject, &expected_state, &payload)?;
        validate_execution_authority_binding(&request, &authority.clone().into())?;
        Ok(Self {
            request,
            authority,
            snapshot,
            mutation,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StepExecutionPublicationOutcomeV1 {
    store_head: StoreHeadV1,
    carrier: StepExecutionCarrierV1,
    replayed: bool,
}

impl StepExecutionPublicationOutcomeV1 {
    pub const fn store_head(&self) -> &StoreHeadV1 {
        &self.store_head
    }

    pub const fn carrier(&self) -> &StepExecutionCarrierV1 {
        &self.carrier
    }

    pub const fn replayed(&self) -> bool {
        self.replayed
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StepSubmissionPublicationV1 {
    request: CanonicalStepSubmissionActionRequestV1,
    authority: SubmitStepAuthorityV1,
    snapshot: StepExecutionSnapshotV1,
    submission: StepSubmissionV1,
    evidence: EvidenceClaimPublicationV1,
    as_of: u64,
}

impl StepSubmissionPublicationV1 {
    pub fn new(
        request: CanonicalStepSubmissionActionRequestV1,
        authority: SubmitStepAuthorityV1,
        snapshot: StepExecutionSnapshotV1,
        submission: StepSubmissionV1,
        evidence: EvidenceClaimPublicationV1,
        as_of: u64,
    ) -> Result<Self, ExecutionStoreErrorV1> {
        let carrier = snapshot
            .carrier()
            .ok_or(ExecutionStoreErrorV1::MissingStepExecutionCarrier)?;
        let payload = step_submission_payload_value(&submission, as_of)?;
        if submission.binding() != snapshot.binding
            || submission.execution_carrier_commitment() != hash(&carrier.canonical_value()?)?
            || carrier.submission_fence(submission.execution_fence().term_id(), as_of)?
                != submission.execution_fence()
            || authority.executor_principal_id() != carrier.tenure().attempt().executor()
            || request.subject_commitment() != hash(&step_binding_store_value(snapshot.binding))?
            || request.expected_state_commitment()
                != hash(&snapshot.state_binding.canonical_value()?)?
            || request.payload_commitment() != hash(&payload)?
            || submission.claim_set_digest() != *evidence.claim_set().digest()
            || authority.subject_commitment() != request.subject_commitment()
            || authority.subject_basis_commitment() != request.expected_state_commitment()
            || authority.exact_payload_commitment() != request.payload_commitment()
        {
            return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
        }
        Ok(Self {
            request,
            authority,
            snapshot,
            submission,
            evidence,
            as_of,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StepSubmissionPublicationOutcomeV1 {
    store_head: StoreHeadV1,
    submission: StepSubmissionV1,
    carrier: StepExecutionCarrierV1,
    replayed: bool,
}

impl StepSubmissionPublicationOutcomeV1 {
    pub const fn store_head(&self) -> &StoreHeadV1 {
        &self.store_head
    }

    pub const fn submission(&self) -> &StepSubmissionV1 {
        &self.submission
    }

    pub const fn carrier(&self) -> &StepExecutionCarrierV1 {
        &self.carrier
    }

    pub const fn replayed(&self) -> bool {
        self.replayed
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StepLeaseMutationV1 {
    ReserveRun {
        expected_run_set_revision: u64,
        as_of: u64,
        reservation: RunReservationV1,
    },
    TransitionRun {
        run_id: RunIdV1,
        expected_run_set_revision: u64,
        as_of: u64,
        next: RunStateV1,
    },
    MarkDefinitelyNotStarted {
        expected_run_set_revision: u64,
        as_of: u64,
        receipt: RunNoStartReceiptV1,
    },
    AppendRunSegment {
        run_id: RunIdV1,
        expected_run_set_revision: u64,
        as_of: u64,
        process_or_job_identity: [u8; 32],
        segment_commitment: [u8; 32],
    },
    RetryRun {
        predecessor_run_id: RunIdV1,
        expected_run_set_revision: u64,
        as_of: u64,
        deadline: u64,
    },
}

impl StepLeaseMutationV1 {
    const fn as_of(&self) -> u64 {
        match self {
            Self::ReserveRun { as_of, .. }
            | Self::TransitionRun { as_of, .. }
            | Self::MarkDefinitelyNotStarted { as_of, .. }
            | Self::AppendRunSegment { as_of, .. }
            | Self::RetryRun { as_of, .. } => *as_of,
        }
    }

    fn canonical_value(&self) -> Result<CborValue, CborError> {
        Ok(match self {
            Self::ReserveRun {
                expected_run_set_revision,
                as_of,
                reservation,
            } => CborValue::Array(vec![
                CborValue::Unsigned(1),
                CborValue::Unsigned(*expected_run_set_revision),
                CborValue::Unsigned(*as_of),
                run_reservation_store_value(reservation),
            ]),
            Self::TransitionRun {
                run_id,
                expected_run_set_revision,
                as_of,
                next,
            } => CborValue::Array(vec![
                CborValue::Unsigned(2),
                bytes(run_id.as_bytes()),
                CborValue::Unsigned(*expected_run_set_revision),
                CborValue::Unsigned(*as_of),
                CborValue::Unsigned(run_state_store_tag(*next)),
            ]),
            Self::MarkDefinitelyNotStarted {
                expected_run_set_revision,
                as_of,
                receipt,
            } => CborValue::Array(vec![
                CborValue::Unsigned(5),
                CborValue::Unsigned(*expected_run_set_revision),
                CborValue::Unsigned(*as_of),
                receipt.canonical_value(),
            ]),
            Self::AppendRunSegment {
                run_id,
                expected_run_set_revision,
                as_of,
                process_or_job_identity,
                segment_commitment,
            } => CborValue::Array(vec![
                CborValue::Unsigned(3),
                bytes(run_id.as_bytes()),
                CborValue::Unsigned(*expected_run_set_revision),
                CborValue::Unsigned(*as_of),
                bytes(process_or_job_identity),
                bytes(segment_commitment),
            ]),
            Self::RetryRun {
                predecessor_run_id,
                expected_run_set_revision,
                as_of,
                deadline,
            } => CborValue::Array(vec![
                CborValue::Unsigned(4),
                bytes(predecessor_run_id.as_bytes()),
                CborValue::Unsigned(*expected_run_set_revision),
                CborValue::Unsigned(*as_of),
                CborValue::Unsigned(*deadline),
            ]),
        })
    }
}

pub struct ExecutionStoreFacadeV1<'store> {
    store: &'store mut StoreV1,
}

impl<'store> ExecutionStoreFacadeV1<'store> {
    pub fn new(store: &'store mut StoreV1) -> Self {
        Self { store }
    }

    pub fn current_state_binding(
        &self,
    ) -> Result<ExecutionStoreStateBindingV1, ExecutionStoreErrorV1> {
        let (state, head, generation, _) = self.store.coherent_publication_snapshot()?;
        if state != StoreStateV1::Active {
            return Err(ExecutionStoreErrorV1::InactiveStore);
        }
        Ok(ExecutionStoreStateBindingV1 {
            store_head_id: head.id(),
            store_generation_id: generation.id(),
            control_head: None,
            control_index_object_id: None,
        })
    }

    pub fn current_effect_state_binding(
        &self,
        intent: EffectIntentIdV1,
    ) -> Result<ExecutionStoreStateBindingV1, ExecutionStoreErrorV1> {
        let (state, head, generation, objects) = self.store.coherent_publication_snapshot()?;
        if state != StoreStateV1::Active {
            return Err(ExecutionStoreErrorV1::InactiveStore);
        }
        Ok(
            load_active_effect_snapshot(&head, &generation, &objects, intent, |generation_id| {
                Ok(self.store.generation(generation_id)?)
            })?
            .state_binding
            .clone(),
        )
    }

    pub fn current_effect_snapshot(
        &self,
        intent_id: EffectIntentIdV1,
    ) -> Result<ActiveStoreEffectSnapshotV1, ExecutionStoreErrorV1> {
        let (state, head, generation, objects) = self.store.coherent_publication_snapshot()?;
        if state != StoreStateV1::Active {
            return Err(ExecutionStoreErrorV1::InactiveStore);
        }
        load_active_effect_snapshot(&head, &generation, &objects, intent_id, |generation_id| {
            Ok(self.store.generation(generation_id)?)
        })
    }

    pub fn current_step_execution(
        &self,
        binding: StepBindingV1,
    ) -> Result<StepExecutionSnapshotV1, ExecutionStoreErrorV1> {
        let (state, head, generation, objects) = self.store.coherent_publication_snapshot()?;
        if state != StoreStateV1::Active {
            return Err(ExecutionStoreErrorV1::InactiveStore);
        }
        if binding.scope().repository_id() != generation.domain().id() {
            return Err(ExecutionStoreErrorV1::StepBindingStoreMismatch);
        }
        validate_rooted_step_state(&generation, &objects, binding)?;
        let binding_commitment = hash(&step_binding_store_value(binding))?;
        let index = load_optional_step_execution_index(&objects)?;
        let entry = index.as_ref().and_then(|index| {
            index
                .entries
                .iter()
                .find(|entry| entry.binding_commitment == binding_commitment)
        });
        let carrier = entry
            .map(|entry| load_step_execution_carrier(&objects, entry))
            .transpose()?;
        let fence_high_water = entry.map_or(0, |entry| entry.fence_high_water);
        Ok(StepExecutionSnapshotV1 {
            binding,
            state_binding: StepExecutionStoreStateBindingV1 {
                store_head_id: head.id(),
                store_generation_id: generation.id(),
                step_binding_commitment: binding_commitment,
                step_index_object_id: index.as_ref().map(|index| index.object.id()),
                carrier_object_id: entry.map(|entry| entry.carrier_object_id),
                fence_high_water,
            },
            carrier,
        })
    }

    pub fn issue_run_no_start_receipt(
        &mut self,
        binding: StepBindingV1,
        run_id: RunIdV1,
        observer: &mut impl PinnedExecutionBoundaryObserverV1,
    ) -> Result<RunNoStartReceiptV1, ExecutionStoreErrorV1> {
        let result = self.store.with_serialized_active_view(|view| {
            let generation = view
                .active_generation()?
                .ok_or(ExecutionStoreErrorV1::InactiveStore)?;
            let objects = view.active_generation_objects()?;
            if binding.scope().repository_id() != generation.domain().id() {
                return Err(ExecutionStoreErrorV1::StepBindingStoreMismatch);
            }
            validate_rooted_step_state(&generation, &objects, binding)?;
            let binding_commitment = hash(&step_binding_store_value(binding))?;
            let index = load_optional_step_execution_index(&objects)?
                .ok_or(ExecutionStoreErrorV1::MissingStepExecutionIndex)?;
            let entry = index
                .entries
                .iter()
                .find(|entry| entry.binding_commitment == binding_commitment)
                .ok_or(ExecutionStoreErrorV1::MissingStepExecutionCarrier)?;
            let carrier = load_step_execution_carrier(&objects, entry)?;
            let run = carrier
                .run_set()
                .runs()
                .iter()
                .find(|run| run.id() == run_id)
                .ok_or(ExecutionStoreErrorV1::Runtime(
                    ExecutionRuntimeErrorV1::UnknownRun,
                ))?;
            let reservation = run.reservation();
            if run.state() != RunStateV1::Reserved
                || observer.execution_boundary_commitment()
                    != reservation.execution_boundary_commitment
                || observer.observer_commitment() == [0; 32]
            {
                return Err(ExecutionStoreErrorV1::InvalidRunNoStartObservation);
            }
            let (accepted_h_time, authority_acceptance_commitment) =
                current_repository_authority_time(view, &generation)?;
            let challenge = RunNoStartObservationChallengeV1 {
                run_id,
                execution_boundary_commitment: reservation.execution_boundary_commitment,
                observed_at: accepted_h_time,
                authority_acceptance_commitment,
            };
            let observation_commitment = observer
                .observe_definitely_not_started(challenge)
                .filter(|commitment| *commitment != [0; 32])
                .ok_or(ExecutionStoreErrorV1::InvalidRunNoStartObservation)?;
            Ok(RunNoStartReceiptV1::from_validated_boundary_observation(
                run,
                accepted_h_time,
                observer.observer_commitment(),
                observation_commitment,
            )?)
        });
        match result {
            Ok(receipt) => Ok(receipt),
            Err(PreparedPublicationError::Store(error)) => Err(map_store_error(error)),
            Err(PreparedPublicationError::Prepare(error)) => Err(error),
        }
    }

    pub fn execute_provider_once(
        &mut self,
        release: ProviderApplicationReleaseV1,
        executor: &mut impl PinnedProviderExecutorV1,
    ) -> Result<ActiveStoreEffectTerminalDraftV1, ExecutionStoreErrorV1> {
        let result = self.store.with_serialized_active_view(|view| {
            let head = view
                .active_head()?
                .ok_or(ExecutionStoreErrorV1::InactiveStore)?;
            let generation = view
                .active_generation()?
                .ok_or(ExecutionStoreErrorV1::InactiveStore)?;
            let objects = view.active_generation_objects()?;
            let snapshot = load_active_effect_snapshot(
                &head,
                &generation,
                &objects,
                release.intent,
                |generation_id| Ok(view.generation(generation_id)?),
            )?;
            if snapshot.control_head.id() != release.sealed_control_head
                || snapshot.dispatch.attempt().id() != release.dispatch_attempt
                || sealed_provider_operation_from_snapshot(&snapshot)? != release.operation
            {
                return Err(ExecutionStoreErrorV1::StaleExternalIoRelease);
            }
            let (accepted_h_time, authority_acceptance_commitment) =
                current_repository_authority_time(view, &generation)?;
            let execution_time = RunExecutionTimeReceiptV1::from_binding(
                release.operation.binding.run_id,
                release.operation.binding.execution_boundary_commitment,
                release.operation.binding.deadline,
                accepted_h_time,
                authority_acceptance_commitment,
            )?;
            release.execute_once(execution_time, executor)
        });
        match result {
            Ok(draft) => Ok(draft),
            Err(PreparedPublicationError::Store(error)) => Err(map_store_error(error)),
            Err(PreparedPublicationError::Prepare(error)) => Err(error),
        }
    }

    pub fn execute_reconciliation_read_once(
        &mut self,
        release: ReconciliationReadReleaseV1,
        reader: &mut impl PinnedReconciliationReaderV1,
    ) -> Result<ActiveStoreEffectReconciliationReadDraftV1, ExecutionStoreErrorV1> {
        let result =
            self.store.with_serialized_active_view(|view| {
                let head = view
                    .active_head()?
                    .ok_or(ExecutionStoreErrorV1::InactiveStore)?;
                let generation = view
                    .active_generation()?
                    .ok_or(ExecutionStoreErrorV1::InactiveStore)?;
                let objects = view.active_generation_objects()?;
                let binding = release.read.binding;
                let snapshot = load_active_effect_snapshot(
                    &head,
                    &generation,
                    &objects,
                    binding.intent,
                    |generation_id| Ok(view.generation(generation_id)?),
                )?;
                let current_reconciliation = snapshot
                    .reconciliation()
                    .ok_or(ExecutionStoreErrorV1::StaleExternalIoRelease)?;
                if snapshot.control_head.id() != binding.control_head
                    || current_reconciliation.read_plan() != binding.read_plan
                    || current_reconciliation.run_set().runs().iter().all(|run| {
                        run.id() != binding.run_id || run.state() != RunStateV1::Reserved
                    })
                {
                    return Err(ExecutionStoreErrorV1::StaleExternalIoRelease);
                }
                let (accepted_h_time, authority_acceptance_commitment) =
                    current_repository_authority_time(view, &generation)?;
                let execution_time = RunExecutionTimeReceiptV1::from_binding(
                    binding.run_id,
                    binding.execution_boundary_commitment,
                    binding.deadline,
                    accepted_h_time,
                    authority_acceptance_commitment,
                )?;
                release.execute_once(execution_time, reader)
            });
        match result {
            Ok(draft) => Ok(draft),
            Err(PreparedPublicationError::Store(error)) => Err(map_store_error(error)),
            Err(PreparedPublicationError::Prepare(error)) => Err(error),
        }
    }

    pub fn canonical_step_request(
        &self,
        snapshot: &StepExecutionSnapshotV1,
        mutation: &AuthorizedStepExecutionMutationV1,
        idempotency_key_id: IdempotencyKeyIdV1,
    ) -> Result<CanonicalExecutionActionRequestV1, ExecutionStoreErrorV1> {
        validate_current_step_snapshot(self.store, snapshot)?;
        if !matches!(mutation, AuthorizedStepExecutionMutationV1::Abandon { .. }) {
            let (state, _, generation, active_objects) =
                self.store.coherent_publication_snapshot()?;
            if state != StoreStateV1::Active {
                return Err(ExecutionStoreErrorV1::InactiveStore);
            }
            require_current_step_graph(&generation, &active_objects, snapshot.binding)?;
        }
        Ok(CanonicalExecutionActionRequestV1::from_values(
            mutation.action(),
            &step_binding_store_value(snapshot.binding),
            &snapshot.state_binding.canonical_value()?,
            &mutation.canonical_value()?,
            idempotency_key_id,
        )?)
    }

    pub fn step_submission_candidate(
        &self,
        snapshot: &StepExecutionSnapshotV1,
        submission_id: StepSubmissionIdV1,
        expected_term_id: LeaseTermIdV1,
        as_of: u64,
        evidence: &EvidenceClaimPublicationV1,
    ) -> Result<StepSubmissionV1, ExecutionStoreErrorV1> {
        validate_current_step_snapshot(self.store, snapshot)?;
        let (state, _, generation, active_objects) = self.store.coherent_publication_snapshot()?;
        if state != StoreStateV1::Active {
            return Err(ExecutionStoreErrorV1::InactiveStore);
        }
        require_current_step_graph(&generation, &active_objects, snapshot.binding)?;
        if !validate_rooted_step_state(&generation, &active_objects, snapshot.binding)? {
            return Err(ExecutionStoreErrorV1::StepBindingNotOpen);
        }
        let carrier = snapshot
            .carrier()
            .ok_or(ExecutionStoreErrorV1::MissingStepExecutionCarrier)?;
        let fence = carrier.submission_fence(expected_term_id, as_of)?;
        Ok(StepSubmissionV1::new(
            submission_id,
            snapshot.binding,
            fence,
            hash(&carrier.canonical_value()?)?,
            evidence.claim_set(),
        )?)
    }

    pub fn canonical_step_submission_request(
        &self,
        snapshot: &StepExecutionSnapshotV1,
        submission: &StepSubmissionV1,
        as_of: u64,
        idempotency_key_id: IdempotencyKeyIdV1,
    ) -> Result<CanonicalStepSubmissionActionRequestV1, ExecutionStoreErrorV1> {
        validate_current_step_snapshot(self.store, snapshot)?;
        let (state, _, generation, active_objects) = self.store.coherent_publication_snapshot()?;
        if state != StoreStateV1::Active {
            return Err(ExecutionStoreErrorV1::InactiveStore);
        }
        require_current_step_graph(&generation, &active_objects, snapshot.binding)?;
        if !validate_rooted_step_state(&generation, &active_objects, snapshot.binding)? {
            return Err(ExecutionStoreErrorV1::StepBindingNotOpen);
        }
        let carrier = snapshot
            .carrier()
            .ok_or(ExecutionStoreErrorV1::MissingStepExecutionCarrier)?;
        if submission.binding() != snapshot.binding
            || submission.execution_carrier_commitment() != hash(&carrier.canonical_value()?)?
            || carrier.submission_fence(submission.execution_fence().term_id(), as_of)?
                != submission.execution_fence()
        {
            return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
        }
        Ok(CanonicalStepSubmissionActionRequestV1::from_values(
            &step_binding_store_value(snapshot.binding),
            &snapshot.state_binding.canonical_value()?,
            &step_submission_payload_value(submission, as_of)?,
            idempotency_key_id,
        )?)
    }

    pub fn canonical_request(
        &self,
        action: ExecutionActionV1,
        subject: &CborValue,
        payload: &CborValue,
        idempotency_key_id: IdempotencyKeyIdV1,
    ) -> Result<CanonicalExecutionActionRequestV1, ExecutionStoreErrorV1> {
        let state = self.current_state_binding()?;
        Ok(CanonicalExecutionActionRequestV1::from_values(
            action,
            subject,
            &state.canonical_value()?,
            payload,
            idempotency_key_id,
        )?)
    }

    pub fn canonical_effect_origination_request(
        &self,
        draft: &ActiveStoreEffectOriginationDraftV1,
        idempotency_key_id: IdempotencyKeyIdV1,
    ) -> Result<CanonicalExecutionActionRequestV1, ExecutionStoreErrorV1> {
        let state = self.current_state_binding()?;
        Ok(CanonicalExecutionActionRequestV1::from_values(
            draft.origin.reservation_action()?,
            &draft.authority_subject_value()?,
            &state.canonical_value()?,
            &draft.request_payload_value()?,
            idempotency_key_id,
        )?)
    }

    pub fn canonical_effect_redispatch_request(
        &self,
        snapshot: &ActiveStoreEffectSnapshotV1,
        draft: &ActiveStoreEffectRedispatchDraftV1,
        idempotency_key_id: IdempotencyKeyIdV1,
    ) -> Result<CanonicalExecutionActionRequestV1, ExecutionStoreErrorV1> {
        validate_current_effect_snapshot(self.store, snapshot)?;
        Ok(CanonicalExecutionActionRequestV1::from_values(
            snapshot.intent.origin().reservation_action()?,
            &effect_intent_subject_value(&snapshot.intent)?,
            &snapshot.state_binding.canonical_value()?,
            &draft.payload_value()?,
            idempotency_key_id,
        )?)
    }

    pub fn canonical_effect_seal_request(
        &self,
        snapshot: &ActiveStoreEffectSnapshotV1,
        draft: ActiveStoreEffectSealDraftV1,
        idempotency_key_id: IdempotencyKeyIdV1,
    ) -> Result<CanonicalExecutionActionRequestV1, ExecutionStoreErrorV1> {
        validate_current_effect_snapshot(self.store, snapshot)?;
        Ok(CanonicalExecutionActionRequestV1::from_values(
            snapshot.intent.origin().outcome_action()?,
            &effect_intent_subject_value(&snapshot.intent)?,
            &snapshot.state_binding.canonical_value()?,
            &draft.payload_value()?,
            idempotency_key_id,
        )?)
    }

    pub fn canonical_effect_recover_reserved_request(
        &self,
        snapshot: &ActiveStoreEffectSnapshotV1,
        draft: &ActiveStoreEffectRecoverReservedDraftV1,
        idempotency_key_id: IdempotencyKeyIdV1,
    ) -> Result<CanonicalExecutionActionRequestV1, ExecutionStoreErrorV1> {
        validate_current_effect_snapshot(self.store, snapshot)?;
        Ok(CanonicalExecutionActionRequestV1::from_values(
            snapshot.intent.origin().reservation_action()?,
            &effect_intent_subject_value(&snapshot.intent)?,
            &snapshot.state_binding.canonical_value()?,
            &draft.payload_value()?,
            idempotency_key_id,
        )?)
    }

    pub fn canonical_effect_terminal_request(
        &self,
        snapshot: &ActiveStoreEffectSnapshotV1,
        draft: &ActiveStoreEffectTerminalDraftV1,
        idempotency_key_id: IdempotencyKeyIdV1,
    ) -> Result<CanonicalExecutionActionRequestV1, ExecutionStoreErrorV1> {
        validate_current_effect_snapshot(self.store, snapshot)?;
        Ok(CanonicalExecutionActionRequestV1::from_values(
            snapshot.intent.origin().outcome_action()?,
            &effect_intent_subject_value(&snapshot.intent)?,
            &snapshot.state_binding.canonical_value()?,
            &draft.payload_value()?,
            idempotency_key_id,
        )?)
    }

    pub fn canonical_effect_reconciliation_begin_request(
        &self,
        snapshot: &ActiveStoreEffectSnapshotV1,
        draft: &ActiveStoreEffectReconciliationBeginDraftV1,
        idempotency_key_id: IdempotencyKeyIdV1,
    ) -> Result<CanonicalExecutionActionRequestV1, ExecutionStoreErrorV1> {
        validate_current_effect_snapshot(self.store, snapshot)?;
        Ok(CanonicalExecutionActionRequestV1::from_values(
            snapshot.intent.origin().reconciliation_action()?,
            &effect_intent_subject_value(&snapshot.intent)?,
            &snapshot.state_binding.canonical_value()?,
            &draft.payload_value()?,
            idempotency_key_id,
        )?)
    }

    pub fn canonical_effect_withdrawal_request(
        &self,
        snapshot: &ActiveStoreEffectSnapshotV1,
        draft: ActiveStoreEffectWithdrawalDraftV1,
        idempotency_key_id: IdempotencyKeyIdV1,
    ) -> Result<CanonicalExecutionActionRequestV1, ExecutionStoreErrorV1> {
        validate_current_effect_snapshot(self.store, snapshot)?;
        Ok(CanonicalExecutionActionRequestV1::from_values(
            snapshot.intent.origin().withdrawal_action()?,
            &effect_intent_subject_value(&snapshot.intent)?,
            &snapshot.state_binding.canonical_value()?,
            &draft.payload_value()?,
            idempotency_key_id,
        )?)
    }

    pub fn canonical_effect_writer_handoff_request(
        &self,
        snapshot: &ActiveStoreEffectSnapshotV1,
        draft: ActiveStoreEffectWriterHandoffDraftV1,
        idempotency_key_id: IdempotencyKeyIdV1,
    ) -> Result<CanonicalExecutionActionRequestV1, ExecutionStoreErrorV1> {
        validate_current_effect_snapshot(self.store, snapshot)?;
        Ok(CanonicalExecutionActionRequestV1::from_values(
            effect_writer_handoff_action(snapshot)?,
            &effect_intent_subject_value(&snapshot.intent)?,
            &snapshot.state_binding.canonical_value()?,
            &draft.payload_value()?,
            idempotency_key_id,
        )?)
    }

    pub fn canonical_effect_health_request(
        &self,
        snapshot: &ActiveStoreEffectSnapshotV1,
        draft: ActiveStoreEffectHealthDraftV1,
        idempotency_key_id: IdempotencyKeyIdV1,
    ) -> Result<CanonicalExecutionActionRequestV1, ExecutionStoreErrorV1> {
        validate_current_effect_snapshot(self.store, snapshot)?;
        Ok(CanonicalExecutionActionRequestV1::from_values(
            snapshot.intent.origin().reconciliation_action()?,
            &effect_intent_subject_value(&snapshot.intent)?,
            &snapshot.state_binding.canonical_value()?,
            &draft.payload_value()?,
            idempotency_key_id,
        )?)
    }

    pub fn canonical_effect_request(
        &self,
        action: ExecutionActionV1,
        intent: EffectIntentIdV1,
        subject: &CborValue,
        payload: &CborValue,
        idempotency_key_id: IdempotencyKeyIdV1,
    ) -> Result<CanonicalExecutionActionRequestV1, ExecutionStoreErrorV1> {
        let state = self.current_effect_state_binding(intent)?;
        Ok(CanonicalExecutionActionRequestV1::from_values(
            action,
            subject,
            &state.canonical_value()?,
            payload,
            idempotency_key_id,
        )?)
    }

    pub fn publish_effect_origination(
        &mut self,
        plan: ActiveStoreEffectOriginationPublicationV1,
    ) -> Result<ActiveStoreEffectOriginationOutcomeV1, ExecutionStoreErrorV1> {
        let meaning_digest = hash(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-origination-authorized-publication.v1")?,
            plan.request.canonical_value()?,
            execution_authority_value(&plan.authority)?,
            plan.draft.subject_value()?,
            plan.draft.payload_value()?,
        ]))?;
        let probe = StoreIdempotencyProbeV1::new(
            EFFECT_ORIGINATION_IDEMPOTENCY_NAMESPACE_V1,
            *plan.request.idempotency_key_id().as_bytes(),
            meaning_digest,
        )?;
        let expected_request = plan.request.clone();
        let expected_authority = plan.authority.clone();
        let expected_state = plan.state_binding.clone();
        let expected_draft = plan.draft.clone();
        let publication = self
            .store
            .publish_generation_atomically_with_prepare(&probe, |view| {
                let current_head = view
                    .active_head()?
                    .ok_or(ExecutionStoreErrorV1::InactiveStore)?;
                let current_generation = view
                    .active_generation()?
                    .ok_or(ExecutionStoreErrorV1::InactiveStore)?;
                if current_head.id() != expected_state.store_head_id()
                    || current_generation.id() != expected_state.store_generation_id()
                    || expected_draft.domain_kind
                        != effect_domain_kind_for_store_role(current_generation.domain().role())
                {
                    return Err(ExecutionStoreErrorV1::StaleExpectedStoreState);
                }
                let active_objects = view.active_generation_objects()?;
                validate_state_binding_against_objects(&expected_state, &active_objects, None)?;
                validate_request_values(
                    &expected_request,
                    &expected_draft.authority_subject_value()?,
                    &expected_state.canonical_value()?,
                    &expected_draft.request_payload_value()?,
                )?;
                let request_object = execution_action_request_object(&expected_request)?;
                let admission = admit_repository_action(
                    view,
                    &current_generation,
                    RepositoryActionAdmissionInputV1::new(
                        expected_request.request_id(),
                        expected_authority.clone(),
                    ),
                )?;
                validate_current_step_effect_origin(
                    &current_generation,
                    &active_objects,
                    &expected_draft.origin,
                    &expected_authority,
                    admission.accepted_h_time(),
                )?;
                let authorized = AuthorizedExecutionActionV1::new(
                    expected_request.clone(),
                    admission.authorization_receipt().clone(),
                )?;
                let logical_result = ActionResultV1::new(
                    expected_request.request_id(),
                    ActionOutcomeV1::Committed,
                    Some(admission.authorization_receipt().clone()),
                    None,
                )
                .map_err(|_| ExecutionStoreErrorV1::PublicationBindingMismatch)?;
                let (home, origination_fence, use_fence) = active_store_effect_fences(
                    &current_generation,
                    &expected_request,
                    &expected_draft,
                    &admission,
                    *logical_result.id().as_bytes(),
                )?;
                let intent = EffectIntentDraftV1 {
                    home,
                    origin: expected_draft.origin.clone(),
                    origination_fence,
                    semantic_use: expected_draft.semantic_use,
                    material_inputs: expected_draft.material_inputs,
                    credential_requirements: expected_draft.credential_requirements,
                }
                .authorize(&authorized)?;
                let initial_control = intent.initial_active_store_control(use_fence)?;
                let preparation = expected_draft
                    .dispatch
                    .clone()
                    .bind(use_fence, *admission.basis_object().id().as_bytes());
                let prepared = intent.prepare_dispatch(
                    initial_control.revision(),
                    preparation,
                    &authorized,
                )?;
                let transition = prepared.control_need().control_transition(
                    initial_control.head(),
                    initial_control.revision(),
                    initial_control.writer_term(),
                )?;
                let (reserved_revision, reserved_head) = transition.apply(
                    initial_control.head(),
                    initial_control.revision(),
                    initial_control.writer_term(),
                    None,
                )?;
                build_effect_origination_publication(
                    view.domain().clone(),
                    &current_head,
                    &current_generation,
                    &active_objects,
                    &expected_request,
                    &expected_authority,
                    &admission,
                    request_object,
                    &intent,
                    initial_control.revision(),
                    initial_control.writer_term(),
                    &prepared,
                    &reserved_revision,
                    &reserved_head,
                    meaning_digest,
                )
            });
        let publication = match publication {
            Ok(value) => value,
            Err(PreparedPublicationError::Store(error)) => return Err(map_store_error(error)),
            Err(PreparedPublicationError::Prepare(error)) => return Err(error),
        };
        decode_effect_origination_outcome(publication)
    }

    pub fn publish_effect_redispatch(
        &mut self,
        plan: ActiveStoreEffectRedispatchPublicationV1,
    ) -> Result<ActiveStoreEffectRedispatchOutcomeV1, ExecutionStoreErrorV1> {
        let meaning_digest = hash(&CborValue::Array(vec![
            CborValue::text(EFFECT_REDISPATCH_IDEMPOTENCY_NAMESPACE_V1)?,
            plan.request.canonical_value()?,
            execution_authority_value(&plan.authority)?,
            plan.draft.payload_value()?,
        ]))?;
        let probe = StoreIdempotencyProbeV1::new(
            EFFECT_REDISPATCH_IDEMPOTENCY_NAMESPACE_V1,
            *plan.request.idempotency_key_id().as_bytes(),
            meaning_digest,
        )?;
        let expected_request = plan.request.clone();
        let expected_authority = plan.authority.clone();
        let expected_snapshot = plan.snapshot.clone();
        let expected_draft = plan.draft.clone();
        let publication = self
            .store
            .publish_generation_atomically_with_prepare(&probe, |view| {
                let current_head = view
                    .active_head()?
                    .ok_or(ExecutionStoreErrorV1::InactiveStore)?;
                let current_generation = view
                    .active_generation()?
                    .ok_or(ExecutionStoreErrorV1::InactiveStore)?;
                let active_objects = view.active_generation_objects()?;
                let current_snapshot = load_active_effect_snapshot(
                    &current_head,
                    &current_generation,
                    &active_objects,
                    expected_snapshot.intent.id(),
                    |generation_id| Ok(view.generation(generation_id)?),
                )?;
                if current_snapshot != expected_snapshot {
                    return Err(ExecutionStoreErrorV1::StaleExpectedStoreState);
                }
                validate_request_values(
                    &expected_request,
                    &effect_intent_subject_value(&current_snapshot.intent)?,
                    &current_snapshot.state_binding.canonical_value()?,
                    &expected_draft.payload_value()?,
                )?;
                let request_object = execution_action_request_object(&expected_request)?;
                let admission = admit_repository_action(
                    view,
                    &current_generation,
                    RepositoryActionAdmissionInputV1::new(
                        expected_request.request_id(),
                        expected_authority.clone(),
                    ),
                )?;
                let authorized = AuthorizedExecutionActionV1::new(
                    expected_request.clone(),
                    admission.authorization_receipt().clone(),
                )?;
                let use_fence = active_store_redispatch_use_fence(
                    &current_generation,
                    &expected_request,
                    &current_snapshot,
                    &expected_draft,
                    &admission,
                )?;
                let preparation = expected_draft
                    .dispatch
                    .clone()
                    .bind(use_fence, *admission.basis_object().id().as_bytes());
                let prepared = current_snapshot.intent.prepare_dispatch(
                    &current_snapshot.control_revision,
                    preparation,
                    &authorized,
                )?;
                if !matches!(
                    prepared.control_need(),
                    EffectControlTransitionNeedV1::RedispatchConclusiveNotApplied { .. }
                ) {
                    return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
                }
                let transition = prepared.control_need().control_transition(
                    &current_snapshot.control_head,
                    &current_snapshot.control_revision,
                    current_snapshot.writer_term,
                )?;
                let (reserved_revision, reserved_head) = transition.apply(
                    &current_snapshot.control_head,
                    &current_snapshot.control_revision,
                    current_snapshot.writer_term,
                    None,
                )?;
                build_effect_redispatch_publication(
                    view.domain().clone(),
                    &current_head,
                    &current_generation,
                    &active_objects,
                    &expected_request,
                    &expected_authority,
                    &admission,
                    request_object,
                    &current_snapshot,
                    &prepared,
                    &reserved_revision,
                    &reserved_head,
                    meaning_digest,
                )
            });
        let publication = match publication {
            Ok(value) => value,
            Err(PreparedPublicationError::Store(error)) => return Err(map_store_error(error)),
            Err(PreparedPublicationError::Prepare(error)) => return Err(error),
        };
        decode_effect_redispatch_outcome(
            publication,
            expected_request.request_id(),
            expected_snapshot.intent.id(),
            meaning_digest,
        )
    }

    pub fn publish_effect_recover_reserved(
        &mut self,
        plan: ActiveStoreEffectRecoverReservedPublicationV1,
    ) -> Result<ActiveStoreEffectRecoverReservedOutcomeV1, ExecutionStoreErrorV1> {
        let meaning_digest = hash(&CborValue::Array(vec![
            CborValue::text(EFFECT_RECOVER_RESERVED_IDEMPOTENCY_NAMESPACE_V1)?,
            plan.request.canonical_value()?,
            execution_authority_value(&plan.authority)?,
            plan.draft.payload_value()?,
        ]))?;
        let probe = StoreIdempotencyProbeV1::new(
            EFFECT_RECOVER_RESERVED_IDEMPOTENCY_NAMESPACE_V1,
            *plan.request.idempotency_key_id().as_bytes(),
            meaning_digest,
        )?;
        let expected_request = plan.request.clone();
        let expected_authority = plan.authority.clone();
        let expected_snapshot = plan.snapshot.clone();
        let expected_draft = plan.draft.clone();
        let publication = self
            .store
            .publish_generation_atomically_with_prepare(&probe, |view| {
                let current_head = view
                    .active_head()?
                    .ok_or(ExecutionStoreErrorV1::InactiveStore)?;
                let current_generation = view
                    .active_generation()?
                    .ok_or(ExecutionStoreErrorV1::InactiveStore)?;
                let active_objects = view.active_generation_objects()?;
                let current_snapshot = load_active_effect_snapshot(
                    &current_head,
                    &current_generation,
                    &active_objects,
                    expected_snapshot.intent.id(),
                    |generation_id| Ok(view.generation(generation_id)?),
                )?;
                if current_snapshot != expected_snapshot {
                    return Err(ExecutionStoreErrorV1::StaleExpectedStoreState);
                }
                validate_request_values(
                    &expected_request,
                    &effect_intent_subject_value(&current_snapshot.intent)?,
                    &current_snapshot.state_binding.canonical_value()?,
                    &expected_draft.payload_value()?,
                )?;
                let request_object = execution_action_request_object(&expected_request)?;
                let admission = admit_repository_action(
                    view,
                    &current_generation,
                    RepositoryActionAdmissionInputV1::new(
                        expected_request.request_id(),
                        expected_authority.clone(),
                    ),
                )?;
                let authorized = AuthorizedExecutionActionV1::new(
                    expected_request.clone(),
                    admission.authorization_receipt().clone(),
                )?;
                let owner = super::runtime::ExecutionAttemptOwnerV1::Dispatch(
                    current_snapshot.dispatch.attempt().id(),
                );
                let recovery_need = EffectControlTransitionNeedV1::RecoverReserved {
                    action_request_id: expected_request.request_id(),
                    attempt: owner,
                    dispatch_fence: current_snapshot.dispatch.attempt().dispatch_fence(),
                };
                let recovery_transition = recovery_need.control_transition(
                    &current_snapshot.control_head,
                    &current_snapshot.control_revision,
                    current_snapshot.writer_term,
                )?;
                if !recovery_transition.accepts_action(expected_request.action()) {
                    return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
                }
                let (recovered_revision, recovered_head) = recovery_transition.apply(
                    &current_snapshot.control_head,
                    &current_snapshot.control_revision,
                    current_snapshot.writer_term,
                    None,
                )?;
                if recovered_revision != current_snapshot.control_revision
                    || recovered_head != current_snapshot.control_head
                {
                    return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
                }
                match &expected_draft {
                    ActiveStoreEffectRecoverReservedDraftV1::Seal(draft) => {
                        let candidate = current_snapshot.dispatch.recover_reserved_seal_candidate(
                            &current_snapshot.intent,
                            &current_snapshot.control_revision,
                            draft.seal_commitment(),
                            &authorized,
                        )?;
                        let transition = candidate.control_need().control_transition(
                            &current_snapshot.control_head,
                            &current_snapshot.control_revision,
                            current_snapshot.writer_term,
                        )?;
                        if !transition.accepts_action(expected_request.action()) {
                            return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
                        }
                        let (sealed_revision, sealed_head) = transition.apply(
                            &current_snapshot.control_head,
                            &current_snapshot.control_revision,
                            current_snapshot.writer_term,
                            None,
                        )?;
                        build_effect_seal_publication(
                            view.domain().clone(),
                            &current_head,
                            &current_generation,
                            &active_objects,
                            &expected_request,
                            &expected_authority,
                            &admission,
                            request_object,
                            &current_snapshot,
                            &candidate,
                            &sealed_revision,
                            &sealed_head,
                            meaning_digest,
                            EFFECT_RECOVER_RESERVED_IDEMPOTENCY_NAMESPACE_V1,
                        )
                    }
                    ActiveStoreEffectRecoverReservedDraftV1::Reject(draft) => {
                        let occurrence = effect_dispatch_terminal_occurrence_object(
                            &expected_request,
                            draft,
                            request_object.id(),
                            admission.basis_object().id(),
                            meaning_digest,
                        )?;
                        let idempotency_commitment = hash(&CborValue::Array(vec![
                            CborValue::text(
                                "maestro.vnext.effect-recover-reserved-terminal-idempotency.v1",
                            )?,
                            CborValue::text(EFFECT_RECOVER_RESERVED_IDEMPOTENCY_NAMESPACE_V1)?,
                            bytes(expected_request.idempotency_key_id().as_bytes()),
                            bytes(&meaning_digest),
                            bytes(occurrence.id().as_bytes()),
                        ]))?;
                        let candidate = current_snapshot
                            .dispatch
                            .recover_reserved_rejection_candidate(
                                &current_snapshot.intent,
                                &current_snapshot.control_revision,
                                draft.outcome(),
                                *occurrence.id().as_bytes(),
                                idempotency_commitment,
                                &authorized,
                            )?;
                        let transition = candidate.control_need().control_transition(
                            &current_snapshot.control_head,
                            &current_snapshot.control_revision,
                            current_snapshot.writer_term,
                        )?;
                        if !transition.accepts_action(expected_request.action()) {
                            return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
                        }
                        let terminal_publication =
                            EffectIntentControlPublicationCommitmentsV1::from_store_publication(
                                *occurrence.id().as_bytes(),
                                idempotency_commitment,
                            )?;
                        let (terminal_revision, terminal_head) = transition.apply(
                            &current_snapshot.control_head,
                            &current_snapshot.control_revision,
                            current_snapshot.writer_term,
                            Some(terminal_publication),
                        )?;
                        build_effect_terminal_publication(
                            view.domain().clone(),
                            &current_head,
                            &current_generation,
                            &active_objects,
                            &expected_request,
                            &expected_authority,
                            &admission,
                            request_object,
                            occurrence,
                            &current_snapshot,
                            &candidate,
                            &terminal_revision,
                            &terminal_head,
                            meaning_digest,
                            EFFECT_RECOVER_RESERVED_IDEMPOTENCY_NAMESPACE_V1,
                        )
                    }
                }
            });
        let publication = match publication {
            Ok(value) => value,
            Err(PreparedPublicationError::Store(error)) => return Err(map_store_error(error)),
            Err(PreparedPublicationError::Prepare(error)) => return Err(error),
        };
        match expected_draft {
            ActiveStoreEffectRecoverReservedDraftV1::Seal(_) => {
                Ok(ActiveStoreEffectRecoverReservedOutcomeV1::Sealed(
                    decode_effect_seal_outcome(publication, &expected_snapshot)?,
                ))
            }
            ActiveStoreEffectRecoverReservedDraftV1::Reject(_) => {
                Ok(ActiveStoreEffectRecoverReservedOutcomeV1::Rejected(
                    decode_effect_terminal_outcome(publication)?,
                ))
            }
        }
    }

    pub fn publish_effect_seal(
        &mut self,
        plan: ActiveStoreEffectSealPublicationV1,
    ) -> Result<ActiveStoreEffectSealOutcomeV1, ExecutionStoreErrorV1> {
        let meaning_digest = hash(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-dispatch-seal-authorized-publication.v1")?,
            plan.request.canonical_value()?,
            execution_authority_value(&plan.authority)?,
            plan.draft.payload_value()?,
        ]))?;
        let probe = StoreIdempotencyProbeV1::new(
            EFFECT_DISPATCH_SEAL_IDEMPOTENCY_NAMESPACE_V1,
            *plan.request.idempotency_key_id().as_bytes(),
            meaning_digest,
        )?;
        let expected_request = plan.request.clone();
        let expected_authority = plan.authority.clone();
        let expected_snapshot = plan.snapshot.clone();
        let expected_draft = plan.draft;
        let publication = self
            .store
            .publish_generation_atomically_with_prepare(&probe, |view| {
                let current_head = view
                    .active_head()?
                    .ok_or(ExecutionStoreErrorV1::InactiveStore)?;
                let current_generation = view
                    .active_generation()?
                    .ok_or(ExecutionStoreErrorV1::InactiveStore)?;
                let active_objects = view.active_generation_objects()?;
                let current_snapshot = load_active_effect_snapshot(
                    &current_head,
                    &current_generation,
                    &active_objects,
                    expected_snapshot.intent.id(),
                    |generation_id| Ok(view.generation(generation_id)?),
                )?;
                if current_snapshot != expected_snapshot {
                    return Err(ExecutionStoreErrorV1::StaleExpectedStoreState);
                }
                validate_request_values(
                    &expected_request,
                    &effect_intent_subject_value(&current_snapshot.intent)?,
                    &current_snapshot.state_binding.canonical_value()?,
                    &expected_draft.payload_value()?,
                )?;
                let request_object = execution_action_request_object(&expected_request)?;
                let admission = admit_repository_action(
                    view,
                    &current_generation,
                    RepositoryActionAdmissionInputV1::new(
                        expected_request.request_id(),
                        expected_authority.clone(),
                    ),
                )?;
                let authorized = AuthorizedExecutionActionV1::new(
                    expected_request.clone(),
                    admission.authorization_receipt().clone(),
                )?;
                let candidate = current_snapshot.dispatch.seal_candidate(
                    &current_snapshot.intent,
                    &current_snapshot.control_revision,
                    expected_draft.seal_commitment(),
                    &authorized,
                )?;
                let transition = candidate.control_need().control_transition(
                    &current_snapshot.control_head,
                    &current_snapshot.control_revision,
                    current_snapshot.writer_term,
                )?;
                let (sealed_revision, sealed_head) = transition.apply(
                    &current_snapshot.control_head,
                    &current_snapshot.control_revision,
                    current_snapshot.writer_term,
                    None,
                )?;
                build_effect_seal_publication(
                    view.domain().clone(),
                    &current_head,
                    &current_generation,
                    &active_objects,
                    &expected_request,
                    &expected_authority,
                    &admission,
                    request_object,
                    &current_snapshot,
                    &candidate,
                    &sealed_revision,
                    &sealed_head,
                    meaning_digest,
                    EFFECT_DISPATCH_SEAL_IDEMPOTENCY_NAMESPACE_V1,
                )
            });
        let publication = match publication {
            Ok(value) => value,
            Err(PreparedPublicationError::Store(error)) => return Err(map_store_error(error)),
            Err(PreparedPublicationError::Prepare(error)) => return Err(error),
        };
        decode_effect_seal_outcome(publication, &expected_snapshot)
    }

    pub fn publish_effect_terminal(
        &mut self,
        plan: ActiveStoreEffectTerminalPublicationV1,
    ) -> Result<ActiveStoreEffectTerminalOutcomeV1, ExecutionStoreErrorV1> {
        let meaning_digest = hash(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-dispatch-terminal-authorized-publication.v1")?,
            plan.request.canonical_value()?,
            execution_authority_value(&plan.authority)?,
            plan.draft.payload_value()?,
        ]))?;
        let probe = StoreIdempotencyProbeV1::new(
            EFFECT_DISPATCH_TERMINAL_IDEMPOTENCY_NAMESPACE_V1,
            *plan.request.idempotency_key_id().as_bytes(),
            meaning_digest,
        )?;
        let expected_request = plan.request.clone();
        let expected_authority = plan.authority.clone();
        let expected_snapshot = plan.snapshot.clone();
        let expected_draft = plan.draft;
        let publication = self
            .store
            .publish_generation_atomically_with_prepare(&probe, |view| {
                let current_head = view
                    .active_head()?
                    .ok_or(ExecutionStoreErrorV1::InactiveStore)?;
                let current_generation = view
                    .active_generation()?
                    .ok_or(ExecutionStoreErrorV1::InactiveStore)?;
                let active_objects = view.active_generation_objects()?;
                let current_snapshot = load_active_effect_snapshot(
                    &current_head,
                    &current_generation,
                    &active_objects,
                    expected_snapshot.intent.id(),
                    |generation_id| Ok(view.generation(generation_id)?),
                )?;
                if current_snapshot != expected_snapshot {
                    return Err(ExecutionStoreErrorV1::StaleExpectedStoreState);
                }
                validate_request_values(
                    &expected_request,
                    &effect_intent_subject_value(&current_snapshot.intent)?,
                    &current_snapshot.state_binding.canonical_value()?,
                    &expected_draft.payload_value()?,
                )?;
                let request_object = execution_action_request_object(&expected_request)?;
                let admission = admit_repository_action(
                    view,
                    &current_generation,
                    RepositoryActionAdmissionInputV1::new(
                        expected_request.request_id(),
                        expected_authority.clone(),
                    ),
                )?;
                let authorized = AuthorizedExecutionActionV1::new(
                    expected_request.clone(),
                    admission.authorization_receipt().clone(),
                )?;
                let occurrence = effect_dispatch_terminal_occurrence_object(
                    &expected_request,
                    &expected_draft,
                    request_object.id(),
                    admission.basis_object().id(),
                    meaning_digest,
                )?;
                let idempotency_commitment = hash(&CborValue::Array(vec![
                    CborValue::text("maestro.vnext.effect-dispatch-terminal-idempotency.v1")?,
                    CborValue::text(EFFECT_DISPATCH_TERMINAL_IDEMPOTENCY_NAMESPACE_V1)?,
                    bytes(expected_request.idempotency_key_id().as_bytes()),
                    bytes(&meaning_digest),
                    bytes(occurrence.id().as_bytes()),
                ]))?;
                let candidate = current_snapshot.dispatch.terminal_candidate(
                    &current_snapshot.intent,
                    &current_snapshot.control_revision,
                    expected_draft.outcome(),
                    *occurrence.id().as_bytes(),
                    idempotency_commitment,
                    &authorized,
                )?;
                let transition = candidate.control_need().control_transition(
                    &current_snapshot.control_head,
                    &current_snapshot.control_revision,
                    current_snapshot.writer_term,
                )?;
                let terminal_publication =
                    EffectIntentControlPublicationCommitmentsV1::from_store_publication(
                        *occurrence.id().as_bytes(),
                        idempotency_commitment,
                    )?;
                let (terminal_revision, terminal_head) = transition.apply(
                    &current_snapshot.control_head,
                    &current_snapshot.control_revision,
                    current_snapshot.writer_term,
                    Some(terminal_publication),
                )?;
                build_effect_terminal_publication(
                    view.domain().clone(),
                    &current_head,
                    &current_generation,
                    &active_objects,
                    &expected_request,
                    &expected_authority,
                    &admission,
                    request_object,
                    occurrence,
                    &current_snapshot,
                    &candidate,
                    &terminal_revision,
                    &terminal_head,
                    meaning_digest,
                    EFFECT_DISPATCH_TERMINAL_IDEMPOTENCY_NAMESPACE_V1,
                )
            });
        let publication = match publication {
            Ok(value) => value,
            Err(PreparedPublicationError::Store(error)) => return Err(map_store_error(error)),
            Err(PreparedPublicationError::Prepare(error)) => return Err(error),
        };
        decode_effect_terminal_outcome(publication)
    }

    pub fn publish_effect_reconciliation_begin(
        &mut self,
        plan: ActiveStoreEffectReconciliationBeginPublicationV1,
    ) -> Result<ActiveStoreEffectReconciliationOutcomeV1, ExecutionStoreErrorV1> {
        let meaning_digest = hash(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-reconciliation-begin-authorized-publication.v1")?,
            plan.request.canonical_value()?,
            execution_authority_value(&plan.authority)?,
            plan.draft.payload_value()?,
        ]))?;
        let probe = StoreIdempotencyProbeV1::new(
            EFFECT_RECONCILIATION_BEGIN_IDEMPOTENCY_NAMESPACE_V1,
            *plan.request.idempotency_key_id().as_bytes(),
            meaning_digest,
        )?;
        let expected_request = plan.request.clone();
        let expected_authority = plan.authority.clone();
        let expected_snapshot = plan.snapshot.clone();
        let expected_draft = plan.draft.clone();
        let publication = self
            .store
            .publish_generation_atomically_with_prepare(&probe, |view| {
                let current_head = view
                    .active_head()?
                    .ok_or(ExecutionStoreErrorV1::InactiveStore)?;
                let current_generation = view
                    .active_generation()?
                    .ok_or(ExecutionStoreErrorV1::InactiveStore)?;
                let active_objects = view.active_generation_objects()?;
                let current_snapshot = load_active_effect_snapshot(
                    &current_head,
                    &current_generation,
                    &active_objects,
                    expected_snapshot.intent.id(),
                    |generation_id| Ok(view.generation(generation_id)?),
                )?;
                if current_snapshot != expected_snapshot {
                    return Err(ExecutionStoreErrorV1::StaleExpectedStoreState);
                }
                validate_request_values(
                    &expected_request,
                    &effect_intent_subject_value(&current_snapshot.intent)?,
                    &current_snapshot.state_binding.canonical_value()?,
                    &expected_draft.payload_value()?,
                )?;
                let request_object = execution_action_request_object(&expected_request)?;
                let admission = admit_repository_action(
                    view,
                    &current_generation,
                    RepositoryActionAdmissionInputV1::new(
                        expected_request.request_id(),
                        expected_authority.clone(),
                    ),
                )?;
                let authorized = AuthorizedExecutionActionV1::new(
                    expected_request.clone(),
                    admission.authorization_receipt().clone(),
                )?;
                let prior_requests = historical_reconciliation_begin_requests(
                    &active_objects,
                    current_snapshot.intent.id(),
                )?;
                let use_fence = active_store_reconciliation_use_fence(
                    &current_generation,
                    &expected_request,
                    &current_snapshot,
                    &expected_draft,
                    &admission,
                )?;
                let prepared = current_snapshot.intent.prepare_reconciliation(
                    &current_snapshot.control_revision,
                    EffectReconciliationPreparationV1 {
                        use_fence,
                        read_plan: expected_draft.read_plan,
                        read_run: expected_draft.read_run.clone(),
                    },
                    &authorized,
                    &prior_requests,
                )?;
                let transition = prepared.control_need().control_transition(
                    &current_snapshot.control_head,
                    &current_snapshot.control_revision,
                    current_snapshot.writer_term,
                )?;
                let (next_revision, next_head) = transition.apply(
                    &current_snapshot.control_head,
                    &current_snapshot.control_revision,
                    current_snapshot.writer_term,
                    None,
                )?;
                build_effect_reconciliation_publication(
                    view.domain().clone(),
                    &current_head,
                    &current_generation,
                    &active_objects,
                    &expected_request,
                    Some(&admission),
                    None,
                    admission.basis_object().id(),
                    request_object,
                    &current_snapshot,
                    ReconciliationPublicationPhaseV1::Begin,
                    effect_reconciliation_authorized_carrier_value(
                        prepared.persistence_carrier_value()?,
                        &expected_authority,
                    )?,
                    None,
                    &next_revision,
                    &next_head,
                    meaning_digest,
                )
            });
        let publication = match publication {
            Ok(value) => value,
            Err(PreparedPublicationError::Store(error)) => return Err(map_store_error(error)),
            Err(PreparedPublicationError::Prepare(error)) => return Err(error),
        };
        let read_plan = expected_draft.read_plan();
        let current_snapshot = self.current_effect_snapshot(expected_snapshot.intent.id())?;
        let read_release_pending = current_snapshot
            .reconciliation
            .as_ref()
            .filter(|reconciliation| {
                reconciliation.read_usage().is_none() && reconciliation.read_plan() == read_plan
            })
            .and_then(|reconciliation| {
                let [run] = reconciliation.run_set().runs() else {
                    return None;
                };
                let reservation = run.reservation();
                Some(PendingReconciliationReadReleaseV1 {
                    run_id: run.id(),
                    execution_boundary_commitment: reservation.execution_boundary_commitment,
                    deadline: reservation.deadline,
                    read_plan,
                })
            });
        decode_effect_reconciliation_outcome(
            publication,
            ReconciliationPublicationPhaseV1::Begin,
            expected_request.request_id(),
            expected_snapshot.intent.id(),
            meaning_digest,
            read_release_pending,
        )
    }

    pub fn publish_effect_reconciliation_read(
        &mut self,
        plan: ActiveStoreEffectReconciliationReadPublicationV1,
    ) -> Result<ActiveStoreEffectReconciliationOutcomeV1, ExecutionStoreErrorV1> {
        let authorization = plan
            .snapshot
            .reconciliation_authorization()
            .cloned()
            .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?;
        let authority = plan
            .snapshot
            .reconciliation_execution_authority()
            .cloned()
            .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?;
        let meaning_digest = hash(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-reconciliation-read-authorized-publication.v1")?,
            authorization.request().canonical_value()?,
            execution_authority_value(&authority)?,
            plan.draft.payload_value()?,
        ]))?;
        let probe = StoreIdempotencyProbeV1::new(
            EFFECT_RECONCILIATION_READ_IDEMPOTENCY_NAMESPACE_V1,
            *authorization.request().idempotency_key_id().as_bytes(),
            meaning_digest,
        )?;
        let expected_authorization = authorization;
        let expected_authority = authority;
        let expected_snapshot = plan.snapshot.clone();
        let expected_draft = plan.draft;
        let publication = self
            .store
            .publish_generation_atomically_with_prepare(&probe, |view| {
                let current_head = view
                    .active_head()?
                    .ok_or(ExecutionStoreErrorV1::InactiveStore)?;
                let current_generation = view
                    .active_generation()?
                    .ok_or(ExecutionStoreErrorV1::InactiveStore)?;
                let active_objects = view.active_generation_objects()?;
                let current_snapshot = load_active_effect_snapshot(
                    &current_head,
                    &current_generation,
                    &active_objects,
                    expected_snapshot.intent.id(),
                    |generation_id| Ok(view.generation(generation_id)?),
                )?;
                if current_snapshot != expected_snapshot {
                    return Err(ExecutionStoreErrorV1::StaleExpectedStoreState);
                }
                if current_snapshot.reconciliation_authorization() != Some(&expected_authorization)
                    || current_snapshot.reconciliation_execution_authority()
                        != Some(&expected_authority)
                {
                    return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
                }
                let expected_request = expected_authorization.request();
                let request_object = execution_action_request_object(expected_request)?;
                if !active_objects
                    .iter()
                    .any(|object| object == &request_object)
                {
                    return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
                }
                let EffectIntentUseFenceV1::ActiveStore(fence) = current_snapshot
                    .reconciliation
                    .as_ref()
                    .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?
                    .use_fence()
                else {
                    return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
                };
                let basis_object_id = StoreObjectIdV1::from_digest(*fence.authority.as_bytes());
                let continuation = continue_repository_action_attempt(
                    view,
                    &current_generation,
                    basis_object_id,
                    &expected_authority,
                    *fence.epoch.as_bytes(),
                )?;
                let occurrence = effect_reconciliation_read_occurrence_object(
                    expected_request,
                    expected_draft,
                    request_object.id(),
                    basis_object_id,
                    meaning_digest,
                )?;
                let idempotency_commitment = hash(&CborValue::Array(vec![
                    CborValue::text("maestro.vnext.effect-reconciliation-read-idempotency.v1")?,
                    bytes(expected_request.idempotency_key_id().as_bytes()),
                    bytes(&meaning_digest),
                    bytes(occurrence.id().as_bytes()),
                ]))?;
                let reconciliation = current_snapshot
                    .reconciliation
                    .clone()
                    .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?;
                let candidate = reconciliation.execute_read_candidate(
                    &current_snapshot.intent,
                    &current_snapshot.control_revision,
                    expected_draft.usage(),
                    *occurrence.id().as_bytes(),
                    idempotency_commitment,
                    &expected_authorization,
                )?;
                let transition = candidate.control_need().control_transition(
                    &current_snapshot.control_head,
                    &current_snapshot.control_revision,
                    current_snapshot.writer_term,
                )?;
                let publication =
                    EffectIntentControlPublicationCommitmentsV1::from_store_publication(
                        *occurrence.id().as_bytes(),
                        idempotency_commitment,
                    )?;
                let (next_revision, next_head) = transition.apply(
                    &current_snapshot.control_head,
                    &current_snapshot.control_revision,
                    current_snapshot.writer_term,
                    Some(publication),
                )?;
                build_effect_reconciliation_publication(
                    view.domain().clone(),
                    &current_head,
                    &current_generation,
                    &active_objects,
                    expected_request,
                    None,
                    Some(&continuation),
                    basis_object_id,
                    request_object,
                    &current_snapshot,
                    ReconciliationPublicationPhaseV1::Read,
                    effect_reconciliation_authorized_carrier_value(
                        candidate.persistence_carrier_value()?,
                        &expected_authority,
                    )?,
                    Some(occurrence),
                    &next_revision,
                    &next_head,
                    meaning_digest,
                )
            });
        let publication = match publication {
            Ok(value) => value,
            Err(PreparedPublicationError::Store(error)) => return Err(map_store_error(error)),
            Err(PreparedPublicationError::Prepare(error)) => return Err(error),
        };
        decode_effect_reconciliation_outcome(
            publication,
            ReconciliationPublicationPhaseV1::Read,
            expected_authorization.request_id(),
            expected_snapshot.intent.id(),
            meaning_digest,
            None,
        )
    }

    pub fn publish_effect_reconciliation_terminal(
        &mut self,
        plan: ActiveStoreEffectReconciliationTerminalPublicationV1,
    ) -> Result<ActiveStoreEffectReconciliationOutcomeV1, ExecutionStoreErrorV1> {
        let authorization = plan
            .snapshot
            .reconciliation_authorization()
            .cloned()
            .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?;
        let authority = plan
            .snapshot
            .reconciliation_execution_authority()
            .cloned()
            .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?;
        let reconciliation = plan
            .snapshot
            .reconciliation
            .as_ref()
            .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?;
        let read_result_commitment = reconciliation
            .read_usage()
            .map(|usage| usage.result_commitment)
            .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?;
        let meaning_digest = hash(&CborValue::Array(vec![
            CborValue::text(
                "maestro.vnext.effect-reconciliation-terminal-authorized-publication.v1",
            )?,
            authorization.request().canonical_value()?,
            execution_authority_value(&authority)?,
            plan.draft.payload_value(read_result_commitment)?,
        ]))?;
        let probe = StoreIdempotencyProbeV1::new(
            EFFECT_RECONCILIATION_TERMINAL_IDEMPOTENCY_NAMESPACE_V1,
            *authorization.request().idempotency_key_id().as_bytes(),
            meaning_digest,
        )?;
        let expected_authorization = authorization;
        let expected_authority = authority;
        let expected_snapshot = plan.snapshot.clone();
        let expected_draft = plan.draft;
        let publication = self
            .store
            .publish_generation_atomically_with_prepare(&probe, |view| {
                let current_head = view
                    .active_head()?
                    .ok_or(ExecutionStoreErrorV1::InactiveStore)?;
                let current_generation = view
                    .active_generation()?
                    .ok_or(ExecutionStoreErrorV1::InactiveStore)?;
                let active_objects = view.active_generation_objects()?;
                let current_snapshot = load_active_effect_snapshot(
                    &current_head,
                    &current_generation,
                    &active_objects,
                    expected_snapshot.intent.id(),
                    |generation_id| Ok(view.generation(generation_id)?),
                )?;
                if current_snapshot != expected_snapshot {
                    return Err(ExecutionStoreErrorV1::StaleExpectedStoreState);
                }
                let reconciliation = current_snapshot
                    .reconciliation
                    .clone()
                    .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?;
                let read_result_commitment = reconciliation
                    .read_usage()
                    .map(|usage| usage.result_commitment)
                    .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?;
                if current_snapshot.reconciliation_authorization() != Some(&expected_authorization)
                    || current_snapshot.reconciliation_execution_authority()
                        != Some(&expected_authority)
                {
                    return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
                }
                let expected_request = expected_authorization.request();
                let request_object = execution_action_request_object(expected_request)?;
                if !active_objects
                    .iter()
                    .any(|object| object == &request_object)
                {
                    return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
                }
                let EffectIntentUseFenceV1::ActiveStore(fence) = reconciliation.use_fence() else {
                    return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
                };
                let basis_object_id = StoreObjectIdV1::from_digest(*fence.authority.as_bytes());
                let continuation = continue_repository_action_attempt(
                    view,
                    &current_generation,
                    basis_object_id,
                    &expected_authority,
                    *fence.epoch.as_bytes(),
                )?;
                let occurrence = effect_reconciliation_terminal_occurrence_object(
                    expected_request,
                    expected_draft,
                    read_result_commitment,
                    request_object.id(),
                    basis_object_id,
                    meaning_digest,
                )?;
                let idempotency_commitment = hash(&CborValue::Array(vec![
                    CborValue::text("maestro.vnext.effect-reconciliation-terminal-idempotency.v1")?,
                    bytes(expected_request.idempotency_key_id().as_bytes()),
                    bytes(&meaning_digest),
                    bytes(occurrence.id().as_bytes()),
                ]))?;
                let candidate = reconciliation.finish_candidate(
                    &current_snapshot.intent,
                    &current_snapshot.control_revision,
                    EffectReconciliationOutcomeV1 {
                        classification: expected_draft.classification(),
                        read_result_commitment,
                        result_commitment: *occurrence.id().as_bytes(),
                        idempotency_commitment,
                    },
                    &expected_authorization,
                )?;
                let transition = candidate.control_need().control_transition(
                    &current_snapshot.control_head,
                    &current_snapshot.control_revision,
                    current_snapshot.writer_term,
                )?;
                let publication =
                    EffectIntentControlPublicationCommitmentsV1::from_store_publication(
                        *occurrence.id().as_bytes(),
                        idempotency_commitment,
                    )?;
                let (next_revision, next_head) = transition.apply(
                    &current_snapshot.control_head,
                    &current_snapshot.control_revision,
                    current_snapshot.writer_term,
                    Some(publication),
                )?;
                build_effect_reconciliation_publication(
                    view.domain().clone(),
                    &current_head,
                    &current_generation,
                    &active_objects,
                    expected_request,
                    None,
                    Some(&continuation),
                    basis_object_id,
                    request_object,
                    &current_snapshot,
                    ReconciliationPublicationPhaseV1::Terminal,
                    effect_reconciliation_authorized_carrier_value(
                        candidate.persistence_carrier_value()?,
                        &expected_authority,
                    )?,
                    Some(occurrence),
                    &next_revision,
                    &next_head,
                    meaning_digest,
                )
            });
        let publication = match publication {
            Ok(value) => value,
            Err(PreparedPublicationError::Store(error)) => return Err(map_store_error(error)),
            Err(PreparedPublicationError::Prepare(error)) => return Err(error),
        };
        decode_effect_reconciliation_outcome(
            publication,
            ReconciliationPublicationPhaseV1::Terminal,
            expected_authorization.request_id(),
            expected_snapshot.intent.id(),
            meaning_digest,
            None,
        )
    }

    pub fn publish_effect_withdrawal(
        &mut self,
        plan: ActiveStoreEffectWithdrawalPublicationV1,
    ) -> Result<ActiveStoreEffectWithdrawalOutcomeV1, ExecutionStoreErrorV1> {
        let meaning_digest = hash(&CborValue::Array(vec![
            CborValue::text(EFFECT_WITHDRAWAL_IDEMPOTENCY_NAMESPACE_V1)?,
            plan.request.canonical_value()?,
            execution_authority_value(&plan.authority)?,
            plan.draft.payload_value()?,
        ]))?;
        let probe = StoreIdempotencyProbeV1::new(
            EFFECT_WITHDRAWAL_IDEMPOTENCY_NAMESPACE_V1,
            *plan.request.idempotency_key_id().as_bytes(),
            meaning_digest,
        )?;
        let expected_request = plan.request.clone();
        let expected_authority = plan.authority.clone();
        let expected_snapshot = plan.snapshot.clone();
        let expected_draft = plan.draft;
        let publication = self
            .store
            .publish_generation_atomically_with_prepare(&probe, |view| {
                let current_head = view
                    .active_head()?
                    .ok_or(ExecutionStoreErrorV1::InactiveStore)?;
                let current_generation = view
                    .active_generation()?
                    .ok_or(ExecutionStoreErrorV1::InactiveStore)?;
                let active_objects = view.active_generation_objects()?;
                let current_snapshot = load_active_effect_snapshot(
                    &current_head,
                    &current_generation,
                    &active_objects,
                    expected_snapshot.intent.id(),
                    |generation_id| Ok(view.generation(generation_id)?),
                )?;
                if current_snapshot != expected_snapshot {
                    return Err(ExecutionStoreErrorV1::StaleExpectedStoreState);
                }
                validate_request_values(
                    &expected_request,
                    &effect_intent_subject_value(&current_snapshot.intent)?,
                    &current_snapshot.state_binding.canonical_value()?,
                    &expected_draft.payload_value()?,
                )?;
                let request_object = execution_action_request_object(&expected_request)?;
                let admission = admit_repository_action(
                    view,
                    &current_generation,
                    RepositoryActionAdmissionInputV1::new(
                        expected_request.request_id(),
                        expected_authority.clone(),
                    ),
                )?;
                let authorized = AuthorizedExecutionActionV1::new(
                    expected_request.clone(),
                    admission.authorization_receipt().clone(),
                )?;
                let current_index = load_control_index(&active_objects)?;
                let current_entry = current_index
                    .entries
                    .iter()
                    .find(|entry| entry.intent == current_snapshot.intent.id())
                    .ok_or(ExecutionStoreErrorV1::MissingIntentControlSelector)?;
                let current_carrier_id = current_entry.control_head_object_id;
                let occurrence = effect_withdrawal_occurrence_object(
                    &expected_request,
                    request_object.id(),
                    admission.basis_object().id(),
                    current_carrier_id,
                    meaning_digest,
                )?;
                let idempotency_commitment = hash(&CborValue::Array(vec![
                    CborValue::text("maestro.vnext.effect-withdrawal-idempotency.v1")?,
                    bytes(expected_request.idempotency_key_id().as_bytes()),
                    bytes(&meaning_digest),
                    bytes(occurrence.id().as_bytes()),
                ]))?;
                let current_carrier = EffectWithdrawalCurrentCarrierV1::new(
                    current_snapshot.intent.home(),
                    expected_request.request_id(),
                    current_snapshot
                        .intent
                        .origin()
                        .withdrawal_authority_path()?,
                    *current_snapshot.control_revision.id().as_bytes(),
                    *current_carrier_id.as_bytes(),
                    *current_carrier_id.as_bytes(),
                )?;
                let withdrawal = current_snapshot.intent.prepare_withdrawal(
                    &current_snapshot.control_revision,
                    current_carrier,
                    &authorized,
                    *occurrence.id().as_bytes(),
                    idempotency_commitment,
                )?;
                let transition = withdrawal.control_need().control_transition(
                    &current_snapshot.control_head,
                    &current_snapshot.control_revision,
                    current_snapshot.writer_term,
                )?;
                let control_publication =
                    EffectIntentControlPublicationCommitmentsV1::from_store_publication(
                        *occurrence.id().as_bytes(),
                        idempotency_commitment,
                    )?;
                let (next_revision, next_head) = transition.apply(
                    &current_snapshot.control_head,
                    &current_snapshot.control_revision,
                    current_snapshot.writer_term,
                    Some(control_publication),
                )?;
                build_effect_withdrawal_publication(
                    view.domain().clone(),
                    &current_head,
                    &current_generation,
                    &active_objects,
                    &expected_request,
                    &expected_authority,
                    &admission,
                    request_object,
                    occurrence,
                    &current_snapshot,
                    &withdrawal,
                    &next_revision,
                    &next_head,
                    meaning_digest,
                )
            });
        let publication = match publication {
            Ok(value) => value,
            Err(PreparedPublicationError::Store(error)) => return Err(map_store_error(error)),
            Err(PreparedPublicationError::Prepare(error)) => return Err(error),
        };
        decode_effect_withdrawal_outcome(
            publication,
            expected_request.request_id(),
            expected_snapshot.intent.id(),
            meaning_digest,
        )
    }

    pub fn publish_effect_health(
        &mut self,
        plan: ActiveStoreEffectHealthPublicationV1,
    ) -> Result<ActiveStoreEffectHealthOutcomeV1, ExecutionStoreErrorV1> {
        let meaning_digest = hash(&CborValue::Array(vec![
            CborValue::text(EFFECT_HEALTH_IDEMPOTENCY_NAMESPACE_V1)?,
            plan.request.canonical_value()?,
            execution_authority_value(&plan.authority)?,
            plan.draft.payload_value()?,
        ]))?;
        let probe = StoreIdempotencyProbeV1::new(
            EFFECT_HEALTH_IDEMPOTENCY_NAMESPACE_V1,
            *plan.request.idempotency_key_id().as_bytes(),
            meaning_digest,
        )?;
        let expected_request = plan.request.clone();
        let expected_authority = plan.authority.clone();
        let expected_snapshot = plan.snapshot.clone();
        let expected_draft = plan.draft;
        let publication = self
            .store
            .publish_generation_atomically_with_prepare(&probe, |view| {
                let current_head = view
                    .active_head()?
                    .ok_or(ExecutionStoreErrorV1::InactiveStore)?;
                let current_generation = view
                    .active_generation()?
                    .ok_or(ExecutionStoreErrorV1::InactiveStore)?;
                let active_objects = view.active_generation_objects()?;
                let current_snapshot = load_active_effect_snapshot(
                    &current_head,
                    &current_generation,
                    &active_objects,
                    expected_snapshot.intent.id(),
                    |generation_id| Ok(view.generation(generation_id)?),
                )?;
                if current_snapshot != expected_snapshot {
                    return Err(ExecutionStoreErrorV1::StaleExpectedStoreState);
                }
                validate_request_values(
                    &expected_request,
                    &effect_intent_subject_value(&current_snapshot.intent)?,
                    &current_snapshot.state_binding.canonical_value()?,
                    &expected_draft.payload_value()?,
                )?;
                if expected_request.action()
                    != current_snapshot.intent.origin().reconciliation_action()?
                {
                    return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
                }
                let request_object = execution_action_request_object(&expected_request)?;
                let admission = admit_repository_action(
                    view,
                    &current_generation,
                    RepositoryActionAdmissionInputV1::new(
                        expected_request.request_id(),
                        expected_authority.clone(),
                    ),
                )?;
                let authorized = AuthorizedExecutionActionV1::new(
                    expected_request.clone(),
                    admission.authorization_receipt().clone(),
                )?;
                let (recovered_dispatch, recovery_occurrence, control_need) =
                    if expected_draft == ActiveStoreEffectHealthDraftV1::RecoverSealedInDoubt {
                        let occurrence = effect_recover_sealed_occurrence_object(
                            &expected_request,
                            &current_snapshot,
                            request_object.id(),
                            admission.basis_object().id(),
                            meaning_digest,
                        )?;
                        let idempotency_commitment = hash(&CborValue::Array(vec![
                            CborValue::text("maestro.vnext.effect-recover-sealed-idempotency.v1")?,
                            bytes(expected_request.idempotency_key_id().as_bytes()),
                            bytes(&meaning_digest),
                            bytes(occurrence.id().as_bytes()),
                        ]))?;
                        let candidate = current_snapshot
                            .dispatch
                            .recover_sealed_in_doubt_candidate(
                                &current_snapshot.intent,
                                &current_snapshot.control_revision,
                                *occurrence.id().as_bytes(),
                                idempotency_commitment,
                                &authorized,
                            )?;
                        let need = candidate.control_need().clone();
                        (Some(candidate), Some(occurrence), need)
                    } else {
                        (
                            None,
                            None,
                            expected_draft.control_need(expected_request.request_id())?,
                        )
                    };
                let transition = control_need.control_transition(
                    &current_snapshot.control_head,
                    &current_snapshot.control_revision,
                    current_snapshot.writer_term,
                )?;
                if !transition.accepts_action(expected_request.action()) {
                    return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
                }
                let publication_commitments = recovered_dispatch
                    .as_ref()
                    .map(|candidate| {
                        let EffectControlTransitionNeedV1::RecoverSealedInDoubt {
                            result_commitment,
                            idempotency_commitment,
                            ..
                        } = candidate.control_need()
                        else {
                            return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
                        };
                        Ok(
                            EffectIntentControlPublicationCommitmentsV1::from_store_publication(
                                *result_commitment,
                                *idempotency_commitment,
                            )?,
                        )
                    })
                    .transpose()?;
                let (candidate_revision, candidate_head) = transition.apply(
                    &current_snapshot.control_head,
                    &current_snapshot.control_revision,
                    current_snapshot.writer_term,
                    publication_commitments,
                )?;
                build_effect_health_publication(
                    view.domain().clone(),
                    &current_head,
                    &current_generation,
                    &active_objects,
                    &expected_request,
                    &expected_authority,
                    &admission,
                    request_object,
                    &current_snapshot,
                    &control_need,
                    recovered_dispatch.as_ref(),
                    recovery_occurrence,
                    &candidate_revision,
                    &candidate_head,
                    meaning_digest,
                )
            });
        let publication = match publication {
            Ok(value) => value,
            Err(PreparedPublicationError::Store(error)) => return Err(map_store_error(error)),
            Err(PreparedPublicationError::Prepare(error)) => return Err(error),
        };
        decode_effect_health_outcome(
            publication,
            expected_request.request_id(),
            expected_snapshot.intent.id(),
            meaning_digest,
        )
    }

    pub fn publish_effect_writer_handoff(
        &mut self,
        plan: ActiveStoreEffectWriterHandoffPublicationV1,
    ) -> Result<ActiveStoreEffectWriterHandoffOutcomeV1, ExecutionStoreErrorV1> {
        let meaning_digest = hash(&CborValue::Array(vec![
            CborValue::text(EFFECT_WRITER_HANDOFF_IDEMPOTENCY_NAMESPACE_V1)?,
            plan.request.canonical_value()?,
            execution_authority_value(&plan.authority)?,
            plan.draft.payload_value()?,
        ]))?;
        let probe = StoreIdempotencyProbeV1::new(
            EFFECT_WRITER_HANDOFF_IDEMPOTENCY_NAMESPACE_V1,
            *plan.request.idempotency_key_id().as_bytes(),
            meaning_digest,
        )?;
        let expected_request = plan.request.clone();
        let expected_authority = plan.authority.clone();
        let expected_snapshot = plan.snapshot.clone();
        let expected_draft = plan.draft;
        let publication = self
            .store
            .publish_generation_atomically_with_prepare(&probe, |view| {
                let current_head = view
                    .active_head()?
                    .ok_or(ExecutionStoreErrorV1::InactiveStore)?;
                let current_generation = view
                    .active_generation()?
                    .ok_or(ExecutionStoreErrorV1::InactiveStore)?;
                let active_objects = view.active_generation_objects()?;
                let current_snapshot = load_active_effect_snapshot(
                    &current_head,
                    &current_generation,
                    &active_objects,
                    expected_snapshot.intent.id(),
                    |generation_id| Ok(view.generation(generation_id)?),
                )?;
                if current_snapshot != expected_snapshot {
                    return Err(ExecutionStoreErrorV1::StaleExpectedStoreState);
                }
                validate_request_values(
                    &expected_request,
                    &effect_intent_subject_value(&current_snapshot.intent)?,
                    &current_snapshot.state_binding.canonical_value()?,
                    &expected_draft.payload_value()?,
                )?;
                if expected_request.action() != effect_writer_handoff_action(&current_snapshot)? {
                    return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
                }
                let request_object = execution_action_request_object(&expected_request)?;
                let admission = admit_repository_action(
                    view,
                    &current_generation,
                    RepositoryActionAdmissionInputV1::new(
                        expected_request.request_id(),
                        expected_authority.clone(),
                    ),
                )?;
                let receipt = SameHomeWriterFencingReceiptV1::issue(
                    current_snapshot.intent.id(),
                    current_snapshot.control_head.home(),
                    current_snapshot.control_head.id(),
                    current_snapshot.writer_term.id(),
                    *current_head.id().as_bytes(),
                    *current_generation.id().as_bytes(),
                    current_generation
                        .ordinal()
                        .checked_add(1)
                        .ok_or(ExecutionStoreErrorV1::GenerationOverflow)?,
                )?;
                let continuity_commitment = hash(&CborValue::Array(vec![
                    CborValue::text("maestro.vnext.same-home-writer-continuity.v1")?,
                    bytes(receipt.id().as_bytes()),
                    bytes(expected_request.request_id().as_bytes()),
                    bytes(current_generation.domain().id().as_bytes()),
                ]))?;
                let successor_writer = EffectIntentControlWriterTermV1::same_home_restore(
                    receipt,
                    continuity_commitment,
                )?;
                let control_need = EffectControlTransitionNeedV1::HandoffWriter {
                    action_request_id: expected_request.request_id(),
                    fencing_receipt: receipt,
                    successor_writer,
                };
                let transition = control_need.control_transition(
                    &current_snapshot.control_head,
                    &current_snapshot.control_revision,
                    current_snapshot.writer_term,
                )?;
                if !transition.accepts_action(expected_request.action()) {
                    return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
                }
                let (candidate_revision, candidate_head) = transition.apply(
                    &current_snapshot.control_head,
                    &current_snapshot.control_revision,
                    current_snapshot.writer_term,
                    None,
                )?;
                build_effect_writer_handoff_publication(
                    view.domain().clone(),
                    &current_head,
                    &current_generation,
                    &active_objects,
                    &expected_request,
                    &expected_authority,
                    &admission,
                    request_object,
                    &current_snapshot,
                    &control_need,
                    receipt,
                    successor_writer,
                    &candidate_revision,
                    &candidate_head,
                    meaning_digest,
                )
            });
        let publication = match publication {
            Ok(value) => value,
            Err(PreparedPublicationError::Store(error)) => return Err(map_store_error(error)),
            Err(PreparedPublicationError::Prepare(error)) => return Err(error),
        };
        decode_effect_writer_handoff_outcome(
            publication,
            expected_request.request_id(),
            expected_snapshot.intent.id(),
            meaning_digest,
        )
    }

    pub fn publish_step_execution(
        &mut self,
        plan: StepExecutionPublicationV1,
    ) -> Result<StepExecutionPublicationOutcomeV1, ExecutionStoreErrorV1> {
        let meaning_digest = hash(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.step-execution-authorized-publication.v1")?,
            plan.request.canonical_value()?,
            execution_authority_value(&plan.authority.clone().into())?,
            plan.mutation.canonical_value()?,
        ]))?;
        let probe = StoreIdempotencyProbeV1::new(
            "maestro.vnext.step-execution-authorized-publication.v1",
            *plan.request.idempotency_key_id().as_bytes(),
            meaning_digest,
        )?;
        let expected_snapshot = plan.snapshot.clone();
        let expected_request = plan.request.clone();
        let expected_mutation = plan.mutation.clone();
        let expected_authority = plan.authority.clone();
        let publication = self
            .store
            .publish_generation_atomically_with_prepare(&probe, |view| {
                let current_head = view
                    .active_head()?
                    .ok_or(ExecutionStoreErrorV1::InactiveStore)?;
                let current_generation = view
                    .active_generation()?
                    .ok_or(ExecutionStoreErrorV1::InactiveStore)?;
                let active_objects = view.active_generation_objects()?;
                validate_step_snapshot_against_view(
                    &expected_snapshot,
                    &current_head,
                    &current_generation,
                    &active_objects,
                )?;
                if !matches!(
                    &expected_mutation,
                    AuthorizedStepExecutionMutationV1::Abandon { .. }
                ) {
                    require_current_step_graph(
                        &current_generation,
                        &active_objects,
                        expected_snapshot.binding,
                    )?;
                    if !validate_rooted_step_state(
                        &current_generation,
                        &active_objects,
                        expected_snapshot.binding,
                    )? {
                        return Err(ExecutionStoreErrorV1::StepBindingNotOpen);
                    }
                }
                let request_object = execution_action_request_object(&expected_request)?;
                let admission = admit_repository_action(
                    view,
                    &current_generation,
                    RepositoryActionAdmissionInputV1::new(
                        expected_request.request_id(),
                        expected_authority.clone(),
                    ),
                )?;
                let authorized = AuthorizedExecutionActionV1::new(
                    expected_request.clone(),
                    admission.authorization_receipt().clone(),
                )?;
                let next_carrier = apply_authorized_step_mutation(
                    &expected_snapshot,
                    &current_generation,
                    admission.authority_epoch(),
                    admission.accepted_h_time(),
                    expected_mutation.clone(),
                    authorized,
                )?;
                let carrier_object = step_execution_carrier_object(&next_carrier)?;
                let artifacts = admission.issue_committed_artifacts(
                    &request_object,
                    std::slice::from_ref(&carrier_object),
                )?;
                let atomic = build_step_execution_authorized_publication(
                    view.domain().clone(),
                    &current_head,
                    &current_generation,
                    &active_objects,
                    &expected_snapshot,
                    &expected_request,
                    &admission,
                    &artifacts,
                    request_object,
                    carrier_object,
                    meaning_digest,
                )?;
                Ok(atomic)
            });
        let publication = match publication {
            Ok(value) => value,
            Err(PreparedPublicationError::Store(error)) => return Err(map_store_error(error)),
            Err(PreparedPublicationError::Prepare(error)) => return Err(error),
        };
        let carrier = load_carrier_from_publication_result(self.store, publication.result())?;
        Ok(StepExecutionPublicationOutcomeV1 {
            store_head: publication.head().clone(),
            carrier,
            replayed: matches!(publication, StorePublicationOutcomeV1::Replayed { .. }),
        })
    }

    pub fn publish_step_submission(
        &mut self,
        plan: StepSubmissionPublicationV1,
    ) -> Result<StepSubmissionPublicationOutcomeV1, ExecutionStoreErrorV1> {
        let meaning_digest = hash(&CborValue::Array(vec![
            CborValue::text(STEP_SUBMISSION_IDEMPOTENCY_NAMESPACE_V1)?,
            plan.request.canonical_value()?,
            submit_step_authority_value(&plan.authority)?,
            plan.submission.canonical_value()?,
            plan.evidence.canonical_value()?,
            CborValue::Unsigned(plan.as_of),
        ]))?;
        let probe = StoreIdempotencyProbeV1::new(
            STEP_SUBMISSION_IDEMPOTENCY_NAMESPACE_V1,
            *plan.request.idempotency_key_id().as_bytes(),
            meaning_digest,
        )?;
        let expected_snapshot = plan.snapshot.clone();
        let expected_request = plan.request.clone();
        let expected_authority = plan.authority.clone();
        let expected_submission = plan.submission.clone();
        let expected_evidence = plan.evidence.clone();
        let expected_as_of = plan.as_of;
        let publication = self
            .store
            .publish_generation_atomically_with_prepare(&probe, |view| {
                let current_head = view
                    .active_head()?
                    .ok_or(ExecutionStoreErrorV1::InactiveStore)?;
                let current_generation = view
                    .active_generation()?
                    .ok_or(ExecutionStoreErrorV1::InactiveStore)?;
                let active_objects = view.active_generation_objects()?;
                validate_step_snapshot_against_view(
                    &expected_snapshot,
                    &current_head,
                    &current_generation,
                    &active_objects,
                )?;
                require_current_step_graph(
                    &current_generation,
                    &active_objects,
                    expected_snapshot.binding,
                )?;
                let current_step_state = rooted_step_state_object(
                    &current_generation,
                    &active_objects,
                    expected_snapshot.binding,
                )?;
                if !rooted_step_state_is_open(current_step_state)? {
                    return Err(ExecutionStoreErrorV1::StepBindingNotOpen);
                }
                let payload = step_submission_payload_value(&expected_submission, expected_as_of)?;
                if expected_request.subject_commitment()
                    != hash(&step_binding_store_value(expected_snapshot.binding))?
                    || expected_request.expected_state_commitment()
                        != hash(&expected_snapshot.state_binding.canonical_value()?)?
                    || expected_request.payload_commitment() != hash(&payload)?
                    || expected_authority.subject_commitment()
                        != expected_request.subject_commitment()
                    || expected_authority.subject_basis_commitment()
                        != expected_request.expected_state_commitment()
                    || expected_authority.exact_payload_commitment()
                        != expected_request.payload_commitment()
                {
                    return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
                }
                let request_object = step_submission_action_request_object(&expected_request)?;
                let admission = admit_repository_action(
                    view,
                    &current_generation,
                    RepositoryActionAdmissionInputV1::new(
                        expected_request.request_id(),
                        expected_authority.clone(),
                    ),
                )?;
                if admission.accepted_h_time() != expected_as_of {
                    return Err(ExecutionStoreErrorV1::UntrustedMutationTime);
                }
                let current_carrier = expected_snapshot
                    .carrier()
                    .ok_or(ExecutionStoreErrorV1::MissingStepExecutionCarrier)?;
                if expected_authority.executor_principal_id()
                    != current_carrier.tenure().attempt().executor()
                    || expected_submission.binding() != expected_snapshot.binding
                    || expected_submission.execution_carrier_commitment()
                        != hash(&current_carrier.canonical_value()?)?
                    || expected_submission.claim_set_digest()
                        != *expected_evidence.claim_set().digest()
                {
                    return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
                }
                let mut submitted_carrier = current_carrier.clone();
                submitted_carrier
                    .close_for_submission(expected_submission.execution_fence(), expected_as_of)?;
                let carrier_object = step_execution_carrier_object(&submitted_carrier)?;
                let claim_objects = step_submission_claim_objects(&expected_evidence)?;
                validate_step_submission_observations(
                    &expected_evidence,
                    expected_snapshot.binding,
                    current_carrier,
                    expected_as_of,
                )?;
                let observation_objects = resolve_current_observation_objects(
                    &active_objects,
                    expected_evidence.observations(),
                )?;
                let claim_set_object = step_submission_claim_set_object(
                    &expected_evidence,
                    &claim_objects,
                    &observation_objects,
                )?;
                let submission_object = step_submission_object(
                    &expected_submission,
                    &carrier_object,
                    &claim_set_object,
                )?;
                let next_step_state =
                    decode_rooted_open_step_state(current_step_state, expected_snapshot.binding)?
                        .submit(&expected_submission)?;
                let step_state_object =
                    submitted_step_state_object(&next_step_state, &submission_object)?;
                let mut produced_objects = vec![
                    carrier_object.clone(),
                    step_state_object.clone(),
                    submission_object.clone(),
                    claim_set_object.clone(),
                ];
                produced_objects.extend(claim_objects.iter().cloned());
                let artifacts =
                    admission.issue_committed_artifacts(&request_object, &produced_objects)?;
                build_step_submission_authorized_publication(
                    view.domain().clone(),
                    &current_head,
                    &current_generation,
                    &active_objects,
                    &expected_snapshot,
                    current_step_state,
                    &expected_request,
                    &admission,
                    &artifacts,
                    request_object,
                    carrier_object,
                    step_state_object,
                    submission_object,
                    claim_set_object,
                    claim_objects,
                    meaning_digest,
                )
            });
        let publication = match publication {
            Ok(value) => value,
            Err(PreparedPublicationError::Store(error)) => return Err(map_store_error(error)),
            Err(PreparedPublicationError::Prepare(error)) => return Err(error),
        };
        let (submission, carrier) = decode_step_submission_outcome(
            self.store,
            publication.result(),
            &expected_request,
            &expected_submission,
            &expected_evidence,
        )?;
        Ok(StepSubmissionPublicationOutcomeV1 {
            store_head: publication.head().clone(),
            submission,
            carrier,
            replayed: matches!(publication, StorePublicationOutcomeV1::Replayed { .. }),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ControlIndexEntryV1 {
    intent: EffectIntentIdV1,
    semantic_uniqueness_commitment: [u8; 32],
    control_head: EffectIntentControlTokenV1,
    control_head_object_id: StoreObjectIdV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ActiveControlIndexV1 {
    object: StoreObjectV1,
    entries: Vec<ControlIndexEntryV1>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct StepExecutionIndexEntryV1 {
    binding_commitment: [u8; 32],
    carrier_object_id: StoreObjectIdV1,
    fence_high_water: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ActiveStepExecutionIndexV1 {
    object: StoreObjectV1,
    entries: Vec<StepExecutionIndexEntryV1>,
}

pub(crate) fn current_run_binding_is_persisted(
    store: &StoreV1,
    run_id: RunIdV1,
    owner: super::runtime::ExecutionAttemptOwnerV1,
) -> Result<bool, ExecutionStoreErrorV1> {
    let (state, head, generation, objects) = store.coherent_publication_snapshot()?;
    if state != StoreStateV1::Active {
        return Err(ExecutionStoreErrorV1::InactiveStore);
    }
    let mut matches = 0_usize;
    if let Some(index) = load_optional_step_execution_index(&objects)? {
        for entry in &index.entries {
            let carrier = load_step_execution_carrier(&objects, entry)?;
            matches += carrier
                .run_set()
                .runs()
                .iter()
                .filter(|run| run.id() == run_id && run.owner() == owner)
                .count();
        }
    }
    if let Some(index) = load_optional_control_index(&objects)? {
        for entry in &index.entries {
            let snapshot = load_active_effect_snapshot(
                &head,
                &generation,
                &objects,
                entry.intent,
                |generation_id| Ok(store.generation(generation_id)?),
            )?;
            matches += snapshot
                .dispatch()
                .run_set()
                .runs()
                .iter()
                .filter(|run| run.id() == run_id && run.owner() == owner)
                .count();
            if let Some(reconciliation) = snapshot.reconciliation() {
                matches += reconciliation
                    .run_set()
                    .runs()
                    .iter()
                    .filter(|run| run.id() == run_id && run.owner() == owner)
                    .count();
            }
        }
    }
    Ok(matches == 1)
}

fn load_optional_step_execution_index(
    active_objects: &[StoreObjectV1],
) -> Result<Option<ActiveStepExecutionIndexV1>, ExecutionStoreErrorV1> {
    let schema = execution_schema_id("maestro.vnext.step-execution-index-schema.v1")?;
    let candidates = active_objects
        .iter()
        .filter(|object| object.schema_id() == schema)
        .collect::<Vec<_>>();
    let ([] | [_]) = candidates.as_slice() else {
        return Err(ExecutionStoreErrorV1::AmbiguousStepExecutionIndex);
    };
    let Some(object) = candidates.first() else {
        return Ok(None);
    };
    let CborValue::Array(fields) = object.value() else {
        return Err(ExecutionStoreErrorV1::InvalidStepExecutionIndex);
    };
    let [CborValue::Text(domain), CborValue::Array(rows)] = fields.as_slice() else {
        return Err(ExecutionStoreErrorV1::InvalidStepExecutionIndex);
    };
    if domain != "maestro.vnext.step-execution-index.v1" || rows.is_empty() {
        return Err(ExecutionStoreErrorV1::InvalidStepExecutionIndex);
    }
    let mut entries = Vec::with_capacity(rows.len());
    for row in rows {
        let CborValue::Array(fields) = row else {
            return Err(ExecutionStoreErrorV1::InvalidStepExecutionIndex);
        };
        let [binding, carrier, CborValue::Unsigned(fence)] = fields.as_slice() else {
            return Err(ExecutionStoreErrorV1::InvalidStepExecutionIndex);
        };
        if *fence == 0 {
            return Err(ExecutionStoreErrorV1::InvalidStepExecutionIndex);
        }
        entries.push(StepExecutionIndexEntryV1 {
            binding_commitment: exact_digest(binding)
                .ok_or(ExecutionStoreErrorV1::InvalidStepExecutionIndex)?,
            carrier_object_id: StoreObjectIdV1::from_digest(
                exact_digest(carrier).ok_or(ExecutionStoreErrorV1::InvalidStepExecutionIndex)?,
            ),
            fence_high_water: *fence,
        });
    }
    if entries.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ExecutionStoreErrorV1::InvalidStepExecutionIndex);
    }
    let expected_references = entries
        .iter()
        .map(|entry| entry.carrier_object_id)
        .collect::<Vec<_>>();
    if object.references() != expected_references {
        return Err(ExecutionStoreErrorV1::InvalidStepExecutionIndex);
    }
    Ok(Some(ActiveStepExecutionIndexV1 {
        object: (*object).clone(),
        entries,
    }))
}

fn build_step_execution_index_object(
    entries: &[StepExecutionIndexEntryV1],
) -> Result<StoreObjectV1, ExecutionStoreErrorV1> {
    let mut entries = entries.to_vec();
    entries.sort_unstable();
    if entries.is_empty()
        || entries
            .windows(2)
            .any(|pair| pair[0].binding_commitment == pair[1].binding_commitment)
    {
        return Err(ExecutionStoreErrorV1::InvalidStepExecutionIndex);
    }
    let references = entries
        .iter()
        .map(|entry| entry.carrier_object_id)
        .collect::<Vec<_>>();
    Ok(StoreObjectV1::new(
        execution_schema_id("maestro.vnext.step-execution-index-schema.v1")?,
        CborValue::Array(vec![
            CborValue::text("maestro.vnext.step-execution-index.v1")?,
            CborValue::Array(
                entries
                    .iter()
                    .map(|entry| {
                        CborValue::Array(vec![
                            bytes(&entry.binding_commitment),
                            bytes(entry.carrier_object_id.as_bytes()),
                            CborValue::Unsigned(entry.fence_high_water),
                        ])
                    })
                    .collect(),
            ),
        ]),
        references,
    )?)
}

fn load_step_execution_carrier(
    active_objects: &[StoreObjectV1],
    entry: &StepExecutionIndexEntryV1,
) -> Result<StepExecutionCarrierV1, ExecutionStoreErrorV1> {
    let schema = execution_schema_id("maestro.vnext.step-execution-carrier-schema.v1")?;
    let matches = active_objects
        .iter()
        .filter(|object| object.id() == entry.carrier_object_id)
        .collect::<Vec<_>>();
    let [object] = matches.as_slice() else {
        return Err(ExecutionStoreErrorV1::InvalidStepExecutionIndex);
    };
    if object.schema_id() != schema || !object.references().is_empty() {
        return Err(ExecutionStoreErrorV1::InvalidStepExecutionIndex);
    }
    let carrier = StepExecutionCarrierV1::from_canonical_value(object.value())?;
    if carrier.tenure().attempt().fence() != entry.fence_high_water {
        return Err(ExecutionStoreErrorV1::InvalidStepExecutionIndex);
    }
    Ok(carrier)
}

fn step_execution_carrier_object(
    carrier: &StepExecutionCarrierV1,
) -> Result<StoreObjectV1, ExecutionStoreErrorV1> {
    Ok(StoreObjectV1::new(
        execution_schema_id("maestro.vnext.step-execution-carrier-schema.v1")?,
        carrier.canonical_value()?,
        vec![],
    )?)
}

fn load_carrier_from_publication_result(
    store: &StoreV1,
    result: &StoreObjectV1,
) -> Result<StepExecutionCarrierV1, ExecutionStoreErrorV1> {
    let carrier_schema = execution_schema_id("maestro.vnext.step-execution-carrier-schema.v1")?;
    let carriers = result
        .references()
        .iter()
        .map(|object_id| store.read_object(*object_id))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|object| object.schema_id() == carrier_schema)
        .collect::<Vec<_>>();
    let [carrier] = carriers.as_slice() else {
        return Err(ExecutionStoreErrorV1::InvalidStepExecutionPublicationResult);
    };
    StepExecutionCarrierV1::from_canonical_value(carrier.value()).map_err(Into::into)
}

fn step_submission_payload_value(
    submission: &StepSubmissionV1,
    as_of: u64,
) -> Result<CborValue, ExecutionStoreErrorV1> {
    Ok(CborValue::Array(vec![
        CborValue::text("maestro.vnext.step-submission-publication-payload.v1")?,
        CborValue::Unsigned(as_of),
        submission.canonical_value()?,
    ]))
}

fn submit_step_authority_value(
    authority: &SubmitStepAuthorityV1,
) -> Result<CborValue, ExecutionStoreErrorV1> {
    Ok(CborValue::Array(vec![
        CborValue::text("maestro.vnext.submit-step-authority.v1")?,
        bytes(authority.selection().actor_binding_id().as_bytes()),
        bytes(authority.selection().actor_session_id().as_bytes()),
        bytes(authority.selection().terminal_grant_id().as_bytes()),
        bytes(&authority.subject_commitment()),
        bytes(&authority.subject_basis_commitment()),
        bytes(&authority.exact_payload_commitment()),
        bytes(authority.executor_principal_id().as_bytes()),
    ]))
}

fn step_submission_action_request_object(
    request: &CanonicalStepSubmissionActionRequestV1,
) -> Result<StoreObjectV1, ExecutionStoreErrorV1> {
    Ok(StoreObjectV1::new(
        execution_schema_id(STEP_SUBMISSION_ACTION_REQUEST_SCHEMA_V1)?,
        request.canonical_value()?,
        vec![],
    )?)
}

fn step_submission_claim_objects(
    evidence: &EvidenceClaimPublicationV1,
) -> Result<Vec<StoreObjectV1>, ExecutionStoreErrorV1> {
    evidence
        .claims()
        .iter()
        .map(|claim| {
            StoreObjectV1::new(
                execution_schema_id(STEP_SUBMISSION_CLAIM_SCHEMA_V1)?,
                claim.canonical_value(),
                vec![],
            )
            .map_err(ExecutionStoreErrorV1::from)
        })
        .collect()
}

fn step_submission_observation_objects(
    evidence: &EvidenceClaimPublicationV1,
) -> Result<Vec<StoreObjectV1>, ExecutionStoreErrorV1> {
    evidence
        .observations()
        .iter()
        .map(|observation| {
            StoreObjectV1::new(
                execution_schema_id(STEP_SUBMISSION_OBSERVATION_SCHEMA_V1)?,
                CborValue::Bytes(observation.canonical_bytes().map_err(ClaimError::from)?),
                vec![],
            )
            .map_err(ExecutionStoreErrorV1::from)
        })
        .collect()
}

fn step_submission_claim_set_object(
    evidence: &EvidenceClaimPublicationV1,
    claim_objects: &[StoreObjectV1],
    observation_objects: &[StoreObjectV1],
) -> Result<StoreObjectV1, ExecutionStoreErrorV1> {
    let mut references = claim_objects
        .iter()
        .chain(observation_objects)
        .map(StoreObjectV1::id)
        .collect::<Vec<_>>();
    references.sort_unstable();
    references.dedup();
    Ok(StoreObjectV1::new(
        execution_schema_id(STEP_SUBMISSION_CLAIM_SET_SCHEMA_V1)?,
        CborValue::Array(vec![
            CborValue::text("maestro.vnext.step-submission-claim-set.v1")?,
            evidence.claim_set_value()?,
        ]),
        references,
    )?)
}

fn validate_step_submission_observations(
    evidence: &EvidenceClaimPublicationV1,
    binding: StepBindingV1,
    carrier: &StepExecutionCarrierV1,
    as_of: u64,
) -> Result<(), ExecutionStoreErrorV1> {
    let expected_attempt = carrier.tenure().attempt().id();
    for observation in evidence.observations() {
        let exact_step_subject = observation.subjects().iter().any(|subject| {
            subject.kind() == ObservationSubjectKindV1::Step
                && subject.subject_id() == binding.step_id().as_bytes()
                && subject.revision_id() == binding.revision_id().as_bytes()
        });
        if observation.store_domain_id() != binding.scope().repository_id()
            || !exact_step_subject
            || observation.observed_at() > as_of
            || observation.recorded_at() > as_of
        {
            return Err(ExecutionStoreErrorV1::ObservationNotApplicableToStep);
        }
        if let Some((run_id, owner)) = observation.acquisition().run_binding()
            && (owner != super::runtime::ExecutionAttemptOwnerV1::Step(expected_attempt)
                || !carrier.run_set().runs().iter().any(|run| {
                    run.id() == run_id
                        && run.owner()
                            == super::runtime::ExecutionAttemptOwnerV1::Step(expected_attempt)
                }))
        {
            return Err(ExecutionStoreErrorV1::ObservationNotApplicableToStep);
        }
    }
    Ok(())
}

fn step_submission_object(
    submission: &StepSubmissionV1,
    carrier_object: &StoreObjectV1,
    claim_set_object: &StoreObjectV1,
) -> Result<StoreObjectV1, ExecutionStoreErrorV1> {
    let mut references = vec![carrier_object.id(), claim_set_object.id()];
    references.sort_unstable();
    references.dedup();
    Ok(StoreObjectV1::new(
        execution_schema_id(STEP_SUBMISSION_SCHEMA_V1)?,
        submission.canonical_value()?,
        references,
    )?)
}

fn submitted_step_state_object(
    state: &StepStateV1,
    submission_object: &StoreObjectV1,
) -> Result<StoreObjectV1, ExecutionStoreErrorV1> {
    let StepLifecycleV1::Submitted {
        submission_id,
        submission_record_hash,
    } = state.lifecycle()
    else {
        return Err(ExecutionStoreErrorV1::InvalidStepSubmissionResult);
    };
    Ok(StoreObjectV1::new(
        RepositoryStoreSchemaV1::StepState
            .schema_id()
            .map_err(|_| ExecutionStoreErrorV1::PublicationBindingMismatch)?,
        CborValue::Array(vec![
            CborValue::text(RepositoryStoreSchemaV1::StepState.domain())?,
            step_binding_store_value(state.binding()),
            CborValue::Array(vec![
                CborValue::Unsigned(2),
                bytes(submission_id.as_bytes()),
                bytes(&submission_record_hash),
            ]),
        ]),
        vec![submission_object.id()],
    )?)
}

fn decode_step_submission_outcome(
    store: &StoreV1,
    result: &StoreObjectV1,
    expected_request: &CanonicalStepSubmissionActionRequestV1,
    expected: &StepSubmissionV1,
    expected_evidence: &EvidenceClaimPublicationV1,
) -> Result<(StepSubmissionV1, StepExecutionCarrierV1), ExecutionStoreErrorV1> {
    let referenced = result
        .references()
        .iter()
        .map(|object_id| store.read_object(*object_id))
        .collect::<Result<Vec<_>, _>>()?;
    let submission_schema = execution_schema_id(STEP_SUBMISSION_SCHEMA_V1)?;
    let carrier_schema = execution_schema_id("maestro.vnext.step-execution-carrier-schema.v1")?;
    let request_schema = execution_schema_id(STEP_SUBMISSION_ACTION_REQUEST_SCHEMA_V1)?;
    let claim_schema = execution_schema_id(STEP_SUBMISSION_CLAIM_SCHEMA_V1)?;
    let claim_set_schema = execution_schema_id(STEP_SUBMISSION_CLAIM_SET_SCHEMA_V1)?;
    let step_state_schema = RepositoryStoreSchemaV1::StepState
        .schema_id()
        .map_err(|_| ExecutionStoreErrorV1::InvalidStepSubmissionResult)?;
    let submissions = referenced
        .iter()
        .filter(|object| object.schema_id() == submission_schema)
        .collect::<Vec<_>>();
    let carriers = referenced
        .iter()
        .filter(|object| object.schema_id() == carrier_schema)
        .collect::<Vec<_>>();
    let requests = referenced
        .iter()
        .filter(|object| object.schema_id() == request_schema)
        .collect::<Vec<_>>();
    let claims = referenced
        .iter()
        .filter(|object| object.schema_id() == claim_schema)
        .collect::<Vec<_>>();
    let claim_sets = referenced
        .iter()
        .filter(|object| object.schema_id() == claim_set_schema)
        .collect::<Vec<_>>();
    let step_states = referenced
        .iter()
        .filter(|object| object.schema_id() == step_state_schema)
        .collect::<Vec<_>>();
    let ([submission_object], [carrier_object], [request_object], [claim_set], [step_state]) = (
        submissions.as_slice(),
        carriers.as_slice(),
        requests.as_slice(),
        claim_sets.as_slice(),
        step_states.as_slice(),
    ) else {
        return Err(ExecutionStoreErrorV1::InvalidStepSubmissionResult);
    };
    let request =
        CanonicalStepSubmissionActionRequestV1::from_canonical_value(request_object.value())?;
    let submission = StepSubmissionV1::from_canonical_value(submission_object.value())?;
    let carrier = StepExecutionCarrierV1::from_canonical_value(carrier_object.value())?;
    let expected_claim_objects = step_submission_claim_objects(expected_evidence)?;
    let expected_observation_objects = step_submission_observation_objects(expected_evidence)?;
    let expected_claim_set = step_submission_claim_set_object(
        expected_evidence,
        &expected_claim_objects,
        &expected_observation_objects,
    )?;
    let expected_submission_object =
        step_submission_object(&submission, carrier_object, &expected_claim_set)?;
    let expected_step_state = submitted_step_state_object(
        &StepStateV1::new_open(submission.binding()).submit(&submission)?,
        &expected_submission_object,
    )?;
    let mut actual_claim_ids = claims.iter().map(|object| object.id()).collect::<Vec<_>>();
    let decoded_claims = claims
        .iter()
        .map(|object| ClaimV1::from_canonical_value(object.value()))
        .collect::<Result<Vec<_>, _>>()?;
    let decoded_evidence = EvidenceClaimPublicationV1::new(
        SubmissionRefV1::Step(submission.id()),
        decoded_claims,
        expected_evidence.observations().to_vec(),
    )?;
    let mut expected_claim_ids = expected_claim_objects
        .iter()
        .map(StoreObjectV1::id)
        .collect::<Vec<_>>();
    actual_claim_ids.sort_unstable();
    expected_claim_ids.sort_unstable();
    if &request != expected_request
        || request.request_id() != expected_request.request_id()
        || &submission != expected
        || &decoded_evidence != expected_evidence
        || submission.claim_set_digest() != *expected_evidence.claim_set().digest()
        || actual_claim_ids != expected_claim_ids
        || **claim_set != expected_claim_set
        || **submission_object != expected_submission_object
        || **step_state != expected_step_state
        || submission.execution_fence().attempt_id() != carrier.tenure().attempt().id()
        || carrier.tenure().attempt().state()
            != super::runtime::StepAttemptStateV1::Terminal(StepAttemptTerminalV1::Submitted)
        || carrier.tenure().lease().state()
            != super::runtime::StepAttemptStateV1::Terminal(StepAttemptTerminalV1::Submitted)
    {
        return Err(ExecutionStoreErrorV1::InvalidStepSubmissionResult);
    }
    Ok((submission, carrier))
}

fn validate_current_step_snapshot(
    store: &StoreV1,
    snapshot: &StepExecutionSnapshotV1,
) -> Result<(), ExecutionStoreErrorV1> {
    let (state, head, generation, active_objects) = store.coherent_publication_snapshot()?;
    if state != StoreStateV1::Active {
        return Err(ExecutionStoreErrorV1::InactiveStore);
    }
    validate_step_snapshot_against_view(snapshot, &head, &generation, &active_objects)?;
    Ok(())
}

fn validate_step_snapshot_against_view(
    snapshot: &StepExecutionSnapshotV1,
    head: &StoreHeadV1,
    generation: &StoreGenerationV1,
    active_objects: &[StoreObjectV1],
) -> Result<Option<ActiveStepExecutionIndexV1>, ExecutionStoreErrorV1> {
    let binding_commitment = hash(&step_binding_store_value(snapshot.binding))?;
    if head.id() != snapshot.state_binding.store_head_id
        || generation.id() != snapshot.state_binding.store_generation_id
        || binding_commitment != snapshot.state_binding.step_binding_commitment
        || snapshot.binding.scope().repository_id() != generation.domain().id()
    {
        return Err(ExecutionStoreErrorV1::StaleExpectedStoreState);
    }
    validate_rooted_step_state(generation, active_objects, snapshot.binding)?;
    let index = load_optional_step_execution_index(active_objects)?;
    if index.as_ref().map(|index| index.object.id()) != snapshot.state_binding.step_index_object_id
    {
        return Err(ExecutionStoreErrorV1::StaleExpectedStoreState);
    }
    let entry = index.as_ref().and_then(|index| {
        index
            .entries
            .iter()
            .find(|entry| entry.binding_commitment == binding_commitment)
    });
    match (entry, snapshot.carrier.as_ref()) {
        (None, None)
            if snapshot.state_binding.carrier_object_id.is_none()
                && snapshot.state_binding.fence_high_water == 0 => {}
        (Some(entry), Some(expected))
            if snapshot.state_binding.carrier_object_id == Some(entry.carrier_object_id)
                && snapshot.state_binding.fence_high_water == entry.fence_high_water =>
        {
            let current = load_step_execution_carrier(active_objects, entry)?;
            if &current != expected
                || current.tenure().attempt().binding() != snapshot.binding
                || current.tenure().attempt().fence() != entry.fence_high_water
            {
                return Err(ExecutionStoreErrorV1::StaleExpectedStoreState);
            }
        }
        _ => return Err(ExecutionStoreErrorV1::StaleExpectedStoreState),
    }
    Ok(index)
}

fn require_current_step_graph(
    generation: &StoreGenerationV1,
    active_objects: &[StoreObjectV1],
    binding: StepBindingV1,
) -> Result<(), ExecutionStoreErrorV1> {
    if binding.contract_root_id() != generation.contract_root_id() {
        return Err(ExecutionStoreErrorV1::StepBindingNotCurrent);
    }
    let graph_schema = RepositoryStoreSchemaV1::StepGraph
        .schema_id()
        .map_err(|_| ExecutionStoreErrorV1::PublicationBindingMismatch)?;
    let scope_value = CborValue::Array(vec![
        bytes(binding.scope().repository_id().as_bytes()),
        bytes(binding.scope().work_id().as_bytes()),
    ]);
    let graphs = active_objects
        .iter()
        .filter(|object| generation.roots().contains(&object.id()))
        .filter(|object| object.schema_id() == graph_schema)
        .filter_map(|object| {
            let CborValue::Array(fields) = object.value() else {
                return None;
            };
            let [CborValue::Text(domain), CborValue::Bytes(encoded)] = fields.as_slice() else {
                return None;
            };
            if domain != RepositoryStoreSchemaV1::StepGraph.domain() {
                return None;
            }
            let decoded = deterministic_cbor::decode(encoded).ok()?;
            let CborValue::Array(graph) = decoded else {
                return None;
            };
            let [
                CborValue::Unsigned(1),
                scope,
                contract_generation,
                contract_root,
                CborValue::Array(nodes),
                _edges,
            ] = graph.as_slice()
            else {
                return None;
            };
            (scope == &scope_value
                && exact_digest(contract_generation)
                    == Some(*binding.contract_generation_id().as_bytes())
                && exact_digest(contract_root) == Some(*generation.contract_root_id().as_bytes()))
            .then_some(nodes.clone())
        })
        .collect::<Vec<_>>();
    let [nodes] = graphs.as_slice() else {
        return Err(ExecutionStoreErrorV1::StepBindingNotCurrent);
    };
    let expected = step_binding_store_value(binding);
    if !nodes.iter().any(|node| {
        matches!(
            node,
            CborValue::Array(fields)
                if fields.as_slice() == [expected.clone(), CborValue::Bool(true)]
        )
    }) {
        return Err(ExecutionStoreErrorV1::StepBindingNotCurrent);
    }
    Ok(())
}

fn validate_current_step_effect_origin(
    generation: &StoreGenerationV1,
    active_objects: &[StoreObjectV1],
    origin: &EffectOriginV1,
    authority: &ExecutionAuthorityV1,
    accepted_h_time: u64,
) -> Result<(), ExecutionStoreErrorV1> {
    let Some(step_origin) = origin.step_authority() else {
        return Ok(());
    };
    let binding = step_origin.binding();
    if generation.domain().role() != StoreRoleV1::Repository
        || binding.scope().repository_id() != generation.domain().id()
    {
        return Err(ExecutionStoreErrorV1::StepBindingStoreMismatch);
    }
    require_current_step_graph(generation, active_objects, binding)?;
    if !validate_rooted_step_state(generation, active_objects, binding)? {
        return Err(ExecutionStoreErrorV1::StepBindingNotOpen);
    }
    let binding_commitment = hash(&step_binding_store_value(binding))?;
    let index = load_optional_step_execution_index(active_objects)?
        .ok_or(ExecutionStoreErrorV1::MissingStepExecutionCarrier)?;
    let entry = index
        .entries
        .iter()
        .find(|entry| entry.binding_commitment == binding_commitment)
        .ok_or(ExecutionStoreErrorV1::MissingStepExecutionCarrier)?;
    let carrier = load_step_execution_carrier(active_objects, entry)?;
    let attempt = carrier.tenure().attempt();
    let lease = carrier.tenure().lease();
    let term = carrier.tenure().current_term();
    if !attempt.is_live()
        || lease.state() != StepAttemptStateV1::Live
        || !term.is_live_at(accepted_h_time)
        || step_origin.attempt_id() != attempt.id()
        || step_origin.lease_id() != lease.id()
        || step_origin.lease_fence() != lease.fence()
        || step_origin.lease_term_id() != term.id()
        || step_origin.lease_term_ordinal() != term.ordinal()
        || authority.executor_principal_id() != attempt.executor()
    {
        return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
    }
    Ok(())
}

fn validate_rooted_step_state(
    generation: &StoreGenerationV1,
    active_objects: &[StoreObjectV1],
    binding: StepBindingV1,
) -> Result<bool, ExecutionStoreErrorV1> {
    rooted_step_state_is_open(rooted_step_state_object(
        generation,
        active_objects,
        binding,
    )?)
}

fn rooted_step_state_object<'object>(
    generation: &StoreGenerationV1,
    active_objects: &'object [StoreObjectV1],
    binding: StepBindingV1,
) -> Result<&'object StoreObjectV1, ExecutionStoreErrorV1> {
    let schema = RepositoryStoreSchemaV1::StepState
        .schema_id()
        .map_err(|_| ExecutionStoreErrorV1::PublicationBindingMismatch)?;
    let expected_binding = step_binding_store_value(binding);
    let matches = active_objects
        .iter()
        .filter(|object| generation.roots().contains(&object.id()))
        .filter(|object| object.schema_id() == schema)
        .filter(|object| {
            let CborValue::Array(fields) = object.value() else {
                return false;
            };
            let [CborValue::Text(domain), stored_binding, _lifecycle] = fields.as_slice() else {
                return false;
            };
            domain == RepositoryStoreSchemaV1::StepState.domain()
                && stored_binding == &expected_binding
        })
        .collect::<Vec<_>>();
    let [object] = matches.as_slice() else {
        return Err(ExecutionStoreErrorV1::StepBindingNotCurrent);
    };
    Ok(object)
}

fn rooted_step_state_is_open(object: &StoreObjectV1) -> Result<bool, ExecutionStoreErrorV1> {
    let CborValue::Array(fields) = object.value() else {
        return Err(ExecutionStoreErrorV1::StepBindingNotCurrent);
    };
    let [CborValue::Text(domain), _binding, lifecycle] = fields.as_slice() else {
        return Err(ExecutionStoreErrorV1::StepBindingNotCurrent);
    };
    if domain != RepositoryStoreSchemaV1::StepState.domain() {
        return Err(ExecutionStoreErrorV1::StepBindingNotCurrent);
    }
    Ok(matches!(
        lifecycle,
        CborValue::Array(fields)
            if matches!(fields.first(), Some(CborValue::Unsigned(1)))
    ))
}

fn decode_rooted_open_step_state(
    object: &StoreObjectV1,
    binding: StepBindingV1,
) -> Result<StepStateV1, ExecutionStoreErrorV1> {
    let CborValue::Array(fields) = object.value() else {
        return Err(ExecutionStoreErrorV1::StepBindingNotCurrent);
    };
    let [
        CborValue::Text(domain),
        stored_binding,
        CborValue::Array(lifecycle),
    ] = fields.as_slice()
    else {
        return Err(ExecutionStoreErrorV1::StepBindingNotCurrent);
    };
    if domain != RepositoryStoreSchemaV1::StepState.domain()
        || stored_binding != &step_binding_store_value(binding)
    {
        return Err(ExecutionStoreErrorV1::StepBindingNotCurrent);
    }
    let [CborValue::Unsigned(1), CborValue::Array(basis)] = lifecycle.as_slice() else {
        return Err(ExecutionStoreErrorV1::StepBindingNotOpen);
    };
    let basis = match basis.as_slice() {
        [CborValue::Unsigned(1)] => StepOpenBasisV1::Fresh,
        [
            CborValue::Unsigned(2),
            submission_id,
            submission_record_hash,
            rejection_receipt_hash,
        ] => StepOpenBasisV1::RejectedSubmission {
            submission_id: StepSubmissionIdV1::from_bytes(
                exact_digest(submission_id).ok_or(ExecutionStoreErrorV1::StepBindingNotCurrent)?,
            )
            .map_err(|_| ExecutionStoreErrorV1::StepBindingNotCurrent)?,
            submission_record_hash: exact_digest(submission_record_hash)
                .ok_or(ExecutionStoreErrorV1::StepBindingNotCurrent)?,
            rejection_receipt_hash: exact_digest(rejection_receipt_hash)
                .ok_or(ExecutionStoreErrorV1::StepBindingNotCurrent)?,
        },
        [
            CborValue::Unsigned(3),
            submission_id,
            submission_record_hash,
            recovery_receipt_hash,
        ] => StepOpenBasisV1::RecoveredSubmission {
            submission_id: StepSubmissionIdV1::from_bytes(
                exact_digest(submission_id).ok_or(ExecutionStoreErrorV1::StepBindingNotCurrent)?,
            )
            .map_err(|_| ExecutionStoreErrorV1::StepBindingNotCurrent)?,
            submission_record_hash: exact_digest(submission_record_hash)
                .ok_or(ExecutionStoreErrorV1::StepBindingNotCurrent)?,
            recovery_receipt_hash: exact_digest(recovery_receipt_hash)
                .ok_or(ExecutionStoreErrorV1::StepBindingNotCurrent)?,
        },
        _ => return Err(ExecutionStoreErrorV1::StepBindingNotCurrent),
    };
    Ok(StepStateV1::from_lifecycle(
        binding,
        StepLifecycleV1::Open { basis },
    ))
}

fn apply_authorized_step_mutation(
    snapshot: &StepExecutionSnapshotV1,
    current_generation: &StoreGenerationV1,
    authority_epoch: u64,
    accepted_h_time: u64,
    mutation: AuthorizedStepExecutionMutationV1,
    authorized: AuthorizedExecutionActionV1,
) -> Result<StepExecutionCarrierV1, ExecutionStoreErrorV1> {
    match mutation {
        AuthorizedStepExecutionMutationV1::Acquire {
            executor,
            fixed_envelope_commitment,
            run_limit,
            issued_at,
            expires_at,
            hard_deadline,
            takeover_safety,
        } => {
            validate_trusted_mutation_time(issued_at, expires_at, accepted_h_time)?;
            let next_fence = snapshot.next_fence()?;
            validate_takeover_safety(
                snapshot.carrier(),
                snapshot.binding,
                next_fence,
                accepted_h_time,
                takeover_safety.as_deref(),
            )?;
            Ok(StepExecutionCarrierV1::acquire(
                StepExecutionAcquisitionV1 {
                    binding: snapshot.binding,
                    next_fence,
                    executor,
                    store_generation_id: current_generation.id(),
                    authority_epoch,
                    fixed_envelope_commitment,
                    run_limit,
                    issued_at,
                    expires_at,
                    hard_deadline,
                    authority: authorized,
                },
            )?)
        }
        AuthorizedStepExecutionMutationV1::Renew {
            expected_term_id,
            issued_at,
            expires_at,
            lease_mutation,
        } => {
            let mut carrier = snapshot
                .carrier
                .clone()
                .ok_or(ExecutionStoreErrorV1::MissingStepExecutionCarrier)?;
            validate_trusted_mutation_time(issued_at, expires_at, accepted_h_time)?;
            if accepted_h_time >= carrier.tenure().current_term().expires_at()
                || lease_mutation
                    .as_deref()
                    .is_some_and(|mutation| mutation.as_of() != accepted_h_time)
            {
                return Err(ExecutionStoreErrorV1::UntrustedMutationTime);
            }
            carrier.renew(expected_term_id, issued_at, expires_at, authorized)?;
            if let Some(mutation) = lease_mutation {
                let current_term_id = carrier.tenure().current_term().id();
                apply_step_lease_mutation(&mut carrier, current_term_id, *mutation)?;
            }
            Ok(carrier)
        }
        AuthorizedStepExecutionMutationV1::Abandon {
            terminal,
            expected_term_id,
            as_of,
            expected_run_set_revision,
        } => {
            let mut carrier = snapshot
                .carrier
                .clone()
                .ok_or(ExecutionStoreErrorV1::MissingStepExecutionCarrier)?;
            let expired = accepted_h_time >= carrier.tenure().current_term().expires_at();
            if as_of != accepted_h_time || (terminal == StepAttemptTerminalV1::TimedOut) != expired
            {
                return Err(ExecutionStoreErrorV1::UntrustedMutationTime);
            }
            carrier.abandon(
                terminal,
                expected_term_id,
                as_of,
                expected_run_set_revision,
                authorized,
            )?;
            Ok(carrier)
        }
    }
}

fn validate_takeover_safety(
    predecessor: Option<&StepExecutionCarrierV1>,
    binding: StepBindingV1,
    successor_fence: u64,
    trusted_time_lower: u64,
    takeover_safety: Option<&TakeoverSafetyV1>,
) -> Result<(), ExecutionStoreErrorV1> {
    match predecessor {
        Some(carrier)
            if carrier.tenure().attempt().is_live()
                && trusted_time_lower < carrier.tenure().current_term().expires_at() =>
        {
            Err(ExecutionStoreErrorV1::LiveStepExecutionAlreadyExists)
        }
        Some(carrier) => {
            takeover_safety
                .ok_or(ExecutionStoreErrorV1::TakeoverSafetyRequired)?
                .validate(carrier, binding, successor_fence, trusted_time_lower)?;
            Ok(())
        }
        None if takeover_safety.is_some() => Err(ExecutionStoreErrorV1::UnexpectedTakeoverSafety),
        None => Ok(()),
    }
}

fn validate_trusted_mutation_time(
    as_of: u64,
    successor_expires_at: u64,
    accepted_h_time: u64,
) -> Result<(), ExecutionStoreErrorV1> {
    if as_of != accepted_h_time || successor_expires_at <= accepted_h_time {
        return Err(ExecutionStoreErrorV1::UntrustedMutationTime);
    }
    Ok(())
}

fn apply_step_lease_mutation(
    carrier: &mut StepExecutionCarrierV1,
    current_term_id: LeaseTermIdV1,
    mutation: StepLeaseMutationV1,
) -> Result<(), ExecutionStoreErrorV1> {
    match mutation {
        StepLeaseMutationV1::ReserveRun {
            expected_run_set_revision,
            as_of,
            reservation,
        } => {
            carrier.reserve_run(
                expected_run_set_revision,
                current_term_id,
                as_of,
                reservation,
            )?;
        }
        StepLeaseMutationV1::TransitionRun {
            run_id,
            expected_run_set_revision,
            as_of,
            next,
        } => carrier.transition_run(
            run_id,
            expected_run_set_revision,
            current_term_id,
            as_of,
            next,
        )?,
        StepLeaseMutationV1::MarkDefinitelyNotStarted {
            expected_run_set_revision,
            as_of,
            receipt,
        } => carrier.mark_run_definitely_not_started(
            expected_run_set_revision,
            current_term_id,
            as_of,
            receipt,
        )?,
        StepLeaseMutationV1::AppendRunSegment {
            run_id,
            expected_run_set_revision,
            as_of,
            process_or_job_identity,
            segment_commitment,
        } => carrier.append_run_segment(RunSegmentAppendV1 {
            run_id,
            expected_run_set_revision,
            expected_term_id: current_term_id,
            as_of,
            process_or_job_identity,
            segment_commitment,
        })?,
        StepLeaseMutationV1::RetryRun {
            predecessor_run_id,
            expected_run_set_revision,
            as_of,
            deadline,
        } => {
            carrier.retry_run(
                predecessor_run_id,
                expected_run_set_revision,
                current_term_id,
                as_of,
                deadline,
            )?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn build_step_execution_authorized_publication(
    domain: crate::domain::persistence::StoreDomainV1,
    current_head: &StoreHeadV1,
    current_generation: &StoreGenerationV1,
    active_objects: &[StoreObjectV1],
    snapshot: &StepExecutionSnapshotV1,
    request: &CanonicalExecutionActionRequestV1,
    admission: &AdmittedRepositoryActionV1,
    artifacts: &RepositoryAuthorityArtifactsV1,
    request_object: StoreObjectV1,
    carrier_object: StoreObjectV1,
    meaning_digest: [u8; 32],
) -> Result<AtomicGenerationPublicationV1, ExecutionStoreErrorV1> {
    let current_index = load_optional_step_execution_index(active_objects)?;
    if current_index.as_ref().map(|index| index.object.id())
        != snapshot.state_binding.step_index_object_id
    {
        return Err(ExecutionStoreErrorV1::StaleExpectedStoreState);
    }
    let mut entries = current_index
        .as_ref()
        .map(|index| index.entries.clone())
        .unwrap_or_default();
    let next_carrier = StepExecutionCarrierV1::from_canonical_value(carrier_object.value())?;
    if next_carrier.tenure().attempt().binding() != snapshot.binding {
        return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
    }
    let next_fence = next_carrier.tenure().attempt().fence();
    if let Some(entry) = entries
        .iter_mut()
        .find(|entry| entry.binding_commitment == snapshot.state_binding.step_binding_commitment)
    {
        entry.carrier_object_id = carrier_object.id();
        let expected_fence = if request.action() == ExecutionActionV1::AcquireStepExecution {
            snapshot
                .state_binding
                .fence_high_water
                .checked_add(1)
                .ok_or(ExecutionStoreErrorV1::FenceOverflow)?
        } else {
            snapshot.state_binding.fence_high_water
        };
        if next_fence != expected_fence {
            return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
        }
        entry.fence_high_water = next_fence;
    } else {
        if snapshot.state_binding.carrier_object_id.is_some()
            || request.action() != ExecutionActionV1::AcquireStepExecution
            || next_fence != 1
        {
            return Err(ExecutionStoreErrorV1::StaleExpectedStoreState);
        }
        entries.push(StepExecutionIndexEntryV1 {
            binding_commitment: snapshot.state_binding.step_binding_commitment,
            carrier_object_id: carrier_object.id(),
            fence_high_water: next_fence,
        });
    }
    let next_index = build_step_execution_index_object(&entries)?;
    let mut roots = current_generation.roots().to_vec();
    if let Some(index) = &current_index {
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
            .ok_or(ExecutionStoreErrorV1::GenerationOverflow)?,
        Some(current_generation.id()),
        current_generation.contract_root_id(),
        current_generation.compatibility().clone(),
        roots,
    )?;
    let idempotency = StoreIdempotencyV1::new(
        "maestro.vnext.step-execution-authorized-publication.v1",
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
        carrier_object,
        next_index,
    ]);
    objects.extend(artifacts.leaf_authority_objects().iter().cloned());
    objects.sort_by_key(StoreObjectV1::id);
    objects.dedup_by_key(|object| object.id());
    Ok(AtomicGenerationPublicationV1::new_from_object_superset(
        generation,
        Some(current_head.id()),
        objects,
        idempotency,
    )?)
}

#[expect(
    clippy::too_many_arguments,
    reason = "the atomic Step-owned Submission join names each independently owned Store carrier explicitly"
)]
fn build_step_submission_authorized_publication(
    domain: crate::domain::persistence::StoreDomainV1,
    current_head: &StoreHeadV1,
    current_generation: &StoreGenerationV1,
    active_objects: &[StoreObjectV1],
    snapshot: &StepExecutionSnapshotV1,
    current_step_state: &StoreObjectV1,
    request: &CanonicalStepSubmissionActionRequestV1,
    admission: &AdmittedRepositoryActionV1,
    artifacts: &RepositoryAuthorityArtifactsV1,
    request_object: StoreObjectV1,
    carrier_object: StoreObjectV1,
    step_state_object: StoreObjectV1,
    submission_object: StoreObjectV1,
    claim_set_object: StoreObjectV1,
    claim_objects: Vec<StoreObjectV1>,
    meaning_digest: [u8; 32],
) -> Result<AtomicGenerationPublicationV1, ExecutionStoreErrorV1> {
    let current_index = load_optional_step_execution_index(active_objects)?
        .ok_or(ExecutionStoreErrorV1::MissingStepExecutionIndex)?;
    if Some(current_index.object.id()) != snapshot.state_binding.step_index_object_id {
        return Err(ExecutionStoreErrorV1::StaleExpectedStoreState);
    }
    let mut entries = current_index.entries.clone();
    let next_carrier = StepExecutionCarrierV1::from_canonical_value(carrier_object.value())?;
    let entry = entries
        .iter_mut()
        .find(|entry| entry.binding_commitment == snapshot.state_binding.step_binding_commitment)
        .ok_or(ExecutionStoreErrorV1::MissingStepExecutionCarrier)?;
    if snapshot.state_binding.carrier_object_id != Some(entry.carrier_object_id)
        || next_carrier.tenure().attempt().binding() != snapshot.binding
        || next_carrier.tenure().attempt().fence() != entry.fence_high_water
        || entry.fence_high_water != snapshot.state_binding.fence_high_water
    {
        return Err(ExecutionStoreErrorV1::StaleExpectedStoreState);
    }
    entry.carrier_object_id = carrier_object.id();
    let next_index = build_step_execution_index_object(&entries)?;
    let mut roots = current_generation.roots().to_vec();
    replace_required_root(&mut roots, current_index.object.id(), next_index.id())?;
    replace_required_root(&mut roots, current_step_state.id(), step_state_object.id())?;
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
            .ok_or(ExecutionStoreErrorV1::GenerationOverflow)?,
        Some(current_generation.id()),
        current_generation.contract_root_id(),
        current_generation.compatibility().clone(),
        roots,
    )?;
    let idempotency = StoreIdempotencyV1::new(
        STEP_SUBMISSION_IDEMPOTENCY_NAMESPACE_V1,
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
        carrier_object,
        step_state_object,
        submission_object,
        claim_set_object,
        next_index,
    ]);
    objects.extend(claim_objects);
    objects.extend(artifacts.leaf_authority_objects().iter().cloned());
    objects.sort_by_key(StoreObjectV1::id);
    objects.dedup_by_key(|object| object.id());
    Ok(AtomicGenerationPublicationV1::new_from_object_superset(
        generation,
        Some(current_head.id()),
        objects,
        idempotency,
    )?)
}

fn execution_action_request_object(
    request: &CanonicalExecutionActionRequestV1,
) -> Result<StoreObjectV1, ExecutionStoreErrorV1> {
    Ok(StoreObjectV1::new(
        execution_schema_id("maestro.vnext.execution-action-request-schema.v1")?,
        request.canonical_value()?,
        vec![],
    )?)
}

fn active_store_effect_fences(
    generation: &StoreGenerationV1,
    request: &CanonicalExecutionActionRequestV1,
    draft: &ActiveStoreEffectOriginationDraftV1,
    admission: &AdmittedRepositoryActionV1,
    action_result_id: [u8; 32],
) -> Result<
    (
        EffectIntentHomeV1,
        EffectIntentOriginationFenceV1,
        EffectIntentUseFenceV1,
    ),
    ExecutionStoreErrorV1,
> {
    let stable_home = HomeTokenV1::new(*generation.domain().id().as_bytes());
    if draft.stable_domain_id != stable_home
        || draft.domain_kind != effect_domain_kind_for_store_role(generation.domain().role())
    {
        return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
    }
    let generation_token = HomeTokenV1::new(*generation.id().as_bytes());
    let epoch = HomeTokenV1::new(hash(&CborValue::Unsigned(admission.authority_epoch()))?);
    let material_token = HomeTokenV1::new(*draft.material_inputs.as_bytes());
    let authority_basis = HomeTokenV1::new(*admission.basis_object().id().as_bytes());
    let credentials = HomeTokenV1::new(*draft.credential_requirements.as_bytes());
    let dispatch_fence = HomeTokenV1::new(hash(&CborValue::Array(vec![
        CborValue::text("maestro.vnext.effect-dispatch-reservation-fence.v1")?,
        bytes(request.request_id().as_bytes()),
        bytes(&draft.dispatch.provider_key_commitment),
        bytes(draft.semantic_namespace.as_bytes()),
    ]))?);
    let home = EffectIntentHomeV1::ActiveStore(ActiveStoreHomeV1 {
        domain_kind: draft.domain_kind,
        stable_domain_id: stable_home,
        realm: draft.realm,
        semantic_namespace: draft.semantic_namespace,
        home_qualified_semantic_uniqueness_namespace: draft.uniqueness_namespace,
    });
    let origination_fence =
        EffectIntentOriginationFenceV1::ActiveStore(ActiveStoreOriginationFenceV1 {
            store: stable_home,
            generation: generation_token,
            epoch,
            namespace: draft.semantic_namespace,
            material_token,
            action_request: HomeTokenV1::new(*request.request_id().as_bytes()),
            action_authority_basis: authority_basis,
            receipt: HomeTokenV1::new(*admission.authorization_receipt().id().as_bytes()),
            result: HomeTokenV1::new(action_result_id),
            effect_origin: HomeTokenV1::new(draft.origin.commitment()?),
            current_authority_commitment: authority_basis,
            credential_commitment: credentials,
            dispatch_reservation_or_fence: dispatch_fence,
        });
    let use_fence = EffectIntentUseFenceV1::ActiveStore(ActiveStoreUseFenceV1 {
        same_stable_home: stable_home,
        generation: generation_token,
        epoch,
        namespace: draft.semantic_namespace,
        material_token,
        authority: authority_basis,
        credentials,
        attempt_fence: dispatch_fence,
        idempotency_binding: HomeTokenV1::new(*request.idempotency_key_id().as_bytes()),
        provider_contract_guards: HomeTokenV1::new(
            draft.dispatch.provider_operation_contract_commitment,
        ),
    });
    Ok((home, origination_fence, use_fence))
}

fn effect_semantic_uniqueness_commitment(
    intent: &EffectIntentV1,
) -> Result<[u8; 32], ExecutionStoreErrorV1> {
    hash(&effect_intent_subject_value(intent)?).map_err(Into::into)
}

fn effect_domain_kind_for_store_role(role: StoreRoleV1) -> EffectIntentDomainKindV1 {
    match role {
        StoreRoleV1::Repository => EffectIntentDomainKindV1::RepositoryDomain,
        StoreRoleV1::Installation => EffectIntentDomainKindV1::InstallationDomain,
    }
}

fn active_store_redispatch_use_fence(
    generation: &StoreGenerationV1,
    request: &CanonicalExecutionActionRequestV1,
    snapshot: &ActiveStoreEffectSnapshotV1,
    draft: &ActiveStoreEffectRedispatchDraftV1,
    admission: &AdmittedRepositoryActionV1,
) -> Result<EffectIntentUseFenceV1, ExecutionStoreErrorV1> {
    let EffectIntentHomeV1::ActiveStore(home) = snapshot.intent.home() else {
        return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
    };
    if home.stable_domain_id.as_bytes() != generation.domain().id().as_bytes()
        || home.domain_kind != effect_domain_kind_for_store_role(generation.domain().role())
    {
        return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
    }
    let attempt_fence = hash(&CborValue::Array(vec![
        CborValue::text("maestro.vnext.effect-dispatch-reservation-fence.v1")?,
        bytes(request.request_id().as_bytes()),
        bytes(&draft.dispatch.provider_key_commitment),
        bytes(home.semantic_namespace.as_bytes()),
    ]))?;
    Ok(EffectIntentUseFenceV1::ActiveStore(ActiveStoreUseFenceV1 {
        same_stable_home: home.stable_domain_id,
        generation: HomeTokenV1::new(*generation.id().as_bytes()),
        epoch: HomeTokenV1::new(hash(&CborValue::Unsigned(admission.authority_epoch()))?),
        namespace: home.semantic_namespace,
        material_token: HomeTokenV1::new(*snapshot.intent.material_inputs().as_bytes()),
        authority: HomeTokenV1::new(*admission.basis_object().id().as_bytes()),
        credentials: HomeTokenV1::new(*snapshot.intent.credential_requirements().as_bytes()),
        attempt_fence: HomeTokenV1::new(attempt_fence),
        idempotency_binding: HomeTokenV1::new(*request.idempotency_key_id().as_bytes()),
        provider_contract_guards: HomeTokenV1::new(
            draft.dispatch.provider_operation_contract_commitment,
        ),
    }))
}

fn active_store_reconciliation_use_fence(
    generation: &StoreGenerationV1,
    request: &CanonicalExecutionActionRequestV1,
    snapshot: &ActiveStoreEffectSnapshotV1,
    draft: &ActiveStoreEffectReconciliationBeginDraftV1,
    admission: &AdmittedRepositoryActionV1,
) -> Result<EffectIntentUseFenceV1, ExecutionStoreErrorV1> {
    let EffectIntentHomeV1::ActiveStore(home) = snapshot.intent.home() else {
        return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
    };
    if home.stable_domain_id.as_bytes() != generation.domain().id().as_bytes() {
        return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
    }
    let attempt_fence = hash(&CborValue::Array(vec![
        CborValue::text("maestro.vnext.effect-reconciliation-attempt-fence.v1")?,
        bytes(snapshot.intent.id().as_bytes()),
        bytes(request.request_id().as_bytes()),
    ]))?;
    Ok(EffectIntentUseFenceV1::ActiveStore(ActiveStoreUseFenceV1 {
        same_stable_home: home.stable_domain_id,
        generation: HomeTokenV1::new(*generation.id().as_bytes()),
        epoch: HomeTokenV1::new(hash(&CborValue::Unsigned(admission.authority_epoch()))?),
        namespace: home.semantic_namespace,
        material_token: HomeTokenV1::new(*snapshot.intent.material_inputs().as_bytes()),
        authority: HomeTokenV1::new(*admission.basis_object().id().as_bytes()),
        credentials: HomeTokenV1::new(*snapshot.intent.credential_requirements().as_bytes()),
        attempt_fence: HomeTokenV1::new(attempt_fence),
        idempotency_binding: HomeTokenV1::new(*request.idempotency_key_id().as_bytes()),
        provider_contract_guards: HomeTokenV1::new(draft.read_plan.commitment()?),
    }))
}

#[allow(clippy::too_many_arguments)]
fn build_effect_origination_publication(
    domain: crate::domain::persistence::StoreDomainV1,
    current_head: &StoreHeadV1,
    current_generation: &StoreGenerationV1,
    active_objects: &[StoreObjectV1],
    request: &CanonicalExecutionActionRequestV1,
    authority: &ExecutionAuthorityV1,
    admission: &AdmittedRepositoryActionV1,
    request_object: StoreObjectV1,
    intent: &EffectIntentV1,
    initial_revision: &EffectIntentControlRevisionV1,
    writer_term: EffectIntentControlWriterTermV1,
    prepared: &PreparedEffectDispatchV1,
    reserved_revision: &EffectIntentControlRevisionV1,
    reserved_head: &EffectIntentControlHeadV1,
    meaning_digest: [u8; 32],
) -> Result<AtomicGenerationPublicationV1, ExecutionStoreErrorV1> {
    if initial_revision.intent() != intent.id()
        || reserved_revision.intent() != intent.id()
        || reserved_head.intent() != intent.id()
        || reserved_head.revision() != reserved_revision.id()
        || reserved_head.writer_term() != writer_term.id()
        || prepared.dispatch().attempt().effect_intent_id() != intent.id()
    {
        return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
    }
    let mut intent_references = vec![request_object.id(), admission.basis_object().id()];
    intent_references.sort_unstable();
    let intent_object = StoreObjectV1::new(
        execution_schema_id("maestro.vnext.effect-intent-schema.v1")?,
        intent.persistence_value()?,
        intent_references,
    )?;
    let initial_revision_object = StoreObjectV1::new(
        execution_schema_id("maestro.vnext.effect-intent-control-revision-schema.v1")?,
        initial_revision.canonical_value()?,
        vec![intent_object.id()],
    )?;
    let writer_term_object = StoreObjectV1::new(
        execution_schema_id("maestro.vnext.effect-intent-control-writer-term-schema.v1")?,
        writer_term.canonical_value(),
        vec![intent_object.id()],
    )?;
    let mut dispatch_references = vec![
        intent_object.id(),
        initial_revision_object.id(),
        request_object.id(),
        admission.basis_object().id(),
    ];
    dispatch_references.sort_unstable();
    let dispatch_carrier_object = StoreObjectV1::new(
        execution_schema_id("maestro.vnext.effect-dispatch-reservation-carrier-schema.v1")?,
        effect_dispatch_authorized_carrier_value(prepared.persistence_carrier_value()?, authority)?,
        dispatch_references,
    )?;
    let mut revision_references = vec![initial_revision_object.id(), dispatch_carrier_object.id()];
    revision_references.sort_unstable();
    let reserved_revision_object = StoreObjectV1::new(
        execution_schema_id("maestro.vnext.effect-intent-control-revision-schema.v1")?,
        reserved_revision.canonical_value()?,
        revision_references,
    )?;
    let mut head_references = vec![
        intent_object.id(),
        reserved_revision_object.id(),
        writer_term_object.id(),
        dispatch_carrier_object.id(),
    ];
    head_references.sort_unstable();
    let head_object = StoreObjectV1::new(
        execution_schema_id("maestro.vnext.effect-intent-control-head-schema.v1")?,
        reserved_head.canonical_value(),
        head_references,
    )?;
    let current_index = load_optional_control_index(active_objects)?;
    let mut index_entries = current_index
        .as_ref()
        .map(|index| index.entries.clone())
        .unwrap_or_default();
    let semantic_uniqueness_commitment = effect_semantic_uniqueness_commitment(intent)?;
    if index_entries.iter().any(|entry| {
        entry.intent == intent.id()
            || entry.semantic_uniqueness_commitment == semantic_uniqueness_commitment
    }) {
        return Err(ExecutionStoreErrorV1::LiveSemanticEffectAlreadyExists);
    }
    index_entries.push(ControlIndexEntryV1 {
        intent: intent.id(),
        semantic_uniqueness_commitment,
        control_head: reserved_head.id(),
        control_head_object_id: head_object.id(),
    });
    let index_object = build_control_index_object(&index_entries)?;
    let produced_objects = vec![
        intent_object.clone(),
        initial_revision_object.clone(),
        writer_term_object.clone(),
        dispatch_carrier_object.clone(),
        reserved_revision_object.clone(),
        head_object.clone(),
    ];
    let artifacts = admission.issue_committed_artifacts(&request_object, &produced_objects)?;
    let dispatch_attempt = prepared.dispatch().attempt().id();
    let mut result_references = vec![
        artifacts.result_object().id(),
        intent_object.id(),
        writer_term_object.id(),
        dispatch_carrier_object.id(),
        reserved_revision_object.id(),
        head_object.id(),
    ];
    result_references.sort_unstable();
    let result_object = StoreObjectV1::new(
        execution_schema_id("maestro.vnext.effect-origination-result-schema.v1")?,
        CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-origination-result.v1")?,
            bytes(request.request_id().as_bytes()),
            bytes(intent.id().as_bytes()),
            bytes(reserved_head.id().as_bytes()),
            bytes(reserved_revision.id().as_bytes()),
            bytes(writer_term.id().as_bytes()),
            bytes(dispatch_attempt.as_bytes()),
            bytes(&meaning_digest),
        ]),
        result_references,
    )?;
    let mut roots = current_generation.roots().to_vec();
    if let Some(index) = &current_index {
        replace_required_root(&mut roots, index.object.id(), index_object.id())?;
    } else {
        roots.push(index_object.id());
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
    roots.push(result_object.id());
    roots.sort_unstable();
    roots.dedup();
    let generation = StoreGenerationV1::new(
        domain,
        current_generation
            .ordinal()
            .checked_add(1)
            .ok_or(ExecutionStoreErrorV1::GenerationOverflow)?,
        Some(current_generation.id()),
        current_generation.contract_root_id(),
        current_generation.compatibility().clone(),
        roots,
    )?;
    let idempotency = StoreIdempotencyV1::new(
        EFFECT_ORIGINATION_IDEMPOTENCY_NAMESPACE_V1,
        *request.idempotency_key_id().as_bytes(),
        meaning_digest,
        result_object.id(),
    )?;
    let mut objects = active_objects.to_vec();
    objects.extend(produced_objects);
    objects.extend([
        index_object,
        request_object,
        admission.basis_object().clone(),
        admission.successor_snapshot().clone(),
        admission.successor_capacity_root().clone(),
        admission.capacity_debit().clone(),
        artifacts.receipt_object().clone(),
        artifacts.result_object().clone(),
        result_object,
    ]);
    objects.extend(artifacts.leaf_authority_objects().iter().cloned());
    objects.sort_by_key(StoreObjectV1::id);
    objects.dedup_by_key(|object| object.id());
    Ok(AtomicGenerationPublicationV1::new_from_object_superset(
        generation,
        Some(current_head.id()),
        objects,
        idempotency,
    )?)
}

fn decode_effect_origination_outcome(
    publication: StorePublicationOutcomeV1,
) -> Result<ActiveStoreEffectOriginationOutcomeV1, ExecutionStoreErrorV1> {
    let replayed = matches!(&publication, StorePublicationOutcomeV1::Replayed { .. });
    let result = publication.result();
    if result.schema_id()
        != execution_schema_id("maestro.vnext.effect-origination-result-schema.v1")?
    {
        return Err(ExecutionStoreErrorV1::InvalidEffectOriginationResult);
    }
    let CborValue::Array(fields) = result.value() else {
        return Err(ExecutionStoreErrorV1::InvalidEffectOriginationResult);
    };
    let [
        CborValue::Text(domain),
        _request,
        intent,
        control_head,
        control_revision,
        writer_term,
        dispatch_attempt,
        _meaning,
    ] = fields.as_slice()
    else {
        return Err(ExecutionStoreErrorV1::InvalidEffectOriginationResult);
    };
    if domain != "maestro.vnext.effect-origination-result.v1" {
        return Err(ExecutionStoreErrorV1::InvalidEffectOriginationResult);
    }
    Ok(ActiveStoreEffectOriginationOutcomeV1 {
        store_head: publication.head().clone(),
        intent: EffectIntentIdV1::from_bytes(
            exact_digest(intent).ok_or(ExecutionStoreErrorV1::InvalidEffectOriginationResult)?,
        )?,
        control_head: EffectIntentControlTokenV1::new(HomeTokenV1::new(
            exact_digest(control_head)
                .ok_or(ExecutionStoreErrorV1::InvalidEffectOriginationResult)?,
        )),
        control_revision: EffectIntentControlTokenV1::new(HomeTokenV1::new(
            exact_digest(control_revision)
                .ok_or(ExecutionStoreErrorV1::InvalidEffectOriginationResult)?,
        )),
        writer_term: EffectIntentControlTokenV1::new(HomeTokenV1::new(
            exact_digest(writer_term)
                .ok_or(ExecutionStoreErrorV1::InvalidEffectOriginationResult)?,
        )),
        dispatch_attempt: super::runtime::DispatchAttemptIdV1::from_bytes(
            exact_digest(dispatch_attempt)
                .ok_or(ExecutionStoreErrorV1::InvalidEffectOriginationResult)?,
        )?,
        replayed,
    })
}

#[allow(clippy::too_many_arguments)]
fn build_effect_seal_publication(
    domain: crate::domain::persistence::StoreDomainV1,
    current_head: &StoreHeadV1,
    current_generation: &StoreGenerationV1,
    active_objects: &[StoreObjectV1],
    request: &CanonicalExecutionActionRequestV1,
    authority: &ExecutionAuthorityV1,
    admission: &AdmittedRepositoryActionV1,
    request_object: StoreObjectV1,
    snapshot: &ActiveStoreEffectSnapshotV1,
    candidate: &EffectDispatchSealCandidateV1,
    sealed_revision: &EffectIntentControlRevisionV1,
    sealed_head: &EffectIntentControlHeadV1,
    meaning_digest: [u8; 32],
    idempotency_namespace: &'static str,
) -> Result<AtomicGenerationPublicationV1, ExecutionStoreErrorV1> {
    if candidate.dispatch().attempt().id() != snapshot.dispatch.attempt().id()
        || sealed_revision.intent() != snapshot.intent.id()
        || sealed_head.intent() != snapshot.intent.id()
        || sealed_head.revision() != sealed_revision.id()
        || sealed_head.writer_term() != snapshot.writer_term.id()
    {
        return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
    }
    let current_index = load_control_index(active_objects)?;
    if current_index.object.id()
        != snapshot
            .state_binding
            .control_index_object_id()
            .ok_or(ExecutionStoreErrorV1::PublicationBindingMismatch)?
    {
        return Err(ExecutionStoreErrorV1::StaleExpectedStoreState);
    }
    let current_entry = current_index
        .entries
        .iter()
        .find(|entry| entry.intent == snapshot.intent.id())
        .ok_or(ExecutionStoreErrorV1::MissingIntentControlSelector)?;
    let current_head_object = active_objects
        .iter()
        .find(|object| object.id() == current_entry.control_head_object_id)
        .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?;
    let intent_object = exact_referenced_schema_object(
        current_head_object,
        active_objects,
        &[execution_schema_id(
            "maestro.vnext.effect-intent-schema.v1",
        )?],
    )?;
    let current_revision_object = exact_referenced_schema_object(
        current_head_object,
        active_objects,
        &[execution_schema_id(
            "maestro.vnext.effect-intent-control-revision-schema.v1",
        )?],
    )?;
    let writer_object = exact_referenced_schema_object(
        current_head_object,
        active_objects,
        &[execution_schema_id(
            "maestro.vnext.effect-intent-control-writer-term-schema.v1",
        )?],
    )?;
    let prior_dispatch_object = exact_referenced_schema_object(
        current_head_object,
        active_objects,
        &[execution_schema_id(
            "maestro.vnext.effect-dispatch-reservation-carrier-schema.v1",
        )?],
    )?;
    let mut carrier_references = vec![
        prior_dispatch_object.id(),
        current_head_object.id(),
        request_object.id(),
        admission.basis_object().id(),
    ];
    carrier_references.sort_unstable();
    let carrier_object = StoreObjectV1::new(
        execution_schema_id("maestro.vnext.effect-dispatch-seal-carrier-schema.v1")?,
        effect_dispatch_authorized_carrier_value(
            candidate.persistence_carrier_value()?,
            authority,
        )?,
        carrier_references,
    )?;
    let mut revision_references = vec![current_revision_object.id(), carrier_object.id()];
    revision_references.sort_unstable();
    let revision_object = StoreObjectV1::new(
        execution_schema_id("maestro.vnext.effect-intent-control-revision-schema.v1")?,
        sealed_revision.canonical_value()?,
        revision_references,
    )?;
    let mut head_references = vec![
        intent_object.id(),
        revision_object.id(),
        writer_object.id(),
        carrier_object.id(),
    ];
    head_references.sort_unstable();
    let head_object = StoreObjectV1::new(
        execution_schema_id("maestro.vnext.effect-intent-control-head-schema.v1")?,
        sealed_head.canonical_value(),
        head_references,
    )?;
    let mut next_entries = current_index.entries.clone();
    let selected = next_entries
        .iter_mut()
        .find(|entry| entry.intent == snapshot.intent.id())
        .ok_or(ExecutionStoreErrorV1::MissingIntentControlSelector)?;
    selected.control_head = sealed_head.id();
    selected.control_head_object_id = head_object.id();
    let index_object = build_control_index_object(&next_entries)?;
    let produced_objects = vec![
        carrier_object.clone(),
        revision_object.clone(),
        head_object.clone(),
    ];
    let artifacts = admission.issue_committed_artifacts(&request_object, &produced_objects)?;
    let mut result_references = vec![
        artifacts.result_object().id(),
        carrier_object.id(),
        revision_object.id(),
        head_object.id(),
    ];
    result_references.sort_unstable();
    let result_object = StoreObjectV1::new(
        execution_schema_id("maestro.vnext.effect-dispatch-seal-result-schema.v1")?,
        CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-dispatch-seal-result.v1")?,
            bytes(request.request_id().as_bytes()),
            bytes(snapshot.intent.id().as_bytes()),
            bytes(sealed_head.id().as_bytes()),
            bytes(sealed_revision.id().as_bytes()),
            bytes(candidate.dispatch().attempt().id().as_bytes()),
            bytes(&meaning_digest),
        ]),
        result_references,
    )?;
    let mut roots = current_generation.roots().to_vec();
    replace_required_root(&mut roots, current_index.object.id(), index_object.id())?;
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
    roots.push(result_object.id());
    roots.sort_unstable();
    roots.dedup();
    let generation = StoreGenerationV1::new(
        domain,
        current_generation
            .ordinal()
            .checked_add(1)
            .ok_or(ExecutionStoreErrorV1::GenerationOverflow)?,
        Some(current_generation.id()),
        current_generation.contract_root_id(),
        current_generation.compatibility().clone(),
        roots,
    )?;
    let idempotency = StoreIdempotencyV1::new(
        idempotency_namespace,
        *request.idempotency_key_id().as_bytes(),
        meaning_digest,
        result_object.id(),
    )?;
    let mut objects = active_objects.to_vec();
    objects.extend(produced_objects);
    objects.extend([
        index_object,
        request_object,
        admission.basis_object().clone(),
        admission.successor_snapshot().clone(),
        admission.successor_capacity_root().clone(),
        admission.capacity_debit().clone(),
        artifacts.receipt_object().clone(),
        artifacts.result_object().clone(),
        result_object,
    ]);
    objects.extend(artifacts.leaf_authority_objects().iter().cloned());
    objects.sort_by_key(StoreObjectV1::id);
    objects.dedup_by_key(|object| object.id());
    Ok(AtomicGenerationPublicationV1::new_from_object_superset(
        generation,
        Some(current_head.id()),
        objects,
        idempotency,
    )?)
}

fn decode_effect_seal_outcome(
    publication: StorePublicationOutcomeV1,
    expected_snapshot: &ActiveStoreEffectSnapshotV1,
) -> Result<ActiveStoreEffectSealOutcomeV1, ExecutionStoreErrorV1> {
    let replayed = matches!(&publication, StorePublicationOutcomeV1::Replayed { .. });
    let result = publication.result();
    if result.schema_id()
        != execution_schema_id("maestro.vnext.effect-dispatch-seal-result-schema.v1")?
    {
        return Err(ExecutionStoreErrorV1::InvalidEffectSealResult);
    }
    let CborValue::Array(fields) = result.value() else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSealResult);
    };
    let [
        CborValue::Text(domain),
        _request,
        intent,
        control_head,
        control_revision,
        attempt,
        _meaning,
    ] = fields.as_slice()
    else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSealResult);
    };
    if domain != "maestro.vnext.effect-dispatch-seal-result.v1" {
        return Err(ExecutionStoreErrorV1::InvalidEffectSealResult);
    }
    let intent = EffectIntentIdV1::from_bytes(
        exact_digest(intent).ok_or(ExecutionStoreErrorV1::InvalidEffectSealResult)?,
    )?;
    let control_head = EffectIntentControlTokenV1::new(HomeTokenV1::new(
        exact_digest(control_head).ok_or(ExecutionStoreErrorV1::InvalidEffectSealResult)?,
    ));
    let control_revision = EffectIntentControlTokenV1::new(HomeTokenV1::new(
        exact_digest(control_revision).ok_or(ExecutionStoreErrorV1::InvalidEffectSealResult)?,
    ));
    let dispatch_attempt = super::runtime::DispatchAttemptIdV1::from_bytes(
        exact_digest(attempt).ok_or(ExecutionStoreErrorV1::InvalidEffectSealResult)?,
    )?;
    if intent != expected_snapshot.intent.id()
        || dispatch_attempt != expected_snapshot.dispatch.attempt().id()
    {
        return Err(ExecutionStoreErrorV1::InvalidEffectSealResult);
    }
    let provider_release = (!replayed).then_some(ProviderApplicationReleaseV1 {
        intent,
        dispatch_attempt,
        sealed_control_head: control_head,
        operation: sealed_provider_operation_from_snapshot(expected_snapshot)?,
    });
    Ok(ActiveStoreEffectSealOutcomeV1 {
        store_head: publication.head().clone(),
        intent,
        control_head,
        control_revision,
        replayed,
        provider_release,
    })
}

fn effect_dispatch_outcome_value(outcome: EffectDispatchOutcomePayloadV1) -> CborValue {
    match outcome {
        EffectDispatchOutcomePayloadV1::LocallyRejected {
            evidence_commitment,
        } => CborValue::Array(vec![CborValue::Unsigned(1), bytes(&evidence_commitment)]),
        EffectDispatchOutcomePayloadV1::DefinitelyNotSent {
            evidence_commitment,
        } => CborValue::Array(vec![CborValue::Unsigned(2), bytes(&evidence_commitment)]),
        EffectDispatchOutcomePayloadV1::ResponseReceived {
            evidence_commitment,
            classification,
        } => CborValue::Array(vec![
            CborValue::Unsigned(3),
            bytes(&evidence_commitment),
            CborValue::Unsigned(remote_classification_store_tag(classification)),
        ]),
        EffectDispatchOutcomePayloadV1::AmbiguousTransport {
            evidence_commitment,
        } => CborValue::Array(vec![CborValue::Unsigned(4), bytes(&evidence_commitment)]),
    }
}

fn effect_dispatch_outcome_evidence_commitment(
    outcome: EffectDispatchOutcomePayloadV1,
) -> [u8; 32] {
    match outcome {
        EffectDispatchOutcomePayloadV1::LocallyRejected {
            evidence_commitment,
        }
        | EffectDispatchOutcomePayloadV1::DefinitelyNotSent {
            evidence_commitment,
        }
        | EffectDispatchOutcomePayloadV1::ResponseReceived {
            evidence_commitment,
            ..
        }
        | EffectDispatchOutcomePayloadV1::AmbiguousTransport {
            evidence_commitment,
        } => evidence_commitment,
    }
}

fn provider_operation_binding_value(
    operation: &ProviderOperationBindingV1,
) -> Result<CborValue, CborError> {
    Ok(CborValue::Array(vec![
        CborValue::text("maestro.vnext.sealed-provider-operation.v1")?,
        bytes(operation.run_id.as_bytes()),
        bytes(&operation.execution_boundary_commitment),
        CborValue::Unsigned(operation.deadline),
        bytes(&operation.application_envelope_commitment),
        bytes(&operation.provider_operation_contract_commitment),
        bytes(&operation.provider_scope_commitment),
        bytes(&operation.provider_key_commitment),
        bytes(&operation.credential_commitment),
        bytes(&operation.semantic_operation_commitment),
        bytes(&operation.payload_commitment),
        bytes(&operation.target_commitment),
    ]))
}

fn sealed_provider_operation_from_snapshot(
    snapshot: &ActiveStoreEffectSnapshotV1,
) -> Result<SealedProviderOperationV1, ExecutionStoreErrorV1> {
    sealed_provider_operation_from_dispatch(&snapshot.dispatch)
}

fn sealed_provider_operation_from_dispatch(
    dispatch: &EffectDispatchAttemptV1,
) -> Result<SealedProviderOperationV1, ExecutionStoreErrorV1> {
    let binding = dispatch.state().binding();
    let run = dispatch
        .run_set()
        .runs()
        .first()
        .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?;
    let reservation = run.reservation();
    Ok(SealedProviderOperationV1 {
        binding: ProviderOperationBindingV1 {
            run_id: run.id(),
            execution_boundary_commitment: reservation.execution_boundary_commitment,
            deadline: reservation.deadline,
            application_envelope_commitment: *binding.application_envelope_id().as_bytes(),
            provider_operation_contract_commitment: *binding
                .provider_operation_contract_id()
                .as_bytes(),
            provider_scope_commitment: *binding.provider_scope_id().as_bytes(),
            provider_key_commitment: *binding.provider_key_id().as_bytes(),
            credential_commitment: *binding.credential_id().as_bytes(),
            semantic_operation_commitment: reservation.semantic_operation_hash,
            payload_commitment: reservation.inputs_commitment,
            target_commitment: reservation.target_commitment,
        },
    })
}

fn decode_effect_dispatch_outcome_value(
    value: &CborValue,
) -> Result<EffectDispatchOutcomePayloadV1, ExecutionStoreErrorV1> {
    let CborValue::Array(fields) = value else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    };
    match fields.as_slice() {
        [CborValue::Unsigned(1), evidence] => Ok(EffectDispatchOutcomePayloadV1::LocallyRejected {
            evidence_commitment: exact_digest(evidence)
                .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?,
        }),
        [CborValue::Unsigned(2), evidence] => {
            Ok(EffectDispatchOutcomePayloadV1::DefinitelyNotSent {
                evidence_commitment: exact_digest(evidence)
                    .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?,
            })
        }
        [
            CborValue::Unsigned(3),
            evidence,
            CborValue::Unsigned(classification),
        ] => Ok(EffectDispatchOutcomePayloadV1::ResponseReceived {
            evidence_commitment: exact_digest(evidence)
                .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?,
            classification: parse_remote_classification_store_tag(*classification)?,
        }),
        [CborValue::Unsigned(4), evidence] => {
            Ok(EffectDispatchOutcomePayloadV1::AmbiguousTransport {
                evidence_commitment: exact_digest(evidence)
                    .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?,
            })
        }
        _ => Err(ExecutionStoreErrorV1::InvalidEffectSnapshot),
    }
}

fn decode_effect_terminal_payload_outcome(
    value: &CborValue,
) -> Result<EffectDispatchOutcomePayloadV1, ExecutionStoreErrorV1> {
    let CborValue::Array(fields) = value else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    };
    let [CborValue::Text(domain), outcome, provider_proof] = fields.as_slice() else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    };
    if domain != "maestro.vnext.effect-dispatch-terminal-payload.v1"
        || !matches!(
            provider_proof,
            CborValue::Array(optional)
                if optional.as_slice() == [CborValue::Unsigned(0)]
                    || matches!(optional.as_slice(), [CborValue::Unsigned(1), _])
        )
    {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    }
    decode_effect_dispatch_outcome_value(outcome)
}

fn decode_sealed_provider_operation_value(
    value: &CborValue,
) -> Result<ProviderOperationBindingV1, ExecutionStoreErrorV1> {
    let CborValue::Array(fields) = value else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    };
    let [
        CborValue::Text(domain),
        run_id,
        execution_boundary,
        CborValue::Unsigned(deadline),
        envelope,
        operation,
        scope,
        provider_key,
        credential,
        semantic_operation,
        payload,
        target,
    ] = fields.as_slice()
    else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    };
    if domain != "maestro.vnext.sealed-provider-operation.v1" {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    }
    Ok(ProviderOperationBindingV1 {
        run_id: RunIdV1::from_bytes(
            exact_digest(run_id).ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?,
        )?,
        execution_boundary_commitment: exact_digest(execution_boundary)
            .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?,
        deadline: *deadline,
        application_envelope_commitment: exact_digest(envelope)
            .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?,
        provider_operation_contract_commitment: exact_digest(operation)
            .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?,
        provider_scope_commitment: exact_digest(scope)
            .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?,
        provider_key_commitment: exact_digest(provider_key)
            .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?,
        credential_commitment: exact_digest(credential)
            .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?,
        semantic_operation_commitment: exact_digest(semantic_operation)
            .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?,
        payload_commitment: exact_digest(payload)
            .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?,
        target_commitment: exact_digest(target)
            .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?,
    })
}

fn validate_effect_terminal_payload_proof(
    value: &CborValue,
    intent: &EffectIntentV1,
    dispatch: &EffectDispatchAttemptV1,
    predecessor_head: EffectIntentControlTokenV1,
) -> Result<(), ExecutionStoreErrorV1> {
    let CborValue::Array(fields) = value else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    };
    let [
        CborValue::Text(domain),
        outcome_value,
        CborValue::Array(optional_proof),
    ] = fields.as_slice()
    else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    };
    if domain != "maestro.vnext.effect-dispatch-terminal-payload.v1" {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    }
    let outcome = decode_effect_dispatch_outcome_value(outcome_value)?;
    match (outcome, optional_proof.as_slice()) {
        (EffectDispatchOutcomePayloadV1::LocallyRejected { .. }, [CborValue::Unsigned(0)]) => {
            Ok(())
        }
        (
            EffectDispatchOutcomePayloadV1::DefinitelyNotSent { .. }
            | EffectDispatchOutcomePayloadV1::ResponseReceived { .. }
            | EffectDispatchOutcomePayloadV1::AmbiguousTransport { .. },
            [CborValue::Unsigned(1), CborValue::Array(proof)],
        ) => {
            let [
                proof_intent,
                proof_attempt,
                proof_head,
                operation,
                outcome_commitment,
            ] = proof.as_slice()
            else {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            };
            let operation = decode_sealed_provider_operation_value(operation)?;
            let expected_commitment = hash(&CborValue::Array(vec![
                CborValue::text("maestro.vnext.provider-dispatch-outcome-proof.v1")?,
                bytes(intent.id().as_bytes()),
                bytes(dispatch.attempt().id().as_bytes()),
                bytes(predecessor_head.as_bytes()),
                provider_operation_binding_value(&operation)?,
                effect_dispatch_outcome_value(outcome),
            ]))?;
            if exact_digest(proof_intent) != Some(*intent.id().as_bytes())
                || exact_digest(proof_attempt) != Some(*dispatch.attempt().id().as_bytes())
                || exact_digest(proof_head) != Some(*predecessor_head.as_bytes())
                || operation != sealed_provider_operation_from_dispatch(dispatch)?.binding
                || exact_digest(outcome_commitment) != Some(expected_commitment)
            {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            }
            Ok(())
        }
        _ => Err(ExecutionStoreErrorV1::InvalidEffectSnapshot),
    }
}

fn effect_dispatch_terminal_occurrence_object(
    request: &CanonicalExecutionActionRequestV1,
    draft: &ActiveStoreEffectTerminalDraftV1,
    request_object_id: StoreObjectIdV1,
    authority_basis_object_id: StoreObjectIdV1,
    meaning_digest: [u8; 32],
) -> Result<StoreObjectV1, ExecutionStoreErrorV1> {
    let mut references = vec![request_object_id, authority_basis_object_id];
    references.sort_unstable();
    Ok(StoreObjectV1::new(
        execution_schema_id("maestro.vnext.effect-dispatch-terminal-occurrence-schema.v1")?,
        CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-dispatch-terminal-occurrence.v1")?,
            bytes(request.request_id().as_bytes()),
            draft.payload_value()?,
            bytes(&meaning_digest),
        ]),
        references,
    )?)
}

fn effect_recover_sealed_occurrence_object(
    request: &CanonicalExecutionActionRequestV1,
    snapshot: &ActiveStoreEffectSnapshotV1,
    request_object_id: StoreObjectIdV1,
    authority_basis_object_id: StoreObjectIdV1,
    meaning_digest: [u8; 32],
) -> Result<StoreObjectV1, ExecutionStoreErrorV1> {
    let current_control_head_id = snapshot
        .state_binding
        .control_index_object_id()
        .ok_or(ExecutionStoreErrorV1::PublicationBindingMismatch)?;
    let owner = snapshot
        .control_revision
        .live_attempt()
        .ok_or(ExecutionStoreErrorV1::PublicationBindingMismatch)?;
    let mut references = vec![request_object_id, authority_basis_object_id];
    references.sort_unstable();
    references.dedup();
    Ok(StoreObjectV1::new(
        execution_schema_id("maestro.vnext.effect-recover-sealed-occurrence-schema.v1")?,
        CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-recover-sealed-occurrence.v1")?,
            bytes(request.request_id().as_bytes()),
            bytes(snapshot.intent.id().as_bytes()),
            bytes(snapshot.control_head.id().as_bytes()),
            bytes(current_control_head_id.as_bytes()),
            super::effects::execution_owner_value(owner),
            bytes(&meaning_digest),
        ]),
        references,
    )?)
}

#[allow(clippy::too_many_arguments)]
fn build_effect_terminal_publication(
    domain: crate::domain::persistence::StoreDomainV1,
    current_head: &StoreHeadV1,
    current_generation: &StoreGenerationV1,
    active_objects: &[StoreObjectV1],
    request: &CanonicalExecutionActionRequestV1,
    authority: &ExecutionAuthorityV1,
    admission: &AdmittedRepositoryActionV1,
    request_object: StoreObjectV1,
    occurrence_object: StoreObjectV1,
    snapshot: &ActiveStoreEffectSnapshotV1,
    candidate: &EffectDispatchTerminalCandidateV1,
    terminal_revision: &EffectIntentControlRevisionV1,
    terminal_head: &EffectIntentControlHeadV1,
    meaning_digest: [u8; 32],
    idempotency_namespace: &'static str,
) -> Result<AtomicGenerationPublicationV1, ExecutionStoreErrorV1> {
    if candidate.dispatch().attempt().id() != snapshot.dispatch.attempt().id()
        || terminal_revision.intent() != snapshot.intent.id()
        || terminal_head.intent() != snapshot.intent.id()
        || terminal_head.revision() != terminal_revision.id()
        || terminal_head.writer_term() != snapshot.writer_term.id()
        || terminal_revision.classification() != candidate.classification()
    {
        return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
    }
    let current_index = load_control_index(active_objects)?;
    if current_index.object.id()
        != snapshot
            .state_binding
            .control_index_object_id()
            .ok_or(ExecutionStoreErrorV1::PublicationBindingMismatch)?
    {
        return Err(ExecutionStoreErrorV1::StaleExpectedStoreState);
    }
    let current_entry = current_index
        .entries
        .iter()
        .find(|entry| entry.intent == snapshot.intent.id())
        .ok_or(ExecutionStoreErrorV1::MissingIntentControlSelector)?;
    let current_head_object = active_objects
        .iter()
        .find(|object| object.id() == current_entry.control_head_object_id)
        .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?;
    let intent_object = exact_referenced_schema_object(
        current_head_object,
        active_objects,
        &[execution_schema_id(
            "maestro.vnext.effect-intent-schema.v1",
        )?],
    )?;
    let current_revision_object = exact_referenced_schema_object(
        current_head_object,
        active_objects,
        &[execution_schema_id(
            "maestro.vnext.effect-intent-control-revision-schema.v1",
        )?],
    )?;
    let writer_object = exact_referenced_schema_object(
        current_head_object,
        active_objects,
        &[execution_schema_id(
            "maestro.vnext.effect-intent-control-writer-term-schema.v1",
        )?],
    )?;
    let prior_dispatch_object = exact_referenced_schema_object(
        current_head_object,
        active_objects,
        &[
            execution_schema_id("maestro.vnext.effect-dispatch-reservation-carrier-schema.v1")?,
            execution_schema_id("maestro.vnext.effect-dispatch-seal-carrier-schema.v1")?,
        ],
    )?;
    let mut carrier_references = vec![
        prior_dispatch_object.id(),
        current_head_object.id(),
        request_object.id(),
        admission.basis_object().id(),
        occurrence_object.id(),
    ];
    carrier_references.sort_unstable();
    let carrier_object = StoreObjectV1::new(
        execution_schema_id("maestro.vnext.effect-dispatch-terminal-carrier-schema.v1")?,
        effect_dispatch_authorized_carrier_value(
            candidate.persistence_carrier_value()?,
            authority,
        )?,
        carrier_references,
    )?;
    let mut revision_references = vec![
        current_revision_object.id(),
        carrier_object.id(),
        occurrence_object.id(),
    ];
    revision_references.sort_unstable();
    let revision_object = StoreObjectV1::new(
        execution_schema_id("maestro.vnext.effect-intent-control-revision-schema.v1")?,
        terminal_revision.canonical_value()?,
        revision_references,
    )?;
    let mut head_references = vec![
        intent_object.id(),
        revision_object.id(),
        writer_object.id(),
        carrier_object.id(),
    ];
    head_references.sort_unstable();
    let head_object = StoreObjectV1::new(
        execution_schema_id("maestro.vnext.effect-intent-control-head-schema.v1")?,
        terminal_head.canonical_value(),
        head_references,
    )?;
    let mut next_entries = current_index.entries.clone();
    let selected = next_entries
        .iter_mut()
        .find(|entry| entry.intent == snapshot.intent.id())
        .ok_or(ExecutionStoreErrorV1::MissingIntentControlSelector)?;
    selected.control_head = terminal_head.id();
    selected.control_head_object_id = head_object.id();
    let index_object = build_control_index_object(&next_entries)?;
    let produced_objects = vec![
        occurrence_object.clone(),
        carrier_object.clone(),
        revision_object.clone(),
        head_object.clone(),
    ];
    let artifacts = admission.issue_committed_artifacts(&request_object, &produced_objects)?;
    let mut result_references = vec![
        artifacts.result_object().id(),
        occurrence_object.id(),
        carrier_object.id(),
        revision_object.id(),
        head_object.id(),
    ];
    result_references.sort_unstable();
    let result_object = StoreObjectV1::new(
        execution_schema_id("maestro.vnext.effect-dispatch-terminal-result-schema.v1")?,
        CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-dispatch-terminal-result.v1")?,
            bytes(request.request_id().as_bytes()),
            bytes(snapshot.intent.id().as_bytes()),
            bytes(terminal_head.id().as_bytes()),
            bytes(terminal_revision.id().as_bytes()),
            CborValue::Unsigned(remote_classification_store_tag(candidate.classification())),
            bytes(&meaning_digest),
        ]),
        result_references,
    )?;
    let mut roots = current_generation.roots().to_vec();
    replace_required_root(&mut roots, current_index.object.id(), index_object.id())?;
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
    roots.push(result_object.id());
    roots.sort_unstable();
    roots.dedup();
    let generation = StoreGenerationV1::new(
        domain,
        current_generation
            .ordinal()
            .checked_add(1)
            .ok_or(ExecutionStoreErrorV1::GenerationOverflow)?,
        Some(current_generation.id()),
        current_generation.contract_root_id(),
        current_generation.compatibility().clone(),
        roots,
    )?;
    let idempotency = StoreIdempotencyV1::new(
        idempotency_namespace,
        *request.idempotency_key_id().as_bytes(),
        meaning_digest,
        result_object.id(),
    )?;
    let mut objects = active_objects.to_vec();
    objects.extend(produced_objects);
    objects.extend([
        index_object,
        request_object,
        admission.basis_object().clone(),
        admission.successor_snapshot().clone(),
        admission.successor_capacity_root().clone(),
        admission.capacity_debit().clone(),
        artifacts.receipt_object().clone(),
        artifacts.result_object().clone(),
        result_object,
    ]);
    objects.extend(artifacts.leaf_authority_objects().iter().cloned());
    objects.sort_by_key(StoreObjectV1::id);
    objects.dedup_by_key(|object| object.id());
    Ok(AtomicGenerationPublicationV1::new_from_object_superset(
        generation,
        Some(current_head.id()),
        objects,
        idempotency,
    )?)
}

fn decode_effect_terminal_outcome(
    publication: StorePublicationOutcomeV1,
) -> Result<ActiveStoreEffectTerminalOutcomeV1, ExecutionStoreErrorV1> {
    let replayed = matches!(&publication, StorePublicationOutcomeV1::Replayed { .. });
    let result = publication.result();
    if result.schema_id()
        != execution_schema_id("maestro.vnext.effect-dispatch-terminal-result-schema.v1")?
    {
        return Err(ExecutionStoreErrorV1::InvalidEffectTerminalResult);
    }
    let CborValue::Array(fields) = result.value() else {
        return Err(ExecutionStoreErrorV1::InvalidEffectTerminalResult);
    };
    let [
        CborValue::Text(domain),
        _request,
        intent,
        control_head,
        control_revision,
        CborValue::Unsigned(classification),
        _meaning,
    ] = fields.as_slice()
    else {
        return Err(ExecutionStoreErrorV1::InvalidEffectTerminalResult);
    };
    if domain != "maestro.vnext.effect-dispatch-terminal-result.v1" {
        return Err(ExecutionStoreErrorV1::InvalidEffectTerminalResult);
    }
    Ok(ActiveStoreEffectTerminalOutcomeV1 {
        store_head: publication.head().clone(),
        intent: EffectIntentIdV1::from_bytes(
            exact_digest(intent).ok_or(ExecutionStoreErrorV1::InvalidEffectTerminalResult)?,
        )?,
        control_head: EffectIntentControlTokenV1::new(HomeTokenV1::new(
            exact_digest(control_head).ok_or(ExecutionStoreErrorV1::InvalidEffectTerminalResult)?,
        )),
        control_revision: EffectIntentControlTokenV1::new(HomeTokenV1::new(
            exact_digest(control_revision)
                .ok_or(ExecutionStoreErrorV1::InvalidEffectTerminalResult)?,
        )),
        classification: parse_remote_classification_store_tag(*classification)?,
        replayed,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReconciliationPublicationPhaseV1 {
    Begin,
    Read,
    Terminal,
}

impl ReconciliationPublicationPhaseV1 {
    const fn carrier_schema(self) -> &'static str {
        match self {
            Self::Begin => "maestro.vnext.effect-reconciliation-begin-carrier-schema.v1",
            Self::Read => "maestro.vnext.effect-reconciliation-read-carrier-schema.v1",
            Self::Terminal => "maestro.vnext.effect-reconciliation-terminal-carrier-schema.v1",
        }
    }

    const fn result_schema(self) -> &'static str {
        match self {
            Self::Begin => "maestro.vnext.effect-reconciliation-begin-result-schema.v1",
            Self::Read => "maestro.vnext.effect-reconciliation-read-result-schema.v1",
            Self::Terminal => "maestro.vnext.effect-reconciliation-terminal-result-schema.v1",
        }
    }

    const fn result_domain(self) -> &'static str {
        match self {
            Self::Begin => "maestro.vnext.effect-reconciliation-begin-result.v1",
            Self::Read => "maestro.vnext.effect-reconciliation-read-result.v1",
            Self::Terminal => "maestro.vnext.effect-reconciliation-terminal-result.v1",
        }
    }

    const fn idempotency_namespace(self) -> &'static str {
        match self {
            Self::Begin => EFFECT_RECONCILIATION_BEGIN_IDEMPOTENCY_NAMESPACE_V1,
            Self::Read => EFFECT_RECONCILIATION_READ_IDEMPOTENCY_NAMESPACE_V1,
            Self::Terminal => EFFECT_RECONCILIATION_TERMINAL_IDEMPOTENCY_NAMESPACE_V1,
        }
    }

    const fn occurrence_idempotency_domain(self) -> Option<&'static str> {
        match self {
            Self::Begin => None,
            Self::Read => Some("maestro.vnext.effect-reconciliation-read-idempotency.v1"),
            Self::Terminal => Some("maestro.vnext.effect-reconciliation-terminal-idempotency.v1"),
        }
    }
}

fn reconciliation_read_usage_store_value(usage: EffectReconciliationReadUsageV1) -> CborValue {
    CborValue::Array(vec![
        CborValue::Unsigned(u64::from(usage.requests)),
        CborValue::Unsigned(u64::from(usage.pages)),
        CborValue::Unsigned(usage.bytes),
        CborValue::Unsigned(usage.duration_ms),
        bytes(&usage.result_commitment),
    ])
}

fn provider_application_fact_classification(
    fact: ProviderApplicationFactV1,
) -> super::withdrawal::RemoteClassificationV1 {
    match fact {
        ProviderApplicationFactV1::Applied => {
            super::withdrawal::RemoteClassificationV1::ConfirmedApplied
        }
        ProviderApplicationFactV1::NotApplied => {
            super::withdrawal::RemoteClassificationV1::ConfirmedNotApplied
        }
        ProviderApplicationFactV1::Pending => super::withdrawal::RemoteClassificationV1::Pending,
        ProviderApplicationFactV1::Unknown => super::withdrawal::RemoteClassificationV1::InDoubt,
        ProviderApplicationFactV1::PartiallyApplied => {
            super::withdrawal::RemoteClassificationV1::PartiallyApplied
        }
        ProviderApplicationFactV1::Conflicted => {
            super::withdrawal::RemoteClassificationV1::Conflicted
        }
    }
}

fn validate_reconciliation_read_observation(
    plan: EffectReconciliationReadPlanV1,
    observation: ReconciliationReadObservationV1,
) -> Result<(), ExecutionStoreErrorV1> {
    let usage = observation.usage;
    if usage.requests == 0
        || usage.requests > plan.max_requests()
        || usage.pages == 0
        || usage.pages > plan.max_pages()
        || usage.bytes == 0
        || usage.bytes > plan.max_bytes()
        || usage.duration_ms == 0
        || usage.duration_ms > plan.max_duration_ms()
        || usage.result_commitment == [0; 32]
    {
        return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
    }
    Ok(())
}

fn reconciliation_read_proof_commitment(
    read: &ReconciliationReadBindingV1,
    usage: EffectReconciliationReadUsageV1,
    classification: super::withdrawal::RemoteClassificationV1,
) -> Result<[u8; 32], ExecutionStoreErrorV1> {
    Ok(hash(&CborValue::Array(vec![
        CborValue::text("maestro.vnext.reconciliation-read-proof.v1")?,
        bytes(read.run_id.as_bytes()),
        bytes(&read.execution_boundary_commitment),
        CborValue::Unsigned(read.deadline),
        bytes(read.intent.as_bytes()),
        bytes(read.control_head.as_bytes()),
        read.read_plan.canonical_value()?,
        reconciliation_read_usage_store_value(usage),
        CborValue::Unsigned(remote_classification_store_tag(classification)),
    ]))?)
}

fn validate_reconciliation_read_draft_for_snapshot(
    snapshot: &ActiveStoreEffectSnapshotV1,
    draft: ActiveStoreEffectReconciliationReadDraftV1,
) -> Result<(), ExecutionStoreErrorV1> {
    let attempt = snapshot
        .reconciliation
        .as_ref()
        .ok_or(ExecutionStoreErrorV1::PublicationBindingMismatch)?;
    if draft.proof.read.intent != snapshot.intent.id()
        || draft.proof.read.control_head != snapshot.control_head.id()
        || draft.proof.read.read_plan != attempt.read_plan()
        || draft.proof.proof_commitment
            != reconciliation_read_proof_commitment(
                &draft.proof.read,
                draft.usage,
                draft.classification,
            )?
    {
        return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
    }
    validate_reconciliation_read_observation(
        attempt.read_plan(),
        ReconciliationReadObservationV1 {
            usage: draft.usage,
            application_fact: match draft.classification {
                super::withdrawal::RemoteClassificationV1::ConfirmedApplied => {
                    ProviderApplicationFactV1::Applied
                }
                super::withdrawal::RemoteClassificationV1::ConfirmedNotApplied => {
                    ProviderApplicationFactV1::NotApplied
                }
                super::withdrawal::RemoteClassificationV1::Pending => {
                    ProviderApplicationFactV1::Pending
                }
                super::withdrawal::RemoteClassificationV1::InDoubt => {
                    ProviderApplicationFactV1::Unknown
                }
                super::withdrawal::RemoteClassificationV1::PartiallyApplied => {
                    ProviderApplicationFactV1::PartiallyApplied
                }
                super::withdrawal::RemoteClassificationV1::Conflicted => {
                    ProviderApplicationFactV1::Conflicted
                }
                _ => return Err(ExecutionStoreErrorV1::PublicationBindingMismatch),
            },
        },
    )
}

fn effect_reconciliation_read_occurrence_object(
    request: &CanonicalExecutionActionRequestV1,
    draft: ActiveStoreEffectReconciliationReadDraftV1,
    request_object_id: StoreObjectIdV1,
    authority_basis_object_id: StoreObjectIdV1,
    meaning_digest: [u8; 32],
) -> Result<StoreObjectV1, ExecutionStoreErrorV1> {
    let mut references = vec![request_object_id, authority_basis_object_id];
    references.sort_unstable();
    Ok(StoreObjectV1::new(
        execution_schema_id("maestro.vnext.effect-reconciliation-read-occurrence-schema.v1")?,
        CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-reconciliation-read-occurrence.v1")?,
            bytes(request.request_id().as_bytes()),
            reconciliation_read_usage_store_value(draft.usage()),
            CborValue::Unsigned(remote_classification_store_tag(draft.classification())),
            reconciliation_read_binding_value(&draft.proof.read)?,
            bytes(&draft.proof.proof_commitment),
            bytes(&meaning_digest),
        ]),
        references,
    )?)
}

fn reconciliation_read_binding_value(
    read: &ReconciliationReadBindingV1,
) -> Result<CborValue, ExecutionStoreErrorV1> {
    Ok(CborValue::Array(vec![
        CborValue::text("maestro.vnext.sealed-reconciliation-read.v1")?,
        bytes(read.run_id.as_bytes()),
        bytes(&read.execution_boundary_commitment),
        CborValue::Unsigned(read.deadline),
        bytes(read.intent.as_bytes()),
        bytes(read.control_head.as_bytes()),
        read.read_plan.canonical_value()?,
    ]))
}

fn decode_sealed_reconciliation_read_value(
    value: &CborValue,
) -> Result<ReconciliationReadBindingV1, ExecutionStoreErrorV1> {
    let CborValue::Array(fields) = value else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    };
    let [
        CborValue::Text(domain),
        run_id,
        execution_boundary,
        CborValue::Unsigned(deadline),
        intent,
        control_head,
        read_plan,
    ] = fields.as_slice()
    else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    };
    if domain != "maestro.vnext.sealed-reconciliation-read.v1" {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    }
    Ok(ReconciliationReadBindingV1 {
        run_id: RunIdV1::from_bytes(
            exact_digest(run_id).ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?,
        )?,
        execution_boundary_commitment: exact_digest(execution_boundary)
            .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?,
        deadline: *deadline,
        intent: EffectIntentIdV1::from_bytes(
            exact_digest(intent).ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?,
        )?,
        control_head: EffectIntentControlTokenV1::new(HomeTokenV1::new(
            exact_digest(control_head).ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?,
        )),
        read_plan: EffectReconciliationReadPlanV1::from_canonical_value(read_plan)?,
    })
}

fn effect_reconciliation_terminal_occurrence_object(
    request: &CanonicalExecutionActionRequestV1,
    draft: ActiveStoreEffectReconciliationTerminalDraftV1,
    read_result_commitment: [u8; 32],
    request_object_id: StoreObjectIdV1,
    authority_basis_object_id: StoreObjectIdV1,
    meaning_digest: [u8; 32],
) -> Result<StoreObjectV1, ExecutionStoreErrorV1> {
    let mut references = vec![request_object_id, authority_basis_object_id];
    references.sort_unstable();
    Ok(StoreObjectV1::new(
        execution_schema_id("maestro.vnext.effect-reconciliation-terminal-occurrence-schema.v1")?,
        CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-reconciliation-terminal-occurrence.v1")?,
            bytes(request.request_id().as_bytes()),
            CborValue::Unsigned(remote_classification_store_tag(draft.classification())),
            bytes(&read_result_commitment),
            bytes(&meaning_digest),
        ]),
        references,
    )?)
}

fn historical_reconciliation_begin_requests(
    active_objects: &[StoreObjectV1],
    intent: EffectIntentIdV1,
) -> Result<Vec<crate::domain::authority::ActionRequestIdV1>, ExecutionStoreErrorV1> {
    let schema =
        execution_schema_id("maestro.vnext.effect-reconciliation-begin-carrier-schema.v1")?;
    let mut requests = active_objects
        .iter()
        .filter(|object| object.schema_id() == schema)
        .map(|object| {
            let (attempt, need, _) =
                decode_effect_reconciliation_authorized_carrier(object.value())?;
            Ok((attempt, need.action_request_id()))
        })
        .collect::<Result<Vec<_>, ExecutionStoreErrorV1>>()?
        .into_iter()
        .filter_map(|(attempt, request)| {
            (attempt.attempt().effect_intent_id() == intent).then_some(request)
        })
        .collect::<Vec<_>>();
    requests.sort_unstable();
    requests.dedup();
    Ok(requests)
}

#[allow(clippy::too_many_arguments)]
fn build_effect_reconciliation_publication(
    domain: crate::domain::persistence::StoreDomainV1,
    current_head: &StoreHeadV1,
    current_generation: &StoreGenerationV1,
    active_objects: &[StoreObjectV1],
    request: &CanonicalExecutionActionRequestV1,
    admission: Option<&AdmittedRepositoryActionV1>,
    continuation: Option<&ContinuedRepositoryActionV1>,
    authority_basis_object_id: StoreObjectIdV1,
    request_object: StoreObjectV1,
    snapshot: &ActiveStoreEffectSnapshotV1,
    phase: ReconciliationPublicationPhaseV1,
    carrier_value: CborValue,
    occurrence_object: Option<StoreObjectV1>,
    next_revision: &EffectIntentControlRevisionV1,
    next_head: &EffectIntentControlHeadV1,
    meaning_digest: [u8; 32],
) -> Result<AtomicGenerationPublicationV1, ExecutionStoreErrorV1> {
    if next_revision.intent() != snapshot.intent.id()
        || next_head.intent() != snapshot.intent.id()
        || next_head.revision() != next_revision.id()
        || next_head.writer_term() != snapshot.writer_term.id()
        || matches!(phase, ReconciliationPublicationPhaseV1::Begin) != occurrence_object.is_none()
        || matches!(phase, ReconciliationPublicationPhaseV1::Begin) != admission.is_some()
        || matches!(phase, ReconciliationPublicationPhaseV1::Begin) == continuation.is_some()
        || admission.is_some_and(|value| value.basis_object().id() != authority_basis_object_id)
    {
        return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
    }
    let current_index = load_control_index(active_objects)?;
    if current_index.object.id()
        != snapshot
            .state_binding
            .control_index_object_id()
            .ok_or(ExecutionStoreErrorV1::PublicationBindingMismatch)?
    {
        return Err(ExecutionStoreErrorV1::StaleExpectedStoreState);
    }
    let current_entry = current_index
        .entries
        .iter()
        .find(|entry| entry.intent == snapshot.intent.id())
        .ok_or(ExecutionStoreErrorV1::MissingIntentControlSelector)?;
    let current_head_object = active_objects
        .iter()
        .find(|object| object.id() == current_entry.control_head_object_id)
        .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?;
    let intent_object = exact_referenced_schema_object(
        current_head_object,
        active_objects,
        &[execution_schema_id(
            "maestro.vnext.effect-intent-schema.v1",
        )?],
    )?;
    let current_revision_object = exact_referenced_schema_object(
        current_head_object,
        active_objects,
        &[execution_schema_id(
            "maestro.vnext.effect-intent-control-revision-schema.v1",
        )?],
    )?;
    let writer_object = exact_referenced_schema_object(
        current_head_object,
        active_objects,
        &[execution_schema_id(
            "maestro.vnext.effect-intent-control-writer-term-schema.v1",
        )?],
    )?;
    let dispatch_object = exact_referenced_schema_object(
        current_head_object,
        active_objects,
        &[
            execution_schema_id("maestro.vnext.effect-dispatch-reservation-carrier-schema.v1")?,
            execution_schema_id("maestro.vnext.effect-dispatch-seal-carrier-schema.v1")?,
            execution_schema_id("maestro.vnext.effect-dispatch-terminal-carrier-schema.v1")?,
        ],
    )?;
    let predecessor = if matches!(phase, ReconciliationPublicationPhaseV1::Begin) {
        None
    } else {
        Some(exact_referenced_schema_object(
            current_head_object,
            active_objects,
            &[
                execution_schema_id("maestro.vnext.effect-reconciliation-begin-carrier-schema.v1")?,
                execution_schema_id("maestro.vnext.effect-reconciliation-read-carrier-schema.v1")?,
            ],
        )?)
    };
    let mut carrier_references = vec![
        request_object.id(),
        authority_basis_object_id,
        current_head_object.id(),
    ];
    carrier_references.extend(predecessor.map(StoreObjectV1::id));
    carrier_references.extend(occurrence_object.as_ref().map(StoreObjectV1::id));
    carrier_references.sort_unstable();
    carrier_references.dedup();
    let carrier_object = StoreObjectV1::new(
        execution_schema_id(phase.carrier_schema())?,
        carrier_value,
        carrier_references,
    )?;
    let mut revision_references = vec![current_revision_object.id(), carrier_object.id()];
    revision_references.extend(occurrence_object.as_ref().map(StoreObjectV1::id));
    revision_references.sort_unstable();
    let revision_object = StoreObjectV1::new(
        execution_schema_id("maestro.vnext.effect-intent-control-revision-schema.v1")?,
        next_revision.canonical_value()?,
        revision_references,
    )?;
    let mut head_references = vec![
        intent_object.id(),
        revision_object.id(),
        writer_object.id(),
        dispatch_object.id(),
        carrier_object.id(),
    ];
    head_references.sort_unstable();
    let head_object = StoreObjectV1::new(
        execution_schema_id("maestro.vnext.effect-intent-control-head-schema.v1")?,
        next_head.canonical_value(),
        head_references,
    )?;
    let mut next_entries = current_index.entries.clone();
    let selected = next_entries
        .iter_mut()
        .find(|entry| entry.intent == snapshot.intent.id())
        .ok_or(ExecutionStoreErrorV1::MissingIntentControlSelector)?;
    selected.control_head = next_head.id();
    selected.control_head_object_id = head_object.id();
    let index_object = build_control_index_object(&next_entries)?;
    let mut produced_objects = vec![
        carrier_object.clone(),
        revision_object.clone(),
        head_object.clone(),
    ];
    produced_objects.extend(occurrence_object.iter().cloned());
    let artifacts = admission
        .map(|value| value.issue_committed_artifacts(&request_object, &produced_objects))
        .transpose()?;
    let mut result_references = vec![carrier_object.id(), revision_object.id(), head_object.id()];
    result_references.extend(artifacts.as_ref().map(|value| value.result_object().id()));
    result_references.extend(occurrence_object.as_ref().map(StoreObjectV1::id));
    result_references.sort_unstable();
    result_references.dedup();
    let result_object = StoreObjectV1::new(
        execution_schema_id(phase.result_schema())?,
        CborValue::Array(vec![
            CborValue::text(phase.result_domain())?,
            bytes(request.request_id().as_bytes()),
            bytes(snapshot.intent.id().as_bytes()),
            bytes(next_head.id().as_bytes()),
            bytes(next_revision.id().as_bytes()),
            CborValue::Unsigned(remote_classification_store_tag(
                next_revision.classification(),
            )),
            bytes(&meaning_digest),
        ]),
        result_references,
    )?;
    let mut roots = current_generation.roots().to_vec();
    replace_required_root(&mut roots, current_index.object.id(), index_object.id())?;
    match (admission, continuation) {
        (Some(admission), None) => {
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
        }
        (None, Some(continuation)) => replace_required_root(
            &mut roots,
            continuation.current_snapshot_id(),
            continuation.successor_snapshot().id(),
        )?,
        _ => return Err(ExecutionStoreErrorV1::PublicationBindingMismatch),
    }
    roots.push(result_object.id());
    roots.sort_unstable();
    roots.dedup();
    let generation = StoreGenerationV1::new(
        domain,
        current_generation
            .ordinal()
            .checked_add(1)
            .ok_or(ExecutionStoreErrorV1::GenerationOverflow)?,
        Some(current_generation.id()),
        current_generation.contract_root_id(),
        current_generation.compatibility().clone(),
        roots,
    )?;
    let idempotency = StoreIdempotencyV1::new(
        phase.idempotency_namespace(),
        *request.idempotency_key_id().as_bytes(),
        meaning_digest,
        result_object.id(),
    )?;
    let mut objects = active_objects.to_vec();
    objects.extend(produced_objects);
    objects.extend([index_object, request_object, result_object]);
    if let (Some(admission), Some(artifacts)) = (admission, artifacts.as_ref()) {
        objects.extend([
            admission.basis_object().clone(),
            admission.successor_snapshot().clone(),
            admission.successor_capacity_root().clone(),
            admission.capacity_debit().clone(),
            artifacts.receipt_object().clone(),
            artifacts.result_object().clone(),
        ]);
        objects.extend(artifacts.leaf_authority_objects().iter().cloned());
    }
    if let Some(continuation) = continuation {
        objects.push(continuation.successor_snapshot().clone());
    }
    objects.sort_by_key(StoreObjectV1::id);
    objects.dedup_by_key(|object| object.id());
    Ok(AtomicGenerationPublicationV1::new_from_object_superset(
        generation,
        Some(current_head.id()),
        objects,
        idempotency,
    )?)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingReconciliationReadReleaseV1 {
    run_id: RunIdV1,
    execution_boundary_commitment: [u8; 32],
    deadline: u64,
    read_plan: EffectReconciliationReadPlanV1,
}

fn decode_effect_reconciliation_outcome(
    publication: StorePublicationOutcomeV1,
    phase: ReconciliationPublicationPhaseV1,
    expected_request: ActionRequestIdV1,
    expected_intent: EffectIntentIdV1,
    expected_meaning: [u8; 32],
    begin_read_release: Option<PendingReconciliationReadReleaseV1>,
) -> Result<ActiveStoreEffectReconciliationOutcomeV1, ExecutionStoreErrorV1> {
    let replayed = matches!(&publication, StorePublicationOutcomeV1::Replayed { .. });
    let result = publication.result();
    if result.schema_id() != execution_schema_id(phase.result_schema())? {
        return Err(ExecutionStoreErrorV1::InvalidEffectReconciliationResult);
    }
    let CborValue::Array(fields) = result.value() else {
        return Err(ExecutionStoreErrorV1::InvalidEffectReconciliationResult);
    };
    let [
        CborValue::Text(domain),
        request,
        intent,
        control_head,
        control_revision,
        CborValue::Unsigned(classification),
        meaning,
    ] = fields.as_slice()
    else {
        return Err(ExecutionStoreErrorV1::InvalidEffectReconciliationResult);
    };
    if domain != phase.result_domain()
        || exact_digest(request) != Some(*expected_request.as_bytes())
        || exact_digest(intent) != Some(*expected_intent.as_bytes())
        || exact_digest(meaning) != Some(expected_meaning)
    {
        return Err(ExecutionStoreErrorV1::InvalidEffectReconciliationResult);
    }
    let control_head = EffectIntentControlTokenV1::new(HomeTokenV1::new(
        exact_digest(control_head)
            .ok_or(ExecutionStoreErrorV1::InvalidEffectReconciliationResult)?,
    ));
    let read_release = (!replayed)
        .then_some(begin_read_release)
        .flatten()
        .map(|pending| ReconciliationReadReleaseV1 {
            read: SealedReconciliationReadV1 {
                binding: ReconciliationReadBindingV1 {
                    run_id: pending.run_id,
                    execution_boundary_commitment: pending.execution_boundary_commitment,
                    deadline: pending.deadline,
                    intent: expected_intent,
                    control_head,
                    read_plan: pending.read_plan,
                },
            },
        });
    Ok(ActiveStoreEffectReconciliationOutcomeV1 {
        store_head: publication.head().clone(),
        intent: EffectIntentIdV1::from_bytes(
            exact_digest(intent).ok_or(ExecutionStoreErrorV1::InvalidEffectReconciliationResult)?,
        )?,
        control_head,
        control_revision: EffectIntentControlTokenV1::new(HomeTokenV1::new(
            exact_digest(control_revision)
                .ok_or(ExecutionStoreErrorV1::InvalidEffectReconciliationResult)?,
        )),
        classification: parse_remote_classification_store_tag(*classification)?,
        replayed,
        read_release,
    })
}

fn effect_withdrawal_occurrence_object(
    request: &CanonicalExecutionActionRequestV1,
    request_object_id: StoreObjectIdV1,
    authority_basis_object_id: StoreObjectIdV1,
    current_carrier_id: StoreObjectIdV1,
    meaning_digest: [u8; 32],
) -> Result<StoreObjectV1, ExecutionStoreErrorV1> {
    let mut references = vec![
        request_object_id,
        authority_basis_object_id,
        current_carrier_id,
    ];
    references.sort_unstable();
    references.dedup();
    Ok(StoreObjectV1::new(
        execution_schema_id("maestro.vnext.effect-withdrawal-occurrence-schema.v1")?,
        CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-withdrawal-occurrence.v1")?,
            bytes(request.request_id().as_bytes()),
            bytes(current_carrier_id.as_bytes()),
            bytes(&meaning_digest),
            CborValue::Unsigned(0),
        ]),
        references,
    )?)
}

fn effect_withdrawal_authorized_carrier_value(
    withdrawal: &EffectWithdrawalV1,
    authority: &ExecutionAuthorityV1,
) -> Result<CborValue, ExecutionStoreErrorV1> {
    Ok(CborValue::Array(vec![
        CborValue::text(EFFECT_WITHDRAWAL_AUTHORIZED_CARRIER_DOMAIN_V1)?,
        withdrawal.persistence_carrier_value()?,
        execution_authority_value(authority)?,
    ]))
}

fn decode_effect_withdrawal_authorized_carrier(
    value: &CborValue,
) -> Result<(EffectWithdrawalV1, ExecutionAuthorityV1), ExecutionStoreErrorV1> {
    let CborValue::Array(fields) = value else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    };
    let [CborValue::Text(domain), withdrawal, authority] = fields.as_slice() else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    };
    if domain != EFFECT_WITHDRAWAL_AUTHORIZED_CARRIER_DOMAIN_V1 {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    }
    let withdrawal = EffectWithdrawalV1::from_persistence_carrier_value(withdrawal)?;
    let authority = decode_execution_authority_value(authority)?;
    Ok((withdrawal, authority))
}

fn effect_control_authorized_carrier_value(
    domain: &str,
    control_need: &EffectControlTransitionNeedV1,
    authority: &ExecutionAuthorityV1,
) -> Result<CborValue, ExecutionStoreErrorV1> {
    Ok(CborValue::Array(vec![
        CborValue::text(domain)?,
        control_need.canonical_value()?,
        execution_authority_value(authority)?,
    ]))
}

fn decode_effect_control_authorized_carrier(
    value: &CborValue,
    expected_domain: &str,
) -> Result<(EffectControlTransitionNeedV1, ExecutionAuthorityV1), ExecutionStoreErrorV1> {
    let CborValue::Array(fields) = value else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    };
    let [CborValue::Text(domain), control_need, authority] = fields.as_slice() else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    };
    if domain != expected_domain {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    }
    Ok((
        EffectControlTransitionNeedV1::from_canonical_value(control_need)?,
        decode_execution_authority_value(authority)?,
    ))
}

#[expect(
    clippy::too_many_arguments,
    reason = "the health publication binds the complete old and successor control closure"
)]
fn build_effect_health_publication(
    domain: crate::domain::persistence::StoreDomainV1,
    current_head: &StoreHeadV1,
    current_generation: &StoreGenerationV1,
    active_objects: &[StoreObjectV1],
    request: &CanonicalExecutionActionRequestV1,
    authority: &ExecutionAuthorityV1,
    admission: &AdmittedRepositoryActionV1,
    request_object: StoreObjectV1,
    snapshot: &ActiveStoreEffectSnapshotV1,
    control_need: &EffectControlTransitionNeedV1,
    recovered_dispatch: Option<&EffectDispatchTerminalCandidateV1>,
    recovery_occurrence: Option<StoreObjectV1>,
    candidate_revision: &EffectIntentControlRevisionV1,
    candidate_head: &EffectIntentControlHeadV1,
    meaning_digest: [u8; 32],
) -> Result<AtomicGenerationPublicationV1, ExecutionStoreErrorV1> {
    if candidate_revision.intent() != snapshot.intent.id()
        || candidate_revision.id() == snapshot.control_revision.id()
        || candidate_head.intent() != snapshot.intent.id()
        || candidate_head.revision() != candidate_revision.id()
        || candidate_head.writer_term() != snapshot.writer_term.id()
        || control_need.action_request_id() != request.request_id()
        || recovered_dispatch.is_some() != recovery_occurrence.is_some()
    {
        return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
    }
    let current_index = load_control_index(active_objects)?;
    let current_entry = current_index
        .entries
        .iter()
        .find(|entry| entry.intent == snapshot.intent.id())
        .ok_or(ExecutionStoreErrorV1::MissingIntentControlSelector)?;
    let current_head_object = active_objects
        .iter()
        .find(|object| object.id() == current_entry.control_head_object_id)
        .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?;
    let intent_object = exact_referenced_schema_object(
        current_head_object,
        active_objects,
        &[execution_schema_id(
            "maestro.vnext.effect-intent-schema.v1",
        )?],
    )?;
    let current_revision_object = exact_referenced_schema_object(
        current_head_object,
        active_objects,
        &[execution_schema_id(
            "maestro.vnext.effect-intent-control-revision-schema.v1",
        )?],
    )?;
    let writer_object = exact_referenced_schema_object(
        current_head_object,
        active_objects,
        &[execution_schema_id(
            "maestro.vnext.effect-intent-control-writer-term-schema.v1",
        )?],
    )?;
    let prior_dispatch_object = exact_referenced_schema_object(
        current_head_object,
        active_objects,
        &[
            execution_schema_id("maestro.vnext.effect-dispatch-reservation-carrier-schema.v1")?,
            execution_schema_id("maestro.vnext.effect-dispatch-seal-carrier-schema.v1")?,
            execution_schema_id("maestro.vnext.effect-dispatch-terminal-carrier-schema.v1")?,
        ],
    )?;
    let reconciliation_object = optional_referenced_schema_object(
        current_head_object,
        active_objects,
        &[
            execution_schema_id("maestro.vnext.effect-reconciliation-begin-carrier-schema.v1")?,
            execution_schema_id("maestro.vnext.effect-reconciliation-read-carrier-schema.v1")?,
            execution_schema_id("maestro.vnext.effect-reconciliation-terminal-carrier-schema.v1")?,
        ],
    )?;
    let withdrawal_object = optional_referenced_schema_object(
        current_head_object,
        active_objects,
        &[execution_schema_id(
            "maestro.vnext.effect-withdrawal-carrier-schema.v1",
        )?],
    )?;
    let recovered_dispatch_carrier = recovered_dispatch
        .map(|candidate| {
            if candidate.dispatch().attempt().id() != snapshot.dispatch.attempt().id()
                || candidate.classification() != super::withdrawal::RemoteClassificationV1::InDoubt
                || candidate.control_need() != control_need
                || candidate_revision.live_attempt().is_some()
                || candidate_revision.live_dispatch()
                    != super::withdrawal::EffectIntentLiveDispatchV1::None
                || !candidate_revision.runs_closed()
            {
                return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
            }
            let occurrence = recovery_occurrence
                .as_ref()
                .ok_or(ExecutionStoreErrorV1::PublicationBindingMismatch)?;
            let mut references = vec![
                prior_dispatch_object.id(),
                current_head_object.id(),
                request_object.id(),
                admission.basis_object().id(),
                occurrence.id(),
            ];
            references.sort_unstable();
            references.dedup();
            Ok(StoreObjectV1::new(
                execution_schema_id("maestro.vnext.effect-dispatch-terminal-carrier-schema.v1")?,
                effect_dispatch_authorized_carrier_value(
                    candidate.persistence_carrier_value()?,
                    authority,
                )?,
                references,
            )?)
        })
        .transpose()?;
    let mut carrier_references = vec![
        current_head_object.id(),
        request_object.id(),
        admission.basis_object().id(),
    ];
    carrier_references.extend(recovered_dispatch_carrier.iter().map(StoreObjectV1::id));
    carrier_references.extend(recovery_occurrence.iter().map(StoreObjectV1::id));
    carrier_references.sort_unstable();
    carrier_references.dedup();
    let carrier_object = StoreObjectV1::new(
        execution_schema_id("maestro.vnext.effect-intent-health-carrier-schema.v1")?,
        effect_control_authorized_carrier_value(
            EFFECT_HEALTH_AUTHORIZED_CARRIER_DOMAIN_V1,
            control_need,
            authority,
        )?,
        carrier_references,
    )?;
    let mut revision_references = vec![current_revision_object.id(), carrier_object.id()];
    revision_references.extend(recovery_occurrence.iter().map(StoreObjectV1::id));
    revision_references.sort_unstable();
    let revision_object = StoreObjectV1::new(
        execution_schema_id("maestro.vnext.effect-intent-control-revision-schema.v1")?,
        candidate_revision.canonical_value()?,
        revision_references,
    )?;
    let mut head_references = vec![
        intent_object.id(),
        revision_object.id(),
        writer_object.id(),
        recovered_dispatch_carrier
            .as_ref()
            .unwrap_or(prior_dispatch_object)
            .id(),
        carrier_object.id(),
    ];
    head_references.extend(reconciliation_object.iter().map(|object| object.id()));
    head_references.extend(withdrawal_object.iter().map(|object| object.id()));
    head_references.sort_unstable();
    head_references.dedup();
    let head_object = StoreObjectV1::new(
        execution_schema_id("maestro.vnext.effect-intent-control-head-schema.v1")?,
        candidate_head.canonical_value(),
        head_references,
    )?;
    let mut next_entries = current_index.entries.clone();
    let selected = next_entries
        .iter_mut()
        .find(|entry| entry.intent == snapshot.intent.id())
        .ok_or(ExecutionStoreErrorV1::MissingIntentControlSelector)?;
    selected.control_head = candidate_head.id();
    selected.control_head_object_id = head_object.id();
    let index_object = build_control_index_object(&next_entries)?;
    let mut produced_objects = vec![
        carrier_object.clone(),
        revision_object.clone(),
        head_object.clone(),
    ];
    produced_objects.extend(recovered_dispatch_carrier.iter().cloned());
    produced_objects.extend(recovery_occurrence.iter().cloned());
    let artifacts = admission.issue_committed_artifacts(&request_object, &produced_objects)?;
    let mut result_references = vec![
        artifacts.result_object().id(),
        carrier_object.id(),
        revision_object.id(),
        head_object.id(),
    ];
    result_references.extend(recovered_dispatch_carrier.iter().map(StoreObjectV1::id));
    result_references.extend(recovery_occurrence.iter().map(StoreObjectV1::id));
    result_references.sort_unstable();
    result_references.dedup();
    let result_object = StoreObjectV1::new(
        execution_schema_id("maestro.vnext.effect-health-result-schema.v1")?,
        CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-health-result.v1")?,
            bytes(request.request_id().as_bytes()),
            bytes(snapshot.intent.id().as_bytes()),
            bytes(candidate_head.id().as_bytes()),
            bytes(candidate_revision.id().as_bytes()),
            CborValue::Unsigned(effect_control_health_tag(candidate_revision.health())),
            bytes(&meaning_digest),
        ]),
        result_references,
    )?;
    let mut roots = current_generation.roots().to_vec();
    replace_required_root(&mut roots, current_index.object.id(), index_object.id())?;
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
    roots.push(result_object.id());
    roots.sort_unstable();
    roots.dedup();
    let generation = StoreGenerationV1::new(
        domain,
        current_generation
            .ordinal()
            .checked_add(1)
            .ok_or(ExecutionStoreErrorV1::GenerationOverflow)?,
        Some(current_generation.id()),
        current_generation.contract_root_id(),
        current_generation.compatibility().clone(),
        roots,
    )?;
    let idempotency = StoreIdempotencyV1::new(
        EFFECT_HEALTH_IDEMPOTENCY_NAMESPACE_V1,
        *request.idempotency_key_id().as_bytes(),
        meaning_digest,
        result_object.id(),
    )?;
    let mut objects = active_objects.to_vec();
    objects.extend(produced_objects);
    objects.extend([
        index_object,
        request_object,
        admission.basis_object().clone(),
        admission.successor_snapshot().clone(),
        admission.successor_capacity_root().clone(),
        admission.capacity_debit().clone(),
        artifacts.receipt_object().clone(),
        artifacts.result_object().clone(),
        result_object,
    ]);
    objects.extend(artifacts.leaf_authority_objects().iter().cloned());
    objects.sort_by_key(StoreObjectV1::id);
    objects.dedup_by_key(|object| object.id());
    Ok(AtomicGenerationPublicationV1::new_from_object_superset(
        generation,
        Some(current_head.id()),
        objects,
        idempotency,
    )?)
}

fn decode_effect_health_outcome(
    publication: StorePublicationOutcomeV1,
    expected_request: ActionRequestIdV1,
    expected_intent: EffectIntentIdV1,
    expected_meaning: [u8; 32],
) -> Result<ActiveStoreEffectHealthOutcomeV1, ExecutionStoreErrorV1> {
    let replayed = matches!(&publication, StorePublicationOutcomeV1::Replayed { .. });
    let result = publication.result();
    if result.schema_id() != execution_schema_id("maestro.vnext.effect-health-result-schema.v1")? {
        return Err(ExecutionStoreErrorV1::InvalidEffectHealthResult);
    }
    let CborValue::Array(fields) = result.value() else {
        return Err(ExecutionStoreErrorV1::InvalidEffectHealthResult);
    };
    let [
        CborValue::Text(domain),
        request,
        intent,
        control_head,
        control_revision,
        CborValue::Unsigned(health),
        meaning,
    ] = fields.as_slice()
    else {
        return Err(ExecutionStoreErrorV1::InvalidEffectHealthResult);
    };
    if domain != "maestro.vnext.effect-health-result.v1"
        || exact_digest(request) != Some(*expected_request.as_bytes())
        || exact_digest(intent) != Some(*expected_intent.as_bytes())
        || exact_digest(meaning) != Some(expected_meaning)
    {
        return Err(ExecutionStoreErrorV1::InvalidEffectHealthResult);
    }
    let token = |value: &CborValue| {
        exact_digest(value)
            .map(|digest| EffectIntentControlTokenV1::new(HomeTokenV1::new(digest)))
            .ok_or(ExecutionStoreErrorV1::InvalidEffectHealthResult)
    };
    Ok(ActiveStoreEffectHealthOutcomeV1 {
        store_head: publication.head().clone(),
        intent: expected_intent,
        control_head: token(control_head)?,
        control_revision: token(control_revision)?,
        health: parse_effect_control_health_tag(*health)?,
        replayed,
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "the atomic withdrawal publication binds every Store, authority, control, and result participant"
)]
fn build_effect_withdrawal_publication(
    domain: crate::domain::persistence::StoreDomainV1,
    current_head: &StoreHeadV1,
    current_generation: &StoreGenerationV1,
    active_objects: &[StoreObjectV1],
    request: &CanonicalExecutionActionRequestV1,
    authority: &ExecutionAuthorityV1,
    admission: &AdmittedRepositoryActionV1,
    request_object: StoreObjectV1,
    occurrence_object: StoreObjectV1,
    snapshot: &ActiveStoreEffectSnapshotV1,
    withdrawal: &EffectWithdrawalV1,
    next_revision: &EffectIntentControlRevisionV1,
    next_head: &EffectIntentControlHeadV1,
    meaning_digest: [u8; 32],
) -> Result<AtomicGenerationPublicationV1, ExecutionStoreErrorV1> {
    if withdrawal.intent_id() != snapshot.intent.id()
        || next_revision.intent() != snapshot.intent.id()
        || next_revision.classification() != super::withdrawal::RemoteClassificationV1::Cancelled
        || next_revision.live_attempt().is_some()
        || next_revision.live_dispatch() != super::withdrawal::EffectIntentLiveDispatchV1::None
        || !next_revision.runs_closed()
        || next_head.intent() != snapshot.intent.id()
        || next_head.revision() != next_revision.id()
        || next_head.writer_term() != snapshot.writer_term.id()
        || withdrawal.provider_io_operations() != 0
        || withdrawal.creates_attempt()
        || withdrawal.creates_run()
    {
        return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
    }
    let current_index = load_control_index(active_objects)?;
    let current_entry = current_index
        .entries
        .iter()
        .find(|entry| entry.intent == snapshot.intent.id())
        .ok_or(ExecutionStoreErrorV1::MissingIntentControlSelector)?;
    if current_index.object.id()
        != snapshot
            .state_binding
            .control_index_object_id()
            .ok_or(ExecutionStoreErrorV1::PublicationBindingMismatch)?
    {
        return Err(ExecutionStoreErrorV1::StaleExpectedStoreState);
    }
    let current_head_object = active_objects
        .iter()
        .find(|object| object.id() == current_entry.control_head_object_id)
        .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?;
    let intent_object = exact_referenced_schema_object(
        current_head_object,
        active_objects,
        &[execution_schema_id(
            "maestro.vnext.effect-intent-schema.v1",
        )?],
    )?;
    let current_revision_object = exact_referenced_schema_object(
        current_head_object,
        active_objects,
        &[execution_schema_id(
            "maestro.vnext.effect-intent-control-revision-schema.v1",
        )?],
    )?;
    let writer_object = exact_referenced_schema_object(
        current_head_object,
        active_objects,
        &[execution_schema_id(
            "maestro.vnext.effect-intent-control-writer-term-schema.v1",
        )?],
    )?;
    let dispatch_object = exact_referenced_schema_object(
        current_head_object,
        active_objects,
        &[execution_schema_id(
            "maestro.vnext.effect-dispatch-terminal-carrier-schema.v1",
        )?],
    )?;
    let reconciliation_object = optional_referenced_schema_object(
        current_head_object,
        active_objects,
        &[execution_schema_id(
            "maestro.vnext.effect-reconciliation-terminal-carrier-schema.v1",
        )?],
    )?;
    let mut carrier_references = vec![
        current_head_object.id(),
        request_object.id(),
        admission.basis_object().id(),
        occurrence_object.id(),
    ];
    carrier_references.sort_unstable();
    carrier_references.dedup();
    let carrier_object = StoreObjectV1::new(
        execution_schema_id("maestro.vnext.effect-withdrawal-carrier-schema.v1")?,
        effect_withdrawal_authorized_carrier_value(withdrawal, authority)?,
        carrier_references,
    )?;
    let mut revision_references = vec![
        current_revision_object.id(),
        carrier_object.id(),
        occurrence_object.id(),
    ];
    revision_references.sort_unstable();
    let revision_object = StoreObjectV1::new(
        execution_schema_id("maestro.vnext.effect-intent-control-revision-schema.v1")?,
        next_revision.canonical_value()?,
        revision_references,
    )?;
    let mut head_references = vec![
        intent_object.id(),
        revision_object.id(),
        writer_object.id(),
        dispatch_object.id(),
        carrier_object.id(),
    ];
    head_references.extend(reconciliation_object.iter().map(|object| object.id()));
    head_references.sort_unstable();
    head_references.dedup();
    let head_object = StoreObjectV1::new(
        execution_schema_id("maestro.vnext.effect-intent-control-head-schema.v1")?,
        next_head.canonical_value(),
        head_references,
    )?;
    let mut next_entries = current_index.entries.clone();
    let selected = next_entries
        .iter_mut()
        .find(|entry| entry.intent == snapshot.intent.id())
        .ok_or(ExecutionStoreErrorV1::MissingIntentControlSelector)?;
    selected.control_head = next_head.id();
    selected.control_head_object_id = head_object.id();
    let index_object = build_control_index_object(&next_entries)?;
    let produced_objects = vec![
        occurrence_object.clone(),
        carrier_object.clone(),
        revision_object.clone(),
        head_object.clone(),
    ];
    let artifacts = admission.issue_committed_artifacts(&request_object, &produced_objects)?;
    let mut result_references = vec![
        artifacts.result_object().id(),
        occurrence_object.id(),
        carrier_object.id(),
        revision_object.id(),
        head_object.id(),
    ];
    result_references.sort_unstable();
    result_references.dedup();
    let result_object = StoreObjectV1::new(
        execution_schema_id("maestro.vnext.effect-withdrawal-result-schema.v1")?,
        CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-withdrawal-result.v1")?,
            bytes(request.request_id().as_bytes()),
            bytes(snapshot.intent.id().as_bytes()),
            bytes(next_head.id().as_bytes()),
            bytes(next_revision.id().as_bytes()),
            bytes(&meaning_digest),
            CborValue::Unsigned(0),
        ]),
        result_references,
    )?;
    let mut roots = current_generation.roots().to_vec();
    replace_required_root(&mut roots, current_index.object.id(), index_object.id())?;
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
    roots.push(result_object.id());
    roots.sort_unstable();
    roots.dedup();
    let generation = StoreGenerationV1::new(
        domain,
        current_generation
            .ordinal()
            .checked_add(1)
            .ok_or(ExecutionStoreErrorV1::GenerationOverflow)?,
        Some(current_generation.id()),
        current_generation.contract_root_id(),
        current_generation.compatibility().clone(),
        roots,
    )?;
    let idempotency = StoreIdempotencyV1::new(
        EFFECT_WITHDRAWAL_IDEMPOTENCY_NAMESPACE_V1,
        *request.idempotency_key_id().as_bytes(),
        meaning_digest,
        result_object.id(),
    )?;
    let mut objects = active_objects.to_vec();
    objects.extend(produced_objects);
    objects.extend([
        index_object,
        request_object,
        admission.basis_object().clone(),
        admission.successor_snapshot().clone(),
        admission.successor_capacity_root().clone(),
        admission.capacity_debit().clone(),
        artifacts.receipt_object().clone(),
        artifacts.result_object().clone(),
        result_object,
    ]);
    objects.extend(artifacts.leaf_authority_objects().iter().cloned());
    objects.sort_by_key(StoreObjectV1::id);
    objects.dedup_by_key(|object| object.id());
    Ok(AtomicGenerationPublicationV1::new_from_object_superset(
        generation,
        Some(current_head.id()),
        objects,
        idempotency,
    )?)
}

fn decode_effect_withdrawal_outcome(
    publication: StorePublicationOutcomeV1,
    expected_request: ActionRequestIdV1,
    expected_intent: EffectIntentIdV1,
    expected_meaning: [u8; 32],
) -> Result<ActiveStoreEffectWithdrawalOutcomeV1, ExecutionStoreErrorV1> {
    let replayed = matches!(&publication, StorePublicationOutcomeV1::Replayed { .. });
    let result = publication.result();
    if result.schema_id()
        != execution_schema_id("maestro.vnext.effect-withdrawal-result-schema.v1")?
    {
        return Err(ExecutionStoreErrorV1::InvalidEffectWithdrawalResult);
    }
    let CborValue::Array(fields) = result.value() else {
        return Err(ExecutionStoreErrorV1::InvalidEffectWithdrawalResult);
    };
    let [
        CborValue::Text(domain),
        request,
        intent,
        control_head,
        control_revision,
        meaning,
        CborValue::Unsigned(provider_io),
    ] = fields.as_slice()
    else {
        return Err(ExecutionStoreErrorV1::InvalidEffectWithdrawalResult);
    };
    if domain != "maestro.vnext.effect-withdrawal-result.v1"
        || exact_digest(request) != Some(*expected_request.as_bytes())
        || exact_digest(intent) != Some(*expected_intent.as_bytes())
        || exact_digest(meaning) != Some(expected_meaning)
        || *provider_io != 0
    {
        return Err(ExecutionStoreErrorV1::InvalidEffectWithdrawalResult);
    }
    Ok(ActiveStoreEffectWithdrawalOutcomeV1 {
        store_head: publication.head().clone(),
        intent: expected_intent,
        control_head: EffectIntentControlTokenV1::new(HomeTokenV1::new(
            exact_digest(control_head)
                .ok_or(ExecutionStoreErrorV1::InvalidEffectWithdrawalResult)?,
        )),
        control_revision: EffectIntentControlTokenV1::new(HomeTokenV1::new(
            exact_digest(control_revision)
                .ok_or(ExecutionStoreErrorV1::InvalidEffectWithdrawalResult)?,
        )),
        replayed,
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "the atomic redispatch publication binds the complete existing Intent and fresh dispatch product"
)]
fn build_effect_redispatch_publication(
    domain: crate::domain::persistence::StoreDomainV1,
    current_head: &StoreHeadV1,
    current_generation: &StoreGenerationV1,
    active_objects: &[StoreObjectV1],
    request: &CanonicalExecutionActionRequestV1,
    authority: &ExecutionAuthorityV1,
    admission: &AdmittedRepositoryActionV1,
    request_object: StoreObjectV1,
    snapshot: &ActiveStoreEffectSnapshotV1,
    prepared: &PreparedEffectDispatchV1,
    reserved_revision: &EffectIntentControlRevisionV1,
    reserved_head: &EffectIntentControlHeadV1,
    meaning_digest: [u8; 32],
) -> Result<AtomicGenerationPublicationV1, ExecutionStoreErrorV1> {
    if !matches!(
        prepared.control_need(),
        EffectControlTransitionNeedV1::RedispatchConclusiveNotApplied { .. }
    ) || prepared.dispatch().attempt().effect_intent_id() != snapshot.intent.id()
        || reserved_revision.intent() != snapshot.intent.id()
        || reserved_head.intent() != snapshot.intent.id()
        || reserved_head.revision() != reserved_revision.id()
        || reserved_head.writer_term() != snapshot.writer_term.id()
    {
        return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
    }
    let current_index = load_control_index(active_objects)?;
    if current_index.object.id()
        != snapshot
            .state_binding
            .control_index_object_id()
            .ok_or(ExecutionStoreErrorV1::PublicationBindingMismatch)?
    {
        return Err(ExecutionStoreErrorV1::StaleExpectedStoreState);
    }
    let current_entry = current_index
        .entries
        .iter()
        .find(|entry| entry.intent == snapshot.intent.id())
        .ok_or(ExecutionStoreErrorV1::MissingIntentControlSelector)?;
    let current_head_object = active_objects
        .iter()
        .find(|object| object.id() == current_entry.control_head_object_id)
        .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?;
    let intent_object = exact_referenced_schema_object(
        current_head_object,
        active_objects,
        &[execution_schema_id(
            "maestro.vnext.effect-intent-schema.v1",
        )?],
    )?;
    let current_revision_object = exact_referenced_schema_object(
        current_head_object,
        active_objects,
        &[execution_schema_id(
            "maestro.vnext.effect-intent-control-revision-schema.v1",
        )?],
    )?;
    let writer_object = exact_referenced_schema_object(
        current_head_object,
        active_objects,
        &[execution_schema_id(
            "maestro.vnext.effect-intent-control-writer-term-schema.v1",
        )?],
    )?;
    let prior_dispatch_object = exact_referenced_schema_object(
        current_head_object,
        active_objects,
        &[execution_schema_id(
            "maestro.vnext.effect-dispatch-terminal-carrier-schema.v1",
        )?],
    )?;
    let mut carrier_references = vec![
        prior_dispatch_object.id(),
        current_head_object.id(),
        request_object.id(),
        admission.basis_object().id(),
    ];
    carrier_references.sort_unstable();
    let carrier_object = StoreObjectV1::new(
        execution_schema_id("maestro.vnext.effect-dispatch-reservation-carrier-schema.v1")?,
        effect_dispatch_authorized_carrier_value(prepared.persistence_carrier_value()?, authority)?,
        carrier_references,
    )?;
    let mut revision_references = vec![current_revision_object.id(), carrier_object.id()];
    revision_references.sort_unstable();
    let revision_object = StoreObjectV1::new(
        execution_schema_id("maestro.vnext.effect-intent-control-revision-schema.v1")?,
        reserved_revision.canonical_value()?,
        revision_references,
    )?;
    let mut head_references = vec![
        intent_object.id(),
        revision_object.id(),
        writer_object.id(),
        carrier_object.id(),
    ];
    head_references.sort_unstable();
    let head_object = StoreObjectV1::new(
        execution_schema_id("maestro.vnext.effect-intent-control-head-schema.v1")?,
        reserved_head.canonical_value(),
        head_references,
    )?;
    let mut next_entries = current_index.entries.clone();
    let selected = next_entries
        .iter_mut()
        .find(|entry| entry.intent == snapshot.intent.id())
        .ok_or(ExecutionStoreErrorV1::MissingIntentControlSelector)?;
    selected.control_head = reserved_head.id();
    selected.control_head_object_id = head_object.id();
    let index_object = build_control_index_object(&next_entries)?;
    let produced_objects = vec![
        carrier_object.clone(),
        revision_object.clone(),
        head_object.clone(),
    ];
    let artifacts = admission.issue_committed_artifacts(&request_object, &produced_objects)?;
    let mut result_references = vec![
        artifacts.result_object().id(),
        carrier_object.id(),
        revision_object.id(),
        head_object.id(),
    ];
    result_references.sort_unstable();
    let result_object = StoreObjectV1::new(
        execution_schema_id("maestro.vnext.effect-redispatch-result-schema.v1")?,
        CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-redispatch-result.v1")?,
            bytes(request.request_id().as_bytes()),
            bytes(snapshot.intent.id().as_bytes()),
            bytes(reserved_head.id().as_bytes()),
            bytes(reserved_revision.id().as_bytes()),
            bytes(prepared.dispatch().attempt().id().as_bytes()),
            bytes(&meaning_digest),
        ]),
        result_references,
    )?;
    let mut roots = current_generation.roots().to_vec();
    replace_required_root(&mut roots, current_index.object.id(), index_object.id())?;
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
    roots.push(result_object.id());
    roots.sort_unstable();
    roots.dedup();
    let generation = StoreGenerationV1::new(
        domain,
        current_generation
            .ordinal()
            .checked_add(1)
            .ok_or(ExecutionStoreErrorV1::GenerationOverflow)?,
        Some(current_generation.id()),
        current_generation.contract_root_id(),
        current_generation.compatibility().clone(),
        roots,
    )?;
    let idempotency = StoreIdempotencyV1::new(
        EFFECT_REDISPATCH_IDEMPOTENCY_NAMESPACE_V1,
        *request.idempotency_key_id().as_bytes(),
        meaning_digest,
        result_object.id(),
    )?;
    let mut objects = active_objects.to_vec();
    objects.extend(produced_objects);
    objects.extend([
        index_object,
        request_object,
        admission.basis_object().clone(),
        admission.successor_snapshot().clone(),
        admission.successor_capacity_root().clone(),
        admission.capacity_debit().clone(),
        artifacts.receipt_object().clone(),
        artifacts.result_object().clone(),
        result_object,
    ]);
    objects.extend(artifacts.leaf_authority_objects().iter().cloned());
    objects.sort_by_key(StoreObjectV1::id);
    objects.dedup_by_key(|object| object.id());
    Ok(AtomicGenerationPublicationV1::new_from_object_superset(
        generation,
        Some(current_head.id()),
        objects,
        idempotency,
    )?)
}

fn decode_effect_redispatch_outcome(
    publication: StorePublicationOutcomeV1,
    expected_request: ActionRequestIdV1,
    expected_intent: EffectIntentIdV1,
    expected_meaning: [u8; 32],
) -> Result<ActiveStoreEffectRedispatchOutcomeV1, ExecutionStoreErrorV1> {
    let replayed = matches!(&publication, StorePublicationOutcomeV1::Replayed { .. });
    let result = publication.result();
    if result.schema_id()
        != execution_schema_id("maestro.vnext.effect-redispatch-result-schema.v1")?
    {
        return Err(ExecutionStoreErrorV1::InvalidEffectOriginationResult);
    }
    let CborValue::Array(fields) = result.value() else {
        return Err(ExecutionStoreErrorV1::InvalidEffectOriginationResult);
    };
    let [
        CborValue::Text(domain),
        request,
        intent,
        control_head,
        control_revision,
        dispatch_attempt,
        meaning,
    ] = fields.as_slice()
    else {
        return Err(ExecutionStoreErrorV1::InvalidEffectOriginationResult);
    };
    if domain != "maestro.vnext.effect-redispatch-result.v1"
        || exact_digest(request) != Some(*expected_request.as_bytes())
        || exact_digest(intent) != Some(*expected_intent.as_bytes())
        || exact_digest(meaning) != Some(expected_meaning)
    {
        return Err(ExecutionStoreErrorV1::InvalidEffectOriginationResult);
    }
    Ok(ActiveStoreEffectRedispatchOutcomeV1 {
        store_head: publication.head().clone(),
        intent: expected_intent,
        control_head: EffectIntentControlTokenV1::new(HomeTokenV1::new(
            exact_digest(control_head)
                .ok_or(ExecutionStoreErrorV1::InvalidEffectOriginationResult)?,
        )),
        control_revision: EffectIntentControlTokenV1::new(HomeTokenV1::new(
            exact_digest(control_revision)
                .ok_or(ExecutionStoreErrorV1::InvalidEffectOriginationResult)?,
        )),
        dispatch_attempt: super::runtime::DispatchAttemptIdV1::from_bytes(
            exact_digest(dispatch_attempt)
                .ok_or(ExecutionStoreErrorV1::InvalidEffectOriginationResult)?,
        )?,
        replayed,
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "the writer handoff publication binds the complete old and successor control closure"
)]
fn build_effect_writer_handoff_publication(
    domain: crate::domain::persistence::StoreDomainV1,
    current_head: &StoreHeadV1,
    current_generation: &StoreGenerationV1,
    active_objects: &[StoreObjectV1],
    request: &CanonicalExecutionActionRequestV1,
    authority: &ExecutionAuthorityV1,
    admission: &AdmittedRepositoryActionV1,
    request_object: StoreObjectV1,
    snapshot: &ActiveStoreEffectSnapshotV1,
    control_need: &EffectControlTransitionNeedV1,
    receipt: SameHomeWriterFencingReceiptV1,
    successor_writer: EffectIntentControlWriterTermV1,
    candidate_revision: &EffectIntentControlRevisionV1,
    candidate_head: &EffectIntentControlHeadV1,
    meaning_digest: [u8; 32],
) -> Result<AtomicGenerationPublicationV1, ExecutionStoreErrorV1> {
    let restores_health =
        snapshot.control_revision.health() == EffectIntentControlHealthV1::RecoveryRequired;
    if candidate_head.intent() != snapshot.intent.id()
        || candidate_head.home() != snapshot.control_head.home()
        || candidate_head.revision() != candidate_revision.id()
        || candidate_head.writer_term() != successor_writer.id()
        || candidate_revision.intent() != snapshot.intent.id()
        || (candidate_revision.health() == EffectIntentControlHealthV1::Healthy)
            != (snapshot.control_revision.health() == EffectIntentControlHealthV1::Healthy
                || restores_health)
        || (candidate_revision.id() != snapshot.control_revision.id()) != restores_health
        || successor_writer.prior_writer_term() != Some(snapshot.writer_term.id())
        || successor_writer.fencing_receipt() != Some(receipt.id())
        || receipt.prior_head() != snapshot.control_head.id()
        || receipt.prior_writer_term() != snapshot.writer_term.id()
        || control_need.action_request_id() != request.request_id()
    {
        return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
    }
    let current_index = load_control_index(active_objects)?;
    let current_entry = current_index
        .entries
        .iter()
        .find(|entry| entry.intent == snapshot.intent.id())
        .ok_or(ExecutionStoreErrorV1::MissingIntentControlSelector)?;
    let current_head_object = active_objects
        .iter()
        .find(|object| object.id() == current_entry.control_head_object_id)
        .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?;
    let intent_object = exact_referenced_schema_object(
        current_head_object,
        active_objects,
        &[execution_schema_id(
            "maestro.vnext.effect-intent-schema.v1",
        )?],
    )?;
    let current_revision_object = exact_referenced_schema_object(
        current_head_object,
        active_objects,
        &[execution_schema_id(
            "maestro.vnext.effect-intent-control-revision-schema.v1",
        )?],
    )?;
    let prior_writer_object = exact_referenced_schema_object(
        current_head_object,
        active_objects,
        &[execution_schema_id(
            "maestro.vnext.effect-intent-control-writer-term-schema.v1",
        )?],
    )?;
    let dispatch_object = exact_referenced_schema_object(
        current_head_object,
        active_objects,
        &[
            execution_schema_id("maestro.vnext.effect-dispatch-reservation-carrier-schema.v1")?,
            execution_schema_id("maestro.vnext.effect-dispatch-seal-carrier-schema.v1")?,
            execution_schema_id("maestro.vnext.effect-dispatch-terminal-carrier-schema.v1")?,
        ],
    )?;
    let reconciliation_object = optional_referenced_schema_object(
        current_head_object,
        active_objects,
        &[
            execution_schema_id("maestro.vnext.effect-reconciliation-begin-carrier-schema.v1")?,
            execution_schema_id("maestro.vnext.effect-reconciliation-read-carrier-schema.v1")?,
            execution_schema_id("maestro.vnext.effect-reconciliation-terminal-carrier-schema.v1")?,
        ],
    )?;
    let withdrawal_object = optional_referenced_schema_object(
        current_head_object,
        active_objects,
        &[execution_schema_id(
            "maestro.vnext.effect-withdrawal-carrier-schema.v1",
        )?],
    )?;
    let mut receipt_references = vec![current_head_object.id(), prior_writer_object.id()];
    receipt_references.sort_unstable();
    let receipt_object = StoreObjectV1::new(
        execution_schema_id("maestro.vnext.same-home-writer-fencing-receipt-schema.v1")?,
        receipt.canonical_value()?,
        receipt_references,
    )?;
    let mut writer_references = vec![prior_writer_object.id(), receipt_object.id()];
    writer_references.sort_unstable();
    let writer_object = StoreObjectV1::new(
        execution_schema_id("maestro.vnext.effect-intent-control-writer-term-schema.v1")?,
        successor_writer.canonical_value(),
        writer_references,
    )?;
    let mut carrier_references = vec![
        current_head_object.id(),
        request_object.id(),
        admission.basis_object().id(),
        receipt_object.id(),
        writer_object.id(),
    ];
    carrier_references.sort_unstable();
    carrier_references.dedup();
    let carrier_object = StoreObjectV1::new(
        execution_schema_id("maestro.vnext.effect-writer-handoff-carrier-schema.v1")?,
        effect_control_authorized_carrier_value(
            EFFECT_WRITER_HANDOFF_AUTHORIZED_CARRIER_DOMAIN_V1,
            control_need,
            authority,
        )?,
        carrier_references,
    )?;
    let revision_object = if restores_health {
        let mut references = vec![current_revision_object.id(), carrier_object.id()];
        references.sort_unstable();
        StoreObjectV1::new(
            execution_schema_id("maestro.vnext.effect-intent-control-revision-schema.v1")?,
            candidate_revision.canonical_value()?,
            references,
        )?
    } else {
        current_revision_object.clone()
    };
    let mut head_references = vec![
        intent_object.id(),
        revision_object.id(),
        writer_object.id(),
        dispatch_object.id(),
        receipt_object.id(),
        carrier_object.id(),
    ];
    head_references.extend(reconciliation_object.iter().map(|object| object.id()));
    head_references.extend(withdrawal_object.iter().map(|object| object.id()));
    head_references.sort_unstable();
    head_references.dedup();
    let head_object = StoreObjectV1::new(
        execution_schema_id("maestro.vnext.effect-intent-control-head-schema.v1")?,
        candidate_head.canonical_value(),
        head_references,
    )?;
    let mut next_entries = current_index.entries.clone();
    let selected = next_entries
        .iter_mut()
        .find(|entry| entry.intent == snapshot.intent.id())
        .ok_or(ExecutionStoreErrorV1::MissingIntentControlSelector)?;
    selected.control_head = candidate_head.id();
    selected.control_head_object_id = head_object.id();
    let index_object = build_control_index_object(&next_entries)?;
    let mut produced_objects = vec![
        receipt_object.clone(),
        writer_object.clone(),
        carrier_object.clone(),
        head_object.clone(),
    ];
    if restores_health {
        produced_objects.push(revision_object.clone());
    }
    let artifacts = admission.issue_committed_artifacts(&request_object, &produced_objects)?;
    let mut result_references = vec![
        artifacts.result_object().id(),
        head_object.id(),
        carrier_object.id(),
        revision_object.id(),
        writer_object.id(),
        receipt_object.id(),
    ];
    result_references.sort_unstable();
    result_references.dedup();
    let result_object = StoreObjectV1::new(
        execution_schema_id("maestro.vnext.effect-writer-handoff-result-schema.v1")?,
        CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-writer-handoff-result.v1")?,
            bytes(request.request_id().as_bytes()),
            bytes(snapshot.intent.id().as_bytes()),
            bytes(candidate_head.id().as_bytes()),
            bytes(candidate_revision.id().as_bytes()),
            bytes(successor_writer.id().as_bytes()),
            bytes(receipt.id().as_bytes()),
            bytes(&meaning_digest),
        ]),
        result_references,
    )?;
    let mut roots = current_generation.roots().to_vec();
    replace_required_root(&mut roots, current_index.object.id(), index_object.id())?;
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
    roots.push(result_object.id());
    roots.sort_unstable();
    roots.dedup();
    let generation = StoreGenerationV1::new(
        domain,
        current_generation
            .ordinal()
            .checked_add(1)
            .ok_or(ExecutionStoreErrorV1::GenerationOverflow)?,
        Some(current_generation.id()),
        current_generation.contract_root_id(),
        current_generation.compatibility().clone(),
        roots,
    )?;
    let idempotency = StoreIdempotencyV1::new(
        EFFECT_WRITER_HANDOFF_IDEMPOTENCY_NAMESPACE_V1,
        *request.idempotency_key_id().as_bytes(),
        meaning_digest,
        result_object.id(),
    )?;
    let mut objects = active_objects.to_vec();
    objects.extend(produced_objects);
    objects.extend([
        index_object,
        request_object,
        admission.basis_object().clone(),
        admission.successor_snapshot().clone(),
        admission.successor_capacity_root().clone(),
        admission.capacity_debit().clone(),
        artifacts.receipt_object().clone(),
        artifacts.result_object().clone(),
        result_object,
    ]);
    objects.extend(artifacts.leaf_authority_objects().iter().cloned());
    objects.sort_by_key(StoreObjectV1::id);
    objects.dedup_by_key(|object| object.id());
    Ok(AtomicGenerationPublicationV1::new_from_object_superset(
        generation,
        Some(current_head.id()),
        objects,
        idempotency,
    )?)
}

fn decode_effect_writer_handoff_outcome(
    publication: StorePublicationOutcomeV1,
    expected_request: ActionRequestIdV1,
    expected_intent: EffectIntentIdV1,
    expected_meaning: [u8; 32],
) -> Result<ActiveStoreEffectWriterHandoffOutcomeV1, ExecutionStoreErrorV1> {
    let replayed = matches!(&publication, StorePublicationOutcomeV1::Replayed { .. });
    let result = publication.result();
    if result.schema_id()
        != execution_schema_id("maestro.vnext.effect-writer-handoff-result-schema.v1")?
    {
        return Err(ExecutionStoreErrorV1::InvalidEffectWriterHandoffResult);
    }
    let CborValue::Array(fields) = result.value() else {
        return Err(ExecutionStoreErrorV1::InvalidEffectWriterHandoffResult);
    };
    let [
        CborValue::Text(domain),
        request,
        intent,
        control_head,
        control_revision,
        writer_term,
        fencing_receipt,
        meaning,
    ] = fields.as_slice()
    else {
        return Err(ExecutionStoreErrorV1::InvalidEffectWriterHandoffResult);
    };
    if domain != "maestro.vnext.effect-writer-handoff-result.v1"
        || exact_digest(request) != Some(*expected_request.as_bytes())
        || exact_digest(intent) != Some(*expected_intent.as_bytes())
        || exact_digest(meaning) != Some(expected_meaning)
    {
        return Err(ExecutionStoreErrorV1::InvalidEffectWriterHandoffResult);
    }
    let token = |value: &CborValue| {
        exact_digest(value)
            .map(|digest| EffectIntentControlTokenV1::new(HomeTokenV1::new(digest)))
            .ok_or(ExecutionStoreErrorV1::InvalidEffectWriterHandoffResult)
    };
    Ok(ActiveStoreEffectWriterHandoffOutcomeV1 {
        store_head: publication.head().clone(),
        intent: expected_intent,
        control_head: token(control_head)?,
        control_revision: token(control_revision)?,
        writer_term: token(writer_term)?,
        fencing_receipt: token(fencing_receipt)?,
        replayed,
    })
}

const fn remote_classification_store_tag(
    classification: super::withdrawal::RemoteClassificationV1,
) -> u64 {
    match classification {
        super::withdrawal::RemoteClassificationV1::Prepared => 1,
        super::withdrawal::RemoteClassificationV1::Dispatching => 2,
        super::withdrawal::RemoteClassificationV1::Pending => 3,
        super::withdrawal::RemoteClassificationV1::InDoubt => 4,
        super::withdrawal::RemoteClassificationV1::ConfirmedApplied => 5,
        super::withdrawal::RemoteClassificationV1::ConfirmedNotApplied => 6,
        super::withdrawal::RemoteClassificationV1::PartiallyApplied => 7,
        super::withdrawal::RemoteClassificationV1::Conflicted => 8,
        super::withdrawal::RemoteClassificationV1::Cancelled => 9,
    }
}

fn parse_remote_classification_store_tag(
    tag: u64,
) -> Result<super::withdrawal::RemoteClassificationV1, ExecutionStoreErrorV1> {
    match tag {
        1 => Ok(super::withdrawal::RemoteClassificationV1::Prepared),
        2 => Ok(super::withdrawal::RemoteClassificationV1::Dispatching),
        3 => Ok(super::withdrawal::RemoteClassificationV1::Pending),
        4 => Ok(super::withdrawal::RemoteClassificationV1::InDoubt),
        5 => Ok(super::withdrawal::RemoteClassificationV1::ConfirmedApplied),
        6 => Ok(super::withdrawal::RemoteClassificationV1::ConfirmedNotApplied),
        7 => Ok(super::withdrawal::RemoteClassificationV1::PartiallyApplied),
        8 => Ok(super::withdrawal::RemoteClassificationV1::Conflicted),
        9 => Ok(super::withdrawal::RemoteClassificationV1::Cancelled),
        _ => Err(ExecutionStoreErrorV1::InvalidEffectTerminalResult),
    }
}

const fn effect_control_health_tag(health: EffectIntentControlHealthV1) -> u64 {
    match health {
        EffectIntentControlHealthV1::Healthy => 1,
        EffectIntentControlHealthV1::RecoveryRequired => 2,
        EffectIntentControlHealthV1::IntegrityBlocked => 3,
    }
}

fn parse_effect_control_health_tag(
    tag: u64,
) -> Result<EffectIntentControlHealthV1, ExecutionStoreErrorV1> {
    match tag {
        1 => Ok(EffectIntentControlHealthV1::Healthy),
        2 => Ok(EffectIntentControlHealthV1::RecoveryRequired),
        3 => Ok(EffectIntentControlHealthV1::IntegrityBlocked),
        _ => Err(ExecutionStoreErrorV1::InvalidEffectHealthResult),
    }
}

fn replace_required_root(
    roots: &mut [StoreObjectIdV1],
    current: StoreObjectIdV1,
    successor: StoreObjectIdV1,
) -> Result<(), ExecutionStoreErrorV1> {
    let positions = roots
        .iter()
        .enumerate()
        .filter_map(|(index, root)| (*root == current).then_some(index))
        .collect::<Vec<_>>();
    let [position] = positions.as_slice() else {
        return Err(ExecutionStoreErrorV1::StaleExpectedStoreState);
    };
    roots[*position] = successor;
    Ok(())
}

fn replace_root_if_present(
    roots: &mut [StoreObjectIdV1],
    current: StoreObjectIdV1,
    successor: StoreObjectIdV1,
) {
    for root in roots.iter_mut().filter(|root| **root == current) {
        *root = successor;
    }
}

fn load_control_index(
    active_objects: &[StoreObjectV1],
) -> Result<ActiveControlIndexV1, ExecutionStoreErrorV1> {
    let schema = execution_schema_id("maestro.vnext.effect-intent-control-index-schema.v1")?;
    let mut candidates = active_objects
        .iter()
        .filter(|object| object.schema_id() == schema);
    let object = candidates
        .next()
        .ok_or(ExecutionStoreErrorV1::MissingIntentControlIndex)?;
    if candidates.next().is_some() {
        return Err(ExecutionStoreErrorV1::AmbiguousIntentControlIndex);
    }
    let CborValue::Array(fields) = object.value() else {
        return Err(ExecutionStoreErrorV1::InvalidIntentControlIndex);
    };
    let [CborValue::Text(domain), CborValue::Array(rows)] = fields.as_slice() else {
        return Err(ExecutionStoreErrorV1::InvalidIntentControlIndex);
    };
    if domain != "maestro.vnext.effect-intent-control-index.v1" || rows.is_empty() {
        return Err(ExecutionStoreErrorV1::InvalidIntentControlIndex);
    }
    let mut entries = Vec::with_capacity(rows.len());
    for row in rows {
        let CborValue::Array(values) = row else {
            return Err(ExecutionStoreErrorV1::InvalidIntentControlIndex);
        };
        let [intent, semantic_uniqueness, control_head, object_id] = values.as_slice() else {
            return Err(ExecutionStoreErrorV1::InvalidIntentControlIndex);
        };
        entries.push(ControlIndexEntryV1 {
            intent: EffectIntentIdV1::from_bytes(
                exact_digest(intent).ok_or(ExecutionStoreErrorV1::InvalidIntentControlIndex)?,
            )?,
            semantic_uniqueness_commitment: exact_digest(semantic_uniqueness)
                .ok_or(ExecutionStoreErrorV1::InvalidIntentControlIndex)?,
            control_head: EffectIntentControlTokenV1::new(HomeTokenV1::new(
                exact_digest(control_head)
                    .ok_or(ExecutionStoreErrorV1::InvalidIntentControlIndex)?,
            )),
            control_head_object_id: StoreObjectIdV1::from_digest(
                exact_digest(object_id).ok_or(ExecutionStoreErrorV1::InvalidIntentControlIndex)?,
            ),
        });
    }
    if entries.windows(2).any(|pair| pair[0] >= pair[1])
        || has_duplicate_control_index_semantic_selector(&entries)
    {
        return Err(ExecutionStoreErrorV1::InvalidIntentControlIndex);
    }
    let mut expected_references = entries
        .iter()
        .map(|entry| entry.control_head_object_id)
        .collect::<Vec<_>>();
    expected_references.sort_unstable();
    expected_references.dedup();
    if object.references() != expected_references {
        return Err(ExecutionStoreErrorV1::InvalidIntentControlIndex);
    }
    Ok(ActiveControlIndexV1 {
        object: object.clone(),
        entries,
    })
}

fn load_optional_control_index(
    active_objects: &[StoreObjectV1],
) -> Result<Option<ActiveControlIndexV1>, ExecutionStoreErrorV1> {
    let schema = execution_schema_id("maestro.vnext.effect-intent-control-index-schema.v1")?;
    if active_objects
        .iter()
        .all(|object| object.schema_id() != schema)
    {
        return Ok(None);
    }
    load_control_index(active_objects).map(Some)
}

fn validate_state_binding_against_objects(
    binding: &ExecutionStoreStateBindingV1,
    active_objects: &[StoreObjectV1],
    expected_intent: Option<EffectIntentIdV1>,
) -> Result<Option<ActiveControlIndexV1>, ExecutionStoreErrorV1> {
    match (
        binding.control_head(),
        binding.control_index_object_id(),
        expected_intent,
    ) {
        (None, None, None) => Ok(None),
        (Some(control_head), Some(index_id), Some(intent)) => {
            let index = load_control_index(active_objects)?;
            if index.object.id() != index_id
                || !index
                    .entries
                    .iter()
                    .any(|entry| entry.intent == intent && entry.control_head == control_head)
            {
                return Err(ExecutionStoreErrorV1::StaleExpectedStoreState);
            }
            Ok(Some(index))
        }
        _ => Err(ExecutionStoreErrorV1::PublicationBindingMismatch),
    }
}

fn exact_referenced_schema_object<'objects>(
    owner: &StoreObjectV1,
    active_objects: &'objects [StoreObjectV1],
    schemas: &[SchemaIdV1],
) -> Result<&'objects StoreObjectV1, ExecutionStoreErrorV1> {
    let matches = owner
        .references()
        .iter()
        .filter_map(|reference| {
            active_objects
                .iter()
                .find(|object| object.id() == *reference)
                .filter(|object| schemas.contains(&object.schema_id()))
        })
        .collect::<Vec<_>>();
    let [object] = matches.as_slice() else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    };
    Ok(object)
}

fn optional_referenced_schema_object<'objects>(
    owner: &StoreObjectV1,
    active_objects: &'objects [StoreObjectV1],
    schemas: &[SchemaIdV1],
) -> Result<Option<&'objects StoreObjectV1>, ExecutionStoreErrorV1> {
    let matches = owner
        .references()
        .iter()
        .filter_map(|reference| {
            active_objects
                .iter()
                .find(|object| object.id() == *reference)
                .filter(|object| schemas.contains(&object.schema_id()))
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [] => Ok(None),
        [object] => Ok(Some(object)),
        _ => Err(ExecutionStoreErrorV1::InvalidEffectSnapshot),
    }
}

fn control_need_from_revision_carrier(
    carrier: &StoreObjectV1,
) -> Result<EffectControlTransitionNeedV1, ExecutionStoreErrorV1> {
    let dispatch_schemas = [
        execution_schema_id("maestro.vnext.effect-dispatch-reservation-carrier-schema.v1")?,
        execution_schema_id("maestro.vnext.effect-dispatch-seal-carrier-schema.v1")?,
        execution_schema_id("maestro.vnext.effect-dispatch-terminal-carrier-schema.v1")?,
    ];
    let reconciliation_schemas = [
        execution_schema_id("maestro.vnext.effect-reconciliation-begin-carrier-schema.v1")?,
        execution_schema_id("maestro.vnext.effect-reconciliation-read-carrier-schema.v1")?,
        execution_schema_id("maestro.vnext.effect-reconciliation-terminal-carrier-schema.v1")?,
    ];
    if dispatch_schemas.contains(&carrier.schema_id()) {
        return Ok(decode_effect_dispatch_authorized_carrier(carrier.value())?.1);
    }
    if reconciliation_schemas.contains(&carrier.schema_id()) {
        return Ok(decode_effect_reconciliation_authorized_carrier(carrier.value())?.1);
    }
    if carrier.schema_id()
        == execution_schema_id("maestro.vnext.effect-withdrawal-carrier-schema.v1")?
    {
        return Ok(
            decode_effect_withdrawal_authorized_carrier(carrier.value())?
                .0
                .control_need()
                .clone(),
        );
    }
    if carrier.schema_id()
        == execution_schema_id("maestro.vnext.effect-intent-health-carrier-schema.v1")?
    {
        return Ok(decode_effect_control_authorized_carrier(
            carrier.value(),
            EFFECT_HEALTH_AUTHORIZED_CARRIER_DOMAIN_V1,
        )?
        .0);
    }
    if carrier.schema_id()
        == execution_schema_id("maestro.vnext.effect-writer-handoff-carrier-schema.v1")?
    {
        return Ok(decode_effect_control_authorized_carrier(
            carrier.value(),
            EFFECT_WRITER_HANDOFF_AUTHORIZED_CARRIER_DOMAIN_V1,
        )?
        .0);
    }
    Err(ExecutionStoreErrorV1::InvalidEffectSnapshot)
}

fn validate_control_revision_history(
    revision_object: &StoreObjectV1,
    active_objects: &[StoreObjectV1],
    intent: &EffectIntentV1,
    visiting: &mut Vec<StoreObjectIdV1>,
) -> Result<EffectIntentControlRevisionV1, ExecutionStoreErrorV1> {
    if revision_object.schema_id()
        != execution_schema_id("maestro.vnext.effect-intent-control-revision-schema.v1")?
        || visiting.contains(&revision_object.id())
    {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    }
    visiting.push(revision_object.id());
    let revision = EffectIntentControlRevisionV1::from_canonical_value(revision_object.value())?;
    let revision_parts = revision.parts();
    if revision.intent() != intent.id()
        || revision_parts.material_commitment != *intent.material_inputs().as_bytes()
        || revision_parts.credential_commitment != *intent.credential_requirements().as_bytes()
    {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    }
    let revision_schema =
        execution_schema_id("maestro.vnext.effect-intent-control-revision-schema.v1")?;
    let predecessors = revision_object
        .references()
        .iter()
        .filter_map(|reference| {
            active_objects
                .iter()
                .find(|candidate| candidate.id() == *reference)
                .filter(|candidate| candidate.schema_id() == revision_schema)
        })
        .collect::<Vec<_>>();
    if predecessors.is_empty() {
        if !revision.attempt_history().is_empty()
            || revision.live_attempt().is_some()
            || revision.live_dispatch() != super::withdrawal::EffectIntentLiveDispatchV1::None
            || revision.classification() != super::withdrawal::RemoteClassificationV1::Prepared
            || revision.dispatch_fence_high_water() != 0
            || !revision.runs_closed()
            || revision_parts.result_commitment.is_some()
            || revision_parts.idempotency_commitment.is_some()
            || revision.health() != super::control_head::EffectIntentControlHealthV1::Healthy
        {
            return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
        }
    } else {
        let [predecessor_object] = predecessors.as_slice() else {
            return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
        };
        let predecessor = validate_control_revision_history(
            predecessor_object,
            active_objects,
            intent,
            visiting,
        )?;
        let carriers = revision_object
            .references()
            .iter()
            .filter_map(|reference| {
                active_objects
                    .iter()
                    .find(|candidate| candidate.id() == *reference)
            })
            .filter_map(|candidate| {
                control_need_from_revision_carrier(candidate)
                    .ok()
                    .map(|need| (candidate, need))
            })
            .collect::<Vec<_>>();
        let [(_, control_need)] = carriers.as_slice() else {
            return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
        };
        if control_need.persisted_candidate_revision(&predecessor)? != revision {
            return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
        }
    }
    visiting.pop();
    Ok(revision)
}

fn dispatch_publication_result_object<'objects>(
    carrier: &StoreObjectV1,
    active_objects: &'objects [StoreObjectV1],
) -> Result<&'objects StoreObjectV1, ExecutionStoreErrorV1> {
    let result_schema = if carrier.schema_id()
        == execution_schema_id("maestro.vnext.effect-dispatch-reservation-carrier-schema.v1")?
    {
        match decode_effect_dispatch_authorized_carrier(carrier.value())?.1 {
            EffectControlTransitionNeedV1::ReserveDispatch { .. } => {
                execution_schema_id("maestro.vnext.effect-origination-result-schema.v1")?
            }
            EffectControlTransitionNeedV1::RedispatchConclusiveNotApplied { .. } => {
                execution_schema_id("maestro.vnext.effect-redispatch-result-schema.v1")?
            }
            _ => return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot),
        }
    } else if carrier.schema_id()
        == execution_schema_id("maestro.vnext.effect-dispatch-seal-carrier-schema.v1")?
    {
        execution_schema_id("maestro.vnext.effect-dispatch-seal-result-schema.v1")?
    } else if carrier.schema_id()
        == execution_schema_id("maestro.vnext.effect-dispatch-terminal-carrier-schema.v1")?
    {
        match decode_effect_dispatch_authorized_carrier(carrier.value())?.1 {
            EffectControlTransitionNeedV1::FinishDispatch { .. } => {
                execution_schema_id("maestro.vnext.effect-dispatch-terminal-result-schema.v1")?
            }
            EffectControlTransitionNeedV1::RecoverSealedInDoubt { .. } => {
                execution_schema_id("maestro.vnext.effect-health-result-schema.v1")?
            }
            _ => return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot),
        }
    } else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    };
    let matches = active_objects
        .iter()
        .filter(|candidate| {
            candidate.schema_id() == result_schema && candidate.references().contains(&carrier.id())
        })
        .collect::<Vec<_>>();
    let [result] = matches.as_slice() else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    };
    Ok(result)
}

fn publication_predecessor_generation(
    current_generation: &StoreGenerationV1,
    result: StoreObjectIdV1,
    load_generation: &mut impl FnMut(
        StoreGenerationIdV1,
    ) -> Result<StoreGenerationV1, ExecutionStoreErrorV1>,
) -> Result<StoreGenerationV1, ExecutionStoreErrorV1> {
    if !current_generation.roots().contains(&result) {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    }
    let mut publication_generation = current_generation.clone();
    loop {
        let predecessor_id = publication_generation
            .previous()
            .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?;
        let predecessor = load_generation(predecessor_id)?;
        if predecessor.id() != predecessor_id
            || predecessor.domain() != publication_generation.domain()
            || predecessor.ordinal().checked_add(1) != Some(publication_generation.ordinal())
        {
            return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
        }
        if predecessor.roots().contains(&result) {
            publication_generation = predecessor;
        } else {
            return Ok(predecessor);
        }
    }
}

fn effect_health_payload_from_need(
    need: &EffectControlTransitionNeedV1,
) -> Result<CborValue, ExecutionStoreErrorV1> {
    let draft = match need {
        EffectControlTransitionNeedV1::RecoverSealedInDoubt { .. } => {
            ActiveStoreEffectHealthDraftV1::RecoverSealedInDoubt
        }
        EffectControlTransitionNeedV1::MarkRecoveryRequired { .. } => {
            ActiveStoreEffectHealthDraftV1::MarkRecoveryRequired
        }
        EffectControlTransitionNeedV1::MarkIntegrityBlocked { .. } => {
            ActiveStoreEffectHealthDraftV1::MarkIntegrityBlocked
        }
        _ => return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot),
    };
    Ok(draft.payload_value()?)
}

fn validate_persisted_control_authority(
    carrier: &StoreObjectV1,
    active_objects: &[StoreObjectV1],
    current_generation: &StoreGenerationV1,
    load_generation: &mut impl FnMut(
        StoreGenerationIdV1,
    ) -> Result<StoreGenerationV1, ExecutionStoreErrorV1>,
    intent: &EffectIntentV1,
) -> Result<EffectControlTransitionNeedV1, ExecutionStoreErrorV1> {
    let health_schema =
        execution_schema_id("maestro.vnext.effect-intent-health-carrier-schema.v1")?;
    let handoff_schema =
        execution_schema_id("maestro.vnext.effect-writer-handoff-carrier-schema.v1")?;
    let (carrier_domain, result_schema, result_domain, expected_payload) =
        if carrier.schema_id() == health_schema {
            let (need, _) = decode_effect_control_authorized_carrier(
                carrier.value(),
                EFFECT_HEALTH_AUTHORIZED_CARRIER_DOMAIN_V1,
            )?;
            (
                EFFECT_HEALTH_AUTHORIZED_CARRIER_DOMAIN_V1,
                execution_schema_id("maestro.vnext.effect-health-result-schema.v1")?,
                "maestro.vnext.effect-health-result.v1",
                effect_health_payload_from_need(&need)?,
            )
        } else if carrier.schema_id() == handoff_schema {
            (
                EFFECT_WRITER_HANDOFF_AUTHORIZED_CARRIER_DOMAIN_V1,
                execution_schema_id("maestro.vnext.effect-writer-handoff-result-schema.v1")?,
                "maestro.vnext.effect-writer-handoff-result.v1",
                ActiveStoreEffectWriterHandoffDraftV1::new().payload_value()?,
            )
        } else {
            return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
        };
    let (need, authority) =
        decode_effect_control_authorized_carrier(carrier.value(), carrier_domain)?;
    let results = active_objects
        .iter()
        .filter(|object| {
            object.schema_id() == result_schema && object.references().contains(&carrier.id())
        })
        .collect::<Vec<_>>();
    let [result] = results.as_slice() else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    };
    let CborValue::Array(result_fields) = result.value() else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    };
    if !matches!(result_fields.first(), Some(CborValue::Text(domain)) if domain == result_domain) {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    }
    let predecessor =
        publication_predecessor_generation(current_generation, result.id(), load_generation)?;
    let request_object = exact_referenced_schema_object(
        carrier,
        active_objects,
        &[execution_schema_id(
            "maestro.vnext.execution-action-request-schema.v1",
        )?],
    )?;
    let request = CanonicalExecutionActionRequestV1::from_canonical_value(request_object.value())?;
    validate_execution_authority_binding(&request, &authority)?;
    validate_effect_authority_origin(&authority, intent.origin().kind())?;
    let validated = carrier
        .references()
        .iter()
        .filter_map(|reference| {
            active_objects
                .iter()
                .find(|candidate| candidate.id() == *reference)
        })
        .filter_map(|candidate| {
            validate_persisted_repository_action_basis(
                &predecessor,
                request.request_id(),
                request_object.id(),
                &authority,
                candidate,
                active_objects,
            )
            .ok()
        })
        .collect::<Vec<_>>();
    let [_] = validated.as_slice() else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    };
    if request.request_id() != need.action_request_id()
        || request.action() != intent.origin().reconciliation_action()?
        || request.subject_commitment() != hash(&effect_intent_subject_value(intent)?)?
        || request.payload_commitment() != hash(&expected_payload)?
    {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    }
    if let EffectControlTransitionNeedV1::RecoverSealedInDoubt {
        attempt,
        result_commitment,
        idempotency_commitment,
        ..
    } = need
    {
        let occurrence = exact_referenced_schema_object(
            carrier,
            active_objects,
            &[execution_schema_id(
                "maestro.vnext.effect-recover-sealed-occurrence-schema.v1",
            )?],
        )?;
        let CborValue::Array(occurrence_fields) = occurrence.value() else {
            return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
        };
        let [
            CborValue::Text(occurrence_domain),
            occurrence_request,
            occurrence_intent,
            _predecessor_head,
            _predecessor_index,
            occurrence_owner,
            occurrence_meaning,
        ] = occurrence_fields.as_slice()
        else {
            return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
        };
        let [
            CborValue::Text(_),
            result_request,
            result_intent,
            _result_head,
            _result_revision,
            CborValue::Unsigned(result_health),
            result_meaning,
        ] = result_fields.as_slice()
        else {
            return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
        };
        let meaning =
            exact_digest(occurrence_meaning).ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?;
        let expected_idempotency = hash(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-recover-sealed-idempotency.v1")?,
            bytes(request.idempotency_key_id().as_bytes()),
            bytes(&meaning),
            bytes(occurrence.id().as_bytes()),
        ]))?;
        if occurrence_domain != "maestro.vnext.effect-recover-sealed-occurrence.v1"
            || occurrence.id().as_bytes() != &result_commitment
            || !occurrence.references().contains(&request_object.id())
            || !occurrence
                .references()
                .iter()
                .all(|reference| carrier.references().contains(reference))
            || exact_digest(occurrence_request) != Some(*request.request_id().as_bytes())
            || exact_digest(occurrence_intent) != Some(*intent.id().as_bytes())
            || super::runtime::parse_attempt_owner(occurrence_owner)? != attempt
            || exact_digest(result_request) != Some(*request.request_id().as_bytes())
            || exact_digest(result_intent) != Some(*intent.id().as_bytes())
            || exact_digest(result_meaning) != Some(meaning)
            || *result_health
                != effect_control_health_tag(EffectIntentControlHealthV1::RecoveryRequired)
            || meaning == [0; 32]
            || expected_idempotency != idempotency_commitment
        {
            return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
        }
    }
    Ok(need)
}

fn dispatch_origination_subject_value(
    intent: &EffectIntentV1,
) -> Result<CborValue, ExecutionStoreErrorV1> {
    let EffectIntentHomeV1::ActiveStore(home) = intent.home() else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    };
    Ok(CborValue::Array(vec![
        CborValue::text("maestro.vnext.active-store-effect-origination-subject.v1")?,
        CborValue::Unsigned(match home.domain_kind {
            EffectIntentDomainKindV1::RepositoryDomain => 1,
            EffectIntentDomainKindV1::InstallationDomain => 2,
        }),
        bytes(home.stable_domain_id.as_bytes()),
        bytes(home.realm.as_bytes()),
        bytes(home.semantic_namespace.as_bytes()),
        bytes(home.home_qualified_semantic_uniqueness_namespace.as_bytes()),
        CborValue::Unsigned(u64::from(intent.origin().kind().tag())),
        bytes(&intent.origin().commitment()?),
        bytes(intent.semantic_use().as_bytes()),
        bytes(intent.material_inputs().as_bytes()),
        bytes(intent.credential_requirements().as_bytes()),
    ]))
}

fn dispatch_reservation_payload_value(
    dispatch: &EffectDispatchAttemptV1,
) -> Result<CborValue, ExecutionStoreErrorV1> {
    let binding = dispatch.state().binding();
    let run = dispatch
        .run_set()
        .runs()
        .first()
        .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?
        .reservation();
    let inputs = EffectDispatchBindingInputsV1 {
        attempt_revision: binding.attempt_revision(),
        application_envelope_commitment: *binding.application_envelope_id().as_bytes(),
        provider_operation_contract_commitment: *binding
            .provider_operation_contract_id()
            .as_bytes(),
        provider_scope_commitment: *binding.provider_scope_id().as_bytes(),
        provider_key_commitment: *binding.provider_key_id().as_bytes(),
        material_stamp_commitment: *binding.material_stamp_id().as_bytes(),
        run_set_revision_commitment: *binding.run_set_revision_id().as_bytes(),
        accounting_basis_commitment: *binding.accounting_basis_id().as_bytes(),
        provider_run: run,
    };
    Ok(CborValue::Array(vec![
        CborValue::text("maestro.vnext.active-store-effect-origination-payload.v1")?,
        inputs.canonical_value()?,
    ]))
}

fn dispatch_origination_request_payload_value(
    intent: &EffectIntentV1,
    dispatch: &EffectDispatchAttemptV1,
) -> Result<CborValue, ExecutionStoreErrorV1> {
    Ok(CborValue::Array(vec![
        CborValue::text("maestro.vnext.active-store-effect-origination-request-payload.v1")?,
        dispatch_origination_subject_value(intent)?,
        dispatch_reservation_payload_value(dispatch)?,
    ]))
}

fn dispatch_redispatch_payload_value(
    dispatch: &EffectDispatchAttemptV1,
) -> Result<CborValue, ExecutionStoreErrorV1> {
    let CborValue::Array(mut fields) = dispatch_reservation_payload_value(dispatch)? else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    };
    let Some(CborValue::Text(domain)) = fields.first_mut() else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    };
    *domain = "maestro.vnext.active-store-effect-redispatch-payload.v1".to_owned();
    Ok(CborValue::Array(fields))
}

fn dispatch_seal_payload_value(
    dispatch: &EffectDispatchAttemptV1,
) -> Result<CborValue, ExecutionStoreErrorV1> {
    let seal = dispatch
        .crossing_seal_commitment()
        .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?;
    Ok(ActiveStoreEffectSealDraftV1::new(seal)?.payload_value()?)
}

#[expect(
    clippy::too_many_arguments,
    reason = "restart validation must bind the complete persisted dispatch authority product"
)]
fn validate_persisted_dispatch_authority(
    carrier: &StoreObjectV1,
    active_objects: &[StoreObjectV1],
    current_generation: &StoreGenerationV1,
    load_generation: &mut impl FnMut(
        StoreGenerationIdV1,
    ) -> Result<StoreGenerationV1, ExecutionStoreErrorV1>,
    intent: &EffectIntentV1,
    dispatch: &EffectDispatchAttemptV1,
    control_need: &EffectControlTransitionNeedV1,
    request: &CanonicalExecutionActionRequestV1,
    authority: &ExecutionAuthorityV1,
) -> Result<(), ExecutionStoreErrorV1> {
    let result = dispatch_publication_result_object(carrier, active_objects)?;
    let predecessor =
        publication_predecessor_generation(current_generation, result.id(), load_generation)?;
    let request_object = exact_referenced_schema_object(
        carrier,
        active_objects,
        &[execution_schema_id(
            "maestro.vnext.execution-action-request-schema.v1",
        )?],
    )?;
    let validated = carrier
        .references()
        .iter()
        .filter_map(|reference| {
            active_objects
                .iter()
                .find(|candidate| candidate.id() == *reference)
        })
        .filter_map(|candidate| {
            validate_persisted_repository_action_basis(
                &predecessor,
                request.request_id(),
                request_object.id(),
                authority,
                candidate,
                active_objects,
            )
            .ok()
        })
        .collect::<Vec<_>>();
    let [validated] = validated.as_slice() else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    };
    let authorized = AuthorizedExecutionActionV1::new(
        request.clone(),
        validated.authorization_receipt().clone(),
    )?;
    let expected_subject = match control_need {
        EffectControlTransitionNeedV1::ReserveDispatch { .. } => {
            if request.action() != intent.origin().reservation_action()?
                || request.payload_commitment()
                    != hash(&dispatch_origination_request_payload_value(
                        intent, dispatch,
                    )?)?
                || intent.origination_authority()
                    != super::effects::EffectOriginationAuthorityV1::from_action(&authorized)?
            {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            }
            effect_intent_subject_value(intent)?
        }
        EffectControlTransitionNeedV1::RedispatchConclusiveNotApplied { .. } => {
            if request.action() != intent.origin().reservation_action()?
                || request.payload_commitment()
                    != hash(&dispatch_redispatch_payload_value(dispatch)?)?
            {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            }
            effect_intent_subject_value(intent)?
        }
        EffectControlTransitionNeedV1::SealDispatch { .. } => {
            if (request.action() != intent.origin().outcome_action()?
                && request.action() != intent.origin().reservation_action()?)
                || request.payload_commitment() != hash(&dispatch_seal_payload_value(dispatch)?)?
            {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            }
            effect_intent_subject_value(intent)?
        }
        EffectControlTransitionNeedV1::FinishDispatch { .. } => {
            if request.action() != intent.origin().outcome_action()?
                && request.action() != intent.origin().reservation_action()?
            {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            }
            effect_intent_subject_value(intent)?
        }
        EffectControlTransitionNeedV1::RecoverSealedInDoubt { .. } => {
            if request.action() != intent.origin().reconciliation_action()?
                || request.payload_commitment()
                    != hash(&ActiveStoreEffectHealthDraftV1::RecoverSealedInDoubt.payload_value()?)?
            {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            }
            effect_intent_subject_value(intent)?
        }
        _ => return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot),
    };
    if request.subject_commitment() != hash(&expected_subject)? {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    }
    Ok(())
}

fn load_validated_dispatch_carrier(
    object: &StoreObjectV1,
    active_objects: &[StoreObjectV1],
    intent: &EffectIntentV1,
    current_generation: &StoreGenerationV1,
    load_generation: &mut impl FnMut(
        StoreGenerationIdV1,
    ) -> Result<StoreGenerationV1, ExecutionStoreErrorV1>,
) -> Result<
    (
        EffectDispatchAttemptV1,
        EffectControlTransitionNeedV1,
        ExecutionAuthorityV1,
    ),
    ExecutionStoreErrorV1,
> {
    let reservation_schema =
        execution_schema_id("maestro.vnext.effect-dispatch-reservation-carrier-schema.v1")?;
    let seal_schema = execution_schema_id("maestro.vnext.effect-dispatch-seal-carrier-schema.v1")?;
    let terminal_schema =
        execution_schema_id("maestro.vnext.effect-dispatch-terminal-carrier-schema.v1")?;
    let (dispatch, control_need, authority) =
        decode_effect_dispatch_authorized_carrier(object.value())?;
    let request_object = exact_referenced_schema_object(
        object,
        active_objects,
        &[execution_schema_id(
            "maestro.vnext.execution-action-request-schema.v1",
        )?],
    )?;
    let request = CanonicalExecutionActionRequestV1::from_canonical_value(request_object.value())?;
    validate_execution_authority_binding(&request, &authority)?;
    validate_effect_authority_origin(&authority, intent.origin().kind())?;
    if request.request_id() != control_need.action_request_id() {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    }
    let expected_schema = match dispatch.state() {
        super::dispatch_state::DispatchAttemptStateV1::ReservedUnsealed(_) => reservation_schema,
        super::dispatch_state::DispatchAttemptStateV1::SealedInFlight(_) => seal_schema,
        super::dispatch_state::DispatchAttemptStateV1::Terminal(_) => terminal_schema,
    };
    if object.schema_id() != expected_schema {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    }
    if let EffectControlTransitionNeedV1::FinishDispatch {
        result_commitment, ..
    } = control_need
    {
        let occurrence_object = exact_referenced_schema_object(
            object,
            active_objects,
            &[execution_schema_id(
                "maestro.vnext.effect-dispatch-terminal-occurrence-schema.v1",
            )?],
        )?;
        if occurrence_object.id().as_bytes() != &result_commitment {
            return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
        }
        let occurrence_request_object = exact_referenced_schema_object(
            occurrence_object,
            active_objects,
            &[execution_schema_id(
                "maestro.vnext.execution-action-request-schema.v1",
            )?],
        )?;
        if occurrence_request_object.id() != request_object.id() {
            return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
        }
        let CborValue::Array(fields) = occurrence_object.value() else {
            return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
        };
        let [CborValue::Text(domain), request_id, payload, meaning] = fields.as_slice() else {
            return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
        };
        let occurrence_request_id =
            exact_digest(request_id).ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?;
        let occurrence_meaning =
            exact_digest(meaning).ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?;
        if domain != "maestro.vnext.effect-dispatch-terminal-occurrence.v1"
            || occurrence_request_id != *request.request_id().as_bytes()
            || occurrence_meaning == [0; 32]
            || request.payload_commitment() != hash(payload)?
            || decode_effect_terminal_payload_outcome(payload)?
                != dispatch.terminal_outcome_payload()?
        {
            return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
        }
    }
    if let EffectControlTransitionNeedV1::RecoverSealedInDoubt {
        attempt,
        result_commitment,
        idempotency_commitment,
        ..
    } = control_need
    {
        let occurrence = exact_referenced_schema_object(
            object,
            active_objects,
            &[execution_schema_id(
                "maestro.vnext.effect-recover-sealed-occurrence-schema.v1",
            )?],
        )?;
        let CborValue::Array(fields) = occurrence.value() else {
            return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
        };
        let [
            CborValue::Text(domain),
            occurrence_request,
            occurrence_intent,
            _predecessor_head,
            _predecessor_index,
            occurrence_owner,
            meaning,
        ] = fields.as_slice()
        else {
            return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
        };
        let meaning = exact_digest(meaning).ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?;
        let expected_idempotency = hash(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-recover-sealed-idempotency.v1")?,
            bytes(request.idempotency_key_id().as_bytes()),
            bytes(&meaning),
            bytes(occurrence.id().as_bytes()),
        ]))?;
        if domain != "maestro.vnext.effect-recover-sealed-occurrence.v1"
            || occurrence.id().as_bytes() != &result_commitment
            || exact_digest(occurrence_request) != Some(*request.request_id().as_bytes())
            || exact_digest(occurrence_intent) != Some(*intent.id().as_bytes())
            || super::runtime::parse_attempt_owner(occurrence_owner)? != attempt
            || meaning == [0; 32]
            || expected_idempotency != idempotency_commitment
            || dispatch
                .state()
                .terminal_outcome()
                .map_err(|_| ExecutionStoreErrorV1::InvalidEffectSnapshot)?
                != super::dispatch_state::DispatchAttemptOutcomeV1::AmbiguousTransport
            || dispatch.terminal_classification()
                != Some(super::withdrawal::RemoteClassificationV1::InDoubt)
            || !dispatch.run_set().all_terminal()
        {
            return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
        }
    }
    let carrier_schemas = [reservation_schema, seal_schema, terminal_schema];
    let predecessor_objects = object
        .references()
        .iter()
        .filter_map(|reference| {
            active_objects
                .iter()
                .find(|candidate| candidate.id() == *reference)
                .filter(|candidate| carrier_schemas.contains(&candidate.schema_id()))
        })
        .collect::<Vec<_>>();
    match dispatch.state() {
        super::dispatch_state::DispatchAttemptStateV1::ReservedUnsealed(_) => match &control_need {
            EffectControlTransitionNeedV1::ReserveDispatch { .. } => {
                if !predecessor_objects.is_empty() {
                    return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
                }
            }
            EffectControlTransitionNeedV1::RedispatchConclusiveNotApplied { .. } => {
                let [predecessor_object] = predecessor_objects.as_slice() else {
                    return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
                };
                if predecessor_object.schema_id() != terminal_schema {
                    return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
                }
                let (predecessor, predecessor_need, _) = load_validated_dispatch_carrier(
                    predecessor_object,
                    active_objects,
                    intent,
                    current_generation,
                    load_generation,
                )?;
                if !matches!(
                    predecessor_need,
                    EffectControlTransitionNeedV1::FinishDispatch {
                        classification:
                            super::withdrawal::RemoteClassificationV1::ConfirmedNotApplied,
                        ..
                    }
                ) || dispatch.attempt().effect_intent_id()
                    != predecessor.attempt().effect_intent_id()
                    || dispatch.attempt().dispatch_fence()
                        != predecessor
                            .attempt()
                            .dispatch_fence()
                            .checked_add(1)
                            .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?
                    || dispatch.run_set().revision()
                        != predecessor
                            .run_set()
                            .revision()
                            .checked_add(1)
                            .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?
                {
                    return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
                }
            }
            _ => return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot),
        },
        super::dispatch_state::DispatchAttemptStateV1::SealedInFlight(_) => {
            let [predecessor_object] = predecessor_objects.as_slice() else {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            };
            if predecessor_object.schema_id() != reservation_schema {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            }
            let (predecessor, predecessor_need, _) = load_validated_dispatch_carrier(
                predecessor_object,
                active_objects,
                intent,
                current_generation,
                load_generation,
            )?;
            if !matches!(
                predecessor_need,
                EffectControlTransitionNeedV1::ReserveDispatch { .. }
                    | EffectControlTransitionNeedV1::RedispatchConclusiveNotApplied { .. }
            ) {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            }
            dispatch.validate_persisted_predecessor(&predecessor)?;
        }
        super::dispatch_state::DispatchAttemptStateV1::Terminal(_) => {
            let [predecessor_object] = predecessor_objects.as_slice() else {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            };
            let expected_predecessor_schema = if dispatch.crossing_seal_value().is_some() {
                seal_schema
            } else {
                reservation_schema
            };
            if predecessor_object.schema_id() != expected_predecessor_schema {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            }
            let (predecessor, predecessor_need, _) = load_validated_dispatch_carrier(
                predecessor_object,
                active_objects,
                intent,
                current_generation,
                load_generation,
            )?;
            let predecessor_phase_matches = matches!(
                (
                    &predecessor_need,
                    expected_predecessor_schema == seal_schema
                ),
                (EffectControlTransitionNeedV1::SealDispatch { .. }, true)
                    | (EffectControlTransitionNeedV1::ReserveDispatch { .. }, false)
                    | (
                        EffectControlTransitionNeedV1::RedispatchConclusiveNotApplied { .. },
                        false,
                    )
            );
            if !predecessor_phase_matches {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            }
            dispatch.validate_persisted_predecessor(&predecessor)?;
        }
    }
    if matches!(
        control_need,
        EffectControlTransitionNeedV1::FinishDispatch { .. }
    ) {
        let occurrence = exact_referenced_schema_object(
            object,
            active_objects,
            &[execution_schema_id(
                "maestro.vnext.effect-dispatch-terminal-occurrence-schema.v1",
            )?],
        )?;
        let CborValue::Array(occurrence_fields) = occurrence.value() else {
            return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
        };
        let Some(payload) = occurrence_fields.get(2) else {
            return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
        };
        let predecessor_head_object = exact_referenced_schema_object(
            object,
            active_objects,
            &[execution_schema_id(
                "maestro.vnext.effect-intent-control-head-schema.v1",
            )?],
        )?;
        let CborValue::Array(head_fields) = predecessor_head_object.value() else {
            return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
        };
        let Some(stored_head_id) = head_fields.first() else {
            return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
        };
        validate_effect_terminal_payload_proof(
            payload,
            intent,
            &dispatch,
            EffectIntentControlTokenV1::new(HomeTokenV1::new(
                exact_digest(stored_head_id).ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?,
            )),
        )?;
    }
    validate_persisted_dispatch_authority(
        object,
        active_objects,
        current_generation,
        load_generation,
        intent,
        &dispatch,
        &control_need,
        &request,
        &authority,
    )?;
    Ok((dispatch, control_need, authority))
}

fn decode_reconciliation_read_usage_store_value(
    value: &CborValue,
) -> Result<EffectReconciliationReadUsageV1, ExecutionStoreErrorV1> {
    let CborValue::Array(fields) = value else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    };
    let [
        CborValue::Unsigned(requests),
        CborValue::Unsigned(pages),
        CborValue::Unsigned(bytes),
        CborValue::Unsigned(duration_ms),
        result_commitment,
    ] = fields.as_slice()
    else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    };
    Ok(EffectReconciliationReadUsageV1 {
        requests: u16::try_from(*requests)
            .map_err(|_| ExecutionStoreErrorV1::InvalidEffectSnapshot)?,
        pages: u16::try_from(*pages).map_err(|_| ExecutionStoreErrorV1::InvalidEffectSnapshot)?,
        bytes: *bytes,
        duration_ms: *duration_ms,
        result_commitment: exact_digest(result_commitment)
            .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?,
    })
}

fn load_validated_reconciliation_carrier(
    object: &StoreObjectV1,
    active_objects: &[StoreObjectV1],
    intent: &EffectIntentV1,
) -> Result<
    (
        EffectReconciliationAttemptV1,
        EffectControlTransitionNeedV1,
        ExecutionAuthorityV1,
    ),
    ExecutionStoreErrorV1,
> {
    let begin_schema =
        execution_schema_id("maestro.vnext.effect-reconciliation-begin-carrier-schema.v1")?;
    let read_schema =
        execution_schema_id("maestro.vnext.effect-reconciliation-read-carrier-schema.v1")?;
    let terminal_schema =
        execution_schema_id("maestro.vnext.effect-reconciliation-terminal-carrier-schema.v1")?;
    let (attempt, control_need, authority) =
        decode_effect_reconciliation_authorized_carrier(object.value())?;
    let phase = match control_need {
        EffectControlTransitionNeedV1::BeginReconciliation { .. } => {
            ReconciliationPublicationPhaseV1::Begin
        }
        EffectControlTransitionNeedV1::RecordReconciliationRead { .. } => {
            ReconciliationPublicationPhaseV1::Read
        }
        EffectControlTransitionNeedV1::FinishReconciliation { .. } => {
            ReconciliationPublicationPhaseV1::Terminal
        }
        _ => return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot),
    };
    let expected_schema = execution_schema_id(phase.carrier_schema())?;
    if object.schema_id() != expected_schema {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    }
    let request_object = exact_referenced_schema_object(
        object,
        active_objects,
        &[execution_schema_id(
            "maestro.vnext.execution-action-request-schema.v1",
        )?],
    )?;
    let request = CanonicalExecutionActionRequestV1::from_canonical_value(request_object.value())?;
    validate_execution_authority_binding(&request, &authority)?;
    validate_effect_authority_origin(&authority, intent.origin().kind())?;
    if request.request_id() != control_need.action_request_id()
        || request.action() != intent.origin().reconciliation_action()?
        || request.subject_commitment() != hash(&effect_intent_subject_value(intent)?)?
    {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    }
    let payload = if matches!(phase, ReconciliationPublicationPhaseV1::Read) {
        let occurrence = exact_referenced_schema_object(
            object,
            active_objects,
            &[execution_schema_id(
                "maestro.vnext.effect-reconciliation-read-occurrence-schema.v1",
            )?],
        )?;
        let CborValue::Array(fields) = occurrence.value() else {
            return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
        };
        let [
            CborValue::Text(domain),
            _request,
            usage,
            CborValue::Unsigned(classification),
            _sealed_read,
            proof_commitment,
            _meaning,
        ] = fields.as_slice()
        else {
            return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
        };
        if domain != "maestro.vnext.effect-reconciliation-read-occurrence.v1" {
            return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
        }
        CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-reconciliation-read-payload.v1")?,
            usage.clone(),
            CborValue::Unsigned(*classification),
            proof_commitment.clone(),
        ])
    } else {
        persisted_reconciliation_payload(&attempt, &control_need)?
    };
    if matches!(phase, ReconciliationPublicationPhaseV1::Begin)
        && request.payload_commitment() != hash(&payload)?
    {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    }
    let expected_meaning = hash(&CborValue::Array(vec![
        CborValue::text(phase.idempotency_namespace())?,
        request.canonical_value()?,
        execution_authority_value(&authority)?,
        payload,
    ]))?;
    let carrier_schemas = [begin_schema, read_schema, terminal_schema];
    let predecessor_objects = object
        .references()
        .iter()
        .filter_map(|reference| {
            active_objects
                .iter()
                .find(|candidate| candidate.id() == *reference)
                .filter(|candidate| carrier_schemas.contains(&candidate.schema_id()))
        })
        .collect::<Vec<_>>();
    match expected_schema {
        schema if schema == begin_schema => {
            if !predecessor_objects.is_empty() {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            }
        }
        schema if schema == read_schema => {
            let [predecessor_object] = predecessor_objects.as_slice() else {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            };
            if predecessor_object.schema_id() != begin_schema {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            }
            let (predecessor, predecessor_need, predecessor_authority) =
                load_validated_reconciliation_carrier(predecessor_object, active_objects, intent)?;
            if !matches!(
                predecessor_need,
                EffectControlTransitionNeedV1::BeginReconciliation { .. }
            ) {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            }
            if predecessor_authority != authority {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            }
            attempt.validate_persisted_predecessor(&predecessor)?;
        }
        schema if schema == terminal_schema => {
            let [predecessor_object] = predecessor_objects.as_slice() else {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            };
            if predecessor_object.schema_id() != read_schema {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            }
            let (predecessor, predecessor_need, predecessor_authority) =
                load_validated_reconciliation_carrier(predecessor_object, active_objects, intent)?;
            let read_result = match predecessor_need {
                EffectControlTransitionNeedV1::RecordReconciliationRead {
                    result_commitment,
                    ..
                } => result_commitment,
                _ => return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot),
            };
            let terminal_read_result = match control_need {
                EffectControlTransitionNeedV1::FinishReconciliation {
                    read_publication_commitment,
                    ..
                } => read_publication_commitment,
                _ => return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot),
            };
            if read_result != terminal_read_result {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            }
            if predecessor_authority != authority {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            }
            attempt.validate_persisted_predecessor(&predecessor)?;
        }
        _ => return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot),
    }
    match control_need {
        EffectControlTransitionNeedV1::BeginReconciliation { .. } => {}
        EffectControlTransitionNeedV1::RecordReconciliationRead {
            result_commitment, ..
        } => {
            let occurrence = exact_referenced_schema_object(
                object,
                active_objects,
                &[execution_schema_id(
                    "maestro.vnext.effect-reconciliation-read-occurrence-schema.v1",
                )?],
            )?;
            if occurrence.id().as_bytes() != &result_commitment {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            }
            let occurrence_request = exact_referenced_schema_object(
                occurrence,
                active_objects,
                &[execution_schema_id(
                    "maestro.vnext.execution-action-request-schema.v1",
                )?],
            )?;
            let CborValue::Array(fields) = occurrence.value() else {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            };
            let [
                CborValue::Text(domain),
                request_id,
                usage,
                CborValue::Unsigned(classification),
                sealed_read,
                proof_commitment,
                meaning,
            ] = fields.as_slice()
            else {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            };
            let usage = decode_reconciliation_read_usage_store_value(usage)?;
            let classification = parse_remote_classification_store_tag(*classification)?;
            let sealed_read = decode_sealed_reconciliation_read_value(sealed_read)?;
            let [attempt_run] = attempt.run_set().runs() else {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            };
            let attempt_reservation = attempt_run.reservation();
            let predecessor_head = exact_referenced_schema_object(
                object,
                active_objects,
                &[execution_schema_id(
                    "maestro.vnext.effect-intent-control-head-schema.v1",
                )?],
            )?;
            let CborValue::Array(predecessor_head_fields) = predecessor_head.value() else {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            };
            let Some(stored_predecessor_head_id) = predecessor_head_fields.first() else {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            };
            if domain != "maestro.vnext.effect-reconciliation-read-occurrence.v1"
                || occurrence_request.id() != request_object.id()
                || exact_digest(request_id) != Some(*request.request_id().as_bytes())
                || exact_digest(meaning) != Some(expected_meaning)
                || usage
                    != attempt
                        .read_usage()
                        .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?
                || sealed_read.intent != intent.id()
                || sealed_read.run_id != attempt_run.id()
                || sealed_read.execution_boundary_commitment
                    != attempt_reservation.execution_boundary_commitment
                || sealed_read.deadline != attempt_reservation.deadline
                || sealed_read.read_plan != attempt.read_plan()
                || exact_digest(stored_predecessor_head_id)
                    != Some(*sealed_read.control_head.as_bytes())
                || exact_digest(proof_commitment)
                    != Some(reconciliation_read_proof_commitment(
                        &sealed_read,
                        usage,
                        classification,
                    )?)
            {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            }
            let expected_idempotency = reconciliation_occurrence_idempotency(
                phase,
                &request,
                expected_meaning,
                occurrence.id(),
            )?;
            let EffectControlTransitionNeedV1::RecordReconciliationRead {
                idempotency_commitment,
                ..
            } = control_need
            else {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            };
            if idempotency_commitment != expected_idempotency {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            }
        }
        EffectControlTransitionNeedV1::FinishReconciliation {
            classification,
            result_commitment,
            ..
        } => {
            let occurrence = exact_referenced_schema_object(
                object,
                active_objects,
                &[execution_schema_id(
                    "maestro.vnext.effect-reconciliation-terminal-occurrence-schema.v1",
                )?],
            )?;
            if occurrence.id().as_bytes() != &result_commitment {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            }
            let occurrence_request = exact_referenced_schema_object(
                occurrence,
                active_objects,
                &[execution_schema_id(
                    "maestro.vnext.execution-action-request-schema.v1",
                )?],
            )?;
            let CborValue::Array(fields) = occurrence.value() else {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            };
            let [
                CborValue::Text(domain),
                request_id,
                CborValue::Unsigned(stored_classification),
                read_result,
                meaning,
            ] = fields.as_slice()
            else {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            };
            if domain != "maestro.vnext.effect-reconciliation-terminal-occurrence.v1"
                || occurrence_request.id() != request_object.id()
                || exact_digest(request_id) != Some(*request.request_id().as_bytes())
                || parse_remote_classification_store_tag(*stored_classification)? != classification
                || exact_digest(read_result)
                    != attempt.read_usage().map(|usage| usage.result_commitment)
                || exact_digest(meaning) != Some(expected_meaning)
            {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            }
            let expected_idempotency = reconciliation_occurrence_idempotency(
                phase,
                &request,
                expected_meaning,
                occurrence.id(),
            )?;
            let EffectControlTransitionNeedV1::FinishReconciliation {
                idempotency_commitment,
                ..
            } = control_need
            else {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            };
            if idempotency_commitment != expected_idempotency {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            }
        }
        _ => return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot),
    }
    Ok((attempt, control_need, authority))
}

fn persisted_reconciliation_payload(
    attempt: &EffectReconciliationAttemptV1,
    control_need: &EffectControlTransitionNeedV1,
) -> Result<CborValue, ExecutionStoreErrorV1> {
    match control_need {
        EffectControlTransitionNeedV1::BeginReconciliation { .. } => {
            let [run] = attempt.run_set().runs() else {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            };
            ActiveStoreEffectReconciliationBeginDraftV1::new(attempt.read_plan(), run.reservation())
                .payload_value()
        }
        EffectControlTransitionNeedV1::RecordReconciliationRead { .. } => {
            Err(ExecutionStoreErrorV1::InvalidEffectSnapshot)
        }
        EffectControlTransitionNeedV1::FinishReconciliation { classification, .. } => {
            ActiveStoreEffectReconciliationTerminalDraftV1::new(*classification)
                .payload_value(
                    attempt
                        .read_usage()
                        .map(|usage| usage.result_commitment)
                        .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?,
                )
                .map_err(Into::into)
        }
        _ => Err(ExecutionStoreErrorV1::InvalidEffectSnapshot),
    }
}

fn reconciliation_occurrence_idempotency(
    phase: ReconciliationPublicationPhaseV1,
    request: &CanonicalExecutionActionRequestV1,
    meaning: [u8; 32],
    occurrence: StoreObjectIdV1,
) -> Result<[u8; 32], ExecutionStoreErrorV1> {
    let domain = phase
        .occurrence_idempotency_domain()
        .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?;
    Ok(hash(&CborValue::Array(vec![
        CborValue::text(domain)?,
        bytes(request.idempotency_key_id().as_bytes()),
        bytes(&meaning),
        bytes(occurrence.as_bytes()),
    ]))?)
}

fn load_validated_withdrawal_carrier(
    _head: &StoreHeadV1,
    generation: &StoreGenerationV1,
    object: &StoreObjectV1,
    active_objects: &[StoreObjectV1],
    intent: &EffectIntentV1,
    load_generation: &mut impl FnMut(
        StoreGenerationIdV1,
    ) -> Result<StoreGenerationV1, ExecutionStoreErrorV1>,
) -> Result<EffectWithdrawalV1, ExecutionStoreErrorV1> {
    if object.schema_id()
        != execution_schema_id("maestro.vnext.effect-withdrawal-carrier-schema.v1")?
    {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    }
    let (withdrawal, authority) = decode_effect_withdrawal_authorized_carrier(object.value())?;
    if withdrawal.intent_id() != intent.id()
        || withdrawal.provider_io_operations() != 0
        || withdrawal.creates_attempt()
        || withdrawal.creates_run()
        || withdrawal.derived_request().home
            != super::effect_home::EffectIntentHomeKindV1::ActiveStore
        || withdrawal.derived_request().path != intent.origin().withdrawal_authority_path()?
    {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    }
    let request_object = exact_referenced_schema_object(
        object,
        active_objects,
        &[execution_schema_id(
            "maestro.vnext.execution-action-request-schema.v1",
        )?],
    )?;
    let request = CanonicalExecutionActionRequestV1::from_canonical_value(request_object.value())?;
    validate_execution_authority_binding(&request, &authority)?;
    validate_effect_authority_origin(&authority, intent.origin().kind())?;
    if request.request_id() != withdrawal.control_need().action_request_id()
        || request.action() != intent.origin().withdrawal_action()?
        || request.subject_commitment() != hash(&effect_intent_subject_value(intent)?)?
        || request.payload_commitment()
            != hash(&ActiveStoreEffectWithdrawalDraftV1::new().payload_value()?)?
    {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    }
    let occurrence = exact_referenced_schema_object(
        object,
        active_objects,
        &[execution_schema_id(
            "maestro.vnext.effect-withdrawal-occurrence-schema.v1",
        )?],
    )?;
    let CborValue::Array(occurrence_fields) = occurrence.value() else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    };
    let [
        CborValue::Text(occurrence_domain),
        occurrence_request,
        current_carrier,
        occurrence_meaning,
        CborValue::Unsigned(provider_io),
    ] = occurrence_fields.as_slice()
    else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    };
    let current_carrier_id = StoreObjectIdV1::from_digest(
        exact_digest(current_carrier).ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?,
    );
    if occurrence_domain != "maestro.vnext.effect-withdrawal-occurrence.v1"
        || exact_digest(occurrence_request) != Some(*request.request_id().as_bytes())
        || *provider_io != 0
        || !occurrence.references().contains(&request_object.id())
        || !occurrence.references().contains(&current_carrier_id)
        || !object.references().contains(&current_carrier_id)
        || !active_objects
            .iter()
            .any(|candidate| candidate.id() == current_carrier_id)
    {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    }
    let meaning = hash(&CborValue::Array(vec![
        CborValue::text(EFFECT_WITHDRAWAL_IDEMPOTENCY_NAMESPACE_V1)?,
        request.canonical_value()?,
        execution_authority_value(&authority)?,
        ActiveStoreEffectWithdrawalDraftV1::new().payload_value()?,
    ]))?;
    if exact_digest(occurrence_meaning) != Some(meaning) {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    }
    let basis_ids = occurrence
        .references()
        .iter()
        .copied()
        .filter(|reference| *reference != request_object.id() && *reference != current_carrier_id)
        .collect::<Vec<_>>();
    let [basis_id] = basis_ids.as_slice() else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    };
    if !object.references().contains(basis_id) {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    }
    let basis_object = active_objects
        .iter()
        .find(|candidate| candidate.id() == *basis_id)
        .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?;
    let predecessor_id = generation
        .previous()
        .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?;
    let predecessor = load_generation(predecessor_id)?;
    if predecessor.id() != predecessor_id
        || predecessor.ordinal().checked_add(1) != Some(generation.ordinal())
        || predecessor.domain() != generation.domain()
    {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    }
    validate_persisted_repository_action_basis(
        &predecessor,
        request.request_id(),
        request_object.id(),
        &authority,
        basis_object,
        active_objects,
    )?;
    let expected_idempotency = hash(&CborValue::Array(vec![
        CborValue::text("maestro.vnext.effect-withdrawal-idempotency.v1")?,
        bytes(request.idempotency_key_id().as_bytes()),
        bytes(&meaning),
        bytes(occurrence.id().as_bytes()),
    ]))?;
    let EffectControlTransitionNeedV1::Withdraw {
        result_commitment,
        idempotency_commitment,
        ..
    } = withdrawal.control_need()
    else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    };
    if result_commitment != occurrence.id().as_bytes()
        || idempotency_commitment != &expected_idempotency
    {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    }
    Ok(withdrawal)
}

fn load_ancestor_generation(
    current: &StoreGenerationV1,
    expected: StoreGenerationIdV1,
    load: &mut impl FnMut(StoreGenerationIdV1) -> Result<StoreGenerationV1, ExecutionStoreErrorV1>,
) -> Result<StoreGenerationV1, ExecutionStoreErrorV1> {
    let mut candidate = current.clone();
    loop {
        if candidate.id() == expected {
            return Ok(candidate);
        }
        let previous_id = candidate
            .previous()
            .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?;
        let previous = load(previous_id)?;
        if previous.id() != previous_id
            || previous.domain() != current.domain()
            || previous.ordinal().checked_add(1) != Some(candidate.ordinal())
            || candidate.previous() != Some(previous.id())
        {
            return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
        }
        candidate = previous;
    }
}

fn validate_persisted_writer_term(
    generation: &StoreGenerationV1,
    head_object: &StoreObjectV1,
    writer_object: &StoreObjectV1,
    active_objects: &[StoreObjectV1],
    writer_term: EffectIntentControlWriterTermV1,
    intent_id: EffectIntentIdV1,
    load_generation: &mut impl FnMut(
        StoreGenerationIdV1,
    ) -> Result<StoreGenerationV1, ExecutionStoreErrorV1>,
) -> Result<(), ExecutionStoreErrorV1> {
    let receipt_schema =
        execution_schema_id("maestro.vnext.same-home-writer-fencing-receipt-schema.v1")?;
    match writer_term.kind() {
        super::control_head::EffectIntentControlWriterTermKindV1::Origination => {
            if writer_term.intent() != intent_id
                || writer_term.prior_writer_term().is_some()
                || writer_term.fencing_receipt().is_some()
                || head_object.references().iter().any(|reference| {
                    active_objects.iter().any(|object| {
                        object.id() == *reference && object.schema_id() == receipt_schema
                    })
                })
            {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            }
        }
        super::control_head::EffectIntentControlWriterTermKindV1::SameHomeRestore => {
            let receipt_object =
                exact_referenced_schema_object(writer_object, active_objects, &[receipt_schema])?;
            if !writer_object.references().contains(&receipt_object.id()) {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            }
            let receipt =
                SameHomeWriterFencingReceiptV1::from_canonical_value(receipt_object.value())?;
            let fence_generation = load_ancestor_generation(
                generation,
                StoreGenerationIdV1::from_digest(receipt.prior_store_generation()),
                load_generation,
            )?;
            let prior_writer_object = exact_referenced_schema_object(
                writer_object,
                active_objects,
                &[execution_schema_id(
                    "maestro.vnext.effect-intent-control-writer-term-schema.v1",
                )?],
            )?;
            let prior_writer =
                EffectIntentControlWriterTermV1::from_canonical_value(prior_writer_object.value())?;
            let prior_head_object = exact_referenced_schema_object(
                receipt_object,
                active_objects,
                &[execution_schema_id(
                    "maestro.vnext.effect-intent-control-head-schema.v1",
                )?],
            )?;
            let CborValue::Array(prior_head_fields) = prior_head_object.value() else {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            };
            let Some(prior_head_id) = prior_head_fields.first().and_then(exact_digest) else {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            };
            if writer_term.intent() != intent_id
                || writer_term.prior_writer_term() != Some(prior_writer.id())
                || writer_term.fencing_receipt() != Some(receipt.id())
                || receipt.intent() != intent_id
                || receipt.home() != writer_term.home()
                || receipt.prior_writer_term() != prior_writer.id()
                || receipt.prior_head().as_bytes() != &prior_head_id
                || receipt.prior_store_head() == [0; 32]
                || fence_generation.domain() != generation.domain()
                || fence_generation.ordinal().checked_add(1) != Some(receipt.fence_ordinal())
                || !receipt_object
                    .references()
                    .contains(&prior_writer_object.id())
            {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            }
        }
    }
    Ok(())
}

fn load_active_effect_snapshot(
    head: &StoreHeadV1,
    generation: &StoreGenerationV1,
    objects: &[StoreObjectV1],
    intent_id: EffectIntentIdV1,
    mut load_generation: impl FnMut(
        StoreGenerationIdV1,
    ) -> Result<StoreGenerationV1, ExecutionStoreErrorV1>,
) -> Result<ActiveStoreEffectSnapshotV1, ExecutionStoreErrorV1> {
    let index = load_control_index(objects)?;
    let entry = index
        .entries
        .iter()
        .find(|entry| entry.intent == intent_id)
        .ok_or(ExecutionStoreErrorV1::MissingIntentControlSelector)?;
    let head_object = objects
        .iter()
        .find(|object| object.id() == entry.control_head_object_id)
        .ok_or(ExecutionStoreErrorV1::InvalidIntentControlIndex)?;
    let intent_object = exact_referenced_schema_object(
        head_object,
        objects,
        &[execution_schema_id(
            "maestro.vnext.effect-intent-schema.v1",
        )?],
    )?;
    let revision_object = exact_referenced_schema_object(
        head_object,
        objects,
        &[execution_schema_id(
            "maestro.vnext.effect-intent-control-revision-schema.v1",
        )?],
    )?;
    let writer_object = exact_referenced_schema_object(
        head_object,
        objects,
        &[execution_schema_id(
            "maestro.vnext.effect-intent-control-writer-term-schema.v1",
        )?],
    )?;
    let dispatch_object = exact_referenced_schema_object(
        head_object,
        objects,
        &[
            execution_schema_id("maestro.vnext.effect-dispatch-reservation-carrier-schema.v1")?,
            execution_schema_id("maestro.vnext.effect-dispatch-seal-carrier-schema.v1")?,
            execution_schema_id("maestro.vnext.effect-dispatch-terminal-carrier-schema.v1")?,
        ],
    )?;
    let withdrawal_object = optional_referenced_schema_object(
        head_object,
        objects,
        &[execution_schema_id(
            "maestro.vnext.effect-withdrawal-carrier-schema.v1",
        )?],
    )?;
    let reconciliation_object = optional_referenced_schema_object(
        head_object,
        objects,
        &[
            execution_schema_id("maestro.vnext.effect-reconciliation-begin-carrier-schema.v1")?,
            execution_schema_id("maestro.vnext.effect-reconciliation-read-carrier-schema.v1")?,
            execution_schema_id("maestro.vnext.effect-reconciliation-terminal-carrier-schema.v1")?,
        ],
    )?;
    let intent = EffectIntentV1::from_persistence_value(intent_object.value())?;
    let (
        EffectIntentHomeV1::ActiveStore(active_home),
        EffectIntentOriginationFenceV1::ActiveStore(origination_fence),
    ) = (intent.home(), intent.origination_fence())
    else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    };
    let expected_domain_kind = match generation.domain().role() {
        StoreRoleV1::Repository => EffectIntentDomainKindV1::RepositoryDomain,
        StoreRoleV1::Installation => EffectIntentDomainKindV1::InstallationDomain,
    };
    if active_home.domain_kind != expected_domain_kind
        || active_home.stable_domain_id.as_bytes() != generation.domain().id().as_bytes()
        || origination_fence.store.as_bytes() != generation.domain().id().as_bytes()
    {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    }
    let origination_generation = load_ancestor_generation(
        generation,
        StoreGenerationIdV1::from_digest(*origination_fence.generation.as_bytes()),
        &mut load_generation,
    )?;
    if origination_generation.domain() != generation.domain() {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    }
    let control_revision =
        validate_control_revision_history(revision_object, objects, &intent, &mut Vec::new())?;
    let writer_term = EffectIntentControlWriterTermV1::from_canonical_value(writer_object.value())?;
    validate_persisted_writer_term(
        generation,
        head_object,
        writer_object,
        objects,
        writer_term,
        intent_id,
        &mut load_generation,
    )?;
    let health_schema =
        execution_schema_id("maestro.vnext.effect-intent-health-carrier-schema.v1")?;
    let health_carriers = revision_object
        .references()
        .iter()
        .filter_map(|reference| objects.iter().find(|object| object.id() == *reference))
        .filter(|object| object.schema_id() == health_schema)
        .collect::<Vec<_>>();
    match (
        control_revision.health() == EffectIntentControlHealthV1::Healthy,
        health_carriers.as_slice(),
    ) {
        (true, []) => {}
        (false, [carrier]) => {
            validate_persisted_control_authority(
                carrier,
                objects,
                generation,
                &mut load_generation,
                &intent,
            )?;
        }
        _ => return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot),
    }
    let handoff_schema =
        execution_schema_id("maestro.vnext.effect-writer-handoff-carrier-schema.v1")?;
    let handoff_carriers = objects
        .iter()
        .filter(|object| {
            object.schema_id() == handoff_schema
                && object.references().contains(&writer_object.id())
        })
        .collect::<Vec<_>>();
    match (writer_term.kind(), handoff_carriers.as_slice()) {
        (super::control_head::EffectIntentControlWriterTermKindV1::Origination, []) => {}
        (super::control_head::EffectIntentControlWriterTermKindV1::SameHomeRestore, [carrier]) => {
            validate_persisted_control_authority(
                carrier,
                objects,
                generation,
                &mut load_generation,
                &intent,
            )?;
        }
        _ => return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot),
    }
    let control_head = EffectIntentControlHeadV1::from_canonical_value(
        head_object.value(),
        &control_revision,
        writer_term,
    )?;
    let (dispatch, control_need, _) = load_validated_dispatch_carrier(
        dispatch_object,
        objects,
        &intent,
        generation,
        &mut load_generation,
    )?;
    let reconciliation = reconciliation_object
        .map(|object| load_validated_reconciliation_carrier(object, objects, &intent))
        .transpose()?;
    let withdrawal = withdrawal_object
        .map(|object| {
            load_validated_withdrawal_carrier(
                head,
                generation,
                object,
                objects,
                &intent,
                &mut load_generation,
            )
        })
        .transpose()?;
    let reconciliation_authorization = if let Some((attempt, need, _)) = &reconciliation {
        let EffectIntentUseFenceV1::ActiveStore(fence) = attempt.use_fence() else {
            return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
        };
        let fence_generation = load_ancestor_generation(
            generation,
            StoreGenerationIdV1::from_digest(*fence.generation.as_bytes()),
            &mut load_generation,
        )?;
        Some(validate_persisted_reconciliation_use_fence(
            &fence_generation,
            &intent,
            attempt,
            need,
            objects,
        )?)
    } else {
        None
    };
    let reconciliation_evaluated_classification = reconciliation
        .as_ref()
        .map(|(attempt, _, _)| {
            load_reconciliation_evaluated_classification(objects, &intent, attempt)
        })
        .transpose()?
        .flatten();
    let execution_is_coherent = match (&withdrawal, &reconciliation) {
        (Some(withdrawal), _) => effect_withdrawal_control_is_coherent(
            intent_id,
            &control_revision,
            &dispatch,
            reconciliation.as_ref().map(|(attempt, _, _)| attempt),
            withdrawal,
        ),
        (None, Some((reconciliation, reconciliation_need, _))) => {
            effect_reconciliation_control_is_coherent(
                intent_id,
                &control_revision,
                &dispatch,
                reconciliation,
                reconciliation_need,
            )
        }
        (None, None) => effect_dispatch_control_is_coherent(
            intent_id,
            &control_revision,
            &dispatch,
            &control_need,
        ),
    };
    if intent.id() != intent_id
        || entry.semantic_uniqueness_commitment != effect_semantic_uniqueness_commitment(&intent)?
        || !intent.matches_control_revision(&control_revision)
        || control_head.id() != entry.control_head
        || control_head.intent() != intent_id
        || !execution_is_coherent
    {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    }
    Ok(ActiveStoreEffectSnapshotV1 {
        state_binding: ExecutionStoreStateBindingV1 {
            store_head_id: head.id(),
            store_generation_id: generation.id(),
            control_head: Some(entry.control_head),
            control_index_object_id: Some(index.object.id()),
        },
        intent,
        control_revision,
        writer_term,
        control_head,
        dispatch,
        reconciliation_execution_authority: reconciliation
            .as_ref()
            .map(|(_, _, authority)| authority.clone()),
        reconciliation: reconciliation.map(|(attempt, _, _)| attempt),
        reconciliation_evaluated_classification,
        reconciliation_authorization,
    })
}

fn load_reconciliation_evaluated_classification(
    active_objects: &[StoreObjectV1],
    intent: &EffectIntentV1,
    attempt: &EffectReconciliationAttemptV1,
) -> Result<Option<super::withdrawal::RemoteClassificationV1>, ExecutionStoreErrorV1> {
    if attempt.read_usage().is_none() {
        return Ok(None);
    }
    let read_schema =
        execution_schema_id("maestro.vnext.effect-reconciliation-read-carrier-schema.v1")?;
    let matching = active_objects
        .iter()
        .filter(|object| object.schema_id() == read_schema)
        .filter_map(|object| {
            decode_effect_reconciliation_authorized_carrier(object.value())
                .ok()
                .filter(|(candidate, _, _)| candidate.attempt().id() == attempt.attempt().id())
                .map(|_| object)
        })
        .collect::<Vec<_>>();
    let [read_carrier] = matching.as_slice() else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    };
    let (validated_attempt, need, _) =
        load_validated_reconciliation_carrier(read_carrier, active_objects, intent)?;
    if validated_attempt != *attempt
        || !matches!(
            need,
            EffectControlTransitionNeedV1::RecordReconciliationRead { .. }
        )
    {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    }
    let occurrence = exact_referenced_schema_object(
        read_carrier,
        active_objects,
        &[execution_schema_id(
            "maestro.vnext.effect-reconciliation-read-occurrence-schema.v1",
        )?],
    )?;
    let CborValue::Array(fields) = occurrence.value() else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    };
    let Some(CborValue::Unsigned(classification)) = fields.get(3) else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    };
    Ok(Some(parse_remote_classification_store_tag(
        *classification,
    )?))
}

fn validate_persisted_reconciliation_use_fence(
    fence_generation: &StoreGenerationV1,
    intent: &EffectIntentV1,
    reconciliation: &EffectReconciliationAttemptV1,
    _control_need: &EffectControlTransitionNeedV1,
    active_objects: &[StoreObjectV1],
) -> Result<AuthorizedExecutionActionV1, ExecutionStoreErrorV1> {
    let EffectIntentHomeV1::ActiveStore(home) = intent.home() else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    };
    let EffectIntentUseFenceV1::ActiveStore(fence) = reconciliation.use_fence() else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    };
    let begin_schema =
        execution_schema_id("maestro.vnext.effect-reconciliation-begin-carrier-schema.v1")?;
    let begin_carriers = active_objects
        .iter()
        .filter(|object| object.schema_id() == begin_schema)
        .filter_map(|object| {
            decode_effect_reconciliation_authorized_carrier(object.value())
                .ok()
                .filter(|(attempt, _, _)| attempt.attempt().id() == reconciliation.attempt().id())
                .map(|_| object)
        })
        .collect::<Vec<_>>();
    let [begin_carrier] = begin_carriers.as_slice() else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    };
    let (_, _, authority) = decode_effect_reconciliation_authorized_carrier(begin_carrier.value())?;
    let request_object = exact_referenced_schema_object(
        begin_carrier,
        active_objects,
        &[execution_schema_id(
            "maestro.vnext.execution-action-request-schema.v1",
        )?],
    )?;
    let request = CanonicalExecutionActionRequestV1::from_canonical_value(request_object.value())?;
    let expected_attempt_fence = hash(&CborValue::Array(vec![
        CborValue::text("maestro.vnext.effect-reconciliation-attempt-fence.v1")?,
        bytes(intent.id().as_bytes()),
        bytes(request.request_id().as_bytes()),
    ]))?;
    let authority_object_id = StoreObjectIdV1::from_digest(*fence.authority.as_bytes());
    let authority_object = active_objects
        .iter()
        .find(|object| object.id() == authority_object_id)
        .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?;
    let validated_basis = validate_persisted_repository_action_basis(
        fence_generation,
        request.request_id(),
        request_object.id(),
        &authority,
        authority_object,
        active_objects,
    )?;
    let expected_epoch = hash(&CborValue::Unsigned(validated_basis.authority_epoch()))?;
    if request.request_id() != reconciliation.attempt().action_request_id()
        || !begin_carrier.references().contains(&authority_object_id)
        || fence.same_stable_home != home.stable_domain_id
        || fence.same_stable_home.as_bytes() != fence_generation.domain().id().as_bytes()
        || fence.generation.as_bytes() != fence_generation.id().as_bytes()
        || fence.authority.as_bytes() != authority_object.id().as_bytes()
        || fence.epoch.as_bytes() != &expected_epoch
        || fence.namespace != home.semantic_namespace
        || fence.material_token.as_bytes() != intent.material_inputs().as_bytes()
        || fence.credentials.as_bytes() != intent.credential_requirements().as_bytes()
        || fence.attempt_fence.as_bytes() != &expected_attempt_fence
        || fence.idempotency_binding.as_bytes() != request.idempotency_key_id().as_bytes()
        || fence.provider_contract_guards.as_bytes() != &reconciliation.read_plan_commitment()?
    {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    }
    Ok(AuthorizedExecutionActionV1::new(
        request,
        validated_basis.authorization_receipt().clone(),
    )?)
}

fn effect_reconciliation_control_is_coherent(
    intent_id: EffectIntentIdV1,
    control_revision: &EffectIntentControlRevisionV1,
    dispatch: &EffectDispatchAttemptV1,
    reconciliation: &EffectReconciliationAttemptV1,
    control_need: &EffectControlTransitionNeedV1,
) -> bool {
    let owner =
        super::runtime::ExecutionAttemptOwnerV1::Reconciliation(reconciliation.attempt().id());
    let parts = control_revision.parts();
    let phase_is_coherent = match control_need {
        EffectControlTransitionNeedV1::BeginReconciliation {
            next_run_set_revision,
            next_use_fence_commitment,
            ..
        } => {
            control_revision.live_attempt() == Some(owner)
                && !control_revision.runs_closed()
                && control_revision.classification() == reconciliation.starting_classification()
                && *next_run_set_revision == reconciliation.run_set().revision()
                && *next_use_fence_commitment == reconciliation.use_fence_commitment()
                && parts.result_commitment.is_none()
                && parts.idempotency_commitment.is_none()
                && reconciliation.read_usage().is_none()
        }
        EffectControlTransitionNeedV1::RecordReconciliationRead {
            next_run_set_revision,
            result_commitment,
            idempotency_commitment,
            ..
        } => {
            control_revision.live_attempt() == Some(owner)
                && !control_revision.runs_closed()
                && control_revision.classification() == reconciliation.starting_classification()
                && *next_run_set_revision == reconciliation.run_set().revision()
                && parts.result_commitment == Some(*result_commitment)
                && parts.idempotency_commitment == Some(*idempotency_commitment)
                && reconciliation.run_set().all_terminal()
                && reconciliation.read_usage().is_some()
        }
        EffectControlTransitionNeedV1::FinishReconciliation {
            classification,
            next_run_set_revision,
            result_commitment,
            idempotency_commitment,
            ..
        } => {
            control_revision.live_attempt().is_none()
                && control_revision.runs_closed()
                && control_revision.classification() == *classification
                && *next_run_set_revision == reconciliation.run_set().revision()
                && parts.result_commitment == Some(*result_commitment)
                && parts.idempotency_commitment == Some(*idempotency_commitment)
                && reconciliation.run_set().all_terminal()
                && reconciliation.read_usage().is_some()
        }
        _ => false,
    };
    reconciliation.attempt().effect_intent_id() == intent_id
        && control_revision.attempt_history().contains(&owner)
        && control_revision.live_dispatch() == super::withdrawal::EffectIntentLiveDispatchV1::None
        && control_revision.run_set_revision() == reconciliation.run_set().revision()
        && parts.use_fence_commitment == reconciliation.use_fence_commitment()
        && matches!(
            dispatch.state(),
            super::dispatch_state::DispatchAttemptStateV1::Terminal(_)
        )
        && dispatch.attempt().effect_intent_id() == intent_id
        && control_revision.attempt_history().contains(
            &super::runtime::ExecutionAttemptOwnerV1::Dispatch(dispatch.attempt().id()),
        )
        && control_revision.dispatch_fence_high_water() == dispatch.attempt().dispatch_fence()
        && phase_is_coherent
}

fn effect_withdrawal_control_is_coherent(
    intent_id: EffectIntentIdV1,
    control_revision: &EffectIntentControlRevisionV1,
    dispatch: &EffectDispatchAttemptV1,
    reconciliation: Option<&EffectReconciliationAttemptV1>,
    withdrawal: &EffectWithdrawalV1,
) -> bool {
    let dispatch_owner = super::runtime::ExecutionAttemptOwnerV1::Dispatch(dispatch.attempt().id());
    let prior_run_set_revision = reconciliation
        .map(EffectReconciliationAttemptV1::run_set)
        .unwrap_or_else(|| dispatch.run_set())
        .revision();
    let prior_execution_is_terminal = matches!(
        dispatch.state(),
        super::dispatch_state::DispatchAttemptStateV1::Terminal(_)
    ) && dispatch.attempt().effect_intent_id() == intent_id
        && dispatch.run_set().all_terminal()
        && control_revision.attempt_history().contains(&dispatch_owner)
        && control_revision.dispatch_fence_high_water() == dispatch.attempt().dispatch_fence()
        && reconciliation.is_none_or(|attempt| {
            let owner =
                super::runtime::ExecutionAttemptOwnerV1::Reconciliation(attempt.attempt().id());
            attempt.attempt().effect_intent_id() == intent_id
                && attempt.run_set().all_terminal()
                && attempt.read_usage().is_some()
                && control_revision.attempt_history().contains(&owner)
        });
    let EffectControlTransitionNeedV1::Withdraw {
        next_run_set_revision,
        result_commitment,
        idempotency_commitment,
        ..
    } = withdrawal.control_need()
    else {
        return false;
    };
    let parts = control_revision.parts();
    withdrawal.intent_id() == intent_id
        && prior_execution_is_terminal
        && prior_run_set_revision.checked_add(1) == Some(*next_run_set_revision)
        && control_revision.run_set_revision() == *next_run_set_revision
        && control_revision.live_attempt().is_none()
        && control_revision.live_dispatch() == super::withdrawal::EffectIntentLiveDispatchV1::None
        && control_revision.classification() == super::withdrawal::RemoteClassificationV1::Cancelled
        && control_revision.runs_closed()
        && parts.result_commitment == Some(*result_commitment)
        && parts.idempotency_commitment == Some(*idempotency_commitment)
}

fn effect_dispatch_control_is_coherent(
    intent_id: EffectIntentIdV1,
    control_revision: &EffectIntentControlRevisionV1,
    dispatch: &EffectDispatchAttemptV1,
    control_need: &EffectControlTransitionNeedV1,
) -> bool {
    let dispatch_owner = super::runtime::ExecutionAttemptOwnerV1::Dispatch(dispatch.attempt().id());
    let control_parts = control_revision.parts();
    let phase_is_coherent = match (dispatch.state(), control_need) {
        (
            super::dispatch_state::DispatchAttemptStateV1::ReservedUnsealed(_),
            EffectControlTransitionNeedV1::ReserveDispatch { .. }
            | EffectControlTransitionNeedV1::RedispatchConclusiveNotApplied { .. },
        ) => {
            control_revision.live_attempt() == Some(dispatch_owner)
                && control_revision.live_dispatch()
                    == super::withdrawal::EffectIntentLiveDispatchV1::Reserved
                && control_revision.classification()
                    == super::withdrawal::RemoteClassificationV1::Dispatching
                && !control_revision.runs_closed()
        }
        (
            super::dispatch_state::DispatchAttemptStateV1::SealedInFlight(_),
            EffectControlTransitionNeedV1::SealDispatch { .. },
        ) => {
            control_revision.live_attempt() == Some(dispatch_owner)
                && control_revision.live_dispatch()
                    == super::withdrawal::EffectIntentLiveDispatchV1::Sealed
                && control_revision.classification()
                    == super::withdrawal::RemoteClassificationV1::InDoubt
                && !control_revision.runs_closed()
        }
        (
            super::dispatch_state::DispatchAttemptStateV1::Terminal(_),
            EffectControlTransitionNeedV1::FinishDispatch {
                classification,
                result_commitment,
                idempotency_commitment,
                ..
            },
        ) => {
            control_revision.live_attempt().is_none()
                && control_revision.live_dispatch()
                    == super::withdrawal::EffectIntentLiveDispatchV1::None
                && control_revision.classification() == *classification
                && control_revision.runs_closed()
                && control_parts.result_commitment == Some(*result_commitment)
                && control_parts.idempotency_commitment == Some(*idempotency_commitment)
        }
        (
            super::dispatch_state::DispatchAttemptStateV1::Terminal(_),
            EffectControlTransitionNeedV1::RecoverSealedInDoubt {
                result_commitment,
                idempotency_commitment,
                ..
            },
        ) => {
            control_revision.live_attempt().is_none()
                && control_revision.live_dispatch()
                    == super::withdrawal::EffectIntentLiveDispatchV1::None
                && control_revision.classification()
                    == super::withdrawal::RemoteClassificationV1::InDoubt
                && control_revision.runs_closed()
                && matches!(
                    control_revision.health(),
                    EffectIntentControlHealthV1::RecoveryRequired
                        | EffectIntentControlHealthV1::Healthy
                )
                && control_parts.result_commitment == Some(*result_commitment)
                && control_parts.idempotency_commitment == Some(*idempotency_commitment)
        }
        _ => false,
    };
    dispatch.attempt().effect_intent_id() == intent_id
        && control_revision.attempt_history().contains(&dispatch_owner)
        && control_revision.dispatch_fence_high_water() == dispatch.attempt().dispatch_fence()
        && control_revision.run_set_revision() == dispatch.run_set().revision()
        && control_parts.use_fence_commitment == dispatch.use_fence_commitment()
        && phase_is_coherent
}

fn validate_current_effect_snapshot(
    store: &StoreV1,
    expected: &ActiveStoreEffectSnapshotV1,
) -> Result<(), ExecutionStoreErrorV1> {
    let (state, head, generation, objects) = store.coherent_publication_snapshot()?;
    if state != StoreStateV1::Active {
        return Err(ExecutionStoreErrorV1::InactiveStore);
    }
    if head.id() != expected.state_binding.store_head_id()
        || generation.id() != expected.state_binding.store_generation_id()
    {
        return Err(ExecutionStoreErrorV1::StaleExpectedStoreState);
    }
    let current = load_active_effect_snapshot(
        &head,
        &generation,
        &objects,
        expected.intent.id(),
        |generation_id| Ok(store.generation(generation_id)?),
    )?;
    if current != *expected {
        return Err(ExecutionStoreErrorV1::StaleExpectedStoreState);
    }
    Ok(())
}

fn effect_intent_subject_value(
    intent: &EffectIntentV1,
) -> Result<CborValue, ExecutionStoreErrorV1> {
    let EffectIntentHomeV1::ActiveStore(home) = intent.home() else {
        return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
    };
    effect_authority_subject_value(
        home.domain_kind,
        home.stable_domain_id,
        home.realm,
        home.semantic_namespace,
        home.home_qualified_semantic_uniqueness_namespace,
        intent.semantic_use(),
    )
    .map_err(Into::into)
}

fn effect_writer_handoff_action(
    snapshot: &ActiveStoreEffectSnapshotV1,
) -> Result<ExecutionActionV1, ExecutionStoreErrorV1> {
    Ok(snapshot.intent.origin().reconciliation_action()?)
}

fn effect_authority_subject_value(
    domain_kind: EffectIntentDomainKindV1,
    stable_domain_id: HomeTokenV1,
    realm: HomeTokenV1,
    semantic_namespace: HomeTokenV1,
    uniqueness_namespace: HomeTokenV1,
    semantic_use: EffectSemanticUseV1,
) -> Result<CborValue, CborError> {
    Ok(CborValue::Array(vec![
        CborValue::text("maestro.vnext.effect-intent-authority-subject.v1")?,
        CborValue::Unsigned(match domain_kind {
            EffectIntentDomainKindV1::RepositoryDomain => 1,
            EffectIntentDomainKindV1::InstallationDomain => 2,
        }),
        bytes(stable_domain_id.as_bytes()),
        bytes(realm.as_bytes()),
        bytes(semantic_namespace.as_bytes()),
        bytes(uniqueness_namespace.as_bytes()),
        bytes(semantic_use.as_bytes()),
    ]))
}

fn build_control_index_object(
    entries: &[ControlIndexEntryV1],
) -> Result<StoreObjectV1, ExecutionStoreErrorV1> {
    let mut entries = entries.to_vec();
    entries.sort_unstable();
    if entries.is_empty()
        || entries
            .windows(2)
            .any(|pair| pair[0].intent == pair[1].intent)
        || has_duplicate_control_index_semantic_selector(&entries)
    {
        return Err(ExecutionStoreErrorV1::InvalidIntentControlIndex);
    }
    let mut references = entries
        .iter()
        .map(|entry| entry.control_head_object_id)
        .collect::<Vec<_>>();
    references.sort_unstable();
    references.dedup();
    Ok(StoreObjectV1::new(
        execution_schema_id("maestro.vnext.effect-intent-control-index-schema.v1")?,
        CborValue::Array(vec![
            CborValue::text("maestro.vnext.effect-intent-control-index.v1")?,
            CborValue::Array(
                entries
                    .iter()
                    .map(|entry| {
                        CborValue::Array(vec![
                            bytes(entry.intent.as_bytes()),
                            bytes(&entry.semantic_uniqueness_commitment),
                            bytes(entry.control_head.as_bytes()),
                            bytes(entry.control_head_object_id.as_bytes()),
                        ])
                    })
                    .collect(),
            ),
        ]),
        references,
    )?)
}

fn has_duplicate_control_index_semantic_selector(entries: &[ControlIndexEntryV1]) -> bool {
    let mut selectors = entries
        .iter()
        .map(|entry| entry.semantic_uniqueness_commitment)
        .collect::<Vec<_>>();
    selectors.sort_unstable();
    selectors.windows(2).any(|pair| pair[0] == pair[1])
}

fn validate_request_values(
    request: &CanonicalExecutionActionRequestV1,
    subject: &CborValue,
    expected_state: &CborValue,
    payload: &CborValue,
) -> Result<(), ExecutionStoreErrorV1> {
    if request.subject_commitment() != hash(subject)?
        || request.expected_state_commitment() != hash(expected_state)?
        || request.payload_commitment() != hash(payload)?
    {
        return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
    }
    Ok(())
}

fn validate_execution_authority_binding(
    request: &CanonicalExecutionActionRequestV1,
    authority: &ExecutionAuthorityV1,
) -> Result<(), ExecutionStoreErrorV1> {
    if authority.action() != repository_leaf_for_execution_action(request.action())
        || authority.subject_commitment() != request.subject_commitment()
        || authority.current_state_commitment() != request.expected_state_commitment()
        || authority.exact_payload_commitment() != request.payload_commitment()
    {
        return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
    }
    Ok(())
}

fn validate_effect_authority_origin(
    authority: &ExecutionAuthorityV1,
    origin: EffectOriginKindV1,
) -> Result<(), ExecutionStoreErrorV1> {
    let expected_purpose = match origin {
        EffectOriginKindV1::TrustedTimeAcquisitionEffectOrigin => {
            Some(CmaObservationPublicationPurposeV1::TrustedTimeAcquisition)
        }
        EffectOriginKindV1::RecoveryExternalRegistrationEffectOrigin => {
            Some(CmaObservationPublicationPurposeV1::RecoveryExternalRegistration)
        }
        EffectOriginKindV1::RecoveryExternalStatusEffectOrigin => {
            Some(CmaObservationPublicationPurposeV1::RecoveryExternalStatus)
        }
        EffectOriginKindV1::MaintenanceExecutorCurrentnessEffectOrigin => {
            Some(CmaObservationPublicationPurposeV1::MaintenanceExecutorCurrentness)
        }
        EffectOriginKindV1::ProspectiveContinuityCarrierEffectOrigin => {
            Some(CmaObservationPublicationPurposeV1::ProspectiveContinuityCarrier)
        }
        _ => None,
    };
    match (authority, expected_purpose) {
        (ExecutionAuthorityV1::ContinuityMaintenance(value), Some(expected))
            if value.purpose() == expected =>
        {
            Ok(())
        }
        (ExecutionAuthorityV1::ContinuityMaintenance(_), _) | (_, Some(_)) => {
            Err(ExecutionStoreErrorV1::PublicationBindingMismatch)
        }
        (_, None) => Ok(()),
    }
}

fn execution_authority_value(authority: &ExecutionAuthorityV1) -> Result<CborValue, CborError> {
    let mut fields = vec![
        CborValue::text("maestro.vnext.execution-authority.v1")?,
        CborValue::Unsigned(authority.basis_kind() as u64),
        CborValue::text(authority.action().literal())?,
        bytes(authority.subject_commitment().as_slice()),
        bytes(authority.current_state_commitment().as_slice()),
        bytes(authority.exact_payload_commitment().as_slice()),
        bytes(authority.executor_principal_id().as_bytes()),
    ];
    match authority {
        ExecutionAuthorityV1::Ordinary(value) => {
            let selection = value.selection();
            fields.extend([
                bytes(selection.actor_binding_id().as_bytes()),
                bytes(selection.actor_session_id().as_bytes()),
                bytes(selection.terminal_grant_id().as_bytes()),
            ]);
        }
        ExecutionAuthorityV1::BootstrapG0(value) => {
            let basis = (*value).basis();
            fields.extend([
                bytes(basis.binding_id.as_bytes()),
                bytes(basis.session_id.as_bytes()),
                bytes(basis.genesis_grant_id.as_bytes()),
            ]);
        }
        ExecutionAuthorityV1::ContinuityMaintenance(value) => {
            let basis = (*value).basis();
            fields.extend([
                bytes(basis.cma_branch_id.as_bytes()),
                bytes(basis.slot_id.as_bytes()),
                bytes(basis.executor_assertion_id.as_bytes()),
                CborValue::optional(
                    (*value)
                        .withdrawal_slot_family()
                        .map(|family| CborValue::Unsigned(family as u64)),
                ),
                CborValue::Unsigned((*value).purpose() as u64),
                bytes((*value).continuity_state_token().as_bytes()),
                bytes((*value).continuity_state_object_id().as_bytes()),
                bytes((*value).guard_object_id().as_bytes()),
                CborValue::Unsigned((*value).authority_epoch()),
                bytes(&(*value).job_applicability_commitment()),
            ]);
        }
    }
    Ok(CborValue::Array(fields))
}

fn effect_dispatch_authorized_carrier_value(
    carrier: CborValue,
    authority: &ExecutionAuthorityV1,
) -> Result<CborValue, ExecutionStoreErrorV1> {
    Ok(CborValue::Array(vec![
        CborValue::text(EFFECT_DISPATCH_AUTHORIZED_CARRIER_DOMAIN_V1)?,
        carrier,
        execution_authority_value(authority)?,
    ]))
}

fn decode_effect_dispatch_authorized_carrier(
    value: &CborValue,
) -> Result<
    (
        EffectDispatchAttemptV1,
        EffectControlTransitionNeedV1,
        ExecutionAuthorityV1,
    ),
    ExecutionStoreErrorV1,
> {
    let CborValue::Array(fields) = value else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    };
    let [CborValue::Text(domain), carrier, authority] = fields.as_slice() else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    };
    if domain != EFFECT_DISPATCH_AUTHORIZED_CARRIER_DOMAIN_V1 {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    }
    let (attempt, need) = EffectDispatchAttemptV1::from_persistence_carrier_value(carrier)?;
    let authority = decode_execution_authority_value(authority)?;
    Ok((attempt, need, authority))
}

fn effect_reconciliation_authorized_carrier_value(
    carrier: CborValue,
    authority: &ExecutionAuthorityV1,
) -> Result<CborValue, ExecutionStoreErrorV1> {
    Ok(CborValue::Array(vec![
        CborValue::text(EFFECT_RECONCILIATION_AUTHORIZED_CARRIER_DOMAIN_V1)?,
        carrier,
        execution_authority_value(authority)?,
    ]))
}

fn decode_effect_reconciliation_authorized_carrier(
    value: &CborValue,
) -> Result<
    (
        EffectReconciliationAttemptV1,
        EffectControlTransitionNeedV1,
        ExecutionAuthorityV1,
    ),
    ExecutionStoreErrorV1,
> {
    let CborValue::Array(fields) = value else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    };
    let [CborValue::Text(domain), carrier, authority] = fields.as_slice() else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    };
    if domain != EFFECT_RECONCILIATION_AUTHORIZED_CARRIER_DOMAIN_V1 {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    }
    let (attempt, need) = EffectReconciliationAttemptV1::from_persistence_carrier_value(carrier)?;
    let authority = decode_execution_authority_value(authority)?;
    Ok((attempt, need, authority))
}

fn decode_execution_authority_value(
    authority: &CborValue,
) -> Result<ExecutionAuthorityV1, ExecutionStoreErrorV1> {
    let CborValue::Array(authority_fields) = authority else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    };
    let [
        CborValue::Text(authority_domain),
        CborValue::Unsigned(basis_kind),
        CborValue::Text(action),
        subject,
        current_state,
        payload,
        executor,
        rest @ ..,
    ] = authority_fields.as_slice()
    else {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    };
    if authority_domain != "maestro.vnext.execution-authority.v1" {
        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
    }
    let action = RepositoryActionLeafV1::ALL
        .into_iter()
        .find(|candidate| candidate.literal() == action)
        .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?;
    let subject = exact_digest(subject).ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?;
    let current_state =
        exact_digest(current_state).ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?;
    let payload = exact_digest(payload).ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?;
    let executor = PrincipalIdV1::from_digest(
        exact_digest(executor).ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?,
    );
    match (*basis_kind, rest) {
        (1, [actor_binding, actor_session, terminal_grant]) => GenericExecutionAuthorityV1::new(
            RepositoryAuthoritySelectionV1::new(
                PrincipalBindingIdV1::from_digest(
                    exact_digest(actor_binding)
                        .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?,
                ),
                SessionIdV1::from_digest(
                    exact_digest(actor_session)
                        .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?,
                ),
                GrantIdV1::from_digest(
                    exact_digest(terminal_grant)
                        .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?,
                ),
            ),
            action,
            subject,
            current_state,
            payload,
            executor,
        )
        .map(Into::into)
        .map_err(|_| ExecutionStoreErrorV1::InvalidEffectSnapshot),
        (2, [binding, session, genesis_grant]) => BootstrapExecutionAuthorityV1::new(
            BootstrapControlG0AuthorityBasisV1::new(
                PrincipalBindingIdV1::from_digest(
                    exact_digest(binding).ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?,
                ),
                SessionIdV1::from_digest(
                    exact_digest(session).ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?,
                ),
                GenesisGrantIdV1::from_digest(
                    exact_digest(genesis_grant)
                        .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?,
                ),
            ),
            action,
            subject,
            current_state,
            payload,
            executor,
        )
        .map(Into::into)
        .map_err(|_| ExecutionStoreErrorV1::InvalidEffectSnapshot),
        (
            3,
            [
                branch,
                slot,
                assertion,
                family,
                CborValue::Unsigned(purpose),
                continuity_state_token,
                continuity_state_object_id,
                guard_object_id,
                CborValue::Unsigned(authority_epoch),
                job_applicability_commitment,
            ],
        ) => {
            let family = match family {
                CborValue::Array(values) if values.as_slice() == [CborValue::Unsigned(0)] => None,
                CborValue::Array(values) if values.len() == 2 => {
                    let [CborValue::Unsigned(1), CborValue::Unsigned(tag)] = values.as_slice()
                    else {
                        return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
                    };
                    let tag = u8::try_from(*tag)
                        .map_err(|_| ExecutionStoreErrorV1::InvalidEffectSnapshot)?;
                    Some(
                        CmaEffectWithdrawalSlotFamilyV1::try_from(tag)
                            .map_err(|_| ExecutionStoreErrorV1::InvalidEffectSnapshot)?,
                    )
                }
                _ => return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot),
            };
            let decoded = ContinuityMaintenanceExecutionAuthorityV1::new(
                ContinuityMaintenanceAuthorityBasisV1::new(
                    CmaBranchIdV1::from_digest(
                        exact_digest(branch).ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?,
                    ),
                    SlotIdV1::from_digest(
                        exact_digest(slot).ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?,
                    ),
                    ExecutorAssertionIdV1::from_digest(
                        exact_digest(assertion)
                            .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?,
                    ),
                ),
                family,
                CmaObservationPublicationPurposeV1::try_from(
                    u8::try_from(*purpose)
                        .map_err(|_| ExecutionStoreErrorV1::InvalidEffectSnapshot)?,
                )
                .map_err(|_| ExecutionStoreErrorV1::InvalidEffectSnapshot)?,
                action,
                subject,
                current_state,
                payload,
                executor,
                StateTokenIdV1::from_digest(
                    exact_digest(continuity_state_token)
                        .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?,
                ),
                StoreObjectIdV1::from_digest(
                    exact_digest(continuity_state_object_id)
                        .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?,
                ),
                StoreObjectIdV1::from_digest(
                    exact_digest(guard_object_id)
                        .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?,
                ),
                *authority_epoch,
                exact_digest(job_applicability_commitment)
                    .ok_or(ExecutionStoreErrorV1::InvalidEffectSnapshot)?,
            )
            .map(ExecutionAuthorityV1::from)
            .map_err(|_| ExecutionStoreErrorV1::InvalidEffectSnapshot)?;
            if execution_authority_value(&decoded)? != *authority {
                return Err(ExecutionStoreErrorV1::InvalidEffectSnapshot);
            }
            Ok(decoded)
        }
        _ => Err(ExecutionStoreErrorV1::InvalidEffectSnapshot),
    }
}

fn execution_schema_id(domain: &str) -> Result<SchemaIdV1, IdentityError> {
    derive_identity(&CborValue::Text(domain.to_owned()))
}

fn repository_leaf_for_execution_action(action: ExecutionActionV1) -> RepositoryActionLeafV1 {
    match action {
        ExecutionActionV1::AcquireStepExecution => RepositoryActionLeafV1::AcquireStepExecution,
        ExecutionActionV1::RenewStepLeaseTerm => RepositoryActionLeafV1::RenewStepLeaseTerm,
        ExecutionActionV1::AbandonStepAttempt => RepositoryActionLeafV1::AbandonStepAttempt,
        ExecutionActionV1::OriginateEffectIntent => RepositoryActionLeafV1::OriginateEffectIntent,
        ExecutionActionV1::OriginateCoordinationDelivery => {
            RepositoryActionLeafV1::OriginateCoordinationDelivery
        }
        ExecutionActionV1::RecordDispatchOutcome => RepositoryActionLeafV1::RecordDispatchOutcome,
        ExecutionActionV1::ReconcileEffectIntent => RepositoryActionLeafV1::ReconcileEffectIntent,
        ExecutionActionV1::ReserveBootstrapMandateInteractionEffect => {
            RepositoryActionLeafV1::ReserveBootstrapMandateInteractionEffect
        }
        ExecutionActionV1::PublishBootstrapMandateInteractionOutcome => {
            RepositoryActionLeafV1::PublishBootstrapMandateInteractionOutcome
        }
        ExecutionActionV1::ReconcileBootstrapMandateInteractionEffect => {
            RepositoryActionLeafV1::ReconcileBootstrapMandateInteractionEffect
        }
        ExecutionActionV1::ReserveContinuityMaintenanceEffect => {
            RepositoryActionLeafV1::ReserveContinuityMaintenanceEffect
        }
        ExecutionActionV1::PublishContinuityMaintenanceEffectOutcome => {
            RepositoryActionLeafV1::PublishContinuityMaintenanceEffectOutcome
        }
        ExecutionActionV1::ReconcileContinuityMaintenanceEffect => {
            RepositoryActionLeafV1::ReconcileContinuityMaintenanceEffect
        }
        ExecutionActionV1::WithdrawEffectIntent => RepositoryActionLeafV1::WithdrawEffectIntent,
        ExecutionActionV1::WithdrawBootstrapMandateInteractionEffect => {
            RepositoryActionLeafV1::WithdrawBootstrapMandateInteractionEffect
        }
        ExecutionActionV1::WithdrawContinuityMaintenanceEffect => {
            RepositoryActionLeafV1::WithdrawContinuityMaintenanceEffect
        }
    }
}

fn step_binding_store_value(binding: StepBindingV1) -> CborValue {
    CborValue::Array(vec![
        bytes(binding.scope().repository_id().as_bytes()),
        bytes(binding.scope().work_id().as_bytes()),
        bytes(binding.contract_generation_id().as_bytes()),
        bytes(binding.contract_root_id().as_bytes()),
        bytes(binding.step_id().as_bytes()),
        bytes(binding.revision_id().as_bytes()),
    ])
}

fn step_terminal_tag(terminal: StepAttemptTerminalV1) -> u64 {
    match terminal {
        StepAttemptTerminalV1::Submitted => 1,
        StepAttemptTerminalV1::Yielded => 2,
        StepAttemptTerminalV1::Failed => 3,
        StepAttemptTerminalV1::Cancelled => 4,
        StepAttemptTerminalV1::TimedOut => 5,
        StepAttemptTerminalV1::Lost => 6,
        StepAttemptTerminalV1::Fenced => 7,
    }
}

fn run_state_store_tag(state: RunStateV1) -> u64 {
    match state {
        RunStateV1::Reserved => 1,
        RunStateV1::Active => 2,
        RunStateV1::DefinitelyNotStarted => 3,
        RunStateV1::Succeeded => 4,
        RunStateV1::Failed => 5,
        RunStateV1::Cancelled => 6,
        RunStateV1::TimedOut => 7,
        RunStateV1::Lost => 8,
        RunStateV1::Fenced => 9,
    }
}

fn run_reservation_store_value(reservation: &RunReservationV1) -> CborValue {
    CborValue::Array(vec![
        bytes(&reservation.semantic_operation_hash),
        bytes(&reservation.inputs_commitment),
        bytes(&reservation.environment_commitment),
        bytes(&reservation.target_commitment),
        bytes(&reservation.execution_boundary_commitment),
        CborValue::Unsigned(reservation.deadline),
        CborValue::Unsigned(u64::from(reservation.launch_ordinal)),
        CborValue::optional(
            reservation
                .current_step_term
                .map(|term| bytes(term.as_bytes())),
        ),
    ])
}

fn exact_digest(value: &CborValue) -> Option<[u8; 32]> {
    let CborValue::Bytes(bytes) = value else {
        return None;
    };
    bytes.as_slice().try_into().ok()
}

fn hash(value: &CborValue) -> Result<[u8; 32], CborError> {
    Ok(Sha256::digest(deterministic_cbor::encode(value)?).into())
}

fn bytes(value: &[u8]) -> CborValue {
    CborValue::Bytes(value.to_vec())
}

fn map_store_error(error: StoreError) -> ExecutionStoreErrorV1 {
    match error {
        StoreError::HeadCasMismatch => ExecutionStoreErrorV1::StaleExpectedStoreState,
        error => ExecutionStoreErrorV1::Store(error),
    }
}

#[derive(Debug, Error)]
pub enum ExecutionStoreErrorV1 {
    #[error("Execution Store is not active")]
    InactiveStore,
    #[error("Execution publication expected-old Store state is stale")]
    StaleExpectedStoreState,
    #[error("Execution request does not bind the concrete subject, current state, and payload")]
    PublicationBindingMismatch,
    #[error("Authorization Receipt belongs to another Action Request")]
    AuthorizationRequestMismatch,
    #[error("Authorization Receipt is absent from the active Store closure")]
    AuthorizationReceiptNotActive,
    #[error("active Store closure contains ambiguous Authorization Receipts")]
    AmbiguousAuthorizationReceipt,
    #[error("active Authorization Receipt or Result is malformed or not current")]
    InvalidAuthorizationReceipt,
    #[error("active Store has no Effect Intent control index")]
    MissingIntentControlIndex,
    #[error("active Store contains more than one Effect Intent control index")]
    AmbiguousIntentControlIndex,
    #[error("active Effect Intent control index is malformed or has invalid references")]
    InvalidIntentControlIndex,
    #[error("Effect Intent has no canonical current control Head selector")]
    MissingIntentControlSelector,
    #[error("a live Effect Intent already owns the same home-qualified semantic operation")]
    LiveSemanticEffectAlreadyExists,
    #[error("Step Binding belongs to a different Repository Store")]
    StepBindingStoreMismatch,
    #[error("generation-scoped Step Binding is not the exact current rooted Step state")]
    StepBindingNotCurrent,
    #[error("Step execution mutation requires the exact current open Step Binding")]
    StepBindingNotOpen,
    #[error("active Store contains more than one Step execution index")]
    AmbiguousStepExecutionIndex,
    #[error("active Step execution index is malformed or has invalid references")]
    InvalidStepExecutionIndex,
    #[error("active Store has no Step execution index")]
    MissingStepExecutionIndex,
    #[error("Step execution mutation requires an exact current Lease/Attempt/Run carrier")]
    MissingStepExecutionCarrier,
    #[error("a live Step execution already owns the exact generation-scoped Step Binding")]
    LiveStepExecutionAlreadyExists,
    #[error("a successor Step execution pair requires pinned takeover-safety proof")]
    TakeoverSafetyRequired,
    #[error("initial Step execution acquisition cannot claim predecessor takeover safety")]
    UnexpectedTakeoverSafety,
    #[error("Step execution mutation time does not equal current accepted Authority H_time")]
    UntrustedMutationTime,
    #[error("Step execution fence high-water overflowed")]
    FenceOverflow,
    #[error("Step execution publication result does not bind exactly one carrier")]
    InvalidStepExecutionPublicationResult,
    #[error(
        "Step Submission publication result is malformed or does not bind its atomic owner join"
    )]
    InvalidStepSubmissionResult,
    #[error("Effect Intent origination publication result is malformed")]
    InvalidEffectOriginationResult,
    #[error("Effect dispatch seal publication result is malformed")]
    InvalidEffectSealResult,
    #[error("Effect dispatch terminal publication result is malformed")]
    InvalidEffectTerminalResult,
    #[error("Run execution-time Receipt is malformed or does not bind the released Run")]
    InvalidRunExecutionTimeReceipt,
    #[error("execution-boundary no-start Observation is absent, unpinned, or malformed")]
    InvalidRunNoStartObservation,
    #[error("released external operation reached its Run deadline before provider I/O")]
    RunDeadlineExpired,
    #[error("external-I/O release is stale against the current control Head or Run")]
    StaleExternalIoRelease,
    #[error("Effect reconciliation publication result is malformed")]
    InvalidEffectReconciliationResult,
    #[error("Effect withdrawal publication result is malformed")]
    InvalidEffectWithdrawalResult,
    #[error("Effect same-home writer handoff publication result is malformed")]
    InvalidEffectWriterHandoffResult,
    #[error("Effect health publication result is malformed")]
    InvalidEffectHealthResult,
    #[error("active Effect Intent snapshot is malformed, ambiguous, or non-canonical")]
    InvalidEffectSnapshot,
    #[error("Execution Store Generation ordinal overflowed")]
    GenerationOverflow,
    #[error("Observation acquisition identity is already bound to another Observation")]
    DuplicateObservationAcquisition,
    #[error("Observation is not bound to the exact current Step revision and Run/Lease fence")]
    ObservationNotApplicableToStep,
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    AtomicPublication(#[from] AtomicPublicationError),
    #[error(transparent)]
    Generation(#[from] GenerationError),
    #[error(transparent)]
    Object(#[from] StoreObjectError),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(transparent)]
    Control(#[from] EffectIntentControlErrorV1),
    #[error(transparent)]
    Runtime(#[from] ExecutionRuntimeErrorV1),
    #[error(transparent)]
    StepSubmission(#[from] StepSubmissionErrorV1),
    #[error(transparent)]
    StepLifecycle(#[from] StepLifecycleError),
    #[error(transparent)]
    EvidenceClaim(#[from] ClaimError),
    #[error(transparent)]
    EvidenceStore(#[from] EvidenceStoreErrorV1),
    #[error(transparent)]
    Effect(#[from] EffectRuntimeErrorV1),
    #[error("Execution Action failed current Repository Authority admission")]
    AuthorityAdmissionFailed,
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
}

impl From<RepositoryAuthorityAdmissionErrorV1> for ExecutionStoreErrorV1 {
    fn from(_: RepositoryAuthorityAdmissionErrorV1) -> Self {
        Self::AuthorityAdmissionFailed
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Barrier, mpsc};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use rusqlite::Connection;

    use super::*;
    use crate::domain::authority::{
        ActionAuthorityBasisKindV1, AuthorityContextIdV1, AuthorizationReceiptV1,
        RepositoryAuthoritySelectionV1,
        test_support::{
            AuthorityFixtureModeV1, RepositoryAuthorityFixtureV1, installation_authority_fixture,
            repository_authority_fixture, repository_authority_fixture_at,
        },
    };
    use crate::domain::contract::runtime::ContractGenerationIdV1;
    use crate::domain::evidence::{
        AuthorizedObservationPublicationV1, ClaimSubjectV1, ClaimV1, EvidenceClaimPublicationV1,
        EvidencePayloadManifestV1, EvidenceRedactionPolicyV1, EvidenceRetentionClassV1,
        EvidenceRetentionPolicyV1, EvidenceSecretScanReceiptV1, EvidenceStoreFacadeV1,
        ObservationAcquisitionV1, ObservationDraftV1, ObservationKindV1,
        ObservationPayloadCommonV1, ObservationPayloadDetailV1, ObservationPayloadV1,
        ObservationPublicationRouteV1, ObservationSubjectKindV1, ObservationSubjectV1,
        ObservationV1, SubmissionRefV1,
    };
    use crate::domain::execution::effects::{
        EffectOriginKindV1, EffectReconciliationReadPlanPartsV1,
        ReconciliationReadOperationClassificationV1, ReconciliationReadOperationKindV1,
    };
    use crate::domain::execution::runtime::{StepAttemptStateV1, TakeoverSafetyMechanismV1};
    use crate::domain::execution::withdrawal::RemoteClassificationV1;
    use crate::domain::identity::{ContractRootIdV1, SchemaIdV1};
    use crate::domain::persistence::{StoreCompatibilityV1, StoreDomainV1, StoreRoleV1};
    use crate::domain::step::{StepIdV1, StepRevisionIdV1, StepScopeV1};
    use crate::domain::work::WorkIdV1;

    fn step_observation(
        binding: StepBindingV1,
        submission_id: StepSubmissionIdV1,
        seed: u8,
    ) -> ObservationV1 {
        step_observation_at(
            binding,
            submission_id,
            seed,
            [seed.wrapping_add(8); 32],
            120,
        )
    }

    fn step_observation_at(
        binding: StepBindingV1,
        submission_id: StepSubmissionIdV1,
        seed: u8,
        acquisition_id: [u8; 32],
        recorded_at: u64,
    ) -> ObservationV1 {
        step_observation_at_with_payload(binding, submission_id, seed, acquisition_id, recorded_at)
            .0
    }

    fn step_observation_at_with_payload(
        binding: StepBindingV1,
        submission_id: StepSubmissionIdV1,
        seed: u8,
        acquisition_id: [u8; 32],
        recorded_at: u64,
    ) -> (ObservationV1, StoreObjectV1) {
        let token = |offset: u8| -> [u8; 32] { Sha256::digest([seed.wrapping_add(offset)]).into() };
        let kind = ObservationKindV1::DeterministicProcedure;
        let subjects = vec![
            ObservationSubjectV1::for_work(
                *binding.scope().work_id().as_bytes(),
                binding.contract_generation_id(),
                *binding.contract_root_id().as_bytes(),
            )
            .unwrap(),
            ObservationSubjectV1::new(
                ObservationSubjectKindV1::Step,
                *binding.step_id().as_bytes(),
                *binding.revision_id().as_bytes(),
            )
            .unwrap(),
            ObservationSubjectV1::new(
                ObservationSubjectKindV1::Submission,
                *submission_id.as_bytes(),
                *binding.contract_generation_id().as_bytes(),
            )
            .unwrap(),
            ObservationSubjectV1::new(
                ObservationSubjectKindV1::Repository,
                *binding.scope().repository_id().as_bytes(),
                *binding.contract_generation_id().as_bytes(),
            )
            .unwrap(),
        ];
        let procedure_hash = token(0);
        let environment_hash = token(1);
        let toolchain_hash = token(2);
        let clock_basis_hash = token(3);
        let typed_payload = ObservationPayloadV1::new(
            kind,
            ObservationPayloadCommonV1::new(
                &subjects,
                procedure_hash,
                environment_hash,
                toolchain_hash,
                recorded_at - 1,
                recorded_at,
                clock_basis_hash,
            )
            .unwrap(),
            ObservationPayloadDetailV1::Deterministic {
                executable_bytes_hash: token(10),
                executable_version_hash: token(11),
                arguments_hash: token(12),
                working_directory_hash: token(13),
                relevant_environment_hash: token(14),
                subject_revision_hash: token(15),
                dirty_state_hash: token(16),
                exit_status_hash: token(17),
                stdout_hash: token(18),
                stderr_hash: token(19),
            },
        )
        .unwrap();
        let payload = StoreObjectV1::new(
            kind.contract().unwrap().payload_schema_id(),
            CborValue::Bytes(typed_payload.canonical_bytes().unwrap()),
            vec![],
        )
        .unwrap();
        let producer = crate::domain::authority::ExecutionProducerV1::SessionBound {
            principal_id: PrincipalIdV1::derive("stage3-actor-principal").unwrap(),
            session_id: SessionIdV1::derive("stage3-actor-session").unwrap(),
        };
        let redaction = EvidenceRedactionPolicyV1::prohibit_secrets_v1(1_048_576).unwrap();
        let scan = EvidenceSecretScanReceiptV1::scan(
            payload.id(),
            &typed_payload,
            redaction,
            producer,
            recorded_at,
        )
        .unwrap();
        let retention = EvidenceRetentionPolicyV1::new(
            EvidenceRetentionClassV1::ExplicitSecurityErasureEligible,
            recorded_at + 1_000,
        )
        .unwrap();
        let observation = ObservationV1::new(ObservationDraftV1 {
            kind,
            store_domain_id: binding.scope().repository_id(),
            subjects,
            producer,
            procedure_hash,
            environment_hash,
            toolchain_hash,
            observed_at: recorded_at - 1,
            recorded_at,
            clock_basis_hash,
            lineage: vec![],
            payload: EvidencePayloadManifestV1::new(
                kind,
                payload.id(),
                &typed_payload,
                "application/cbor",
                redaction,
                scan,
                retention,
            )
            .unwrap(),
            acquisition: ObservationAcquisitionV1::effect_free(
                acquisition_id,
                [seed.wrapping_add(9); 32],
            )
            .unwrap(),
            publication_route: ObservationPublicationRouteV1::new(kind, 39, None, None).unwrap(),
        })
        .unwrap();
        (observation, payload)
    }

    fn publish_step_observation_for_claim(
        store: &mut StoreV1,
        binding: StepBindingV1,
        submission_id: StepSubmissionIdV1,
        claim_byte: u8,
        recorded_at: u64,
        fixture: &RepositoryAuthorityFixtureV1,
    ) -> ObservationV1 {
        let seed = claim_byte.wrapping_add(1);
        let (observation, payload) = step_observation_at_with_payload(
            binding,
            submission_id,
            seed,
            [seed.wrapping_add(8); 32],
            recorded_at,
        );
        let state = EvidenceStoreFacadeV1::new(store)
            .current_state_binding()
            .unwrap();
        let request = EvidenceStoreFacadeV1::new(store)
            .canonical_observation_request(
                state,
                &observation,
                &payload,
                IdempotencyKeyIdV1::derive(&format!("stage5-step-observation-{seed}")).unwrap(),
            )
            .unwrap();
        let authority = GenericExecutionAuthorityV1::new(
            fixture.selection,
            RepositoryActionLeafV1::PublishObservation,
            request.subject_commitment(),
            request.expected_state_commitment(),
            request.payload_commitment(),
            fixture.actor_principal,
        )
        .unwrap();
        EvidenceStoreFacadeV1::new(store)
            .publish_observation(
                AuthorizedObservationPublicationV1::new(
                    state,
                    request,
                    authority,
                    observation.clone(),
                    payload,
                )
                .unwrap(),
            )
            .unwrap();
        observation
    }

    fn step_observation_subject_commitment(observation: &ObservationV1) -> [u8; 32] {
        hash(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.evidence-observation-subject.v1").unwrap(),
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
        .unwrap()
    }

    struct TestPinnedProviderExecutorV1 {
        operation: ProviderOperationBindingV1,
        observation: ProviderTransportObservationV1,
        calls: u8,
    }

    impl PinnedProviderExecutorV1 for TestPinnedProviderExecutorV1 {
        fn provider_operation_contract_commitment(&self) -> [u8; 32] {
            self.operation.provider_operation_contract_commitment
        }

        fn provider_scope_commitment(&self) -> [u8; 32] {
            self.operation.provider_scope_commitment
        }

        fn provider_key_commitment(&self) -> [u8; 32] {
            self.operation.provider_key_commitment
        }

        fn credential_commitment(&self) -> [u8; 32] {
            self.operation.credential_commitment
        }

        fn hidden_retries_disabled(&self) -> bool {
            true
        }

        fn execute_once(
            &mut self,
            operation: &SealedProviderOperationV1,
        ) -> ProviderTransportObservationV1 {
            assert_eq!(operation.binding, self.operation);
            self.calls += 1;
            self.observation
        }
    }

    struct BlockingPinnedProviderExecutorV1 {
        operation: ProviderOperationBindingV1,
        observation: ProviderTransportObservationV1,
        entered: Arc<Barrier>,
        continue_execution: Arc<Barrier>,
        calls: u8,
    }

    impl PinnedProviderExecutorV1 for BlockingPinnedProviderExecutorV1 {
        fn provider_operation_contract_commitment(&self) -> [u8; 32] {
            self.operation.provider_operation_contract_commitment
        }

        fn provider_scope_commitment(&self) -> [u8; 32] {
            self.operation.provider_scope_commitment
        }

        fn provider_key_commitment(&self) -> [u8; 32] {
            self.operation.provider_key_commitment
        }

        fn credential_commitment(&self) -> [u8; 32] {
            self.operation.credential_commitment
        }

        fn hidden_retries_disabled(&self) -> bool {
            true
        }

        fn execute_once(
            &mut self,
            operation: &SealedProviderOperationV1,
        ) -> ProviderTransportObservationV1 {
            assert_eq!(operation.binding, self.operation);
            self.calls += 1;
            self.entered.wait();
            self.continue_execution.wait();
            self.observation
        }
    }

    struct TestPinnedExecutionBoundaryObserverV1 {
        execution_boundary_commitment: [u8; 32],
        observer_commitment: [u8; 32],
        observation_commitment: Option<[u8; 32]>,
        calls: u8,
    }

    impl PinnedExecutionBoundaryObserverV1 for TestPinnedExecutionBoundaryObserverV1 {
        fn execution_boundary_commitment(&self) -> [u8; 32] {
            self.execution_boundary_commitment
        }

        fn observer_commitment(&self) -> [u8; 32] {
            self.observer_commitment
        }

        fn observe_definitely_not_started(
            &mut self,
            challenge: RunNoStartObservationChallengeV1,
        ) -> Option<[u8; 32]> {
            assert_eq!(
                challenge.execution_boundary_commitment(),
                self.execution_boundary_commitment
            );
            assert_ne!(challenge.authority_acceptance_commitment(), [0; 32]);
            self.calls += 1;
            self.observation_commitment
        }
    }

    fn provider_terminal_draft_from_outcome(
        store: &mut StoreV1,
        release: ProviderApplicationReleaseV1,
        outcome: EffectDispatchOutcomePayloadV1,
    ) -> Result<ActiveStoreEffectTerminalDraftV1, ExecutionStoreErrorV1> {
        let evidence = effect_dispatch_outcome_evidence_commitment(outcome);
        let observation = match outcome {
            EffectDispatchOutcomePayloadV1::DefinitelyNotSent { .. } => {
                ProviderTransportObservationV1::DefinitelyNotSent {
                    authenticated_evidence_commitment: evidence,
                }
            }
            EffectDispatchOutcomePayloadV1::ResponseReceived { classification, .. } => {
                ProviderTransportObservationV1::ResponseReceived {
                    authenticated_evidence_commitment: evidence,
                    application_fact: match classification {
                        RemoteClassificationV1::ConfirmedApplied => {
                            ProviderApplicationFactV1::Applied
                        }
                        RemoteClassificationV1::ConfirmedNotApplied => {
                            ProviderApplicationFactV1::NotApplied
                        }
                        RemoteClassificationV1::Pending => ProviderApplicationFactV1::Pending,
                        RemoteClassificationV1::InDoubt => ProviderApplicationFactV1::Unknown,
                        RemoteClassificationV1::PartiallyApplied => {
                            ProviderApplicationFactV1::PartiallyApplied
                        }
                        RemoteClassificationV1::Conflicted => ProviderApplicationFactV1::Conflicted,
                        RemoteClassificationV1::Prepared
                        | RemoteClassificationV1::Dispatching
                        | RemoteClassificationV1::Cancelled => {
                            return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
                        }
                    },
                }
            }
            EffectDispatchOutcomePayloadV1::AmbiguousTransport { .. } => {
                ProviderTransportObservationV1::AmbiguousTransport {
                    authenticated_evidence_commitment: evidence,
                }
            }
            EffectDispatchOutcomePayloadV1::LocallyRejected { .. } => {
                return Err(ExecutionStoreErrorV1::PublicationBindingMismatch);
            }
        };
        let operation = release.operation.binding;
        let mut executor = TestPinnedProviderExecutorV1 {
            operation,
            observation,
            calls: 0,
        };
        let draft =
            ExecutionStoreFacadeV1::new(store).execute_provider_once(release, &mut executor)?;
        assert_eq!(executor.calls, 1);
        Ok(draft)
    }

    struct TestPinnedReconciliationReaderV1 {
        read: ReconciliationReadBindingV1,
        observation: ReconciliationReadObservationV1,
        calls: u8,
    }

    impl PinnedReconciliationReaderV1 for TestPinnedReconciliationReaderV1 {
        fn provider_commitment(&self) -> [u8; 32] {
            self.read.read_plan.provider_commitment()
        }

        fn account_commitment(&self) -> [u8; 32] {
            self.read.read_plan.account_commitment()
        }

        fn target_commitment(&self) -> [u8; 32] {
            self.read.read_plan.target_commitment()
        }

        fn correlation_commitment(&self) -> [u8; 32] {
            self.read.read_plan.correlation_commitment()
        }

        fn credential_commitment(&self) -> [u8; 32] {
            self.read.read_plan.credential_commitment()
        }

        fn visibility_commitment(&self) -> [u8; 32] {
            self.read.read_plan.visibility_commitment()
        }

        fn query_commitment(&self) -> [u8; 32] {
            self.read.read_plan.query_commitment()
        }

        fn evaluator_commitment(&self) -> [u8; 32] {
            self.read.read_plan.evaluator_commitment()
        }

        fn hidden_retries_disabled(&self) -> bool {
            true
        }

        fn read_once(
            &mut self,
            read: &SealedReconciliationReadV1,
        ) -> ReconciliationReadObservationV1 {
            assert_eq!(read.binding, self.read);
            self.calls += 1;
            self.observation
        }
    }

    fn reconciliation_read_draft(
        store: &mut StoreV1,
        release: ReconciliationReadReleaseV1,
        result_commitment: [u8; 32],
        application_fact: ProviderApplicationFactV1,
    ) -> ActiveStoreEffectReconciliationReadDraftV1 {
        let read = release.read.binding;
        let mut reader = TestPinnedReconciliationReaderV1 {
            read,
            observation: ReconciliationReadObservationV1 {
                usage: EffectReconciliationReadUsageV1 {
                    requests: 1,
                    pages: 1,
                    bytes: 512,
                    duration_ms: 250,
                    result_commitment,
                },
                application_fact,
            },
            calls: 0,
        };
        let draft = ExecutionStoreFacadeV1::new(store)
            .execute_reconciliation_read_once(release, &mut reader)
            .unwrap();
        assert_eq!(reader.calls, 1);
        draft
    }

    #[test]
    fn external_io_releases_reject_deadline_equality_before_calling_adapters() {
        let operation = ProviderOperationBindingV1 {
            run_id: RunIdV1::from_bytes([81; 32]).unwrap(),
            execution_boundary_commitment: [82; 32],
            deadline: 100,
            application_envelope_commitment: [83; 32],
            provider_operation_contract_commitment: [84; 32],
            provider_scope_commitment: [85; 32],
            provider_key_commitment: [86; 32],
            credential_commitment: [87; 32],
            semantic_operation_commitment: [88; 32],
            payload_commitment: [89; 32],
            target_commitment: [90; 32],
        };
        let release = ProviderApplicationReleaseV1 {
            intent: EffectIntentIdV1::from_bytes([91; 32]).unwrap(),
            dispatch_attempt: super::super::runtime::DispatchAttemptIdV1::from_bytes([92; 32])
                .unwrap(),
            sealed_control_head: EffectIntentControlTokenV1::new(HomeTokenV1::new([93; 32])),
            operation: SealedProviderOperationV1 { binding: operation },
        };
        let execution_time = release.execution_time_receipt(100, [94; 32]).unwrap();
        let mut executor = TestPinnedProviderExecutorV1 {
            operation,
            observation: ProviderTransportObservationV1::DefinitelyNotSent {
                authenticated_evidence_commitment: [95; 32],
            },
            calls: 0,
        };
        assert!(matches!(
            release.execute_once(execution_time, &mut executor),
            Err(ExecutionStoreErrorV1::RunDeadlineExpired)
        ));
        assert_eq!(executor.calls, 0);

        let read = ReconciliationReadBindingV1 {
            run_id: RunIdV1::from_bytes([96; 32]).unwrap(),
            execution_boundary_commitment: [97; 32],
            deadline: 200,
            intent: EffectIntentIdV1::from_bytes([98; 32]).unwrap(),
            control_head: EffectIntentControlTokenV1::new(HomeTokenV1::new([99; 32])),
            read_plan: reconciliation_read_plan(),
        };
        let release = ReconciliationReadReleaseV1 {
            read: SealedReconciliationReadV1 { binding: read },
        };
        let execution_time = release.execution_time_receipt(200, [100; 32]).unwrap();
        let mut reader = TestPinnedReconciliationReaderV1 {
            read,
            observation: ReconciliationReadObservationV1 {
                usage: EffectReconciliationReadUsageV1 {
                    requests: 1,
                    pages: 1,
                    bytes: 1,
                    duration_ms: 1,
                    result_commitment: [101; 32],
                },
                application_fact: ProviderApplicationFactV1::NotApplied,
            },
            calls: 0,
        };
        assert!(matches!(
            release.execute_once(execution_time, &mut reader),
            Err(ExecutionStoreErrorV1::RunDeadlineExpired)
        ));
        assert_eq!(reader.calls, 0);
    }

    #[test]
    fn provider_gateway_loads_current_authority_time_at_the_io_boundary() {
        let (_store_root, _domain, mut store, draft, selection, principal) =
            effect_store_fixture_at(b"stage4-provider-current-time", 150, 160);
        let origination = active_effect_origination_plan(
            &mut store,
            &draft,
            selection,
            principal,
            "stage4-provider-current-time-origination",
        );
        let originated = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_origination(origination)
            .unwrap();
        let seal = active_effect_seal_plan(
            &mut store,
            originated.intent(),
            selection,
            principal,
            "stage4-provider-current-time-seal",
            [204; 32],
        );
        let mut sealed = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_seal(seal)
            .unwrap();
        let release = sealed.take_provider_release().unwrap();
        let operation = release.operation.binding;
        let mut executor = TestPinnedProviderExecutorV1 {
            operation,
            observation: ProviderTransportObservationV1::DefinitelyNotSent {
                authenticated_evidence_commitment: [205; 32],
            },
            calls: 0,
        };

        assert!(matches!(
            ExecutionStoreFacadeV1::new(&mut store).execute_provider_once(release, &mut executor),
            Err(ExecutionStoreErrorV1::RunDeadlineExpired)
        ));
        assert_eq!(executor.calls, 0);
    }

    #[test]
    fn provider_gateway_serializes_writer_handoff_across_external_io() {
        let (store_root, domain, mut store, draft, selection, principal) =
            effect_store_fixture(b"stage4-provider-writer-handoff");
        let origination = active_effect_origination_plan(
            &mut store,
            &draft,
            selection,
            principal,
            "stage4-provider-writer-handoff-origination",
        );
        let originated = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_origination(origination)
            .unwrap();
        let seal = active_effect_seal_plan(
            &mut store,
            originated.intent(),
            selection,
            principal,
            "stage4-provider-writer-handoff-seal",
            [206; 32],
        );
        let mut sealed = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_seal(seal)
            .unwrap();
        let release = sealed.take_provider_release().unwrap();
        let operation = release.operation.binding;
        let handoff = active_effect_writer_handoff_plan(
            &mut store,
            originated.intent(),
            selection,
            principal,
            "stage4-provider-writer-handoff-publication",
        );
        drop(store);

        let entered = Arc::new(Barrier::new(2));
        let continue_execution = Arc::new(Barrier::new(2));
        let gateway_worker = {
            let store_root = store_root.clone();
            let domain = domain.clone();
            let entered = Arc::clone(&entered);
            let continue_execution = Arc::clone(&continue_execution);
            std::thread::spawn(move || {
                let mut store = StoreV1::open(store_root, domain).unwrap();
                let mut executor = BlockingPinnedProviderExecutorV1 {
                    operation,
                    observation: ProviderTransportObservationV1::DefinitelyNotSent {
                        authenticated_evidence_commitment: [207; 32],
                    },
                    entered,
                    continue_execution,
                    calls: 0,
                };
                let result = ExecutionStoreFacadeV1::new(&mut store)
                    .execute_provider_once(release, &mut executor);
                (result, executor.calls)
            })
        };
        entered.wait();

        let (handoff_sender, handoff_receiver) = mpsc::channel();
        let handoff_worker = {
            let store_root = store_root.clone();
            let domain = domain.clone();
            std::thread::spawn(move || {
                let mut store = StoreV1::open(store_root, domain).unwrap();
                let result =
                    ExecutionStoreFacadeV1::new(&mut store).publish_effect_writer_handoff(handoff);
                handoff_sender.send(result).unwrap();
            })
        };
        assert!(matches!(
            handoff_receiver.recv_timeout(Duration::from_millis(200)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
        continue_execution.wait();

        let (gateway_result, calls) = gateway_worker.join().unwrap();
        assert!(gateway_result.is_ok());
        assert_eq!(calls, 1);
        let handoff_result = handoff_receiver
            .recv_timeout(Duration::from_secs(10))
            .unwrap();
        handoff_worker.join().unwrap();
        assert!(handoff_result.is_ok(), "{handoff_result:?}");
    }

    #[test]
    fn step_execution_store_is_single_selector_atomic_and_restart_decodable() {
        let domain = StoreDomainV1::derive(StoreRoleV1::Repository, b"stage4-step-store")
            .expect("test fixture");
        let root = ContractRootIdV1::parse(&render_digest([41; 32])).expect("test fixture");
        let scope = StepScopeV1::new(domain.id(), WorkIdV1::derive("stage4-work").unwrap());
        let binding = StepBindingV1::new(
            scope,
            ContractGenerationIdV1::parse(&render_digest([42; 32])).unwrap(),
            root,
            StepIdV1::from_bytes(scope, [43; 32]).unwrap(),
            StepRevisionIdV1::from_bytes([44; 32]).unwrap(),
        )
        .unwrap();
        let subject_commitment = hash(&step_binding_store_value(binding)).unwrap();
        let fixture = repository_authority_fixture(
            vec![
                ("AcquireStepExecution", subject_commitment),
                ("RenewStepLeaseTerm", subject_commitment),
                ("AbandonStepAttempt", subject_commitment),
            ],
            AuthorityFixtureModeV1::Valid,
        );
        let step_state = open_step_state_object(binding);
        let step_graph = current_step_graph_object(binding);
        let mut objects = fixture.objects;
        objects.push(step_state.clone());
        objects.push(step_graph.clone());
        let store_root = test_root();
        let mut store = StoreV1::create(&store_root, domain.clone()).unwrap();
        put_objects_in_reference_order(&mut store, objects);
        let mut roots = vec![fixture.authority_root_id, step_state.id(), step_graph.id()];
        roots.sort_unstable();
        let generation = StoreGenerationV1::new(
            domain.clone(),
            1,
            None,
            root,
            StoreCompatibilityV1::stage0_successor().unwrap(),
            roots,
        )
        .unwrap();
        store.publish_generation(&generation, None).unwrap();
        activate_store(&store_root);

        let initial = ExecutionStoreFacadeV1::new(&mut store)
            .current_step_execution(binding)
            .unwrap();
        assert!(initial.carrier().is_none());
        assert_eq!(initial.next_fence().unwrap(), 1);
        let executor = PrincipalIdV1::derive("stage3-actor-principal").unwrap();
        let mutation_context = StepMutationTestContext {
            binding,
            selection: fixture.selection,
            executor,
        };
        let reservation = RunReservationV1 {
            semantic_operation_hash: [61; 32],
            inputs_commitment: [62; 32],
            environment_commitment: [63; 32],
            target_commitment: [64; 32],
            execution_boundary_commitment: [65; 32],
            deadline: 140,
            launch_ordinal: 1,
            current_step_term: None,
        };
        let acquire = AuthorizedStepExecutionMutationV1::Acquire {
            executor,
            fixed_envelope_commitment: reservation.fixed_envelope_commitment().unwrap(),
            run_limit: 8,
            issued_at: 120,
            expires_at: 150,
            hard_deadline: 180,
            takeover_safety: None,
        };
        let request_a = ExecutionStoreFacadeV1::new(&mut store)
            .canonical_step_request(
                &initial,
                &acquire,
                IdempotencyKeyIdV1::derive("stage4-acquire-a").unwrap(),
            )
            .unwrap();
        let authority_a = GenericExecutionAuthorityV1::new(
            fixture.selection,
            RepositoryActionLeafV1::AcquireStepExecution,
            request_a.subject_commitment(),
            request_a.expected_state_commitment(),
            request_a.payload_commitment(),
            executor,
        )
        .unwrap();
        let request_b = ExecutionStoreFacadeV1::new(&mut store)
            .canonical_step_request(
                &initial,
                &acquire,
                IdempotencyKeyIdV1::derive("stage4-acquire-b").unwrap(),
            )
            .unwrap();
        let authority_b = GenericExecutionAuthorityV1::new(
            fixture.selection,
            RepositoryActionLeafV1::AcquireStepExecution,
            request_b.subject_commitment(),
            request_b.expected_state_commitment(),
            request_b.payload_commitment(),
            executor,
        )
        .unwrap();
        let contender_b = StepExecutionPublicationV1::new(
            request_b,
            authority_b,
            initial.clone(),
            acquire.clone(),
        )
        .unwrap();
        let contender_a =
            StepExecutionPublicationV1::new(request_a, authority_a, initial, acquire).unwrap();
        drop(store);
        let barrier = Arc::new(Barrier::new(3));
        let workers = [contender_a, contender_b]
            .into_iter()
            .map(|plan| {
                let barrier = Arc::clone(&barrier);
                let store_root = store_root.clone();
                let domain = domain.clone();
                std::thread::spawn(move || {
                    let mut store = StoreV1::open(store_root, domain).unwrap();
                    barrier.wait();
                    ExecutionStoreFacadeV1::new(&mut store).publish_step_execution(plan)
                })
            })
            .collect::<Vec<_>>();
        barrier.wait();
        let results = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            results.iter().filter(|result| result.is_ok()).count(),
            1,
            "{results:?}"
        );
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(
                    result,
                    Err(ExecutionStoreErrorV1::StaleExpectedStoreState)
                ))
                .count(),
            1,
            "{results:?}"
        );
        let committed = results
            .into_iter()
            .find_map(Result::ok)
            .expect("one contender commits");
        assert_eq!(committed.carrier().tenure().attempt().fence(), 1);
        let stale_takeover_proof = TakeoverSafetyV1::from_owner_receipt(
            committed.carrier(),
            binding,
            2,
            150,
            TakeoverSafetyMechanismV1::PinnedQuiescenceEvidence,
            [79; 32],
        )
        .unwrap();
        assert!(matches!(
            validate_takeover_safety(
                Some(committed.carrier()),
                binding,
                2,
                149,
                Some(&stale_takeover_proof),
            ),
            Err(ExecutionStoreErrorV1::LiveStepExecutionAlreadyExists)
        ));
        assert!(matches!(
            validate_takeover_safety(Some(committed.carrier()), binding, 2, 150, None),
            Err(ExecutionStoreErrorV1::TakeoverSafetyRequired)
        ));
        assert!(
            validate_takeover_safety(
                Some(committed.carrier()),
                binding,
                2,
                150,
                Some(&stale_takeover_proof),
            )
            .is_ok()
        );

        let mut store = StoreV1::open(&store_root, domain.clone()).unwrap();
        assert_eq!(maximum_capacity_spent(&store), 1);
        let selected = ExecutionStoreFacadeV1::new(&mut store)
            .current_step_execution(binding)
            .unwrap();
        let current = selected.carrier().unwrap();
        assert_eq!(
            current.tenure().attempt().id(),
            committed.carrier().tenure().attempt().id()
        );
        let term = current.tenure().current_term().id();
        let revision = current.run_set().revision();
        let owner = current.run_set().owner();
        let mutation = AuthorizedStepExecutionMutationV1::Renew {
            expected_term_id: term,
            issued_at: 120,
            expires_at: 160,
            lease_mutation: Some(Box::new(StepLeaseMutationV1::ReserveRun {
                expected_run_set_revision: revision,
                as_of: 120,
                reservation: reservation.clone(),
            })),
        };
        let request = ExecutionStoreFacadeV1::new(&mut store)
            .canonical_step_request(
                &selected,
                &mutation,
                IdempotencyKeyIdV1::derive("stage4-reserve-run").unwrap(),
            )
            .unwrap();
        let authority = GenericExecutionAuthorityV1::new(
            fixture.selection,
            RepositoryActionLeafV1::RenewStepLeaseTerm,
            request.subject_commitment(),
            request.expected_state_commitment(),
            request.payload_commitment(),
            executor,
        )
        .unwrap();
        let plan = StepExecutionPublicationV1::new(request, authority, selected, mutation).unwrap();
        let replay_plan = plan.clone();
        let run_outcome = ExecutionStoreFacadeV1::new(&mut store)
            .publish_step_execution(plan)
            .unwrap();
        assert_eq!(run_outcome.carrier().run_set().runs().len(), 1);
        assert_eq!(run_outcome.carrier().run_set().runs()[0].owner(), owner);
        assert_eq!(maximum_capacity_spent(&store), 2);
        let replay = ExecutionStoreFacadeV1::new(&mut store)
            .publish_step_execution(replay_plan)
            .unwrap();
        assert!(replay.replayed());
        assert_eq!(replay.store_head(), run_outcome.store_head());
        assert_eq!(replay.carrier(), run_outcome.carrier());
        assert_eq!(maximum_capacity_spent(&store), 2);

        let first_run_id = run_outcome.carrier().run_set().runs()[0].id();
        let mut wrong_boundary_observer = TestPinnedExecutionBoundaryObserverV1 {
            execution_boundary_commitment: [201; 32],
            observer_commitment: [202; 32],
            observation_commitment: Some([203; 32]),
            calls: 0,
        };
        assert!(matches!(
            ExecutionStoreFacadeV1::new(&mut store).issue_run_no_start_receipt(
                binding,
                first_run_id,
                &mut wrong_boundary_observer,
            ),
            Err(ExecutionStoreErrorV1::InvalidRunNoStartObservation)
        ));
        assert_eq!(wrong_boundary_observer.calls, 0);
        let mut absent_observation = TestPinnedExecutionBoundaryObserverV1 {
            execution_boundary_commitment: reservation.execution_boundary_commitment,
            observer_commitment: [202; 32],
            observation_commitment: None,
            calls: 0,
        };
        assert!(matches!(
            ExecutionStoreFacadeV1::new(&mut store).issue_run_no_start_receipt(
                binding,
                first_run_id,
                &mut absent_observation,
            ),
            Err(ExecutionStoreErrorV1::InvalidRunNoStartObservation)
        ));
        assert_eq!(absent_observation.calls, 1);
        let mut observer = TestPinnedExecutionBoundaryObserverV1 {
            execution_boundary_commitment: reservation.execution_boundary_commitment,
            observer_commitment: [202; 32],
            observation_commitment: Some([203; 32]),
            calls: 0,
        };
        let no_start_receipt = ExecutionStoreFacadeV1::new(&mut store)
            .issue_run_no_start_receipt(binding, first_run_id, &mut observer)
            .unwrap();
        assert_eq!(observer.calls, 1);
        let definitely_not_started = publish_renewed_run_mutation(
            &mut store,
            &mutation_context,
            "stage4-run-definitely-not-started",
            120,
            165,
            |carrier| StepLeaseMutationV1::MarkDefinitelyNotStarted {
                expected_run_set_revision: carrier.run_set().revision(),
                as_of: 120,
                receipt: no_start_receipt,
            },
        );
        let retried = publish_renewed_run_mutation(
            &mut store,
            &mutation_context,
            "stage4-run-retry",
            120,
            170,
            |carrier| StepLeaseMutationV1::RetryRun {
                predecessor_run_id: first_run_id,
                expected_run_set_revision: carrier.run_set().revision(),
                as_of: 120,
                deadline: 139,
            },
        );
        let retry_run_id = retried.carrier().run_set().runs()[1].id();
        let active = publish_renewed_run_mutation(
            &mut store,
            &mutation_context,
            "stage4-run-active",
            120,
            175,
            |carrier| StepLeaseMutationV1::TransitionRun {
                run_id: retry_run_id,
                expected_run_set_revision: carrier.run_set().revision(),
                as_of: 120,
                next: RunStateV1::Active,
            },
        );
        let segmented = publish_renewed_run_mutation(
            &mut store,
            &mutation_context,
            "stage4-run-segment",
            120,
            178,
            |carrier| StepLeaseMutationV1::AppendRunSegment {
                run_id: retry_run_id,
                expected_run_set_revision: carrier.run_set().revision(),
                as_of: 120,
                process_or_job_identity: [66; 32],
                segment_commitment: [67; 32],
            },
        );
        let succeeded = publish_renewed_run_mutation(
            &mut store,
            &mutation_context,
            "stage4-run-succeeded",
            120,
            179,
            |carrier| StepLeaseMutationV1::TransitionRun {
                run_id: retry_run_id,
                expected_run_set_revision: carrier.run_set().revision(),
                as_of: 120,
                next: RunStateV1::Succeeded,
            },
        );
        assert_eq!(
            definitely_not_started.carrier().run_set().runs()[0].state(),
            RunStateV1::DefinitelyNotStarted
        );
        assert_eq!(
            active.carrier().run_set().runs()[1].state(),
            RunStateV1::Active
        );
        assert_eq!(segmented.carrier().run_set().runs()[1].segments().len(), 1);
        assert_eq!(
            succeeded.carrier().run_set().runs()[1].state(),
            RunStateV1::Succeeded
        );

        let snapshot = ExecutionStoreFacadeV1::new(&mut store)
            .current_step_execution(binding)
            .unwrap();
        let carrier = snapshot.carrier().unwrap();
        let abandoned = publish_authorized_step_mutation(
            &mut store,
            fixture.selection,
            snapshot.clone(),
            executor,
            "stage4-attempt-yielded",
            AuthorizedStepExecutionMutationV1::Abandon {
                terminal: StepAttemptTerminalV1::Yielded,
                expected_term_id: carrier.tenure().current_term().id(),
                as_of: 120,
                expected_run_set_revision: carrier.run_set().revision(),
            },
        );
        assert_eq!(
            abandoned.carrier().tenure().attempt().state(),
            StepAttemptStateV1::Terminal(StepAttemptTerminalV1::Yielded)
        );
        assert_eq!(maximum_capacity_spent(&store), 8);
        drop(store);
        let mut store = StoreV1::open(&store_root, domain).unwrap();
        let restored = ExecutionStoreFacadeV1::new(&mut store)
            .current_step_execution(binding)
            .unwrap();
        assert_eq!(restored.carrier(), Some(abandoned.carrier()));

        let takeover = |takeover_safety: Option<TakeoverSafetyV1>| {
            AuthorizedStepExecutionMutationV1::Acquire {
                executor,
                fixed_envelope_commitment: reservation.fixed_envelope_commitment().unwrap(),
                run_limit: 8,
                issued_at: 120,
                expires_at: 170,
                hard_deadline: 190,
                takeover_safety: takeover_safety.map(Box::new),
            }
        };
        let missing_plan = authorized_step_mutation_plan(
            &mut store,
            fixture.selection,
            restored.clone(),
            executor,
            "stage4-takeover-missing-safety",
            takeover(None),
        );
        assert!(matches!(
            ExecutionStoreFacadeV1::new(&mut store).publish_step_execution(missing_plan),
            Err(ExecutionStoreErrorV1::TakeoverSafetyRequired)
        ));
        let current_takeover_proof = TakeoverSafetyV1::from_owner_receipt(
            restored.carrier().unwrap(),
            binding,
            restored.next_fence().unwrap(),
            120,
            TakeoverSafetyMechanismV1::PinnedQuiescenceEvidence,
            [97; 32],
        )
        .unwrap();
        assert_eq!(
            TakeoverSafetyV1::from_canonical_value(&current_takeover_proof.canonical_value())
                .unwrap(),
            current_takeover_proof
        );
        let unknown_plan = authorized_step_mutation_plan(
            &mut store,
            fixture.selection,
            restored.clone(),
            executor,
            "stage4-takeover-unknown-safety",
            takeover(Some(stale_takeover_proof)),
        );
        assert!(matches!(
            ExecutionStoreFacadeV1::new(&mut store).publish_step_execution(unknown_plan),
            Err(ExecutionStoreErrorV1::Runtime(
                ExecutionRuntimeErrorV1::TakeoverSafetyUnknown
            ))
        ));
        let successor = publish_authorized_step_mutation(
            &mut store,
            fixture.selection,
            restored,
            executor,
            "stage4-takeover-pinned-safety",
            takeover(Some(current_takeover_proof)),
        );
        assert_eq!(successor.carrier().tenure().attempt().fence(), 2);
        assert_eq!(maximum_capacity_spent(&store), 9);
    }

    #[test]
    fn current_step_effect_origin_is_authorized_and_committed() {
        let (_store_root, domain, mut store, binding, fixture, executor) =
            step_origin_effect_store_fixture(b"stage4-current-step-effect-origin");
        let snapshot = ExecutionStoreFacadeV1::new(&mut store)
            .current_step_execution(binding)
            .unwrap();
        let draft =
            step_effect_origination_draft(&domain, snapshot.carrier().unwrap().tenure(), 120);
        let plan = active_effect_origination_plan(
            &mut store,
            &draft,
            fixture.selection,
            executor,
            "stage4-current-step-origin-publish",
        );
        let originated = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_origination(plan)
            .unwrap();
        let effect = ExecutionStoreFacadeV1::new(&mut store)
            .current_effect_snapshot(originated.intent())
            .unwrap();
        assert_eq!(
            effect.intent().origin().originating_step_binding(),
            Some(binding)
        );
        assert_eq!(maximum_capacity_spent(&store), 1);
    }

    #[test]
    fn step_timeout_is_refused_before_expiry_and_restart_closes_at_exact_expiry() {
        let (_root, _domain, mut store, binding, fixture, executor) =
            step_store_fixture_with_seeded_carrier(
                b"stage4-timeout-pre-expiry",
                &["AbandonStepAttempt"],
                169,
                170,
            );
        let snapshot = ExecutionStoreFacadeV1::new(&mut store)
            .current_step_execution(binding)
            .unwrap();
        let carrier = snapshot.carrier().unwrap();
        let pre_expiry = authorized_step_mutation_plan(
            &mut store,
            fixture.selection,
            snapshot.clone(),
            executor,
            "stage4-timeout-before-expiry",
            AuthorizedStepExecutionMutationV1::Abandon {
                terminal: StepAttemptTerminalV1::TimedOut,
                expected_term_id: carrier.tenure().current_term().id(),
                as_of: 169,
                expected_run_set_revision: carrier.run_set().revision(),
            },
        );
        assert!(matches!(
            ExecutionStoreFacadeV1::new(&mut store).publish_step_execution(pre_expiry),
            Err(ExecutionStoreErrorV1::UntrustedMutationTime)
        ));
        assert_eq!(maximum_capacity_spent(&store), 0);
        assert!(
            ExecutionStoreFacadeV1::new(&mut store)
                .current_step_execution(binding)
                .unwrap()
                .carrier()
                .unwrap()
                .tenure()
                .attempt()
                .is_live()
        );

        let (root, domain, store, binding, fixture, executor) =
            step_store_fixture_with_seeded_carrier(
                b"stage4-timeout-at-expiry",
                &["AbandonStepAttempt"],
                170,
                170,
            );
        drop(store);
        let mut reopened = StoreV1::open(&root, domain).unwrap();
        let snapshot = ExecutionStoreFacadeV1::new(&mut reopened)
            .current_step_execution(binding)
            .unwrap();
        let carrier = snapshot.carrier().unwrap();
        let expected_term_id = carrier.tenure().current_term().id();
        let expected_run_set_revision = carrier.run_set().revision();
        let timed_out = publish_authorized_step_mutation(
            &mut reopened,
            fixture.selection,
            snapshot,
            executor,
            "stage4-timeout-at-expiry",
            AuthorizedStepExecutionMutationV1::Abandon {
                terminal: StepAttemptTerminalV1::TimedOut,
                expected_term_id,
                as_of: 170,
                expected_run_set_revision,
            },
        );
        assert_eq!(
            timed_out.carrier().tenure().attempt().state(),
            StepAttemptStateV1::Terminal(StepAttemptTerminalV1::TimedOut)
        );
        assert!(timed_out.carrier().run_set().all_terminal());
        assert_eq!(maximum_capacity_spent(&reopened), 1);
    }

    #[test]
    fn renewed_step_term_rejects_stale_effect_origin_without_write_or_debit() {
        let (_store_root, domain, mut store, binding, fixture, executor) =
            step_origin_effect_store_fixture(b"stage4-renewed-step-effect-origin");
        let stale_snapshot = ExecutionStoreFacadeV1::new(&mut store)
            .current_step_execution(binding)
            .unwrap();
        let stale_draft =
            step_effect_origination_draft(&domain, stale_snapshot.carrier().unwrap().tenure(), 120);
        let stale_term = stale_snapshot
            .carrier()
            .unwrap()
            .tenure()
            .current_term()
            .id();
        let mut renewed_carrier = stale_snapshot.carrier().unwrap().clone();
        renewed_carrier
            .renew(
                stale_term,
                120,
                160,
                test_authorized_execution_action(
                    ExecutionActionV1::RenewStepLeaseTerm,
                    "stage4-step-origin-renew",
                ),
            )
            .unwrap();
        let plan = active_effect_origination_plan(
            &mut store,
            &stale_draft,
            fixture.selection,
            executor,
            "stage4-stale-step-origin-publish",
        );
        let head_before = store.coherent_publication_snapshot().unwrap().1;
        let spent_before = maximum_capacity_spent(&store);
        let (_, _, generation, mut active_objects) = store.coherent_publication_snapshot().unwrap();
        let carrier_schema =
            execution_schema_id("maestro.vnext.step-execution-carrier-schema.v1").unwrap();
        let index_schema =
            execution_schema_id("maestro.vnext.step-execution-index-schema.v1").unwrap();
        active_objects.retain(|object| {
            object.schema_id() != carrier_schema && object.schema_id() != index_schema
        });
        let renewed_carrier_object = step_execution_carrier_object(&renewed_carrier).unwrap();
        let renewed_index = build_step_execution_index_object(&[StepExecutionIndexEntryV1 {
            binding_commitment: hash(&step_binding_store_value(binding)).unwrap(),
            carrier_object_id: renewed_carrier_object.id(),
            fence_high_water: renewed_carrier.tenure().attempt().fence(),
        }])
        .unwrap();
        active_objects.extend([renewed_carrier_object, renewed_index]);
        assert!(matches!(
            validate_current_step_effect_origin(
                &generation,
                &active_objects,
                &stale_draft.origin,
                &plan.authority,
                120,
            ),
            Err(ExecutionStoreErrorV1::PublicationBindingMismatch)
        ));
        assert_eq!(
            store.coherent_publication_snapshot().unwrap().1,
            head_before
        );
        assert_eq!(maximum_capacity_spent(&store), spent_before);
    }

    #[test]
    fn takeover_submission_expiry_and_supersession_reject_stale_step_effect_origin() {
        let (_store_root, domain, mut store, binding, fixture, executor) =
            step_origin_effect_store_fixture(b"stage4-stale-step-effect-origin-table");
        let stale_snapshot = ExecutionStoreFacadeV1::new(&mut store)
            .current_step_execution(binding)
            .unwrap();
        let stale_carrier = stale_snapshot.carrier().unwrap();
        let stale_draft = step_effect_origination_draft(&domain, stale_carrier.tenure(), 120);
        let plan = active_effect_origination_plan(
            &mut store,
            &stale_draft,
            fixture.selection,
            executor,
            "stage4-stale-step-origin-table-publish",
        );
        let head_before = store.coherent_publication_snapshot().unwrap().1;
        let spent_before = maximum_capacity_spent(&store);
        let (_, _, generation, active_objects) = store.coherent_publication_snapshot().unwrap();

        let takeover = StepExecutionCarrierV1::acquire(StepExecutionAcquisitionV1 {
            binding,
            next_fence: 2,
            executor,
            store_generation_id: generation.id(),
            authority_epoch: fixture.authority_epoch,
            fixed_envelope_commitment: [145; 32],
            run_limit: 4,
            issued_at: 120,
            expires_at: 160,
            hard_deadline: 190,
            authority: test_authorized_execution_action(
                ExecutionActionV1::AcquireStepExecution,
                "stage4-step-origin-takeover",
            ),
        })
        .unwrap();
        let takeover_objects = replace_step_carrier_objects(&active_objects, binding, &takeover);
        assert!(matches!(
            validate_current_step_effect_origin(
                &generation,
                &takeover_objects,
                &stale_draft.origin,
                &plan.authority,
                120,
            ),
            Err(ExecutionStoreErrorV1::PublicationBindingMismatch)
        ));

        let mut submitted = stale_carrier.clone();
        let submission_fence = submitted
            .submission_fence(submitted.tenure().current_term().id(), 120)
            .unwrap();
        submitted
            .close_for_submission(submission_fence, 120)
            .unwrap();
        let submitted_objects = replace_step_carrier_objects(&active_objects, binding, &submitted);
        assert!(matches!(
            validate_current_step_effect_origin(
                &generation,
                &submitted_objects,
                &stale_draft.origin,
                &plan.authority,
                120,
            ),
            Err(ExecutionStoreErrorV1::PublicationBindingMismatch)
        ));

        assert!(matches!(
            validate_current_step_effect_origin(
                &generation,
                &active_objects,
                &stale_draft.origin,
                &plan.authority,
                151,
            ),
            Err(ExecutionStoreErrorV1::PublicationBindingMismatch)
        ));

        let superseding_root = ContractRootIdV1::parse(&render_digest([149; 32])).unwrap();
        let superseding_generation = StoreGenerationV1::new(
            domain,
            generation.ordinal() + 1,
            Some(generation.id()),
            superseding_root,
            StoreCompatibilityV1::stage0_successor().unwrap(),
            generation.roots().to_vec(),
        )
        .unwrap();
        assert!(matches!(
            validate_current_step_effect_origin(
                &superseding_generation,
                &active_objects,
                &stale_draft.origin,
                &plan.authority,
                120,
            ),
            Err(ExecutionStoreErrorV1::StepBindingNotCurrent)
        ));

        assert_eq!(
            store.coherent_publication_snapshot().unwrap().1,
            head_before
        );
        assert_eq!(maximum_capacity_spent(&store), spent_before);
    }

    #[test]
    fn step_submission_one_and_many_claims_are_atomic_restart_decodable_and_idempotent() {
        for claim_count in [1_usize, 3_usize] {
            let domain_seed: &[u8] = if claim_count == 1 {
                b"stage4-step-submission-one"
            } else {
                b"stage4-step-submission-many"
            };
            let submission_id =
                StepSubmissionIdV1::derive(&format!("stage4-step-submission-{claim_count}"))
                    .unwrap();
            let observation_specs = (0..claim_count)
                .map(|index| (u8::try_from(72 + index * 2).unwrap(), 120, submission_id))
                .collect::<Vec<_>>();
            let (store_root, domain, mut store, binding, fixture, executor) =
                step_store_fixture_with_observations(
                    domain_seed,
                    &["AcquireStepExecution", "SubmitStep"],
                    &observation_specs,
                );
            publish_initial_step_acquisition(
                &mut store,
                binding,
                fixture.selection,
                executor,
                &format!("stage4-submit-acquire-{claim_count}"),
            );
            let observations = (0..claim_count)
                .map(|index| {
                    let observation_byte = u8::try_from(72 + index * 2).unwrap();
                    publish_step_observation_for_claim(
                        &mut store,
                        binding,
                        submission_id,
                        observation_byte.wrapping_sub(1),
                        120,
                        &fixture,
                    )
                })
                .collect::<Vec<_>>();
            let snapshot = ExecutionStoreFacadeV1::new(&mut store)
                .current_step_execution(binding)
                .unwrap();
            let term_id = snapshot.carrier().unwrap().tenure().current_term().id();
            let fence = snapshot
                .carrier()
                .unwrap()
                .submission_fence(term_id, 120)
                .unwrap();
            let claims = observations
                .iter()
                .enumerate()
                .map(|(index, observation)| {
                    let claim_byte = u8::try_from(71 + index * 2).unwrap();
                    ClaimV1::new(
                        SubmissionRefV1::for_step(submission_id).unwrap(),
                        ClaimSubjectV1::for_step(binding, fence.fence()).unwrap(),
                        [claim_byte; 32],
                        vec![observation.id()],
                    )
                    .unwrap()
                })
                .collect::<Vec<_>>();
            let evidence = EvidenceClaimPublicationV1::new(
                SubmissionRefV1::for_step(submission_id).unwrap(),
                claims,
                observations,
            )
            .unwrap();
            let submission = ExecutionStoreFacadeV1::new(&mut store)
                .step_submission_candidate(&snapshot, submission_id, term_id, 120, &evidence)
                .unwrap();
            let request = ExecutionStoreFacadeV1::new(&mut store)
                .canonical_step_submission_request(
                    &snapshot,
                    &submission,
                    120,
                    IdempotencyKeyIdV1::derive(&format!("stage4-submit-step-{claim_count}"))
                        .unwrap(),
                )
                .unwrap();
            let authority = SubmitStepAuthorityV1::new(
                fixture.selection,
                request.subject_commitment(),
                request.expected_state_commitment(),
                request.payload_commitment(),
                executor,
            )
            .unwrap();
            let plan = StepSubmissionPublicationV1::new(
                request,
                authority,
                snapshot,
                submission.clone(),
                evidence,
                120,
            )
            .unwrap();
            let replay_plan = plan.clone();
            let (_, _, predecessor_generation, predecessor_objects) =
                store.coherent_publication_snapshot().unwrap();
            let committed = ExecutionStoreFacadeV1::new(&mut store)
                .publish_step_submission(plan)
                .unwrap();
            assert!(!committed.replayed());
            assert_eq!(committed.submission(), &submission);
            assert_eq!(
                committed.carrier().tenure().attempt().state(),
                StepAttemptStateV1::Terminal(StepAttemptTerminalV1::Submitted)
            );
            assert_eq!(
                committed.carrier().tenure().lease().state(),
                StepAttemptStateV1::Terminal(StepAttemptTerminalV1::Submitted)
            );
            assert_eq!(
                maximum_capacity_spent(&store),
                u64::try_from(claim_count).unwrap() + 2
            );
            let (_, _, committed_generation, committed_objects) =
                store.coherent_publication_snapshot().unwrap();
            assert_eq!(
                committed_generation.ordinal(),
                predecessor_generation.ordinal() + 1
            );
            assert_eq!(
                committed_generation.previous(),
                Some(predecessor_generation.id())
            );
            for schema in [
                STEP_SUBMISSION_SCHEMA_V1,
                STEP_SUBMISSION_CLAIM_SCHEMA_V1,
                STEP_SUBMISSION_CLAIM_SET_SCHEMA_V1,
            ] {
                let schema_id = execution_schema_id(schema).unwrap();
                assert!(
                    !predecessor_objects
                        .iter()
                        .any(|object| object.schema_id() == schema_id)
                );
                assert!(
                    committed_objects
                        .iter()
                        .any(|object| object.schema_id() == schema_id)
                );
            }
            let replay = ExecutionStoreFacadeV1::new(&mut store)
                .publish_step_submission(replay_plan)
                .unwrap();
            assert!(replay.replayed());
            assert_eq!(replay.store_head(), committed.store_head());
            assert_eq!(replay.submission(), committed.submission());
            assert_eq!(replay.carrier(), committed.carrier());
            assert_eq!(
                maximum_capacity_spent(&store),
                u64::try_from(claim_count).unwrap() + 2
            );

            drop(store);
            let mut reopened = StoreV1::open(&store_root, domain).unwrap();
            let selected = ExecutionStoreFacadeV1::new(&mut reopened)
                .current_step_execution(binding)
                .unwrap();
            assert_eq!(selected.carrier().unwrap(), committed.carrier());
            let (_, _, generation, active_objects) =
                reopened.coherent_publication_snapshot().unwrap();
            let rooted = rooted_step_state_object(&generation, &active_objects, binding).unwrap();
            assert!(!rooted_step_state_is_open(rooted).unwrap());
            assert_eq!(rooted.references().len(), 1);
            let stored_submission = active_objects
                .iter()
                .find(|object| {
                    object.schema_id() == execution_schema_id(STEP_SUBMISSION_SCHEMA_V1).unwrap()
                })
                .unwrap();
            assert_eq!(
                StepSubmissionV1::from_canonical_value(stored_submission.value()).unwrap(),
                submission
            );
            assert!(rooted.references().contains(&stored_submission.id()));
            assert_eq!(
                active_objects
                    .iter()
                    .filter(|object| object.schema_id()
                        == execution_schema_id(STEP_SUBMISSION_CLAIM_SCHEMA_V1).unwrap())
                    .count(),
                claim_count
            );
            assert_eq!(
                active_objects
                    .iter()
                    .filter(|object| object.schema_id()
                        == execution_schema_id(STEP_SUBMISSION_CLAIM_SET_SCHEMA_V1).unwrap())
                    .count(),
                1
            );
        }
    }

    #[test]
    fn stale_step_submission_fence_writes_nothing_and_spends_no_authority() {
        let (_store_root, _domain, mut store, binding, fixture, executor) = step_store_fixture(
            b"stage4-stale-step-submission",
            &["AcquireStepExecution", "RenewStepLeaseTerm", "SubmitStep"],
        );
        publish_initial_step_acquisition(
            &mut store,
            binding,
            fixture.selection,
            executor,
            "stage4-stale-submit-acquire",
        );
        let snapshot = ExecutionStoreFacadeV1::new(&mut store)
            .current_step_execution(binding)
            .unwrap();
        let term_id = snapshot.carrier().unwrap().tenure().current_term().id();
        let fence = snapshot
            .carrier()
            .unwrap()
            .submission_fence(term_id, 120)
            .unwrap();
        let submission_id = StepSubmissionIdV1::derive("stage4-stale-submission-id").unwrap();
        let observation = step_observation(binding, submission_id, 74);
        let claim = ClaimV1::new(
            SubmissionRefV1::for_step(submission_id).unwrap(),
            ClaimSubjectV1::for_step(binding, fence.fence()).unwrap(),
            [73; 32],
            vec![observation.id()],
        )
        .unwrap();
        let evidence = EvidenceClaimPublicationV1::new(
            SubmissionRefV1::for_step(submission_id).unwrap(),
            vec![claim],
            vec![observation],
        )
        .unwrap();
        let submission = ExecutionStoreFacadeV1::new(&mut store)
            .step_submission_candidate(&snapshot, submission_id, term_id, 120, &evidence)
            .unwrap();
        let submit_request = ExecutionStoreFacadeV1::new(&mut store)
            .canonical_step_submission_request(
                &snapshot,
                &submission,
                120,
                IdempotencyKeyIdV1::derive("stage4-stale-submit").unwrap(),
            )
            .unwrap();
        let submit_plan = StepSubmissionPublicationV1::new(
            submit_request.clone(),
            SubmitStepAuthorityV1::new(
                fixture.selection,
                submit_request.subject_commitment(),
                submit_request.expected_state_commitment(),
                submit_request.payload_commitment(),
                executor,
            )
            .unwrap(),
            snapshot.clone(),
            submission,
            evidence,
            120,
        )
        .unwrap();
        let renew = AuthorizedStepExecutionMutationV1::Renew {
            expected_term_id: term_id,
            issued_at: 120,
            expires_at: 160,
            lease_mutation: None,
        };
        let renew_request = ExecutionStoreFacadeV1::new(&mut store)
            .canonical_step_request(
                &snapshot,
                &renew,
                IdempotencyKeyIdV1::derive("stage4-renew-before-submit").unwrap(),
            )
            .unwrap();
        let renew_plan = StepExecutionPublicationV1::new(
            renew_request.clone(),
            GenericExecutionAuthorityV1::new(
                fixture.selection,
                RepositoryActionLeafV1::RenewStepLeaseTerm,
                renew_request.subject_commitment(),
                renew_request.expected_state_commitment(),
                renew_request.payload_commitment(),
                executor,
            )
            .unwrap(),
            snapshot,
            renew,
        )
        .unwrap();
        ExecutionStoreFacadeV1::new(&mut store)
            .publish_step_execution(renew_plan)
            .unwrap();
        let spent_before = maximum_capacity_spent(&store);
        let head_before = store.coherent_publication_snapshot().unwrap().1;
        assert!(matches!(
            ExecutionStoreFacadeV1::new(&mut store).publish_step_submission(submit_plan),
            Err(ExecutionStoreErrorV1::StaleExpectedStoreState)
        ));
        assert_eq!(maximum_capacity_spent(&store), spent_before);
        assert_eq!(
            store.coherent_publication_snapshot().unwrap().1,
            head_before
        );
        let (_, _, generation, active_objects) = store.coherent_publication_snapshot().unwrap();
        assert!(
            rooted_step_state_is_open(
                rooted_step_state_object(&generation, &active_objects, binding).unwrap()
            )
            .unwrap()
        );
        assert!(!active_objects.iter().any(|object| {
            object.schema_id() == execution_schema_id(STEP_SUBMISSION_SCHEMA_V1).unwrap()
        }));
    }

    #[test]
    fn step_submission_rejects_empty_and_wrong_fence_claim_sets_before_publication() {
        let (_store_root, _domain, mut store, binding, fixture, executor) = step_store_fixture(
            b"stage4-invalid-step-submission",
            &["AcquireStepExecution", "SubmitStep"],
        );
        publish_initial_step_acquisition(
            &mut store,
            binding,
            fixture.selection,
            executor,
            "stage4-invalid-submit-acquire",
        );
        let snapshot = ExecutionStoreFacadeV1::new(&mut store)
            .current_step_execution(binding)
            .unwrap();
        let term_id = snapshot.carrier().unwrap().tenure().current_term().id();
        let fence = snapshot
            .carrier()
            .unwrap()
            .submission_fence(term_id, 120)
            .unwrap();
        let submission_id = StepSubmissionIdV1::derive("stage4-invalid-submission-id").unwrap();
        assert!(matches!(
            EvidenceClaimPublicationV1::new(
                SubmissionRefV1::for_step(submission_id).unwrap(),
                vec![],
                vec![],
            ),
            Err(crate::domain::evidence::ClaimError::SubmissionClaimSet(
                crate::domain::evidence::SubmissionClaimSetError::Empty
            ))
        ));
        let wrong_observation = step_observation(binding, submission_id, 76);
        let wrong_fence_claim = ClaimV1::new(
            SubmissionRefV1::for_step(submission_id).unwrap(),
            ClaimSubjectV1::for_step(binding, fence.fence() + 1).unwrap(),
            [75; 32],
            vec![wrong_observation.id()],
        )
        .unwrap();
        let wrong_evidence = EvidenceClaimPublicationV1::new(
            SubmissionRefV1::for_step(submission_id).unwrap(),
            vec![wrong_fence_claim],
            vec![wrong_observation],
        )
        .unwrap();
        assert!(matches!(
            ExecutionStoreFacadeV1::new(&mut store).step_submission_candidate(
                &snapshot,
                submission_id,
                term_id,
                120,
                &wrong_evidence,
            ),
            Err(ExecutionStoreErrorV1::StepSubmission(
                StepSubmissionErrorV1::ClaimBindingMismatch
            ))
        ));
        assert_eq!(maximum_capacity_spent(&store), 1);
    }

    #[test]
    fn step_submission_and_renewal_race_has_one_atomic_winner() {
        let submission_id = StepSubmissionIdV1::derive("stage4-race-submission-id").unwrap();
        let (store_root, domain, mut store, binding, fixture, executor) =
            step_store_fixture_with_observations(
                b"stage4-submit-renew-race",
                &["AcquireStepExecution", "RenewStepLeaseTerm", "SubmitStep"],
                &[(78, 120, submission_id)],
            );
        publish_initial_step_acquisition(
            &mut store,
            binding,
            fixture.selection,
            executor,
            "stage4-race-acquire",
        );
        let observation = publish_step_observation_for_claim(
            &mut store,
            binding,
            submission_id,
            77,
            120,
            &fixture,
        );
        let snapshot = ExecutionStoreFacadeV1::new(&mut store)
            .current_step_execution(binding)
            .unwrap();
        let term_id = snapshot.carrier().unwrap().tenure().current_term().id();
        let fence = snapshot
            .carrier()
            .unwrap()
            .submission_fence(term_id, 120)
            .unwrap();
        let claim = ClaimV1::new(
            SubmissionRefV1::for_step(submission_id).unwrap(),
            ClaimSubjectV1::for_step(binding, fence.fence()).unwrap(),
            [77; 32],
            vec![observation.id()],
        )
        .unwrap();
        let evidence = EvidenceClaimPublicationV1::new(
            SubmissionRefV1::for_step(submission_id).unwrap(),
            vec![claim],
            vec![observation],
        )
        .unwrap();
        let submission = ExecutionStoreFacadeV1::new(&mut store)
            .step_submission_candidate(&snapshot, submission_id, term_id, 120, &evidence)
            .unwrap();
        let submit_request = ExecutionStoreFacadeV1::new(&mut store)
            .canonical_step_submission_request(
                &snapshot,
                &submission,
                120,
                IdempotencyKeyIdV1::derive("stage4-race-submit").unwrap(),
            )
            .unwrap();
        let submit_plan = StepSubmissionPublicationV1::new(
            submit_request.clone(),
            SubmitStepAuthorityV1::new(
                fixture.selection,
                submit_request.subject_commitment(),
                submit_request.expected_state_commitment(),
                submit_request.payload_commitment(),
                executor,
            )
            .unwrap(),
            snapshot.clone(),
            submission,
            evidence,
            120,
        )
        .unwrap();
        let renew = AuthorizedStepExecutionMutationV1::Renew {
            expected_term_id: term_id,
            issued_at: 120,
            expires_at: 160,
            lease_mutation: None,
        };
        let renew_request = ExecutionStoreFacadeV1::new(&mut store)
            .canonical_step_request(
                &snapshot,
                &renew,
                IdempotencyKeyIdV1::derive("stage4-race-renew").unwrap(),
            )
            .unwrap();
        let renew_plan = StepExecutionPublicationV1::new(
            renew_request.clone(),
            GenericExecutionAuthorityV1::new(
                fixture.selection,
                RepositoryActionLeafV1::RenewStepLeaseTerm,
                renew_request.subject_commitment(),
                renew_request.expected_state_commitment(),
                renew_request.payload_commitment(),
                executor,
            )
            .unwrap(),
            snapshot,
            renew,
        )
        .unwrap();
        drop(store);

        let barrier = Arc::new(Barrier::new(3));
        let submit_worker = {
            let barrier = Arc::clone(&barrier);
            let store_root = store_root.clone();
            let domain = domain.clone();
            std::thread::spawn(move || {
                let mut store = StoreV1::open(store_root, domain).unwrap();
                barrier.wait();
                ExecutionStoreFacadeV1::new(&mut store)
                    .publish_step_submission(submit_plan)
                    .map(|_| true)
            })
        };
        let renew_worker = {
            let barrier = Arc::clone(&barrier);
            let store_root = store_root.clone();
            let domain = domain.clone();
            std::thread::spawn(move || {
                let mut store = StoreV1::open(store_root, domain).unwrap();
                barrier.wait();
                ExecutionStoreFacadeV1::new(&mut store)
                    .publish_step_execution(renew_plan)
                    .map(|_| false)
            })
        };
        barrier.wait();
        let results = [submit_worker.join().unwrap(), renew_worker.join().unwrap()];
        assert_eq!(
            results.iter().filter(|result| result.is_ok()).count(),
            1,
            "{results:?}"
        );
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(
                    result,
                    Err(ExecutionStoreErrorV1::StaleExpectedStoreState)
                ))
                .count(),
            1,
            "{results:?}"
        );
        let submission_won = matches!(results[0], Ok(true));
        let mut reopened = StoreV1::open(&store_root, domain).unwrap();
        assert_eq!(maximum_capacity_spent(&reopened), 3);
        let selected = ExecutionStoreFacadeV1::new(&mut reopened)
            .current_step_execution(binding)
            .unwrap();
        let (_, _, generation, active_objects) = reopened.coherent_publication_snapshot().unwrap();
        let is_open = rooted_step_state_is_open(
            rooted_step_state_object(&generation, &active_objects, binding).unwrap(),
        )
        .unwrap();
        if submission_won {
            assert!(!is_open);
            assert_eq!(
                selected.carrier().unwrap().tenure().attempt().state(),
                StepAttemptStateV1::Terminal(StepAttemptTerminalV1::Submitted)
            );
            assert!(active_objects.iter().any(|object| {
                object.schema_id() == execution_schema_id(STEP_SUBMISSION_SCHEMA_V1).unwrap()
            }));
        } else {
            assert!(is_open);
            assert_eq!(
                selected.carrier().unwrap().tenure().attempt().state(),
                StepAttemptStateV1::Live
            );
            assert!(!active_objects.iter().any(|object| {
                object.schema_id() == execution_schema_id(STEP_SUBMISSION_SCHEMA_V1).unwrap()
            }));
        }
    }

    #[test]
    fn competing_step_submissions_have_one_atomic_winner() {
        let submission_a = StepSubmissionIdV1::derive("stage4-submit-submit-a-submission").unwrap();
        let submission_b = StepSubmissionIdV1::derive("stage4-submit-submit-b-submission").unwrap();
        let (store_root, domain, mut store, binding, fixture, executor) =
            step_store_fixture_with_observations(
                b"stage4-submit-submit-race",
                &["AcquireStepExecution", "SubmitStep"],
                &[(102, 120, submission_a), (104, 120, submission_b)],
            );
        publish_initial_step_acquisition(
            &mut store,
            binding,
            fixture.selection,
            executor,
            "stage4-submit-submit-acquire",
        );
        let observation_a = publish_step_observation_for_claim(
            &mut store,
            binding,
            submission_a,
            101,
            120,
            &fixture,
        );
        let observation_b = publish_step_observation_for_claim(
            &mut store,
            binding,
            submission_b,
            103,
            120,
            &fixture,
        );
        let snapshot = ExecutionStoreFacadeV1::new(&mut store)
            .current_step_execution(binding)
            .unwrap();
        let contender_a = step_submission_plan_for_claim(
            &mut store,
            &snapshot,
            observation_a,
            submission_a,
            &fixture,
            "stage4-submit-submit-a",
            101,
        );
        let contender_b = step_submission_plan_for_claim(
            &mut store,
            &snapshot,
            observation_b,
            submission_b,
            &fixture,
            "stage4-submit-submit-b",
            103,
        );
        drop(store);

        let barrier = Arc::new(Barrier::new(3));
        let workers = [contender_a, contender_b]
            .into_iter()
            .map(|plan| {
                let barrier = Arc::clone(&barrier);
                let store_root = store_root.clone();
                let domain = domain.clone();
                std::thread::spawn(move || {
                    let mut store = StoreV1::open(store_root, domain).unwrap();
                    barrier.wait();
                    ExecutionStoreFacadeV1::new(&mut store).publish_step_submission(plan)
                })
            })
            .collect::<Vec<_>>();
        barrier.wait();
        let results = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            results.iter().filter(|result| result.is_ok()).count(),
            1,
            "{results:?}"
        );
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(
                    result,
                    Err(ExecutionStoreErrorV1::StaleExpectedStoreState)
                ))
                .count(),
            1,
            "{results:?}"
        );

        let mut reopened = StoreV1::open(&store_root, domain).unwrap();
        assert_eq!(maximum_capacity_spent(&reopened), 4);
        let selected = ExecutionStoreFacadeV1::new(&mut reopened)
            .current_step_execution(binding)
            .unwrap();
        assert_eq!(
            selected.carrier().unwrap().tenure().attempt().state(),
            StepAttemptStateV1::Terminal(StepAttemptTerminalV1::Submitted)
        );
        let (_, _, generation, active_objects) = reopened.coherent_publication_snapshot().unwrap();
        assert!(
            !rooted_step_state_is_open(
                rooted_step_state_object(&generation, &active_objects, binding).unwrap()
            )
            .unwrap()
        );
        for (schema, count) in [
            (STEP_SUBMISSION_SCHEMA_V1, 1),
            (STEP_SUBMISSION_CLAIM_SCHEMA_V1, 1),
            (STEP_SUBMISSION_CLAIM_SET_SCHEMA_V1, 1),
        ] {
            assert_eq!(
                active_objects
                    .iter()
                    .filter(|object| object.schema_id() == execution_schema_id(schema).unwrap())
                    .count(),
                count
            );
        }
    }

    #[test]
    fn step_submission_and_takeover_boundary_proves_both_atomic_linearizations() {
        let submit_submission =
            StepSubmissionIdV1::derive("stage4-submit-takeover-submit-submission").unwrap();
        let (_store_root, _domain, mut store, binding, fixture, executor) =
            step_store_fixture_with_observations(
                b"stage4-submit-takeover-race",
                &["AcquireStepExecution", "SubmitStep"],
                &[(106, 120, submit_submission)],
            );
        publish_initial_step_acquisition(
            &mut store,
            binding,
            fixture.selection,
            executor,
            "stage4-submit-takeover-acquire",
        );
        let observation = publish_step_observation_for_claim(
            &mut store,
            binding,
            submit_submission,
            105,
            120,
            &fixture,
        );
        let snapshot = ExecutionStoreFacadeV1::new(&mut store)
            .current_step_execution(binding)
            .unwrap();
        let submit_plan = step_submission_plan_for_claim(
            &mut store,
            &snapshot,
            observation,
            submit_submission,
            &fixture,
            "stage4-submit-takeover-submit",
            105,
        );
        let takeover_safety = TakeoverSafetyV1::from_owner_receipt(
            snapshot.carrier().unwrap(),
            binding,
            snapshot.next_fence().unwrap(),
            120,
            TakeoverSafetyMechanismV1::PinnedQuiescenceEvidence,
            [107; 32],
        )
        .unwrap();
        let takeover = AuthorizedStepExecutionMutationV1::Acquire {
            executor,
            fixed_envelope_commitment: [145; 32],
            run_limit: 4,
            issued_at: 120,
            expires_at: 170,
            hard_deadline: 190,
            takeover_safety: Some(Box::new(takeover_safety)),
        };
        let takeover_request = ExecutionStoreFacadeV1::new(&mut store)
            .canonical_step_request(
                &snapshot,
                &takeover,
                IdempotencyKeyIdV1::derive("stage4-submit-takeover-contender").unwrap(),
            )
            .unwrap();
        let takeover_plan = StepExecutionPublicationV1::new(
            takeover_request.clone(),
            GenericExecutionAuthorityV1::new(
                fixture.selection,
                RepositoryActionLeafV1::AcquireStepExecution,
                takeover_request.subject_commitment(),
                takeover_request.expected_state_commitment(),
                takeover_request.payload_commitment(),
                executor,
            )
            .unwrap(),
            snapshot,
            takeover,
        )
        .unwrap();
        ExecutionStoreFacadeV1::new(&mut store)
            .publish_step_submission(submit_plan)
            .unwrap();
        assert!(matches!(
            ExecutionStoreFacadeV1::new(&mut store).publish_step_execution(takeover_plan),
            Err(ExecutionStoreErrorV1::StaleExpectedStoreState)
        ));
        assert_eq!(maximum_capacity_spent(&store), 3);
        let selected = ExecutionStoreFacadeV1::new(&mut store)
            .current_step_execution(binding)
            .unwrap();
        let (_, _, generation, active_objects) = store.coherent_publication_snapshot().unwrap();
        let is_open = rooted_step_state_is_open(
            rooted_step_state_object(&generation, &active_objects, binding).unwrap(),
        )
        .unwrap();
        assert!(!is_open);
        assert_eq!(
            selected.carrier().unwrap().tenure().attempt().state(),
            StepAttemptStateV1::Terminal(StepAttemptTerminalV1::Submitted)
        );
        for schema in [
            STEP_SUBMISSION_SCHEMA_V1,
            STEP_SUBMISSION_CLAIM_SCHEMA_V1,
            STEP_SUBMISSION_CLAIM_SET_SCHEMA_V1,
        ] {
            assert_eq!(
                active_objects
                    .iter()
                    .filter(|object| object.schema_id() == execution_schema_id(schema).unwrap())
                    .count(),
                1
            );
        }

        let stale_submission =
            StepSubmissionIdV1::derive("stage4-takeover-submit-stale-submit-submission").unwrap();
        let (_root, _domain, mut store, binding, fixture, executor) =
            step_store_fixture_with_seeded_carrier_and_observations(
                b"stage4-takeover-submit-race",
                &["AcquireStepExecution", "SubmitStep"],
                150,
                150,
                &[(110, 150, stale_submission)],
            );
        let observation = publish_step_observation_for_claim(
            &mut store,
            binding,
            stale_submission,
            109,
            150,
            &fixture,
        );
        let snapshot = ExecutionStoreFacadeV1::new(&mut store)
            .current_step_execution(binding)
            .unwrap();
        let stale_submit_plan = step_submission_plan_for_claim(
            &mut store,
            &snapshot,
            observation,
            stale_submission,
            &fixture,
            "stage4-takeover-submit-stale-submit",
            109,
        );
        let takeover_safety = TakeoverSafetyV1::from_owner_receipt(
            snapshot.carrier().unwrap(),
            binding,
            snapshot.next_fence().unwrap(),
            150,
            TakeoverSafetyMechanismV1::PinnedQuiescenceEvidence,
            [110; 32],
        )
        .unwrap();
        let takeover = AuthorizedStepExecutionMutationV1::Acquire {
            executor,
            fixed_envelope_commitment: [145; 32],
            run_limit: 4,
            issued_at: 150,
            expires_at: 170,
            hard_deadline: 190,
            takeover_safety: Some(Box::new(takeover_safety)),
        };
        let takeover_plan = authorized_step_mutation_plan(
            &mut store,
            fixture.selection,
            snapshot,
            executor,
            "stage4-takeover-submit-valid-takeover",
            takeover,
        );
        let takeover_outcome = ExecutionStoreFacadeV1::new(&mut store)
            .publish_step_execution(takeover_plan)
            .unwrap();
        assert_eq!(takeover_outcome.carrier().tenure().attempt().fence(), 2);
        assert!(matches!(
            ExecutionStoreFacadeV1::new(&mut store).publish_step_submission(stale_submit_plan),
            Err(ExecutionStoreErrorV1::StaleExpectedStoreState)
        ));
        assert_eq!(maximum_capacity_spent(&store), 2);
        let selected = ExecutionStoreFacadeV1::new(&mut store)
            .current_step_execution(binding)
            .unwrap();
        assert!(selected.carrier().unwrap().tenure().attempt().is_live());
        let (_, _, generation, active_objects) = store.coherent_publication_snapshot().unwrap();
        assert!(
            rooted_step_state_is_open(
                rooted_step_state_object(&generation, &active_objects, binding).unwrap()
            )
            .unwrap()
        );
        for schema in [
            STEP_SUBMISSION_SCHEMA_V1,
            STEP_SUBMISSION_CLAIM_SCHEMA_V1,
            STEP_SUBMISSION_CLAIM_SET_SCHEMA_V1,
        ] {
            assert_eq!(
                active_objects
                    .iter()
                    .filter(|object| object.schema_id() == execution_schema_id(schema).unwrap())
                    .count(),
                0
            );
        }
    }

    #[test]
    fn effect_origination_reserves_dispatch_atomically_replays_and_restarts() {
        let (store_root, domain, mut store, draft, selection, executor) =
            effect_store_fixture(b"stage4-effect-origination");
        let plan = active_effect_origination_plan(
            &mut store,
            &draft,
            selection,
            executor,
            "stage4-effect-origination-key",
        );
        let replay_plan = plan.clone();
        let committed = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_origination(plan)
            .unwrap();
        assert!(!committed.replayed());
        assert_eq!(maximum_capacity_spent(&store), 1);
        let selected = ExecutionStoreFacadeV1::new(&mut store)
            .current_effect_state_binding(committed.intent())
            .unwrap();
        assert_eq!(selected.control_head(), Some(committed.control_head()));
        let replayed = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_origination(replay_plan)
            .unwrap();
        assert!(replayed.replayed());
        assert_eq!(replayed.store_head(), committed.store_head());
        assert_eq!(replayed.intent(), committed.intent());
        assert_eq!(replayed.control_head(), committed.control_head());
        assert_eq!(replayed.dispatch_attempt(), committed.dispatch_attempt());
        assert_eq!(maximum_capacity_spent(&store), 1);
        let snapshot = ExecutionStoreFacadeV1::new(&mut store)
            .current_effect_snapshot(committed.intent())
            .unwrap();
        let seal_draft = ActiveStoreEffectSealDraftV1::new([101; 32]).unwrap();
        let seal_request = ExecutionStoreFacadeV1::new(&mut store)
            .canonical_effect_seal_request(
                &snapshot,
                seal_draft,
                IdempotencyKeyIdV1::derive("stage4-effect-seal").unwrap(),
            )
            .unwrap();
        let seal_authority = GenericExecutionAuthorityV1::new(
            selection,
            RepositoryActionLeafV1::RecordDispatchOutcome,
            seal_request.subject_commitment(),
            seal_request.expected_state_commitment(),
            seal_request.payload_commitment(),
            executor,
        )
        .unwrap();
        let seal_plan = ActiveStoreEffectSealPublicationV1::new(
            seal_request,
            seal_authority,
            snapshot,
            seal_draft,
        )
        .unwrap();
        let seal_replay_plan = seal_plan.clone();
        let mut sealed = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_seal(seal_plan)
            .unwrap();
        assert!(!sealed.replayed());
        let release = sealed.take_provider_release().unwrap();
        assert_eq!(release.intent(), committed.intent());
        assert_eq!(release.dispatch_attempt(), committed.dispatch_attempt());
        assert_eq!(release.sealed_control_head(), sealed.control_head());
        assert!(sealed.take_provider_release().is_none());
        let mut seal_replay = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_seal(seal_replay_plan)
            .unwrap();
        assert!(seal_replay.replayed());
        assert!(seal_replay.take_provider_release().is_none());
        assert_eq!(seal_replay.control_head(), sealed.control_head());
        assert_eq!(maximum_capacity_spent(&store), 2);
        let sealed_snapshot = ExecutionStoreFacadeV1::new(&mut store)
            .current_effect_snapshot(committed.intent())
            .unwrap();
        let terminal_draft = provider_terminal_draft_from_outcome(
            &mut store,
            release,
            EffectDispatchOutcomePayloadV1::AmbiguousTransport {
                evidence_commitment: [102; 32],
            },
        )
        .unwrap();
        let terminal_request = ExecutionStoreFacadeV1::new(&mut store)
            .canonical_effect_terminal_request(
                &sealed_snapshot,
                &terminal_draft,
                IdempotencyKeyIdV1::derive("stage4-effect-terminal").unwrap(),
            )
            .unwrap();
        let terminal_authority = GenericExecutionAuthorityV1::new(
            selection,
            RepositoryActionLeafV1::RecordDispatchOutcome,
            terminal_request.subject_commitment(),
            terminal_request.expected_state_commitment(),
            terminal_request.payload_commitment(),
            executor,
        )
        .unwrap();
        let terminal_plan = ActiveStoreEffectTerminalPublicationV1::new(
            terminal_request,
            terminal_authority,
            sealed_snapshot,
            terminal_draft,
        )
        .unwrap();
        let terminal_replay_plan = terminal_plan.clone();
        let terminal = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_terminal(terminal_plan)
            .unwrap();
        assert!(!terminal.replayed());
        assert_eq!(
            terminal.classification(),
            super::super::withdrawal::RemoteClassificationV1::InDoubt
        );
        let terminal_replay = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_terminal(terminal_replay_plan)
            .unwrap();
        assert!(terminal_replay.replayed());
        assert_eq!(terminal_replay.control_head(), terminal.control_head());
        assert_eq!(maximum_capacity_spent(&store), 3);
        let mut changed_dispatch_identity = draft.clone();
        changed_dispatch_identity.dispatch.provider_key_commitment = [103; 32];
        changed_dispatch_identity
            .dispatch
            .application_envelope_commitment = [104; 32];
        let duplicate_semantic = active_effect_origination_plan(
            &mut store,
            &changed_dispatch_identity,
            selection,
            executor,
            "stage4-effect-origination-fresh-provider-key",
        );
        assert!(matches!(
            ExecutionStoreFacadeV1::new(&mut store).publish_effect_origination(duplicate_semantic),
            Err(ExecutionStoreErrorV1::LiveSemanticEffectAlreadyExists)
        ));
        assert_eq!(maximum_capacity_spent(&store), 3);
        let (_, head, generation, mut objects) = store.coherent_publication_snapshot().unwrap();
        let index_schema =
            execution_schema_id("maestro.vnext.effect-intent-control-index-schema.v1").unwrap();
        let index_position = objects
            .iter()
            .position(|object| object.schema_id() == index_schema)
            .unwrap();
        let mut tampered_index_value = objects[index_position].value().clone();
        let CborValue::Array(index_fields) = &mut tampered_index_value else {
            unreachable!("control index is an array")
        };
        let CborValue::Array(rows) = &mut index_fields[1] else {
            unreachable!("control index rows are an array")
        };
        let CborValue::Array(first_row) = &mut rows[0] else {
            unreachable!("control index row is an array")
        };
        first_row[1] = bytes(&[250; 32]);
        let tampered_index = StoreObjectV1::new(
            index_schema,
            tampered_index_value,
            objects[index_position].references().to_vec(),
        )
        .unwrap();
        objects[index_position] = tampered_index;
        assert!(matches!(
            load_active_effect_snapshot(
                &head,
                &generation,
                &objects,
                committed.intent(),
                |generation_id| Ok(store.generation(generation_id)?),
            ),
            Err(ExecutionStoreErrorV1::InvalidEffectSnapshot)
        ));
        drop(store);

        let mut reopened = StoreV1::open(&store_root, domain.clone()).unwrap();
        let selected = ExecutionStoreFacadeV1::new(&mut reopened)
            .current_effect_state_binding(committed.intent())
            .unwrap();
        assert_eq!(selected.control_head(), Some(terminal.control_head()));
        let snapshot = ExecutionStoreFacadeV1::new(&mut reopened)
            .current_effect_snapshot(committed.intent())
            .unwrap();
        assert_eq!(snapshot.intent().id(), committed.intent());
        assert_eq!(snapshot.control_head().id(), terminal.control_head());
        assert_eq!(
            snapshot.dispatch().attempt().id(),
            committed.dispatch_attempt()
        );
        assert_eq!(
            snapshot.control_revision().live_dispatch(),
            super::super::withdrawal::EffectIntentLiveDispatchV1::None
        );
        assert_eq!(
            snapshot.control_revision().classification(),
            super::super::withdrawal::RemoteClassificationV1::InDoubt
        );
        assert_eq!(
            snapshot.dispatch().terminal_classification(),
            Some(super::super::withdrawal::RemoteClassificationV1::InDoubt)
        );
        assert!(snapshot.control_revision().runs_closed());
        assert_eq!(
            snapshot.dispatch().run_set().runs()[0].state(),
            RunStateV1::Succeeded
        );
        assert!(
            !snapshot
                .dispatch()
                .state()
                .can_reconstruct_live_release_capability()
        );
        let (_, _, _, objects) = reopened.coherent_publication_snapshot().unwrap();
        let index = load_control_index(&objects).unwrap();
        assert_eq!(index.entries.len(), 1);
        let entry = index.entries[0];
        let stored_head = objects
            .iter()
            .find(|object| object.id() == entry.control_head_object_id)
            .unwrap();
        assert_eq!(stored_head.references().len(), 4);
        assert_eq!(maximum_capacity_spent(&reopened), 3);
    }

    #[test]
    fn effect_redispatch_requires_conclusive_not_applied_and_persists_higher_fence() {
        let (store_root, domain, mut store, draft, selection, executor) =
            effect_store_fixture(b"stage4-effect-redispatch");
        let origination_plan = active_effect_origination_plan(
            &mut store,
            &draft,
            selection,
            executor,
            "stage4-effect-redispatch-origin",
        );
        let originated = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_origination(origination_plan)
            .unwrap();
        let first_attempt = originated.dispatch_attempt();
        let terminal_plan = active_effect_terminal_plan(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-effect-redispatch-not-applied",
            EffectDispatchOutcomePayloadV1::LocallyRejected {
                evidence_commitment: [171; 32],
            },
        );
        let terminal = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_terminal(terminal_plan)
            .unwrap();
        assert_eq!(
            terminal.classification(),
            RemoteClassificationV1::ConfirmedNotApplied
        );
        let mut redispatch_inputs = draft.dispatch.clone();
        redispatch_inputs.attempt_revision = 2;
        redispatch_inputs.provider_run.deadline = 250;
        let redispatch_plan = active_effect_redispatch_plan(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-effect-redispatch-key",
            redispatch_inputs,
        );
        let replay_plan = redispatch_plan.clone();
        let redispatched = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_redispatch(redispatch_plan)
            .unwrap();
        assert!(!redispatched.replayed());
        assert_ne!(redispatched.dispatch_attempt(), first_attempt);
        assert_eq!(maximum_capacity_spent(&store), 3);
        let replayed = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_redispatch(replay_plan)
            .unwrap();
        assert!(replayed.replayed());
        assert_eq!(replayed.store_head(), redispatched.store_head());
        assert_eq!(maximum_capacity_spent(&store), 3);
        let (_, head, generation, objects) = store.coherent_publication_snapshot().unwrap();
        let index = load_control_index(&objects).unwrap();
        let entry = index
            .entries
            .iter()
            .find(|entry| entry.intent == originated.intent())
            .unwrap();
        let head_object = objects
            .iter()
            .find(|object| object.id() == entry.control_head_object_id)
            .unwrap();
        let intent_object = exact_referenced_schema_object(
            head_object,
            &objects,
            &[execution_schema_id("maestro.vnext.effect-intent-schema.v1").unwrap()],
        )
        .unwrap();
        let persisted_intent =
            EffectIntentV1::from_persistence_value(intent_object.value()).unwrap();
        let revision_object = exact_referenced_schema_object(
            head_object,
            &objects,
            &[
                execution_schema_id("maestro.vnext.effect-intent-control-revision-schema.v1")
                    .unwrap(),
            ],
        )
        .unwrap();
        let revision_schema =
            execution_schema_id("maestro.vnext.effect-intent-control-revision-schema.v1").unwrap();
        let predecessor_object = revision_object
            .references()
            .iter()
            .filter_map(|reference| objects.iter().find(|object| object.id() == *reference))
            .find(|object| object.schema_id() == revision_schema)
            .unwrap();
        let predecessor_revision = validate_control_revision_history(
            predecessor_object,
            &objects,
            &persisted_intent,
            &mut Vec::new(),
        )
        .unwrap();
        let revision_carrier_object = revision_object
            .references()
            .iter()
            .filter_map(|reference| objects.iter().find(|object| object.id() == *reference))
            .find(|object| {
                object.schema_id()
                    == execution_schema_id(
                        "maestro.vnext.effect-dispatch-reservation-carrier-schema.v1",
                    )
                    .unwrap()
            })
            .unwrap();
        let CborValue::Array(authorized_carrier_fields) = revision_carrier_object.value() else {
            panic!("authorized carrier must be an array")
        };
        let [CborValue::Text(_), raw_carrier, raw_authority] = authorized_carrier_fields.as_slice()
        else {
            panic!("authorized carrier must have three fields")
        };
        let (_, revision_carrier) =
            EffectDispatchAttemptV1::from_persistence_carrier_value(raw_carrier).unwrap();
        decode_execution_authority_value(raw_authority).unwrap();
        let expected_revision = revision_carrier
            .persisted_candidate_revision(&predecessor_revision)
            .unwrap();
        assert_eq!(
            expected_revision,
            EffectIntentControlRevisionV1::from_canonical_value(revision_object.value()).unwrap()
        );
        let persisted_revision = validate_control_revision_history(
            revision_object,
            &objects,
            &persisted_intent,
            &mut Vec::new(),
        )
        .unwrap();
        let dispatch_object = exact_referenced_schema_object(
            head_object,
            &objects,
            &[
                execution_schema_id("maestro.vnext.effect-dispatch-reservation-carrier-schema.v1")
                    .unwrap(),
                execution_schema_id("maestro.vnext.effect-dispatch-seal-carrier-schema.v1")
                    .unwrap(),
                execution_schema_id("maestro.vnext.effect-dispatch-terminal-carrier-schema.v1")
                    .unwrap(),
            ],
        )
        .unwrap();
        let (persisted_dispatch, persisted_need, _) = load_validated_dispatch_carrier(
            dispatch_object,
            &objects,
            &persisted_intent,
            &generation,
            &mut |generation_id| Ok(store.generation(generation_id)?),
        )
        .unwrap();
        assert!(effect_dispatch_control_is_coherent(
            originated.intent(),
            &persisted_revision,
            &persisted_dispatch,
            &persisted_need,
        ));
        assert_eq!(head, *redispatched.store_head());
        let snapshot = ExecutionStoreFacadeV1::new(&mut store)
            .current_effect_snapshot(originated.intent())
            .unwrap();
        assert_eq!(snapshot.dispatch().attempt().dispatch_fence(), 2);
        assert_eq!(
            snapshot.control_revision().classification(),
            RemoteClassificationV1::Dispatching
        );
        drop(store);

        let mut reopened = StoreV1::open(&store_root, domain).unwrap();
        let restored = ExecutionStoreFacadeV1::new(&mut reopened)
            .current_effect_snapshot(originated.intent())
            .unwrap();
        assert_eq!(
            restored.dispatch().attempt().id(),
            redispatched.dispatch_attempt()
        );
        assert_eq!(restored.dispatch().attempt().dispatch_fence(), 2);
        assert_eq!(maximum_capacity_spent(&reopened), 3);
    }

    #[test]
    fn effect_recover_reserved_has_one_seal_winner_zero_io_replay_and_restart() {
        let (store_root, domain, mut store, draft, selection, executor) =
            effect_store_fixture(b"stage4-effect-recover-reserved-seal");
        let origination_plan = active_effect_origination_plan(
            &mut store,
            &draft,
            selection,
            executor,
            "stage4-effect-recover-reserved-origin",
        );
        let originated = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_origination(origination_plan)
            .unwrap();
        drop(store);

        let mut reopened = StoreV1::open(&store_root, domain.clone()).unwrap();
        let ordinary_seal_loser = active_effect_seal_plan(
            &mut reopened,
            originated.intent(),
            selection,
            executor,
            "stage4-effect-recover-reserved-losing-seal",
            [172; 32],
        );
        let recovery_plan = active_effect_recover_reserved_plan(
            &mut reopened,
            originated.intent(),
            selection,
            executor,
            "stage4-effect-recover-reserved-seal",
            ActiveStoreEffectRecoverReservedDraftV1::seal(
                ActiveStoreEffectSealDraftV1::new([173; 32]).unwrap(),
            ),
        );
        let replay_plan = recovery_plan.clone();
        let mut recovered = ExecutionStoreFacadeV1::new(&mut reopened)
            .publish_effect_recover_reserved(recovery_plan)
            .unwrap();
        assert!(!recovered.replayed());
        let release = recovered.take_provider_release().unwrap();
        assert_eq!(release.intent(), originated.intent());
        assert_eq!(release.dispatch_attempt(), originated.dispatch_attempt());
        assert!(recovered.take_provider_release().is_none());
        assert!(matches!(
            ExecutionStoreFacadeV1::new(&mut reopened).publish_effect_seal(ordinary_seal_loser),
            Err(ExecutionStoreErrorV1::StaleExpectedStoreState)
        ));
        let mut replayed = ExecutionStoreFacadeV1::new(&mut reopened)
            .publish_effect_recover_reserved(replay_plan)
            .unwrap();
        assert!(replayed.replayed());
        assert!(replayed.take_provider_release().is_none());
        assert_eq!(maximum_capacity_spent(&reopened), 2);
        drop(reopened);

        let mut restored = StoreV1::open(&store_root, domain).unwrap();
        let snapshot = ExecutionStoreFacadeV1::new(&mut restored)
            .current_effect_snapshot(originated.intent())
            .unwrap();
        assert_eq!(
            snapshot.dispatch().attempt().id(),
            originated.dispatch_attempt()
        );
        assert!(matches!(
            snapshot.dispatch().state(),
            super::super::dispatch_state::DispatchAttemptStateV1::SealedInFlight(_)
        ));
        assert_eq!(maximum_capacity_spent(&restored), 2);
    }

    #[test]
    fn effect_recover_reserved_can_commit_exact_local_rejection_without_release() {
        let (store_root, domain, mut store, draft, selection, executor) =
            effect_store_fixture(b"stage4-effect-recover-reserved-reject");
        let origination_plan = active_effect_origination_plan(
            &mut store,
            &draft,
            selection,
            executor,
            "stage4-effect-recover-reserved-reject-origin",
        );
        let originated = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_origination(origination_plan)
            .unwrap();
        let recovery_draft = ActiveStoreEffectRecoverReservedDraftV1::reject(
            ActiveStoreEffectTerminalDraftV1::new(
                EffectDispatchOutcomePayloadV1::LocallyRejected {
                    evidence_commitment: [174; 32],
                },
            )
            .unwrap(),
        )
        .unwrap();
        let recovery_plan = active_effect_recover_reserved_plan(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-effect-recover-reserved-reject",
            recovery_draft,
        );
        let replay_plan = recovery_plan.clone();
        let mut rejected = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_recover_reserved(recovery_plan)
            .unwrap();
        assert!(!rejected.replayed());
        assert!(rejected.take_provider_release().is_none());
        let mut replayed = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_recover_reserved(replay_plan)
            .unwrap();
        assert!(replayed.replayed());
        assert!(replayed.take_provider_release().is_none());
        assert_eq!(maximum_capacity_spent(&store), 2);
        drop(store);

        let mut reopened = StoreV1::open(&store_root, domain).unwrap();
        let restored = ExecutionStoreFacadeV1::new(&mut reopened)
            .current_effect_snapshot(originated.intent())
            .unwrap();
        assert_eq!(
            restored.control_revision().classification(),
            RemoteClassificationV1::ConfirmedNotApplied
        );
        assert!(matches!(
            restored.dispatch().state(),
            super::super::dispatch_state::DispatchAttemptStateV1::Terminal(
                super::super::dispatch_state::DispatchAttemptTerminalV1::PreSealLocallyRejected(_)
            )
        ));
        assert_eq!(maximum_capacity_spent(&reopened), 2);
    }

    #[test]
    fn installation_effect_uses_installation_authority_capacity_and_restarts() {
        let (store_root, domain, mut store, draft, selection, executor) =
            effect_store_fixture_for_role(StoreRoleV1::Installation, b"stage4-installation-effect");
        assert_eq!(domain.role(), StoreRoleV1::Installation);
        assert_eq!(
            draft.domain_kind,
            EffectIntentDomainKindV1::InstallationDomain
        );
        let origination = active_effect_origination_plan(
            &mut store,
            &draft,
            selection,
            executor,
            "stage4-installation-effect-origination",
        );
        let originated = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_origination(origination)
            .unwrap();
        let seal = active_effect_seal_plan(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-installation-effect-seal",
            [159; 32],
        );
        let mut sealed = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_seal(seal)
            .unwrap();
        let release = sealed.take_provider_release().unwrap();
        let terminal_draft = provider_terminal_draft_from_outcome(
            &mut store,
            release,
            EffectDispatchOutcomePayloadV1::ResponseReceived {
                evidence_commitment: [160; 32],
                classification: RemoteClassificationV1::ConfirmedApplied,
            },
        )
        .unwrap();
        let terminal = active_effect_terminal_plan_with_draft(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-installation-effect-terminal",
            terminal_draft,
        );
        let terminal = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_terminal(terminal)
            .unwrap();
        assert_eq!(maximum_capacity_spent(&store), 3);
        drop(store);

        let mut reopened = StoreV1::open(&store_root, domain).unwrap();
        let restored = ExecutionStoreFacadeV1::new(&mut reopened)
            .current_effect_snapshot(originated.intent())
            .unwrap();
        assert_eq!(restored.control_head().id(), terminal.control_head());
        assert_eq!(
            restored.control_revision().classification(),
            RemoteClassificationV1::ConfirmedApplied
        );
        assert_eq!(maximum_capacity_spent(&reopened), 3);
    }

    #[test]
    fn installation_effect_operation_matrix_replays_restarts_and_fences_races() {
        installation_withdrawal_parity();
        installation_redispatch_and_recovery_parity();
        installation_health_and_writer_parity();
        installation_reconciliation_parity();
        installation_writer_race_parity();
    }

    fn installation_withdrawal_parity() {
        let (store_root, domain, mut store, draft, selection, executor) =
            effect_store_fixture_for_role(StoreRoleV1::Installation, b"stage4-install-withdraw");
        let origination_plan = active_effect_origination_plan(
            &mut store,
            &draft,
            selection,
            executor,
            "stage4-install-withdraw-origin",
        );
        let originated = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_origination(origination_plan)
            .unwrap();
        let terminal_plan = active_effect_terminal_plan(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-install-withdraw-terminal",
            EffectDispatchOutcomePayloadV1::LocallyRejected {
                evidence_commitment: [201; 32],
            },
        );
        ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_terminal(terminal_plan)
            .unwrap();
        let plan = active_effect_withdrawal_plan(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-install-withdraw-publish",
        );
        let replay_plan = plan.clone();
        let withdrawn = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_withdrawal(plan)
            .unwrap();
        assert_eq!(withdrawn.provider_io_operations(), 0);
        assert!(
            ExecutionStoreFacadeV1::new(&mut store)
                .publish_effect_withdrawal(replay_plan.clone())
                .unwrap()
                .replayed()
        );
        assert_eq!(maximum_capacity_spent(&store), 3);
        drop(store);

        let mut reopened = StoreV1::open(&store_root, domain).unwrap();
        let replayed = ExecutionStoreFacadeV1::new(&mut reopened)
            .publish_effect_withdrawal(replay_plan)
            .unwrap();
        assert!(replayed.replayed());
        let snapshot = ExecutionStoreFacadeV1::new(&mut reopened)
            .current_effect_snapshot(originated.intent())
            .unwrap();
        assert_eq!(
            snapshot.control_revision().classification(),
            RemoteClassificationV1::Cancelled
        );
        assert_eq!(maximum_capacity_spent(&reopened), 3);
    }

    fn installation_redispatch_and_recovery_parity() {
        let (store_root, domain, mut store, draft, selection, executor) =
            effect_store_fixture_for_role(StoreRoleV1::Installation, b"stage4-install-recover");
        let origination_plan = active_effect_origination_plan(
            &mut store,
            &draft,
            selection,
            executor,
            "stage4-install-recover-origin",
        );
        let originated = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_origination(origination_plan)
            .unwrap();
        let terminal_plan = active_effect_terminal_plan(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-install-recover-terminal",
            EffectDispatchOutcomePayloadV1::LocallyRejected {
                evidence_commitment: [202; 32],
            },
        );
        ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_terminal(terminal_plan)
            .unwrap();
        let mut dispatch = draft.dispatch.clone();
        dispatch.attempt_revision = 2;
        dispatch.provider_run.deadline = 250;
        let redispatch_plan = active_effect_redispatch_plan(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-install-redispatch",
            dispatch,
        );
        let redispatch_replay = redispatch_plan.clone();
        ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_redispatch(redispatch_plan)
            .unwrap();
        assert!(
            ExecutionStoreFacadeV1::new(&mut store)
                .publish_effect_redispatch(redispatch_replay)
                .unwrap()
                .replayed()
        );
        let recovery_plan = active_effect_recover_reserved_plan(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-install-recover-reserved",
            ActiveStoreEffectRecoverReservedDraftV1::seal(
                ActiveStoreEffectSealDraftV1::new([203; 32]).unwrap(),
            ),
        );
        let recovery_replay = recovery_plan.clone();
        let mut recovered = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_recover_reserved(recovery_plan)
            .unwrap();
        assert!(recovered.take_provider_release().is_some());
        assert!(
            ExecutionStoreFacadeV1::new(&mut store)
                .publish_effect_recover_reserved(recovery_replay.clone())
                .unwrap()
                .replayed()
        );
        assert_eq!(maximum_capacity_spent(&store), 4);
        drop(store);

        let mut reopened = StoreV1::open(&store_root, domain).unwrap();
        let mut replayed = ExecutionStoreFacadeV1::new(&mut reopened)
            .publish_effect_recover_reserved(recovery_replay)
            .unwrap();
        assert!(replayed.replayed());
        assert!(replayed.take_provider_release().is_none());
        let snapshot = ExecutionStoreFacadeV1::new(&mut reopened)
            .current_effect_snapshot(originated.intent())
            .unwrap();
        assert_eq!(snapshot.dispatch().attempt().dispatch_fence(), 2);
        assert!(matches!(
            snapshot.dispatch().state(),
            super::super::dispatch_state::DispatchAttemptStateV1::SealedInFlight(_)
        ));
        assert_eq!(maximum_capacity_spent(&reopened), 4);
    }

    fn installation_health_and_writer_parity() {
        let (store_root, domain, mut store, draft, selection, executor) =
            effect_store_fixture_for_role(StoreRoleV1::Installation, b"stage4-install-health");
        let origination_plan = active_effect_origination_plan(
            &mut store,
            &draft,
            selection,
            executor,
            "stage4-install-health-origin",
        );
        let originated = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_origination(origination_plan)
            .unwrap();
        let seal_plan = active_effect_seal_plan(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-install-health-seal",
            [204; 32],
        );
        let mut sealed = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_seal(seal_plan)
            .unwrap();
        assert!(sealed.take_provider_release().is_some());
        let health_plan = active_effect_health_plan(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-install-health-recover",
            ActiveStoreEffectHealthDraftV1::RecoverSealedInDoubt,
        );
        let health_replay = health_plan.clone();
        let health = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_health(health_plan)
            .unwrap();
        assert_eq!(
            health.health(),
            EffectIntentControlHealthV1::RecoveryRequired
        );
        assert!(
            ExecutionStoreFacadeV1::new(&mut store)
                .publish_effect_health(health_replay)
                .unwrap()
                .replayed()
        );
        let before = ExecutionStoreFacadeV1::new(&mut store)
            .current_effect_snapshot(originated.intent())
            .unwrap();
        let old_writer = before.writer_term().id();
        let handoff_plan = active_effect_writer_handoff_plan(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-install-health-handoff",
        );
        let handoff_replay = handoff_plan.clone();
        let handoff = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_writer_handoff(handoff_plan)
            .unwrap();
        assert_ne!(handoff.writer_term(), old_writer);
        assert!(
            ExecutionStoreFacadeV1::new(&mut store)
                .publish_effect_writer_handoff(handoff_replay)
                .unwrap()
                .replayed()
        );
        assert_eq!(maximum_capacity_spent(&store), 4);
        drop(store);

        let mut reopened = StoreV1::open(&store_root, domain).unwrap();
        let snapshot = ExecutionStoreFacadeV1::new(&mut reopened)
            .current_effect_snapshot(originated.intent())
            .unwrap();
        assert_eq!(
            snapshot.control_revision().health(),
            EffectIntentControlHealthV1::Healthy
        );
        assert_eq!(snapshot.writer_term().id(), handoff.writer_term());
        assert_eq!(maximum_capacity_spent(&reopened), 4);
    }

    fn installation_reconciliation_parity() {
        let (store_root, domain, mut store, draft, selection, executor) =
            effect_store_fixture_for_role(StoreRoleV1::Installation, b"stage4-install-reconcile");
        let origination_plan = active_effect_origination_plan(
            &mut store,
            &draft,
            selection,
            executor,
            "stage4-install-reconcile-origin",
        );
        let originated = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_origination(origination_plan)
            .unwrap();
        let seal_plan = active_effect_seal_plan(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-install-reconcile-seal",
            [205; 32],
        );
        let mut sealed = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_seal(seal_plan)
            .unwrap();
        let release = sealed.take_provider_release().unwrap();
        let dispatch_terminal_draft = provider_terminal_draft_from_outcome(
            &mut store,
            release,
            EffectDispatchOutcomePayloadV1::AmbiguousTransport {
                evidence_commitment: [206; 32],
            },
        )
        .unwrap();
        let dispatch_terminal_plan = active_effect_terminal_plan_with_draft(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-install-reconcile-terminal",
            dispatch_terminal_draft,
        );
        ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_terminal(dispatch_terminal_plan)
            .unwrap();
        let begin_plan = active_effect_reconciliation_begin_plan(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-install-reconcile-begin",
        );
        let begin_replay = begin_plan.clone();
        let mut begun = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_reconciliation_begin(begin_plan)
            .unwrap();
        let read_release = begun.take_read_release().unwrap();
        assert!(
            ExecutionStoreFacadeV1::new(&mut store)
                .publish_effect_reconciliation_begin(begin_replay)
                .unwrap()
                .replayed()
        );
        let read_draft = reconciliation_read_draft(
            &mut store,
            read_release,
            [207; 32],
            ProviderApplicationFactV1::NotApplied,
        );
        let read_plan =
            active_effect_reconciliation_read_plan(&mut store, originated.intent(), read_draft);
        let read_replay = read_plan.clone();
        ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_reconciliation_read(read_plan)
            .unwrap();
        assert!(
            ExecutionStoreFacadeV1::new(&mut store)
                .publish_effect_reconciliation_read(read_replay)
                .unwrap()
                .replayed()
        );
        let terminal_plan = active_effect_reconciliation_terminal_plan(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-install-reconcile-finish",
            RemoteClassificationV1::ConfirmedNotApplied,
        );
        let terminal_replay = terminal_plan.clone();
        let terminal = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_reconciliation_terminal(terminal_plan)
            .unwrap();
        assert_eq!(
            terminal.classification(),
            RemoteClassificationV1::ConfirmedNotApplied
        );
        assert!(
            ExecutionStoreFacadeV1::new(&mut store)
                .publish_effect_reconciliation_terminal(terminal_replay)
                .unwrap()
                .replayed()
        );
        assert_eq!(maximum_capacity_spent(&store), 4);
        drop(store);

        let mut reopened = StoreV1::open(&store_root, domain).unwrap();
        let snapshot = ExecutionStoreFacadeV1::new(&mut reopened)
            .current_effect_snapshot(originated.intent())
            .unwrap();
        assert_eq!(
            snapshot.control_revision().classification(),
            RemoteClassificationV1::ConfirmedNotApplied
        );
        assert!(snapshot.control_revision().runs_closed());
        assert_eq!(maximum_capacity_spent(&reopened), 4);
    }

    fn installation_writer_race_parity() {
        let (store_root, domain, mut store, draft, selection, executor) =
            effect_store_fixture_for_role(StoreRoleV1::Installation, b"stage4-install-race");
        let origination_plan = active_effect_origination_plan(
            &mut store,
            &draft,
            selection,
            executor,
            "stage4-install-race-origin",
        );
        let originated = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_origination(origination_plan)
            .unwrap();
        let contenders = [
            active_effect_writer_handoff_plan(
                &mut store,
                originated.intent(),
                selection,
                executor,
                "stage4-install-race-a",
            ),
            active_effect_writer_handoff_plan(
                &mut store,
                originated.intent(),
                selection,
                executor,
                "stage4-install-race-b",
            ),
        ];
        drop(store);
        let barrier = Arc::new(Barrier::new(3));
        let workers = contenders.map(|plan| {
            let barrier = Arc::clone(&barrier);
            let store_root = store_root.clone();
            let domain = domain.clone();
            std::thread::spawn(move || {
                let mut store = StoreV1::open(store_root, domain).unwrap();
                barrier.wait();
                ExecutionStoreFacadeV1::new(&mut store).publish_effect_writer_handoff(plan)
            })
        });
        barrier.wait();
        let outcomes = workers.map(|worker| worker.join().unwrap());
        assert_eq!(outcomes.iter().filter(|outcome| outcome.is_ok()).count(), 1);
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| matches!(
                    outcome,
                    Err(ExecutionStoreErrorV1::StaleExpectedStoreState)
                ))
                .count(),
            1
        );
        let mut reopened = StoreV1::open(&store_root, domain).unwrap();
        assert_eq!(reopened.domain().role(), StoreRoleV1::Installation);
        assert_eq!(maximum_capacity_spent(&reopened), 2);
        let snapshot = ExecutionStoreFacadeV1::new(&mut reopened)
            .current_effect_snapshot(originated.intent())
            .unwrap();
        assert!(snapshot.writer_term().prior_writer_term().is_some());
    }

    #[test]
    fn bootstrap_g0_effect_route_is_exact_authorized_reconcilable_and_withdrawable() {
        exercise_specialized_effect_route(
            EffectOriginKindV1::BootstrapMandatePresentationDeliveryOrigin,
            b"stage4-bootstrap-route-reconciliation",
            b"stage4-bootstrap-route-withdrawal",
        );
    }

    #[test]
    fn continuity_maintenance_effect_route_is_slot_authorized_reconcilable_and_withdrawable() {
        exercise_specialized_effect_route(
            EffectOriginKindV1::MaintenanceExecutorCurrentnessEffectOrigin,
            b"stage4-cma-route-reconciliation",
            b"stage4-cma-route-withdrawal",
        );
    }

    #[test]
    fn specialized_effect_authority_rejects_cross_basis_and_substituted_carriers_without_debit() {
        let (_, _, mut bootstrap_store, bootstrap_draft, bootstrap_fixture) =
            specialized_effect_store_fixture(
                b"stage4-bootstrap-negative-authority",
                EffectOriginKindV1::BootstrapMandatePresentationDeliveryOrigin,
            );
        let request = ExecutionStoreFacadeV1::new(&mut bootstrap_store)
            .canonical_effect_origination_request(
                &bootstrap_draft,
                IdempotencyKeyIdV1::derive("stage4-bootstrap-wrong-genesis").unwrap(),
            )
            .unwrap();
        let state = ExecutionStoreFacadeV1::new(&mut bootstrap_store)
            .current_state_binding()
            .unwrap();
        let bad_basis = BootstrapControlG0AuthorityBasisV1::new(
            bootstrap_fixture.bootstrap_basis.binding_id,
            bootstrap_fixture.bootstrap_basis.session_id,
            GenesisGrantIdV1::derive("stage4-substituted-genesis-grant").unwrap(),
        );
        let bad_authority = BootstrapExecutionAuthorityV1::new(
            bad_basis,
            RepositoryActionLeafV1::ReserveBootstrapMandateInteractionEffect,
            request.subject_commitment(),
            request.expected_state_commitment(),
            request.payload_commitment(),
            bootstrap_fixture.actor_principal,
        )
        .unwrap();
        let plan = ActiveStoreEffectOriginationPublicationV1::new(
            request.clone(),
            bad_authority,
            state,
            bootstrap_draft,
        )
        .unwrap();
        assert!(matches!(
            ExecutionStoreFacadeV1::new(&mut bootstrap_store).publish_effect_origination(plan),
            Err(ExecutionStoreErrorV1::AuthorityAdmissionFailed)
        ));
        assert_eq!(maximum_capacity_spent(&bootstrap_store), 0);
        assert!(
            GenericExecutionAuthorityV1::new(
                bootstrap_fixture.selection,
                RepositoryActionLeafV1::ReserveBootstrapMandateInteractionEffect,
                request.subject_commitment(),
                request.expected_state_commitment(),
                request.payload_commitment(),
                bootstrap_fixture.actor_principal,
            )
            .is_err()
        );
        assert!(
            BootstrapExecutionAuthorityV1::new(
                bootstrap_fixture.bootstrap_basis,
                RepositoryActionLeafV1::ReserveContinuityMaintenanceEffect,
                request.subject_commitment(),
                request.expected_state_commitment(),
                request.payload_commitment(),
                bootstrap_fixture.actor_principal,
            )
            .is_err()
        );

        let (_, _, mut cma_store, cma_draft, cma_fixture) = specialized_effect_store_fixture(
            b"stage4-cma-negative-authority",
            EffectOriginKindV1::MaintenanceExecutorCurrentnessEffectOrigin,
        );
        let reserve_entry = cma_fixture
            .cma_bases
            .iter()
            .find(|(action, ..)| {
                *action == RepositoryActionLeafV1::ReserveContinuityMaintenanceEffect
            })
            .copied()
            .unwrap();
        let high_tag_request = ExecutionStoreFacadeV1::new(&mut cma_store)
            .canonical_effect_origination_request(
                &cma_draft,
                IdempotencyKeyIdV1::derive("stage4-cma-high-withdrawal-tag").unwrap(),
            )
            .unwrap();
        let high_tag_authority =
            specialized_execution_authority(&cma_fixture, &high_tag_request, 0);
        let CborValue::Array(mut high_tag_value) =
            execution_authority_value(&high_tag_authority).unwrap()
        else {
            unreachable!("execution authority encoding is an array")
        };
        high_tag_value[10] =
            CborValue::Array(vec![CborValue::Unsigned(1), CborValue::Unsigned(257)]);
        assert!(matches!(
            decode_execution_authority_value(&CborValue::Array(high_tag_value)),
            Err(ExecutionStoreErrorV1::InvalidEffectSnapshot)
        ));
        let wrong_purpose_request = ExecutionStoreFacadeV1::new(&mut cma_store)
            .canonical_effect_origination_request(
                &cma_draft,
                IdempotencyKeyIdV1::derive("stage4-cma-wrong-purpose").unwrap(),
            )
            .unwrap();
        let wrong_purpose_authority = ContinuityMaintenanceExecutionAuthorityV1::new(
            reserve_entry.1,
            None,
            CmaObservationPublicationPurposeV1::ProspectiveContinuityCarrier,
            RepositoryActionLeafV1::ReserveContinuityMaintenanceEffect,
            wrong_purpose_request.subject_commitment(),
            wrong_purpose_request.expected_state_commitment(),
            wrong_purpose_request.payload_commitment(),
            cma_fixture.actor_principal,
            cma_fixture.continuity_state_token,
            cma_fixture.continuity_state_object_id,
            cma_fixture.guard_object_id,
            cma_fixture.authority_epoch,
            reserve_entry.4,
        )
        .unwrap();
        assert!(matches!(
            ActiveStoreEffectOriginationPublicationV1::new(
                wrong_purpose_request,
                wrong_purpose_authority,
                ExecutionStoreFacadeV1::new(&mut cma_store)
                    .current_state_binding()
                    .unwrap(),
                cma_draft.clone(),
            ),
            Err(ExecutionStoreErrorV1::PublicationBindingMismatch)
        ));
        let wrong_epoch_request = ExecutionStoreFacadeV1::new(&mut cma_store)
            .canonical_effect_origination_request(
                &cma_draft,
                IdempotencyKeyIdV1::derive("stage4-cma-wrong-epoch").unwrap(),
            )
            .unwrap();
        let wrong_epoch_authority = ContinuityMaintenanceExecutionAuthorityV1::new(
            reserve_entry.1,
            None,
            reserve_entry.3,
            RepositoryActionLeafV1::ReserveContinuityMaintenanceEffect,
            wrong_epoch_request.subject_commitment(),
            wrong_epoch_request.expected_state_commitment(),
            wrong_epoch_request.payload_commitment(),
            cma_fixture.actor_principal,
            cma_fixture.continuity_state_token,
            cma_fixture.continuity_state_object_id,
            cma_fixture.guard_object_id,
            cma_fixture.authority_epoch + 1,
            reserve_entry.4,
        )
        .unwrap();
        let wrong_epoch_plan = ActiveStoreEffectOriginationPublicationV1::new(
            wrong_epoch_request,
            wrong_epoch_authority,
            ExecutionStoreFacadeV1::new(&mut cma_store)
                .current_state_binding()
                .unwrap(),
            cma_draft.clone(),
        )
        .unwrap();
        assert!(matches!(
            ExecutionStoreFacadeV1::new(&mut cma_store)
                .publish_effect_origination(wrong_epoch_plan),
            Err(ExecutionStoreErrorV1::AuthorityAdmissionFailed)
        ));
        assert_eq!(maximum_capacity_spent(&cma_store), 0);
        for (key, basis, executor) in [
            (
                "stage4-cma-wrong-slot",
                ContinuityMaintenanceAuthorityBasisV1::new(
                    reserve_entry.1.cma_branch_id,
                    SlotIdV1::derive("stage4-substituted-cma-slot").unwrap(),
                    reserve_entry.1.executor_assertion_id,
                ),
                cma_fixture.actor_principal,
            ),
            (
                "stage4-cma-wrong-executor",
                reserve_entry.1,
                PrincipalIdV1::derive("stage4-substituted-cma-executor").unwrap(),
            ),
        ] {
            let request = ExecutionStoreFacadeV1::new(&mut cma_store)
                .canonical_effect_origination_request(
                    &cma_draft,
                    IdempotencyKeyIdV1::derive(key).unwrap(),
                )
                .unwrap();
            let state = ExecutionStoreFacadeV1::new(&mut cma_store)
                .current_state_binding()
                .unwrap();
            let authority = ContinuityMaintenanceExecutionAuthorityV1::new(
                basis,
                None,
                reserve_entry.3,
                RepositoryActionLeafV1::ReserveContinuityMaintenanceEffect,
                request.subject_commitment(),
                request.expected_state_commitment(),
                request.payload_commitment(),
                executor,
                cma_fixture.continuity_state_token,
                cma_fixture.continuity_state_object_id,
                cma_fixture.guard_object_id,
                cma_fixture.authority_epoch,
                reserve_entry.4,
            )
            .unwrap();
            let plan = ActiveStoreEffectOriginationPublicationV1::new(
                request,
                authority,
                state,
                cma_draft.clone(),
            )
            .unwrap();
            assert!(matches!(
                ExecutionStoreFacadeV1::new(&mut cma_store).publish_effect_origination(plan),
                Err(ExecutionStoreErrorV1::AuthorityAdmissionFailed)
            ));
            assert_eq!(maximum_capacity_spent(&cma_store), 0);
        }

        let head_before_mismatch_cases = cma_store.coherent_publication_snapshot().unwrap().1;
        let wrong_applicability_request = ExecutionStoreFacadeV1::new(&mut cma_store)
            .canonical_effect_origination_request(
                &cma_draft,
                IdempotencyKeyIdV1::derive("stage4-cma-wrong-applicability").unwrap(),
            )
            .unwrap();
        let wrong_applicability = ContinuityMaintenanceExecutionAuthorityV1::new(
            reserve_entry.1,
            None,
            reserve_entry.3,
            RepositoryActionLeafV1::ReserveContinuityMaintenanceEffect,
            wrong_applicability_request.subject_commitment(),
            wrong_applicability_request.expected_state_commitment(),
            wrong_applicability_request.payload_commitment(),
            cma_fixture.actor_principal,
            cma_fixture.continuity_state_token,
            cma_fixture.continuity_state_object_id,
            cma_fixture.guard_object_id,
            cma_fixture.authority_epoch,
            [222; 32],
        )
        .unwrap();
        let wrong_applicability_plan = ActiveStoreEffectOriginationPublicationV1::new(
            wrong_applicability_request,
            wrong_applicability,
            ExecutionStoreFacadeV1::new(&mut cma_store)
                .current_state_binding()
                .unwrap(),
            cma_draft.clone(),
        )
        .unwrap();
        assert!(matches!(
            ExecutionStoreFacadeV1::new(&mut cma_store)
                .publish_effect_origination(wrong_applicability_plan),
            Err(ExecutionStoreErrorV1::AuthorityAdmissionFailed)
        ));

        let wrong_subject_request = ExecutionStoreFacadeV1::new(&mut cma_store)
            .canonical_effect_origination_request(
                &cma_draft,
                IdempotencyKeyIdV1::derive("stage4-cma-wrong-subject").unwrap(),
            )
            .unwrap();
        let wrong_subject = ContinuityMaintenanceExecutionAuthorityV1::new(
            reserve_entry.1,
            None,
            reserve_entry.3,
            RepositoryActionLeafV1::ReserveContinuityMaintenanceEffect,
            [223; 32],
            wrong_subject_request.expected_state_commitment(),
            wrong_subject_request.payload_commitment(),
            cma_fixture.actor_principal,
            cma_fixture.continuity_state_token,
            cma_fixture.continuity_state_object_id,
            cma_fixture.guard_object_id,
            cma_fixture.authority_epoch,
            reserve_entry.4,
        )
        .unwrap();
        assert!(matches!(
            ActiveStoreEffectOriginationPublicationV1::new(
                wrong_subject_request,
                wrong_subject,
                ExecutionStoreFacadeV1::new(&mut cma_store)
                    .current_state_binding()
                    .unwrap(),
                cma_draft.clone(),
            ),
            Err(ExecutionStoreErrorV1::PublicationBindingMismatch)
        ));

        let original_request = ExecutionStoreFacadeV1::new(&mut cma_store)
            .canonical_effect_origination_request(
                &cma_draft,
                IdempotencyKeyIdV1::derive("stage4-cma-original-request").unwrap(),
            )
            .unwrap();
        let original_authority = ContinuityMaintenanceExecutionAuthorityV1::new(
            reserve_entry.1,
            None,
            reserve_entry.3,
            RepositoryActionLeafV1::ReserveContinuityMaintenanceEffect,
            original_request.subject_commitment(),
            original_request.expected_state_commitment(),
            original_request.payload_commitment(),
            cma_fixture.actor_principal,
            cma_fixture.continuity_state_token,
            cma_fixture.continuity_state_object_id,
            cma_fixture.guard_object_id,
            cma_fixture.authority_epoch,
            reserve_entry.4,
        )
        .unwrap();
        let mut substituted_draft = cma_draft.clone();
        substituted_draft.dispatch.provider_key_commitment = [224; 32];
        let substituted_request = ExecutionStoreFacadeV1::new(&mut cma_store)
            .canonical_effect_origination_request(
                &substituted_draft,
                IdempotencyKeyIdV1::derive("stage4-cma-substituted-request").unwrap(),
            )
            .unwrap();
        assert!(matches!(
            ActiveStoreEffectOriginationPublicationV1::new(
                substituted_request,
                original_authority,
                ExecutionStoreFacadeV1::new(&mut cma_store)
                    .current_state_binding()
                    .unwrap(),
                substituted_draft,
            ),
            Err(ExecutionStoreErrorV1::PublicationBindingMismatch)
        ));
        assert_eq!(maximum_capacity_spent(&cma_store), 0);
        assert_eq!(
            cma_store.coherent_publication_snapshot().unwrap().1,
            head_before_mismatch_cases
        );

        let origination = specialized_effect_origination_plan(
            &mut cma_store,
            &cma_draft,
            &cma_fixture,
            "stage4-cma-negative-valid-origination",
        );
        let originated = ExecutionStoreFacadeV1::new(&mut cma_store)
            .publish_effect_origination(origination)
            .unwrap();
        let snapshot = ExecutionStoreFacadeV1::new(&mut cma_store)
            .current_effect_snapshot(originated.intent())
            .unwrap();
        let seal_draft = ActiveStoreEffectSealDraftV1::new([170; 32]).unwrap();
        let seal_request = ExecutionStoreFacadeV1::new(&mut cma_store)
            .canonical_effect_seal_request(
                &snapshot,
                seal_draft,
                IdempotencyKeyIdV1::derive("stage4-cma-wrong-action-slot").unwrap(),
            )
            .unwrap();
        let wrong_action_authority = ContinuityMaintenanceExecutionAuthorityV1::new(
            reserve_entry.1,
            None,
            reserve_entry.3,
            RepositoryActionLeafV1::PublishContinuityMaintenanceEffectOutcome,
            seal_request.subject_commitment(),
            seal_request.expected_state_commitment(),
            seal_request.payload_commitment(),
            cma_fixture.actor_principal,
            cma_fixture.continuity_state_token,
            cma_fixture.continuity_state_object_id,
            cma_fixture.guard_object_id,
            cma_fixture.authority_epoch,
            reserve_entry.4,
        )
        .unwrap();
        let plan = ActiveStoreEffectSealPublicationV1::new(
            seal_request,
            wrong_action_authority,
            snapshot,
            seal_draft,
        )
        .unwrap();
        assert!(matches!(
            ExecutionStoreFacadeV1::new(&mut cma_store).publish_effect_seal(plan),
            Err(ExecutionStoreErrorV1::AuthorityAdmissionFailed)
        ));
        assert_eq!(maximum_capacity_spent(&cma_store), 1);

        let terminal = specialized_effect_terminal_plan(
            &mut cma_store,
            originated.intent(),
            &cma_fixture,
            "stage4-cma-negative-valid-terminal",
            ActiveStoreEffectTerminalDraftV1::new(
                EffectDispatchOutcomePayloadV1::LocallyRejected {
                    evidence_commitment: [171; 32],
                },
            )
            .unwrap(),
        );
        ExecutionStoreFacadeV1::new(&mut cma_store)
            .publish_effect_terminal(terminal)
            .unwrap();
        let snapshot = ExecutionStoreFacadeV1::new(&mut cma_store)
            .current_effect_snapshot(originated.intent())
            .unwrap();
        let withdrawal_draft = ActiveStoreEffectWithdrawalDraftV1::new();
        let withdrawal_request = ExecutionStoreFacadeV1::new(&mut cma_store)
            .canonical_effect_withdrawal_request(
                &snapshot,
                withdrawal_draft,
                IdempotencyKeyIdV1::derive("stage4-cma-wrong-withdrawal-family").unwrap(),
            )
            .unwrap();
        let (_, withdrawal_basis, _, withdrawal_purpose, withdrawal_applicability) = cma_fixture
            .cma_bases
            .iter()
            .find(|(action, ..)| {
                *action == RepositoryActionLeafV1::WithdrawContinuityMaintenanceEffect
            })
            .copied()
            .unwrap();
        let wrong_family_authority = ContinuityMaintenanceExecutionAuthorityV1::new(
            withdrawal_basis,
            Some(CmaEffectWithdrawalSlotFamilyV1::ProspectiveContinuityCarrier),
            withdrawal_purpose,
            RepositoryActionLeafV1::WithdrawContinuityMaintenanceEffect,
            withdrawal_request.subject_commitment(),
            withdrawal_request.expected_state_commitment(),
            withdrawal_request.payload_commitment(),
            cma_fixture.actor_principal,
            cma_fixture.continuity_state_token,
            cma_fixture.continuity_state_object_id,
            cma_fixture.guard_object_id,
            cma_fixture.authority_epoch,
            withdrawal_applicability,
        );
        assert!(matches!(
            wrong_family_authority,
            Err(
                crate::domain::authority::RepositoryLeafAuthorityErrorV1::ExecutionAuthorityBasisMismatch
            )
        ));
        assert_eq!(maximum_capacity_spent(&cma_store), 2);
    }

    #[test]
    fn effect_reconciliation_is_fresh_authorized_bounded_atomic_and_restart_decodable() {
        let (store_root, domain, mut store, draft, selection, executor) =
            effect_store_fixture(b"stage4-effect-reconciliation");
        let origination_plan = active_effect_origination_plan(
            &mut store,
            &draft,
            selection,
            executor,
            "stage4-reconciliation-origination",
        );
        let originated = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_origination(origination_plan)
            .unwrap();
        let seal_plan = active_effect_seal_plan(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-reconciliation-seal",
            [121; 32],
        );
        let mut sealed = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_seal(seal_plan)
            .unwrap();
        let release = sealed.take_provider_release().unwrap();
        let dispatch_terminal_draft = provider_terminal_draft_from_outcome(
            &mut store,
            release,
            EffectDispatchOutcomePayloadV1::AmbiguousTransport {
                evidence_commitment: [122; 32],
            },
        )
        .unwrap();
        let dispatch_terminal_plan = active_effect_terminal_plan_with_draft(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-reconciliation-dispatch-terminal",
            dispatch_terminal_draft,
        );
        let terminal = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_terminal(dispatch_terminal_plan)
            .unwrap();
        assert_eq!(terminal.classification(), RemoteClassificationV1::InDoubt);
        assert_eq!(maximum_capacity_spent(&store), 3);

        let begin_plan = active_effect_reconciliation_begin_plan(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-reconciliation-begin",
        );
        let begin_request = begin_plan.request.request_id();
        let begin_replay_plan = begin_plan.clone();
        let mut begin = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_reconciliation_begin(begin_plan)
            .unwrap();
        let read_draft = reconciliation_read_draft(
            &mut store,
            begin.take_read_release().unwrap(),
            [143; 32],
            ProviderApplicationFactV1::Applied,
        );
        assert!(!begin.replayed());
        assert_eq!(begin.classification(), RemoteClassificationV1::InDoubt);
        assert_eq!(maximum_capacity_spent(&store), 4);
        drop(store);

        let mut store = StoreV1::open(&store_root, domain.clone()).unwrap();
        let mut replay = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_reconciliation_begin(begin_replay_plan)
            .unwrap();
        assert!(replay.replayed());
        assert!(replay.take_read_release().is_none());
        assert_eq!(replay.control_head(), begin.control_head());
        assert_eq!(maximum_capacity_spent(&store), 4);
        let begun = ExecutionStoreFacadeV1::new(&mut store)
            .current_effect_snapshot(originated.intent())
            .unwrap();
        let reconciliation = begun.reconciliation().unwrap();
        assert_eq!(reconciliation.attempt().action_request_id(), begin_request);
        assert!(!reconciliation.has_step_lease_authority());
        assert!(!reconciliation.may_mutate_originating_step());
        assert_eq!(reconciliation.run_set().runs().len(), 1);
        assert_eq!(
            reconciliation.run_set().runs()[0].state(),
            RunStateV1::Reserved
        );
        assert_eq!(
            reconciliation.run_set().runs()[0].owner(),
            super::super::runtime::ExecutionAttemptOwnerV1::Reconciliation(
                reconciliation.attempt().id(),
            )
        );

        let read_plan =
            active_effect_reconciliation_read_plan(&mut store, originated.intent(), read_draft);
        let read_request = begin_request;
        let read_replay_plan = read_plan.clone();
        let read = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_reconciliation_read(read_plan)
            .unwrap();
        assert!(!read.replayed());
        assert_eq!(maximum_capacity_spent(&store), 4);
        let replay = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_reconciliation_read(read_replay_plan)
            .unwrap();
        assert!(replay.replayed());
        assert_eq!(replay.control_head(), read.control_head());
        assert_eq!(maximum_capacity_spent(&store), 4);
        drop(store);

        let mut store = StoreV1::open(&store_root, domain.clone()).unwrap();
        let read_snapshot = ExecutionStoreFacadeV1::new(&mut store)
            .current_effect_snapshot(originated.intent())
            .unwrap();
        let reconciliation = read_snapshot.reconciliation().unwrap();
        assert_eq!(
            reconciliation.read_execution_request_id(),
            Some(read_request)
        );
        assert_eq!(
            reconciliation.run_set().runs()[0].state(),
            RunStateV1::Succeeded
        );
        assert!(matches!(
            reconciliation.request_additional_poll(),
            Err(EffectRuntimeErrorV1::SecondReconciliationPollRequiresNewAttempt)
        ));

        let terminal_plan = active_effect_reconciliation_terminal_plan(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-reconciliation-terminal",
            RemoteClassificationV1::ConfirmedApplied,
        );
        let terminal_replay_plan = terminal_plan.clone();
        let reconciled = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_reconciliation_terminal(terminal_plan)
            .unwrap();
        assert!(!reconciled.replayed());
        assert_eq!(
            reconciled.classification(),
            RemoteClassificationV1::ConfirmedApplied
        );
        assert_eq!(maximum_capacity_spent(&store), 4);
        let replay = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_reconciliation_terminal(terminal_replay_plan)
            .unwrap();
        assert!(replay.replayed());
        assert_eq!(replay.control_head(), reconciled.control_head());
        assert_eq!(maximum_capacity_spent(&store), 4);
        drop(store);

        let mut reopened = StoreV1::open(&store_root, domain).unwrap();
        let final_snapshot = ExecutionStoreFacadeV1::new(&mut reopened)
            .current_effect_snapshot(originated.intent())
            .unwrap();
        assert!(final_snapshot.control_revision().live_attempt().is_none());
        assert!(final_snapshot.control_revision().runs_closed());
        assert_eq!(
            final_snapshot.control_revision().classification(),
            RemoteClassificationV1::ConfirmedApplied
        );
        assert_eq!(
            final_snapshot.reconciliation().unwrap().run_set().runs()[0].state(),
            RunStateV1::Succeeded
        );
        assert_eq!(maximum_capacity_spent(&reopened), 4);

        let (_, _, _, mut objects) = reopened.coherent_publication_snapshot().unwrap();
        let terminal_schema =
            execution_schema_id("maestro.vnext.effect-reconciliation-terminal-carrier-schema.v1")
                .unwrap();
        let terminal_occurrence_schema = execution_schema_id(
            "maestro.vnext.effect-reconciliation-terminal-occurrence-schema.v1",
        )
        .unwrap();
        let terminal_carrier = objects
            .iter()
            .find(|object| object.schema_id() == terminal_schema)
            .unwrap()
            .clone();
        let terminal_occurrence = objects
            .iter()
            .find(|object| {
                object.schema_id() == terminal_occurrence_schema
                    && terminal_carrier.references().contains(&object.id())
            })
            .unwrap()
            .clone();
        let mut rewritten_occurrence_value = terminal_occurrence.value().clone();
        let CborValue::Array(occurrence_fields) = &mut rewritten_occurrence_value else {
            unreachable!()
        };
        occurrence_fields[2] = CborValue::Unsigned(6);
        let rewritten_occurrence = StoreObjectV1::new(
            terminal_occurrence_schema,
            rewritten_occurrence_value,
            terminal_occurrence.references().to_vec(),
        )
        .unwrap();
        let mut rewritten_carrier_value = terminal_carrier.value().clone();
        let CborValue::Array(wrapper_fields) = &mut rewritten_carrier_value else {
            unreachable!()
        };
        let CborValue::Array(carrier_fields) = &mut wrapper_fields[1] else {
            unreachable!()
        };
        let CborValue::Array(control_need_fields) = &mut carrier_fields[2] else {
            unreachable!()
        };
        let CborValue::Array(control_need_body) = &mut control_need_fields[1] else {
            unreachable!()
        };
        control_need_body[3] = CborValue::Unsigned(6);
        let (_, rewritten_need, _) =
            decode_effect_reconciliation_authorized_carrier(&rewritten_carrier_value).unwrap();
        assert!(matches!(
            rewritten_need,
            EffectControlTransitionNeedV1::FinishReconciliation {
                classification: RemoteClassificationV1::ConfirmedNotApplied,
                ..
            }
        ));
        let mut rewritten_references = terminal_carrier.references().to_vec();
        let selected = rewritten_references
            .iter_mut()
            .find(|reference| **reference == terminal_occurrence.id())
            .unwrap();
        *selected = rewritten_occurrence.id();
        rewritten_references.sort_unstable();
        let rewritten_carrier = StoreObjectV1::new(
            terminal_schema,
            rewritten_carrier_value,
            rewritten_references,
        )
        .unwrap();
        objects.extend([rewritten_occurrence, rewritten_carrier.clone()]);
        assert!(matches!(
            load_validated_reconciliation_carrier(
                &rewritten_carrier,
                &objects,
                final_snapshot.intent(),
            ),
            Err(ExecutionStoreErrorV1::InvalidEffectSnapshot)
        ));
    }

    #[test]
    fn reconciliation_continues_after_interleaved_unrelated_authorized_publication() {
        let (_store_root, domain, mut store, draft, selection, executor) =
            effect_store_fixture(b"stage4-reconciliation-authority-interleave");
        let origination = active_effect_origination_plan(
            &mut store,
            &draft,
            selection,
            executor,
            "stage4-reconciliation-interleave-origin",
        );
        let originated = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_origination(origination)
            .unwrap();
        let seal = active_effect_seal_plan(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-reconciliation-interleave-seal",
            [221; 32],
        );
        let mut sealed = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_seal(seal)
            .unwrap();
        let terminal_draft = provider_terminal_draft_from_outcome(
            &mut store,
            sealed.take_provider_release().unwrap(),
            EffectDispatchOutcomePayloadV1::AmbiguousTransport {
                evidence_commitment: [222; 32],
            },
        )
        .unwrap();
        let terminal = active_effect_terminal_plan_with_draft(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-reconciliation-interleave-dispatch-terminal",
            terminal_draft,
        );
        ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_terminal(terminal)
            .unwrap();
        let begin = active_effect_reconciliation_begin_plan(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-reconciliation-interleave-begin",
        );
        let mut begun = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_reconciliation_begin(begin)
            .unwrap();
        let read_draft = reconciliation_read_draft(
            &mut store,
            begun.take_read_release().unwrap(),
            [223; 32],
            ProviderApplicationFactV1::Applied,
        );
        assert_eq!(maximum_capacity_spent(&store), 4);

        let unrelated_draft = unrelated_effect_origination_draft(StoreRoleV1::Repository, &domain);
        let unrelated = active_effect_origination_plan(
            &mut store,
            &unrelated_draft,
            selection,
            executor,
            "stage4-reconciliation-interleave-unrelated",
        );
        ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_origination(unrelated)
            .unwrap();
        assert_eq!(maximum_capacity_spent(&store), 5);

        let read =
            active_effect_reconciliation_read_plan(&mut store, originated.intent(), read_draft);
        ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_reconciliation_read(read)
            .unwrap();
        let terminal = active_effect_reconciliation_terminal_plan(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-reconciliation-interleave-terminal",
            RemoteClassificationV1::ConfirmedApplied,
        );
        let reconciled = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_reconciliation_terminal(terminal)
            .unwrap();
        assert_eq!(
            reconciled.classification(),
            RemoteClassificationV1::ConfirmedApplied
        );
        assert_eq!(maximum_capacity_spent(&store), 5);
    }

    #[test]
    fn effect_withdrawal_is_authorized_atomic_zero_io_replayable_and_restart_decodable() {
        let (store_root, domain, mut store, draft, selection, executor) =
            effect_store_fixture(b"stage4-effect-withdrawal");
        let origination_plan = active_effect_origination_plan(
            &mut store,
            &draft,
            selection,
            executor,
            "stage4-withdrawal-origination",
        );
        let origination = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_origination(origination_plan)
            .unwrap();
        let terminal_plan = active_effect_terminal_plan(
            &mut store,
            origination.intent(),
            selection,
            executor,
            "stage4-withdrawal-terminal",
            EffectDispatchOutcomePayloadV1::LocallyRejected {
                evidence_commitment: [151; 32],
            },
        );
        let terminal = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_terminal(terminal_plan)
            .unwrap();
        assert_eq!(
            terminal.classification(),
            RemoteClassificationV1::ConfirmedNotApplied
        );
        assert_eq!(maximum_capacity_spent(&store), 2);
        let before = ExecutionStoreFacadeV1::new(&mut store)
            .current_effect_snapshot(origination.intent())
            .unwrap();
        let before_dispatch = before.dispatch().clone();
        let before_attempt_history = before.control_revision().attempt_history().to_vec();
        let plan = active_effect_withdrawal_plan(
            &mut store,
            origination.intent(),
            selection,
            executor,
            "stage4-withdrawal",
        );
        let replay_plan = plan.clone();
        let withdrawn = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_withdrawal(plan)
            .unwrap();
        assert!(!withdrawn.replayed());
        assert_eq!(withdrawn.provider_io_operations(), 0);
        assert_eq!(maximum_capacity_spent(&store), 3);
        let replay = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_withdrawal(replay_plan.clone())
            .unwrap();
        assert!(replay.replayed());
        assert_eq!(replay.provider_io_operations(), 0);
        assert_eq!(replay.control_head(), withdrawn.control_head());
        assert_eq!(maximum_capacity_spent(&store), 3);
        drop(store);

        let mut reopened = StoreV1::open(&store_root, domain).unwrap();
        let replay = ExecutionStoreFacadeV1::new(&mut reopened)
            .publish_effect_withdrawal(replay_plan)
            .unwrap();
        assert!(replay.replayed());
        assert_eq!(replay.provider_io_operations(), 0);
        assert_eq!(maximum_capacity_spent(&reopened), 3);
        let after = ExecutionStoreFacadeV1::new(&mut reopened)
            .current_effect_snapshot(origination.intent())
            .unwrap();
        assert_eq!(after.dispatch(), &before_dispatch);
        assert_eq!(
            after.control_revision().attempt_history(),
            before_attempt_history.as_slice()
        );
        assert!(after.control_revision().live_attempt().is_none());
        assert_eq!(
            after.control_revision().live_dispatch(),
            super::super::withdrawal::EffectIntentLiveDispatchV1::None
        );
        assert_eq!(
            after.control_revision().classification(),
            RemoteClassificationV1::Cancelled
        );
        assert!(after.control_revision().runs_closed());
        assert_eq!(after.dispatch().run_set(), before_dispatch.run_set());

        let cancelled_plan = active_effect_withdrawal_plan(
            &mut reopened,
            origination.intent(),
            selection,
            executor,
            "stage4-withdrawal-after-cancelled",
        );
        assert!(
            ExecutionStoreFacadeV1::new(&mut reopened)
                .publish_effect_withdrawal(cancelled_plan)
                .is_err()
        );
        assert_eq!(maximum_capacity_spent(&reopened), 3);
    }

    #[test]
    fn effect_writer_handoff_fences_the_old_writer_atomically_and_restarts() {
        let (store_root, domain, mut store, draft, selection, executor) =
            effect_store_fixture(b"stage4-effect-writer-handoff");
        let origination_plan = active_effect_origination_plan(
            &mut store,
            &draft,
            selection,
            executor,
            "stage4-writer-origination",
        );
        let origination = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_origination(origination_plan)
            .unwrap();
        let before = ExecutionStoreFacadeV1::new(&mut store)
            .current_effect_snapshot(origination.intent())
            .unwrap();
        let old_writer = before.writer_term().id();
        let old_revision = before.control_revision().id();
        let old_dispatch = before.dispatch().clone();
        let stale_old_writer_plan = active_effect_seal_plan(
            &mut store,
            origination.intent(),
            selection,
            executor,
            "stage4-writer-stale-seal",
            [152; 32],
        );
        let handoff_plan = active_effect_writer_handoff_plan(
            &mut store,
            origination.intent(),
            selection,
            executor,
            "stage4-writer-handoff",
        );
        let replay_plan = handoff_plan.clone();
        let handoff = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_writer_handoff(handoff_plan)
            .unwrap();
        assert!(!handoff.replayed());
        assert_ne!(handoff.writer_term(), old_writer);
        assert_eq!(handoff.control_revision(), old_revision);
        assert_eq!(maximum_capacity_spent(&store), 2);
        assert!(matches!(
            ExecutionStoreFacadeV1::new(&mut store).publish_effect_seal(stale_old_writer_plan),
            Err(ExecutionStoreErrorV1::StaleExpectedStoreState)
        ));
        assert_eq!(maximum_capacity_spent(&store), 2);
        let replay = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_writer_handoff(replay_plan.clone())
            .unwrap();
        assert!(replay.replayed());
        assert_eq!(replay.writer_term(), handoff.writer_term());
        assert_eq!(maximum_capacity_spent(&store), 2);
        drop(store);

        let mut reopened = StoreV1::open(&store_root, domain).unwrap();
        let replay = ExecutionStoreFacadeV1::new(&mut reopened)
            .publish_effect_writer_handoff(replay_plan)
            .unwrap();
        assert!(replay.replayed());
        let after = ExecutionStoreFacadeV1::new(&mut reopened)
            .current_effect_snapshot(origination.intent())
            .unwrap();
        assert_eq!(after.control_revision().id(), old_revision);
        assert_eq!(after.dispatch(), &old_dispatch);
        assert_eq!(after.writer_term().id(), handoff.writer_term());
        assert_eq!(after.writer_term().prior_writer_term(), Some(old_writer));
        assert_eq!(
            after.writer_term().fencing_receipt(),
            Some(handoff.fencing_receipt())
        );
        assert_eq!(maximum_capacity_spent(&reopened), 2);
    }

    #[test]
    fn effect_recover_sealed_health_is_durable_and_writer_handoff_restores_atomically() {
        let (store_root, domain, mut store, draft, selection, executor) =
            effect_store_fixture(b"stage4-effect-recover-sealed-health");
        let origination = active_effect_origination_plan(
            &mut store,
            &draft,
            selection,
            executor,
            "stage4-health-origin",
        );
        let originated = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_origination(origination)
            .unwrap();
        let seal = active_effect_seal_plan(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-health-seal",
            [181; 32],
        );
        let mut sealed = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_seal(seal)
            .unwrap();
        assert!(sealed.take_provider_release().is_some());
        let health_plan = active_effect_health_plan(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-health-recover-sealed",
            ActiveStoreEffectHealthDraftV1::RecoverSealedInDoubt,
        );
        let replay_plan = health_plan.clone();
        let recovered = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_health(health_plan)
            .unwrap();
        assert_eq!(
            recovered.health(),
            EffectIntentControlHealthV1::RecoveryRequired
        );
        assert_eq!(recovered.provider_io_operations(), 0);
        assert!(!recovered.replayed());
        let replayed = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_health(replay_plan.clone())
            .unwrap();
        assert!(replayed.replayed());
        assert_eq!(replayed.provider_io_operations(), 0);
        assert_eq!(maximum_capacity_spent(&store), 3);
        drop(store);

        let mut reopened = StoreV1::open(&store_root, domain.clone()).unwrap();
        let snapshot = ExecutionStoreFacadeV1::new(&mut reopened)
            .current_effect_snapshot(originated.intent())
            .unwrap();
        assert_eq!(
            snapshot.control_revision().health(),
            EffectIntentControlHealthV1::RecoveryRequired
        );
        let prior_revision = snapshot.control_revision().id();
        let prior_writer = snapshot.writer_term().id();
        let replayed = ExecutionStoreFacadeV1::new(&mut reopened)
            .publish_effect_health(replay_plan)
            .unwrap();
        assert!(replayed.replayed());

        let handoff_plan = active_effect_writer_handoff_plan(
            &mut reopened,
            originated.intent(),
            selection,
            executor,
            "stage4-health-writer-restore",
        );
        let handoff_replay = handoff_plan.clone();
        let restored = ExecutionStoreFacadeV1::new(&mut reopened)
            .publish_effect_writer_handoff(handoff_plan)
            .unwrap();
        assert_ne!(restored.control_revision(), prior_revision);
        assert_ne!(restored.writer_term(), prior_writer);
        let replayed = ExecutionStoreFacadeV1::new(&mut reopened)
            .publish_effect_writer_handoff(handoff_replay)
            .unwrap();
        assert!(replayed.replayed());
        assert_eq!(maximum_capacity_spent(&reopened), 4);
        drop(reopened);

        let mut restored_store = StoreV1::open(&store_root, domain).unwrap();
        let restored_snapshot = ExecutionStoreFacadeV1::new(&mut restored_store)
            .current_effect_snapshot(originated.intent())
            .unwrap();
        assert_eq!(
            restored_snapshot.control_revision().health(),
            EffectIntentControlHealthV1::Healthy
        );
        assert_eq!(restored_snapshot.writer_term().id(), restored.writer_term());
        assert_eq!(
            restored_snapshot.control_revision().id(),
            restored.control_revision()
        );
    }

    #[test]
    fn recovered_sealed_dispatch_restarts_into_fresh_authorized_reconciliation() {
        let (store_root, domain, mut store, draft, selection, executor) =
            effect_store_fixture(b"stage4-recovered-sealed-reconciliation");
        let origination_plan = active_effect_origination_plan(
            &mut store,
            &draft,
            selection,
            executor,
            "stage4-recovered-reconcile-origin",
        );
        let originated = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_origination(origination_plan)
            .unwrap();
        let seal_plan = active_effect_seal_plan(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-recovered-reconcile-seal",
            [182; 32],
        );
        let mut sealed = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_seal(seal_plan)
            .unwrap();
        assert!(sealed.take_provider_release().is_some());
        let sealed_snapshot = ExecutionStoreFacadeV1::new(&mut store)
            .current_effect_snapshot(originated.intent())
            .unwrap();
        let sealed_run_revision = sealed_snapshot.control_revision().run_set_revision();
        let health_plan = active_effect_health_plan(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-recovered-reconcile-health",
            ActiveStoreEffectHealthDraftV1::RecoverSealedInDoubt,
        );
        let recovered = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_health(health_plan)
            .unwrap();
        assert_eq!(recovered.provider_io_operations(), 0);
        drop(store);

        let mut reopened = StoreV1::open(&store_root, domain).unwrap();
        let recovered_snapshot = ExecutionStoreFacadeV1::new(&mut reopened)
            .current_effect_snapshot(originated.intent())
            .unwrap();
        assert_eq!(recovered_snapshot.control_revision().live_attempt(), None);
        assert_eq!(
            recovered_snapshot.control_revision().live_dispatch(),
            super::super::withdrawal::EffectIntentLiveDispatchV1::None
        );
        assert_eq!(
            recovered_snapshot.control_revision().classification(),
            RemoteClassificationV1::InDoubt
        );
        assert!(recovered_snapshot.control_revision().runs_closed());
        assert_eq!(
            recovered_snapshot.control_revision().run_set_revision(),
            sealed_run_revision + 1
        );
        assert!(recovered_snapshot.reconciliation().is_none());
        assert_eq!(
            recovered_snapshot
                .dispatch()
                .state()
                .terminal_outcome()
                .unwrap(),
            super::super::dispatch_state::DispatchAttemptOutcomeV1::AmbiguousTransport
        );

        let begin_plan = active_effect_reconciliation_begin_plan(
            &mut reopened,
            originated.intent(),
            selection,
            executor,
            "stage4-recovered-reconcile-begin",
        );
        let begin_replay = begin_plan.clone();
        let mut begun = ExecutionStoreFacadeV1::new(&mut reopened)
            .publish_effect_reconciliation_begin(begin_plan)
            .unwrap();
        assert!(begun.take_read_release().is_some());
        assert_eq!(begun.classification(), RemoteClassificationV1::InDoubt);
        assert!(
            ExecutionStoreFacadeV1::new(&mut reopened)
                .publish_effect_reconciliation_begin(begin_replay)
                .unwrap()
                .replayed()
        );
        let reconciling = ExecutionStoreFacadeV1::new(&mut reopened)
            .current_effect_snapshot(originated.intent())
            .unwrap();
        assert_eq!(
            reconciling.control_revision().health(),
            EffectIntentControlHealthV1::Healthy
        );
        assert!(matches!(
            reconciling.control_revision().live_attempt(),
            Some(super::super::runtime::ExecutionAttemptOwnerV1::Reconciliation(_))
        ));
        assert!(reconciling.reconciliation().is_some());
        assert_eq!(maximum_capacity_spent(&reopened), 4);
    }

    #[test]
    fn effect_health_fail_closed_and_integrity_blocked_cannot_handoff() {
        let (_store_root, _domain, mut store, draft, selection, executor) =
            effect_store_fixture(b"stage4-effect-health-fail-closed");
        let origination = active_effect_origination_plan(
            &mut store,
            &draft,
            selection,
            executor,
            "stage4-health-fail-closed-origin",
        );
        let originated = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_origination(origination)
            .unwrap();
        let recovery = active_effect_health_plan(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-health-mark-recovery",
            ActiveStoreEffectHealthDraftV1::MarkRecoveryRequired,
        );
        let marked = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_health(recovery)
            .unwrap();
        assert_eq!(
            marked.health(),
            EffectIntentControlHealthV1::RecoveryRequired
        );
        let seal = active_effect_seal_plan(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-health-blocked-seal",
            [182; 32],
        );
        assert!(matches!(
            ExecutionStoreFacadeV1::new(&mut store).publish_effect_seal(seal),
            Err(ExecutionStoreErrorV1::Control(
                EffectIntentControlErrorV1::IllegalControlTransition
            ))
        ));
        assert_eq!(maximum_capacity_spent(&store), 2);
        let restore = active_effect_writer_handoff_plan(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-health-fail-closed-restore",
        );
        ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_writer_handoff(restore)
            .unwrap();
        let integrity = active_effect_health_plan(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-health-integrity-blocked",
            ActiveStoreEffectHealthDraftV1::MarkIntegrityBlocked,
        );
        let blocked = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_health(integrity)
            .unwrap();
        assert_eq!(
            blocked.health(),
            EffectIntentControlHealthV1::IntegrityBlocked
        );
        let handoff = active_effect_writer_handoff_plan(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-health-integrity-handoff",
        );
        assert!(matches!(
            ExecutionStoreFacadeV1::new(&mut store).publish_effect_writer_handoff(handoff),
            Err(ExecutionStoreErrorV1::Control(
                EffectIntentControlErrorV1::IllegalControlTransition
            ))
        ));
        assert_eq!(maximum_capacity_spent(&store), 4);
    }

    #[test]
    fn effect_health_and_writer_handoff_race_has_one_expected_old_winner() {
        let (store_root, domain, mut store, draft, selection, executor) =
            effect_store_fixture(b"stage4-effect-health-writer-race");
        let origination = active_effect_origination_plan(
            &mut store,
            &draft,
            selection,
            executor,
            "stage4-health-race-origin",
        );
        let originated = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_origination(origination)
            .unwrap();
        let health = active_effect_health_plan(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-health-race-block",
            ActiveStoreEffectHealthDraftV1::MarkIntegrityBlocked,
        );
        let handoff = active_effect_writer_handoff_plan(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-health-race-handoff",
        );
        drop(store);
        let barrier = Arc::new(Barrier::new(3));
        let health_worker = {
            let barrier = Arc::clone(&barrier);
            let store_root = store_root.clone();
            let domain = domain.clone();
            std::thread::spawn(move || {
                let mut store = StoreV1::open(store_root, domain).unwrap();
                barrier.wait();
                ExecutionStoreFacadeV1::new(&mut store)
                    .publish_effect_health(health)
                    .map(|outcome| outcome.health())
            })
        };
        let handoff_worker = {
            let barrier = Arc::clone(&barrier);
            let store_root = store_root.clone();
            let domain = domain.clone();
            std::thread::spawn(move || {
                let mut store = StoreV1::open(store_root, domain).unwrap();
                barrier.wait();
                ExecutionStoreFacadeV1::new(&mut store)
                    .publish_effect_writer_handoff(handoff)
                    .map(|_| EffectIntentControlHealthV1::Healthy)
            })
        };
        barrier.wait();
        let outcomes = [
            health_worker.join().unwrap(),
            handoff_worker.join().unwrap(),
        ];
        assert_eq!(outcomes.iter().filter(|outcome| outcome.is_ok()).count(), 1);
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| matches!(
                    outcome,
                    Err(ExecutionStoreErrorV1::StaleExpectedStoreState)
                ))
                .count(),
            1
        );
        let mut store = StoreV1::open(&store_root, domain).unwrap();
        let snapshot = ExecutionStoreFacadeV1::new(&mut store)
            .current_effect_snapshot(originated.intent())
            .unwrap();
        assert_eq!(
            snapshot.control_revision().health(),
            *outcomes
                .iter()
                .find_map(|outcome| outcome.as_ref().ok())
                .unwrap()
        );
        assert_eq!(maximum_capacity_spent(&store), 2);
    }

    #[test]
    fn writer_handoff_and_withdrawal_head_writer_races_have_one_winner_and_one_debit() {
        let (store_root, domain, mut store, draft, selection, executor) =
            effect_store_fixture(b"stage4-writer-handoff-race");
        let origination = active_effect_origination_plan(
            &mut store,
            &draft,
            selection,
            executor,
            "stage4-writer-race-origination",
        );
        let originated = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_origination(origination)
            .unwrap();
        let contenders = [
            active_effect_writer_handoff_plan(
                &mut store,
                originated.intent(),
                selection,
                executor,
                "stage4-writer-race-a",
            ),
            active_effect_writer_handoff_plan(
                &mut store,
                originated.intent(),
                selection,
                executor,
                "stage4-writer-race-b",
            ),
        ];
        drop(store);
        let barrier = Arc::new(Barrier::new(3));
        let workers = contenders.map(|plan| {
            let barrier = Arc::clone(&barrier);
            let store_root = store_root.clone();
            let domain = domain.clone();
            std::thread::spawn(move || {
                let mut store = StoreV1::open(store_root, domain).unwrap();
                barrier.wait();
                ExecutionStoreFacadeV1::new(&mut store).publish_effect_writer_handoff(plan)
            })
        });
        barrier.wait();
        let outcomes = workers.map(|worker| worker.join().unwrap());
        assert_eq!(outcomes.iter().filter(|outcome| outcome.is_ok()).count(), 1);
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| matches!(
                    outcome,
                    Err(ExecutionStoreErrorV1::StaleExpectedStoreState)
                ))
                .count(),
            1
        );
        let mut store = StoreV1::open(&store_root, domain).unwrap();
        assert_eq!(maximum_capacity_spent(&store), 2);
        let snapshot = ExecutionStoreFacadeV1::new(&mut store)
            .current_effect_snapshot(originated.intent())
            .unwrap();
        assert!(snapshot.writer_term().prior_writer_term().is_some());

        let (store_root, domain, mut store, draft, selection, executor) =
            effect_store_fixture(b"stage4-withdrawal-writer-race");
        let origination = active_effect_origination_plan(
            &mut store,
            &draft,
            selection,
            executor,
            "stage4-withdrawal-writer-race-origination",
        );
        let originated = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_origination(origination)
            .unwrap();
        let terminal = active_effect_terminal_plan(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-withdrawal-writer-race-terminal",
            EffectDispatchOutcomePayloadV1::LocallyRejected {
                evidence_commitment: [172; 32],
            },
        );
        ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_terminal(terminal)
            .unwrap();
        let withdrawal = active_effect_withdrawal_plan(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-withdrawal-writer-race-withdrawal",
        );
        let handoff = active_effect_writer_handoff_plan(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-withdrawal-writer-race-handoff",
        );
        drop(store);

        #[derive(Debug)]
        enum HeadWriterRaceOutcomeV1 {
            Withdrawal(u8),
            Handoff,
        }

        let barrier = Arc::new(Barrier::new(3));
        let withdrawal_worker = {
            let barrier = Arc::clone(&barrier);
            let store_root = store_root.clone();
            let domain = domain.clone();
            std::thread::spawn(move || {
                let mut store = StoreV1::open(store_root, domain).unwrap();
                barrier.wait();
                ExecutionStoreFacadeV1::new(&mut store)
                    .publish_effect_withdrawal(withdrawal)
                    .map(|outcome| {
                        HeadWriterRaceOutcomeV1::Withdrawal(outcome.provider_io_operations())
                    })
            })
        };
        let handoff_worker = {
            let barrier = Arc::clone(&barrier);
            let store_root = store_root.clone();
            let domain = domain.clone();
            std::thread::spawn(move || {
                let mut store = StoreV1::open(store_root, domain).unwrap();
                barrier.wait();
                ExecutionStoreFacadeV1::new(&mut store)
                    .publish_effect_writer_handoff(handoff)
                    .map(|_| HeadWriterRaceOutcomeV1::Handoff)
            })
        };
        barrier.wait();
        let outcomes = [
            withdrawal_worker.join().unwrap(),
            handoff_worker.join().unwrap(),
        ];
        assert_eq!(outcomes.iter().filter(|outcome| outcome.is_ok()).count(), 1);
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| matches!(
                    outcome,
                    Err(ExecutionStoreErrorV1::StaleExpectedStoreState)
                ))
                .count(),
            1
        );
        assert!(outcomes.iter().filter_map(|outcome| outcome.as_ref().ok()).all(
            |outcome| !matches!(outcome, HeadWriterRaceOutcomeV1::Withdrawal(io) if *io != 0)
        ));
        let mut store = StoreV1::open(&store_root, domain).unwrap();
        assert_eq!(maximum_capacity_spent(&store), 3);
        let snapshot = ExecutionStoreFacadeV1::new(&mut store)
            .current_effect_snapshot(originated.intent())
            .unwrap();
        assert!(matches!(
            snapshot.control_revision().classification(),
            RemoteClassificationV1::ConfirmedNotApplied | RemoteClassificationV1::Cancelled
        ));
    }

    #[test]
    fn effect_reconciliation_phase_contenders_have_one_winner_and_one_debit() {
        let (store_root, domain, mut store, draft, selection, executor) =
            effect_store_fixture(b"stage4-effect-reconciliation-races");
        let origination = active_effect_origination_plan(
            &mut store,
            &draft,
            selection,
            executor,
            "stage4-reconciliation-race-origination",
        );
        let originated = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_origination(origination)
            .unwrap();
        let seal = active_effect_seal_plan(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-reconciliation-race-seal",
            [151; 32],
        );
        let mut sealed = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_seal(seal)
            .unwrap();
        let release = sealed.take_provider_release().unwrap();
        let terminal_draft = provider_terminal_draft_from_outcome(
            &mut store,
            release,
            EffectDispatchOutcomePayloadV1::AmbiguousTransport {
                evidence_commitment: [152; 32],
            },
        )
        .unwrap();
        let terminal = active_effect_terminal_plan_with_draft(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-reconciliation-race-dispatch-terminal",
            terminal_draft,
        );
        ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_terminal(terminal)
            .unwrap();
        assert_eq!(maximum_capacity_spent(&store), 3);

        let begin_plans = [
            active_effect_reconciliation_begin_plan(
                &mut store,
                originated.intent(),
                selection,
                executor,
                "stage4-reconciliation-race-begin-a",
            ),
            active_effect_reconciliation_begin_plan(
                &mut store,
                originated.intent(),
                selection,
                executor,
                "stage4-reconciliation-race-begin-b",
            ),
        ];
        drop(store);
        let begin_results = race_effect_reconciliation_publications(
            &store_root,
            &domain,
            begin_plans,
            |store, plan| {
                ExecutionStoreFacadeV1::new(store).publish_effect_reconciliation_begin(plan)
            },
        );
        assert_reconciliation_race_result(&begin_results);
        let mut begin_winner = begin_results.into_iter().find_map(Result::ok).unwrap();
        let mut store = StoreV1::open(&store_root, domain.clone()).unwrap();
        let read_draft = reconciliation_read_draft(
            &mut store,
            begin_winner.take_read_release().unwrap(),
            [143; 32],
            ProviderApplicationFactV1::Applied,
        );
        assert_eq!(maximum_capacity_spent(&store), 4);
        assert_eq!(
            ExecutionStoreFacadeV1::new(&mut store)
                .current_effect_snapshot(originated.intent())
                .unwrap()
                .control_head()
                .id(),
            begin_winner.control_head()
        );

        let read_plans = [
            active_effect_reconciliation_read_plan(&mut store, originated.intent(), read_draft),
            active_effect_reconciliation_read_plan(&mut store, originated.intent(), read_draft),
        ];
        drop(store);
        let read_results = race_effect_reconciliation_publications(
            &store_root,
            &domain,
            read_plans,
            |store, plan| {
                ExecutionStoreFacadeV1::new(store).publish_effect_reconciliation_read(plan)
            },
        );
        assert_eq!(
            read_results.iter().filter(|result| result.is_ok()).count(),
            2
        );
        assert_eq!(
            read_results
                .iter()
                .filter_map(|result| result.as_ref().ok())
                .filter(|result| result.replayed())
                .count(),
            1
        );
        let read_winner = read_results
            .into_iter()
            .find_map(|result| result.ok().filter(|outcome| !outcome.replayed()))
            .unwrap();
        let mut store = StoreV1::open(&store_root, domain.clone()).unwrap();
        assert_eq!(maximum_capacity_spent(&store), 4);
        assert_eq!(
            ExecutionStoreFacadeV1::new(&mut store)
                .current_effect_snapshot(originated.intent())
                .unwrap()
                .control_head()
                .id(),
            read_winner.control_head()
        );

        let terminal_plans = [
            active_effect_reconciliation_terminal_plan(
                &mut store,
                originated.intent(),
                selection,
                executor,
                "stage4-reconciliation-race-terminal-a",
                RemoteClassificationV1::ConfirmedApplied,
            ),
            active_effect_reconciliation_terminal_plan(
                &mut store,
                originated.intent(),
                selection,
                executor,
                "stage4-reconciliation-race-terminal-b",
                RemoteClassificationV1::ConfirmedApplied,
            ),
        ];
        drop(store);
        let terminal_results = race_effect_reconciliation_publications(
            &store_root,
            &domain,
            terminal_plans,
            |store, plan| {
                ExecutionStoreFacadeV1::new(store).publish_effect_reconciliation_terminal(plan)
            },
        );
        assert_eq!(
            terminal_results
                .iter()
                .filter(|result| result.is_ok())
                .count(),
            2
        );
        assert_eq!(
            terminal_results
                .iter()
                .filter_map(|result| result.as_ref().ok())
                .filter(|result| result.replayed())
                .count(),
            1
        );
        let terminal_winner = terminal_results.into_iter().find_map(Result::ok).unwrap();
        let mut store = StoreV1::open(&store_root, domain).unwrap();
        assert_eq!(maximum_capacity_spent(&store), 4);
        let current = ExecutionStoreFacadeV1::new(&mut store)
            .current_effect_snapshot(originated.intent())
            .unwrap();
        assert_eq!(current.control_head().id(), terminal_winner.control_head());
        assert_eq!(
            current.control_revision().classification(),
            terminal_winner.classification()
        );
    }

    #[test]
    fn effect_origination_stale_contenders_spend_capacity_once() {
        let (store_root, domain, mut store, draft, selection, executor) =
            effect_store_fixture(b"stage4-effect-origination-race");
        let contenders = ["stage4-effect-race-a", "stage4-effect-race-b"].map(|key| {
            active_effect_origination_plan(&mut store, &draft, selection, executor, key)
        });
        drop(store);
        let barrier = Arc::new(Barrier::new(3));
        let workers = contenders
            .into_iter()
            .map(|plan| {
                let barrier = Arc::clone(&barrier);
                let store_root = store_root.clone();
                let domain = domain.clone();
                std::thread::spawn(move || {
                    let mut store = StoreV1::open(store_root, domain).unwrap();
                    barrier.wait();
                    ExecutionStoreFacadeV1::new(&mut store).publish_effect_origination(plan)
                })
            })
            .collect::<Vec<_>>();
        barrier.wait();
        let results = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(
                    result,
                    Err(ExecutionStoreErrorV1::StaleExpectedStoreState)
                ))
                .count(),
            1
        );
        let store = StoreV1::open(&store_root, domain).unwrap();
        assert_eq!(maximum_capacity_spent(&store), 1);
    }

    #[test]
    fn effect_publication_binds_authority_and_replay_meaning() {
        let (_store_root, _domain, mut store, draft, selection, executor) =
            effect_store_fixture(b"stage4-effect-authority-binding");
        let plan = active_effect_origination_plan(
            &mut store,
            &draft,
            selection,
            executor,
            "stage4-effect-authority-binding-key",
        );
        let wrong_action_authority = GenericExecutionAuthorityV1::new(
            selection,
            RepositoryActionLeafV1::RenewStepLeaseTerm,
            plan.request.subject_commitment(),
            plan.request.expected_state_commitment(),
            plan.request.payload_commitment(),
            executor,
        )
        .unwrap();
        assert!(matches!(
            ActiveStoreEffectOriginationPublicationV1::new(
                plan.request.clone(),
                wrong_action_authority,
                plan.state_binding.clone(),
                plan.draft.clone(),
            ),
            Err(ExecutionStoreErrorV1::PublicationBindingMismatch)
        ));
        let alternate_authority = GenericExecutionAuthorityV1::new(
            selection,
            RepositoryActionLeafV1::OriginateEffectIntent,
            plan.request.subject_commitment(),
            plan.request.expected_state_commitment(),
            plan.request.payload_commitment(),
            PrincipalIdV1::derive("stage4-alternate-effect-principal").unwrap(),
        )
        .unwrap();
        let alternate_plan = ActiveStoreEffectOriginationPublicationV1::new(
            plan.request.clone(),
            alternate_authority,
            plan.state_binding.clone(),
            plan.draft.clone(),
        )
        .unwrap();
        let originated = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_origination(plan)
            .unwrap();
        assert!(
            ExecutionStoreFacadeV1::new(&mut store)
                .publish_effect_origination(alternate_plan)
                .is_err()
        );
        assert_eq!(maximum_capacity_spent(&store), 1);

        let seal_plan = active_effect_seal_plan(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-effect-authority-seal",
            [201; 32],
        );
        let wrong_seal_action = GenericExecutionAuthorityV1::new(
            selection,
            RepositoryActionLeafV1::OriginateEffectIntent,
            seal_plan.request.subject_commitment(),
            seal_plan.request.expected_state_commitment(),
            seal_plan.request.payload_commitment(),
            executor,
        )
        .unwrap();
        assert!(matches!(
            ActiveStoreEffectSealPublicationV1::new(
                seal_plan.request.clone(),
                wrong_seal_action,
                seal_plan.snapshot.clone(),
                seal_plan.draft,
            ),
            Err(ExecutionStoreErrorV1::PublicationBindingMismatch)
        ));
        let wrong_seal_commitment = GenericExecutionAuthorityV1::new(
            selection,
            RepositoryActionLeafV1::RecordDispatchOutcome,
            seal_plan.request.subject_commitment(),
            seal_plan.request.expected_state_commitment(),
            [202; 32],
            executor,
        )
        .unwrap();
        assert!(matches!(
            ActiveStoreEffectSealPublicationV1::new(
                seal_plan.request.clone(),
                wrong_seal_commitment,
                seal_plan.snapshot.clone(),
                seal_plan.draft,
            ),
            Err(ExecutionStoreErrorV1::PublicationBindingMismatch)
        ));
        let alternate_seal_authority = GenericExecutionAuthorityV1::new(
            selection,
            RepositoryActionLeafV1::RecordDispatchOutcome,
            seal_plan.request.subject_commitment(),
            seal_plan.request.expected_state_commitment(),
            seal_plan.request.payload_commitment(),
            PrincipalIdV1::derive("stage4-alternate-seal-principal").unwrap(),
        )
        .unwrap();
        let alternate_seal_plan = ActiveStoreEffectSealPublicationV1::new(
            seal_plan.request.clone(),
            alternate_seal_authority,
            seal_plan.snapshot.clone(),
            seal_plan.draft,
        )
        .unwrap();
        let mut sealed = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_seal(seal_plan)
            .unwrap();
        let release = sealed.take_provider_release().unwrap();
        assert!(
            ExecutionStoreFacadeV1::new(&mut store)
                .publish_effect_seal(alternate_seal_plan)
                .is_err()
        );
        assert_eq!(maximum_capacity_spent(&store), 2);

        let terminal_draft = provider_terminal_draft_from_outcome(
            &mut store,
            release,
            EffectDispatchOutcomePayloadV1::AmbiguousTransport {
                evidence_commitment: [203; 32],
            },
        )
        .unwrap();
        let terminal_plan = active_effect_terminal_plan_with_draft(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-effect-authority-terminal",
            terminal_draft,
        );
        let wrong_terminal_action = GenericExecutionAuthorityV1::new(
            selection,
            RepositoryActionLeafV1::OriginateEffectIntent,
            terminal_plan.request.subject_commitment(),
            terminal_plan.request.expected_state_commitment(),
            terminal_plan.request.payload_commitment(),
            executor,
        )
        .unwrap();
        assert!(matches!(
            ActiveStoreEffectTerminalPublicationV1::new(
                terminal_plan.request.clone(),
                wrong_terminal_action,
                terminal_plan.snapshot.clone(),
                terminal_plan.draft.clone(),
            ),
            Err(ExecutionStoreErrorV1::PublicationBindingMismatch)
        ));
        let wrong_terminal_commitment = GenericExecutionAuthorityV1::new(
            selection,
            RepositoryActionLeafV1::RecordDispatchOutcome,
            terminal_plan.request.subject_commitment(),
            [204; 32],
            terminal_plan.request.payload_commitment(),
            executor,
        )
        .unwrap();
        assert!(matches!(
            ActiveStoreEffectTerminalPublicationV1::new(
                terminal_plan.request.clone(),
                wrong_terminal_commitment,
                terminal_plan.snapshot.clone(),
                terminal_plan.draft.clone(),
            ),
            Err(ExecutionStoreErrorV1::PublicationBindingMismatch)
        ));
        let alternate_terminal_authority = GenericExecutionAuthorityV1::new(
            selection,
            RepositoryActionLeafV1::RecordDispatchOutcome,
            terminal_plan.request.subject_commitment(),
            terminal_plan.request.expected_state_commitment(),
            terminal_plan.request.payload_commitment(),
            PrincipalIdV1::derive("stage4-alternate-terminal-principal").unwrap(),
        )
        .unwrap();
        let alternate_terminal_plan = ActiveStoreEffectTerminalPublicationV1::new(
            terminal_plan.request.clone(),
            alternate_terminal_authority,
            terminal_plan.snapshot.clone(),
            terminal_plan.draft.clone(),
        )
        .unwrap();
        ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_terminal(terminal_plan)
            .unwrap();
        assert!(
            ExecutionStoreFacadeV1::new(&mut store)
                .publish_effect_terminal(alternate_terminal_plan)
                .is_err()
        );
        assert_eq!(maximum_capacity_spent(&store), 3);
    }

    #[test]
    fn effect_restart_replay_never_reconstructs_provider_release() {
        let (store_root, domain, mut store, draft, selection, executor) =
            effect_store_fixture(b"stage4-effect-restart-replay");
        let origination_plan = active_effect_origination_plan(
            &mut store,
            &draft,
            selection,
            executor,
            "stage4-effect-restart-origination",
        );
        let origination_replay = origination_plan.clone();
        let originated = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_origination(origination_plan)
            .unwrap();
        drop(store);

        let mut store = StoreV1::open(&store_root, domain.clone()).unwrap();
        let replayed = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_origination(origination_replay)
            .unwrap();
        assert!(replayed.replayed());
        assert_eq!(maximum_capacity_spent(&store), 1);
        let seal_plan = active_effect_seal_plan(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-effect-restart-seal",
            [103; 32],
        );
        let seal_replay = seal_plan.clone();
        let mut sealed = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_seal(seal_plan)
            .unwrap();
        let release = sealed.take_provider_release().unwrap();
        let operation = release.operation.binding;
        let execution_time = release
            .execution_time_receipt(operation.deadline, [104; 32])
            .unwrap();
        let mut executor = TestPinnedProviderExecutorV1 {
            operation,
            observation: ProviderTransportObservationV1::DefinitelyNotSent {
                authenticated_evidence_commitment: [105; 32],
            },
            calls: 0,
        };
        assert!(matches!(
            release.execute_once(execution_time, &mut executor),
            Err(ExecutionStoreErrorV1::RunDeadlineExpired)
        ));
        assert_eq!(executor.calls, 0);
        drop(store);

        let mut reopened = StoreV1::open(&store_root, domain.clone()).unwrap();
        let snapshot = ExecutionStoreFacadeV1::new(&mut reopened)
            .current_effect_snapshot(originated.intent())
            .unwrap();
        assert_eq!(
            snapshot.control_revision().classification(),
            super::super::withdrawal::RemoteClassificationV1::InDoubt
        );
        let mut replayed_seal = ExecutionStoreFacadeV1::new(&mut reopened)
            .publish_effect_seal(seal_replay)
            .unwrap();
        assert!(replayed_seal.replayed());
        assert!(replayed_seal.take_provider_release().is_none());
        assert_eq!(maximum_capacity_spent(&reopened), 2);

        assert!(
            ActiveStoreEffectTerminalDraftV1::new(
                EffectDispatchOutcomePayloadV1::AmbiguousTransport {
                    evidence_commitment: [205; 32],
                },
            )
            .is_err()
        );
        let still_sealed = ExecutionStoreFacadeV1::new(&mut reopened)
            .current_effect_snapshot(originated.intent())
            .unwrap();
        assert_eq!(
            still_sealed.control_revision().live_dispatch(),
            super::super::withdrawal::EffectIntentLiveDispatchV1::Sealed
        );
        assert_eq!(maximum_capacity_spent(&reopened), 2);
        drop(reopened);

        let reopened = StoreV1::open(&store_root, domain).unwrap();
        assert_eq!(maximum_capacity_spent(&reopened), 2);
    }

    #[test]
    fn effect_seal_and_terminal_contenders_commit_once() {
        let (store_root, domain, mut store, draft, selection, executor) =
            effect_store_fixture(b"stage4-effect-seal-terminal-races");
        let origination_plan = active_effect_origination_plan(
            &mut store,
            &draft,
            selection,
            executor,
            "stage4-race-origination",
        );
        let originated = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_origination(origination_plan)
            .unwrap();
        let seal_contenders = [
            ("stage4-race-seal-a", [104; 32]),
            ("stage4-race-seal-b", [105; 32]),
        ]
        .map(|(key, seal)| {
            active_effect_seal_plan(
                &mut store,
                originated.intent(),
                selection,
                executor,
                key,
                seal,
            )
        });
        drop(store);
        let barrier = Arc::new(Barrier::new(3));
        let workers = seal_contenders
            .into_iter()
            .map(|plan| {
                let barrier = Arc::clone(&barrier);
                let store_root = store_root.clone();
                let domain = domain.clone();
                std::thread::spawn(move || {
                    let mut store = StoreV1::open(store_root, domain).unwrap();
                    barrier.wait();
                    ExecutionStoreFacadeV1::new(&mut store).publish_effect_seal(plan)
                })
            })
            .collect::<Vec<_>>();
        barrier.wait();
        let mut seal_results = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            seal_results.iter().filter(|result| result.is_ok()).count(),
            1
        );
        let winning_seal_index = seal_results
            .iter()
            .position(Result::is_ok)
            .expect("exactly one seal contender wins");
        let winning_seal_head = seal_results[winning_seal_index]
            .as_ref()
            .unwrap()
            .control_head();
        let winning_seal_commitment = [[104; 32], [105; 32]][winning_seal_index];
        assert_eq!(
            seal_results
                .iter()
                .filter(|result| matches!(
                    result,
                    Err(ExecutionStoreErrorV1::StaleExpectedStoreState)
                ))
                .count(),
            1
        );
        let releases = seal_results
            .iter_mut()
            .filter_map(|result| result.as_mut().ok())
            .filter_map(ActiveStoreEffectSealOutcomeV1::take_provider_release)
            .collect::<Vec<_>>();
        let [release] = releases.try_into().expect("exactly one provider release");
        let mut store = StoreV1::open(&store_root, domain.clone()).unwrap();
        assert_eq!(maximum_capacity_spent(&store), 2);
        let sealed_snapshot = ExecutionStoreFacadeV1::new(&mut store)
            .current_effect_snapshot(originated.intent())
            .unwrap();
        assert_eq!(sealed_snapshot.control_head().id(), winning_seal_head);
        assert_eq!(
            sealed_snapshot.dispatch().crossing_seal_commitment(),
            Some(winning_seal_commitment)
        );
        let terminal_draft = provider_terminal_draft_from_outcome(
            &mut store,
            release,
            EffectDispatchOutcomePayloadV1::AmbiguousTransport {
                evidence_commitment: [106; 32],
            },
        )
        .unwrap();
        let terminal_contenders = ["stage4-race-terminal-a", "stage4-race-terminal-b"].map(|key| {
            active_effect_terminal_plan_with_draft(
                &mut store,
                originated.intent(),
                selection,
                executor,
                key,
                terminal_draft.clone(),
            )
        });
        drop(store);
        let barrier = Arc::new(Barrier::new(3));
        let workers = terminal_contenders
            .into_iter()
            .map(|plan| {
                let barrier = Arc::clone(&barrier);
                let store_root = store_root.clone();
                let domain = domain.clone();
                std::thread::spawn(move || {
                    let mut store = StoreV1::open(store_root, domain).unwrap();
                    barrier.wait();
                    ExecutionStoreFacadeV1::new(&mut store).publish_effect_terminal(plan)
                })
            })
            .collect::<Vec<_>>();
        barrier.wait();
        let terminal_results = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            terminal_results
                .iter()
                .filter(|result| result.is_ok())
                .count(),
            1
        );
        assert_eq!(
            terminal_results
                .iter()
                .filter(|result| matches!(
                    result,
                    Err(ExecutionStoreErrorV1::StaleExpectedStoreState)
                ))
                .count(),
            1
        );
        let winning_terminal = terminal_results
            .iter()
            .find_map(|result| result.as_ref().ok())
            .expect("exactly one terminal contender wins");
        let winning_terminal_head = winning_terminal.control_head();
        let winning_terminal_classification = winning_terminal.classification();
        let mut store = StoreV1::open(&store_root, domain).unwrap();
        assert_eq!(maximum_capacity_spent(&store), 3);
        let snapshot = ExecutionStoreFacadeV1::new(&mut store)
            .current_effect_snapshot(originated.intent())
            .unwrap();
        assert_eq!(snapshot.control_head().id(), winning_terminal_head);
        assert_eq!(
            snapshot.control_revision().classification(),
            winning_terminal_classification
        );
        assert_eq!(
            snapshot.dispatch().terminal_classification(),
            Some(winning_terminal_classification)
        );
        assert!(snapshot.control_revision().runs_closed());
        assert_eq!(
            snapshot.control_revision().live_dispatch(),
            super::super::withdrawal::EffectIntentLiveDispatchV1::None
        );
        assert_eq!(
            snapshot.dispatch().run_set().runs()[0].state(),
            RunStateV1::Succeeded
        );
    }

    #[test]
    fn effect_dispatch_control_cross_invariants_reject_individually_valid_mismatches() {
        let (_store_root, _domain, mut store, draft, selection, executor) =
            effect_store_fixture(b"stage4-effect-cross-invariants");
        let origination_plan = active_effect_origination_plan(
            &mut store,
            &draft,
            selection,
            executor,
            "stage4-cross-origination",
        );
        let originated = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_origination(origination_plan)
            .unwrap();
        let reserved = ExecutionStoreFacadeV1::new(&mut store)
            .current_effect_snapshot(originated.intent())
            .unwrap();
        let reserved_need = EffectControlTransitionNeedV1::ReserveDispatch {
            action_request_id: reserved.dispatch().reserve_action_request_id(),
            attempt: super::super::runtime::ExecutionAttemptOwnerV1::Dispatch(
                reserved.dispatch().attempt().id(),
            ),
            next_dispatch_fence: reserved.dispatch().attempt().dispatch_fence(),
            next_run_set_revision: reserved.dispatch().run_set().revision(),
            next_use_fence_commitment: reserved.dispatch().use_fence_commitment(),
        };
        assert!(effect_dispatch_control_is_coherent(
            originated.intent(),
            reserved.control_revision(),
            reserved.dispatch(),
            &reserved_need,
        ));
        let seal_plan = active_effect_seal_plan(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-cross-seal",
            [108; 32],
        );
        let mut sealed_outcome = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_seal(seal_plan)
            .unwrap();
        let release = sealed_outcome.take_provider_release().unwrap();
        let sealed = ExecutionStoreFacadeV1::new(&mut store)
            .current_effect_snapshot(originated.intent())
            .unwrap();
        let sealed_need = EffectControlTransitionNeedV1::SealDispatch {
            action_request_id: sealed.dispatch().seal_action_request_id().unwrap(),
            attempt: super::super::runtime::ExecutionAttemptOwnerV1::Dispatch(
                sealed.dispatch().attempt().id(),
            ),
            next_run_set_revision: sealed.dispatch().run_set().revision(),
        };
        assert!(effect_dispatch_control_is_coherent(
            originated.intent(),
            sealed.control_revision(),
            sealed.dispatch(),
            &sealed_need,
        ));
        assert!(!effect_dispatch_control_is_coherent(
            originated.intent(),
            reserved.control_revision(),
            sealed.dispatch(),
            &sealed_need,
        ));
        let terminal_draft = provider_terminal_draft_from_outcome(
            &mut store,
            release,
            EffectDispatchOutcomePayloadV1::AmbiguousTransport {
                evidence_commitment: [109; 32],
            },
        )
        .unwrap();
        let terminal_plan = active_effect_terminal_plan_with_draft(
            &mut store,
            originated.intent(),
            selection,
            executor,
            "stage4-cross-terminal",
            terminal_draft,
        );
        ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_terminal(terminal_plan)
            .unwrap();
        let terminal = ExecutionStoreFacadeV1::new(&mut store)
            .current_effect_snapshot(originated.intent())
            .unwrap();
        let terminal_parts = terminal.control_revision().parts();
        let terminal_need = EffectControlTransitionNeedV1::FinishDispatch {
            action_request_id: crate::domain::authority::ActionRequestIdV1::from_digest([110; 32]),
            attempt: super::super::runtime::ExecutionAttemptOwnerV1::Dispatch(
                terminal.dispatch().attempt().id(),
            ),
            classification: terminal.control_revision().classification(),
            next_run_set_revision: terminal.dispatch().run_set().revision(),
            result_commitment: terminal_parts.result_commitment.unwrap(),
            idempotency_commitment: terminal_parts.idempotency_commitment.unwrap(),
        };
        assert!(effect_dispatch_control_is_coherent(
            originated.intent(),
            terminal.control_revision(),
            terminal.dispatch(),
            &terminal_need,
        ));
        let mut wrong_classification_parts = terminal_parts.clone();
        wrong_classification_parts.classification =
            super::super::withdrawal::RemoteClassificationV1::ConfirmedApplied;
        let wrong_classification =
            EffectIntentControlRevisionV1::new(wrong_classification_parts).unwrap();
        assert!(!effect_dispatch_control_is_coherent(
            originated.intent(),
            &wrong_classification,
            terminal.dispatch(),
            &terminal_need,
        ));
        let mut wrong_run_revision_parts = terminal_parts;
        wrong_run_revision_parts.run_set_revision += 1;
        let wrong_run_revision =
            EffectIntentControlRevisionV1::new(wrong_run_revision_parts).unwrap();
        assert!(!effect_dispatch_control_is_coherent(
            originated.intent(),
            &wrong_run_revision,
            terminal.dispatch(),
            &terminal_need,
        ));
        let (_, _, generation, objects) = store.coherent_publication_snapshot().unwrap();
        let index = load_control_index(&objects).unwrap();
        let entry = index
            .entries
            .iter()
            .find(|entry| entry.intent == originated.intent())
            .unwrap();
        let head_object = objects
            .iter()
            .find(|object| object.id() == entry.control_head_object_id)
            .unwrap();
        let terminal_object = exact_referenced_schema_object(
            head_object,
            &objects,
            &[
                execution_schema_id("maestro.vnext.effect-dispatch-terminal-carrier-schema.v1")
                    .unwrap(),
            ],
        )
        .unwrap();
        let mut rewritten_value = terminal_object.value().clone();
        let CborValue::Array(wrapper) = &mut rewritten_value else {
            unreachable!()
        };
        let CborValue::Array(carrier) = &mut wrapper[1] else {
            unreachable!()
        };
        let CborValue::Array(need) = &mut carrier[2] else {
            unreachable!()
        };
        let CborValue::Array(body) = &mut need[1] else {
            unreachable!()
        };
        body[1] = bytes(&[111; 32]);
        let rewritten = StoreObjectV1::new(
            terminal_object.schema_id(),
            rewritten_value,
            terminal_object.references().to_vec(),
        )
        .unwrap();
        assert!(matches!(
            load_validated_dispatch_carrier(
                &rewritten,
                &objects,
                terminal.intent(),
                &generation,
                &mut |generation_id| Ok(store.generation(generation_id)?),
            ),
            Err(ExecutionStoreErrorV1::InvalidEffectSnapshot)
        ));

        let occurrence_object = exact_referenced_schema_object(
            terminal_object,
            &objects,
            &[
                execution_schema_id("maestro.vnext.effect-dispatch-terminal-occurrence-schema.v1")
                    .unwrap(),
            ],
        )
        .unwrap();
        let mut detached_result_value = terminal_object.value().clone();
        let CborValue::Array(wrapper) = &mut detached_result_value else {
            unreachable!()
        };
        let CborValue::Array(carrier) = &mut wrapper[1] else {
            unreachable!()
        };
        let CborValue::Array(need) = &mut carrier[2] else {
            unreachable!()
        };
        let CborValue::Array(body) = &mut need[1] else {
            unreachable!()
        };
        body[5] = bytes(&[112; 32]);
        let detached_result = StoreObjectV1::new(
            terminal_object.schema_id(),
            detached_result_value,
            terminal_object.references().to_vec(),
        )
        .unwrap();
        assert!(matches!(
            load_validated_dispatch_carrier(
                &detached_result,
                &objects,
                terminal.intent(),
                &generation,
                &mut |generation_id| Ok(store.generation(generation_id)?),
            ),
            Err(ExecutionStoreErrorV1::InvalidEffectSnapshot)
        ));

        let mut rewritten_occurrence_value = occurrence_object.value().clone();
        let CborValue::Array(occurrence_fields) = &mut rewritten_occurrence_value else {
            unreachable!()
        };
        let CborValue::Array(payload_fields) = &mut occurrence_fields[2] else {
            unreachable!()
        };
        let CborValue::Array(outcome_fields) = &mut payload_fields[1] else {
            unreachable!()
        };
        outcome_fields[1] = bytes(&[113; 32]);
        let rewritten_occurrence = StoreObjectV1::new(
            occurrence_object.schema_id(),
            rewritten_occurrence_value,
            occurrence_object.references().to_vec(),
        )
        .unwrap();
        let mut coordinated_carrier_value = terminal_object.value().clone();
        let CborValue::Array(wrapper) = &mut coordinated_carrier_value else {
            unreachable!()
        };
        let CborValue::Array(carrier) = &mut wrapper[1] else {
            unreachable!()
        };
        let CborValue::Array(need) = &mut carrier[2] else {
            unreachable!()
        };
        let CborValue::Array(body) = &mut need[1] else {
            unreachable!()
        };
        body[5] = bytes(rewritten_occurrence.id().as_bytes());
        let mut coordinated_references = terminal_object.references().to_vec();
        let occurrence_reference = coordinated_references
            .iter_mut()
            .find(|reference| **reference == occurrence_object.id())
            .unwrap();
        *occurrence_reference = rewritten_occurrence.id();
        coordinated_references.sort_unstable();
        let coordinated_carrier = StoreObjectV1::new(
            terminal_object.schema_id(),
            coordinated_carrier_value,
            coordinated_references,
        )
        .unwrap();
        let mut coordinated_objects = objects.clone();
        coordinated_objects.push(rewritten_occurrence);
        assert!(matches!(
            load_validated_dispatch_carrier(
                &coordinated_carrier,
                &coordinated_objects,
                terminal.intent(),
                &generation,
                &mut |generation_id| Ok(store.generation(generation_id)?),
            ),
            Err(ExecutionStoreErrorV1::InvalidEffectSnapshot)
        ));
    }

    #[test]
    fn historical_open_step_with_same_contract_root_cannot_acquire_execution() {
        let domain = StoreDomainV1::derive(StoreRoleV1::Repository, b"stage4-historical-step")
            .expect("test fixture");
        let root = ContractRootIdV1::parse(&render_digest([71; 32])).unwrap();
        let scope = StepScopeV1::new(domain.id(), WorkIdV1::derive("historical-work").unwrap());
        let current = StepBindingV1::new(
            scope,
            ContractGenerationIdV1::parse(&render_digest([72; 32])).unwrap(),
            root,
            StepIdV1::from_bytes(scope, [73; 32]).unwrap(),
            StepRevisionIdV1::from_bytes([74; 32]).unwrap(),
        )
        .unwrap();
        let historical = StepBindingV1::new(
            scope,
            ContractGenerationIdV1::parse(&render_digest([75; 32])).unwrap(),
            root,
            StepIdV1::from_bytes(scope, [76; 32]).unwrap(),
            StepRevisionIdV1::from_bytes([77; 32]).unwrap(),
        )
        .unwrap();
        let current_state = open_step_state_object(current);
        let historical_state = open_step_state_object(historical);
        let graph = current_step_graph_object(current);
        let store_root = test_root();
        let mut store = StoreV1::create(&store_root, domain.clone()).unwrap();
        put_objects_in_reference_order(
            &mut store,
            vec![
                current_state.clone(),
                historical_state.clone(),
                graph.clone(),
            ],
        );
        let mut roots = vec![current_state.id(), historical_state.id(), graph.id()];
        roots.sort_unstable();
        let generation = StoreGenerationV1::new(
            domain,
            1,
            None,
            root,
            StoreCompatibilityV1::stage0_successor().unwrap(),
            roots,
        )
        .unwrap();
        store.publish_generation(&generation, None).unwrap();
        activate_store(&store_root);
        let snapshot = ExecutionStoreFacadeV1::new(&mut store)
            .current_step_execution(historical)
            .unwrap();
        let mutation = AuthorizedStepExecutionMutationV1::Acquire {
            executor: PrincipalIdV1::derive("historical-executor").unwrap(),
            fixed_envelope_commitment: [78; 32],
            run_limit: 1,
            issued_at: 120,
            expires_at: 130,
            hard_deadline: 140,
            takeover_safety: None,
        };
        assert!(matches!(
            ExecutionStoreFacadeV1::new(&mut store).canonical_step_request(
                &snapshot,
                &mutation,
                IdempotencyKeyIdV1::derive("historical-acquire").unwrap(),
            ),
            Err(ExecutionStoreErrorV1::StepBindingNotCurrent)
        ));
    }

    fn open_step_state_object(binding: StepBindingV1) -> StoreObjectV1 {
        StoreObjectV1::new(
            RepositoryStoreSchemaV1::StepState.schema_id().unwrap(),
            CborValue::Array(vec![
                CborValue::text("maestro.vnext.repository-step-state.v1").unwrap(),
                step_binding_store_value(binding),
                CborValue::Array(vec![
                    CborValue::Unsigned(1),
                    CborValue::Array(vec![CborValue::Unsigned(1)]),
                ]),
            ]),
            vec![],
        )
        .unwrap()
    }

    fn step_store_fixture(
        domain_seed: &[u8],
        action_literals: &[&'static str],
    ) -> (
        std::path::PathBuf,
        StoreDomainV1,
        StoreV1,
        StepBindingV1,
        RepositoryAuthorityFixtureV1,
        PrincipalIdV1,
    ) {
        step_store_fixture_with_observations(domain_seed, action_literals, &[])
    }

    fn step_store_fixture_with_observations(
        domain_seed: &[u8],
        action_literals: &[&'static str],
        observations: &[(u8, u64, StepSubmissionIdV1)],
    ) -> (
        std::path::PathBuf,
        StoreDomainV1,
        StoreV1,
        StepBindingV1,
        RepositoryAuthorityFixtureV1,
        PrincipalIdV1,
    ) {
        let domain = StoreDomainV1::derive(StoreRoleV1::Repository, domain_seed).unwrap();
        let root = ContractRootIdV1::parse(&render_digest([141; 32])).unwrap();
        let scope = StepScopeV1::new(domain.id(), WorkIdV1::derive("stage4-submit-work").unwrap());
        let binding = StepBindingV1::new(
            scope,
            ContractGenerationIdV1::parse(&render_digest([142; 32])).unwrap(),
            root,
            StepIdV1::from_bytes(scope, [143; 32]).unwrap(),
            StepRevisionIdV1::from_bytes([144; 32]).unwrap(),
        )
        .unwrap();
        let subject_commitment = hash(&step_binding_store_value(binding)).unwrap();
        let mut authority_scopes = action_literals
            .iter()
            .map(|literal| (*literal, subject_commitment))
            .collect::<Vec<_>>();
        authority_scopes.extend(
            observations
                .iter()
                .map(|(seed, recorded_at, submission_id)| {
                    let observation = step_observation_at(
                        binding,
                        *submission_id,
                        *seed,
                        [seed.wrapping_add(8); 32],
                        *recorded_at,
                    );
                    (
                        RepositoryActionLeafV1::PublishObservation.literal(),
                        step_observation_subject_commitment(&observation),
                    )
                }),
        );
        let fixture = repository_authority_fixture(authority_scopes, AuthorityFixtureModeV1::Valid);
        let step_state = open_step_state_object(binding);
        let step_graph = current_step_graph_object(binding);
        let mut objects = fixture.objects.clone();
        objects.extend([step_state.clone(), step_graph.clone()]);
        let store_root = test_root();
        let mut store = StoreV1::create(&store_root, domain.clone()).unwrap();
        put_objects_in_reference_order(&mut store, objects);
        let mut roots = vec![fixture.authority_root_id, step_state.id(), step_graph.id()];
        roots.sort_unstable();
        let generation = StoreGenerationV1::new(
            domain.clone(),
            1,
            None,
            root,
            StoreCompatibilityV1::stage0_successor().unwrap(),
            roots,
        )
        .unwrap();
        store.publish_generation(&generation, None).unwrap();
        activate_store(&store_root);
        let executor = fixture.actor_principal;
        (store_root, domain, store, binding, fixture, executor)
    }

    fn step_store_fixture_with_seeded_carrier(
        domain_seed: &[u8],
        action_literals: &[&'static str],
        accepted_h_time: u64,
        carrier_expires_at: u64,
    ) -> (
        std::path::PathBuf,
        StoreDomainV1,
        StoreV1,
        StepBindingV1,
        RepositoryAuthorityFixtureV1,
        PrincipalIdV1,
    ) {
        step_store_fixture_with_seeded_carrier_and_observations(
            domain_seed,
            action_literals,
            accepted_h_time,
            carrier_expires_at,
            &[],
        )
    }

    fn step_store_fixture_with_seeded_carrier_and_observations(
        domain_seed: &[u8],
        action_literals: &[&'static str],
        accepted_h_time: u64,
        carrier_expires_at: u64,
        observations: &[(u8, u64, StepSubmissionIdV1)],
    ) -> (
        std::path::PathBuf,
        StoreDomainV1,
        StoreV1,
        StepBindingV1,
        RepositoryAuthorityFixtureV1,
        PrincipalIdV1,
    ) {
        let domain = StoreDomainV1::derive(StoreRoleV1::Repository, domain_seed).unwrap();
        let root = ContractRootIdV1::parse(&render_digest([141; 32])).unwrap();
        let scope = StepScopeV1::new(
            domain.id(),
            WorkIdV1::derive("stage4-seeded-step-work").unwrap(),
        );
        let binding = StepBindingV1::new(
            scope,
            ContractGenerationIdV1::parse(&render_digest([142; 32])).unwrap(),
            root,
            StepIdV1::from_bytes(scope, [143; 32]).unwrap(),
            StepRevisionIdV1::from_bytes([144; 32]).unwrap(),
        )
        .unwrap();
        let subject_commitment = hash(&step_binding_store_value(binding)).unwrap();
        let mut authority_scopes = action_literals
            .iter()
            .map(|literal| (*literal, subject_commitment))
            .collect::<Vec<_>>();
        authority_scopes.extend(
            observations
                .iter()
                .map(|(seed, recorded_at, submission_id)| {
                    let observation = step_observation_at(
                        binding,
                        *submission_id,
                        *seed,
                        [seed.wrapping_add(8); 32],
                        *recorded_at,
                    );
                    (
                        RepositoryActionLeafV1::PublishObservation.literal(),
                        step_observation_subject_commitment(&observation),
                    )
                }),
        );
        let fixture = repository_authority_fixture_at(
            authority_scopes,
            AuthorityFixtureModeV1::Valid,
            accepted_h_time,
            accepted_h_time + 10,
        );
        let executor = fixture.actor_principal;
        let carrier = StepExecutionCarrierV1::acquire(StepExecutionAcquisitionV1 {
            binding,
            next_fence: 1,
            executor,
            store_generation_id: StoreGenerationIdV1::from_digest([230; 32]),
            authority_epoch: fixture.authority_epoch,
            fixed_envelope_commitment: [145; 32],
            run_limit: 4,
            issued_at: 120,
            expires_at: carrier_expires_at,
            hard_deadline: 190,
            authority: test_authorized_execution_action(
                ExecutionActionV1::AcquireStepExecution,
                "stage4-seeded-carrier-acquire",
            ),
        })
        .unwrap();
        let carrier_object = step_execution_carrier_object(&carrier).unwrap();
        let index = build_step_execution_index_object(&[StepExecutionIndexEntryV1 {
            binding_commitment: hash(&step_binding_store_value(binding)).unwrap(),
            carrier_object_id: carrier_object.id(),
            fence_high_water: 1,
        }])
        .unwrap();
        let step_state = open_step_state_object(binding);
        let step_graph = current_step_graph_object(binding);
        let mut objects = fixture.objects.clone();
        objects.extend([
            step_state.clone(),
            step_graph.clone(),
            carrier_object,
            index.clone(),
        ]);
        let store_root = test_root();
        let mut store = StoreV1::create(&store_root, domain.clone()).unwrap();
        put_objects_in_reference_order(&mut store, objects);
        let mut roots = vec![
            fixture.authority_root_id,
            step_state.id(),
            step_graph.id(),
            index.id(),
        ];
        roots.sort_unstable();
        let generation = StoreGenerationV1::new(
            domain.clone(),
            1,
            None,
            root,
            StoreCompatibilityV1::stage0_successor().unwrap(),
            roots,
        )
        .unwrap();
        store.publish_generation(&generation, None).unwrap();
        activate_store(&store_root);
        (store_root, domain, store, binding, fixture, executor)
    }

    fn step_origin_effect_store_fixture(
        domain_seed: &[u8],
    ) -> (
        std::path::PathBuf,
        StoreDomainV1,
        StoreV1,
        StepBindingV1,
        RepositoryAuthorityFixtureV1,
        PrincipalIdV1,
    ) {
        let domain = StoreDomainV1::derive(StoreRoleV1::Repository, domain_seed).unwrap();
        let root = ContractRootIdV1::parse(&render_digest([141; 32])).unwrap();
        let scope = StepScopeV1::new(
            domain.id(),
            WorkIdV1::derive("stage4-step-origin-work").unwrap(),
        );
        let binding = StepBindingV1::new(
            scope,
            ContractGenerationIdV1::parse(&render_digest([142; 32])).unwrap(),
            root,
            StepIdV1::from_bytes(scope, [143; 32]).unwrap(),
            StepRevisionIdV1::from_bytes([144; 32]).unwrap(),
        )
        .unwrap();
        let effect_subject_commitment = hash(
            &effect_authority_subject_value(
                EffectIntentDomainKindV1::RepositoryDomain,
                HomeTokenV1::new(*domain.id().as_bytes()),
                HomeTokenV1::new([82; 32]),
                HomeTokenV1::new([83; 32]),
                HomeTokenV1::new([84; 32]),
                EffectSemanticUseV1::new([86; 32]).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let fixture = repository_authority_fixture(
            vec![("OriginateEffectIntent", effect_subject_commitment)],
            AuthorityFixtureModeV1::Valid,
        );
        let executor = fixture.actor_principal;
        let carrier = StepExecutionCarrierV1::acquire(StepExecutionAcquisitionV1 {
            binding,
            next_fence: 1,
            executor,
            store_generation_id: StoreGenerationIdV1::from_digest([230; 32]),
            authority_epoch: fixture.authority_epoch,
            fixed_envelope_commitment: [145; 32],
            run_limit: 4,
            issued_at: 120,
            expires_at: 150,
            hard_deadline: 180,
            authority: test_authorized_execution_action(
                ExecutionActionV1::AcquireStepExecution,
                "stage4-step-origin-fixture-acquire",
            ),
        })
        .unwrap();
        let carrier_object = step_execution_carrier_object(&carrier).unwrap();
        let index = build_step_execution_index_object(&[StepExecutionIndexEntryV1 {
            binding_commitment: hash(&step_binding_store_value(binding)).unwrap(),
            carrier_object_id: carrier_object.id(),
            fence_high_water: 1,
        }])
        .unwrap();
        let step_state = open_step_state_object(binding);
        let step_graph = current_step_graph_object(binding);
        let mut objects = fixture.objects.clone();
        objects.extend([
            step_state.clone(),
            step_graph.clone(),
            carrier_object,
            index.clone(),
        ]);
        let store_root = test_root();
        let mut store = StoreV1::create(&store_root, domain.clone()).unwrap();
        put_objects_in_reference_order(&mut store, objects);
        let mut roots = vec![
            fixture.authority_root_id,
            step_state.id(),
            step_graph.id(),
            index.id(),
        ];
        roots.sort_unstable();
        let initial_generation = StoreGenerationV1::new(
            domain.clone(),
            1,
            None,
            root,
            StoreCompatibilityV1::stage0_successor().unwrap(),
            roots,
        )
        .unwrap();
        store.publish_generation(&initial_generation, None).unwrap();
        activate_store(&store_root);
        (store_root, domain, store, binding, fixture, executor)
    }

    fn test_authorized_execution_action(
        action: ExecutionActionV1,
        seed: &str,
    ) -> AuthorizedExecutionActionV1 {
        let request = CanonicalExecutionActionRequestV1::new(
            action,
            [231; 32],
            [232; 32],
            [233; 32],
            IdempotencyKeyIdV1::derive(seed).unwrap(),
        )
        .unwrap();
        let receipt = AuthorizationReceiptV1::new(
            request.request_id(),
            AuthorityContextIdV1::derive("stage4-step-origin-test-context").unwrap(),
            ActionAuthorityBasisKindV1::OrdinaryLiveRuntime,
            StateTokenIdV1::derive(&format!("{seed}-prior")).unwrap(),
            StateTokenIdV1::derive(&format!("{seed}-result")).unwrap(),
        )
        .unwrap();
        AuthorizedExecutionActionV1::new(request, receipt).unwrap()
    }

    fn replace_step_carrier_objects(
        active_objects: &[StoreObjectV1],
        binding: StepBindingV1,
        carrier: &StepExecutionCarrierV1,
    ) -> Vec<StoreObjectV1> {
        let carrier_schema =
            execution_schema_id("maestro.vnext.step-execution-carrier-schema.v1").unwrap();
        let index_schema =
            execution_schema_id("maestro.vnext.step-execution-index-schema.v1").unwrap();
        let mut replaced = active_objects
            .iter()
            .filter(|object| {
                object.schema_id() != carrier_schema && object.schema_id() != index_schema
            })
            .cloned()
            .collect::<Vec<_>>();
        let carrier_object = step_execution_carrier_object(carrier).unwrap();
        let index = build_step_execution_index_object(&[StepExecutionIndexEntryV1 {
            binding_commitment: hash(&step_binding_store_value(binding)).unwrap(),
            carrier_object_id: carrier_object.id(),
            fence_high_water: carrier.tenure().attempt().fence(),
        }])
        .unwrap();
        replaced.extend([carrier_object, index]);
        replaced
    }

    fn publish_initial_step_acquisition(
        store: &mut StoreV1,
        binding: StepBindingV1,
        selection: RepositoryAuthoritySelectionV1,
        executor: PrincipalIdV1,
        idempotency_seed: &str,
    ) -> StepExecutionPublicationOutcomeV1 {
        let snapshot = ExecutionStoreFacadeV1::new(store)
            .current_step_execution(binding)
            .unwrap();
        let mutation = AuthorizedStepExecutionMutationV1::Acquire {
            executor,
            fixed_envelope_commitment: [145; 32],
            run_limit: 4,
            issued_at: 120,
            expires_at: 150,
            hard_deadline: 180,
            takeover_safety: None,
        };
        let request = ExecutionStoreFacadeV1::new(store)
            .canonical_step_request(
                &snapshot,
                &mutation,
                IdempotencyKeyIdV1::derive(idempotency_seed).unwrap(),
            )
            .unwrap();
        let authority = GenericExecutionAuthorityV1::new(
            selection,
            RepositoryActionLeafV1::AcquireStepExecution,
            request.subject_commitment(),
            request.expected_state_commitment(),
            request.payload_commitment(),
            executor,
        )
        .unwrap();
        ExecutionStoreFacadeV1::new(store)
            .publish_step_execution(
                StepExecutionPublicationV1::new(request, authority, snapshot, mutation).unwrap(),
            )
            .unwrap()
    }

    fn step_submission_plan_for_claim(
        store: &mut StoreV1,
        snapshot: &StepExecutionSnapshotV1,
        observation: ObservationV1,
        submission_id: StepSubmissionIdV1,
        fixture: &RepositoryAuthorityFixtureV1,
        seed: &str,
        claim_byte: u8,
    ) -> StepSubmissionPublicationV1 {
        let binding = snapshot.binding();
        let term_id = snapshot.carrier().unwrap().tenure().current_term().id();
        let fence = snapshot
            .carrier()
            .unwrap()
            .submission_fence(term_id, 120)
            .unwrap();
        let evidence = EvidenceClaimPublicationV1::new(
            SubmissionRefV1::for_step(submission_id).unwrap(),
            vec![
                ClaimV1::new(
                    SubmissionRefV1::for_step(submission_id).unwrap(),
                    ClaimSubjectV1::for_step(binding, fence.fence()).unwrap(),
                    [claim_byte; 32],
                    vec![observation.id()],
                )
                .unwrap(),
            ],
            vec![observation],
        )
        .unwrap();
        let submission = ExecutionStoreFacadeV1::new(store)
            .step_submission_candidate(snapshot, submission_id, term_id, 120, &evidence)
            .unwrap();
        let request = ExecutionStoreFacadeV1::new(store)
            .canonical_step_submission_request(
                snapshot,
                &submission,
                120,
                IdempotencyKeyIdV1::derive(&format!("{seed}-request")).unwrap(),
            )
            .unwrap();
        let authority = SubmitStepAuthorityV1::new(
            fixture.selection,
            request.subject_commitment(),
            request.expected_state_commitment(),
            request.payload_commitment(),
            fixture.actor_principal,
        )
        .unwrap();
        StepSubmissionPublicationV1::new(
            request,
            authority,
            snapshot.clone(),
            submission,
            evidence,
            120,
        )
        .unwrap()
    }

    fn current_step_graph_object(binding: StepBindingV1) -> StoreObjectV1 {
        let canonical_graph = CborValue::Array(vec![
            CborValue::Unsigned(1),
            CborValue::Array(vec![
                bytes(binding.scope().repository_id().as_bytes()),
                bytes(binding.scope().work_id().as_bytes()),
            ]),
            bytes(binding.contract_generation_id().as_bytes()),
            bytes(binding.contract_root_id().as_bytes()),
            CborValue::Array(vec![CborValue::Array(vec![
                step_binding_store_value(binding),
                CborValue::Bool(true),
            ])]),
            CborValue::Array(vec![]),
        ]);
        StoreObjectV1::new(
            RepositoryStoreSchemaV1::StepGraph.schema_id().unwrap(),
            CborValue::Array(vec![
                CborValue::text("maestro.vnext.repository-step-graph.v1").unwrap(),
                CborValue::Bytes(deterministic_cbor::encode(&canonical_graph).unwrap()),
            ]),
            vec![],
        )
        .unwrap()
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

    fn activate_store(root: &std::path::Path) {
        let connection = Connection::open(root.join("store.sqlite3")).unwrap();
        connection
            .execute(
                "UPDATE store_state SET state = 'active', state_revision = state_revision + 1 WHERE singleton = 1",
                [],
            )
            .unwrap();
    }

    fn maximum_capacity_spent(store: &StoreV1) -> u64 {
        let capacity_schema = SchemaIdV1::parse(
            "sha256:aad1f227fc516a5429332870548038f691809424e75f1ac26a52ffcc5f762ea2",
        )
        .unwrap();
        store
            .coherent_publication_snapshot()
            .unwrap()
            .3
            .into_iter()
            .filter(|object| object.schema_id() == capacity_schema)
            .filter_map(|object| match object.value() {
                CborValue::Array(fields) => match fields.get(6) {
                    Some(CborValue::Unsigned(spent)) => Some(*spent),
                    _ => None,
                },
                _ => None,
            })
            .max()
            .unwrap()
    }

    fn effect_store_fixture(
        domain_seed: &[u8],
    ) -> (
        std::path::PathBuf,
        StoreDomainV1,
        StoreV1,
        ActiveStoreEffectOriginationDraftV1,
        RepositoryAuthoritySelectionV1,
        PrincipalIdV1,
    ) {
        effect_store_fixture_for_role(StoreRoleV1::Repository, domain_seed)
    }

    fn effect_store_fixture_at(
        domain_seed: &[u8],
        trusted_time_lower: u64,
        trusted_time_upper: u64,
    ) -> (
        std::path::PathBuf,
        StoreDomainV1,
        StoreV1,
        ActiveStoreEffectOriginationDraftV1,
        RepositoryAuthoritySelectionV1,
        PrincipalIdV1,
    ) {
        effect_store_fixture_for_role_at(
            StoreRoleV1::Repository,
            domain_seed,
            trusted_time_lower,
            trusted_time_upper,
        )
    }

    fn effect_store_fixture_for_role(
        role: StoreRoleV1,
        domain_seed: &[u8],
    ) -> (
        std::path::PathBuf,
        StoreDomainV1,
        StoreV1,
        ActiveStoreEffectOriginationDraftV1,
        RepositoryAuthoritySelectionV1,
        PrincipalIdV1,
    ) {
        effect_store_fixture_for_role_at(role, domain_seed, 120, 130)
    }

    fn effect_store_fixture_for_role_at(
        role: StoreRoleV1,
        domain_seed: &[u8],
        trusted_time_lower: u64,
        trusted_time_upper: u64,
    ) -> (
        std::path::PathBuf,
        StoreDomainV1,
        StoreV1,
        ActiveStoreEffectOriginationDraftV1,
        RepositoryAuthoritySelectionV1,
        PrincipalIdV1,
    ) {
        let domain = StoreDomainV1::derive(role, domain_seed).unwrap();
        let root = ContractRootIdV1::parse(&render_digest([81; 32])).unwrap();
        let draft =
            effect_origination_draft(role, &domain, EffectOriginKindV1::EffectRemediationOrigin);
        let unrelated_draft = unrelated_effect_origination_draft(role, &domain);
        let subject_commitment = hash(&draft.authority_subject_value().unwrap()).unwrap();
        let unrelated_subject_commitment =
            hash(&unrelated_draft.authority_subject_value().unwrap()).unwrap();
        let continuation_subject_commitment =
            hash(&draft.authority_subject_value().unwrap()).unwrap();
        let fixture = match role {
            StoreRoleV1::Repository => repository_authority_fixture_at(
                vec![
                    ("OriginateEffectIntent", subject_commitment),
                    ("OriginateEffectIntent", unrelated_subject_commitment),
                    ("RecordDispatchOutcome", continuation_subject_commitment),
                    ("ReconcileEffectIntent", continuation_subject_commitment),
                    ("WithdrawEffectIntent", continuation_subject_commitment),
                ],
                AuthorityFixtureModeV1::Valid,
                trusted_time_lower,
                trusted_time_upper,
            ),
            StoreRoleV1::Installation => installation_authority_fixture(
                vec![
                    ("OriginateEffectIntent", subject_commitment),
                    ("OriginateEffectIntent", unrelated_subject_commitment),
                    ("RecordDispatchOutcome", continuation_subject_commitment),
                    ("ReconcileEffectIntent", continuation_subject_commitment),
                    ("WithdrawEffectIntent", continuation_subject_commitment),
                ],
                AuthorityFixtureModeV1::Valid,
            ),
        };
        let store_root = test_root();
        let mut store = StoreV1::create(&store_root, domain.clone()).unwrap();
        put_objects_in_reference_order(&mut store, fixture.objects);
        let generation = StoreGenerationV1::new(
            domain.clone(),
            1,
            None,
            root,
            StoreCompatibilityV1::stage0_successor().unwrap(),
            vec![fixture.authority_root_id],
        )
        .unwrap();
        store.publish_generation(&generation, None).unwrap();
        activate_store(&store_root);
        (
            store_root,
            domain,
            store,
            draft,
            fixture.selection,
            fixture.actor_principal,
        )
    }

    fn effect_origination_draft(
        role: StoreRoleV1,
        domain: &StoreDomainV1,
        origin_kind: EffectOriginKindV1,
    ) -> ActiveStoreEffectOriginationDraftV1 {
        ActiveStoreEffectOriginationDraftV1 {
            domain_kind: effect_domain_kind_for_store_role(role),
            stable_domain_id: HomeTokenV1::new(*domain.id().as_bytes()),
            realm: HomeTokenV1::new([82; 32]),
            semantic_namespace: HomeTokenV1::new([83; 32]),
            uniqueness_namespace: HomeTokenV1::new([84; 32]),
            origin: EffectOriginV1::non_step(origin_kind, [85; 32]).unwrap(),
            semantic_use: EffectSemanticUseV1::new([86; 32]).unwrap(),
            material_inputs: EffectMaterialInputsV1::new([87; 32]).unwrap(),
            credential_requirements: EffectCredentialRequirementsV1::new([88; 32]).unwrap(),
            dispatch: EffectDispatchBindingInputsV1 {
                attempt_revision: 1,
                application_envelope_commitment: [89; 32],
                provider_operation_contract_commitment: [90; 32],
                provider_scope_commitment: [91; 32],
                provider_key_commitment: [92; 32],
                material_stamp_commitment: [93; 32],
                run_set_revision_commitment: [94; 32],
                accounting_basis_commitment: [95; 32],
                provider_run: RunReservationV1 {
                    semantic_operation_hash: [96; 32],
                    inputs_commitment: [97; 32],
                    environment_commitment: [98; 32],
                    target_commitment: [99; 32],
                    execution_boundary_commitment: [100; 32],
                    deadline: 150,
                    launch_ordinal: 1,
                    current_step_term: None,
                },
            },
        }
    }

    fn step_effect_origination_draft(
        domain: &StoreDomainV1,
        tenure: &super::super::runtime::StepExecutionTenureV1,
        as_of: u64,
    ) -> ActiveStoreEffectOriginationDraftV1 {
        ActiveStoreEffectOriginationDraftV1 {
            domain_kind: EffectIntentDomainKindV1::RepositoryDomain,
            stable_domain_id: HomeTokenV1::new(*domain.id().as_bytes()),
            realm: HomeTokenV1::new([82; 32]),
            semantic_namespace: HomeTokenV1::new([83; 32]),
            uniqueness_namespace: HomeTokenV1::new([84; 32]),
            origin: EffectOriginV1::step(tenure, as_of).unwrap(),
            semantic_use: EffectSemanticUseV1::new([86; 32]).unwrap(),
            material_inputs: EffectMaterialInputsV1::new([87; 32]).unwrap(),
            credential_requirements: EffectCredentialRequirementsV1::new([88; 32]).unwrap(),
            dispatch: EffectDispatchBindingInputsV1 {
                attempt_revision: 1,
                application_envelope_commitment: [89; 32],
                provider_operation_contract_commitment: [90; 32],
                provider_scope_commitment: [91; 32],
                provider_key_commitment: [92; 32],
                material_stamp_commitment: [93; 32],
                run_set_revision_commitment: [94; 32],
                accounting_basis_commitment: [95; 32],
                provider_run: RunReservationV1 {
                    semantic_operation_hash: [96; 32],
                    inputs_commitment: [97; 32],
                    environment_commitment: [98; 32],
                    target_commitment: [99; 32],
                    execution_boundary_commitment: [100; 32],
                    deadline: 150,
                    launch_ordinal: 1,
                    current_step_term: None,
                },
            },
        }
    }

    fn unrelated_effect_origination_draft(
        role: StoreRoleV1,
        domain: &StoreDomainV1,
    ) -> ActiveStoreEffectOriginationDraftV1 {
        let mut draft =
            effect_origination_draft(role, domain, EffectOriginKindV1::EffectRemediationOrigin);
        draft.origin =
            EffectOriginV1::non_step(EffectOriginKindV1::EffectRemediationOrigin, [201; 32])
                .unwrap();
        draft.semantic_use = EffectSemanticUseV1::new([202; 32]).unwrap();
        draft.material_inputs = EffectMaterialInputsV1::new([203; 32]).unwrap();
        draft.dispatch.application_envelope_commitment = [204; 32];
        draft.dispatch.provider_key_commitment = [205; 32];
        draft.dispatch.provider_run.semantic_operation_hash = [206; 32];
        draft
    }

    fn specialized_effect_store_fixture(
        domain_seed: &[u8],
        origin_kind: EffectOriginKindV1,
    ) -> (
        std::path::PathBuf,
        StoreDomainV1,
        StoreV1,
        ActiveStoreEffectOriginationDraftV1,
        RepositoryAuthorityFixtureV1,
    ) {
        let domain = StoreDomainV1::derive(StoreRoleV1::Repository, domain_seed).unwrap();
        let root = ContractRootIdV1::parse(&render_digest([161; 32])).unwrap();
        let draft = effect_origination_draft(StoreRoleV1::Repository, &domain, origin_kind);
        let subject_commitment = hash(&draft.authority_subject_value().unwrap()).unwrap();
        let continuation_subject_commitment =
            hash(&draft.authority_subject_value().unwrap()).unwrap();
        let action_scopes = vec![
            (
                draft.origin.reservation_action().unwrap(),
                subject_commitment,
            ),
            (
                draft.origin.outcome_action().unwrap(),
                continuation_subject_commitment,
            ),
            (
                draft.origin.outcome_action().unwrap(),
                continuation_subject_commitment,
            ),
            (
                draft.origin.reconciliation_action().unwrap(),
                continuation_subject_commitment,
            ),
            (
                draft.origin.withdrawal_action().unwrap(),
                continuation_subject_commitment,
            ),
        ];
        let action_scopes = action_scopes
            .into_iter()
            .map(|(action, subject)| {
                (
                    repository_leaf_for_execution_action(action).literal(),
                    subject,
                )
            })
            .collect();
        let mut fixture =
            repository_authority_fixture(action_scopes, AuthorityFixtureModeV1::Valid);
        let store_root = test_root();
        let mut store = StoreV1::create(&store_root, domain.clone()).unwrap();
        put_objects_in_reference_order(&mut store, std::mem::take(&mut fixture.objects));
        let generation = StoreGenerationV1::new(
            domain.clone(),
            1,
            None,
            root,
            StoreCompatibilityV1::stage0_successor().unwrap(),
            vec![fixture.authority_root_id],
        )
        .unwrap();
        store.publish_generation(&generation, None).unwrap();
        activate_store(&store_root);
        (store_root, domain, store, draft, fixture)
    }

    fn specialized_execution_authority(
        fixture: &RepositoryAuthorityFixtureV1,
        request: &CanonicalExecutionActionRequestV1,
        phase_ordinal: usize,
    ) -> ExecutionAuthorityV1 {
        let action = repository_leaf_for_execution_action(request.action());
        match action.execution_authority_basis().unwrap() {
            ActionAuthorityBasisKindV1::BootstrapControlG0 => BootstrapExecutionAuthorityV1::new(
                fixture.bootstrap_basis,
                action,
                request.subject_commitment(),
                request.expected_state_commitment(),
                request.payload_commitment(),
                fixture.actor_principal,
            )
            .unwrap()
            .into(),
            ActionAuthorityBasisKindV1::ContinuityMaintenance => {
                let (_, basis, withdrawal_slot_family, purpose, job_applicability_commitment) =
                    fixture
                        .cma_bases
                        .iter()
                        .filter(|(candidate, ..)| *candidate == action)
                        .nth(phase_ordinal)
                        .copied()
                        .unwrap();
                ContinuityMaintenanceExecutionAuthorityV1::new(
                    basis,
                    withdrawal_slot_family,
                    purpose,
                    action,
                    request.subject_commitment(),
                    request.expected_state_commitment(),
                    request.payload_commitment(),
                    fixture.actor_principal,
                    fixture.continuity_state_token,
                    fixture.continuity_state_object_id,
                    fixture.guard_object_id,
                    fixture.authority_epoch,
                    job_applicability_commitment,
                )
                .unwrap()
                .into()
            }
            ActionAuthorityBasisKindV1::OrdinaryLiveRuntime => {
                panic!("invariant: specialized fixture requested ordinary authority")
            }
        }
    }

    fn specialized_effect_origination_plan(
        store: &mut StoreV1,
        draft: &ActiveStoreEffectOriginationDraftV1,
        fixture: &RepositoryAuthorityFixtureV1,
        key: &str,
    ) -> ActiveStoreEffectOriginationPublicationV1 {
        let request = ExecutionStoreFacadeV1::new(store)
            .canonical_effect_origination_request(draft, IdempotencyKeyIdV1::derive(key).unwrap())
            .unwrap();
        let authority = specialized_execution_authority(fixture, &request, 0);
        let state_binding = ExecutionStoreFacadeV1::new(store)
            .current_state_binding()
            .unwrap();
        ActiveStoreEffectOriginationPublicationV1::new(
            request,
            authority,
            state_binding,
            draft.clone(),
        )
        .unwrap()
    }

    fn specialized_effect_seal_plan(
        store: &mut StoreV1,
        intent: EffectIntentIdV1,
        fixture: &RepositoryAuthorityFixtureV1,
        key: &str,
    ) -> ActiveStoreEffectSealPublicationV1 {
        let snapshot = ExecutionStoreFacadeV1::new(store)
            .current_effect_snapshot(intent)
            .unwrap();
        let draft = ActiveStoreEffectSealDraftV1::new([162; 32]).unwrap();
        let request = ExecutionStoreFacadeV1::new(store)
            .canonical_effect_seal_request(
                &snapshot,
                draft,
                IdempotencyKeyIdV1::derive(key).unwrap(),
            )
            .unwrap();
        let authority = specialized_execution_authority(fixture, &request, 0);
        ActiveStoreEffectSealPublicationV1::new(request, authority, snapshot, draft).unwrap()
    }

    fn specialized_effect_terminal_plan(
        store: &mut StoreV1,
        intent: EffectIntentIdV1,
        fixture: &RepositoryAuthorityFixtureV1,
        key: &str,
        draft: ActiveStoreEffectTerminalDraftV1,
    ) -> ActiveStoreEffectTerminalPublicationV1 {
        let snapshot = ExecutionStoreFacadeV1::new(store)
            .current_effect_snapshot(intent)
            .unwrap();
        let request = ExecutionStoreFacadeV1::new(store)
            .canonical_effect_terminal_request(
                &snapshot,
                &draft,
                IdempotencyKeyIdV1::derive(key).unwrap(),
            )
            .unwrap();
        let phase_ordinal = usize::from(matches!(
            snapshot.dispatch().state(),
            crate::domain::execution::dispatch_state::DispatchAttemptStateV1::SealedInFlight(_)
        ));
        let authority = specialized_execution_authority(fixture, &request, phase_ordinal);
        ActiveStoreEffectTerminalPublicationV1::new(request, authority, snapshot, draft).unwrap()
    }

    fn specialized_effect_reconciliation_begin_plan(
        store: &mut StoreV1,
        intent: EffectIntentIdV1,
        fixture: &RepositoryAuthorityFixtureV1,
        key: &str,
    ) -> ActiveStoreEffectReconciliationBeginPublicationV1 {
        let snapshot = ExecutionStoreFacadeV1::new(store)
            .current_effect_snapshot(intent)
            .unwrap();
        let read_plan = reconciliation_read_plan();
        let draft = ActiveStoreEffectReconciliationBeginDraftV1::new(
            read_plan,
            RunReservationV1 {
                semantic_operation_hash: read_plan.commitment().unwrap(),
                inputs_commitment: [163; 32],
                environment_commitment: [164; 32],
                target_commitment: [165; 32],
                execution_boundary_commitment: [166; 32],
                deadline: 200,
                launch_ordinal: 1,
                current_step_term: None,
            },
        );
        let request = ExecutionStoreFacadeV1::new(store)
            .canonical_effect_reconciliation_begin_request(
                &snapshot,
                &draft,
                IdempotencyKeyIdV1::derive(key).unwrap(),
            )
            .unwrap();
        let authority = specialized_execution_authority(fixture, &request, 0);
        ActiveStoreEffectReconciliationBeginPublicationV1::new(request, authority, snapshot, draft)
            .unwrap()
    }

    fn specialized_effect_withdrawal_plan(
        store: &mut StoreV1,
        intent: EffectIntentIdV1,
        fixture: &RepositoryAuthorityFixtureV1,
        key: &str,
    ) -> ActiveStoreEffectWithdrawalPublicationV1 {
        let snapshot = ExecutionStoreFacadeV1::new(store)
            .current_effect_snapshot(intent)
            .unwrap();
        let draft = ActiveStoreEffectWithdrawalDraftV1::new();
        let request = ExecutionStoreFacadeV1::new(store)
            .canonical_effect_withdrawal_request(
                &snapshot,
                draft,
                IdempotencyKeyIdV1::derive(key).unwrap(),
            )
            .unwrap();
        let authority = specialized_execution_authority(fixture, &request, 0);
        ActiveStoreEffectWithdrawalPublicationV1::new(request, authority, snapshot, draft).unwrap()
    }

    fn exercise_specialized_effect_route(
        origin_kind: EffectOriginKindV1,
        reconciliation_seed: &[u8],
        withdrawal_seed: &[u8],
    ) {
        let (store_root, domain, mut store, draft, fixture) =
            specialized_effect_store_fixture(reconciliation_seed, origin_kind);
        let origination = specialized_effect_origination_plan(
            &mut store,
            &draft,
            &fixture,
            "stage4-specialized-origination",
        );
        let originated = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_origination(origination)
            .unwrap();
        let seal = specialized_effect_seal_plan(
            &mut store,
            originated.intent(),
            &fixture,
            "stage4-specialized-seal",
        );
        let mut sealed = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_seal(seal)
            .unwrap();
        let terminal_draft = provider_terminal_draft_from_outcome(
            &mut store,
            sealed.take_provider_release().unwrap(),
            EffectDispatchOutcomePayloadV1::AmbiguousTransport {
                evidence_commitment: [167; 32],
            },
        )
        .unwrap();
        let terminal = specialized_effect_terminal_plan(
            &mut store,
            originated.intent(),
            &fixture,
            "stage4-specialized-terminal",
            terminal_draft,
        );
        ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_terminal(terminal)
            .unwrap();
        let begin = specialized_effect_reconciliation_begin_plan(
            &mut store,
            originated.intent(),
            &fixture,
            "stage4-specialized-reconciliation",
        );
        let mut begun = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_reconciliation_begin(begin)
            .unwrap();
        let read_release = begun.take_read_release().unwrap();
        let read_snapshot = ExecutionStoreFacadeV1::new(&mut store)
            .current_effect_snapshot(originated.intent())
            .unwrap();
        let read = ActiveStoreEffectReconciliationReadPublicationV1::new(
            read_snapshot,
            reconciliation_read_draft(
                &mut store,
                read_release,
                [168; 32],
                ProviderApplicationFactV1::Applied,
            ),
        )
        .unwrap();
        ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_reconciliation_read(read)
            .unwrap();
        let terminal_snapshot = ExecutionStoreFacadeV1::new(&mut store)
            .current_effect_snapshot(originated.intent())
            .unwrap();
        let terminal_draft = terminal_snapshot.reconciliation_terminal_draft().unwrap();
        let reconciliation_terminal = ActiveStoreEffectReconciliationTerminalPublicationV1::new(
            terminal_snapshot,
            terminal_draft,
        )
        .unwrap();
        let reconciled = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_reconciliation_terminal(reconciliation_terminal)
            .unwrap();
        assert_eq!(maximum_capacity_spent(&store), 4);
        drop(store);

        let mut reopened = StoreV1::open(&store_root, domain).unwrap();
        let restored = ExecutionStoreFacadeV1::new(&mut reopened)
            .current_effect_snapshot(originated.intent())
            .unwrap();
        assert_eq!(restored.control_head().id(), reconciled.control_head());
        assert_eq!(
            restored.control_revision().classification(),
            RemoteClassificationV1::ConfirmedApplied
        );
        assert_eq!(maximum_capacity_spent(&reopened), 4);

        let (store_root, domain, mut store, draft, fixture) =
            specialized_effect_store_fixture(withdrawal_seed, origin_kind);
        let origination = specialized_effect_origination_plan(
            &mut store,
            &draft,
            &fixture,
            "stage4-specialized-withdrawal-origination",
        );
        let originated = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_origination(origination)
            .unwrap();
        let terminal = specialized_effect_terminal_plan(
            &mut store,
            originated.intent(),
            &fixture,
            "stage4-specialized-withdrawal-terminal",
            ActiveStoreEffectTerminalDraftV1::new(
                EffectDispatchOutcomePayloadV1::LocallyRejected {
                    evidence_commitment: [169; 32],
                },
            )
            .unwrap(),
        );
        ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_terminal(terminal)
            .unwrap();
        let withdrawal = specialized_effect_withdrawal_plan(
            &mut store,
            originated.intent(),
            &fixture,
            "stage4-specialized-withdrawal",
        );
        let withdrawn = ExecutionStoreFacadeV1::new(&mut store)
            .publish_effect_withdrawal(withdrawal)
            .unwrap();
        assert_eq!(withdrawn.provider_io_operations(), 0);
        assert_eq!(maximum_capacity_spent(&store), 3);
        drop(store);

        let mut reopened = StoreV1::open(&store_root, domain).unwrap();
        let restored = ExecutionStoreFacadeV1::new(&mut reopened)
            .current_effect_snapshot(originated.intent())
            .unwrap();
        assert_eq!(restored.control_head().id(), withdrawn.control_head());
        assert_eq!(
            restored.control_revision().classification(),
            RemoteClassificationV1::Cancelled
        );
        assert_eq!(maximum_capacity_spent(&reopened), 3);
    }

    fn active_effect_origination_plan(
        store: &mut StoreV1,
        draft: &ActiveStoreEffectOriginationDraftV1,
        selection: RepositoryAuthoritySelectionV1,
        executor: PrincipalIdV1,
        key: &str,
    ) -> ActiveStoreEffectOriginationPublicationV1 {
        let request = ExecutionStoreFacadeV1::new(store)
            .canonical_effect_origination_request(draft, IdempotencyKeyIdV1::derive(key).unwrap())
            .unwrap();
        let state_binding = ExecutionStoreFacadeV1::new(store)
            .current_state_binding()
            .unwrap();
        let authority = GenericExecutionAuthorityV1::new(
            selection,
            repository_leaf_for_execution_action(request.action()),
            request.subject_commitment(),
            request.expected_state_commitment(),
            request.payload_commitment(),
            executor,
        )
        .unwrap();
        ActiveStoreEffectOriginationPublicationV1::new(
            request,
            authority,
            state_binding,
            draft.clone(),
        )
        .unwrap()
    }

    fn active_effect_redispatch_plan(
        store: &mut StoreV1,
        intent: EffectIntentIdV1,
        selection: RepositoryAuthoritySelectionV1,
        executor: PrincipalIdV1,
        key: &str,
        dispatch: EffectDispatchBindingInputsV1,
    ) -> ActiveStoreEffectRedispatchPublicationV1 {
        let snapshot = ExecutionStoreFacadeV1::new(store)
            .current_effect_snapshot(intent)
            .unwrap();
        let draft = ActiveStoreEffectRedispatchDraftV1::new(dispatch);
        let request = ExecutionStoreFacadeV1::new(store)
            .canonical_effect_redispatch_request(
                &snapshot,
                &draft,
                IdempotencyKeyIdV1::derive(key).unwrap(),
            )
            .unwrap();
        let authority = GenericExecutionAuthorityV1::new(
            selection,
            repository_leaf_for_execution_action(request.action()),
            request.subject_commitment(),
            request.expected_state_commitment(),
            request.payload_commitment(),
            executor,
        )
        .unwrap();
        ActiveStoreEffectRedispatchPublicationV1::new(request, authority, snapshot, draft).unwrap()
    }

    fn active_effect_recover_reserved_plan(
        store: &mut StoreV1,
        intent: EffectIntentIdV1,
        selection: RepositoryAuthoritySelectionV1,
        executor: PrincipalIdV1,
        key: &str,
        draft: ActiveStoreEffectRecoverReservedDraftV1,
    ) -> ActiveStoreEffectRecoverReservedPublicationV1 {
        let snapshot = ExecutionStoreFacadeV1::new(store)
            .current_effect_snapshot(intent)
            .unwrap();
        let request = ExecutionStoreFacadeV1::new(store)
            .canonical_effect_recover_reserved_request(
                &snapshot,
                &draft,
                IdempotencyKeyIdV1::derive(key).unwrap(),
            )
            .unwrap();
        let authority = GenericExecutionAuthorityV1::new(
            selection,
            repository_leaf_for_execution_action(request.action()),
            request.subject_commitment(),
            request.expected_state_commitment(),
            request.payload_commitment(),
            executor,
        )
        .unwrap();
        ActiveStoreEffectRecoverReservedPublicationV1::new(request, authority, snapshot, draft)
            .unwrap()
    }

    fn active_effect_seal_plan(
        store: &mut StoreV1,
        intent: EffectIntentIdV1,
        selection: RepositoryAuthoritySelectionV1,
        executor: PrincipalIdV1,
        key: &str,
        seal_commitment: [u8; 32],
    ) -> ActiveStoreEffectSealPublicationV1 {
        let snapshot = ExecutionStoreFacadeV1::new(store)
            .current_effect_snapshot(intent)
            .unwrap();
        let draft = ActiveStoreEffectSealDraftV1::new(seal_commitment).unwrap();
        let request = ExecutionStoreFacadeV1::new(store)
            .canonical_effect_seal_request(
                &snapshot,
                draft,
                IdempotencyKeyIdV1::derive(key).unwrap(),
            )
            .unwrap();
        let authority = GenericExecutionAuthorityV1::new(
            selection,
            repository_leaf_for_execution_action(request.action()),
            request.subject_commitment(),
            request.expected_state_commitment(),
            request.payload_commitment(),
            executor,
        )
        .unwrap();
        ActiveStoreEffectSealPublicationV1::new(request, authority, snapshot, draft).unwrap()
    }

    fn active_effect_terminal_plan(
        store: &mut StoreV1,
        intent: EffectIntentIdV1,
        selection: RepositoryAuthoritySelectionV1,
        executor: PrincipalIdV1,
        key: &str,
        outcome: EffectDispatchOutcomePayloadV1,
    ) -> ActiveStoreEffectTerminalPublicationV1 {
        active_effect_terminal_plan_with_draft(
            store,
            intent,
            selection,
            executor,
            key,
            ActiveStoreEffectTerminalDraftV1::new(outcome).unwrap(),
        )
    }

    fn active_effect_terminal_plan_with_draft(
        store: &mut StoreV1,
        intent: EffectIntentIdV1,
        selection: RepositoryAuthoritySelectionV1,
        executor: PrincipalIdV1,
        key: &str,
        draft: ActiveStoreEffectTerminalDraftV1,
    ) -> ActiveStoreEffectTerminalPublicationV1 {
        let snapshot = ExecutionStoreFacadeV1::new(store)
            .current_effect_snapshot(intent)
            .unwrap();
        let request = ExecutionStoreFacadeV1::new(store)
            .canonical_effect_terminal_request(
                &snapshot,
                &draft,
                IdempotencyKeyIdV1::derive(key).unwrap(),
            )
            .unwrap();
        let authority = GenericExecutionAuthorityV1::new(
            selection,
            repository_leaf_for_execution_action(request.action()),
            request.subject_commitment(),
            request.expected_state_commitment(),
            request.payload_commitment(),
            executor,
        )
        .unwrap();
        ActiveStoreEffectTerminalPublicationV1::new(request, authority, snapshot, draft).unwrap()
    }

    fn active_effect_withdrawal_plan(
        store: &mut StoreV1,
        intent: EffectIntentIdV1,
        selection: RepositoryAuthoritySelectionV1,
        executor: PrincipalIdV1,
        key: &str,
    ) -> ActiveStoreEffectWithdrawalPublicationV1 {
        let snapshot = ExecutionStoreFacadeV1::new(store)
            .current_effect_snapshot(intent)
            .unwrap();
        let draft = ActiveStoreEffectWithdrawalDraftV1::new();
        let request = ExecutionStoreFacadeV1::new(store)
            .canonical_effect_withdrawal_request(
                &snapshot,
                draft,
                IdempotencyKeyIdV1::derive(key).unwrap(),
            )
            .unwrap();
        let authority = GenericExecutionAuthorityV1::new(
            selection,
            RepositoryActionLeafV1::WithdrawEffectIntent,
            request.subject_commitment(),
            request.expected_state_commitment(),
            request.payload_commitment(),
            executor,
        )
        .unwrap();
        ActiveStoreEffectWithdrawalPublicationV1::new(request, authority, snapshot, draft).unwrap()
    }

    fn active_effect_writer_handoff_plan(
        store: &mut StoreV1,
        intent: EffectIntentIdV1,
        selection: RepositoryAuthoritySelectionV1,
        executor: PrincipalIdV1,
        key: &str,
    ) -> ActiveStoreEffectWriterHandoffPublicationV1 {
        let snapshot = ExecutionStoreFacadeV1::new(store)
            .current_effect_snapshot(intent)
            .unwrap();
        let draft = ActiveStoreEffectWriterHandoffDraftV1::new();
        let request = ExecutionStoreFacadeV1::new(store)
            .canonical_effect_writer_handoff_request(
                &snapshot,
                draft,
                IdempotencyKeyIdV1::derive(key).unwrap(),
            )
            .unwrap();
        let authority = GenericExecutionAuthorityV1::new(
            selection,
            repository_leaf_for_execution_action(request.action()),
            request.subject_commitment(),
            request.expected_state_commitment(),
            request.payload_commitment(),
            executor,
        )
        .unwrap();
        ActiveStoreEffectWriterHandoffPublicationV1::new(request, authority, snapshot, draft)
            .unwrap()
    }

    fn active_effect_health_plan(
        store: &mut StoreV1,
        intent: EffectIntentIdV1,
        selection: RepositoryAuthoritySelectionV1,
        executor: PrincipalIdV1,
        key: &str,
        draft: ActiveStoreEffectHealthDraftV1,
    ) -> ActiveStoreEffectHealthPublicationV1 {
        let snapshot = ExecutionStoreFacadeV1::new(store)
            .current_effect_snapshot(intent)
            .unwrap();
        let request = ExecutionStoreFacadeV1::new(store)
            .canonical_effect_health_request(
                &snapshot,
                draft,
                IdempotencyKeyIdV1::derive(key).unwrap(),
            )
            .unwrap();
        let authority = GenericExecutionAuthorityV1::new(
            selection,
            repository_leaf_for_execution_action(request.action()),
            request.subject_commitment(),
            request.expected_state_commitment(),
            request.payload_commitment(),
            executor,
        )
        .unwrap();
        ActiveStoreEffectHealthPublicationV1::new(request, authority, snapshot, draft).unwrap()
    }

    fn reconciliation_read_plan() -> EffectReconciliationReadPlanV1 {
        EffectReconciliationReadPlanV1::new(EffectReconciliationReadPlanPartsV1 {
            classification: ReconciliationReadOperationClassificationV1::EffectFreeRead,
            operation_kind: ReconciliationReadOperationKindV1::ProviderStatus,
            provider_commitment: [131; 32],
            account_commitment: [132; 32],
            target_commitment: [133; 32],
            correlation_commitment: [134; 32],
            credential_commitment: [135; 32],
            visibility_commitment: [136; 32],
            query_commitment: [137; 32],
            evaluator_commitment: [138; 32],
            max_requests: 1,
            max_pages: 1,
            max_bytes: 2048,
            max_duration_ms: 750,
        })
        .unwrap()
    }

    fn active_effect_reconciliation_begin_plan(
        store: &mut StoreV1,
        intent: EffectIntentIdV1,
        selection: RepositoryAuthoritySelectionV1,
        executor: PrincipalIdV1,
        key: &str,
    ) -> ActiveStoreEffectReconciliationBeginPublicationV1 {
        let snapshot = ExecutionStoreFacadeV1::new(store)
            .current_effect_snapshot(intent)
            .unwrap();
        let read_plan = reconciliation_read_plan();
        let draft = ActiveStoreEffectReconciliationBeginDraftV1::new(
            read_plan,
            RunReservationV1 {
                semantic_operation_hash: read_plan.commitment().unwrap(),
                inputs_commitment: [139; 32],
                environment_commitment: [140; 32],
                target_commitment: [141; 32],
                execution_boundary_commitment: [142; 32],
                deadline: 200,
                launch_ordinal: 1,
                current_step_term: None,
            },
        );
        let request = ExecutionStoreFacadeV1::new(store)
            .canonical_effect_reconciliation_begin_request(
                &snapshot,
                &draft,
                IdempotencyKeyIdV1::derive(key).unwrap(),
            )
            .unwrap();
        let authority = GenericExecutionAuthorityV1::new(
            selection,
            RepositoryActionLeafV1::ReconcileEffectIntent,
            request.subject_commitment(),
            request.expected_state_commitment(),
            request.payload_commitment(),
            executor,
        )
        .unwrap();
        ActiveStoreEffectReconciliationBeginPublicationV1::new(request, authority, snapshot, draft)
            .unwrap()
    }

    fn active_effect_reconciliation_read_plan(
        store: &mut StoreV1,
        intent: EffectIntentIdV1,
        draft: ActiveStoreEffectReconciliationReadDraftV1,
    ) -> ActiveStoreEffectReconciliationReadPublicationV1 {
        let snapshot = ExecutionStoreFacadeV1::new(store)
            .current_effect_snapshot(intent)
            .unwrap();
        ActiveStoreEffectReconciliationReadPublicationV1::new(snapshot, draft).unwrap()
    }

    fn active_effect_reconciliation_terminal_plan(
        store: &mut StoreV1,
        intent: EffectIntentIdV1,
        _selection: RepositoryAuthoritySelectionV1,
        _executor: PrincipalIdV1,
        _key: &str,
        classification: RemoteClassificationV1,
    ) -> ActiveStoreEffectReconciliationTerminalPublicationV1 {
        let snapshot = ExecutionStoreFacadeV1::new(store)
            .current_effect_snapshot(intent)
            .unwrap();
        let draft = snapshot.reconciliation_terminal_draft().unwrap();
        assert_eq!(draft.classification(), classification);
        ActiveStoreEffectReconciliationTerminalPublicationV1::new(snapshot, draft).unwrap()
    }

    fn race_effect_reconciliation_publications<P, F>(
        store_root: &std::path::Path,
        domain: &StoreDomainV1,
        plans: [P; 2],
        publish: F,
    ) -> [Result<ActiveStoreEffectReconciliationOutcomeV1, ExecutionStoreErrorV1>; 2]
    where
        P: Send + 'static,
        F: Fn(
                &mut StoreV1,
                P,
            )
                -> Result<ActiveStoreEffectReconciliationOutcomeV1, ExecutionStoreErrorV1>
            + Copy
            + Send
            + 'static,
    {
        let barrier = Arc::new(Barrier::new(3));
        let workers = plans.map(|plan| {
            let barrier = Arc::clone(&barrier);
            let store_root = store_root.to_path_buf();
            let domain = domain.clone();
            std::thread::spawn(move || {
                let mut store = StoreV1::open(store_root, domain).unwrap();
                barrier.wait();
                publish(&mut store, plan)
            })
        });
        barrier.wait();
        workers.map(|worker| worker.join().unwrap())
    }

    fn assert_reconciliation_race_result(
        results: &[Result<ActiveStoreEffectReconciliationOutcomeV1, ExecutionStoreErrorV1>; 2],
    ) {
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(
                    result,
                    Err(ExecutionStoreErrorV1::StaleExpectedStoreState)
                ))
                .count(),
            1
        );
    }

    #[derive(Clone, Copy)]
    struct StepMutationTestContext {
        binding: StepBindingV1,
        selection: RepositoryAuthoritySelectionV1,
        executor: PrincipalIdV1,
    }

    fn publish_renewed_run_mutation<F>(
        store: &mut StoreV1,
        context: &StepMutationTestContext,
        key: &str,
        issued_at: u64,
        expires_at: u64,
        build_mutation: F,
    ) -> StepExecutionPublicationOutcomeV1
    where
        F: FnOnce(&StepExecutionCarrierV1) -> StepLeaseMutationV1,
    {
        let snapshot = ExecutionStoreFacadeV1::new(store)
            .current_step_execution(context.binding)
            .unwrap();
        let carrier = snapshot.carrier().unwrap();
        let lease_mutation = build_mutation(carrier);
        let expected_term_id = carrier.tenure().current_term().id();
        publish_authorized_step_mutation(
            store,
            context.selection,
            snapshot,
            context.executor,
            key,
            AuthorizedStepExecutionMutationV1::Renew {
                expected_term_id,
                issued_at,
                expires_at,
                lease_mutation: Some(Box::new(lease_mutation)),
            },
        )
    }

    fn publish_authorized_step_mutation(
        store: &mut StoreV1,
        selection: RepositoryAuthoritySelectionV1,
        snapshot: StepExecutionSnapshotV1,
        executor: PrincipalIdV1,
        key: &str,
        mutation: AuthorizedStepExecutionMutationV1,
    ) -> StepExecutionPublicationOutcomeV1 {
        let plan =
            authorized_step_mutation_plan(store, selection, snapshot, executor, key, mutation);
        ExecutionStoreFacadeV1::new(store)
            .publish_step_execution(plan)
            .unwrap_or_else(|error| panic!("{key}: {error:?}"))
    }

    fn authorized_step_mutation_plan(
        store: &mut StoreV1,
        selection: RepositoryAuthoritySelectionV1,
        snapshot: StepExecutionSnapshotV1,
        executor: PrincipalIdV1,
        key: &str,
        mutation: AuthorizedStepExecutionMutationV1,
    ) -> StepExecutionPublicationV1 {
        let request = ExecutionStoreFacadeV1::new(store)
            .canonical_step_request(
                &snapshot,
                &mutation,
                IdempotencyKeyIdV1::derive(key).unwrap(),
            )
            .unwrap();
        let authority = GenericExecutionAuthorityV1::new(
            selection,
            repository_leaf_for_execution_action(request.action()),
            request.subject_commitment(),
            request.expected_state_commitment(),
            request.payload_commitment(),
            executor,
        )
        .unwrap();
        StepExecutionPublicationV1::new(request, authority, snapshot, mutation).unwrap()
    }

    fn test_root() -> std::path::PathBuf {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "maestro-vnext-stage4-execution-{}-{nonce}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&path).unwrap();
        fs::canonicalize(path).unwrap()
    }

    fn render_digest(bytes: [u8; 32]) -> String {
        format!(
            "sha256:{}",
            bytes
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        )
    }
}
