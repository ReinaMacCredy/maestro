//! Authoritative Repository-Store publication for implemented Stage 3 owner actions.

mod bootstrap;

#[allow(
    unused_imports,
    reason = "the Repository facade exports the complete bootstrap owner contract"
)]
pub(crate) use bootstrap::{
    CommittedRepositoryBootstrapV1, RepositoryBootstrapAdmissionV1,
    RepositoryBootstrapAuthorizationV1, RepositoryBootstrapEffectObservationV1,
    RepositoryBootstrapEffectPermitV1, RepositoryBootstrapErrorV1, RepositoryBootstrapOwnerFactsV1,
    RepositoryBootstrapReadbackV1, RepositoryBootstrapTargetFactsV1,
    RepositoryBootstrapTargetReadbackV1,
};

use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::authority::{
    ActionRequestIdV1, ActionResultIdV1, AdmittedRepositoryActionV1, AmendContractAuthorityV1,
    AppendDesignRevisionAuthorityV1, CancelWorkAuthorityV1, CreateDraftWorkAuthorityV1,
    IdempotencyKeyIdV1, PublishInitialContractAuthorityV1, RepositoryActionAdmissionInputV1,
    RepositoryActionLeafV1, RepositoryAuthenticatedHumanV1, RepositoryAuthorityAdmissionErrorV1,
    RepositoryAuthoritySelectionV1, RepositoryDecisionAuthorityCarrierV1,
    RepositoryDecisionOptionMappingV1, RepositoryDecisionPresentationV1,
    RepositoryLeafAuthorityErrorV1, RepositoryPolicyComponentSetV1, RepositoryPolicySnapshotV1,
    RepositoryPolicyStrengthV1, RepositoryPolicyTransitionAuthorityV1,
    RepositoryPolicyTransitionV1, ResolveDecisionAuthorityV1, ResponseOriginV1,
    SubmitWorkCompletionAuthorityV1, admit_repository_action,
};
use crate::domain::contract::{
    component_kind::ContractComponentKindV1,
    finalization::DesignFinalizationManifestV1,
    root::CandidateContractRootV1,
    runtime::{
        ContractAmendmentPreparationV1, ContractGenerationV1, ContractPublicationAuthorityV1,
        ContractPublicationKindV1, ContractPublicationRequestV1, ContractRevisionV1,
        ContractRuntimeError, InitialContractStepPublicationV1, PreparedContractPublicationV1,
    },
};
use crate::domain::design::{
    AdmittedCommittedActionV1, AdmittedMaterializationAuthorityV1, AlternativeIdV1,
    AlternativeRejectionV1, CommittedActionAdmissionErrorV1, DecisionMaterializationPreflightV1,
    DecisionMaterializationV1, DecisionRevisionIdV1, DecisionStateV1, DecisionV1, DecisionV1Error,
    DesignAppendEligibilityV1, DesignRevisionV1, DesignStreamV1, DesignV1Error,
    MaterializationV1Error, WorkDecisionEligibilityV1,
};
use crate::domain::evidence::{
    ClaimError, EvidenceClaimPublicationV1, EvidenceStoreErrorV1, GateAssessmentResolutionV1,
    ValidatedWorkCompletionEvidenceV1, validate_work_completion_evidence,
};
use crate::domain::gate::{GateError, GateSnapshotV1};
use crate::domain::identity::{
    ContractComponentIdV1, ContractRootIdV1, SchemaClosureV1, SchemaIdV1, StoreGenerationIdV1,
    StoreHeadIdV1, StoreObjectIdV1, derive_identity,
};
use crate::domain::persistence::{
    AtomicGenerationPublicationV1, AtomicPublicationError, GenerationError,
    PreparedPublicationError, StoreCompatibilityV1, StoreError, StoreGenerationV1, StoreHeadV1,
    StoreIdempotencyProbeV1, StoreIdempotencyV1, StoreObjectError, StoreObjectV1,
    StorePublicationOutcomeV1, StorePublicationViewV1, StoreRoleV1, StoreStateV1, StoreV1,
};
use crate::domain::step::{
    AppliedStepAmendmentV1, StepAmendmentError, StepAmendmentPlanV1, StepBindingV1, StepGraphError,
    StepGraphSnapshotV1, StepLifecycleError, StepLifecycleV1, StepOpenBasisV1, StepStateV1,
    StepSubmissionErrorV1, StepSubmissionV1,
};
use crate::domain::work::{
    WorkIdV1, WorkLifecycleError, WorkLifecycleStateV1, WorkRecordV1, WorkRecordWriterV1,
    WorkSubmissionError, WorkSubmissionV1, WorkTransitionReasonV1, WorkTransitionV1,
};
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

const REPOSITORY_ACTION_IDEMPOTENCY_NAMESPACE_V1: &str = "repository.action.v1";
const REPOSITORY_ACTION_REQUEST_DOMAIN_V1: &str = "maestro.vnext.repository-action-request.v1";
const REPOSITORY_WORK_RECORD_DOMAIN_V1: &str = "maestro.vnext.repository-work-record.v1";
const REPOSITORY_DESIGN_STREAM_DOMAIN_V1: &str = "maestro.vnext.repository-design-stream.v1";
const REPOSITORY_CONTRACT_REVISION_DOMAIN_V1: &str =
    "maestro.vnext.repository-contract-revision.v1";
const REPOSITORY_CONTRACT_GENERATION_DOMAIN_V1: &str =
    "maestro.vnext.repository-contract-generation.v1";
const REPOSITORY_FINALIZATION_MANIFEST_DOMAIN_V1: &str =
    "maestro.vnext.repository-design-finalization-manifest.v1";
const REPOSITORY_CONTRACT_ROOT_DOMAIN_V1: &str = "maestro.vnext.repository-contract-root.v1";
const REPOSITORY_DECISION_DOMAIN_V1: &str = "maestro.vnext.repository-decision.v1";
const REPOSITORY_STEP_GRAPH_DOMAIN_V1: &str = "maestro.vnext.repository-step-graph.v1";
const REPOSITORY_STEP_STATE_DOMAIN_V1: &str = "maestro.vnext.repository-step-state.v1";
const REPOSITORY_STEP_AMENDMENT_AUDIT_DOMAIN_V1: &str =
    "maestro.vnext.repository-step-amendment-audit.v1";
const REPOSITORY_DECISION_MATERIALIZATION_AUDIT_DOMAIN_V1: &str =
    "maestro.vnext.repository-decision-materialization-audit.v1";
const REPOSITORY_EXACT_EQUIVALENCE_RECEIPT_DOMAIN_V1: &str =
    "maestro.vnext.repository-exact-equivalence-receipt.v1";
const REPOSITORY_EXACT_EQUIVALENCE_EVALUATOR_DOMAIN_V1: &str =
    "maestro.vnext.repository-exact-equivalence-evaluator.v1";
const REPOSITORY_EXACT_EQUIVALENCE_PURPOSE_V1: &str = "decision-materialization-already-satisfied";
const REPOSITORY_COMPONENT_INVALIDATION_RECEIPT_DOMAIN_V1: &str =
    "maestro.vnext.repository-component-invalidation-receipt.v1";
const EXECUTION_STEP_SUBMISSION_SCHEMA_V1: &str = "maestro.vnext.step-submission-schema.v1";
const EVIDENCE_CLAIM_SCHEMA_V1: &str = "maestro.vnext.evidence-claim-schema.v1";
const WORK_SUBMISSION_CLAIM_SET_SCHEMA_V1: &str =
    "maestro.vnext.work-submission-claim-set-schema.v1";
const WORK_SUBMISSION_SCHEMA_V1: &str = "maestro.vnext.work-submission-schema.v1";
const WORK_COMPLETION_EVIDENCE_BASIS_SCHEMA_V1: &str =
    "maestro.vnext.work-completion-evidence-basis-schema.v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepositoryStoreSchemaV1 {
    ActionRequest,
    WorkRecord,
    DesignStream,
    ContractRevision,
    ContractGeneration,
    DesignFinalizationManifest,
    ContractRoot,
    Decision,
    StepGraph,
    StepState,
    StepAmendmentAudit,
    DecisionMaterializationAudit,
    ExactEquivalenceReceipt,
    ComponentInvalidationReceipt,
}

impl RepositoryStoreSchemaV1 {
    pub const ALL: [Self; 14] = [
        Self::ActionRequest,
        Self::WorkRecord,
        Self::DesignStream,
        Self::ContractRevision,
        Self::ContractGeneration,
        Self::DesignFinalizationManifest,
        Self::ContractRoot,
        Self::Decision,
        Self::StepGraph,
        Self::StepState,
        Self::StepAmendmentAudit,
        Self::DecisionMaterializationAudit,
        Self::ExactEquivalenceReceipt,
        Self::ComponentInvalidationReceipt,
    ];

    pub const fn domain(self) -> &'static str {
        match self {
            Self::ActionRequest => REPOSITORY_ACTION_REQUEST_DOMAIN_V1,
            Self::WorkRecord => REPOSITORY_WORK_RECORD_DOMAIN_V1,
            Self::DesignStream => REPOSITORY_DESIGN_STREAM_DOMAIN_V1,
            Self::ContractRevision => REPOSITORY_CONTRACT_REVISION_DOMAIN_V1,
            Self::ContractGeneration => REPOSITORY_CONTRACT_GENERATION_DOMAIN_V1,
            Self::DesignFinalizationManifest => REPOSITORY_FINALIZATION_MANIFEST_DOMAIN_V1,
            Self::ContractRoot => REPOSITORY_CONTRACT_ROOT_DOMAIN_V1,
            Self::Decision => REPOSITORY_DECISION_DOMAIN_V1,
            Self::StepGraph => REPOSITORY_STEP_GRAPH_DOMAIN_V1,
            Self::StepState => REPOSITORY_STEP_STATE_DOMAIN_V1,
            Self::StepAmendmentAudit => REPOSITORY_STEP_AMENDMENT_AUDIT_DOMAIN_V1,
            Self::DecisionMaterializationAudit => {
                REPOSITORY_DECISION_MATERIALIZATION_AUDIT_DOMAIN_V1
            }
            Self::ExactEquivalenceReceipt => REPOSITORY_EXACT_EQUIVALENCE_RECEIPT_DOMAIN_V1,
            Self::ComponentInvalidationReceipt => {
                REPOSITORY_COMPONENT_INVALIDATION_RECEIPT_DOMAIN_V1
            }
        }
    }

    pub fn schema_id(self) -> Result<SchemaIdV1, RepositoryPublicationErrorV1> {
        repository_schema_id(self.domain())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepositoryActionKindV1 {
    CreateDraftWork,
    SubmitWorkCompletion,
    CancelWork,
    AbsorbWork,
    PublishInitialContract,
    AmendContract,
    AppendDesignRevision,
    ResolveDecision,
}

impl RepositoryActionKindV1 {
    pub const ALL: [Self; 8] = [
        Self::CreateDraftWork,
        Self::SubmitWorkCompletion,
        Self::CancelWork,
        Self::AbsorbWork,
        Self::PublishInitialContract,
        Self::AmendContract,
        Self::AppendDesignRevision,
        Self::ResolveDecision,
    ];

    pub const fn authority_leaf(self) -> RepositoryActionLeafV1 {
        match self {
            Self::CreateDraftWork => RepositoryActionLeafV1::CreateDraftWork,
            Self::SubmitWorkCompletion => RepositoryActionLeafV1::SubmitWorkCompletion,
            Self::CancelWork => RepositoryActionLeafV1::CancelWork,
            Self::AbsorbWork => RepositoryActionLeafV1::AbsorbWork,
            Self::PublishInitialContract => RepositoryActionLeafV1::PublishInitialContract,
            Self::AmendContract => RepositoryActionLeafV1::AmendContract,
            Self::AppendDesignRevision => RepositoryActionLeafV1::AppendDesignRevision,
            Self::ResolveDecision => RepositoryActionLeafV1::ResolveDecision,
        }
    }

    const fn tag(self) -> u64 {
        self.authority_leaf().global_tag()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RepositoryStoreBasisV1 {
    expected_head_id: StoreHeadIdV1,
    expected_generation_id: StoreGenerationIdV1,
    expected_generation_ordinal: u64,
    expected_contract_root_id: ContractRootIdV1,
}

impl RepositoryStoreBasisV1 {
    pub fn new(
        expected_head_id: StoreHeadIdV1,
        expected_generation_id: StoreGenerationIdV1,
        expected_generation_ordinal: u64,
        expected_contract_root_id: ContractRootIdV1,
    ) -> Result<Self, RepositoryPublicationErrorV1> {
        if expected_generation_ordinal == 0 {
            return Err(RepositoryPublicationErrorV1::InvalidStoreBasis);
        }
        Ok(Self {
            expected_head_id,
            expected_generation_id,
            expected_generation_ordinal,
            expected_contract_root_id,
        })
    }

    pub const fn expected_head_id(self) -> StoreHeadIdV1 {
        self.expected_head_id
    }

    pub const fn expected_generation_id(self) -> StoreGenerationIdV1 {
        self.expected_generation_id
    }

    pub const fn expected_generation_ordinal(self) -> u64 {
        self.expected_generation_ordinal
    }

    pub const fn expected_contract_root_id(self) -> ContractRootIdV1 {
        self.expected_contract_root_id
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RepositoryActionIdentityV1 {
    request_id: ActionRequestIdV1,
    idempotency_key: IdempotencyKeyIdV1,
}

impl RepositoryActionIdentityV1 {
    pub const fn new(request_id: ActionRequestIdV1, idempotency_key: IdempotencyKeyIdV1) -> Self {
        Self {
            request_id,
            idempotency_key,
        }
    }

    pub const fn request_id(self) -> ActionRequestIdV1 {
        self.request_id
    }

    pub const fn idempotency_key(self) -> IdempotencyKeyIdV1 {
        self.idempotency_key
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateDraftWorkPublicationV1 {
    plan: CreateDraftWorkPlanV1,
}

impl CreateDraftWorkPublicationV1 {
    pub fn new(
        identity: RepositoryActionIdentityV1,
        store_basis: RepositoryStoreBasisV1,
        authority: RepositoryAuthoritySelectionV1,
        work_id: WorkIdV1,
    ) -> Result<Self, RepositoryPublicationErrorV1> {
        let work = WorkRecordV1::create_draft(WorkRecordWriterV1::Work, work_id)?;
        let work_object = work_record_object(&work)?;
        let subject_commitment = work_subject_commitment(work_id)?;
        let subject_basis_commitment = hash(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.repository-work-creation-basis.v1")?,
            bytes(work_id.as_bytes()),
            bytes(work_object.id().as_bytes()),
        ]))?;
        let request_object = action_request_object(
            RepositoryActionKindV1::CreateDraftWork,
            identity,
            store_basis,
            authority,
            subject_commitment,
            subject_basis_commitment,
            CborValue::Array(vec![
                bytes(work_id.as_bytes()),
                bytes(work_object.id().as_bytes()),
            ]),
        )?;
        let meaning_digest = Sha256::digest(request_object.canonical_bytes()).into();
        Ok(Self {
            plan: CreateDraftWorkPlanV1 {
                identity,
                store_basis,
                authority,
                work,
                work_object,
                request_object,
                subject_commitment,
                subject_basis_commitment,
                meaning_digest,
            },
        })
    }

    pub fn work(&self) -> &WorkRecordV1 {
        &self.plan.work
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CreateDraftWorkPlanV1 {
    identity: RepositoryActionIdentityV1,
    store_basis: RepositoryStoreBasisV1,
    authority: RepositoryAuthoritySelectionV1,
    work: WorkRecordV1,
    work_object: StoreObjectV1,
    request_object: StoreObjectV1,
    subject_commitment: [u8; 32],
    subject_basis_commitment: [u8; 32],
    meaning_digest: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CancelWorkPublicationV1 {
    plan: WorkMutationPublicationV1,
}

impl CancelWorkPublicationV1 {
    pub fn new(
        identity: RepositoryActionIdentityV1,
        store_basis: RepositoryStoreBasisV1,
        authority: RepositoryAuthoritySelectionV1,
        current: WorkRecordV1,
        reason: WorkTransitionReasonV1,
    ) -> Result<Self, RepositoryPublicationErrorV1> {
        Ok(Self {
            plan: WorkMutationPublicationV1::new(
                RepositoryActionKindV1::CancelWork,
                identity,
                store_basis,
                authority,
                current,
                WorkTransitionV1::CancelWork { reason },
            )?,
        })
    }

    pub fn successor(&self) -> &WorkRecordV1 {
        &self.plan.successor
    }
}

#[derive(Clone, Debug)]
pub struct SubmitWorkCompletionPublicationV1 {
    identity: RepositoryActionIdentityV1,
    store_basis: RepositoryStoreBasisV1,
    authority: RepositoryAuthoritySelectionV1,
    current_work: WorkRecordV1,
    successor_work: WorkRecordV1,
    current_generation: ContractGenerationV1,
    current_root: CandidateContractRootV1,
    current_step_graph: StepGraphSnapshotV1,
    current_step_states: Vec<StepStateV1>,
    current_step_submissions: Vec<StepSubmissionV1>,
    submission: WorkSubmissionV1,
    evidence: EvidenceClaimPublicationV1,
    gate_snapshot: GateSnapshotV1,
    gate_resolutions: Vec<GateAssessmentResolutionV1>,
    as_of: u64,
    evidence_basis_value: CborValue,
    current_work_object: StoreObjectV1,
    successor_work_object: StoreObjectV1,
    current_generation_object: StoreObjectV1,
    current_root_object: StoreObjectV1,
    current_step_graph_object: StoreObjectV1,
    current_step_state_objects: Vec<StoreObjectV1>,
    request_object: StoreObjectV1,
    subject_commitment: [u8; 32],
    subject_basis_commitment: [u8; 32],
    meaning_digest: [u8; 32],
}

impl SubmitWorkCompletionPublicationV1 {
    #[expect(
        clippy::too_many_arguments,
        reason = "Work completion closes over the exact Work, Contract Generation/root, Step graph/state partition, Step Submissions, and authority basis"
    )]
    pub fn new(
        identity: RepositoryActionIdentityV1,
        store_basis: RepositoryStoreBasisV1,
        authority: RepositoryAuthoritySelectionV1,
        current_work: WorkRecordV1,
        current_generation: ContractGenerationV1,
        current_root: CandidateContractRootV1,
        current_step_graph: StepGraphSnapshotV1,
        current_step_states: Vec<StepStateV1>,
        mut current_step_submissions: Vec<StepSubmissionV1>,
        submission: WorkSubmissionV1,
        evidence: EvidenceClaimPublicationV1,
        gate_snapshot: GateSnapshotV1,
        mut gate_resolutions: Vec<GateAssessmentResolutionV1>,
        as_of: u64,
    ) -> Result<Self, RepositoryPublicationErrorV1> {
        current_step_submissions.sort_by_key(|item| item.binding());
        gate_resolutions.sort_unstable_by_key(GateAssessmentResolutionV1::gate_id);
        let current_gate_component_id = current_root
            .components()
            .iter()
            .find(|component| component.kind() == ContractComponentKindV1::GateSnapshot)
            .map(|component| component.component_id())
            .copied()
            .ok_or(RepositoryPublicationErrorV1::WorkCompletionBasisMismatch)?;
        if !validate_current_work_completion_basis(
            store_basis,
            &current_work,
            &current_generation,
            &current_root,
            &current_step_graph,
            &current_step_states,
            &current_step_submissions,
            &submission,
        ) || evidence.claim_set().submission_ref()
            != Some(crate::domain::evidence::SubmissionRefV1::Work(
                submission.id(),
            ))
            || evidence.claim_set().digest() != submission.claim_set().digest()
            || gate_snapshot.work_id() != current_work.id()
            || gate_snapshot.contract_generation_id() != current_generation.id()
            || gate_snapshot.contract_root_id() != current_generation.root_id()
            || gate_snapshot.contract_component_id() != current_gate_component_id
            || as_of == 0
            || gate_resolutions.is_empty()
            || gate_resolutions
                .windows(2)
                .any(|pair| pair[0].gate_id() == pair[1].gate_id())
            || gate_resolutions.iter().any(|resolution| {
                resolution.snapshot_id() != gate_snapshot.id() || resolution.as_of() != as_of
            })
        {
            return Err(RepositoryPublicationErrorV1::WorkCompletionBasisMismatch);
        }
        let successor_work = current_work.apply_verified_completion(
            WorkRecordWriterV1::Work,
            current_work.revision(),
            submission.clone(),
        )?;
        let current_work_object = work_record_object(&current_work)?;
        let successor_work_object = work_record_object(&successor_work)?;
        let current_generation_object = contract_generation_object(&current_generation)?;
        let current_root_object = contract_root_object(&current_root)?;
        let current_step_graph_object = step_graph_object(&current_step_graph)?;
        let current_step_state_objects = current_step_states
            .iter()
            .map(step_state_object)
            .collect::<Result<Vec<_>, _>>()?;
        let evidence_basis_value = work_completion_evidence_basis_value(
            &gate_snapshot,
            &gate_resolutions,
            &evidence,
            as_of,
        )?;
        let evidence_basis_commitment = hash(&evidence_basis_value)?;
        let subject_commitment = work_subject_commitment(current_work.id())?;
        let subject_basis_commitment = hash(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.repository-work-completion-basis.v1")?,
            bytes(current_work_object.id().as_bytes()),
            bytes(current_generation_object.id().as_bytes()),
            bytes(current_root_object.id().as_bytes()),
            bytes(current_step_graph_object.id().as_bytes()),
            CborValue::Array(
                current_step_state_objects
                    .iter()
                    .map(|object| bytes(object.id().as_bytes()))
                    .collect(),
            ),
            CborValue::Array(
                current_step_submissions
                    .iter()
                    .map(|item| bytes(item.record_hash().as_slice()))
                    .collect(),
            ),
            bytes(submission.digest()),
            bytes(&evidence_basis_commitment),
        ]))?;
        let request_object = action_request_object(
            RepositoryActionKindV1::SubmitWorkCompletion,
            identity,
            store_basis,
            authority,
            subject_commitment,
            subject_basis_commitment,
            CborValue::Array(vec![
                bytes(current_work_object.id().as_bytes()),
                bytes(successor_work_object.id().as_bytes()),
                bytes(current_generation.id().as_bytes()),
                bytes(current_generation.root_id().as_bytes()),
                bytes(submission.digest()),
                bytes(&evidence_basis_commitment),
                CborValue::Unsigned(as_of),
            ]),
        )?;
        let meaning_digest = Sha256::digest(request_object.canonical_bytes()).into();
        Ok(Self {
            identity,
            store_basis,
            authority,
            current_work,
            successor_work,
            current_generation,
            current_root,
            current_step_graph,
            current_step_states,
            current_step_submissions,
            submission,
            evidence,
            gate_snapshot,
            gate_resolutions,
            as_of,
            evidence_basis_value,
            current_work_object,
            successor_work_object,
            current_generation_object,
            current_root_object,
            current_step_graph_object,
            current_step_state_objects,
            request_object,
            subject_commitment,
            subject_basis_commitment,
            meaning_digest,
        })
    }

    pub fn successor(&self) -> &WorkRecordV1 {
        &self.successor_work
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WorkMutationPublicationV1 {
    kind: RepositoryActionKindV1,
    identity: RepositoryActionIdentityV1,
    store_basis: RepositoryStoreBasisV1,
    authority: RepositoryAuthoritySelectionV1,
    current: WorkRecordV1,
    transition: WorkTransitionV1,
    successor: WorkRecordV1,
    current_object: StoreObjectV1,
    successor_object: StoreObjectV1,
    request_object: StoreObjectV1,
    subject_commitment: [u8; 32],
    subject_basis_commitment: [u8; 32],
    meaning_digest: [u8; 32],
}

impl WorkMutationPublicationV1 {
    fn new(
        kind: RepositoryActionKindV1,
        identity: RepositoryActionIdentityV1,
        store_basis: RepositoryStoreBasisV1,
        authority: RepositoryAuthoritySelectionV1,
        current: WorkRecordV1,
        transition: WorkTransitionV1,
    ) -> Result<Self, RepositoryPublicationErrorV1> {
        if !matches!(
            (kind, &transition),
            (
                RepositoryActionKindV1::CancelWork,
                WorkTransitionV1::CancelWork { .. }
            )
        ) {
            return Err(RepositoryPublicationErrorV1::UnsupportedRepositoryAction);
        }
        let successor = current.apply(
            WorkRecordWriterV1::Work,
            current.revision(),
            transition.clone(),
        )?;
        let current_object = work_record_object(&current)?;
        let successor_object = work_record_object(&successor)?;
        let subject_commitment = hash(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.repository-work-subject.v1")?,
            bytes(current.id().as_bytes()),
        ]))?;
        let subject_basis_commitment = hash(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.repository-work-revision-basis.v1")?,
            bytes(current.id().as_bytes()),
            CborValue::Unsigned(current.revision().get()),
            bytes(current_object.id().as_bytes()),
        ]))?;
        let request_object = action_request_object(
            kind,
            identity,
            store_basis,
            authority,
            subject_commitment,
            subject_basis_commitment,
            CborValue::Array(vec![
                bytes(current.id().as_bytes()),
                CborValue::Unsigned(current.revision().get()),
                CborValue::Unsigned(transition.kind().tag()),
                bytes(successor_object.id().as_bytes()),
            ]),
        )?;
        let meaning_digest = Sha256::digest(request_object.canonical_bytes()).into();
        Ok(Self {
            kind,
            identity,
            store_basis,
            authority,
            current,
            transition,
            successor,
            current_object,
            successor_object,
            request_object,
            subject_commitment,
            subject_basis_commitment,
            meaning_digest,
        })
    }
}

// Stage 4/5 publication handlers intentionally remain out of the Stage 3 build.

#[derive(Clone, Debug)]
pub struct InitialContractPublicationV1 {
    store_basis: RepositoryStoreBasisV1,
    authority: PublishInitialContractAuthorityV1,
    current_work: WorkRecordV1,
    successor_work: WorkRecordV1,
    request: ContractPublicationRequestV1,
    prepared: PreparedContractPublicationV1,
    candidate_root: CandidateContractRootV1,
    steps: InitialContractStepPublicationV1,
    current_work_object: StoreObjectV1,
    successor_work_object: StoreObjectV1,
    revision_object: StoreObjectV1,
    finalization_object: StoreObjectV1,
    candidate_root_object: StoreObjectV1,
    step_graph_object: StoreObjectV1,
    step_state_objects: Vec<StoreObjectV1>,
    request_object: StoreObjectV1,
    meaning_digest: [u8; 32],
}

impl InitialContractPublicationV1 {
    #[expect(
        clippy::too_many_arguments,
        reason = "initial Contract publication closes over the exact Work, Revision, finalization, root, and typed Step publication"
    )]
    pub fn new(
        store_basis: RepositoryStoreBasisV1,
        authority: RepositoryAuthoritySelectionV1,
        authenticated_human: RepositoryAuthenticatedHumanV1,
        authority_nonce: [u8; 32],
        authority_expires_at: u64,
        current_work: WorkRecordV1,
        request: ContractPublicationRequestV1,
        revision: ContractRevisionV1,
        finalization: DesignFinalizationManifestV1,
        candidate_root: CandidateContractRootV1,
        steps: InitialContractStepPublicationV1,
    ) -> Result<Self, RepositoryPublicationErrorV1> {
        if request.kind() != ContractPublicationKindV1::Initial
            || request.work_id() != current_work.id()
            || request.candidate_revision_id() != revision.id()
            || request.finalization_manifest_id() != *finalization.manifest_id()
            || request.candidate_root_id() != *candidate_root.root_id()
            || revision.finalization().manifest_id() != *finalization.manifest_id()
            || revision.finalization().candidate_root_id() != *candidate_root.root_id()
            || steps.graph().scope().work_id() != current_work.id()
            || steps.graph().contract_root_id() != *candidate_root.root_id()
        {
            return Err(RepositoryPublicationErrorV1::ContractPublicationBasisMismatch);
        }
        let prepared = PreparedContractPublicationV1::initial(request.clone(), revision.clone())?;
        if steps.graph().contract_generation_id() != prepared.predicted_generation_id()? {
            return Err(RepositoryPublicationErrorV1::ContractStepPublicationMismatch);
        }
        let successor_work = current_work.apply(
            WorkRecordWriterV1::Work,
            current_work.revision(),
            WorkTransitionV1::PublishInitialContract,
        )?;
        let current_work_object = work_record_object(&current_work)?;
        let successor_work_object = work_record_object(&successor_work)?;
        let revision_object = contract_revision_object(&revision)?;
        let finalization_object = finalization_manifest_object(&finalization)?;
        let candidate_root_object = contract_root_object(&candidate_root)?;
        let step_graph_object = step_graph_object(steps.graph())?;
        let step_state_objects = steps
            .step_states()
            .iter()
            .map(step_state_object)
            .collect::<Result<Vec<_>, _>>()?;
        let subject_commitment = work_subject_commitment(current_work.id())?;
        let contract_basis_commitment = contract_publication_basis_commitment(
            &current_work_object,
            None,
            None,
            &revision_object,
            &finalization_object,
            &candidate_root_object,
        )?;
        let subject_basis_commitment = hash(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.repository-initial-contract-step-basis.v1")?,
            bytes(&contract_basis_commitment),
            bytes(successor_work_object.id().as_bytes()),
            bytes(step_graph_object.id().as_bytes()),
            CborValue::Array(
                step_state_objects
                    .iter()
                    .map(|object| bytes(object.id().as_bytes()))
                    .collect(),
            ),
        ]))?;
        let policy_transition =
            RepositoryPolicyTransitionV1::initial(repository_policy_snapshot(&candidate_root)?)?;
        let transition_authority = RepositoryPolicyTransitionAuthorityV1::new(
            subject_commitment,
            subject_basis_commitment,
            &policy_transition,
            authenticated_human,
            authority_nonce,
            authority_expires_at,
        )?;
        let authority = PublishInitialContractAuthorityV1::new(
            authority,
            subject_commitment,
            subject_basis_commitment,
            policy_transition,
            transition_authority,
        )?;
        let identity =
            RepositoryActionIdentityV1::new(request.request_id(), request.idempotency_key_id());
        let request_object = action_request_object(
            RepositoryActionKindV1::PublishInitialContract,
            identity,
            store_basis,
            authority.selection(),
            subject_commitment,
            subject_basis_commitment,
            CborValue::Array(vec![
                CborValue::Bytes(request.canonical_bytes()?),
                bytes(current_work_object.id().as_bytes()),
                bytes(successor_work_object.id().as_bytes()),
                bytes(revision_object.id().as_bytes()),
                bytes(finalization_object.id().as_bytes()),
                bytes(candidate_root_object.id().as_bytes()),
                bytes(step_graph_object.id().as_bytes()),
                CborValue::Array(
                    step_state_objects
                        .iter()
                        .map(|object| bytes(object.id().as_bytes()))
                        .collect(),
                ),
            ]),
        )?;
        let meaning_digest = Sha256::digest(request_object.canonical_bytes()).into();
        Ok(Self {
            store_basis,
            authority,
            current_work,
            successor_work,
            request,
            prepared,
            candidate_root,
            steps,
            current_work_object,
            successor_work_object,
            revision_object,
            finalization_object,
            candidate_root_object,
            step_graph_object,
            step_state_objects,
            request_object,
            meaning_digest,
        })
    }
}

#[derive(Debug)]
pub struct ContractAmendmentPublicationV1 {
    store_basis: RepositoryStoreBasisV1,
    authority: AmendContractAuthorityV1,
    current_work: WorkRecordV1,
    successor_work: WorkRecordV1,
    current_generation: ContractGenerationV1,
    current_root: CandidateContractRootV1,
    request: ContractPublicationRequestV1,
    prepared: PreparedContractPublicationV1,
    candidate_root: CandidateContractRootV1,
    step_plan: StepAmendmentPlanV1,
    current_step_graph: StepGraphSnapshotV1,
    candidate_step_graph: StepGraphSnapshotV1,
    current_step_states: Vec<StepStateV1>,
    materialization_audits: Vec<DecisionMaterializationCandidateV1>,
    current_work_object: StoreObjectV1,
    successor_work_object: StoreObjectV1,
    current_generation_object: StoreObjectV1,
    current_root_object: StoreObjectV1,
    revision_object: StoreObjectV1,
    finalization_object: StoreObjectV1,
    candidate_root_object: StoreObjectV1,
    current_step_graph_object: StoreObjectV1,
    candidate_step_graph_object: StoreObjectV1,
    current_step_state_objects: Vec<StoreObjectV1>,
    request_object: StoreObjectV1,
    meaning_digest: [u8; 32],
}

#[derive(Debug)]
pub enum ContractAmendmentRepositoryPreparationV1 {
    NoOp(Box<ContractAmendmentNoOpPublicationV1>),
    Required(Box<ContractAmendmentPublicationV1>),
}

#[derive(Clone, Debug)]
pub struct ContractAmendmentNoOpPublicationV1 {
    no_op: crate::domain::contract::runtime::ContractAmendmentNoOpV1,
    store_basis: RepositoryStoreBasisV1,
    current_step_graph: StepGraphSnapshotV1,
    current_work_object: StoreObjectV1,
    current_generation_object: StoreObjectV1,
    current_root_object: StoreObjectV1,
    current_step_graph_object: StoreObjectV1,
    current_step_state_objects: Vec<StoreObjectV1>,
}

impl ContractAmendmentNoOpPublicationV1 {
    pub const fn no_op(&self) -> crate::domain::contract::runtime::ContractAmendmentNoOpV1 {
        self.no_op
    }
}

#[derive(Clone, Debug)]
pub struct DecisionResolutionPublicationV1 {
    identity: RepositoryActionIdentityV1,
    store_basis: RepositoryStoreBasisV1,
    authority: ResolveDecisionAuthorityV1,
    current_work: WorkRecordV1,
    current_decision: DecisionV1,
    expected_head: DecisionRevisionIdV1,
    selected_alternative: AlternativeIdV1,
    rationale: Vec<u8>,
    rejected_alternatives: Vec<AlternativeRejectionV1>,
    current_work_object: StoreObjectV1,
    current_decision_object: StoreObjectV1,
    request_object: StoreObjectV1,
    meaning_digest: [u8; 32],
}

impl DecisionResolutionPublicationV1 {
    #[expect(
        clippy::too_many_arguments,
        reason = "Decision resolution commits the exact open Decision, Work eligibility, selection, and rejection reasons"
    )]
    pub fn new(
        identity: RepositoryActionIdentityV1,
        store_basis: RepositoryStoreBasisV1,
        authority: RepositoryAuthoritySelectionV1,
        authenticated_human: RepositoryAuthenticatedHumanV1,
        authority_nonce: [u8; 32],
        authority_expires_at: u64,
        current_work: WorkRecordV1,
        current_decision: DecisionV1,
        expected_head: DecisionRevisionIdV1,
        selected_alternative: AlternativeIdV1,
        rationale: Vec<u8>,
        rejected_alternatives: Vec<AlternativeRejectionV1>,
    ) -> Result<Self, RepositoryPublicationErrorV1> {
        if current_work.state().is_terminal()
            || current_decision.work_id() != current_work.id()
            || !matches!(current_decision.state(), DecisionStateV1::Open)
            || current_decision.head().revision_id() != &expected_head
        {
            return Err(RepositoryPublicationErrorV1::DecisionResolutionBasisMismatch);
        }
        let current_work_object = work_record_object(&current_work)?;
        let current_decision_object = decision_object(&current_decision)?;
        let subject_commitment = hash(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.repository-decision-subject.v1")?,
            bytes(current_work.id().as_bytes()),
            CborValue::Text(current_decision.decision_id().as_str().to_owned()),
        ]))?;
        let subject_basis_commitment = hash(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.repository-decision-resolution-basis.v1")?,
            bytes(current_work_object.id().as_bytes()),
            bytes(current_decision_object.id().as_bytes()),
            bytes(expected_head.as_bytes()),
        ]))?;
        let presentation = RepositoryDecisionPresentationV1::new(
            current_decision.decision_id().as_str(),
            *expected_head.as_bytes(),
            current_decision.head().question(),
            current_decision
                .head()
                .alternatives()
                .iter()
                .map(|alternative| {
                    RepositoryDecisionOptionMappingV1::new(
                        *alternative.alternative_id().as_bytes(),
                        alternative.preview(),
                    )
                })
                .collect::<Result<Vec<_>, _>>()?,
            *selected_alternative.as_bytes(),
        )?;
        let carrier = RepositoryDecisionAuthorityCarrierV1::new(
            subject_commitment,
            subject_basis_commitment,
            &presentation,
            authenticated_human,
            authority_nonce,
            authority_expires_at,
        )?;
        let authority = ResolveDecisionAuthorityV1::new(
            authority,
            subject_commitment,
            subject_basis_commitment,
            presentation,
            carrier,
        )?;
        let request_object = action_request_object(
            RepositoryActionKindV1::ResolveDecision,
            identity,
            store_basis,
            authority.selection(),
            subject_commitment,
            subject_basis_commitment,
            CborValue::Array(vec![
                CborValue::Text(current_decision.decision_id().as_str().to_owned()),
                bytes(expected_head.as_bytes()),
                bytes(selected_alternative.as_bytes()),
                CborValue::Bytes(rationale.clone()),
                CborValue::Array(
                    rejected_alternatives
                        .iter()
                        .map(|rejection| {
                            CborValue::Array(vec![
                                bytes(rejection.alternative_id().as_bytes()),
                                CborValue::Bytes(rejection.reason().to_vec()),
                            ])
                        })
                        .collect(),
                ),
            ]),
        )?;
        let meaning_digest = Sha256::digest(request_object.canonical_bytes()).into();
        Ok(Self {
            identity,
            store_basis,
            authority,
            current_work,
            current_decision,
            expected_head,
            selected_alternative,
            rationale,
            rejected_alternatives,
            current_work_object,
            current_decision_object,
            request_object,
            meaning_digest,
        })
    }
}

#[derive(Clone, Debug)]
pub struct DecisionResolutionPublicationOutcomeV1 {
    publication: RepositoryPublicationOutcomeV1,
    resolved_decision: DecisionV1,
}

impl DecisionResolutionPublicationOutcomeV1 {
    pub const fn publication(&self) -> &RepositoryPublicationOutcomeV1 {
        &self.publication
    }

    pub const fn resolved_decision(&self) -> &DecisionV1 {
        &self.resolved_decision
    }
}

#[derive(Debug)]
pub enum DecisionMaterializationRepositoryPreparationV1 {
    EqualRoot(Box<DecisionMaterializationNoOpPublicationV1>),
    Candidate(Box<DecisionMaterializationCandidateV1>),
}

#[derive(Debug)]
pub struct DecisionMaterializationNoOpPublicationV1 {
    materialization: DecisionMaterializationV1,
    store_basis: RepositoryStoreBasisV1,
    current_work: WorkRecordV1,
    current_work_object: StoreObjectV1,
    decision_object: StoreObjectV1,
    base_root_object: StoreObjectV1,
}

impl DecisionMaterializationNoOpPublicationV1 {
    pub const fn materialization(&self) -> &DecisionMaterializationV1 {
        &self.materialization
    }
}

#[derive(Debug)]
pub struct DecisionMaterializationCandidateV1 {
    store_basis: RepositoryStoreBasisV1,
    current_work: WorkRecordV1,
    current_work_object: StoreObjectV1,
    resolved_decision: DecisionV1,
    preflight: DecisionMaterializationPreflightV1,
    base_root_object: StoreObjectV1,
    candidate_root_object: StoreObjectV1,
    decision_object: StoreObjectV1,
    invalidation_receipts: Vec<(ContractComponentIdV1, StoreObjectIdV1)>,
}

impl DecisionMaterializationCandidateV1 {
    pub fn prepare(
        store_basis: RepositoryStoreBasisV1,
        current_work: WorkRecordV1,
        resolved_decision: DecisionV1,
        schemas: &SchemaClosureV1,
        base_root: &CandidateContractRootV1,
        mut invalidation_receipts: Vec<(ContractComponentIdV1, StoreObjectIdV1)>,
    ) -> Result<DecisionMaterializationRepositoryPreparationV1, RepositoryPublicationErrorV1> {
        if current_work.state().is_terminal() || resolved_decision.work_id() != current_work.id() {
            return Err(RepositoryPublicationErrorV1::MaterializationBasisMismatch);
        }
        invalidation_receipts.sort_unstable_by_key(|(component, _)| *component);
        if invalidation_receipts
            .windows(2)
            .any(|pair| pair[0].0 == pair[1].0)
        {
            return Err(RepositoryPublicationErrorV1::MaterializationReceiptMismatch);
        }
        let resolution = resolved_decision
            .resolution()
            .ok_or(RepositoryPublicationErrorV1::MaterializationBasisMismatch)?;
        let decision_object = decision_object(&resolved_decision)?;
        let current_work_object = work_record_object(&current_work)?;
        let base_root_object = contract_root_object(base_root)?;
        let preflight = DecisionMaterializationV1::preflight(resolution, schemas, base_root)?;
        if preflight.is_equal_root() {
            if !invalidation_receipts.is_empty() {
                return Err(RepositoryPublicationErrorV1::MaterializationReceiptMismatch);
            }
            return Ok(DecisionMaterializationRepositoryPreparationV1::EqualRoot(
                Box::new(DecisionMaterializationNoOpPublicationV1 {
                    materialization: preflight.complete_equal_root()?,
                    store_basis,
                    current_work,
                    current_work_object,
                    decision_object,
                    base_root_object,
                }),
            ));
        }
        let candidate_root_object = contract_root_object(preflight.candidate_root())?;
        Ok(DecisionMaterializationRepositoryPreparationV1::Candidate(
            Box::new(Self {
                store_basis,
                current_work,
                current_work_object,
                resolved_decision,
                preflight,
                base_root_object,
                candidate_root_object,
                decision_object,
                invalidation_receipts,
            }),
        ))
    }

    pub fn candidate_root(&self) -> &CandidateContractRootV1 {
        self.preflight.candidate_root()
    }

    pub fn evaluate_exact_equivalence_receipt(
        &self,
    ) -> Result<StoreObjectV1, RepositoryPublicationErrorV1> {
        exact_equivalence_receipt_object(
            &self.resolved_decision,
            &self.preflight,
            &self.decision_object,
            &self.base_root_object,
            &self.candidate_root_object,
        )
    }
}

impl ContractAmendmentPublicationV1 {
    #[expect(
        clippy::too_many_arguments,
        reason = "closed Contract amendment commits exact Work and Contract lineage plus lifecycle cause"
    )]
    pub fn prepare(
        store_basis: RepositoryStoreBasisV1,
        authority: RepositoryAuthoritySelectionV1,
        authenticated_human: RepositoryAuthenticatedHumanV1,
        authority_nonce: [u8; 32],
        authority_expires_at: u64,
        current_work: WorkRecordV1,
        current_generation: ContractGenerationV1,
        current_root: CandidateContractRootV1,
        request: ContractPublicationRequestV1,
        revision: ContractRevisionV1,
        finalization: DesignFinalizationManifestV1,
        candidate_root: CandidateContractRootV1,
        work_transition: Option<WorkTransitionV1>,
        step_plan: Option<StepAmendmentPlanV1>,
        current_step_graph: StepGraphSnapshotV1,
        candidate_step_graph: StepGraphSnapshotV1,
        mut current_step_states: Vec<StepStateV1>,
        materialization_audits: Vec<DecisionMaterializationCandidateV1>,
    ) -> Result<ContractAmendmentRepositoryPreparationV1, RepositoryPublicationErrorV1> {
        let is_contract_no_op = candidate_root.root_id() == current_root.root_id();
        if current_work.state().is_terminal()
            || request.kind() != ContractPublicationKindV1::Amendment
            || request.work_id() != current_work.id()
            || current_generation.work_id() != current_work.id()
            || request.expected_current_generation_id() != Some(current_generation.id())
            || request.expected_current_root_id() != Some(current_generation.root_id())
            || current_generation.root_id() != *current_root.root_id()
            || store_basis.expected_contract_root_id() != *current_root.root_id()
            || request.candidate_revision_id() != revision.id()
            || request.finalization_manifest_id() != *finalization.manifest_id()
            || request.candidate_root_id() != *candidate_root.root_id()
            || current_step_graph.scope().work_id() != current_work.id()
            || candidate_step_graph.scope().work_id() != current_work.id()
            || current_step_graph.contract_generation_id() != current_generation.id()
            || current_step_graph.contract_root_id() != *current_root.root_id()
            || candidate_step_graph.contract_root_id() != *candidate_root.root_id()
            || !validate_current_step_state_set(&current_step_graph, &current_step_states)
            || !validate_contract_amendment_mode(
                is_contract_no_op,
                work_transition.as_ref(),
                step_plan.is_some(),
                materialization_audits.is_empty(),
                candidate_step_graph == current_step_graph,
            )
            || (!is_contract_no_op
                && !validate_materialization_chain(
                    store_basis,
                    &current_work,
                    &current_root,
                    &candidate_root,
                    &materialization_audits,
                ))
        {
            return Err(RepositoryPublicationErrorV1::ContractPublicationBasisMismatch);
        }
        let current_work_object = work_record_object(&current_work)?;
        let current_generation_object = contract_generation_object(&current_generation)?;
        let current_root_object = contract_root_object(&current_root)?;
        let current_step_graph_object = step_graph_object(&current_step_graph)?;
        current_step_states.sort_by_key(StepStateV1::binding);
        let current_step_state_objects = current_step_states
            .iter()
            .map(step_state_object)
            .collect::<Result<Vec<_>, _>>()?;
        let preparation = PreparedContractPublicationV1::amendment(
            &current_generation,
            request.clone(),
            revision.clone(),
        )?;
        let ContractAmendmentPreparationV1::Required(prepared) = preparation else {
            let ContractAmendmentPreparationV1::NoOp(no_op) = preparation else {
                unreachable!("closed Contract amendment preparation variants")
            };
            return Ok(ContractAmendmentRepositoryPreparationV1::NoOp(Box::new(
                ContractAmendmentNoOpPublicationV1 {
                    no_op,
                    store_basis,
                    current_step_graph,
                    current_work_object,
                    current_generation_object,
                    current_root_object,
                    current_step_graph_object,
                    current_step_state_objects,
                },
            )));
        };
        let Some(work_transition) = work_transition else {
            return Err(RepositoryPublicationErrorV1::ContractPublicationBasisMismatch);
        };
        let Some(step_plan) = step_plan else {
            return Err(RepositoryPublicationErrorV1::ContractPublicationBasisMismatch);
        };
        if candidate_step_graph.contract_generation_id() != prepared.predicted_generation_id()? {
            return Err(RepositoryPublicationErrorV1::ContractStepPublicationMismatch);
        }
        let successor_work = current_work.apply(
            WorkRecordWriterV1::Work,
            current_work.revision(),
            work_transition,
        )?;
        let successor_work_object = work_record_object(&successor_work)?;
        let revision_object = contract_revision_object(&revision)?;
        let finalization_object = finalization_manifest_object(&finalization)?;
        let candidate_root_object = contract_root_object(&candidate_root)?;
        let candidate_step_graph_object = step_graph_object(&candidate_step_graph)?;
        let subject_commitment = work_subject_commitment(current_work.id())?;
        let subject_basis_commitment = contract_publication_basis_commitment(
            &current_work_object,
            Some(&current_generation_object),
            Some(&current_root_object),
            &revision_object,
            &finalization_object,
            &candidate_root_object,
        )?;
        let subject_basis_commitment = hash(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.repository-contract-amendment-basis.v1")?,
            bytes(&subject_basis_commitment),
            bytes(current_step_graph_object.id().as_bytes()),
            bytes(candidate_step_graph_object.id().as_bytes()),
            bytes(&hash(&CborValue::Bytes(step_plan.canonical_bytes()?))?),
            CborValue::Array(
                materialization_audits
                    .iter()
                    .map(materialization_candidate_commitment)
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            CborValue::Array(
                current_step_state_objects
                    .iter()
                    .map(|object| bytes(object.id().as_bytes()))
                    .collect(),
            ),
        ]))?;
        let policy_transition = RepositoryPolicyTransitionV1::amendment(
            repository_policy_snapshot(&current_root)?,
            repository_policy_snapshot(&candidate_root)?,
        )?;
        let transition_authority = RepositoryPolicyTransitionAuthorityV1::new(
            subject_commitment,
            subject_basis_commitment,
            &policy_transition,
            authenticated_human,
            authority_nonce,
            authority_expires_at,
        )?;
        let authority = AmendContractAuthorityV1::new(
            authority,
            subject_commitment,
            subject_basis_commitment,
            policy_transition,
            Some(transition_authority),
        )?;
        let identity =
            RepositoryActionIdentityV1::new(request.request_id(), request.idempotency_key_id());
        let request_object = action_request_object(
            RepositoryActionKindV1::AmendContract,
            identity,
            store_basis,
            authority.selection(),
            subject_commitment,
            subject_basis_commitment,
            CborValue::Array(vec![
                CborValue::Bytes(request.canonical_bytes()?),
                bytes(current_work_object.id().as_bytes()),
                bytes(successor_work_object.id().as_bytes()),
                bytes(current_generation_object.id().as_bytes()),
                bytes(current_root_object.id().as_bytes()),
                bytes(revision_object.id().as_bytes()),
                bytes(finalization_object.id().as_bytes()),
                bytes(candidate_root_object.id().as_bytes()),
                bytes(current_step_graph_object.id().as_bytes()),
                bytes(candidate_step_graph_object.id().as_bytes()),
                bytes(&hash(&CborValue::Bytes(step_plan.canonical_bytes()?))?),
                CborValue::Array(
                    current_step_state_objects
                        .iter()
                        .map(|object| bytes(object.id().as_bytes()))
                        .collect(),
                ),
                CborValue::Array(
                    materialization_audits
                        .iter()
                        .map(materialization_candidate_commitment)
                        .collect::<Result<Vec<_>, _>>()?,
                ),
            ]),
        )?;
        let meaning_digest = Sha256::digest(request_object.canonical_bytes()).into();
        Ok(ContractAmendmentRepositoryPreparationV1::Required(
            Box::new(Self {
                store_basis,
                authority,
                current_work,
                successor_work,
                current_generation,
                current_root,
                request,
                prepared: *prepared,
                candidate_root,
                step_plan,
                current_step_graph,
                candidate_step_graph,
                current_step_states,
                materialization_audits,
                current_work_object,
                successor_work_object,
                current_generation_object,
                current_root_object,
                revision_object,
                finalization_object,
                candidate_root_object,
                current_step_graph_object,
                candidate_step_graph_object,
                current_step_state_objects,
                request_object,
                meaning_digest,
            }),
        ))
    }
}

fn validate_contract_amendment_mode(
    is_contract_no_op: bool,
    work_transition: Option<&WorkTransitionV1>,
    has_step_plan: bool,
    materializations_are_empty: bool,
    step_graphs_are_equal: bool,
) -> bool {
    if is_contract_no_op {
        work_transition.is_none()
            && !has_step_plan
            && materializations_are_empty
            && step_graphs_are_equal
    } else {
        matches!(
            work_transition,
            Some(WorkTransitionV1::AmendContract { .. })
        ) && has_step_plan
            && !step_graphs_are_equal
    }
}

fn validate_current_step_state_set(graph: &StepGraphSnapshotV1, states: &[StepStateV1]) -> bool {
    if states.len() != graph.nodes().len() {
        return false;
    }
    let bindings = states
        .iter()
        .map(StepStateV1::binding)
        .collect::<BTreeSet<_>>();
    bindings.len() == states.len()
        && graph
            .nodes()
            .iter()
            .all(|node| bindings.contains(&node.binding()))
}

#[expect(
    clippy::too_many_arguments,
    reason = "the Work completion admission compares every authoritative current basis"
)]
fn validate_current_work_completion_basis(
    store_basis: RepositoryStoreBasisV1,
    current_work: &WorkRecordV1,
    current_generation: &ContractGenerationV1,
    current_root: &CandidateContractRootV1,
    current_step_graph: &StepGraphSnapshotV1,
    current_step_states: &[StepStateV1],
    current_step_submissions: &[StepSubmissionV1],
    submission: &WorkSubmissionV1,
) -> bool {
    if current_work.state() != &WorkLifecycleStateV1::Active
        || current_generation.work_id() != current_work.id()
        || current_generation.root_id() != *current_root.root_id()
        || store_basis.expected_contract_root_id != current_generation.root_id()
        || current_step_graph.scope().work_id() != current_work.id()
        || current_step_graph.contract_generation_id() != current_generation.id()
        || current_step_graph.contract_root_id() != current_generation.root_id()
        || !validate_current_step_state_set(current_step_graph, current_step_states)
        || current_step_submissions.len() != current_step_states.len()
        || current_step_submissions
            .windows(2)
            .any(|pair| pair[0].binding() == pair[1].binding())
        || submission.work_id() != current_work.id()
        || submission.contract_root() != current_generation.root_id()
        || submission.expected_work_revision() != current_work.revision().get()
    {
        return false;
    }
    for state in current_step_states {
        let StepLifecycleV1::Satisfied {
            submission_record_hash,
            ..
        } = state.lifecycle()
        else {
            return false;
        };
        let matching = current_step_submissions
            .iter()
            .filter(|candidate| candidate.binding() == state.binding())
            .collect::<Vec<_>>();
        let [matching] = matching.as_slice() else {
            return false;
        };
        if matching.record_hash() != submission_record_hash {
            return false;
        }
    }
    let mut expected_submission_ids = current_step_submissions
        .iter()
        .map(StepSubmissionV1::id)
        .collect::<Vec<_>>();
    expected_submission_ids.sort_unstable();
    submission.current_step_submissions() == expected_submission_ids
}

fn work_completion_evidence_basis_value(
    gate_snapshot: &GateSnapshotV1,
    resolutions: &[GateAssessmentResolutionV1],
    evidence: &EvidenceClaimPublicationV1,
    as_of: u64,
) -> Result<CborValue, RepositoryPublicationErrorV1> {
    Ok(CborValue::Array(vec![
        CborValue::text("maestro.vnext.work-completion-evidence-basis.v1")?,
        bytes(gate_snapshot.id().as_bytes()),
        CborValue::Bytes(gate_snapshot.canonical_bytes()?),
        CborValue::Unsigned(as_of),
        evidence.claim_set_value()?,
        CborValue::Array(
            evidence
                .claims()
                .iter()
                .map(|claim| bytes(claim.claim_id().as_bytes()))
                .collect(),
        ),
        CborValue::Array(
            evidence
                .observations()
                .iter()
                .map(|observation| bytes(observation.id().as_bytes()))
                .collect(),
        ),
        CborValue::Array(
            resolutions
                .iter()
                .map(GateAssessmentResolutionV1::canonical_value)
                .collect(),
        ),
    ]))
}

fn work_completion_claim_objects(
    evidence: &EvidenceClaimPublicationV1,
) -> Result<Vec<StoreObjectV1>, RepositoryPublicationErrorV1> {
    let schema = derive_identity(&CborValue::Text(EVIDENCE_CLAIM_SCHEMA_V1.to_owned()))?;
    evidence
        .claims()
        .iter()
        .map(|claim| Ok(StoreObjectV1::new(schema, claim.canonical_value(), vec![])?))
        .collect()
}

fn work_submission_claim_set_object(
    evidence: &EvidenceClaimPublicationV1,
    claims: &[StoreObjectV1],
    observations: &[StoreObjectV1],
) -> Result<StoreObjectV1, RepositoryPublicationErrorV1> {
    let mut references = claims
        .iter()
        .chain(observations)
        .map(StoreObjectV1::id)
        .collect::<Vec<_>>();
    references.sort_unstable();
    references.dedup();
    Ok(StoreObjectV1::new(
        derive_identity(&CborValue::Text(
            WORK_SUBMISSION_CLAIM_SET_SCHEMA_V1.to_owned(),
        ))?,
        CborValue::Array(vec![
            CborValue::text("maestro.vnext.work-submission-claim-set.v1")?,
            evidence.claim_set_value()?,
        ]),
        references,
    )?)
}

fn work_completion_evidence_basis_object(
    value: CborValue,
    validated: &ValidatedWorkCompletionEvidenceV1,
) -> Result<StoreObjectV1, RepositoryPublicationErrorV1> {
    let mut references = vec![
        validated.gate_snapshot_object().id(),
        validated.evidence_index_object().id(),
    ];
    references.extend(
        validated
            .observation_objects()
            .iter()
            .map(StoreObjectV1::id),
    );
    references.extend(validated.assessment_objects().iter().map(StoreObjectV1::id));
    references.sort_unstable();
    references.dedup();
    let CborValue::Array(mut fields) = value else {
        return Err(RepositoryPublicationErrorV1::WorkCompletionBasisMismatch);
    };
    fields.push(bytes(validated.complete_cut_hash()));
    Ok(StoreObjectV1::new(
        derive_identity(&CborValue::Text(
            WORK_COMPLETION_EVIDENCE_BASIS_SCHEMA_V1.to_owned(),
        ))?,
        CborValue::Array(fields),
        references,
    )?)
}

fn work_submission_object(
    submission: &WorkSubmissionV1,
    claim_set: &StoreObjectV1,
    evidence_basis: &StoreObjectV1,
) -> Result<StoreObjectV1, RepositoryPublicationErrorV1> {
    let mut references = vec![claim_set.id(), evidence_basis.id()];
    references.sort_unstable();
    Ok(StoreObjectV1::new(
        derive_identity(&CborValue::Text(WORK_SUBMISSION_SCHEMA_V1.to_owned()))?,
        submission.canonical_value()?,
        references,
    )?)
}

fn validate_materialization_chain(
    store_basis: RepositoryStoreBasisV1,
    current_work: &WorkRecordV1,
    current_root: &CandidateContractRootV1,
    candidate_root: &CandidateContractRootV1,
    materializations: &[DecisionMaterializationCandidateV1],
) -> bool {
    if materializations.is_empty() {
        return true;
    }
    let mut expected_base = *current_root.root_id();
    let mut resolution_ids = BTreeSet::new();
    let mut touched_kinds = BTreeSet::<ContractComponentKindV1>::new();
    for materialization in materializations {
        let Some(resolution) = materialization.resolved_decision.resolution() else {
            return false;
        };
        if materialization.store_basis != store_basis
            || &materialization.current_work != current_work
            || materialization.preflight.base_root().root_id() != &expected_base
            || !resolution_ids.insert(*resolution.resolution_id())
        {
            return false;
        }
        let mut materialization_kinds = BTreeSet::<ContractComponentKindV1>::new();
        for component_id in materialization.preflight.delta().removed() {
            let Some(kind) = materialization
                .preflight
                .base_root()
                .components()
                .iter()
                .find(|component| component.component_id() == component_id)
                .map(|component| component.kind())
            else {
                return false;
            };
            materialization_kinds.insert(kind);
        }
        for component_id in materialization.preflight.delta().added() {
            let Some(kind) = materialization
                .preflight
                .candidate_root()
                .components()
                .iter()
                .find(|component| component.component_id() == component_id)
                .map(|component| component.kind())
            else {
                return false;
            };
            materialization_kinds.insert(kind);
        }
        if materialization_kinds
            .iter()
            .any(|kind| touched_kinds.contains(kind))
        {
            return false;
        }
        touched_kinds.extend(materialization_kinds);
        expected_base = *materialization.preflight.candidate_root().root_id();
    }
    expected_base == *candidate_root.root_id()
}

fn materialization_candidate_commitment(
    materialization: &DecisionMaterializationCandidateV1,
) -> Result<CborValue, RepositoryPublicationErrorV1> {
    let resolution = materialization
        .resolved_decision
        .resolution()
        .ok_or(RepositoryPublicationErrorV1::MaterializationBasisMismatch)?;
    Ok(CborValue::Array(vec![
        bytes(resolution.resolution_id().as_bytes()),
        bytes(materialization.current_work_object.id().as_bytes()),
        bytes(materialization.decision_object.id().as_bytes()),
        bytes(materialization.base_root_object.id().as_bytes()),
        bytes(materialization.candidate_root_object.id().as_bytes()),
        CborValue::Array(
            materialization
                .invalidation_receipts
                .iter()
                .map(|(component, receipt)| {
                    CborValue::Array(vec![bytes(component.as_bytes()), bytes(receipt.as_bytes())])
                })
                .collect(),
        ),
    ]))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppendDesignRevisionPublicationV1 {
    identity: RepositoryActionIdentityV1,
    store_basis: RepositoryStoreBasisV1,
    authority: RepositoryAuthoritySelectionV1,
    current: DesignStreamV1,
    successor: DesignStreamV1,
    appended_revision: DesignRevisionV1,
    current_object: StoreObjectV1,
    successor_object: StoreObjectV1,
    request_object: StoreObjectV1,
    subject_commitment: [u8; 32],
    subject_basis_commitment: [u8; 32],
    meaning_digest: [u8; 32],
}

impl AppendDesignRevisionPublicationV1 {
    pub fn new(
        identity: RepositoryActionIdentityV1,
        store_basis: RepositoryStoreBasisV1,
        authority: RepositoryAuthoritySelectionV1,
        current: DesignStreamV1,
        appended_revision: DesignRevisionV1,
        eligibility: DesignAppendEligibilityV1,
    ) -> Result<Self, RepositoryPublicationErrorV1> {
        let current_head = *current.candidate_head().revision_id();
        let successor = current.append(&current_head, appended_revision.clone(), eligibility)?;
        let current_object = design_stream_object(&current)?;
        let successor_object = design_stream_object(&successor)?;
        let subject_commitment = hash(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.repository-design-subject.v1")?,
            bytes(
                current
                    .candidate_head()
                    .repository_installation_id()
                    .as_bytes(),
            ),
            bytes(current.candidate_head().work_id().as_bytes()),
        ]))?;
        let subject_basis_commitment = hash(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.repository-design-revision-basis.v1")?,
            bytes(current_head.as_bytes()),
            bytes(current_object.id().as_bytes()),
        ]))?;
        let request_object = action_request_object(
            RepositoryActionKindV1::AppendDesignRevision,
            identity,
            store_basis,
            authority,
            subject_commitment,
            subject_basis_commitment,
            CborValue::Array(vec![
                bytes(current_head.as_bytes()),
                bytes(appended_revision.revision_id().as_bytes()),
                bytes(successor_object.id().as_bytes()),
            ]),
        )?;
        let meaning_digest = Sha256::digest(request_object.canonical_bytes()).into();
        Ok(Self {
            identity,
            store_basis,
            authority,
            current,
            successor,
            appended_revision,
            current_object,
            successor_object,
            request_object,
            subject_commitment,
            subject_basis_commitment,
            meaning_digest,
        })
    }

    pub fn successor(&self) -> &DesignStreamV1 {
        &self.successor
    }

    pub fn appended_revision(&self) -> &DesignRevisionV1 {
        &self.appended_revision
    }
}

pub struct RepositoryStoreV1<'store> {
    store: &'store mut StoreV1,
}

impl<'store> RepositoryStoreV1<'store> {
    pub fn new(store: &'store mut StoreV1) -> Self {
        Self { store }
    }

    pub fn create_draft_work(
        &mut self,
        publication: CreateDraftWorkPublicationV1,
    ) -> Result<RepositoryPublicationOutcomeV1, RepositoryPublicationErrorV1> {
        self.publish(publication.plan)
    }

    pub fn cancel_work(
        &mut self,
        publication: CancelWorkPublicationV1,
    ) -> Result<RepositoryPublicationOutcomeV1, RepositoryPublicationErrorV1> {
        self.publish(publication.plan)
    }

    pub fn submit_work_completion(
        &mut self,
        publication: SubmitWorkCompletionPublicationV1,
    ) -> Result<RepositoryPublicationOutcomeV1, RepositoryPublicationErrorV1> {
        self.publish(publication)
    }

    pub fn publish_initial_contract(
        &mut self,
        publication: InitialContractPublicationV1,
    ) -> Result<RepositoryPublicationOutcomeV1, RepositoryPublicationErrorV1> {
        self.publish_contract_initial(publication)
    }

    pub fn publish_contract_amendment(
        &mut self,
        publication: ContractAmendmentPublicationV1,
    ) -> Result<RepositoryPublicationOutcomeV1, RepositoryPublicationErrorV1> {
        self.publish_contract_required_amendment(publication)
    }

    pub fn validate_contract_amendment_no_op(
        &self,
        publication: &ContractAmendmentNoOpPublicationV1,
    ) -> Result<
        crate::domain::contract::runtime::ContractAmendmentNoOpV1,
        RepositoryPublicationErrorV1,
    > {
        let mut exact_roots = vec![
            &publication.current_work_object,
            &publication.current_generation_object,
            &publication.current_root_object,
            &publication.current_step_graph_object,
        ];
        exact_roots.extend(publication.current_step_state_objects.iter());
        let (generation, active_objects) =
            validate_no_op_store_basis(self.store, publication.store_basis, &exact_roots)?;
        validate_rooted_step_state_partition(
            &generation,
            &active_objects,
            &publication.current_step_graph,
            &publication.current_step_state_objects,
        )?;
        Ok(publication.no_op)
    }

    pub fn resolve_decision(
        &mut self,
        publication: DecisionResolutionPublicationV1,
    ) -> Result<RepositoryPublicationOutcomeV1, RepositoryPublicationErrorV1> {
        if self.store.state()?.0 != StoreStateV1::Active {
            return Err(RepositoryPublicationErrorV1::InactiveStore);
        }
        let request_id = publication.identity.request_id();
        let probe = StoreIdempotencyProbeV1::new(
            REPOSITORY_ACTION_IDEMPOTENCY_NAMESPACE_V1,
            *publication.identity.idempotency_key().as_bytes(),
            publication.meaning_digest,
        )?;
        let outcome = self
            .store
            .publish_generation_atomically_with_prepare(&probe, |view| {
                prepare_decision_resolution(view, publication)
            });
        match outcome {
            Ok(outcome) => publication_outcome(outcome, request_id),
            Err(PreparedPublicationError::Store(error)) => Err(error.into()),
            Err(PreparedPublicationError::Prepare(error)) => Err(error),
        }
    }

    pub fn validate_equal_root_materialization<'publication>(
        &self,
        publication: &'publication DecisionMaterializationNoOpPublicationV1,
    ) -> Result<&'publication DecisionMaterializationV1, RepositoryPublicationErrorV1> {
        if publication.current_work.state().is_terminal() {
            return Err(RepositoryPublicationErrorV1::MaterializationBasisMismatch);
        }
        let _ = validate_no_op_store_basis(
            self.store,
            publication.store_basis,
            &[
                &publication.current_work_object,
                &publication.decision_object,
                &publication.base_root_object,
            ],
        )?;
        Ok(&publication.materialization)
    }

    pub fn validate_exactly_equivalent_materialization(
        &self,
        candidate: DecisionMaterializationCandidateV1,
    ) -> Result<DecisionMaterializationV1, RepositoryPublicationErrorV1> {
        let (generation, active_objects) = validate_no_op_store_basis(
            self.store,
            candidate.store_basis,
            &[
                &candidate.current_work_object,
                &candidate.decision_object,
                &candidate.base_root_object,
            ],
        )?;
        if candidate.current_work.state().is_terminal()
            || candidate.resolved_decision.work_id() != candidate.current_work.id()
            || candidate.resolved_decision.repository_installation_id() != &self.store.domain().id()
            || candidate.preflight.is_equal_root()
            || candidate.preflight.base_root().root_id() != &generation.contract_root_id()
        {
            return Err(RepositoryPublicationErrorV1::MaterializationBasisMismatch);
        }
        let expected = candidate.evaluate_exact_equivalence_receipt()?;
        let referenced = [
            &candidate.current_work_object,
            &candidate.decision_object,
            &candidate.base_root_object,
        ];
        if referenced.iter().any(|expected_object| {
            active_objects
                .iter()
                .filter(|object| object.id() == expected_object.id())
                .collect::<Vec<_>>()
                .as_slice()
                != [*expected_object]
        }) {
            return Err(RepositoryPublicationErrorV1::MaterializationReceiptMismatch);
        }
        Ok(candidate
            .preflight
            .complete_exact_equivalent(expected.id())?)
    }

    pub fn append_design_revision(
        &mut self,
        publication: AppendDesignRevisionPublicationV1,
    ) -> Result<RepositoryPublicationOutcomeV1, RepositoryPublicationErrorV1> {
        self.publish(publication)
    }

    fn publish<P: SealedRepositoryActionPlanV1>(
        &mut self,
        publication: P,
    ) -> Result<RepositoryPublicationOutcomeV1, RepositoryPublicationErrorV1> {
        if self.store.state()?.0 != StoreStateV1::Active {
            return Err(RepositoryPublicationErrorV1::InactiveStore);
        }
        let request_id = publication.identity().request_id;
        let probe = StoreIdempotencyProbeV1::new(
            REPOSITORY_ACTION_IDEMPOTENCY_NAMESPACE_V1,
            *publication.identity().idempotency_key.as_bytes(),
            publication.meaning_digest(),
        )?;
        let outcome = self
            .store
            .publish_generation_atomically_with_prepare(&probe, |view| {
                prepare_publication(view, publication)
            });
        match outcome {
            Ok(outcome) => publication_outcome(outcome, request_id),
            Err(PreparedPublicationError::Store(error)) => Err(error.into()),
            Err(PreparedPublicationError::Prepare(error)) => Err(error),
        }
    }

    fn publish_contract_initial(
        &mut self,
        publication: InitialContractPublicationV1,
    ) -> Result<RepositoryPublicationOutcomeV1, RepositoryPublicationErrorV1> {
        if self.store.state()?.0 != StoreStateV1::Active {
            return Err(RepositoryPublicationErrorV1::InactiveStore);
        }
        let request_id = publication.request.request_id();
        let probe = StoreIdempotencyProbeV1::new(
            REPOSITORY_ACTION_IDEMPOTENCY_NAMESPACE_V1,
            *publication.request.idempotency_key_id().as_bytes(),
            publication.meaning_digest,
        )?;
        let outcome = self
            .store
            .publish_generation_atomically_with_prepare(&probe, |view| {
                prepare_initial_contract_publication(view, publication)
            });
        match outcome {
            Ok(outcome) => publication_outcome(outcome, request_id),
            Err(PreparedPublicationError::Store(error)) => Err(error.into()),
            Err(PreparedPublicationError::Prepare(error)) => Err(error),
        }
    }

    fn publish_contract_required_amendment(
        &mut self,
        publication: ContractAmendmentPublicationV1,
    ) -> Result<RepositoryPublicationOutcomeV1, RepositoryPublicationErrorV1> {
        if self.store.state()?.0 != StoreStateV1::Active {
            return Err(RepositoryPublicationErrorV1::InactiveStore);
        }
        let request_id = publication.request.request_id();
        let probe = StoreIdempotencyProbeV1::new(
            REPOSITORY_ACTION_IDEMPOTENCY_NAMESPACE_V1,
            *publication.request.idempotency_key_id().as_bytes(),
            publication.meaning_digest,
        )?;
        let outcome = self
            .store
            .publish_generation_atomically_with_prepare(&probe, |view| {
                prepare_contract_amendment_publication(view, publication)
            });
        match outcome {
            Ok(outcome) => publication_outcome(outcome, request_id),
            Err(PreparedPublicationError::Store(error)) => Err(error.into()),
            Err(PreparedPublicationError::Prepare(error)) => Err(error),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepositoryPublicationKindV1 {
    Committed,
    Replayed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryPublicationOutcomeV1 {
    kind: RepositoryPublicationKindV1,
    head: crate::domain::persistence::StoreHeadV1,
    result: StoreObjectV1,
    logical_result_id: ActionResultIdV1,
}

impl RepositoryPublicationOutcomeV1 {
    pub const fn kind(&self) -> RepositoryPublicationKindV1 {
        self.kind
    }

    pub fn head(&self) -> &crate::domain::persistence::StoreHeadV1 {
        &self.head
    }

    pub fn result(&self) -> &StoreObjectV1 {
        &self.result
    }

    pub const fn logical_result_id(&self) -> ActionResultIdV1 {
        self.logical_result_id
    }

    pub const fn response_origin(&self) -> ResponseOriginV1 {
        match self.kind {
            RepositoryPublicationKindV1::Committed => ResponseOriginV1::Fresh,
            RepositoryPublicationKindV1::Replayed => ResponseOriginV1::Replay {
                original_result_id: self.logical_result_id,
            },
        }
    }
}

trait SealedRepositoryActionPlanV1: private::Sealed {
    fn kind(&self) -> RepositoryActionKindV1;
    fn identity(&self) -> RepositoryActionIdentityV1;
    fn store_basis(&self) -> RepositoryStoreBasisV1;
    fn authority(&self) -> RepositoryAuthoritySelectionV1;
    fn authority_leaf(&self) -> RepositoryActionLeafV1 {
        self.kind().authority_leaf()
    }
    fn produced_objects(&self) -> Vec<StoreObjectV1>;
    fn root_replacements(&self) -> Vec<(StoreObjectIdV1, StoreObjectIdV1)>;
    fn added_roots(&self) -> Vec<StoreObjectIdV1> {
        Vec::new()
    }
    fn request_object(&self) -> &StoreObjectV1;
    fn subject_commitment(&self) -> [u8; 32];
    fn subject_basis_commitment(&self) -> [u8; 32];
    fn meaning_digest(&self) -> [u8; 32];
    fn additional_store_effects(
        &self,
        _view: &StorePublicationViewV1<'_>,
        _generation: &StoreGenerationV1,
        _active_objects: &[StoreObjectV1],
    ) -> Result<RepositoryStoreEffectsV1, RepositoryPublicationErrorV1> {
        Ok(RepositoryStoreEffectsV1::default())
    }
    fn validate_admission(
        &self,
        _admission: &AdmittedRepositoryActionV1,
    ) -> Result<(), RepositoryPublicationErrorV1> {
        Ok(())
    }
    fn validate_current_subject(
        &self,
        generation: &StoreGenerationV1,
        active_objects: &[StoreObjectV1],
    ) -> Result<(), RepositoryPublicationErrorV1>;
}

#[derive(Default)]
struct RepositoryStoreEffectsV1 {
    produced_objects: Vec<StoreObjectV1>,
    root_replacements: Vec<(StoreObjectIdV1, StoreObjectIdV1)>,
}

mod private {
    pub trait Sealed {}
}

impl private::Sealed for CreateDraftWorkPlanV1 {}

impl SealedRepositoryActionPlanV1 for CreateDraftWorkPlanV1 {
    fn kind(&self) -> RepositoryActionKindV1 {
        RepositoryActionKindV1::CreateDraftWork
    }

    fn identity(&self) -> RepositoryActionIdentityV1 {
        self.identity
    }

    fn store_basis(&self) -> RepositoryStoreBasisV1 {
        self.store_basis
    }

    fn authority(&self) -> RepositoryAuthoritySelectionV1 {
        self.authority
    }

    fn produced_objects(&self) -> Vec<StoreObjectV1> {
        vec![self.work_object.clone()]
    }

    fn root_replacements(&self) -> Vec<(StoreObjectIdV1, StoreObjectIdV1)> {
        Vec::new()
    }

    fn added_roots(&self) -> Vec<StoreObjectIdV1> {
        vec![self.work_object.id()]
    }

    fn request_object(&self) -> &StoreObjectV1 {
        &self.request_object
    }

    fn subject_commitment(&self) -> [u8; 32] {
        self.subject_commitment
    }

    fn subject_basis_commitment(&self) -> [u8; 32] {
        self.subject_basis_commitment
    }

    fn meaning_digest(&self) -> [u8; 32] {
        self.meaning_digest
    }

    fn validate_current_subject(
        &self,
        _generation: &StoreGenerationV1,
        active_objects: &[StoreObjectV1],
    ) -> Result<(), RepositoryPublicationErrorV1> {
        let duplicate = active_objects.iter().any(|object| {
            object.schema_id()
                == repository_schema_id(REPOSITORY_WORK_RECORD_DOMAIN_V1)
                    .expect("invariant: static Work Record schema is valid")
                && matches!(
                    object.value(),
                    CborValue::Array(fields)
                        if matches!(fields.get(1), Some(CborValue::Bytes(id))
                            if id.as_slice() == self.work.id().as_bytes())
                )
        });
        if duplicate {
            return Err(RepositoryPublicationErrorV1::SubjectAlreadyExists);
        }
        Ok(())
    }
}

impl private::Sealed for WorkMutationPublicationV1 {}

impl SealedRepositoryActionPlanV1 for WorkMutationPublicationV1 {
    fn kind(&self) -> RepositoryActionKindV1 {
        self.kind
    }

    fn identity(&self) -> RepositoryActionIdentityV1 {
        self.identity
    }

    fn store_basis(&self) -> RepositoryStoreBasisV1 {
        self.store_basis
    }

    fn authority(&self) -> RepositoryAuthoritySelectionV1 {
        self.authority
    }

    fn produced_objects(&self) -> Vec<StoreObjectV1> {
        vec![self.successor_object.clone()]
    }

    fn root_replacements(&self) -> Vec<(StoreObjectIdV1, StoreObjectIdV1)> {
        vec![(self.current_object.id(), self.successor_object.id())]
    }

    fn request_object(&self) -> &StoreObjectV1 {
        &self.request_object
    }

    fn subject_commitment(&self) -> [u8; 32] {
        self.subject_commitment
    }

    fn subject_basis_commitment(&self) -> [u8; 32] {
        self.subject_basis_commitment
    }

    fn meaning_digest(&self) -> [u8; 32] {
        self.meaning_digest
    }

    fn validate_current_subject(
        &self,
        generation: &StoreGenerationV1,
        _active_objects: &[StoreObjectV1],
    ) -> Result<(), RepositoryPublicationErrorV1> {
        if !generation.roots().contains(&self.current_object.id())
            || self.current.id() != self.successor.id()
            || self.current.revision().get().checked_add(1) != Some(self.successor.revision().get())
        {
            return Err(RepositoryPublicationErrorV1::SubjectBasisMismatch);
        }
        Ok(())
    }
}

impl private::Sealed for SubmitWorkCompletionPublicationV1 {}

impl SealedRepositoryActionPlanV1 for SubmitWorkCompletionPublicationV1 {
    fn kind(&self) -> RepositoryActionKindV1 {
        RepositoryActionKindV1::SubmitWorkCompletion
    }

    fn identity(&self) -> RepositoryActionIdentityV1 {
        self.identity
    }

    fn store_basis(&self) -> RepositoryStoreBasisV1 {
        self.store_basis
    }

    fn authority(&self) -> RepositoryAuthoritySelectionV1 {
        self.authority
    }

    fn produced_objects(&self) -> Vec<StoreObjectV1> {
        vec![self.successor_work_object.clone()]
    }

    fn additional_store_effects(
        &self,
        view: &StorePublicationViewV1<'_>,
        generation: &StoreGenerationV1,
        active_objects: &[StoreObjectV1],
    ) -> Result<RepositoryStoreEffectsV1, RepositoryPublicationErrorV1> {
        let head = view
            .active_head()?
            .ok_or(RepositoryPublicationErrorV1::StaleStoreBasis)?;
        let validated = validate_work_completion_evidence(
            view,
            &head,
            generation,
            active_objects,
            self.current_work.id(),
            self.current_generation.id(),
            self.current_generation.root_id(),
            &self.gate_snapshot,
            &self.gate_resolutions,
            &self.evidence,
            self.as_of,
        )?;
        let claim_objects = work_completion_claim_objects(&self.evidence)?;
        let claim_set_object = work_submission_claim_set_object(
            &self.evidence,
            &claim_objects,
            validated.observation_objects(),
        )?;
        let evidence_basis_object =
            work_completion_evidence_basis_object(self.evidence_basis_value.clone(), &validated)?;
        let submission_object =
            work_submission_object(&self.submission, &claim_set_object, &evidence_basis_object)?;
        let mut produced_objects = claim_objects;
        produced_objects.extend([claim_set_object, evidence_basis_object, submission_object]);
        Ok(RepositoryStoreEffectsV1 {
            produced_objects,
            root_replacements: Vec::new(),
        })
    }

    fn validate_admission(
        &self,
        admission: &AdmittedRepositoryActionV1,
    ) -> Result<(), RepositoryPublicationErrorV1> {
        if admission.accepted_h_time() != self.as_of {
            return Err(RepositoryPublicationErrorV1::WorkCompletionBasisMismatch);
        }
        Ok(())
    }

    fn root_replacements(&self) -> Vec<(StoreObjectIdV1, StoreObjectIdV1)> {
        vec![(
            self.current_work_object.id(),
            self.successor_work_object.id(),
        )]
    }

    fn request_object(&self) -> &StoreObjectV1 {
        &self.request_object
    }

    fn subject_commitment(&self) -> [u8; 32] {
        self.subject_commitment
    }

    fn subject_basis_commitment(&self) -> [u8; 32] {
        self.subject_basis_commitment
    }

    fn meaning_digest(&self) -> [u8; 32] {
        self.meaning_digest
    }

    fn validate_current_subject(
        &self,
        generation: &StoreGenerationV1,
        active_objects: &[StoreObjectV1],
    ) -> Result<(), RepositoryPublicationErrorV1> {
        if generation.domain().id() != self.current_step_graph.scope().repository_id()
            || generation.contract_root_id() != self.current_generation.root_id()
            || !validate_current_work_completion_basis(
                self.store_basis,
                &self.current_work,
                &self.current_generation,
                &self.current_root,
                &self.current_step_graph,
                &self.current_step_states,
                &self.current_step_submissions,
                self.successor_work
                    .current_submission()
                    .ok_or(RepositoryPublicationErrorV1::WorkCompletionBasisMismatch)?,
            )
        {
            return Err(RepositoryPublicationErrorV1::WorkCompletionBasisMismatch);
        }
        let exact_rooted = [
            &self.current_work_object,
            &self.current_generation_object,
            &self.current_root_object,
            &self.current_step_graph_object,
        ]
        .into_iter()
        .chain(self.current_step_state_objects.iter())
        .all(|expected| {
            generation.roots().contains(&expected.id())
                && active_objects
                    .iter()
                    .filter(|object| **object == *expected)
                    .count()
                    == 1
        });
        if !exact_rooted {
            return Err(RepositoryPublicationErrorV1::WorkCompletionBasisMismatch);
        }
        let submission_schema = derive_identity(&CborValue::Text(
            EXECUTION_STEP_SUBMISSION_SCHEMA_V1.to_owned(),
        ))?;
        for expected in &self.current_step_submissions {
            let expected_value = expected.canonical_value()?;
            if active_objects
                .iter()
                .filter(|object| {
                    object.schema_id() == submission_schema && object.value() == &expected_value
                })
                .count()
                != 1
            {
                return Err(RepositoryPublicationErrorV1::WorkCompletionBasisMismatch);
            }
        }
        Ok(())
    }
}

impl private::Sealed for AppendDesignRevisionPublicationV1 {}

impl SealedRepositoryActionPlanV1 for AppendDesignRevisionPublicationV1 {
    fn kind(&self) -> RepositoryActionKindV1 {
        RepositoryActionKindV1::AppendDesignRevision
    }

    fn identity(&self) -> RepositoryActionIdentityV1 {
        self.identity
    }

    fn store_basis(&self) -> RepositoryStoreBasisV1 {
        self.store_basis
    }

    fn authority(&self) -> RepositoryAuthoritySelectionV1 {
        self.authority
    }

    fn produced_objects(&self) -> Vec<StoreObjectV1> {
        vec![self.successor_object.clone()]
    }

    fn root_replacements(&self) -> Vec<(StoreObjectIdV1, StoreObjectIdV1)> {
        vec![(self.current_object.id(), self.successor_object.id())]
    }

    fn request_object(&self) -> &StoreObjectV1 {
        &self.request_object
    }

    fn subject_commitment(&self) -> [u8; 32] {
        self.subject_commitment
    }

    fn subject_basis_commitment(&self) -> [u8; 32] {
        self.subject_basis_commitment
    }

    fn meaning_digest(&self) -> [u8; 32] {
        self.meaning_digest
    }

    fn validate_current_subject(
        &self,
        generation: &StoreGenerationV1,
        _active_objects: &[StoreObjectV1],
    ) -> Result<(), RepositoryPublicationErrorV1> {
        if !generation.roots().contains(&self.current_object.id())
            || self.current.candidate_head().revision_id()
                != self
                    .appended_revision
                    .parent_revision_id()
                    .ok_or(RepositoryPublicationErrorV1::SubjectBasisMismatch)?
            || self.successor.candidate_head().revision_id() != self.appended_revision.revision_id()
        {
            return Err(RepositoryPublicationErrorV1::SubjectBasisMismatch);
        }
        Ok(())
    }
}

fn prepare_initial_contract_publication(
    view: &StorePublicationViewV1<'_>,
    publication: InitialContractPublicationV1,
) -> Result<AtomicGenerationPublicationV1, RepositoryPublicationErrorV1> {
    let (current_head, current_generation, active_objects) =
        exact_store_basis(view, publication.store_basis)?;
    require_exact_rooted_object(
        &current_generation,
        &active_objects,
        &publication.current_work_object,
    )?;
    if publication.current_work.id() != publication.successor_work.id()
        || publication.current_work.revision().get().checked_add(1)
            != Some(publication.successor_work.revision().get())
        || current_generation
            .roots()
            .iter()
            .filter_map(|root| active_objects.iter().find(|object| object.id() == *root))
            .any(|object| {
                object.schema_id()
                    == repository_schema_id(REPOSITORY_CONTRACT_GENERATION_DOMAIN_V1)
                        .expect("invariant: static Repository Contract schema is valid")
            })
    {
        return Err(RepositoryPublicationErrorV1::ContractPublicationBasisMismatch);
    }
    let admission = admit_repository_action(
        view,
        &current_generation,
        RepositoryActionAdmissionInputV1::new(
            publication.request.request_id(),
            publication.authority.clone(),
        ),
    )?;
    let provisional_products = vec![
        publication.successor_work_object.clone(),
        publication.revision_object.clone(),
        publication.finalization_object.clone(),
        publication.candidate_root_object.clone(),
    ];
    let provisional =
        admission.issue_committed_artifacts(&publication.request_object, &provisional_products)?;
    let guard = exact_admitted_transition_guard(&active_objects, admission.basis_object())?;
    let authority = ContractPublicationAuthorityV1::from_store_commit(
        &publication.request,
        provisional.logical_result(),
        hash(guard.value())?,
    )?;
    let contract_generation = publication.prepared.authorize(authority)?;
    if publication.steps.graph().contract_generation_id() != contract_generation.id()
        || publication.steps.graph().contract_root_id() != contract_generation.root_id()
        || publication.steps.step_states().iter().any(|state| {
            state.binding().contract_generation_id() != contract_generation.id()
                || state.binding().contract_root_id() != contract_generation.root_id()
                || !matches!(
                    state.lifecycle(),
                    StepLifecycleV1::Open {
                        basis: StepOpenBasisV1::Fresh
                    }
                )
        })
    {
        return Err(RepositoryPublicationErrorV1::ContractStepPublicationMismatch);
    }
    let contract_generation_object = contract_generation_object(&contract_generation)?;
    let mut produced_objects = provisional_products;
    produced_objects.extend([
        contract_generation_object.clone(),
        publication.step_graph_object.clone(),
    ]);
    produced_objects.extend(publication.step_state_objects.iter().cloned());
    let artifacts =
        admission.issue_committed_artifacts(&publication.request_object, &produced_objects)?;
    let mut roots = current_generation.roots().to_vec();
    replace_root(
        &mut roots,
        publication.current_work_object.id(),
        publication.successor_work_object.id(),
    )?;
    replace_root(
        &mut roots,
        admission.current_snapshot_id(),
        admission.successor_snapshot().id(),
    )?;
    replace_root_if_direct(
        &mut roots,
        admission.current_capacity_root_id(),
        admission.successor_capacity_root().id(),
    )?;
    roots.extend([
        contract_generation_object.id(),
        publication.candidate_root_object.id(),
        publication.step_graph_object.id(),
        artifacts.result_object().id(),
    ]);
    roots.extend(publication.step_state_objects.iter().map(StoreObjectV1::id));
    roots.sort_unstable();
    roots.dedup();
    let generation = StoreGenerationV1::new(
        view.domain().clone(),
        current_generation
            .ordinal()
            .checked_add(1)
            .ok_or(RepositoryPublicationErrorV1::StaleStoreBasis)?,
        Some(current_generation.id()),
        *publication.candidate_root.root_id(),
        StoreCompatibilityV1::stage0_successor()?,
        roots,
    )?;
    let idempotency = StoreIdempotencyV1::new(
        REPOSITORY_ACTION_IDEMPOTENCY_NAMESPACE_V1,
        *publication.request.idempotency_key_id().as_bytes(),
        publication.meaning_digest,
        artifacts.result_object().id(),
    )?;
    produced_objects.extend([
        publication.request_object,
        admission.basis_object().clone(),
        admission.successor_snapshot().clone(),
        admission.successor_capacity_root().clone(),
        admission.capacity_debit().clone(),
        artifacts.receipt_object().clone(),
        artifacts.result_object().clone(),
    ]);
    produced_objects.extend(artifacts.leaf_authority_objects().iter().cloned());
    produced_objects.sort_by_key(StoreObjectV1::id);
    produced_objects.dedup_by_key(|object| object.id());
    Ok(AtomicGenerationPublicationV1::new_from_object_superset(
        generation,
        Some(current_head.id()),
        produced_objects,
        idempotency,
    )?)
}

fn prepare_decision_resolution(
    view: &StorePublicationViewV1<'_>,
    publication: DecisionResolutionPublicationV1,
) -> Result<AtomicGenerationPublicationV1, RepositoryPublicationErrorV1> {
    let (current_head, current_generation, active_objects) =
        exact_store_basis(view, publication.store_basis)?;
    if publication.current_decision.repository_installation_id() != &view.domain().id()
        || publication.current_work.state().is_terminal()
        || publication.current_decision.work_id() != publication.current_work.id()
        || !matches!(publication.current_decision.state(), DecisionStateV1::Open)
        || publication.current_decision.head().revision_id() != &publication.expected_head
    {
        return Err(RepositoryPublicationErrorV1::DecisionResolutionBasisMismatch);
    }
    for object in [
        &publication.current_work_object,
        &publication.current_decision_object,
    ] {
        require_exact_rooted_object(&current_generation, &active_objects, object)?;
    }
    let admission = admit_repository_action(
        view,
        &current_generation,
        RepositoryActionAdmissionInputV1::new(
            publication.identity.request_id(),
            publication.authority.clone(),
        ),
    )?;
    let provisional = admission.issue_committed_artifacts(
        &publication.request_object,
        std::slice::from_ref(&publication.current_decision_object),
    )?;
    let guard = exact_admitted_transition_guard(&active_objects, admission.basis_object())?;
    let admitted = AdmittedCommittedActionV1::from_store_commit(
        provisional.logical_result(),
        guard.id(),
        view.domain().id(),
        current_head.id(),
        current_generation.id(),
    )?;
    let resolved = publication.current_decision.resolve(
        &publication.expected_head,
        &publication.selected_alternative,
        publication.rationale,
        publication.rejected_alternatives,
        &admitted,
        WorkDecisionEligibilityV1::Eligible,
    )?;
    let resolved_object = decision_object(&resolved)?;
    let produced_objects = vec![resolved_object.clone()];
    let artifacts =
        admission.issue_committed_artifacts(&publication.request_object, &produced_objects)?;
    let mut roots = current_generation.roots().to_vec();
    for (current, successor) in [
        (
            publication.current_decision_object.id(),
            resolved_object.id(),
        ),
        (
            admission.current_snapshot_id(),
            admission.successor_snapshot().id(),
        ),
    ] {
        replace_root(&mut roots, current, successor)?;
    }
    replace_root_if_direct(
        &mut roots,
        admission.current_capacity_root_id(),
        admission.successor_capacity_root().id(),
    )?;
    roots.push(artifacts.result_object().id());
    roots.sort_unstable();
    roots.dedup();
    let generation = StoreGenerationV1::new(
        view.domain().clone(),
        current_generation
            .ordinal()
            .checked_add(1)
            .ok_or(RepositoryPublicationErrorV1::StaleStoreBasis)?,
        Some(current_generation.id()),
        current_generation.contract_root_id(),
        StoreCompatibilityV1::stage0_successor()?,
        roots,
    )?;
    let idempotency = StoreIdempotencyV1::new(
        REPOSITORY_ACTION_IDEMPOTENCY_NAMESPACE_V1,
        *publication.identity.idempotency_key().as_bytes(),
        publication.meaning_digest,
        artifacts.result_object().id(),
    )?;
    let mut objects = produced_objects;
    objects.extend([
        publication.request_object,
        admission.basis_object().clone(),
        admission.successor_snapshot().clone(),
        admission.successor_capacity_root().clone(),
        admission.capacity_debit().clone(),
        artifacts.receipt_object().clone(),
        artifacts.result_object().clone(),
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

fn prepare_contract_amendment_publication(
    view: &StorePublicationViewV1<'_>,
    publication: ContractAmendmentPublicationV1,
) -> Result<AtomicGenerationPublicationV1, RepositoryPublicationErrorV1> {
    let (current_head, current_store_generation, active_objects) =
        exact_store_basis(view, publication.store_basis)?;
    for object in [
        &publication.current_work_object,
        &publication.current_generation_object,
        &publication.current_root_object,
        &publication.current_step_graph_object,
    ] {
        require_exact_rooted_object(&current_store_generation, &active_objects, object)?;
    }
    for materialization in &publication.materialization_audits {
        require_exact_rooted_object(
            &current_store_generation,
            &active_objects,
            &materialization.current_work_object,
        )?;
        require_exact_rooted_object(
            &current_store_generation,
            &active_objects,
            &materialization.decision_object,
        )?;
        validate_candidate_materialization_receipts(
            &current_store_generation,
            &active_objects,
            materialization,
        )?;
    }
    for object in &publication.current_step_state_objects {
        require_exact_rooted_object(&current_store_generation, &active_objects, object)?;
    }
    validate_rooted_step_state_partition(
        &current_store_generation,
        &active_objects,
        &publication.current_step_graph,
        &publication.current_step_state_objects,
    )?;
    if publication.current_generation.root_id() != current_store_generation.contract_root_id()
        || publication.current_root.root_id() != &current_store_generation.contract_root_id()
        || publication.current_work.id() != publication.successor_work.id()
        || publication.current_work.revision().get().checked_add(1)
            != Some(publication.successor_work.revision().get())
        || publication.current_step_state_objects.len() != publication.current_step_states.len()
        || publication
            .materialization_audits
            .iter()
            .any(|materialization| {
                materialization
                    .resolved_decision
                    .repository_installation_id()
                    != &view.domain().id()
            })
    {
        return Err(RepositoryPublicationErrorV1::ContractPublicationBasisMismatch);
    }
    let admission = admit_repository_action(
        view,
        &current_store_generation,
        RepositoryActionAdmissionInputV1::new(
            publication.request.request_id(),
            publication.authority.clone(),
        ),
    )?;
    let provisional_products = vec![
        publication.successor_work_object.clone(),
        publication.revision_object.clone(),
        publication.finalization_object.clone(),
        publication.candidate_root_object.clone(),
    ];
    let provisional =
        admission.issue_committed_artifacts(&publication.request_object, &provisional_products)?;
    let guard = exact_admitted_transition_guard(&active_objects, admission.basis_object())?;
    let authority = ContractPublicationAuthorityV1::from_store_commit(
        &publication.request,
        provisional.logical_result(),
        hash(guard.value())?,
    )?;
    let contract_generation = publication.prepared.authorize(authority)?;
    if publication.candidate_step_graph.contract_generation_id() != contract_generation.id()
        || publication.candidate_step_graph.contract_root_id() != contract_generation.root_id()
    {
        return Err(RepositoryPublicationErrorV1::ContractStepPublicationMismatch);
    }
    let contract_generation_object = contract_generation_object(&contract_generation)?;
    let amendment_receipt_hash = *provisional
        .logical_result()
        .authorization_receipt()
        .ok_or(RepositoryPublicationErrorV1::ContractStepPublicationMismatch)?
        .id()
        .as_bytes();
    let applied_steps = publication.step_plan.apply(
        &publication.current_step_graph,
        &publication.candidate_step_graph,
        &publication.current_step_states,
        amendment_receipt_hash,
    )?;
    let historical_states = applied_steps
        .retain_exact()
        .iter()
        .map(|disposition| disposition.historical_state())
        .chain(
            applied_steps
                .replace()
                .iter()
                .map(|disposition| disposition.historical_state()),
        )
        .chain(
            applied_steps
                .remove()
                .iter()
                .map(|disposition| disposition.historical_state()),
        )
        .collect::<Vec<_>>();
    let next_states = applied_steps
        .retain_exact()
        .iter()
        .map(|disposition| disposition.next_state())
        .chain(
            applied_steps
                .replace()
                .iter()
                .map(|disposition| disposition.next_state()),
        )
        .chain(
            applied_steps
                .add()
                .iter()
                .map(|disposition| disposition.next_state()),
        )
        .map(|state| state.materialize())
        .collect::<Result<Vec<_>, _>>()?;
    if next_states.len() != publication.candidate_step_graph.nodes().len()
        || next_states.iter().any(|state| {
            !publication
                .candidate_step_graph
                .nodes()
                .iter()
                .any(|node| node.binding() == state.binding())
        })
    {
        return Err(RepositoryPublicationErrorV1::ContractStepPublicationMismatch);
    }
    let historical_state_objects = historical_states
        .iter()
        .map(step_state_object)
        .collect::<Result<Vec<_>, _>>()?;
    let next_state_objects = next_states
        .iter()
        .map(step_state_object)
        .collect::<Result<Vec<_>, _>>()?;
    let materialization_candidate_root_objects = publication
        .materialization_audits
        .iter()
        .map(|candidate| candidate.candidate_root_object.clone())
        .collect::<Vec<_>>();
    let materializations = publication
        .materialization_audits
        .into_iter()
        .map(|candidate| {
            let admitted = AdmittedCommittedActionV1::from_store_commit(
                provisional.logical_result(),
                guard.id(),
                view.domain().id(),
                current_head.id(),
                current_store_generation.id(),
            )?;
            let authority = AdmittedMaterializationAuthorityV1::from_store_commit(
                admitted,
                &candidate.preflight,
                candidate.invalidation_receipts,
            )?;
            candidate
                .preflight
                .complete_with_authority(authority)
                .map_err(RepositoryPublicationErrorV1::from)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let materialization_audit_objects = materializations
        .iter()
        .map(decision_materialization_audit_object)
        .collect::<Result<Vec<_>, _>>()?;
    let step_amendment_audit_object = step_amendment_audit_object(
        &publication.step_plan,
        &applied_steps,
        &publication.current_step_graph_object,
        &publication.candidate_step_graph_object,
        &materialization_audit_objects,
        &historical_state_objects,
        &next_state_objects,
    )?;
    let mut produced_objects = provisional_products;
    produced_objects.extend([
        contract_generation_object.clone(),
        publication.candidate_step_graph_object.clone(),
        step_amendment_audit_object.clone(),
    ]);
    produced_objects.extend(materialization_candidate_root_objects);
    produced_objects.extend(materialization_audit_objects.iter().cloned());
    produced_objects.extend(historical_state_objects.iter().cloned());
    produced_objects.extend(next_state_objects.iter().cloned());
    let artifacts =
        admission.issue_committed_artifacts(&publication.request_object, &produced_objects)?;
    let mut roots = current_store_generation.roots().to_vec();
    for (current, successor) in [
        (
            publication.current_work_object.id(),
            publication.successor_work_object.id(),
        ),
        (
            publication.current_generation_object.id(),
            contract_generation_object.id(),
        ),
        (
            publication.current_root_object.id(),
            publication.candidate_root_object.id(),
        ),
        (
            publication.current_step_graph_object.id(),
            publication.candidate_step_graph_object.id(),
        ),
        (
            admission.current_snapshot_id(),
            admission.successor_snapshot().id(),
        ),
    ] {
        replace_root(&mut roots, current, successor)?;
    }
    for (current_state, current_object) in publication
        .current_step_states
        .iter()
        .zip(&publication.current_step_state_objects)
    {
        let historical_object = historical_states
            .iter()
            .zip(&historical_state_objects)
            .find_map(|(state, object)| {
                (state.binding() == current_state.binding()).then_some(object)
            })
            .ok_or(RepositoryPublicationErrorV1::ContractStepPublicationMismatch)?;
        replace_root(&mut roots, current_object.id(), historical_object.id())?;
    }
    replace_root_if_direct(
        &mut roots,
        admission.current_capacity_root_id(),
        admission.successor_capacity_root().id(),
    )?;
    roots.extend(materialization_audit_objects.iter().map(StoreObjectV1::id));
    roots.extend([
        step_amendment_audit_object.id(),
        artifacts.result_object().id(),
    ]);
    roots.extend(next_state_objects.iter().map(StoreObjectV1::id));
    roots.sort_unstable();
    roots.dedup();
    let generation = StoreGenerationV1::new(
        view.domain().clone(),
        current_store_generation
            .ordinal()
            .checked_add(1)
            .ok_or(RepositoryPublicationErrorV1::StaleStoreBasis)?,
        Some(current_store_generation.id()),
        *publication.candidate_root.root_id(),
        StoreCompatibilityV1::stage0_successor()?,
        roots,
    )?;
    let idempotency = StoreIdempotencyV1::new(
        REPOSITORY_ACTION_IDEMPOTENCY_NAMESPACE_V1,
        *publication.request.idempotency_key_id().as_bytes(),
        publication.meaning_digest,
        artifacts.result_object().id(),
    )?;
    produced_objects.extend([
        publication.request_object,
        admission.basis_object().clone(),
        admission.successor_snapshot().clone(),
        admission.successor_capacity_root().clone(),
        admission.capacity_debit().clone(),
        artifacts.receipt_object().clone(),
        artifacts.result_object().clone(),
    ]);
    produced_objects.extend(artifacts.leaf_authority_objects().iter().cloned());
    produced_objects.sort_by_key(StoreObjectV1::id);
    produced_objects.dedup_by_key(|object| object.id());
    let mut successor_active_objects = active_objects;
    successor_active_objects.extend(produced_objects.iter().cloned());
    successor_active_objects.sort_by_key(StoreObjectV1::id);
    successor_active_objects.dedup_by_key(|object| object.id());
    validate_rooted_step_state_partition(
        &generation,
        &successor_active_objects,
        &publication.candidate_step_graph,
        &next_state_objects,
    )?;
    Ok(AtomicGenerationPublicationV1::new_from_object_superset(
        generation,
        Some(current_head.id()),
        produced_objects,
        idempotency,
    )?)
}

fn exact_store_basis(
    view: &StorePublicationViewV1<'_>,
    basis: RepositoryStoreBasisV1,
) -> Result<(StoreHeadV1, StoreGenerationV1, Vec<StoreObjectV1>), RepositoryPublicationErrorV1> {
    if view.role() != StoreRoleV1::Repository {
        return Err(RepositoryPublicationErrorV1::WrongStoreRole);
    }
    let head = view
        .active_head()?
        .ok_or(RepositoryPublicationErrorV1::StaleStoreBasis)?;
    let generation = view
        .active_generation()?
        .ok_or(RepositoryPublicationErrorV1::StaleStoreBasis)?;
    if head.id() != basis.expected_head_id
        || head.generation_id() != basis.expected_generation_id
        || generation.id() != basis.expected_generation_id
        || generation.ordinal() != basis.expected_generation_ordinal
        || generation.contract_root_id() != basis.expected_contract_root_id
    {
        return Err(RepositoryPublicationErrorV1::StaleStoreBasis);
    }
    Ok((head, generation, view.active_generation_objects()?))
}

fn validate_no_op_store_basis(
    store: &StoreV1,
    basis: RepositoryStoreBasisV1,
    exact_roots: &[&StoreObjectV1],
) -> Result<(StoreGenerationV1, Vec<StoreObjectV1>), RepositoryPublicationErrorV1> {
    let (store_state, head, generation, active_objects) = store.coherent_publication_snapshot()?;
    if store_state != StoreStateV1::Active {
        return Err(RepositoryPublicationErrorV1::InactiveStore);
    }
    if head.id() != basis.expected_head_id
        || head.generation_id() != basis.expected_generation_id
        || generation.id() != basis.expected_generation_id
        || generation.ordinal() != basis.expected_generation_ordinal
        || generation.contract_root_id() != basis.expected_contract_root_id
    {
        return Err(RepositoryPublicationErrorV1::StaleStoreBasis);
    }
    for expected in exact_roots {
        if !generation.roots().contains(&expected.id())
            || active_objects
                .iter()
                .filter(|object| object.id() == expected.id())
                .collect::<Vec<_>>()
                .as_slice()
                != [*expected]
        {
            return Err(RepositoryPublicationErrorV1::SubjectBasisMismatch);
        }
    }
    Ok((generation, active_objects))
}

fn validate_rooted_step_state_partition(
    generation: &StoreGenerationV1,
    active_objects: &[StoreObjectV1],
    graph: &StepGraphSnapshotV1,
    expected_objects: &[StoreObjectV1],
) -> Result<(), RepositoryPublicationErrorV1> {
    let expected_ids = expected_objects
        .iter()
        .map(StoreObjectV1::id)
        .collect::<BTreeSet<_>>();
    if expected_ids.len() != expected_objects.len() {
        return Err(RepositoryPublicationErrorV1::ContractStepPublicationMismatch);
    }
    let step_state_schema = RepositoryStoreSchemaV1::StepState.schema_id()?;
    let mut actual_ids = BTreeSet::new();
    for root_id in generation.roots() {
        let mut matches = active_objects
            .iter()
            .filter(|object| object.id() == *root_id);
        let object = matches
            .next()
            .ok_or(RepositoryPublicationErrorV1::ContractStepPublicationMismatch)?;
        if matches.next().is_some() {
            return Err(RepositoryPublicationErrorV1::ContractStepPublicationMismatch);
        }
        if object.schema_id() == step_state_schema
            && step_state_object_matches_graph_contract(object, graph)?
        {
            actual_ids.insert(object.id());
        }
    }
    if actual_ids != expected_ids {
        return Err(RepositoryPublicationErrorV1::ContractStepPublicationMismatch);
    }
    Ok(())
}

fn step_state_object_matches_graph_contract(
    object: &StoreObjectV1,
    graph: &StepGraphSnapshotV1,
) -> Result<bool, RepositoryPublicationErrorV1> {
    let CborValue::Array(fields) = object.value() else {
        return Err(RepositoryPublicationErrorV1::ContractStepPublicationMismatch);
    };
    let [
        CborValue::Text(domain),
        CborValue::Array(binding),
        _lifecycle,
    ] = fields.as_slice()
    else {
        return Err(RepositoryPublicationErrorV1::ContractStepPublicationMismatch);
    };
    if domain != REPOSITORY_STEP_STATE_DOMAIN_V1 {
        return Err(RepositoryPublicationErrorV1::ContractStepPublicationMismatch);
    }
    let [
        CborValue::Bytes(repository_id),
        CborValue::Bytes(work_id),
        CborValue::Bytes(generation_id),
        CborValue::Bytes(root_id),
        CborValue::Bytes(step_id),
        CborValue::Bytes(revision_id),
    ] = binding.as_slice()
    else {
        return Err(RepositoryPublicationErrorV1::ContractStepPublicationMismatch);
    };
    if [
        repository_id,
        work_id,
        generation_id,
        root_id,
        step_id,
        revision_id,
    ]
    .into_iter()
    .any(|bytes| bytes.len() != 32)
    {
        return Err(RepositoryPublicationErrorV1::ContractStepPublicationMismatch);
    }
    Ok(
        repository_id.as_slice() == graph.scope().repository_id().as_bytes()
            && work_id.as_slice() == graph.scope().work_id().as_bytes()
            && generation_id.as_slice() == graph.contract_generation_id().as_bytes()
            && root_id.as_slice() == graph.contract_root_id().as_bytes(),
    )
}

fn require_exact_rooted_object(
    generation: &StoreGenerationV1,
    active_objects: &[StoreObjectV1],
    expected: &StoreObjectV1,
) -> Result<(), RepositoryPublicationErrorV1> {
    if !generation.roots().contains(&expected.id())
        || active_objects
            .iter()
            .filter(|object| object.id() == expected.id())
            .collect::<Vec<_>>()
            .as_slice()
            != [expected]
    {
        return Err(RepositoryPublicationErrorV1::SubjectBasisMismatch);
    }
    Ok(())
}

fn exact_admitted_transition_guard<'a>(
    active_objects: &'a [StoreObjectV1],
    basis_object: &StoreObjectV1,
) -> Result<&'a StoreObjectV1, RepositoryPublicationErrorV1> {
    let references = basis_object
        .references()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let matches = active_objects
        .iter()
        .filter(|object| references.contains(&object.id()))
        .filter(|object| {
            matches!(
                object.value(),
                CborValue::Array(fields)
                    if matches!(fields.first(), Some(CborValue::Text(domain))
                        if domain == "maestro.vnext.authority-transition-guard-evaluation.v1")
            )
        })
        .collect::<Vec<_>>();
    let [guard] = matches.as_slice() else {
        return Err(RepositoryPublicationErrorV1::InvalidAdmittedTransitionGuard);
    };
    Ok(*guard)
}

fn prepare_publication<P: SealedRepositoryActionPlanV1>(
    view: &StorePublicationViewV1<'_>,
    publication: P,
) -> Result<AtomicGenerationPublicationV1, RepositoryPublicationErrorV1> {
    if view.role() != StoreRoleV1::Repository {
        return Err(RepositoryPublicationErrorV1::WrongStoreRole);
    }
    let current_head = view
        .active_head()?
        .ok_or(RepositoryPublicationErrorV1::StaleStoreBasis)?;
    let current_generation = view
        .active_generation()?
        .ok_or(RepositoryPublicationErrorV1::StaleStoreBasis)?;
    let basis = publication.store_basis();
    if current_head.id() != basis.expected_head_id
        || current_head.generation_id() != basis.expected_generation_id
        || current_generation.id() != basis.expected_generation_id
        || current_generation.ordinal() != basis.expected_generation_ordinal
        || current_generation.contract_root_id() != basis.expected_contract_root_id
    {
        return Err(RepositoryPublicationErrorV1::StaleStoreBasis);
    }
    let active_objects = view.active_generation_objects()?;
    publication.validate_current_subject(&current_generation, &active_objects)?;

    let additional_effects =
        publication.additional_store_effects(view, &current_generation, &active_objects)?;
    let admission_input = match publication.authority_leaf() {
        RepositoryActionLeafV1::CreateDraftWork => RepositoryActionAdmissionInputV1::new(
            publication.identity().request_id,
            CreateDraftWorkAuthorityV1::new(
                publication.authority(),
                publication.subject_commitment(),
                publication.subject_basis_commitment(),
            )?,
        ),
        RepositoryActionLeafV1::CancelWork => RepositoryActionAdmissionInputV1::new(
            publication.identity().request_id,
            CancelWorkAuthorityV1::new(
                publication.authority(),
                publication.subject_commitment(),
                publication.subject_basis_commitment(),
            )?,
        ),
        RepositoryActionLeafV1::SubmitWorkCompletion => RepositoryActionAdmissionInputV1::new(
            publication.identity().request_id,
            SubmitWorkCompletionAuthorityV1::new(
                publication.authority(),
                publication.subject_commitment(),
                publication.subject_basis_commitment(),
            )?,
        ),
        RepositoryActionLeafV1::AppendDesignRevision => RepositoryActionAdmissionInputV1::new(
            publication.identity().request_id,
            AppendDesignRevisionAuthorityV1::new(
                publication.authority(),
                publication.subject_commitment(),
                publication.subject_basis_commitment(),
            )?,
        ),
        _ => {
            return Err(RepositoryPublicationErrorV1::UnsupportedRepositoryAction);
        }
    };
    let admission = admit_repository_action(view, &current_generation, admission_input)?;
    if admission.request_id() != publication.identity().request_id {
        return Err(RepositoryPublicationErrorV1::AuthorityRequestMismatch);
    }
    publication.validate_admission(&admission)?;
    let mut produced_objects = publication.produced_objects();
    produced_objects.extend(additional_effects.produced_objects);
    let artifacts =
        admission.issue_committed_artifacts(publication.request_object(), &produced_objects)?;
    let mut roots = current_generation.roots().to_vec();
    let mut root_replacements = publication.root_replacements();
    root_replacements.extend(additional_effects.root_replacements);
    for (current, successor) in root_replacements {
        replace_root(&mut roots, current, successor)?;
    }
    roots.extend(publication.added_roots());
    replace_root(
        &mut roots,
        admission.current_snapshot_id(),
        admission.successor_snapshot().id(),
    )?;
    replace_root_if_direct(
        &mut roots,
        admission.current_capacity_root_id(),
        admission.successor_capacity_root().id(),
    )?;
    roots.push(artifacts.result_object().id());
    roots.sort_unstable();
    roots.dedup();

    let next_ordinal = current_generation
        .ordinal()
        .checked_add(1)
        .ok_or(RepositoryPublicationErrorV1::StaleStoreBasis)?;
    let generation = StoreGenerationV1::new(
        view.domain().clone(),
        next_ordinal,
        Some(current_generation.id()),
        current_generation.contract_root_id(),
        StoreCompatibilityV1::stage0_successor()?,
        roots,
    )?;
    let idempotency = StoreIdempotencyV1::new(
        REPOSITORY_ACTION_IDEMPOTENCY_NAMESPACE_V1,
        *publication.identity().idempotency_key.as_bytes(),
        publication.meaning_digest(),
        artifacts.result_object().id(),
    )?;
    let mut objects = active_objects;
    objects.extend(produced_objects);
    objects.extend([
        publication.request_object().clone(),
        admission.basis_object().clone(),
        admission.successor_snapshot().clone(),
        admission.successor_capacity_root().clone(),
        admission.capacity_debit().clone(),
        artifacts.receipt_object().clone(),
        artifacts.result_object().clone(),
    ]);
    objects.extend(artifacts.leaf_authority_objects().iter().cloned());
    objects.sort_by_key(StoreObjectV1::id);
    objects.dedup_by_key(|object| object.id());
    if artifacts.logical_result().request_id() != publication.identity().request_id {
        return Err(RepositoryPublicationErrorV1::AuthorityRequestMismatch);
    }
    Ok(AtomicGenerationPublicationV1::new_from_object_superset(
        generation,
        Some(current_head.id()),
        objects,
        idempotency,
    )?)
}

fn publication_outcome(
    outcome: StorePublicationOutcomeV1,
    expected_request_id: ActionRequestIdV1,
) -> Result<RepositoryPublicationOutcomeV1, RepositoryPublicationErrorV1> {
    let (kind, head, result) = match outcome {
        StorePublicationOutcomeV1::Committed { head, result } => {
            (RepositoryPublicationKindV1::Committed, head, result)
        }
        StorePublicationOutcomeV1::Replayed { head, result } => {
            (RepositoryPublicationKindV1::Replayed, head, result)
        }
    };
    let CborValue::Array(fields) = result.value() else {
        return Err(RepositoryPublicationErrorV1::InvalidPublishedResult);
    };
    if fields.len() != 11
        || exact_digest(&fields[1])? != *expected_request_id.as_bytes()
        || fields[2] != CborValue::Unsigned(1)
        || fields[3] != CborValue::Unsigned(1)
        || !matches!(&fields[6], CborValue::Array(values) if values.len() == 1)
        || !matches!(&fields[7], CborValue::Array(values) if !values.is_empty())
        || !matches!(&fields[8], CborValue::Array(values) if values.is_empty())
    {
        return Err(RepositoryPublicationErrorV1::InvalidPublishedResult);
    }
    let logical_result_id = ActionResultIdV1::from_digest(exact_digest(&fields[0])?);
    Ok(RepositoryPublicationOutcomeV1 {
        kind,
        head,
        result,
        logical_result_id,
    })
}

fn replace_root(
    roots: &mut [StoreObjectIdV1],
    expected: StoreObjectIdV1,
    replacement: StoreObjectIdV1,
) -> Result<(), RepositoryPublicationErrorV1> {
    let matches = roots.iter().filter(|root| **root == expected).count();
    if matches != 1 {
        return Err(RepositoryPublicationErrorV1::SubjectBasisMismatch);
    }
    let root = roots
        .iter_mut()
        .find(|root| **root == expected)
        .expect("invariant: exact one-element root match");
    *root = replacement;
    Ok(())
}

fn replace_root_if_direct(
    roots: &mut [StoreObjectIdV1],
    expected: StoreObjectIdV1,
    replacement: StoreObjectIdV1,
) -> Result<(), RepositoryPublicationErrorV1> {
    if roots.contains(&expected) {
        replace_root(roots, expected, replacement)?;
    }
    Ok(())
}

fn action_request_object(
    kind: RepositoryActionKindV1,
    identity: RepositoryActionIdentityV1,
    store_basis: RepositoryStoreBasisV1,
    authority: RepositoryAuthoritySelectionV1,
    subject_commitment: [u8; 32],
    subject_basis_commitment: [u8; 32],
    typed_input: CborValue,
) -> Result<StoreObjectV1, RepositoryPublicationErrorV1> {
    StoreObjectV1::new(
        repository_schema_id(REPOSITORY_ACTION_REQUEST_DOMAIN_V1)?,
        CborValue::Array(vec![
            CborValue::text(REPOSITORY_ACTION_REQUEST_DOMAIN_V1)?,
            bytes(identity.request_id.as_bytes()),
            bytes(identity.idempotency_key.as_bytes()),
            CborValue::Unsigned(kind.tag()),
            bytes(store_basis.expected_head_id.as_bytes()),
            bytes(store_basis.expected_generation_id.as_bytes()),
            CborValue::Unsigned(store_basis.expected_generation_ordinal),
            bytes(store_basis.expected_contract_root_id.as_bytes()),
            bytes(&subject_commitment),
            bytes(&subject_basis_commitment),
            bytes(authority.actor_binding_id().as_bytes()),
            bytes(authority.actor_session_id().as_bytes()),
            bytes(authority.terminal_grant_id().as_bytes()),
            typed_input,
        ]),
        vec![],
    )
    .map_err(Into::into)
}

fn step_graph_object(
    graph: &StepGraphSnapshotV1,
) -> Result<StoreObjectV1, RepositoryPublicationErrorV1> {
    StoreObjectV1::new(
        RepositoryStoreSchemaV1::StepGraph.schema_id()?,
        CborValue::Array(vec![
            CborValue::text(REPOSITORY_STEP_GRAPH_DOMAIN_V1)?,
            CborValue::Bytes(graph.canonical_bytes()?),
        ]),
        vec![],
    )
    .map_err(Into::into)
}

fn decision_materialization_audit_object(
    materialization: &DecisionMaterializationV1,
) -> Result<StoreObjectV1, RepositoryPublicationErrorV1> {
    StoreObjectV1::new(
        RepositoryStoreSchemaV1::DecisionMaterializationAudit.schema_id()?,
        CborValue::Array(vec![
            CborValue::text(REPOSITORY_DECISION_MATERIALIZATION_AUDIT_DOMAIN_V1)?,
            CborValue::Bytes(materialization.canonical_bytes()?),
        ]),
        vec![],
    )
    .map_err(Into::into)
}

fn step_amendment_audit_object(
    plan: &StepAmendmentPlanV1,
    applied: &AppliedStepAmendmentV1,
    current_graph: &StoreObjectV1,
    candidate_graph: &StoreObjectV1,
    materialization_audits: &[StoreObjectV1],
    historical_states: &[StoreObjectV1],
    next_states: &[StoreObjectV1],
) -> Result<StoreObjectV1, RepositoryPublicationErrorV1> {
    let conservation = applied.obligation_conservation();
    let mut references = vec![current_graph.id(), candidate_graph.id()];
    references.extend(materialization_audits.iter().map(StoreObjectV1::id));
    references.extend(historical_states.iter().map(StoreObjectV1::id));
    references.extend(next_states.iter().map(StoreObjectV1::id));
    StoreObjectV1::new(
        RepositoryStoreSchemaV1::StepAmendmentAudit.schema_id()?,
        CborValue::Array(vec![
            CborValue::text(REPOSITORY_STEP_AMENDMENT_AUDIT_DOMAIN_V1)?,
            CborValue::Bytes(plan.canonical_bytes()?),
            CborValue::Unsigned(conservation.current_obligation_count() as u64),
            CborValue::Unsigned(conservation.candidate_obligation_count() as u64),
            CborValue::Unsigned(conservation.retain_exact_count() as u64),
            CborValue::Unsigned(conservation.replace_count() as u64),
            CborValue::Unsigned(conservation.remove_count() as u64),
            CborValue::Unsigned(conservation.add_count() as u64),
            CborValue::Array(
                materialization_audits
                    .iter()
                    .map(|object| bytes(object.id().as_bytes()))
                    .collect(),
            ),
            CborValue::Array(
                historical_states
                    .iter()
                    .map(|object| bytes(object.id().as_bytes()))
                    .collect(),
            ),
            CborValue::Array(
                next_states
                    .iter()
                    .map(|object| bytes(object.id().as_bytes()))
                    .collect(),
            ),
        ]),
        sorted_references(references),
    )
    .map_err(Into::into)
}

fn step_state_object(step: &StepStateV1) -> Result<StoreObjectV1, RepositoryPublicationErrorV1> {
    StoreObjectV1::new(
        repository_schema_id(REPOSITORY_STEP_STATE_DOMAIN_V1)?,
        CborValue::Array(vec![
            CborValue::text(REPOSITORY_STEP_STATE_DOMAIN_V1)?,
            step_binding_value(step.binding()),
            step_lifecycle_value(step.lifecycle()),
        ]),
        vec![],
    )
    .map_err(Into::into)
}

fn step_binding_value(binding: StepBindingV1) -> CborValue {
    CborValue::Array(vec![
        bytes(binding.scope().repository_id().as_bytes()),
        bytes(binding.scope().work_id().as_bytes()),
        bytes(binding.contract_generation_id().as_bytes()),
        bytes(binding.contract_root_id().as_bytes()),
        bytes(binding.step_id().as_bytes()),
        bytes(binding.revision_id().as_bytes()),
    ])
}

fn step_lifecycle_value(lifecycle: StepLifecycleV1) -> CborValue {
    match lifecycle {
        StepLifecycleV1::Open { basis } => CborValue::Array(vec![
            CborValue::Unsigned(1),
            match basis {
                StepOpenBasisV1::Fresh => CborValue::Array(vec![CborValue::Unsigned(1)]),
                StepOpenBasisV1::RejectedSubmission {
                    submission_id,
                    submission_record_hash,
                    rejection_receipt_hash,
                } => CborValue::Array(vec![
                    CborValue::Unsigned(2),
                    bytes(submission_id.as_bytes()),
                    bytes(&submission_record_hash),
                    bytes(&rejection_receipt_hash),
                ]),
                StepOpenBasisV1::RecoveredSubmission {
                    submission_id,
                    submission_record_hash,
                    recovery_receipt_hash,
                } => CborValue::Array(vec![
                    CborValue::Unsigned(3),
                    bytes(submission_id.as_bytes()),
                    bytes(&submission_record_hash),
                    bytes(&recovery_receipt_hash),
                ]),
            },
        ]),
        StepLifecycleV1::Submitted {
            submission_id,
            submission_record_hash,
        } => CborValue::Array(vec![
            CborValue::Unsigned(2),
            bytes(submission_id.as_bytes()),
            bytes(&submission_record_hash),
        ]),
        StepLifecycleV1::Satisfied {
            submission_record_hash,
            satisfaction_basis_hash,
        } => CborValue::Array(vec![
            CborValue::Unsigned(3),
            bytes(&submission_record_hash),
            bytes(&satisfaction_basis_hash),
        ]),
        StepLifecycleV1::Cancelled {
            amendment_receipt_hash,
        } => CborValue::Array(vec![CborValue::Unsigned(4), bytes(&amendment_receipt_hash)]),
        StepLifecycleV1::Superseded {
            successor,
            amendment_receipt_hash,
        } => CborValue::Array(vec![
            CborValue::Unsigned(5),
            step_binding_value(successor),
            bytes(&amendment_receipt_hash),
        ]),
    }
}

fn work_subject_commitment(work_id: WorkIdV1) -> Result<[u8; 32], CborError> {
    hash(&CborValue::Array(vec![
        CborValue::text("maestro.vnext.repository-work-subject.v1")?,
        bytes(work_id.as_bytes()),
    ]))
}

fn contract_revision_object(
    revision: &ContractRevisionV1,
) -> Result<StoreObjectV1, RepositoryPublicationErrorV1> {
    StoreObjectV1::new(
        repository_schema_id(REPOSITORY_CONTRACT_REVISION_DOMAIN_V1)?,
        CborValue::Bytes(revision.canonical_bytes()?),
        vec![],
    )
    .map_err(Into::into)
}

fn contract_generation_object(
    generation: &ContractGenerationV1,
) -> Result<StoreObjectV1, RepositoryPublicationErrorV1> {
    StoreObjectV1::new(
        repository_schema_id(REPOSITORY_CONTRACT_GENERATION_DOMAIN_V1)?,
        CborValue::Bytes(generation.canonical_bytes()?),
        vec![],
    )
    .map_err(Into::into)
}

fn finalization_manifest_object(
    manifest: &DesignFinalizationManifestV1,
) -> Result<StoreObjectV1, RepositoryPublicationErrorV1> {
    let canonical = manifest
        .canonical_bytes()
        .map_err(|_| RepositoryPublicationErrorV1::ContractPublicationBasisMismatch)?;
    StoreObjectV1::new(
        repository_schema_id(REPOSITORY_FINALIZATION_MANIFEST_DOMAIN_V1)?,
        CborValue::Bytes(canonical),
        vec![],
    )
    .map_err(Into::into)
}

fn contract_root_object(
    root: &CandidateContractRootV1,
) -> Result<StoreObjectV1, RepositoryPublicationErrorV1> {
    let canonical = root
        .canonical_bytes()
        .map_err(|_| RepositoryPublicationErrorV1::ContractPublicationBasisMismatch)?;
    StoreObjectV1::new(
        repository_schema_id(REPOSITORY_CONTRACT_ROOT_DOMAIN_V1)?,
        CborValue::Bytes(canonical),
        vec![],
    )
    .map_err(Into::into)
}

fn decision_object(decision: &DecisionV1) -> Result<StoreObjectV1, RepositoryPublicationErrorV1> {
    let revisions = decision
        .revisions()
        .iter()
        .map(|revision| {
            revision
                .canonical_bytes()
                .map(CborValue::Bytes)
                .map_err(RepositoryPublicationErrorV1::from)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let state = match decision.state() {
        DecisionStateV1::Open => CborValue::Array(vec![CborValue::Unsigned(1)]),
        DecisionStateV1::Resolved(resolution) => CborValue::Array(vec![
            CborValue::Unsigned(2),
            CborValue::Bytes(resolution.canonical_bytes()?),
        ]),
        DecisionStateV1::Withdrawn(_) | DecisionStateV1::Superseded { .. } => {
            return Err(RepositoryPublicationErrorV1::DecisionResolutionBasisMismatch);
        }
    };
    StoreObjectV1::new(
        repository_schema_id(REPOSITORY_DECISION_DOMAIN_V1)?,
        CborValue::Array(vec![
            CborValue::text(REPOSITORY_DECISION_DOMAIN_V1)?,
            bytes(decision.repository_installation_id().as_bytes()),
            bytes(decision.work_id().as_bytes()),
            CborValue::Text(decision.decision_id().as_str().to_owned()),
            CborValue::Array(revisions),
            state,
        ]),
        vec![],
    )
    .map_err(Into::into)
}

fn exact_rooted_object_by_id<'a>(
    generation: &StoreGenerationV1,
    active_objects: &'a [StoreObjectV1],
    object_id: StoreObjectIdV1,
) -> Result<&'a StoreObjectV1, RepositoryPublicationErrorV1> {
    let matches = active_objects
        .iter()
        .filter(|object| object.id() == object_id)
        .collect::<Vec<_>>();
    let [object] = matches.as_slice() else {
        return Err(RepositoryPublicationErrorV1::MaterializationReceiptMismatch);
    };
    if !generation.roots().contains(&object_id) {
        return Err(RepositoryPublicationErrorV1::MaterializationReceiptMismatch);
    }
    Ok(*object)
}

fn exact_equivalence_receipt_object(
    decision: &DecisionV1,
    preflight: &DecisionMaterializationPreflightV1,
    decision_object: &StoreObjectV1,
    base_root_object: &StoreObjectV1,
    candidate_root_object: &StoreObjectV1,
) -> Result<StoreObjectV1, RepositoryPublicationErrorV1> {
    let resolution = decision
        .resolution()
        .ok_or(RepositoryPublicationErrorV1::MaterializationBasisMismatch)?;
    let base_semantics = contract_semantic_equivalence_digest(preflight.base_root())?;
    let candidate_semantics = contract_semantic_equivalence_digest(preflight.candidate_root())?;
    if preflight.is_equal_root() || base_semantics != candidate_semantics {
        return Err(RepositoryPublicationErrorV1::MaterializationReceiptMismatch);
    }
    let evaluator_id = hash(&CborValue::Array(vec![
        CborValue::text(REPOSITORY_EXACT_EQUIVALENCE_EVALUATOR_DOMAIN_V1)?,
        CborValue::Unsigned(1),
    ]))?;
    let proof_input_commitment = hash(&CborValue::Array(vec![
        CborValue::text("maestro.vnext.repository-exact-equivalence-input.v1")?,
        bytes(decision_object.id().as_bytes()),
        bytes(base_root_object.id().as_bytes()),
        bytes(candidate_root_object.id().as_bytes()),
        bytes(resolution.resolution_id().as_bytes()),
    ]))?;
    StoreObjectV1::new(
        RepositoryStoreSchemaV1::ExactEquivalenceReceipt.schema_id()?,
        CborValue::Array(vec![
            CborValue::text(REPOSITORY_EXACT_EQUIVALENCE_RECEIPT_DOMAIN_V1)?,
            CborValue::Unsigned(1),
            bytes(&evaluator_id),
            CborValue::text(REPOSITORY_EXACT_EQUIVALENCE_PURPOSE_V1)?,
            bytes(preflight.base_root().root_id().as_bytes()),
            bytes(preflight.candidate_root().root_id().as_bytes()),
            bytes(resolution.resolution_id().as_bytes()),
            bytes(&proof_input_commitment),
            bytes(&base_semantics),
        ]),
        sorted_references(vec![
            decision_object.id(),
            base_root_object.id(),
            candidate_root_object.id(),
        ]),
    )
    .map_err(Into::into)
}

fn contract_semantic_equivalence_digest(
    root: &CandidateContractRootV1,
) -> Result<[u8; 32], RepositoryPublicationErrorV1> {
    let mut component_semantics = BTreeMap::new();
    let mut root_members = Vec::with_capacity(root.components().len());
    for component in root.components() {
        let mut dependency_semantics = component
            .dependencies()
            .iter()
            .map(|dependency| {
                component_semantics
                    .get(dependency)
                    .copied()
                    .ok_or(RepositoryPublicationErrorV1::MaterializationReceiptMismatch)
            })
            .collect::<Result<Vec<[u8; 32]>, _>>()?;
        dependency_semantics.sort_unstable();
        let semantic_digest = hash(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.contract-component-semantic-equivalence.v1")?,
            CborValue::Unsigned(component.kind().tag()),
            bytes(component.schema_id().as_bytes()),
            component.value().clone(),
            CborValue::Array(
                dependency_semantics
                    .iter()
                    .map(|dependency| bytes(dependency))
                    .collect(),
            ),
        ]))?;
        component_semantics.insert(*component.component_id(), semantic_digest);
        root_members.push((component.kind().tag(), semantic_digest));
    }
    root_members.sort_unstable();
    hash(&CborValue::Array(vec![
        CborValue::text("maestro.vnext.contract-root-semantic-equivalence.v1")?,
        CborValue::Array(
            root_members
                .iter()
                .map(|(kind, digest)| {
                    CborValue::Array(vec![CborValue::Unsigned(*kind), bytes(digest)])
                })
                .collect(),
        ),
    ]))
    .map_err(Into::into)
}

fn validate_candidate_materialization_receipts(
    generation: &StoreGenerationV1,
    active_objects: &[StoreObjectV1],
    candidate: &DecisionMaterializationCandidateV1,
) -> Result<(), RepositoryPublicationErrorV1> {
    let resolution = candidate
        .resolved_decision
        .resolution()
        .ok_or(RepositoryPublicationErrorV1::MaterializationBasisMismatch)?;
    let removed = candidate
        .invalidation_receipts
        .iter()
        .map(|(component, _)| *component)
        .collect::<Vec<_>>();
    if removed != candidate.preflight.delta().removed() {
        return Err(RepositoryPublicationErrorV1::MaterializationReceiptMismatch);
    }
    for (component_id, receipt_id) in &candidate.invalidation_receipts {
        let receipt = exact_rooted_object_by_id(generation, active_objects, *receipt_id)?;
        if receipt.schema_id()
            != RepositoryStoreSchemaV1::ComponentInvalidationReceipt.schema_id()?
            || !receipt.references().is_empty()
            || receipt.value()
                != &CborValue::Array(vec![
                    CborValue::text(REPOSITORY_COMPONENT_INVALIDATION_RECEIPT_DOMAIN_V1)?,
                    bytes(component_id.as_bytes()),
                    bytes(candidate.preflight.base_root().root_id().as_bytes()),
                    bytes(candidate.preflight.candidate_root().root_id().as_bytes()),
                    bytes(resolution.resolution_id().as_bytes()),
                ])
        {
            return Err(RepositoryPublicationErrorV1::MaterializationReceiptMismatch);
        }
    }
    Ok(())
}

fn contract_publication_basis_commitment(
    current_work: &StoreObjectV1,
    current_generation: Option<&StoreObjectV1>,
    current_root: Option<&StoreObjectV1>,
    revision: &StoreObjectV1,
    finalization: &StoreObjectV1,
    candidate_root: &StoreObjectV1,
) -> Result<[u8; 32], RepositoryPublicationErrorV1> {
    Ok(hash(&CborValue::Array(vec![
        CborValue::text("maestro.vnext.repository-contract-publication-basis.v1")?,
        bytes(current_work.id().as_bytes()),
        CborValue::optional(current_generation.map(|object| bytes(object.id().as_bytes()))),
        CborValue::optional(current_root.map(|object| bytes(object.id().as_bytes()))),
        bytes(revision.id().as_bytes()),
        bytes(finalization.id().as_bytes()),
        bytes(candidate_root.id().as_bytes()),
    ]))?)
}

fn repository_policy_snapshot(
    root: &CandidateContractRootV1,
) -> Result<RepositoryPolicySnapshotV1, RepositoryPublicationErrorV1> {
    let component = |kind| {
        root.components()
            .iter()
            .find(|component| component.kind() == kind)
            .map(|component| *component.component_id().as_bytes())
            .ok_or(RepositoryPublicationErrorV1::ContractPublicationBasisMismatch)
    };
    let components = RepositoryPolicyComponentSetV1::new(
        component(ContractComponentKindV1::GateSnapshot)?,
        component(ContractComponentKindV1::PolicyProfileProvenance)?,
        component(ContractComponentKindV1::PublicationAuthorityRequirement)?,
        component(ContractComponentKindV1::CompletionAuthorityRequirement)?,
        component(ContractComponentKindV1::StageProofMatrix)?,
    )?;
    Ok(RepositoryPolicySnapshotV1::new(
        *root.root_id().as_bytes(),
        components,
        RepositoryPolicyStrengthV1::stage3_strict(),
    )?)
}

fn work_record_object(work: &WorkRecordV1) -> Result<StoreObjectV1, RepositoryPublicationErrorV1> {
    let histories = work
        .history()
        .iter()
        .map(|fact| fact.canonical_bytes().map(CborValue::Bytes))
        .collect::<Result<Vec<_>, _>>()?;
    let submissions = work
        .submissions()
        .iter()
        .map(|submission| submission.canonical_bytes().map(CborValue::Bytes))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            RepositoryPublicationErrorV1::WorkRecordSerialization(error.to_string())
        })?;
    StoreObjectV1::new(
        repository_schema_id(REPOSITORY_WORK_RECORD_DOMAIN_V1)?,
        CborValue::Array(vec![
            CborValue::text(REPOSITORY_WORK_RECORD_DOMAIN_V1)?,
            bytes(work.id().as_bytes()),
            CborValue::Unsigned(work.revision().get()),
            work_state_value(work.state()),
            CborValue::Array(histories),
            CborValue::Array(submissions),
            CborValue::optional(
                work.current_submission()
                    .map(|submission| bytes(submission.id().as_bytes())),
            ),
        ]),
        vec![],
    )
    .map_err(Into::into)
}

fn design_stream_object(
    design: &DesignStreamV1,
) -> Result<StoreObjectV1, RepositoryPublicationErrorV1> {
    let revisions = design
        .revisions()
        .iter()
        .map(|revision| revision.canonical_bytes().map(CborValue::Bytes))
        .collect::<Result<Vec<_>, _>>()?;
    let head = design.candidate_head();
    StoreObjectV1::new(
        repository_schema_id(REPOSITORY_DESIGN_STREAM_DOMAIN_V1)?,
        CborValue::Array(vec![
            CborValue::text(REPOSITORY_DESIGN_STREAM_DOMAIN_V1)?,
            bytes(head.repository_installation_id().as_bytes()),
            bytes(head.work_id().as_bytes()),
            bytes(head.revision_id().as_bytes()),
            CborValue::Array(revisions),
        ]),
        vec![],
    )
    .map_err(Into::into)
}

fn work_state_value(state: &WorkLifecycleStateV1) -> CborValue {
    let successor = match state {
        WorkLifecycleStateV1::Superseded { successor } => Some(bytes(successor.as_bytes())),
        _ => None,
    };
    CborValue::Array(vec![
        CborValue::Unsigned(state.tag()),
        CborValue::optional(successor),
    ])
}

fn repository_schema_id(domain: &str) -> Result<SchemaIdV1, RepositoryPublicationErrorV1> {
    let value = CborValue::Array(vec![
        CborValue::text("maestro.vnext.repository-runtime-schema.v1")?,
        CborValue::text(domain)?,
    ]);
    SchemaIdV1::parse(&render_digest(hash(&value)?)).map_err(Into::into)
}

fn exact_digest(value: &CborValue) -> Result<[u8; 32], RepositoryPublicationErrorV1> {
    let CborValue::Bytes(bytes) = value else {
        return Err(RepositoryPublicationErrorV1::InvalidPublishedResult);
    };
    bytes
        .as_slice()
        .try_into()
        .map_err(|_| RepositoryPublicationErrorV1::InvalidPublishedResult)
}

fn hash(value: &CborValue) -> Result<[u8; 32], CborError> {
    Ok(Sha256::digest(deterministic_cbor::encode(value)?).into())
}

fn bytes(value: &[u8]) -> CborValue {
    CborValue::Bytes(value.to_vec())
}

fn sorted_references(mut references: Vec<StoreObjectIdV1>) -> Vec<StoreObjectIdV1> {
    references.sort_unstable();
    references.dedup();
    references
}

fn render_digest(value: [u8; 32]) -> String {
    let mut rendered = String::with_capacity(71);
    rendered.push_str("sha256:");
    for byte in value {
        use std::fmt::Write;
        write!(&mut rendered, "{byte:02x}")
            .expect("invariant: writing hexadecimal into String cannot fail");
    }
    rendered
}

#[derive(Debug, Error)]
pub enum RepositoryPublicationErrorV1 {
    #[error("Repository action publication requires an active Store")]
    InactiveStore,
    #[error("Repository actions require a Repository-role Store")]
    WrongStoreRole,
    #[error("Repository Store basis must bind a positive exact Generation")]
    InvalidStoreBasis,
    #[error("Repository Store Head, Generation, or Contract Root changed before admission")]
    StaleStoreBasis,
    #[error("the action-specific Work, Design, Contract, Step, or Decision basis is stale")]
    SubjectBasisMismatch,
    #[error("the Repository action is outside the implemented Stage 3 mutation surface")]
    UnsupportedRepositoryAction,
    #[error("the Repository subject already exists")]
    SubjectAlreadyExists,
    #[error(
        "Work completion does not bind the exact current Work, Contract Generation/root, and satisfied Step Submission closure"
    )]
    WorkCompletionBasisMismatch,
    #[error(
        "Contract publication does not bind the exact Work, Generation, Revision, manifest, and root"
    )]
    ContractPublicationBasisMismatch,
    #[error("Contract publication does not close over the exact generation-scoped Step graph")]
    ContractStepPublicationMismatch,
    #[error("Repository admission did not retain one exact transition-guard object")]
    InvalidAdmittedTransitionGuard,
    #[error("Decision resolution does not bind the exact open Decision head and nonterminal Work")]
    DecisionResolutionBasisMismatch,
    #[error("Decision Materialization does not bind the exact resolved Decision and current root")]
    MaterializationBasisMismatch,
    #[error(
        "Decision Materialization receipts do not exactly bind its removed components or equivalent roots"
    )]
    MaterializationReceiptMismatch,
    #[error("Authority admission does not bind the exact Repository action request")]
    AuthorityRequestMismatch,
    #[error("the published Repository Action Result carrier is invalid")]
    InvalidPublishedResult,
    #[error("Work Record serialization failed: {0}")]
    WorkRecordSerialization(String),
    #[error("Repository Authority admission failed: {0}")]
    Authority(String),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    StoreObject(#[from] StoreObjectError),
    #[error(transparent)]
    Generation(#[from] GenerationError),
    #[error(transparent)]
    AtomicPublication(#[from] AtomicPublicationError),
    #[error(transparent)]
    Work(#[from] WorkLifecycleError),
    #[error(transparent)]
    WorkSubmission(#[from] WorkSubmissionError),
    #[error(transparent)]
    Evidence(#[from] EvidenceStoreErrorV1),
    #[error(transparent)]
    Claim(#[from] ClaimError),
    #[error(transparent)]
    Gate(#[from] GateError),
    #[error(transparent)]
    StepSubmission(#[from] StepSubmissionErrorV1),
    #[error(transparent)]
    StepLifecycle(#[from] StepLifecycleError),
    #[error(transparent)]
    StepIdentity(#[from] crate::domain::step::StepIdentityError),
    #[error(transparent)]
    StepAmendment(#[from] StepAmendmentError),
    #[error(transparent)]
    StepGraph(#[from] StepGraphError),
    #[error(transparent)]
    Design(#[from] DesignV1Error),
    #[error(transparent)]
    Decision(#[from] DecisionV1Error),
    #[error(transparent)]
    CommittedActionAdmission(#[from] CommittedActionAdmissionErrorV1),
    #[error(transparent)]
    Materialization(#[from] MaterializationV1Error),
    #[error(transparent)]
    ContractRuntime(#[from] ContractRuntimeError),
    #[error(transparent)]
    Identity(#[from] crate::domain::identity::IdentityError),
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error(transparent)]
    RepositoryLeafAuthority(#[from] RepositoryLeafAuthorityErrorV1),
}

impl From<RepositoryAuthorityAdmissionErrorV1> for RepositoryPublicationErrorV1 {
    fn from(error: RepositoryAuthorityAdmissionErrorV1) -> Self {
        Self::Authority(error.to_string())
    }
}

#[cfg(test)]
mod tests;
