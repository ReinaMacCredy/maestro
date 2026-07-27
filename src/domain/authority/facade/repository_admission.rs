use std::collections::BTreeSet;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::identity::{SchemaIdV1, StoreObjectIdV1, derive_identity};
use crate::domain::persistence::{
    StoreGenerationV1, StoreObjectError, StoreObjectV1, StorePublicationViewV1, StoreRoleV1,
};
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::super::publication::AuthoritySchemaV1;
use super::super::{
    ActionAuthorityBasisKindV1, ActionOutcomeV1, ActionRequestIdV1, ActionResultError,
    ActionResultV1, AuthorityContextIdV1, AuthorityContextKindV1, AuthorityContinuityManifestV1,
    AuthorityValidationError, AuthorizationReceiptIdV1, AuthorizationReceiptV1,
    BootstrapAuthoritySnapshotErrorV1, BootstrapAuthoritySnapshotV1, CapacityRootIdV1,
    CapacityUseDispositionV1, CmaBranchIdV1, CmaEffectWithdrawalSlotFamilyV1,
    CmaObservationPublicationPurposeV1, DelegationAncestryV1, EffectReferenceIdV1,
    ExecutorAssertionIdV1, GovernedCapacityKindV1, GovernedCapacityRootV1, GrantIdV1,
    InstallationGovernedCapacitySlotKindV1, OrdinaryBoundedGrantV1, OrdinaryGrantDelegationV1,
    PrincipalIdV1, RepositoryActionLeafV1, RepositoryGovernedCapacitySlotKindV1,
    RevocationTargetV1, ScopeAtomV1, SlotIdV1, StateTokenIdV1,
    SuccessVisibleAuthorityContinuityStateV1, TrustedTimeV1, grant_is_revoked_by_closure,
    validate_delegation, validate_ordinary_authority,
};
use super::repository_leaf_authority::{
    BootstrapExecutionAuthorityV1, ContinuityMaintenanceExecutionAuthorityV1, ExecutionAuthorityV1,
    RepositoryAuthoritySelectionV1, RepositoryLeafAuthorityEvaluationContextV1,
    RepositoryLeafAuthorityEvaluationErrorV1, RepositoryLeafAuthorityInputV1,
    authenticated_human_carrier_commitment, repository_leaf_authority_consumptions,
};

const ORDINARY_REPOSITORY_ACTION_PROTOCOL_VERSION_V1: u64 = 1;
const CMA_EXECUTION_SLOT_SCHEMA_DOMAIN_V1: &str =
    "maestro.vnext.continuity-maintenance-execution-slot-schema.v1";
const CMA_EXECUTION_SLOT_VALUE_DOMAIN_V1: &str =
    "maestro.vnext.continuity-maintenance-execution-slot.v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ContinuityMaintenanceExecutionSlotV1 {
    context_id: AuthorityContextIdV1,
    cma_branch_id: CmaBranchIdV1,
    slot_id: SlotIdV1,
    executor_assertion_id: ExecutorAssertionIdV1,
    executor_principal_id: PrincipalIdV1,
    purpose: CmaObservationPublicationPurposeV1,
    action: RepositoryActionLeafV1,
    withdrawal_slot_family: Option<CmaEffectWithdrawalSlotFamilyV1>,
    subject_commitment: [u8; 32],
    request_scope_commitment: [u8; 32],
    continuity_state_token: StateTokenIdV1,
    continuity_state_object_id: StoreObjectIdV1,
    guard_object_id: StoreObjectIdV1,
    authority_epoch: u64,
    job_applicability_commitment: [u8; 32],
    capacity_root_id: CapacityRootIdV1,
}

impl ContinuityMaintenanceExecutionSlotV1 {
    #[expect(
        clippy::too_many_arguments,
        reason = "the preallocated CMA slot binds every non-donatable authority dimension"
    )]
    pub(crate) fn new(
        context_id: AuthorityContextIdV1,
        cma_branch_id: CmaBranchIdV1,
        slot_id: SlotIdV1,
        executor_assertion_id: ExecutorAssertionIdV1,
        executor_principal_id: PrincipalIdV1,
        purpose: CmaObservationPublicationPurposeV1,
        action: RepositoryActionLeafV1,
        withdrawal_slot_family: Option<CmaEffectWithdrawalSlotFamilyV1>,
        subject_commitment: [u8; 32],
        continuity_state_token: StateTokenIdV1,
        continuity_state_object_id: StoreObjectIdV1,
        guard_object_id: StoreObjectIdV1,
        authority_epoch: u64,
        capacity_root_id: CapacityRootIdV1,
    ) -> Result<Self, RepositoryAuthorityAdmissionErrorV1> {
        let expected_withdrawal_slot_family = (action
            == RepositoryActionLeafV1::WithdrawContinuityMaintenanceEffect)
            .then_some(purpose.effect_withdrawal_slot_family());
        if action.execution_authority_basis()
            != Some(ActionAuthorityBasisKindV1::ContinuityMaintenance)
            || withdrawal_slot_family != expected_withdrawal_slot_family
            || context_id.as_bytes() == &[0; 32]
            || cma_branch_id.as_bytes() == &[0; 32]
            || slot_id.as_bytes() == &[0; 32]
            || executor_assertion_id.as_bytes() == &[0; 32]
            || executor_principal_id.as_bytes() == &[0; 32]
            || subject_commitment == [0; 32]
            || continuity_state_token.as_bytes() == &[0; 32]
            || continuity_state_object_id.as_bytes() == &[0; 32]
            || guard_object_id.as_bytes() == &[0; 32]
            || authority_epoch == 0
            || capacity_root_id.as_bytes() == &[0; 32]
        {
            return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
        }
        let request_scope_commitment = cma_request_scope_commitment(
            purpose,
            action,
            withdrawal_slot_family,
            subject_commitment,
        )?;
        let job_applicability_commitment = cma_job_applicability_commitment(
            context_id,
            cma_branch_id,
            purpose,
            action,
            continuity_state_token,
            continuity_state_object_id,
            guard_object_id,
            authority_epoch,
        )?;
        Ok(Self {
            context_id,
            cma_branch_id,
            slot_id,
            executor_assertion_id,
            executor_principal_id,
            purpose,
            action,
            withdrawal_slot_family,
            subject_commitment,
            request_scope_commitment,
            continuity_state_token,
            continuity_state_object_id,
            guard_object_id,
            authority_epoch,
            job_applicability_commitment,
            capacity_root_id,
        })
    }

    fn schema_id() -> Result<SchemaIdV1, crate::domain::identity::IdentityError> {
        derive_identity(&CborValue::Text(
            CMA_EXECUTION_SLOT_SCHEMA_DOMAIN_V1.to_owned(),
        ))
    }

    fn schema_value(self) -> Result<CborValue, CborError> {
        Ok(CborValue::Array(vec![
            CborValue::text(CMA_EXECUTION_SLOT_VALUE_DOMAIN_V1)?,
            bytes(self.context_id.as_bytes()),
            bytes(self.cma_branch_id.as_bytes()),
            bytes(self.slot_id.as_bytes()),
            bytes(self.executor_assertion_id.as_bytes()),
            bytes(self.executor_principal_id.as_bytes()),
            CborValue::Unsigned(self.purpose as u64),
            CborValue::text(self.action.literal())?,
            CborValue::optional(
                self.withdrawal_slot_family
                    .map(|family| CborValue::Unsigned(family as u64)),
            ),
            bytes(&self.subject_commitment),
            bytes(&self.request_scope_commitment),
            bytes(self.continuity_state_token.as_bytes()),
            bytes(self.continuity_state_object_id.as_bytes()),
            bytes(self.guard_object_id.as_bytes()),
            CborValue::Unsigned(self.authority_epoch),
            bytes(&self.job_applicability_commitment),
            bytes(self.capacity_root_id.as_bytes()),
        ]))
    }

    #[cfg(test)]
    pub(crate) fn store_object(
        self,
        references: Vec<StoreObjectIdV1>,
    ) -> Result<StoreObjectV1, RepositoryAuthorityAdmissionErrorV1> {
        Ok(StoreObjectV1::new(
            Self::schema_id()?,
            self.schema_value()?,
            references,
        )?)
    }
}

fn cma_request_scope_commitment(
    purpose: CmaObservationPublicationPurposeV1,
    action: RepositoryActionLeafV1,
    withdrawal_slot_family: Option<CmaEffectWithdrawalSlotFamilyV1>,
    subject_commitment: [u8; 32],
) -> Result<[u8; 32], RepositoryAuthorityAdmissionErrorV1> {
    Ok(
        Sha256::digest(deterministic_cbor::encode(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.cma-execution-request-scope.v1")?,
            CborValue::Unsigned(purpose as u64),
            CborValue::text(action.literal())?,
            CborValue::optional(
                withdrawal_slot_family.map(|family| CborValue::Unsigned(family as u64)),
            ),
            bytes(&subject_commitment),
        ]))?)
        .into(),
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "CMA applicability is exact across branch, purpose, state, guard, and Authority epoch"
)]
fn cma_job_applicability_commitment(
    context_id: AuthorityContextIdV1,
    cma_branch_id: CmaBranchIdV1,
    purpose: CmaObservationPublicationPurposeV1,
    action: RepositoryActionLeafV1,
    continuity_state_token: StateTokenIdV1,
    continuity_state_object_id: StoreObjectIdV1,
    guard_object_id: StoreObjectIdV1,
    authority_epoch: u64,
) -> Result<[u8; 32], RepositoryAuthorityAdmissionErrorV1> {
    Ok(
        Sha256::digest(deterministic_cbor::encode(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.cma-job-applicability.v1")?,
            bytes(context_id.as_bytes()),
            bytes(cma_branch_id.as_bytes()),
            CborValue::Unsigned(purpose as u64),
            CborValue::text(action.literal())?,
            bytes(continuity_state_token.as_bytes()),
            bytes(continuity_state_object_id.as_bytes()),
            bytes(guard_object_id.as_bytes()),
            CborValue::Unsigned(authority_epoch),
        ]))?)
        .into(),
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RepositoryActionAdmissionInputV1 {
    request_id: ActionRequestIdV1,
    authority: RepositoryLeafAuthorityInputV1,
}

impl RepositoryActionAdmissionInputV1 {
    pub(crate) fn new<A>(request_id: ActionRequestIdV1, authority: A) -> Self
    where
        A: Into<RepositoryLeafAuthorityInputV1>,
    {
        Self {
            request_id,
            authority: authority.into(),
        }
    }
}

pub(crate) struct AdmittedRepositoryActionV1 {
    request_id: ActionRequestIdV1,
    action: RepositoryActionLeafV1,
    principal_id: PrincipalIdV1,
    selection: Option<RepositoryAuthoritySelectionV1>,
    authority_context_id: AuthorityContextIdV1,
    subject_commitment: [u8; 32],
    subject_basis_commitment: [u8; 32],
    exact_payload_commitment: Option<[u8; 32]>,
    receipt: AuthorizationReceiptV1,
    authority_epoch: u64,
    accepted_h_time: u64,
    basis_object: StoreObjectV1,
    current_snapshot_id: StoreObjectIdV1,
    successor_snapshot: StoreObjectV1,
    successor_store_generation: u64,
    current_capacity_root_id: StoreObjectIdV1,
    successor_capacity_root: StoreObjectV1,
    capacity_debit: StoreObjectV1,
    leaf_authority_carrier: Option<StoreObjectV1>,
    leaf_authority_consumption: Option<StoreObjectV1>,
    guard_object_id: StoreObjectIdV1,
    state_object_id: StoreObjectIdV1,
    state_token: StateTokenIdV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::domain::authority) struct MaterializationAuthorityAdmissionV1 {
    pub(in crate::domain::authority) request_id: ActionRequestIdV1,
    pub(in crate::domain::authority) action: RepositoryActionLeafV1,
    pub(in crate::domain::authority) receipt_id: AuthorizationReceiptIdV1,
    pub(in crate::domain::authority) authority_epoch: u64,
    pub(in crate::domain::authority) accepted_h_time: u64,
    pub(in crate::domain::authority) basis_object_id: StoreObjectIdV1,
    pub(in crate::domain::authority) current_snapshot_id: StoreObjectIdV1,
    pub(in crate::domain::authority) successor_snapshot_id: StoreObjectIdV1,
    pub(in crate::domain::authority) successor_store_generation: u64,
    pub(in crate::domain::authority) current_capacity_root_id: StoreObjectIdV1,
    pub(in crate::domain::authority) successor_capacity_root_id: StoreObjectIdV1,
    pub(in crate::domain::authority) capacity_debit_id: StoreObjectIdV1,
    pub(in crate::domain::authority) leaf_authority_carrier_id: Option<StoreObjectIdV1>,
    pub(in crate::domain::authority) leaf_authority_consumption_id: Option<StoreObjectIdV1>,
    pub(in crate::domain::authority) guard_object_id: StoreObjectIdV1,
    pub(in crate::domain::authority) state_object_id: StoreObjectIdV1,
    pub(in crate::domain::authority) state_token: StateTokenIdV1,
    pub(in crate::domain::authority) principal_id: PrincipalIdV1,
    pub(in crate::domain::authority) selection: Option<RepositoryAuthoritySelectionV1>,
    pub(in crate::domain::authority) authority_context_id: AuthorityContextIdV1,
    pub(in crate::domain::authority) subject_commitment: [u8; 32],
    pub(in crate::domain::authority) subject_basis_commitment: [u8; 32],
    pub(in crate::domain::authority) exact_payload_commitment: Option<[u8; 32]>,
}

pub(crate) struct ContinuedRepositoryActionV1 {
    current_snapshot_id: StoreObjectIdV1,
    successor_snapshot: StoreObjectV1,
}

impl ContinuedRepositoryActionV1 {
    pub(crate) const fn current_snapshot_id(&self) -> StoreObjectIdV1 {
        self.current_snapshot_id
    }

    pub(crate) const fn successor_snapshot(&self) -> &StoreObjectV1 {
        &self.successor_snapshot
    }
}

impl AdmittedRepositoryActionV1 {
    pub(in crate::domain::authority) fn materialization_admission(
        &self,
    ) -> MaterializationAuthorityAdmissionV1 {
        MaterializationAuthorityAdmissionV1 {
            request_id: self.request_id,
            action: self.action,
            receipt_id: self.receipt.id(),
            authority_epoch: self.authority_epoch,
            accepted_h_time: self.accepted_h_time,
            basis_object_id: self.basis_object.id(),
            current_snapshot_id: self.current_snapshot_id,
            successor_snapshot_id: self.successor_snapshot.id(),
            successor_store_generation: self.successor_store_generation,
            current_capacity_root_id: self.current_capacity_root_id,
            successor_capacity_root_id: self.successor_capacity_root.id(),
            capacity_debit_id: self.capacity_debit.id(),
            leaf_authority_carrier_id: self.leaf_authority_carrier.as_ref().map(StoreObjectV1::id),
            leaf_authority_consumption_id: self
                .leaf_authority_consumption
                .as_ref()
                .map(StoreObjectV1::id),
            guard_object_id: self.guard_object_id,
            state_object_id: self.state_object_id,
            state_token: self.state_token,
            principal_id: self.principal_id,
            selection: self.selection,
            authority_context_id: self.authority_context_id,
            subject_commitment: self.subject_commitment,
            subject_basis_commitment: self.subject_basis_commitment,
            exact_payload_commitment: self.exact_payload_commitment,
        }
    }

    pub(crate) const fn request_id(&self) -> ActionRequestIdV1 {
        self.request_id
    }

    pub(crate) const fn action(&self) -> RepositoryActionLeafV1 {
        self.action
    }

    pub(crate) const fn authorization_receipt(&self) -> &AuthorizationReceiptV1 {
        &self.receipt
    }

    pub(crate) const fn authority_epoch(&self) -> u64 {
        self.authority_epoch
    }

    pub(crate) fn authority_epoch_commitment(
        &self,
    ) -> Result<[u8; 32], RepositoryAuthorityAdmissionErrorV1> {
        Ok(hash(&CborValue::Unsigned(self.authority_epoch))?)
    }

    pub(crate) const fn accepted_h_time(&self) -> u64 {
        self.accepted_h_time
    }

    pub(crate) fn basis_object(&self) -> &StoreObjectV1 {
        &self.basis_object
    }

    pub(crate) const fn current_snapshot_id(&self) -> StoreObjectIdV1 {
        self.current_snapshot_id
    }

    pub(crate) fn successor_snapshot(&self) -> &StoreObjectV1 {
        &self.successor_snapshot
    }

    pub(crate) const fn current_capacity_root_id(&self) -> StoreObjectIdV1 {
        self.current_capacity_root_id
    }

    pub(crate) fn successor_capacity_root(&self) -> &StoreObjectV1 {
        &self.successor_capacity_root
    }

    pub(crate) fn capacity_debit(&self) -> &StoreObjectV1 {
        &self.capacity_debit
    }

    pub(crate) fn issue_committed_artifacts(
        &self,
        request_object: &StoreObjectV1,
        produced_objects: &[StoreObjectV1],
    ) -> Result<RepositoryAuthorityArtifactsV1, RepositoryAuthorityAdmissionErrorV1> {
        self.issue_artifacts(
            request_object,
            produced_objects,
            produced_objects,
            ActionOutcomeV1::Committed,
            None,
        )
    }

    pub(crate) fn issue_committed_evidence_artifacts(
        &self,
        request_object: &StoreObjectV1,
        produced_objects: &[StoreObjectV1],
        durable_result_references: &[StoreObjectV1],
    ) -> Result<RepositoryAuthorityArtifactsV1, RepositoryAuthorityAdmissionErrorV1> {
        self.issue_artifacts(
            request_object,
            produced_objects,
            durable_result_references,
            ActionOutcomeV1::Committed,
            None,
        )
    }

    pub(crate) fn issue_in_doubt_evidence_artifacts(
        &self,
        request_object: &StoreObjectV1,
        produced_objects: &[StoreObjectV1],
        durable_result_references: &[StoreObjectV1],
        effect_reference: EffectReferenceIdV1,
    ) -> Result<RepositoryAuthorityArtifactsV1, RepositoryAuthorityAdmissionErrorV1> {
        self.issue_artifacts(
            request_object,
            produced_objects,
            durable_result_references,
            ActionOutcomeV1::InDoubt,
            Some(effect_reference),
        )
    }

    fn issue_artifacts(
        &self,
        request_object: &StoreObjectV1,
        produced_objects: &[StoreObjectV1],
        durable_result_references: &[StoreObjectV1],
        outcome: ActionOutcomeV1,
        effect_reference: Option<EffectReferenceIdV1>,
    ) -> Result<RepositoryAuthorityArtifactsV1, RepositoryAuthorityAdmissionErrorV1> {
        if produced_objects.is_empty()
            || durable_result_references.is_empty()
            || durable_result_references
                .iter()
                .any(|reference| !produced_objects.iter().any(|object| object == reference))
            || durable_result_references
                .iter()
                .map(StoreObjectV1::id)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != durable_result_references.len()
        {
            return Err(RepositoryAuthorityAdmissionErrorV1::InvalidProducedObjects);
        }
        let result = ActionResultV1::new(
            self.request_id,
            outcome,
            Some(self.receipt.clone()),
            effect_reference,
        )?;
        let leaf_authority_objects = self
            .leaf_authority_carrier
            .iter()
            .chain(self.leaf_authority_consumption.iter())
            .cloned()
            .collect::<Vec<_>>();
        let mut receipt_references = vec![
            request_object.id(),
            self.basis_object.id(),
            self.guard_object_id,
            self.state_object_id,
            self.current_snapshot_id,
            self.successor_snapshot.id(),
            self.successor_capacity_root.id(),
            self.capacity_debit.id(),
        ];
        receipt_references.extend(leaf_authority_objects.iter().map(StoreObjectV1::id));
        let receipt_object = authority_object(
            AuthoritySchemaV1::AuthorizationReceipt,
            CborValue::Array(vec![
                bytes(self.receipt.id().as_bytes()),
                bytes(self.receipt.context_id().as_bytes()),
                bytes(self.receipt.request_id().as_bytes()),
                bytes(self.basis_object.id().as_bytes()),
                CborValue::Unsigned(ORDINARY_REPOSITORY_ACTION_PROTOCOL_VERSION_V1),
                CborValue::Bool(true),
                bytes(result.id().as_bytes()),
            ]),
            receipt_references,
        )?;
        let produced_ids = produced_objects
            .iter()
            .map(|object| bytes(object.id().as_bytes()))
            .collect::<Vec<_>>();
        let mut result_references = vec![
            request_object.id(),
            receipt_object.id(),
            self.basis_object.id(),
            self.guard_object_id,
            self.successor_snapshot.id(),
            self.successor_capacity_root.id(),
            self.capacity_debit.id(),
        ];
        result_references.extend(durable_result_references.iter().map(StoreObjectV1::id));
        result_references.extend(leaf_authority_objects.iter().map(StoreObjectV1::id));
        let result_object = authority_object(
            AuthoritySchemaV1::ActionResult,
            CborValue::Array(vec![
                bytes(result.id().as_bytes()),
                bytes(result.request_id().as_bytes()),
                CborValue::Unsigned(result.outcome() as u64),
                CborValue::Unsigned(1),
                CborValue::Array(vec![bytes(self.state_token.as_bytes())]),
                CborValue::Array(vec![bytes(self.state_token.as_bytes())]),
                CborValue::Array(vec![bytes(self.receipt.id().as_bytes())]),
                CborValue::Array(produced_ids),
                CborValue::Array(Vec::new()),
                CborValue::optional(None),
                CborValue::optional(None),
            ]),
            result_references,
        )?;
        Ok(RepositoryAuthorityArtifactsV1 {
            logical_result: result,
            receipt_object,
            result_object,
            leaf_authority_objects,
        })
    }
}

pub(crate) struct RepositoryAuthorityArtifactsV1 {
    logical_result: ActionResultV1,
    receipt_object: StoreObjectV1,
    result_object: StoreObjectV1,
    leaf_authority_objects: Vec<StoreObjectV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ValidatedRepositoryActionBasisV1 {
    authority_epoch: u64,
    receipt: AuthorizationReceiptV1,
}

impl ValidatedRepositoryActionBasisV1 {
    pub(crate) const fn authority_epoch(&self) -> u64 {
        self.authority_epoch
    }

    pub(crate) const fn authorization_receipt(&self) -> &AuthorizationReceiptV1 {
        &self.receipt
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ResolvedRepositoryAuthorityChainV1 {
    terminal_grant_object_id: StoreObjectIdV1,
    terminal_grant: OrdinaryBoundedGrantV1,
    terminal_delegation_object_id: StoreObjectIdV1,
    validated_ordinary_ancestry_object_ids: Vec<StoreObjectIdV1>,
}

struct StoredOrdinaryDelegationV1<'object> {
    object: &'object StoreObjectV1,
    carrier: OrdinaryGrantDelegationV1,
}

impl RepositoryAuthorityArtifactsV1 {
    pub(crate) fn logical_result(&self) -> &ActionResultV1 {
        &self.logical_result
    }

    pub(crate) fn receipt_object(&self) -> &StoreObjectV1 {
        &self.receipt_object
    }

    pub(crate) fn result_object(&self) -> &StoreObjectV1 {
        &self.result_object
    }

    pub(crate) fn leaf_authority_objects(&self) -> &[StoreObjectV1] {
        &self.leaf_authority_objects
    }
}

pub(crate) fn current_repository_authority_time(
    view: &StorePublicationViewV1<'_>,
    current_generation: &StoreGenerationV1,
) -> Result<(u64, [u8; 32]), RepositoryAuthorityAdmissionErrorV1> {
    if current_generation.domain() != view.domain() {
        return Err(RepositoryAuthorityAdmissionErrorV1::Unavailable);
    }
    let expected_context_kind = authority_context_kind_for_role(view.role());
    let active_objects = view.active_generation_objects()?;
    let snapshot_schema = AuthoritySchemaV1::BootstrapAuthoritySnapshot.id()?;
    let mut snapshots = active_objects
        .iter()
        .filter(|object| object.schema_id() == snapshot_schema)
        .filter_map(|object| {
            BootstrapAuthoritySnapshotV1::from_canonical_bytes(&object_value_bytes(object).ok()?)
                .ok()
                .filter(|facts| {
                    facts.context().kind() == expected_context_kind
                        && facts.context().store_generation() == current_generation.ordinal()
                        && facts.snapshot().store_generation == current_generation.ordinal()
                })
                .map(|facts| (object, facts))
        })
        .collect::<Vec<_>>();
    if snapshots.len() != 1 {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidCurrentAuthority);
    }
    let (snapshot_object, facts) = snapshots
        .pop()
        .expect("invariant: exact one-element current Authority snapshot");
    if !current_generation.roots().contains(&snapshot_object.id()) {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidCurrentAuthority);
    }
    let manifest = authority_manifest_for_role(view.role())?;
    let referenced = direct_references(snapshot_object, &active_objects)?;
    let state_object = one_schema_object(
        &referenced,
        AuthoritySchemaV1::SuccessVisibleAuthorityContinuityState,
    )?;
    let guard_object = one_schema_object(&referenced, AuthoritySchemaV1::AdmittedTransitionGuard)?;
    let current = validate_current_guard(
        current_generation,
        &facts,
        &manifest,
        &state_object,
        &guard_object,
    )?;
    let accepted_h_time = current.accepted_time().lower_bound();
    if accepted_h_time == 0 {
        return Err(RepositoryAuthorityAdmissionErrorV1::Unavailable);
    }
    let acceptance_value = CborValue::Array(vec![
        CborValue::text("maestro.vnext.current-repository-authority-time.v1")?,
        bytes(current_generation.id().as_bytes()),
        bytes(snapshot_object.id().as_bytes()),
        bytes(state_object.id().as_bytes()),
        bytes(guard_object.id().as_bytes()),
        bytes(current.state_token().as_bytes()),
        CborValue::Unsigned(accepted_h_time),
    ]);
    let acceptance_commitment: [u8; 32] =
        Sha256::digest(deterministic_cbor::encode(&acceptance_value)?).into();
    Ok((accepted_h_time, acceptance_commitment))
}

pub(crate) fn current_authorization_receipt_is_persisted(
    active_objects: &[StoreObjectV1],
    receipt: &AuthorizationReceiptV1,
) -> Result<bool, RepositoryAuthorityAdmissionErrorV1> {
    let receipt_schema = AuthoritySchemaV1::AuthorizationReceipt.id()?;
    let basis_schema = AuthoritySchemaV1::ActionAuthorityBasis.id()?;
    let result_schema = AuthoritySchemaV1::ActionResult.id()?;
    let matching = active_objects
        .iter()
        .filter(|object| object.schema_id() == receipt_schema)
        .filter_map(|receipt_object| {
            let CborValue::Array(fields) = receipt_object.value() else {
                return None;
            };
            let [
                receipt_id,
                context_id,
                request_id,
                basis_object_id,
                CborValue::Unsigned(1),
                CborValue::Bool(true),
                result_id,
            ] = fields.as_slice()
            else {
                return None;
            };
            let basis_object_id = StoreObjectIdV1::from_digest(exact_digest(basis_object_id).ok()?);
            let result_id = exact_digest(result_id).ok()?;
            if exact_digest(receipt_id).ok()? != *receipt.id().as_bytes()
                || exact_digest(context_id).ok()? != *receipt.context_id().as_bytes()
                || exact_digest(request_id).ok()? != *receipt.request_id().as_bytes()
                || !receipt_object.references().contains(&basis_object_id)
            {
                return None;
            }
            let basis_object = active_objects.iter().find(|object| {
                object.id() == basis_object_id && object.schema_id() == basis_schema
            })?;
            let CborValue::Array(basis_fields) = basis_object.value() else {
                return None;
            };
            let [
                CborValue::Unsigned(basis_kind),
                basis_context_id,
                basis_commitment,
            ] = basis_fields.as_slice()
            else {
                return None;
            };
            if *basis_kind != receipt.basis_kind() as u64
                || exact_digest(basis_context_id).ok()? != *receipt.context_id().as_bytes()
                || exact_digest(basis_commitment).ok()? == [0; 32]
            {
                return None;
            }
            let result_objects = active_objects
                .iter()
                .filter(|object| object.schema_id() == result_schema)
                .filter(|object| {
                    let CborValue::Array(result_fields) = object.value() else {
                        return false;
                    };
                    let [
                        logical_result_id,
                        result_request_id,
                        CborValue::Unsigned(_),
                        CborValue::Unsigned(1),
                        CborValue::Array(prior_tokens),
                        CborValue::Array(resulting_tokens),
                        CborValue::Array(receipt_ids),
                        CborValue::Array(_),
                        CborValue::Array(_),
                        _,
                        _,
                    ] = result_fields.as_slice()
                    else {
                        return false;
                    };
                    exact_digest(logical_result_id).ok() == Some(result_id)
                        && exact_digest(result_request_id).ok()
                            == Some(*receipt.request_id().as_bytes())
                        && prior_tokens.len() == 1
                        && exact_digest(&prior_tokens[0]).ok()
                            == Some(*receipt.prior_state_token().as_bytes())
                        && resulting_tokens.len() == 1
                        && exact_digest(&resulting_tokens[0]).ok()
                            == Some(*receipt.resulting_state_token().as_bytes())
                        && receipt_ids.len() == 1
                        && exact_digest(&receipt_ids[0]).ok() == Some(*receipt.id().as_bytes())
                        && object.references().contains(&receipt_object.id())
                })
                .count();
            (result_objects == 1).then_some(receipt_object.id())
        })
        .collect::<std::collections::BTreeSet<_>>();
    Ok(matching.len() == 1)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PersistedEvidenceMutationAuthorityExpectationV1 {
    receipt_id: AuthorizationReceiptIdV1,
    request_id: ActionRequestIdV1,
    accepted_h_time: u64,
    produced_object_id: StoreObjectIdV1,
    effect: Option<(EffectReferenceIdV1, StoreObjectIdV1)>,
}

impl PersistedEvidenceMutationAuthorityExpectationV1 {
    pub(crate) fn new(
        receipt_id: AuthorizationReceiptIdV1,
        request_id: ActionRequestIdV1,
        accepted_h_time: u64,
        produced_object_id: StoreObjectIdV1,
        effect: Option<(EffectReferenceIdV1, StoreObjectIdV1)>,
    ) -> Self {
        Self {
            receipt_id,
            request_id,
            accepted_h_time,
            produced_object_id,
            effect,
        }
    }
}

pub(crate) fn validate_persisted_evidence_mutation_authority(
    generation: &StoreGenerationV1,
    active_objects: &[StoreObjectV1],
    request_object: &StoreObjectV1,
    expected: PersistedEvidenceMutationAuthorityExpectationV1,
) -> Result<AuthorizationReceiptV1, RepositoryAuthorityAdmissionErrorV1> {
    if expected.accepted_h_time == 0 {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    }
    let expected_effect_reference = expected.effect.map(|(reference, _)| reference);
    let expected_effect_object_id = expected.effect.map(|(_, object_id)| object_id);
    let receipt_schema = AuthoritySchemaV1::AuthorizationReceipt.id()?;
    let receipt_objects = active_objects
        .iter()
        .filter(|object| object.schema_id() == receipt_schema)
        .filter(|object| object.references().contains(&request_object.id()))
        .filter(|object| {
            matches!(object.value(), CborValue::Array(fields)
                if fields.len() == 7
                    && fields.first().and_then(|value| exact_digest(value).ok())
                        == Some(*expected.receipt_id.as_bytes())
                    && fields.get(2).and_then(|value| exact_digest(value).ok())
                        == Some(*expected.request_id.as_bytes()))
        })
        .collect::<Vec<_>>();
    let [receipt_object] = receipt_objects.as_slice() else {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    };
    let CborValue::Array(receipt_fields) = receipt_object.value() else {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    };
    let [
        receipt_id,
        context_id,
        request_id,
        basis_object_id,
        CborValue::Unsigned(ORDINARY_REPOSITORY_ACTION_PROTOCOL_VERSION_V1),
        CborValue::Bool(true),
        logical_result_id,
    ] = receipt_fields.as_slice()
    else {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    };
    let context_id = AuthorityContextIdV1::from_digest(exact_digest(context_id)?);
    let basis_object_id = StoreObjectIdV1::from_digest(exact_digest(basis_object_id)?);
    let logical_result_id = exact_digest(logical_result_id)?;
    if exact_digest(receipt_id)? != *expected.receipt_id.as_bytes()
        || exact_digest(request_id)? != *expected.request_id.as_bytes()
    {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    }
    let basis_schema = AuthoritySchemaV1::ActionAuthorityBasis.id()?;
    let basis_object = active_objects
        .iter()
        .find(|object| object.id() == basis_object_id && object.schema_id() == basis_schema)
        .ok_or(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier)?;
    let CborValue::Array(basis_fields) = basis_object.value() else {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    };
    let [
        CborValue::Unsigned(basis_kind),
        basis_context_id,
        basis_commitment,
    ] = basis_fields.as_slice()
    else {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    };
    let basis_kind = ActionAuthorityBasisKindV1::try_from(
        u8::try_from(*basis_kind)
            .map_err(|_| RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier)?,
    )
    .map_err(|_| RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier)?;
    if exact_digest(basis_context_id)? != *context_id.as_bytes()
        || exact_digest(basis_commitment)? == [0; 32]
        || !receipt_object.references().contains(&basis_object.id())
    {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    }
    let result_schema = AuthoritySchemaV1::ActionResult.id()?;
    let result_objects = active_objects
        .iter()
        .filter(|object| object.schema_id() == result_schema)
        .filter(|object| object.references().contains(&receipt_object.id()))
        .filter(|object| {
            matches!(object.value(), CborValue::Array(fields)
                if fields.first().and_then(|value| exact_digest(value).ok()) == Some(logical_result_id))
        })
        .collect::<Vec<_>>();
    let [result_object] = result_objects.as_slice() else {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    };
    let CborValue::Array(result_fields) = result_object.value() else {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    };
    let [
        result_id,
        result_request_id,
        CborValue::Unsigned(outcome),
        CborValue::Unsigned(ORDINARY_REPOSITORY_ACTION_PROTOCOL_VERSION_V1),
        CborValue::Array(prior_tokens),
        CborValue::Array(resulting_tokens),
        CborValue::Array(receipt_ids),
        CborValue::Array(produced_ids),
        CborValue::Array(invalidated_ids),
        _,
        _,
    ] = result_fields.as_slice()
    else {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    };
    let ([prior_token], [resulting_token], [result_receipt_id]) = (
        prior_tokens.as_slice(),
        resulting_tokens.as_slice(),
        receipt_ids.as_slice(),
    ) else {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    };
    if exact_digest(result_request_id)? != *expected.request_id.as_bytes()
        || exact_digest(result_receipt_id)? != *expected.receipt_id.as_bytes()
        || !invalidated_ids.is_empty()
        || produced_ids
            .iter()
            .filter(|id| exact_digest(id).ok() == Some(*expected.produced_object_id.as_bytes()))
            .count()
            != 1
        || !result_object
            .references()
            .contains(&expected.produced_object_id)
        || expected_effect_object_id
            .is_some_and(|object_id| !result_object.references().contains(&object_id))
    {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    }
    let prior_state_token = StateTokenIdV1::from_digest(exact_digest(prior_token)?);
    let resulting_state_token = StateTokenIdV1::from_digest(exact_digest(resulting_token)?);
    let receipt = AuthorizationReceiptV1::new(
        expected.request_id,
        context_id,
        basis_kind,
        prior_state_token,
        resulting_state_token,
    )?;
    let expected_outcome = if expected_effect_reference.is_some() {
        ActionOutcomeV1::InDoubt
    } else {
        ActionOutcomeV1::Committed
    };
    let logical_result = ActionResultV1::new(
        expected.request_id,
        expected_outcome,
        Some(receipt.clone()),
        expected_effect_reference,
    )?;
    if receipt.id() != expected.receipt_id
        || exact_digest(result_id)? != logical_result_id
        || logical_result.id().as_bytes() != &logical_result_id
        || *outcome != expected_outcome as u64
        || !current_authorization_receipt_is_persisted(active_objects, &receipt)?
    {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    }
    let state_schema = AuthoritySchemaV1::SuccessVisibleAuthorityContinuityState.id()?;
    let state_objects = receipt_object
        .references()
        .iter()
        .filter_map(|id| active_objects.iter().find(|object| object.id() == *id))
        .filter(|object| object.schema_id() == state_schema)
        .collect::<Vec<_>>();
    let [state_object] = state_objects.as_slice() else {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    };
    let manifest = authority_manifest_for_role(generation.domain().role())?;
    let state = SuccessVisibleAuthorityContinuityStateV1::decode(
        &object_value_bytes(state_object)?,
        &manifest,
    )?;
    if state.context_id() != context_id
        || state.state_token() != prior_state_token
        || prior_state_token != resulting_state_token
        || state.accepted_time().lower_bound() != expected.accepted_h_time
    {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    }
    Ok(receipt)
}

pub(crate) fn admit_repository_action(
    view: &StorePublicationViewV1<'_>,
    current_generation: &StoreGenerationV1,
    input: RepositoryActionAdmissionInputV1,
) -> Result<AdmittedRepositoryActionV1, RepositoryAuthorityAdmissionErrorV1> {
    if current_generation.domain() != view.domain() || current_generation.ordinal() == u64::MAX {
        return Err(RepositoryAuthorityAdmissionErrorV1::Unavailable);
    }
    let expected_context_kind = authority_context_kind_for_role(view.role());
    let active_objects = view.active_generation_objects()?;
    let snapshot_schema = AuthoritySchemaV1::BootstrapAuthoritySnapshot.id()?;
    let mut snapshots = active_objects
        .iter()
        .filter(|object| object.schema_id() == snapshot_schema)
        .filter_map(|object| {
            BootstrapAuthoritySnapshotV1::from_canonical_bytes(&object_value_bytes(object).ok()?)
                .ok()
                .filter(|facts| {
                    facts.context().kind() == expected_context_kind
                        && facts.context().store_generation() == current_generation.ordinal()
                        && facts.snapshot().store_generation == current_generation.ordinal()
                })
                .map(|facts| (object, facts))
        })
        .collect::<Vec<_>>();
    if snapshots.len() != 1 {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidCurrentAuthority);
    }
    let (snapshot_object, facts) = snapshots
        .pop()
        .expect("invariant: exact one-element current Authority snapshot");
    if !current_generation.roots().contains(&snapshot_object.id()) {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidCurrentAuthority);
    }

    let manifest = authority_manifest_for_role(view.role())?;
    let referenced = direct_references(snapshot_object, &active_objects)?;
    let state_object = one_schema_object(
        &referenced,
        AuthoritySchemaV1::SuccessVisibleAuthorityContinuityState,
    )?;
    let guard_object = one_schema_object(&referenced, AuthoritySchemaV1::AdmittedTransitionGuard)?;
    let current_continuity_state = validate_current_guard(
        current_generation,
        &facts,
        &manifest,
        &state_object,
        &guard_object,
    )?;

    let specialized_execution_authority = match &input.authority {
        RepositoryLeafAuthorityInputV1::Execution(
            authority @ (ExecutionAuthorityV1::BootstrapG0(_)
            | ExecutionAuthorityV1::ContinuityMaintenance(_)),
        ) => Some(authority.clone()),
        _ => None,
    };
    if let Some(authority) = specialized_execution_authority {
        return admit_specialized_repository_execution_action(
            input.request_id,
            authority,
            current_generation,
            &active_objects,
            snapshot_object,
            &facts,
            &state_object,
            &guard_object,
            &current_continuity_state,
        );
    }

    let action = input.authority.action();
    let subject_commitment = input.authority.subject_commitment();
    let subject_basis_commitment = input.authority.subject_basis_commitment();
    let exact_payload_commitment = input.authority.exact_payload_commitment();
    let executor_principal_id = input.authority.executor_principal_id();
    let selection = input
        .authority
        .selection()
        .ok_or(RepositoryAuthorityAdmissionErrorV1::UnsupportedExecutionAuthority)?;
    if facts.actor_binding().id() != selection.actor_binding_id()
        || facts.actor_session().id() != selection.actor_session_id()
        || executor_principal_id
            .is_some_and(|principal_id| principal_id != facts.actor_binding().principal_id())
    {
        return Err(RepositoryAuthorityAdmissionErrorV1::AuthoritySelectionMismatch);
    }
    let authority_objects = active_objects
        .iter()
        .filter(|object| {
            current_generation.roots().contains(&object.id())
                || snapshot_object.references().contains(&object.id())
        })
        .collect::<Vec<_>>();
    let resolved = resolve_repository_authority_chain(
        &facts,
        selection.terminal_grant_id(),
        &authority_objects,
    )?;
    let (trusted_time_lower, trusted_time_upper) = match facts.snapshot().trusted_time {
        TrustedTimeV1::Verified {
            lower_bound,
            upper_bound,
        } => (lower_bound, upper_bound),
        TrustedTimeV1::Unavailable => {
            return Err(RepositoryAuthorityAdmissionErrorV1::Unavailable);
        }
    };
    let leaf_evaluation_context = RepositoryLeafAuthorityEvaluationContextV1 {
        human_binding_id: facts.responder_binding().id(),
        human_session_id: facts.responder_session().id(),
        human_capable: facts.responder_binding().human_capable()
            && facts.responder_session().binding_id() == facts.responder_binding().id()
            && facts.responder_binding().context_id() == facts.context().context_id()
            && facts.responder_session().context_id() == facts.context().context_id()
            && facts.responder_session().store_generation() == current_generation.ordinal()
            && facts.responder_session().authority_epoch() == facts.snapshot().authority_epoch
            && facts
                .snapshot()
                .trusted_time
                .is_within(facts.responder_binding().validity())?
            && facts
                .snapshot()
                .trusted_time
                .is_within(facts.responder_session().validity())?,
        human_revoked: facts.revocations().revocations().contains(
            RevocationTargetV1::PrincipalBinding(facts.responder_binding().id()),
        ) || facts
            .revocations()
            .revocations()
            .contains(RevocationTargetV1::Session(facts.responder_session().id())),
        authenticated_carrier_commitment: authenticated_human_carrier_commitment(
            facts.responder_session().request_commitment().as_bytes(),
        )?,
        human_valid_until: facts
            .responder_binding()
            .validity()
            .expires_at()
            .min(facts.responder_session().validity().expires_at()),
        trusted_time_lower: Some(trusted_time_lower),
        trusted_time_upper: Some(trusted_time_upper),
        prior_consumptions: repository_leaf_authority_consumptions(&active_objects)?,
    };
    let specialized_authority = input
        .authority
        .evaluate_specialized(&leaf_evaluation_context)?;
    let leaf_authority_carrier = specialized_authority
        .as_ref()
        .map(|authority| authority.carrier_object(vec![snapshot_object.id()]))
        .transpose()?;
    let selected_grant_object_id = resolved.terminal_grant_object_id;
    let selected_grant = &resolved.terminal_grant;
    let delegation_object_id = resolved.terminal_delegation_object_id;
    let validated_ordinary_ancestry_object_ids =
        resolved.validated_ordinary_ancestry_object_ids.clone();
    let capacity_root_id = selected_grant.capacity_root_id();
    let expected_capacity_kind =
        governed_capacity_kind_for(view.role(), action.is_external_effect_action());
    let (current_capacity_root_object, current_capacity_root) = current_capacity_root(
        &active_objects,
        current_generation,
        snapshot_object,
        facts.context().context_id(),
        capacity_root_id,
        expected_capacity_kind,
    )?;
    let capacity_transition = current_capacity_root.transition(
        facts.context().context_id(),
        expected_capacity_kind,
        current_capacity_root.spent(),
        CapacityUseDispositionV1::FreshCommit,
    )?;
    let successor_capacity_root = authority_object(
        AuthoritySchemaV1::GovernedCapacityRoot,
        capacity_transition.root().schema_value()?,
        vec![current_capacity_root_object.id()],
    )?;
    let capacity_debit = authority_object(
        AuthoritySchemaV1::GovernedCapacityDebit,
        capacity_transition
            .debit()
            .ok_or(RepositoryAuthorityAdmissionErrorV1::CapacityUnavailable)?
            .schema_value()?,
        vec![
            current_capacity_root_object.id(),
            successor_capacity_root.id(),
        ],
    )?;
    let required_scope = ScopeAtomV1::new(
        action.literal(),
        &render_digest(subject_commitment),
        facts.snapshot().subject_revision,
    )?;
    validate_ordinary_authority(
        facts.snapshot(),
        facts.actor_binding(),
        facts.actor_session(),
        selected_grant.grant(),
        &required_scope,
        facts.revocations().revocations(),
    )?;
    if !facts
        .snapshot()
        .trusted_time
        .is_within(facts.continuity().validity())?
    {
        return Err(RepositoryAuthorityAdmissionErrorV1::Unavailable);
    }

    let guard_digest: [u8; 32] =
        Sha256::digest(deterministic_cbor::encode(guard_object.value())?).into();
    let basis_commitment = repository_action_basis_commitment(
        input.request_id,
        action,
        subject_commitment,
        subject_basis_commitment,
        exact_payload_commitment,
        executor_principal_id,
        current_generation,
        &facts,
        selection,
        specialized_authority
            .as_ref()
            .map(|authority| authority.leaf_commitment()),
        guard_digest,
    )?;
    let mut basis_references = vec![
        snapshot_object.id(),
        guard_object.id(),
        state_object.id(),
        selected_grant_object_id,
        current_capacity_root_object.id(),
    ];
    basis_references.extend(leaf_authority_carrier.iter().map(StoreObjectV1::id));
    let basis_object = authority_object(
        AuthoritySchemaV1::ActionAuthorityBasis,
        CborValue::Array(vec![
            CborValue::Unsigned(ActionAuthorityBasisKindV1::OrdinaryLiveRuntime as u64),
            bytes(facts.context().context_id().as_bytes()),
            bytes(&basis_commitment),
        ]),
        basis_references,
    )?;
    let leaf_authority_consumption = specialized_authority
        .as_ref()
        .zip(leaf_authority_carrier.as_ref())
        .map(|(authority, carrier)| {
            authority.consumption_object(
                input.request_id,
                current_generation.id(),
                basis_object.id(),
                carrier.id(),
            )
        })
        .transpose()?;
    let receipt = AuthorizationReceiptV1::new(
        input.request_id,
        facts.context().context_id(),
        ActionAuthorityBasisKindV1::OrdinaryLiveRuntime,
        facts.continuity().state_token(),
        facts.continuity().state_token(),
    )?;
    let next_generation = current_generation
        .ordinal()
        .checked_add(1)
        .ok_or(RepositoryAuthorityAdmissionErrorV1::Unavailable)?;
    let successor_facts = facts.continue_at_store_generation(
        next_generation,
        facts.continuity().manifest_id(),
        facts.continuity().guard_kind(),
        facts.continuity().state_token(),
    )?;
    let mut successor_snapshot_references = vec![
        snapshot_object.id(),
        guard_object.id(),
        state_object.id(),
        basis_object.id(),
        selected_grant_object_id,
        delegation_object_id,
        successor_capacity_root.id(),
        capacity_debit.id(),
    ];
    successor_snapshot_references.extend(validated_ordinary_ancestry_object_ids);
    successor_snapshot_references.extend(leaf_authority_carrier.iter().map(StoreObjectV1::id));
    successor_snapshot_references.extend(leaf_authority_consumption.iter().map(StoreObjectV1::id));
    let successor_snapshot = authority_object(
        AuthoritySchemaV1::BootstrapAuthoritySnapshot,
        successor_facts.schema_value()?,
        successor_snapshot_references,
    )?;
    Ok(AdmittedRepositoryActionV1 {
        request_id: input.request_id,
        action,
        principal_id: facts.actor_binding().principal_id(),
        selection: Some(selection),
        authority_context_id: facts.context().context_id(),
        subject_commitment,
        subject_basis_commitment,
        exact_payload_commitment,
        receipt,
        authority_epoch: facts.snapshot().authority_epoch,
        accepted_h_time: current_continuity_state.accepted_time().lower_bound(),
        basis_object,
        current_snapshot_id: snapshot_object.id(),
        successor_snapshot,
        successor_store_generation: next_generation,
        current_capacity_root_id: current_capacity_root_object.id(),
        successor_capacity_root,
        capacity_debit,
        leaf_authority_carrier,
        leaf_authority_consumption,
        guard_object_id: guard_object.id(),
        state_object_id: state_object.id(),
        state_token: facts.continuity().state_token(),
    })
}

pub(crate) fn continue_repository_action_attempt(
    view: &StorePublicationViewV1<'_>,
    current_generation: &StoreGenerationV1,
    basis_object_id: StoreObjectIdV1,
    authority: &ExecutionAuthorityV1,
    expected_authority_epoch_commitment: [u8; 32],
) -> Result<ContinuedRepositoryActionV1, RepositoryAuthorityAdmissionErrorV1> {
    if current_generation.domain() != view.domain() || current_generation.ordinal() == u64::MAX {
        return Err(RepositoryAuthorityAdmissionErrorV1::Unavailable);
    }
    let expected_context_kind = authority_context_kind_for_role(view.role());
    let active_objects = view.active_generation_objects()?;
    let snapshot_schema = AuthoritySchemaV1::BootstrapAuthoritySnapshot.id()?;
    let mut snapshots = active_objects
        .iter()
        .filter(|object| object.schema_id() == snapshot_schema)
        .filter_map(|object| {
            BootstrapAuthoritySnapshotV1::from_canonical_bytes(&object_value_bytes(object).ok()?)
                .ok()
                .filter(|facts| {
                    facts.context().kind() == expected_context_kind
                        && facts.context().store_generation() == current_generation.ordinal()
                        && facts.snapshot().store_generation == current_generation.ordinal()
                        && current_generation.roots().contains(&object.id())
                })
                .map(|facts| (object, facts))
        })
        .collect::<Vec<_>>();
    if snapshots.len() != 1 {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidCurrentAuthority);
    }
    let (snapshot_object, facts) = snapshots
        .pop()
        .expect("invariant: exact one-element current Authority snapshot");
    let referenced = direct_references(snapshot_object, &active_objects)?;
    let state_object = one_schema_object(
        &referenced,
        AuthoritySchemaV1::SuccessVisibleAuthorityContinuityState,
    )?;
    let guard_object = one_schema_object(&referenced, AuthoritySchemaV1::AdmittedTransitionGuard)?;
    let manifest = authority_manifest_for_role(view.role())?;
    validate_current_guard(
        current_generation,
        &facts,
        &manifest,
        &state_object,
        &guard_object,
    )?;
    if !facts
        .snapshot()
        .trusted_time
        .is_within(facts.continuity().validity())?
    {
        return Err(RepositoryAuthorityAdmissionErrorV1::Unavailable);
    }
    let basis_schema = AuthoritySchemaV1::ActionAuthorityBasis.id()?;
    let basis_object = active_objects
        .iter()
        .find(|object| object.id() == basis_object_id)
        .filter(|object| object.schema_id() == basis_schema)
        .ok_or(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier)?;
    let CborValue::Array(basis_fields) = basis_object.value() else {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    };
    let [CborValue::Unsigned(kind), context, commitment] = basis_fields.as_slice() else {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    };
    if *kind != authority.basis_kind() as u64
        || exact_digest(context)? != *facts.context().context_id().as_bytes()
        || exact_digest(commitment)? == [0; 32]
        || !reference_closure_contains(snapshot_object, basis_object_id, &active_objects)?
        || hash(&CborValue::Unsigned(facts.snapshot().authority_epoch))?
            != expected_authority_epoch_commitment
    {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    }
    validate_continuing_execution_authority_currentness(
        authority,
        current_generation,
        &active_objects,
        snapshot_object,
        &facts,
        basis_object,
        &state_object,
        &guard_object,
    )?;
    let successor_facts = facts.continue_at_store_generation(
        current_generation
            .ordinal()
            .checked_add(1)
            .ok_or(RepositoryAuthorityAdmissionErrorV1::Unavailable)?,
        facts.continuity().manifest_id(),
        facts.continuity().guard_kind(),
        facts.continuity().state_token(),
    )?;
    let mut successor_references = snapshot_object.references().to_vec();
    successor_references.extend([
        snapshot_object.id(),
        guard_object.id(),
        state_object.id(),
        basis_object.id(),
    ]);
    let successor_snapshot = authority_object(
        AuthoritySchemaV1::BootstrapAuthoritySnapshot,
        successor_facts.schema_value()?,
        successor_references,
    )?;
    Ok(ContinuedRepositoryActionV1 {
        current_snapshot_id: snapshot_object.id(),
        successor_snapshot,
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "durable recovery binds the exact admitted request, basis, Receipt, in-doubt Result, effect carrier, and original Authority epoch"
)]
pub(crate) fn continue_durably_admitted_repository_action_attempt(
    view: &StorePublicationViewV1<'_>,
    current_generation: &StoreGenerationV1,
    request_object_id: StoreObjectIdV1,
    basis_object_id: StoreObjectIdV1,
    receipt: &AuthorizationReceiptV1,
    effect_reference: EffectReferenceIdV1,
    effect_object_id: StoreObjectIdV1,
    admitted_authority_epoch: u64,
    expected_authority_epoch_commitment: [u8; 32],
) -> Result<ContinuedRepositoryActionV1, RepositoryAuthorityAdmissionErrorV1> {
    if current_generation.domain() != view.domain()
        || current_generation.ordinal() == u64::MAX
        || admitted_authority_epoch == 0
        || hash(&CborValue::Unsigned(admitted_authority_epoch))?
            != expected_authority_epoch_commitment
    {
        return Err(RepositoryAuthorityAdmissionErrorV1::Unavailable);
    }
    let active_objects = view.active_generation_objects()?;
    let expected_context_kind = authority_context_kind_for_role(view.role());
    let snapshot_schema = AuthoritySchemaV1::BootstrapAuthoritySnapshot.id()?;
    let mut snapshots = active_objects
        .iter()
        .filter(|object| object.schema_id() == snapshot_schema)
        .filter_map(|object| {
            BootstrapAuthoritySnapshotV1::from_canonical_bytes(&object_value_bytes(object).ok()?)
                .ok()
                .filter(|facts| {
                    facts.context().kind() == expected_context_kind
                        && facts.context().store_generation() == current_generation.ordinal()
                        && facts.snapshot().store_generation == current_generation.ordinal()
                        && current_generation.roots().contains(&object.id())
                })
                .map(|facts| (object, facts))
        })
        .collect::<Vec<_>>();
    if snapshots.len() != 1 {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidCurrentAuthority);
    }
    let (snapshot_object, facts) = snapshots
        .pop()
        .expect("invariant: exact one-element current Authority snapshot");
    let referenced = direct_references(snapshot_object, &active_objects)?;
    let state_object = one_schema_object(
        &referenced,
        AuthoritySchemaV1::SuccessVisibleAuthorityContinuityState,
    )?;
    let guard_object = one_schema_object(&referenced, AuthoritySchemaV1::AdmittedTransitionGuard)?;
    let manifest = authority_manifest_for_role(view.role())?;
    validate_current_guard(
        current_generation,
        &facts,
        &manifest,
        &state_object,
        &guard_object,
    )?;

    let basis_schema = AuthoritySchemaV1::ActionAuthorityBasis.id()?;
    let basis_object = active_objects
        .iter()
        .find(|object| object.id() == basis_object_id && object.schema_id() == basis_schema)
        .ok_or(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier)?;
    let CborValue::Array(basis_fields) = basis_object.value() else {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    };
    let [
        CborValue::Unsigned(basis_kind),
        basis_context,
        basis_commitment,
    ] = basis_fields.as_slice()
    else {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    };
    if *basis_kind != receipt.basis_kind() as u64
        || exact_digest(basis_context)? != *receipt.context_id().as_bytes()
        || exact_digest(basis_commitment)? == [0; 32]
        || !reference_closure_contains(snapshot_object, basis_object_id, &active_objects)?
        || !current_authorization_receipt_is_persisted(&active_objects, receipt)?
    {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    }
    let request_object = active_objects
        .iter()
        .find(|object| object.id() == request_object_id)
        .ok_or(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier)?;
    let effect_object = active_objects
        .iter()
        .find(|object| object.id() == effect_object_id)
        .ok_or(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier)?;
    let logical_result = ActionResultV1::new(
        receipt.request_id(),
        ActionOutcomeV1::InDoubt,
        Some(receipt.clone()),
        Some(effect_reference),
    )?;
    let receipt_schema = AuthoritySchemaV1::AuthorizationReceipt.id()?;
    let mut receipt_objects = active_objects
        .iter()
        .filter(|object| object.schema_id() == receipt_schema)
        .filter(|object| {
            let CborValue::Array(fields) = object.value() else {
                return false;
            };
            let [
                receipt_id,
                context_id,
                request_id,
                stored_basis_id,
                CborValue::Unsigned(ORDINARY_REPOSITORY_ACTION_PROTOCOL_VERSION_V1),
                CborValue::Bool(true),
                result_id,
            ] = fields.as_slice()
            else {
                return false;
            };
            exact_digest(receipt_id).ok() == Some(*receipt.id().as_bytes())
                && exact_digest(context_id).ok() == Some(*receipt.context_id().as_bytes())
                && exact_digest(request_id).ok() == Some(*receipt.request_id().as_bytes())
                && exact_digest(stored_basis_id).ok() == Some(*basis_object_id.as_bytes())
                && exact_digest(result_id).ok() == Some(*logical_result.id().as_bytes())
                && object.references().contains(&request_object_id)
                && object.references().contains(&basis_object_id)
        })
        .collect::<Vec<_>>();
    if receipt_objects.len() != 1 {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    }
    let receipt_object = receipt_objects
        .pop()
        .expect("invariant: exact one durable Authorization Receipt carrier");
    let result_schema = AuthoritySchemaV1::ActionResult.id()?;
    let mut result_objects = active_objects
        .iter()
        .filter(|object| object.schema_id() == result_schema)
        .filter(|object| {
            let CborValue::Array(fields) = object.value() else {
                return false;
            };
            let [
                logical_result_id,
                request_id,
                CborValue::Unsigned(outcome),
                CborValue::Unsigned(ORDINARY_REPOSITORY_ACTION_PROTOCOL_VERSION_V1),
                CborValue::Array(prior_tokens),
                CborValue::Array(resulting_tokens),
                CborValue::Array(receipt_ids),
                CborValue::Array(produced_ids),
                CborValue::Array(invalidated_ids),
                CborValue::Array(first_optional),
                CborValue::Array(second_optional),
            ] = fields.as_slice()
            else {
                return false;
            };
            exact_digest(logical_result_id).ok() == Some(*logical_result.id().as_bytes())
                && exact_digest(request_id).ok() == Some(*receipt.request_id().as_bytes())
                && *outcome == ActionOutcomeV1::InDoubt as u64
                && prior_tokens.as_slice() == [bytes(receipt.prior_state_token().as_bytes())]
                && resulting_tokens.as_slice()
                    == [bytes(receipt.resulting_state_token().as_bytes())]
                && receipt_ids.as_slice() == [bytes(receipt.id().as_bytes())]
                && produced_ids
                    .iter()
                    .any(|id| exact_digest(id).ok() == Some(*effect_object_id.as_bytes()))
                && invalidated_ids.is_empty()
                && first_optional.as_slice() == [CborValue::Unsigned(0)]
                && second_optional.as_slice() == [CborValue::Unsigned(0)]
                && current_generation.roots().contains(&object.id())
                && object.references().contains(&request_object_id)
                && object.references().contains(&basis_object_id)
                && object.references().contains(&receipt_object.id())
                && object.references().contains(&effect_object_id)
        })
        .collect::<Vec<_>>();
    if result_objects.len() != 1 {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    }
    let result_object = result_objects
        .pop()
        .expect("invariant: exact one durable in-doubt Action Result carrier");

    let successor_facts = facts.continue_at_store_generation(
        current_generation
            .ordinal()
            .checked_add(1)
            .ok_or(RepositoryAuthorityAdmissionErrorV1::Unavailable)?,
        facts.continuity().manifest_id(),
        facts.continuity().guard_kind(),
        facts.continuity().state_token(),
    )?;
    let mut successor_references = snapshot_object.references().to_vec();
    successor_references.extend([
        snapshot_object.id(),
        guard_object.id(),
        state_object.id(),
        basis_object.id(),
        request_object.id(),
        receipt_object.id(),
        result_object.id(),
        effect_object.id(),
    ]);
    successor_references.sort_unstable();
    successor_references.dedup();
    let successor_snapshot = authority_object(
        AuthoritySchemaV1::BootstrapAuthoritySnapshot,
        successor_facts.schema_value()?,
        successor_references,
    )?;
    Ok(ContinuedRepositoryActionV1 {
        current_snapshot_id: snapshot_object.id(),
        successor_snapshot,
    })
}

fn reference_closure_contains(
    root: &StoreObjectV1,
    target: StoreObjectIdV1,
    objects: &[StoreObjectV1],
) -> Result<bool, RepositoryAuthorityAdmissionErrorV1> {
    let mut pending = root.references().to_vec();
    let mut visited = BTreeSet::new();
    while let Some(candidate) = pending.pop() {
        if candidate == target {
            return Ok(true);
        }
        if !visited.insert(candidate) {
            continue;
        }
        let mut matches = objects.iter().filter(|object| object.id() == candidate);
        let object = matches
            .next()
            .ok_or(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier)?;
        if matches.next().is_some() || visited.len() > objects.len() {
            return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
        }
        pending.extend(object.references());
    }
    Ok(false)
}

#[expect(
    clippy::too_many_arguments,
    reason = "continuing authority validation compares the exact current generation, snapshot, basis, continuity state, and guard carriers without an ambiguous aggregate"
)]
fn validate_continuing_execution_authority_currentness(
    authority: &ExecutionAuthorityV1,
    current_generation: &StoreGenerationV1,
    active_objects: &[StoreObjectV1],
    snapshot_object: &StoreObjectV1,
    facts: &BootstrapAuthoritySnapshotV1,
    basis_object: &StoreObjectV1,
    state_object: &StoreObjectV1,
    guard_object: &StoreObjectV1,
) -> Result<(), RepositoryAuthorityAdmissionErrorV1> {
    match authority {
        ExecutionAuthorityV1::Ordinary(authority) => {
            let selection = authority.selection();
            if facts.actor_binding().id() != selection.actor_binding_id()
                || facts.actor_session().id() != selection.actor_session_id()
                || facts.actor_binding().principal_id() != authority.executor_principal_id()
            {
                return Err(RepositoryAuthorityAdmissionErrorV1::AuthoritySelectionMismatch);
            }
            let authority_objects = active_objects
                .iter()
                .filter(|object| {
                    current_generation.roots().contains(&object.id())
                        || snapshot_object.references().contains(&object.id())
                })
                .collect::<Vec<_>>();
            let resolved = resolve_repository_authority_chain(
                facts,
                selection.terminal_grant_id(),
                &authority_objects,
            )?;
            let required_scope = ScopeAtomV1::new(
                authority.action().literal(),
                &render_digest(authority.subject_commitment()),
                facts.snapshot().subject_revision,
            )?;
            validate_ordinary_authority(
                facts.snapshot(),
                facts.actor_binding(),
                facts.actor_session(),
                resolved.terminal_grant.grant(),
                &required_scope,
                facts.revocations().revocations(),
            )?;
        }
        ExecutionAuthorityV1::BootstrapG0(authority) => {
            validate_bootstrap_execution_authority(
                authority,
                authority.action(),
                current_generation,
                active_objects,
                snapshot_object,
                facts,
            )?;
        }
        ExecutionAuthorityV1::ContinuityMaintenance(authority) => {
            let cma_slot_schema = ContinuityMaintenanceExecutionSlotV1::schema_id()?;
            let slots = basis_object
                .references()
                .iter()
                .filter_map(|reference| {
                    active_objects
                        .iter()
                        .find(|object| object.id() == *reference)
                        .filter(|object| object.schema_id() == cma_slot_schema)
                })
                .collect::<Vec<_>>();
            let [slot_object] = slots.as_slice() else {
                return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
            };
            let slot = parse_cma_execution_slot(slot_object)?;
            if !cma_slot_matches_authority(slot, authority, facts, state_object, guard_object)? {
                return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
            }
        }
    }
    Ok(())
}

#[expect(
    clippy::too_many_arguments,
    reason = "specialized admission binds the complete current Authority and Store cut"
)]
fn admit_specialized_repository_execution_action(
    request_id: ActionRequestIdV1,
    authority: ExecutionAuthorityV1,
    current_generation: &StoreGenerationV1,
    active_objects: &[StoreObjectV1],
    snapshot_object: &StoreObjectV1,
    facts: &BootstrapAuthoritySnapshotV1,
    state_object: &StoreObjectV1,
    guard_object: &StoreObjectV1,
    current_continuity_state: &SuccessVisibleAuthorityContinuityStateV1,
) -> Result<AdmittedRepositoryActionV1, RepositoryAuthorityAdmissionErrorV1> {
    if !facts
        .snapshot()
        .trusted_time
        .is_within(facts.continuity().validity())?
    {
        return Err(RepositoryAuthorityAdmissionErrorV1::Unavailable);
    }
    let action = authority.action();
    let expected_capacity_kind =
        governed_capacity_kind_for(current_generation.domain().role(), true);
    let (capacity_root_id, authority_carrier) = match &authority {
        ExecutionAuthorityV1::BootstrapG0(value) => validate_bootstrap_execution_authority(
            value,
            action,
            current_generation,
            active_objects,
            snapshot_object,
            facts,
        )?,
        ExecutionAuthorityV1::ContinuityMaintenance(value) => validate_cma_execution_authority(
            value,
            active_objects,
            snapshot_object,
            facts,
            state_object,
            guard_object,
        )?,
        ExecutionAuthorityV1::Ordinary(_) => {
            return Err(RepositoryAuthorityAdmissionErrorV1::UnsupportedExecutionAuthority);
        }
    };
    let (current_capacity_root_object, current_capacity_root) = current_capacity_root(
        active_objects,
        current_generation,
        snapshot_object,
        facts.context().context_id(),
        capacity_root_id,
        expected_capacity_kind,
    )?;
    let capacity_transition = current_capacity_root.transition(
        facts.context().context_id(),
        expected_capacity_kind,
        current_capacity_root.spent(),
        CapacityUseDispositionV1::FreshCommit,
    )?;
    let successor_capacity_root = authority_object(
        AuthoritySchemaV1::GovernedCapacityRoot,
        capacity_transition.root().schema_value()?,
        vec![current_capacity_root_object.id()],
    )?;
    let capacity_debit = authority_object(
        AuthoritySchemaV1::GovernedCapacityDebit,
        capacity_transition
            .debit()
            .ok_or(RepositoryAuthorityAdmissionErrorV1::CapacityUnavailable)?
            .schema_value()?,
        vec![
            current_capacity_root_object.id(),
            successor_capacity_root.id(),
        ],
    )?;
    let guard_digest: [u8; 32] =
        Sha256::digest(deterministic_cbor::encode(guard_object.value())?).into();
    let basis_commitment = specialized_repository_action_basis_commitment(
        request_id,
        &authority,
        current_generation,
        facts,
        authority_carrier.id(),
        current_capacity_root_object.id(),
        guard_digest,
    )?;
    let basis_object = authority_object(
        AuthoritySchemaV1::ActionAuthorityBasis,
        CborValue::Array(vec![
            CborValue::Unsigned(authority.basis_kind() as u64),
            bytes(facts.context().context_id().as_bytes()),
            bytes(&basis_commitment),
        ]),
        vec![
            snapshot_object.id(),
            guard_object.id(),
            state_object.id(),
            authority_carrier.id(),
            current_capacity_root_object.id(),
        ],
    )?;
    let receipt = AuthorizationReceiptV1::new(
        request_id,
        facts.context().context_id(),
        authority.basis_kind(),
        facts.continuity().state_token(),
        facts.continuity().state_token(),
    )?;
    let next_generation = current_generation
        .ordinal()
        .checked_add(1)
        .ok_or(RepositoryAuthorityAdmissionErrorV1::Unavailable)?;
    let successor_facts = facts.continue_at_store_generation(
        next_generation,
        facts.continuity().manifest_id(),
        facts.continuity().guard_kind(),
        facts.continuity().state_token(),
    )?;
    let bootstrap_grant_schema = AuthoritySchemaV1::BootstrapGenesisGrant.id()?;
    let cma_slot_schema = ContinuityMaintenanceExecutionSlotV1::schema_id()?;
    let consumed_cma_slot = matches!(&authority, ExecutionAuthorityV1::ContinuityMaintenance(_))
        .then_some(authority_carrier.id());
    let mut successor_references = vec![
        snapshot_object.id(),
        guard_object.id(),
        state_object.id(),
        basis_object.id(),
        successor_capacity_root.id(),
        capacity_debit.id(),
    ];
    successor_references.extend(active_objects.iter().filter_map(|object| {
        (snapshot_object.references().contains(&object.id())
            && (object.schema_id() == bootstrap_grant_schema
                || object.schema_id() == cma_slot_schema)
            && consumed_cma_slot != Some(object.id()))
        .then_some(object.id())
    }));
    let successor_snapshot = authority_object(
        AuthoritySchemaV1::BootstrapAuthoritySnapshot,
        successor_facts.schema_value()?,
        successor_references,
    )?;
    Ok(AdmittedRepositoryActionV1 {
        request_id,
        action,
        principal_id: facts.actor_binding().principal_id(),
        selection: None,
        authority_context_id: facts.context().context_id(),
        subject_commitment: authority.subject_commitment(),
        subject_basis_commitment: authority.current_state_commitment(),
        exact_payload_commitment: Some(authority.exact_payload_commitment()),
        receipt,
        authority_epoch: facts.snapshot().authority_epoch,
        accepted_h_time: current_continuity_state.accepted_time().lower_bound(),
        basis_object,
        current_snapshot_id: snapshot_object.id(),
        successor_snapshot,
        successor_store_generation: next_generation,
        current_capacity_root_id: current_capacity_root_object.id(),
        successor_capacity_root,
        capacity_debit,
        leaf_authority_carrier: Some(authority_carrier),
        leaf_authority_consumption: None,
        guard_object_id: guard_object.id(),
        state_object_id: state_object.id(),
        state_token: facts.continuity().state_token(),
    })
}

fn validate_bootstrap_execution_authority(
    authority: &BootstrapExecutionAuthorityV1,
    action: RepositoryActionLeafV1,
    current_generation: &StoreGenerationV1,
    active_objects: &[StoreObjectV1],
    snapshot_object: &StoreObjectV1,
    facts: &BootstrapAuthoritySnapshotV1,
) -> Result<(CapacityRootIdV1, StoreObjectV1), RepositoryAuthorityAdmissionErrorV1> {
    let basis = (*authority).basis();
    let required_scope = ScopeAtomV1::new(
        action.literal(),
        &render_digest(authority.subject_commitment()),
        facts.snapshot().subject_revision,
    )?;
    let mut paths = facts
        .g0_candidate_paths()
        .iter()
        .filter(|path| path.genesis_grant_id() == basis.genesis_grant_id)
        .collect::<Vec<_>>();
    if paths.len() != 1 {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    }
    let path = paths
        .pop()
        .expect("invariant: exact one-element Bootstrap G0 path");
    let [capacity_root_id] = path.root_contributions() else {
        return Err(RepositoryAuthorityAdmissionErrorV1::CapacityUnavailable);
    };
    if basis.binding_id != facts.actor_binding().id()
        || basis.session_id != facts.actor_session().id()
        || authority.executor_principal_id() != facts.actor_binding().principal_id()
        || path.store_generation() != current_generation.ordinal()
        || path.store_generation() != facts.snapshot().store_generation
        || path.authority_epoch() != facts.snapshot().authority_epoch
        || path.trust_root_revision() != facts.snapshot().trust_root_revision
        || !path.complete()
        || path.grant().context_id() != facts.context().context_id()
        || path.grant().grantee_principal_id() != facts.actor_binding().principal_id()
        || path.grant().parent_grant_id().is_some()
        || path.grant().delegation_id().is_some()
        || !path.grant().terminal_scope().contains(&required_scope)
        || facts
            .revocations()
            .revocations()
            .contains(RevocationTargetV1::Grant(path.grant().id()))
        || !facts
            .snapshot()
            .trusted_time
            .is_within(path.grant().validity())?
    {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    }
    let schema_id = AuthoritySchemaV1::BootstrapGenesisGrant.id()?;
    let expected_genesis_value = path.genesis_grant().schema_value()?;
    let mut carriers = active_objects
        .iter()
        .filter(|object| object.schema_id() == schema_id)
        .filter(|object| snapshot_object.references().contains(&object.id()))
        .filter(|object| object.value() == &expected_genesis_value)
        .cloned()
        .collect::<Vec<_>>();
    if carriers.len() != 1 {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    }
    Ok((
        *capacity_root_id,
        carriers
            .pop()
            .expect("invariant: exact one-element Bootstrap Grant carrier"),
    ))
}

fn validate_cma_execution_authority(
    authority: &ContinuityMaintenanceExecutionAuthorityV1,
    active_objects: &[StoreObjectV1],
    snapshot_object: &StoreObjectV1,
    facts: &BootstrapAuthoritySnapshotV1,
    state_object: &StoreObjectV1,
    guard_object: &StoreObjectV1,
) -> Result<(CapacityRootIdV1, StoreObjectV1), RepositoryAuthorityAdmissionErrorV1> {
    let schema_id = ContinuityMaintenanceExecutionSlotV1::schema_id()?;
    let mut carriers = active_objects
        .iter()
        .filter(|object| object.schema_id() == schema_id)
        .filter(|object| snapshot_object.references().contains(&object.id()))
        .filter_map(|object| {
            parse_cma_execution_slot(object)
                .ok()
                .filter(|slot| {
                    cma_slot_matches_authority(*slot, authority, facts, state_object, guard_object)
                        .unwrap_or(false)
                })
                .map(|slot| (object.clone(), slot))
        })
        .collect::<Vec<_>>();
    if carriers.len() != 1 {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    }
    let (carrier, slot) = carriers
        .pop()
        .expect("invariant: exact one-element CMA execution slot");
    Ok((slot.capacity_root_id, carrier))
}

fn cma_slot_matches_authority(
    slot: ContinuityMaintenanceExecutionSlotV1,
    authority: &ContinuityMaintenanceExecutionAuthorityV1,
    facts: &BootstrapAuthoritySnapshotV1,
    state_object: &StoreObjectV1,
    guard_object: &StoreObjectV1,
) -> Result<bool, RepositoryAuthorityAdmissionErrorV1> {
    let basis = (*authority).basis();
    let request_scope_commitment = cma_request_scope_commitment(
        authority.purpose(),
        authority.action(),
        (*authority).withdrawal_slot_family(),
        authority.subject_commitment(),
    )?;
    let job_applicability_commitment = cma_job_applicability_commitment(
        facts.context().context_id(),
        basis.cma_branch_id,
        authority.purpose(),
        authority.action(),
        facts.continuity().state_token(),
        state_object.id(),
        guard_object.id(),
        facts.snapshot().authority_epoch,
    )?;
    Ok(slot.context_id == facts.context().context_id()
        && slot.cma_branch_id == basis.cma_branch_id
        && slot.slot_id == basis.slot_id
        && slot.executor_assertion_id == basis.executor_assertion_id
        && slot.executor_principal_id == authority.executor_principal_id()
        && slot.purpose == authority.purpose()
        && slot.action == authority.action()
        && slot.withdrawal_slot_family == (*authority).withdrawal_slot_family()
        && slot.subject_commitment == authority.subject_commitment()
        && slot.request_scope_commitment == request_scope_commitment
        && slot.continuity_state_token == facts.continuity().state_token()
        && slot.continuity_state_token == authority.continuity_state_token()
        && slot.continuity_state_object_id == state_object.id()
        && slot.continuity_state_object_id == authority.continuity_state_object_id()
        && slot.guard_object_id == guard_object.id()
        && slot.guard_object_id == authority.guard_object_id()
        && slot.authority_epoch == facts.snapshot().authority_epoch
        && slot.authority_epoch == authority.authority_epoch()
        && slot.job_applicability_commitment == job_applicability_commitment
        && slot.job_applicability_commitment == authority.job_applicability_commitment())
}

fn parse_cma_execution_slot(
    object: &StoreObjectV1,
) -> Result<ContinuityMaintenanceExecutionSlotV1, RepositoryAuthorityAdmissionErrorV1> {
    if object.schema_id() != ContinuityMaintenanceExecutionSlotV1::schema_id()? {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    }
    let CborValue::Array(fields) = object.value() else {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    };
    let [
        CborValue::Text(domain),
        context,
        branch,
        slot,
        assertion,
        executor,
        CborValue::Unsigned(purpose),
        CborValue::Text(action),
        withdrawal_family,
        subject_commitment,
        request_scope_commitment,
        continuity_state_token,
        continuity_state_object_id,
        guard_object_id,
        CborValue::Unsigned(authority_epoch),
        job_applicability_commitment,
        capacity_root,
    ] = fields.as_slice()
    else {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    };
    if domain != CMA_EXECUTION_SLOT_VALUE_DOMAIN_V1 {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    }
    let action = RepositoryActionLeafV1::ALL
        .into_iter()
        .find(|candidate| candidate.literal() == action)
        .ok_or(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier)?;
    let withdrawal_slot_family = match withdrawal_family {
        CborValue::Array(values) if values.as_slice() == [CborValue::Unsigned(0)] => None,
        CborValue::Array(values) if values.len() == 2 => {
            let [CborValue::Unsigned(1), CborValue::Unsigned(tag)] = values.as_slice() else {
                return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
            };
            let tag = u8::try_from(*tag)
                .map_err(|_| RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier)?;
            Some(
                CmaEffectWithdrawalSlotFamilyV1::try_from(tag)
                    .map_err(|_| RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier)?,
            )
        }
        _ => return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier),
    };
    let slot = ContinuityMaintenanceExecutionSlotV1::new(
        AuthorityContextIdV1::from_digest(exact_digest(context)?),
        CmaBranchIdV1::from_digest(exact_digest(branch)?),
        SlotIdV1::from_digest(exact_digest(slot)?),
        ExecutorAssertionIdV1::from_digest(exact_digest(assertion)?),
        PrincipalIdV1::from_digest(exact_digest(executor)?),
        CmaObservationPublicationPurposeV1::try_from(
            u8::try_from(*purpose)
                .map_err(|_| RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier)?,
        )
        .map_err(|_| RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier)?,
        action,
        withdrawal_slot_family,
        exact_digest(subject_commitment)?,
        StateTokenIdV1::from_digest(exact_digest(continuity_state_token)?),
        StoreObjectIdV1::from_digest(exact_digest(continuity_state_object_id)?),
        StoreObjectIdV1::from_digest(exact_digest(guard_object_id)?),
        *authority_epoch,
        CapacityRootIdV1::from_digest(exact_digest(capacity_root)?),
    )?;
    if slot.request_scope_commitment != exact_digest(request_scope_commitment)?
        || slot.job_applicability_commitment != exact_digest(job_applicability_commitment)?
        || slot.schema_value()? != *object.value()
    {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    }
    Ok(slot)
}

fn specialized_repository_action_basis_commitment(
    request_id: ActionRequestIdV1,
    authority: &ExecutionAuthorityV1,
    generation: &StoreGenerationV1,
    facts: &BootstrapAuthoritySnapshotV1,
    authority_carrier_id: StoreObjectIdV1,
    capacity_root_object_id: StoreObjectIdV1,
    guard_digest: [u8; 32],
) -> Result<[u8; 32], CborError> {
    let specialized_basis = match authority {
        ExecutionAuthorityV1::BootstrapG0(value) => {
            let basis = (*value).basis();
            CborValue::Array(vec![
                bytes(basis.binding_id.as_bytes()),
                bytes(basis.session_id.as_bytes()),
                bytes(basis.genesis_grant_id.as_bytes()),
            ])
        }
        ExecutionAuthorityV1::ContinuityMaintenance(value) => {
            let basis = (*value).basis();
            CborValue::Array(vec![
                bytes(basis.cma_branch_id.as_bytes()),
                bytes(basis.slot_id.as_bytes()),
                bytes(basis.executor_assertion_id.as_bytes()),
                CborValue::optional(
                    (*value)
                        .withdrawal_slot_family()
                        .map(|family| CborValue::Unsigned(family as u64)),
                ),
            ])
        }
        ExecutionAuthorityV1::Ordinary(_) => CborValue::Array(Vec::new()),
    };
    hash(&CborValue::Array(vec![
        CborValue::text("maestro.vnext.specialized-repository-action-authority-basis.v1")?,
        bytes(request_id.as_bytes()),
        CborValue::Unsigned(authority.basis_kind() as u64),
        CborValue::text(authority.action().literal())?,
        bytes(&authority.subject_commitment()),
        bytes(&authority.current_state_commitment()),
        bytes(&authority.exact_payload_commitment()),
        bytes(authority.executor_principal_id().as_bytes()),
        specialized_basis,
        bytes(generation.id().as_bytes()),
        bytes(generation.contract_root_id().as_bytes()),
        bytes(facts.context().context_id().as_bytes()),
        CborValue::Unsigned(facts.snapshot().authority_epoch),
        bytes(authority_carrier_id.as_bytes()),
        bytes(capacity_root_object_id.as_bytes()),
        bytes(&guard_digest),
        bytes(facts.continuity().state_token().as_bytes()),
    ]))
}

pub(crate) fn validate_persisted_repository_action_basis(
    generation: &StoreGenerationV1,
    request_id: ActionRequestIdV1,
    request_object_id: StoreObjectIdV1,
    authority: &ExecutionAuthorityV1,
    basis_object: &StoreObjectV1,
    active_objects: &[StoreObjectV1],
) -> Result<ValidatedRepositoryActionBasisV1, RepositoryAuthorityAdmissionErrorV1> {
    if basis_object.schema_id() != AuthoritySchemaV1::ActionAuthorityBasis.id()? {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    }
    let expected_context_kind = authority_context_kind_for_role(generation.domain().role());
    let referenced = direct_references(basis_object, active_objects)?;
    let snapshot_object =
        one_schema_object(&referenced, AuthoritySchemaV1::BootstrapAuthoritySnapshot)?;
    let guard_object = one_schema_object(&referenced, AuthoritySchemaV1::AdmittedTransitionGuard)?;
    let state_object = one_schema_object(
        &referenced,
        AuthoritySchemaV1::SuccessVisibleAuthorityContinuityState,
    )?;
    let facts =
        BootstrapAuthoritySnapshotV1::from_canonical_bytes(&object_value_bytes(&snapshot_object)?)?;
    if !generation.roots().contains(&snapshot_object.id())
        || facts.context().kind() != expected_context_kind
        || facts.context().store_generation() != generation.ordinal()
        || facts.snapshot().store_generation != generation.ordinal()
    {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidCurrentAuthority);
    }
    let manifest = authority_manifest_for_role(generation.domain().role())?;
    let continuity_state =
        validate_current_guard(generation, &facts, &manifest, &state_object, &guard_object)?;
    if !matches!(authority, ExecutionAuthorityV1::Ordinary(_)) {
        return validate_persisted_specialized_repository_action_basis(
            generation,
            request_id,
            request_object_id,
            authority,
            basis_object,
            active_objects,
            &snapshot_object,
            &guard_object,
            &state_object,
            &facts,
            &continuity_state,
        );
    }
    let authority = authority
        .ordinary()
        .expect("invariant: ordinary branch was selected above");
    let selection = authority.selection();
    if facts.actor_binding().id() != selection.actor_binding_id()
        || facts.actor_session().id() != selection.actor_session_id()
        || facts.actor_binding().principal_id() != authority.executor_principal_id()
    {
        return Err(RepositoryAuthorityAdmissionErrorV1::AuthoritySelectionMismatch);
    }
    let authority_objects = active_objects
        .iter()
        .filter(|object| {
            generation.roots().contains(&object.id())
                || snapshot_object.references().contains(&object.id())
        })
        .collect::<Vec<_>>();
    let resolved = resolve_repository_authority_chain(
        &facts,
        selection.terminal_grant_id(),
        &authority_objects,
    )?;
    let selected_grant = &resolved.terminal_grant;
    let expected_capacity_kind = governed_capacity_kind_for(
        generation.domain().role(),
        authority.action().is_external_effect_action(),
    );
    let (capacity_root_object, capacity_root) = current_capacity_root(
        active_objects,
        generation,
        &snapshot_object,
        facts.context().context_id(),
        selected_grant.capacity_root_id(),
        expected_capacity_kind,
    )?;
    if capacity_root.kind() != expected_capacity_kind {
        return Err(RepositoryAuthorityAdmissionErrorV1::CapacityUnavailable);
    }
    let required_scope = ScopeAtomV1::new(
        authority.action().literal(),
        &render_digest(authority.subject_commitment()),
        facts.snapshot().subject_revision,
    )?;
    validate_ordinary_authority(
        facts.snapshot(),
        facts.actor_binding(),
        facts.actor_session(),
        selected_grant.grant(),
        &required_scope,
        facts.revocations().revocations(),
    )?;
    if !facts
        .snapshot()
        .trusted_time
        .is_within(facts.continuity().validity())?
    {
        return Err(RepositoryAuthorityAdmissionErrorV1::Unavailable);
    }
    let guard_digest: [u8; 32] =
        Sha256::digest(deterministic_cbor::encode(guard_object.value())?).into();
    let expected_commitment = repository_action_basis_commitment(
        request_id,
        authority.action(),
        authority.subject_commitment(),
        authority.subject_basis_commitment(),
        Some(authority.exact_payload_commitment()),
        Some(authority.executor_principal_id()),
        generation,
        &facts,
        selection,
        None,
        guard_digest,
    )?;
    let expected_basis_value = CborValue::Array(vec![
        CborValue::Unsigned(ActionAuthorityBasisKindV1::OrdinaryLiveRuntime as u64),
        bytes(facts.context().context_id().as_bytes()),
        bytes(&expected_commitment),
    ]);
    let mut expected_references = vec![
        snapshot_object.id(),
        guard_object.id(),
        state_object.id(),
        resolved.terminal_grant_object_id,
        capacity_root_object.id(),
    ];
    expected_references.sort_unstable();
    expected_references.dedup();
    if basis_object.value() != &expected_basis_value
        || basis_object.references() != expected_references.as_slice()
    {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    }
    let expected_receipt = AuthorizationReceiptV1::new(
        request_id,
        facts.context().context_id(),
        ActionAuthorityBasisKindV1::OrdinaryLiveRuntime,
        facts.continuity().state_token(),
        facts.continuity().state_token(),
    )?;
    let receipt_schema = AuthoritySchemaV1::AuthorizationReceipt.id()?;
    let matching_receipts = active_objects
        .iter()
        .filter(|object| object.schema_id() == receipt_schema)
        .filter(|object| {
            let CborValue::Array(fields) = object.value() else {
                return false;
            };
            let [
                receipt,
                context,
                request,
                basis,
                CborValue::Unsigned(protocol),
                CborValue::Bool(committed),
                result,
            ] = fields.as_slice()
            else {
                return false;
            };
            exact_digest(receipt).ok() == Some(*expected_receipt.id().as_bytes())
                && exact_digest(context).ok() == Some(*facts.context().context_id().as_bytes())
                && exact_digest(request).ok() == Some(*request_id.as_bytes())
                && exact_digest(basis).ok() == Some(*basis_object.id().as_bytes())
                && *protocol == ORDINARY_REPOSITORY_ACTION_PROTOCOL_VERSION_V1
                && *committed
                && exact_digest(result).is_ok_and(|value| value != [0; 32])
                && object.references().contains(&request_object_id)
                && object.references().contains(&basis_object.id())
        })
        .collect::<Vec<_>>();
    if matching_receipts.len() != 1 || continuity_state.accepted_time().lower_bound() == 0 {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    }
    Ok(ValidatedRepositoryActionBasisV1 {
        authority_epoch: facts.snapshot().authority_epoch,
        receipt: expected_receipt,
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "persisted specialized validation replays the complete exact-basis publication cut"
)]
fn validate_persisted_specialized_repository_action_basis(
    generation: &StoreGenerationV1,
    request_id: ActionRequestIdV1,
    request_object_id: StoreObjectIdV1,
    authority: &ExecutionAuthorityV1,
    basis_object: &StoreObjectV1,
    active_objects: &[StoreObjectV1],
    snapshot_object: &StoreObjectV1,
    guard_object: &StoreObjectV1,
    state_object: &StoreObjectV1,
    facts: &BootstrapAuthoritySnapshotV1,
    continuity_state: &SuccessVisibleAuthorityContinuityStateV1,
) -> Result<ValidatedRepositoryActionBasisV1, RepositoryAuthorityAdmissionErrorV1> {
    if !facts
        .snapshot()
        .trusted_time
        .is_within(facts.continuity().validity())?
    {
        return Err(RepositoryAuthorityAdmissionErrorV1::Unavailable);
    }
    let (capacity_root_id, authority_carrier) = match authority {
        ExecutionAuthorityV1::BootstrapG0(value) => validate_bootstrap_execution_authority(
            value,
            authority.action(),
            generation,
            active_objects,
            snapshot_object,
            facts,
        )?,
        ExecutionAuthorityV1::ContinuityMaintenance(value) => validate_cma_execution_authority(
            value,
            active_objects,
            snapshot_object,
            facts,
            state_object,
            guard_object,
        )?,
        ExecutionAuthorityV1::Ordinary(_) => {
            return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
        }
    };
    let expected_capacity_kind = governed_capacity_kind_for(generation.domain().role(), true);
    let (capacity_root_object, capacity_root) = current_capacity_root(
        active_objects,
        generation,
        snapshot_object,
        facts.context().context_id(),
        capacity_root_id,
        expected_capacity_kind,
    )?;
    if capacity_root.kind() != expected_capacity_kind {
        return Err(RepositoryAuthorityAdmissionErrorV1::CapacityUnavailable);
    }
    let guard_digest: [u8; 32] =
        Sha256::digest(deterministic_cbor::encode(guard_object.value())?).into();
    let expected_commitment = specialized_repository_action_basis_commitment(
        request_id,
        authority,
        generation,
        facts,
        authority_carrier.id(),
        capacity_root_object.id(),
        guard_digest,
    )?;
    let expected_basis_value = CborValue::Array(vec![
        CborValue::Unsigned(authority.basis_kind() as u64),
        bytes(facts.context().context_id().as_bytes()),
        bytes(&expected_commitment),
    ]);
    let mut expected_references = vec![
        snapshot_object.id(),
        guard_object.id(),
        state_object.id(),
        authority_carrier.id(),
        capacity_root_object.id(),
    ];
    expected_references.sort_unstable();
    expected_references.dedup();
    if basis_object.value() != &expected_basis_value
        || basis_object.references() != expected_references.as_slice()
    {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    }
    let expected_receipt = AuthorizationReceiptV1::new(
        request_id,
        facts.context().context_id(),
        authority.basis_kind(),
        facts.continuity().state_token(),
        facts.continuity().state_token(),
    )?;
    let receipt_schema = AuthoritySchemaV1::AuthorizationReceipt.id()?;
    let matching_receipts = active_objects
        .iter()
        .filter(|object| object.schema_id() == receipt_schema)
        .filter(|object| {
            let CborValue::Array(fields) = object.value() else {
                return false;
            };
            let [
                receipt,
                context,
                request,
                basis,
                CborValue::Unsigned(protocol),
                CborValue::Bool(committed),
                result,
            ] = fields.as_slice()
            else {
                return false;
            };
            exact_digest(receipt).ok() == Some(*expected_receipt.id().as_bytes())
                && exact_digest(context).ok() == Some(*facts.context().context_id().as_bytes())
                && exact_digest(request).ok() == Some(*request_id.as_bytes())
                && exact_digest(basis).ok() == Some(*basis_object.id().as_bytes())
                && *protocol == ORDINARY_REPOSITORY_ACTION_PROTOCOL_VERSION_V1
                && *committed
                && exact_digest(result).is_ok_and(|value| value != [0; 32])
                && object.references().contains(&request_object_id)
                && object.references().contains(&basis_object.id())
        })
        .count();
    if matching_receipts != 1 || continuity_state.accepted_time().lower_bound() == 0 {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    }
    Ok(ValidatedRepositoryActionBasisV1 {
        authority_epoch: facts.snapshot().authority_epoch,
        receipt: expected_receipt,
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "the canonical basis hash binds every ordinary action authority dimension"
)]
fn repository_action_basis_commitment(
    request_id: ActionRequestIdV1,
    action: RepositoryActionLeafV1,
    subject_commitment: [u8; 32],
    subject_basis_commitment: [u8; 32],
    exact_payload_commitment: Option<[u8; 32]>,
    executor_principal_id: Option<PrincipalIdV1>,
    generation: &StoreGenerationV1,
    facts: &BootstrapAuthoritySnapshotV1,
    selection: RepositoryAuthoritySelectionV1,
    specialized_leaf_commitment: Option<[u8; 32]>,
    guard_digest: [u8; 32],
) -> Result<[u8; 32], CborError> {
    hash(&CborValue::Array(vec![
        CborValue::text("maestro.vnext.repository-action-authority-basis.v1")?,
        bytes(request_id.as_bytes()),
        CborValue::text(action.literal())?,
        CborValue::Unsigned(action.global_tag()),
        CborValue::Unsigned(action.owner_tag()),
        CborValue::Unsigned(action.family_tag()),
        CborValue::Unsigned(action.local_tag()),
        CborValue::text(action.owner_descriptor_id())?,
        CborValue::text(action.descriptor_id())?,
        CborValue::Unsigned(action.protocol_revision()),
        CborValue::text(action.manifest_id())?,
        CborValue::text(action.grammar_id())?,
        bytes(&subject_commitment),
        bytes(&subject_basis_commitment),
        CborValue::optional(exact_payload_commitment.map(|commitment| bytes(&commitment))),
        CborValue::optional(
            executor_principal_id.map(|principal_id| bytes(principal_id.as_bytes())),
        ),
        bytes(generation.id().as_bytes()),
        bytes(generation.contract_root_id().as_bytes()),
        bytes(facts.context().context_id().as_bytes()),
        CborValue::Unsigned(facts.snapshot().authority_epoch),
        bytes(selection.actor_binding_id().as_bytes()),
        bytes(selection.actor_session_id().as_bytes()),
        bytes(selection.terminal_grant_id().as_bytes()),
        CborValue::optional(specialized_leaf_commitment.map(|commitment| bytes(&commitment))),
        bytes(&guard_digest),
        bytes(facts.continuity().state_token().as_bytes()),
    ]))
}

fn resolve_repository_authority_chain(
    facts: &BootstrapAuthoritySnapshotV1,
    terminal_grant_id: GrantIdV1,
    authority_objects: &[&StoreObjectV1],
) -> Result<ResolvedRepositoryAuthorityChainV1, RepositoryAuthorityAdmissionErrorV1> {
    let grant_schema = OrdinaryBoundedGrantV1::store_schema_id()?;
    let grants = authority_objects
        .iter()
        .filter(|object| object.schema_id() == grant_schema)
        .map(|object| {
            Ok((
                *object,
                OrdinaryBoundedGrantV1::from_canonical_bytes(&deterministic_cbor::encode(
                    object.value(),
                )?)?,
            ))
        })
        .collect::<Result<Vec<_>, RepositoryAuthorityAdmissionErrorV1>>()?;
    let mut terminal_grants = grants
        .iter()
        .filter(|(_, grant)| grant.grant().id() == terminal_grant_id)
        .collect::<Vec<_>>();
    if terminal_grants.len() != 1 {
        return Err(RepositoryAuthorityAdmissionErrorV1::AuthoritySelectionMismatch);
    }
    let (terminal_grant_object, terminal_grant) = terminal_grants
        .pop()
        .expect("invariant: exact one-element terminal ordinary Grant");
    let expected_context_id = facts.context().context_id();
    let expected_capacity_root_id = terminal_grant.capacity_root_id();

    let delegation_schema = OrdinaryGrantDelegationV1::store_schema_id()?;
    let mut delegations = Vec::new();
    for object in authority_objects
        .iter()
        .filter(|object| object.schema_id() == delegation_schema)
    {
        let encoded = deterministic_cbor::encode(object.value())?;
        let mut carriers = grants
            .iter()
            .filter_map(|(_, child)| {
                OrdinaryGrantDelegationV1::from_canonical_bytes(&encoded, child).ok()
            })
            .collect::<Vec<_>>();
        if carriers.len() != 1 {
            return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
        }
        delegations.push(StoredOrdinaryDelegationV1 {
            object,
            carrier: carriers
                .pop()
                .expect("invariant: exact one-element canonical Delegation carrier"),
        });
    }

    let mut visited = BTreeSet::new();
    let mut reverse_chain = Vec::new();
    let mut child_object = *terminal_grant_object;
    let mut child = terminal_grant;
    let root_path = loop {
        if !visited.insert(child.grant().id())
            || child.grant().context_id() != expected_context_id
            || child.capacity_root_id() != expected_capacity_root_id
            || !facts
                .snapshot()
                .trusted_time
                .is_within(child.grant().validity())?
        {
            return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
        }
        let mut child_delegations = delegations
            .iter()
            .filter(|entry| {
                entry.carrier.delegation().child_grant_id == child.grant().id()
                    && entry.carrier.context_id() == expected_context_id
                    && entry.carrier.capacity_root_id() == expected_capacity_root_id
            })
            .collect::<Vec<_>>();
        if child_delegations.len() != 1 {
            return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
        }
        let delegation = child_delegations
            .pop()
            .expect("invariant: exact one-element Delegation for ancestry hop");
        reverse_chain.push((child_object, child, delegation));

        let parent_id = child
            .grant()
            .parent_grant_id()
            .ok_or(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier)?;
        let ordinary_parents = grants
            .iter()
            .filter(|(_, grant)| grant.grant().id() == parent_id)
            .collect::<Vec<_>>();
        let root_paths = facts
            .g0_candidate_paths()
            .iter()
            .filter(|path| path.grant().id() == parent_id)
            .collect::<Vec<_>>();
        if ordinary_parents.len() + root_paths.len() != 1 {
            return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
        }
        if let [(parent_object, parent)] = ordinary_parents.as_slice() {
            child_object = *parent_object;
            child = parent;
            continue;
        }
        break root_paths[0];
    };

    if !root_path.complete()
        || root_path.store_generation() != facts.context().store_generation()
        || root_path.store_generation() != facts.snapshot().store_generation
        || root_path.authority_epoch() != facts.snapshot().authority_epoch
        || root_path.trust_root_revision() != facts.snapshot().trust_root_revision
        || root_path.grant().context_id() != expected_context_id
        || root_path
            .root_contributions()
            .iter()
            .any(|root_id| *root_id != expected_capacity_root_id)
        || !facts
            .snapshot()
            .trusted_time
            .is_within(root_path.grant().validity())?
    {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    }

    let mut ancestry_grant_ids = vec![root_path.grant().id()];
    let mut ancestry_principal_ids = vec![root_path.grant().grantee_principal_id()];
    let mut has_bounded_root = root_path
        .root_contributions()
        .contains(&expected_capacity_root_id);
    let mut structural_parent = root_path.grant().definition();
    structural_parent.delegation_depth_remaining = u8::MAX;
    let mut parent = structural_parent.validate()?;
    for (_, child, delegation) in reverse_chain.iter().rev() {
        let ancestry = DelegationAncestryV1::new(
            ancestry_grant_ids.clone(),
            ancestry_principal_ids.clone(),
            has_bounded_root,
        )?;
        validate_delegation(
            &parent,
            child.grant(),
            &delegation.carrier.delegation(),
            &ancestry,
        )?;
        ancestry_grant_ids.push(child.grant().id());
        ancestry_principal_ids.push(child.grant().grantee_principal_id());
        has_bounded_root = true;
        parent = child.grant().clone();
    }

    let chain_grants = reverse_chain
        .iter()
        .map(|(_, grant, _)| (*grant).clone())
        .collect::<Vec<_>>();
    if grant_is_revoked_by_closure(
        terminal_grant,
        &chain_grants,
        facts.revocations().revocations(),
    )? {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    }
    let terminal_delegation = reverse_chain
        .first()
        .expect("invariant: ordinary Grant ancestry contains its terminal hop")
        .2;
    let validated_ordinary_ancestry_object_ids = reverse_chain
        .iter()
        .flat_map(|(grant_object, _, delegation)| [grant_object.id(), delegation.object.id()])
        .collect();
    Ok(ResolvedRepositoryAuthorityChainV1 {
        terminal_grant_object_id: terminal_grant_object.id(),
        terminal_grant: terminal_grant.clone(),
        terminal_delegation_object_id: terminal_delegation.object.id(),
        validated_ordinary_ancestry_object_ids,
    })
}

pub(crate) fn admit_repository_authority_candidate(
    facts: &BootstrapAuthoritySnapshotV1,
    expected_capacity_root_id: CapacityRootIdV1,
    candidate: &OrdinaryBoundedGrantV1,
    delegation: &OrdinaryGrantDelegationV1,
) -> Result<(), RepositoryAuthorityAdmissionErrorV1> {
    if candidate.capacity_root_id() != expected_capacity_root_id
        || delegation.capacity_root_id() != expected_capacity_root_id
    {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    }
    let parent_id = candidate
        .grant()
        .parent_grant_id()
        .ok_or(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier)?;
    let mut parents = facts
        .g0_candidate_paths()
        .iter()
        .filter(|path| {
            path.grant().id() == parent_id
                && path.grant().context_id() == candidate.grant().context_id()
                && path.store_generation() == facts.context().store_generation()
                && path.complete()
        })
        .collect::<Vec<_>>();
    if parents.len() != 1 {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    }
    let parent = parents
        .pop()
        .expect("invariant: exact one-element root-attached Grant parent");
    let ancestry = DelegationAncestryV1::new(
        vec![parent.grant().id()],
        vec![parent.grant().grantee_principal_id()],
        false,
    )?;
    let mut structural_parent = parent.grant().definition();
    structural_parent.delegation_depth_remaining = u8::MAX;
    validate_delegation(
        &structural_parent.validate()?,
        candidate.grant(),
        &delegation.delegation(),
        &ancestry,
    )?;
    Ok(())
}

fn validate_current_guard(
    current_generation: &StoreGenerationV1,
    facts: &BootstrapAuthoritySnapshotV1,
    manifest: &AuthorityContinuityManifestV1,
    state_object: &StoreObjectV1,
    guard_object: &StoreObjectV1,
) -> Result<SuccessVisibleAuthorityContinuityStateV1, RepositoryAuthorityAdmissionErrorV1> {
    let state = SuccessVisibleAuthorityContinuityStateV1::decode(
        &object_value_bytes(state_object)?,
        manifest,
    )?;
    let CborValue::Array(guard_fields) = guard_object.value() else {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidCurrentGuard);
    };
    let CborValue::Array(state_fields) = state_object.value() else {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidCurrentGuard);
    };
    let guard_digest: [u8; 32] =
        Sha256::digest(deterministic_cbor::encode(guard_object.value())?).into();
    let expected_context_kind = facts.context().kind();
    if guard_fields.len() != 26
        || state_fields.len() != 26
        || !matches!(&guard_fields[0], CborValue::Text(domain) if domain == "maestro.vnext.authority-transition-guard-evaluation.v1")
        || guard_fields[3] != CborValue::Unsigned(expected_context_kind as u64)
        || exact_digest(&guard_fields[4])? != *facts.context().context_id().as_bytes()
        || !matches!(guard_fields[5], CborValue::Unsigned(value) if value > 0 && value <= current_generation.ordinal())
        || guard_fields[6] != CborValue::Unsigned(facts.snapshot().authority_epoch)
        || exact_digest(&guard_fields[7])? != *facts.continuity().manifest_id().as_bytes()
        || exact_digest(&guard_fields[8])? != *state.closure_id().as_bytes()
        || exact_digest(&state_fields[25])? != guard_digest
        || state.context_kind() != expected_context_kind
        || state.context_id() != facts.context().context_id()
        || state.store_generation() > current_generation.ordinal()
        || state.authority_epoch() != facts.snapshot().authority_epoch
        || state.manifest_id() != facts.continuity().manifest_id()
        || state.state_token() != facts.continuity().state_token()
        || state.guard_kind() != facts.continuity().guard_kind()
    {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidCurrentGuard);
    }
    Ok(state)
}

fn current_capacity_root(
    active_objects: &[StoreObjectV1],
    current_generation: &StoreGenerationV1,
    snapshot_object: &StoreObjectV1,
    context_id: AuthorityContextIdV1,
    expected_id: CapacityRootIdV1,
    expected_kind: GovernedCapacityKindV1,
) -> Result<(StoreObjectV1, GovernedCapacityRootV1), RepositoryAuthorityAdmissionErrorV1> {
    let schema_id = AuthoritySchemaV1::GovernedCapacityRoot.id()?;
    let mut roots = active_objects
        .iter()
        .filter(|object| object.schema_id() == schema_id)
        .filter(|object| {
            current_generation.roots().contains(&object.id())
                || snapshot_object.references().contains(&object.id())
        })
        .filter_map(|object| {
            parse_capacity_root(object)
                .ok()
                .filter(|root| root.id() == expected_id)
                .map(|root| (object.clone(), root))
        })
        .collect::<Vec<_>>();
    if roots.len() != 1 {
        return Err(RepositoryAuthorityAdmissionErrorV1::CapacityUnavailable);
    }
    let (object, root) = roots
        .pop()
        .expect("invariant: exact one-element governed-capacity root");
    if root.context_kind() != expected_kind.context_kind()
        || root.context_id() != context_id
        || root.kind() != expected_kind
    {
        return Err(RepositoryAuthorityAdmissionErrorV1::CapacityUnavailable);
    }
    Ok((object, root))
}

fn parse_capacity_root(
    object: &StoreObjectV1,
) -> Result<GovernedCapacityRootV1, RepositoryAuthorityAdmissionErrorV1> {
    let CborValue::Array(fields) = object.value() else {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    };
    if fields.len() != 7
        || !matches!(&fields[0], CborValue::Text(domain) if domain == GovernedCapacityRootV1::SCHEMA_DOMAIN)
    {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    }
    let context_kind = AuthorityContextKindV1::try_from(
        u8::try_from(exact_unsigned(&fields[2])?)
            .map_err(|_| RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier)?,
    )
    .map_err(|_| RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier)?;
    let capacity_kind_tag = u8::try_from(exact_unsigned(&fields[4])?)
        .map_err(|_| RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier)?;
    let capacity_kind = match context_kind {
        AuthorityContextKindV1::RepositoryAuthorityContext => GovernedCapacityKindV1::Repository(
            RepositoryGovernedCapacitySlotKindV1::try_from(capacity_kind_tag)
                .map_err(|_| RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier)?,
        ),
        AuthorityContextKindV1::InstallationAuthorityContext => {
            GovernedCapacityKindV1::Installation(
                InstallationGovernedCapacitySlotKindV1::try_from(capacity_kind_tag)
                    .map_err(|_| RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier)?,
            )
        }
    };
    let initial_max = u32::try_from(exact_unsigned(&fields[5])?)
        .map_err(|_| RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier)?;
    let spent = u32::try_from(exact_unsigned(&fields[6])?)
        .map_err(|_| RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier)?;
    Ok(GovernedCapacityRootV1::from_persisted_state(
        CapacityRootIdV1::from_digest(exact_digest(&fields[1])?),
        context_kind,
        AuthorityContextIdV1::from_digest(exact_digest(&fields[3])?),
        capacity_kind,
        initial_max,
        spent,
    )?)
}

fn authority_context_kind_for_role(role: StoreRoleV1) -> AuthorityContextKindV1 {
    match role {
        StoreRoleV1::Repository => AuthorityContextKindV1::RepositoryAuthorityContext,
        StoreRoleV1::Installation => AuthorityContextKindV1::InstallationAuthorityContext,
    }
}

fn authority_manifest_for_role(
    role: StoreRoleV1,
) -> Result<AuthorityContinuityManifestV1, super::super::AuthorityContinuityError> {
    match role {
        StoreRoleV1::Repository => AuthorityContinuityManifestV1::repository(),
        StoreRoleV1::Installation => AuthorityContinuityManifestV1::installation(),
    }
}

fn governed_capacity_kind_for(role: StoreRoleV1, external_effect: bool) -> GovernedCapacityKindV1 {
    match (role, external_effect) {
        (StoreRoleV1::Repository, false) => GovernedCapacityKindV1::Repository(
            RepositoryGovernedCapacitySlotKindV1::RepositoryOrdinaryMutation,
        ),
        (StoreRoleV1::Repository, true) => GovernedCapacityKindV1::Repository(
            RepositoryGovernedCapacitySlotKindV1::RepositoryExternalEffect,
        ),
        (StoreRoleV1::Installation, false) => GovernedCapacityKindV1::Installation(
            InstallationGovernedCapacitySlotKindV1::InstallationDistributionMutation,
        ),
        (StoreRoleV1::Installation, true) => GovernedCapacityKindV1::Installation(
            InstallationGovernedCapacitySlotKindV1::InstallationExternalEffect,
        ),
    }
}

fn exact_unsigned(value: &CborValue) -> Result<u64, RepositoryAuthorityAdmissionErrorV1> {
    match value {
        CborValue::Unsigned(value) => Ok(*value),
        _ => Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier),
    }
}

fn direct_references(
    object: &StoreObjectV1,
    active_objects: &[StoreObjectV1],
) -> Result<Vec<StoreObjectV1>, RepositoryAuthorityAdmissionErrorV1> {
    object
        .references()
        .iter()
        .map(|reference| {
            active_objects
                .iter()
                .find(|candidate| candidate.id() == *reference)
                .cloned()
                .ok_or(RepositoryAuthorityAdmissionErrorV1::InvalidCurrentAuthority)
        })
        .collect()
}

fn one_schema_object(
    objects: &[StoreObjectV1],
    schema: AuthoritySchemaV1,
) -> Result<StoreObjectV1, RepositoryAuthorityAdmissionErrorV1> {
    let schema_id = schema.id()?;
    let mut matches = objects
        .iter()
        .filter(|object| object.schema_id() == schema_id)
        .cloned()
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidCurrentAuthority);
    }
    Ok(matches
        .pop()
        .expect("invariant: exact one-element Authority schema match"))
}

fn authority_object(
    schema: AuthoritySchemaV1,
    value: CborValue,
    mut references: Vec<StoreObjectIdV1>,
) -> Result<StoreObjectV1, RepositoryAuthorityAdmissionErrorV1> {
    if !schema.accepts_value(&value) {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier);
    }
    references.sort_unstable();
    references.dedup();
    Ok(StoreObjectV1::new(schema.id()?, value, references)?)
}

fn object_value_bytes(
    object: &StoreObjectV1,
) -> Result<Vec<u8>, RepositoryAuthorityAdmissionErrorV1> {
    Ok(deterministic_cbor::encode(object.value())?)
}

fn exact_digest(value: &CborValue) -> Result<[u8; 32], RepositoryAuthorityAdmissionErrorV1> {
    let CborValue::Bytes(bytes) = value else {
        return Err(RepositoryAuthorityAdmissionErrorV1::InvalidCurrentGuard);
    };
    bytes
        .as_slice()
        .try_into()
        .map_err(|_| RepositoryAuthorityAdmissionErrorV1::InvalidCurrentGuard)
}

fn hash(value: &CborValue) -> Result<[u8; 32], CborError> {
    Ok(Sha256::digest(deterministic_cbor::encode(value)?).into())
}

fn bytes(value: &[u8]) -> CborValue {
    CborValue::Bytes(value.to_vec())
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
pub(crate) enum RepositoryAuthorityAdmissionErrorV1 {
    #[error("Repository action Authority is unavailable")]
    Unavailable,
    #[error("the current Repository Authority snapshot is absent, ambiguous, or stale")]
    InvalidCurrentAuthority,
    #[error("the current Repository Authority transition guard is substituted or stale")]
    InvalidCurrentGuard,
    #[error("the selected Binding, Session, or Grant does not match current Authority facts")]
    AuthoritySelectionMismatch,
    #[error("the selected Repository action capacity basis is unavailable")]
    CapacityUnavailable,
    #[error("the selected Execution action requires its exact non-ordinary Authority basis")]
    UnsupportedExecutionAuthority,
    #[error("the Authority carrier has an invalid canonical schema shape")]
    InvalidAuthorityCarrier,
    #[error("a committed Repository action must produce at least one owner object")]
    InvalidProducedObjects,
    #[error(transparent)]
    Store(#[from] crate::domain::persistence::StoreError),
    #[error(transparent)]
    StoreObject(#[from] StoreObjectError),
    #[error(transparent)]
    Identity(#[from] crate::domain::identity::IdentityError),
    #[error(transparent)]
    AuthorityValidation(#[from] AuthorityValidationError),
    #[error(transparent)]
    Capacity(#[from] super::super::CapacityError),
    #[error(transparent)]
    AuthoritySnapshot(#[from] BootstrapAuthoritySnapshotErrorV1),
    #[error(transparent)]
    AuthorityState(#[from] super::super::AuthorityContinuityStateError),
    #[error(transparent)]
    AuthorityContinuity(#[from] super::super::AuthorityContinuityError),
    #[error(transparent)]
    ActionResult(#[from] ActionResultError),
    #[error(transparent)]
    LeafAuthority(#[from] RepositoryLeafAuthorityEvaluationErrorV1),
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::super::super::continuity::StoreAllocatedContinuityStateTokenV1;
    use super::super::super::*;
    use super::*;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) enum AuthorityFixtureModeV1 {
        Valid,
        MultiHop,
        MultiHopCycle,
        MultiHopExpiredAncestor,
        MultiHopForeignRoot,
        MultiHopNonAttenuated,
        MultiHopRevokedAncestor,
        MultiHopStaleRoot,
        OrphanedGrant,
        RevokedGrant,
        ExpiredGrant,
        SubstitutedGuard,
    }

    pub(crate) type CmaFixtureBasisV1 = (
        RepositoryActionLeafV1,
        ContinuityMaintenanceAuthorityBasisV1,
        Option<CmaEffectWithdrawalSlotFamilyV1>,
        CmaObservationPublicationPurposeV1,
        [u8; 32],
    );

    pub(crate) struct RepositoryAuthorityFixtureV1 {
        pub(crate) objects: Vec<StoreObjectV1>,
        pub(crate) authority_root_id: StoreObjectIdV1,
        pub(crate) selection: RepositoryAuthoritySelectionV1,
        pub(crate) actor_principal: PrincipalIdV1,
        pub(crate) bootstrap_basis: BootstrapControlG0AuthorityBasisV1,
        pub(crate) cma_bases: Vec<CmaFixtureBasisV1>,
        pub(crate) continuity_state_token: StateTokenIdV1,
        pub(crate) continuity_state_object_id: StoreObjectIdV1,
        pub(crate) guard_object_id: StoreObjectIdV1,
        pub(crate) authority_epoch: u64,
        pub(crate) authenticated_human: RepositoryAuthenticatedHumanV1,
        pub(crate) leaf_authority_expires_at: u64,
    }

    pub(crate) fn repository_authority_fixture(
        scopes: Vec<(&'static str, [u8; 32])>,
        mode: AuthorityFixtureModeV1,
    ) -> RepositoryAuthorityFixtureV1 {
        repository_authority_fixture_at(scopes, mode, 120, 130)
    }

    pub(crate) fn repository_authority_fixture_at(
        scopes: Vec<(&'static str, [u8; 32])>,
        mode: AuthorityFixtureModeV1,
        trusted_time_lower: u64,
        trusted_time_upper: u64,
    ) -> RepositoryAuthorityFixtureV1 {
        authority_fixture(
            scopes,
            mode,
            StoreRoleV1::Repository,
            trusted_time_lower,
            trusted_time_upper,
            false,
        )
    }

    pub(crate) fn repository_owner_family_authority_fixture(
        scopes: Vec<(&'static str, [u8; 32])>,
        mode: AuthorityFixtureModeV1,
    ) -> RepositoryAuthorityFixtureV1 {
        authority_fixture(scopes, mode, StoreRoleV1::Repository, 120, 130, true)
    }

    pub(crate) fn installation_authority_fixture(
        scopes: Vec<(&'static str, [u8; 32])>,
        mode: AuthorityFixtureModeV1,
    ) -> RepositoryAuthorityFixtureV1 {
        authority_fixture(scopes, mode, StoreRoleV1::Installation, 120, 130, false)
    }

    fn authority_fixture(
        scopes: Vec<(&'static str, [u8; 32])>,
        mode: AuthorityFixtureModeV1,
        role: StoreRoleV1,
        trusted_time_lower: u64,
        trusted_time_upper: u64,
        allow_owner_family_scopes: bool,
    ) -> RepositoryAuthorityFixtureV1 {
        let external_effect_only = scopes.iter().all(|(literal, _)| {
            RepositoryActionLeafV1::ALL
                .into_iter()
                .find(|leaf| leaf.literal() == *literal)
                .is_some_and(RepositoryActionLeafV1::is_external_effect_action)
        });
        let multi_hop = matches!(
            mode,
            AuthorityFixtureModeV1::MultiHop
                | AuthorityFixtureModeV1::MultiHopCycle
                | AuthorityFixtureModeV1::MultiHopExpiredAncestor
                | AuthorityFixtureModeV1::MultiHopForeignRoot
                | AuthorityFixtureModeV1::MultiHopNonAttenuated
                | AuthorityFixtureModeV1::MultiHopRevokedAncestor
                | AuthorityFixtureModeV1::MultiHopStaleRoot
        );
        let manifest = match role {
            StoreRoleV1::Repository => AuthorityContinuityManifestV1::repository().unwrap(),
            StoreRoleV1::Installation => AuthorityContinuityManifestV1::installation().unwrap(),
        };
        let context_id = AuthorityContextIdV1::derive(match role {
            StoreRoleV1::Repository => "stage3-repository-context",
            StoreRoleV1::Installation => "stage4-installation-context",
        })
        .unwrap();
        let (closure, guard, state) = continuity_generation(
            &manifest,
            context_id,
            trusted_time_lower,
            trusted_time_upper,
        );
        let manifest_object = authority_object(
            AuthoritySchemaV1::AuthorityContinuityManifest,
            manifest.schema_value().unwrap(),
            vec![],
        )
        .unwrap();
        let closure_object = authority_object(
            AuthoritySchemaV1::AuthorityContinuityClosure,
            closure.schema_value().unwrap(),
            vec![],
        )
        .unwrap();
        let guard_object = authority_object(
            AuthoritySchemaV1::AdmittedTransitionGuard,
            guard.schema_value().unwrap(),
            vec![closure_object.id()],
        )
        .unwrap();
        let state_object = authority_object(
            AuthoritySchemaV1::SuccessVisibleAuthorityContinuityState,
            state.schema_value().unwrap(),
            vec![closure_object.id(), guard_object.id()],
        )
        .unwrap();
        let selected_guard = if mode == AuthorityFixtureModeV1::SubstitutedGuard {
            let CborValue::Array(mut fields) = guard.schema_value().unwrap() else {
                unreachable!("guard schema is an array")
            };
            fields[4] = bytes(&[93; 32]);
            authority_object(
                AuthoritySchemaV1::AdmittedTransitionGuard,
                CborValue::Array(fields),
                vec![closure_object.id()],
            )
            .unwrap()
        } else {
            guard_object.clone()
        };

        let validity = HalfOpenValidityV1::new(100, 200).unwrap();
        let context = match role {
            StoreRoleV1::Repository => AuthorityContextV1::repository(
                context_id,
                "stage3-repository-installation",
                1,
                7,
                11,
            )
            .unwrap(),
            StoreRoleV1::Installation => AuthorityContextV1::installation(
                context_id,
                "stage4-installation",
                "stage4-global-user-agent-realm",
                1,
                7,
                11,
                1,
            )
            .unwrap(),
        };
        let actor_principal = PrincipalIdV1::derive("stage3-actor-principal").unwrap();
        let actor_binding = PrincipalBindingV1::new(
            PrincipalBindingIdV1::derive("stage3-actor-binding").unwrap(),
            actor_principal,
            context_id,
            11,
            1,
            validity,
            false,
        )
        .unwrap();
        let responder_binding = PrincipalBindingV1::new(
            PrincipalBindingIdV1::derive("stage3-responder-binding").unwrap(),
            PrincipalIdV1::derive("stage3-responder-principal").unwrap(),
            context_id,
            11,
            1,
            validity,
            true,
        )
        .unwrap();
        let target_head = StateTokenIdV1::derive("stage3-target-head").unwrap();
        let target = TargetActionProjectionV1::new(
            BootstrapMandateTargetV1::RotateRecoveryCommitmentSelection,
            "stage3-recovery-selection",
            1,
            TargetActionOwnerV1::Authority,
            TargetActionProtocolV1::RecoveryCommitmentSelection,
            TargetActionEffectKindV1::Rotate,
            "sha256:stage3-effect-closure",
            TargetExpectedHeadsV1::new(context_id, 1, 7, 11, 1, target_head).unwrap(),
            validity,
        )
        .unwrap();
        let target_commitment = target.target_action_commitment().unwrap();
        let request_commitment = target_commitment.render();
        let actor_session = SessionV1::new(
            SessionIdV1::derive("stage3-actor-session").unwrap(),
            actor_binding.id(),
            context_id,
            1,
            7,
            &request_commitment,
            validity,
        )
        .unwrap();
        let responder_session = SessionV1::new(
            SessionIdV1::derive("stage3-responder-session").unwrap(),
            responder_binding.id(),
            context_id,
            1,
            7,
            &request_commitment,
            validity,
        )
        .unwrap();
        let consent = ConsentSlotEvaluationFactsV1::derive_for_target(&target, validity).unwrap();
        let procedure = StateTokenIdV1::derive("stage3-interaction-procedure").unwrap();
        let subject = BootstrapInteractionSubjectV1::new(
            context_id,
            StateTokenIdV1::derive("stage3-interaction-plan").unwrap(),
            ActionRequestIdV1::derive("stage3-interaction-attempt").unwrap(),
            responder_binding.id(),
            1,
            target_commitment,
            consent.binding().clone(),
            StateTokenIdV1::derive("stage3-option-map").unwrap(),
            StateTokenIdV1::derive("stage3-affirmative-option").unwrap(),
        );
        let presentation = BootstrapMandatePresentationObservationV1::new(
            subject.clone(),
            StateTokenIdV1::derive("stage3-interaction-carrier").unwrap(),
            procedure,
        )
        .unwrap();
        let response = BootstrapMandateResponseObservationV1::new(
            subject,
            presentation.id(),
            BootstrapResponseDispositionV1::Affirmative,
            StateTokenIdV1::derive("stage3-affirmative-option").unwrap(),
        )
        .unwrap();
        let interaction = BootstrapMandateInteractionObservationJoinV1::new(
            &presentation,
            &response,
            responder_session.id(),
            procedure,
        )
        .unwrap();

        let scope_entries = scopes
            .into_iter()
            .map(|(action, subject)| {
                let leaf = RepositoryActionLeafV1::ALL
                    .into_iter()
                    .find(|leaf| leaf.literal() == action)
                    .unwrap();
                assert!(
                    leaf.stage5_owner_dispatch().is_stage5_admitted()
                        || (allow_owner_family_scopes
                            && matches!(leaf, RepositoryActionLeafV1::Downstream(_))),
                    "ordinary Stage 5 fixtures cannot mint a scope for owner-unavailable Action {action}"
                );
                (
                    leaf,
                    ScopeAtomV1::new(action, &render_digest(subject), 1).unwrap(),
                    subject,
                )
            })
            .collect::<Vec<_>>();
        let ordinary_scope = scope_entries
            .iter()
            .filter(|(leaf, _, _)| {
                !matches!(
                    leaf.execution_authority_basis(),
                    Some(ActionAuthorityBasisKindV1::BootstrapControlG0)
                        | Some(ActionAuthorityBasisKindV1::ContinuityMaintenance)
                )
            })
            .map(|(_, scope, _)| scope.clone())
            .collect::<Vec<_>>();
        let mut bootstrap_terminal_scope =
            vec![ScopeAtomV1::new("IssueBootstrapMandate", &request_commitment, 1).unwrap()];
        bootstrap_terminal_scope.extend(
            scope_entries
                .iter()
                .filter(|(leaf, _, _)| {
                    leaf.execution_authority_basis()
                        == Some(ActionAuthorityBasisKindV1::BootstrapControlG0)
                })
                .map(|(_, scope, _)| scope.clone()),
        );
        let bootstrap_execution_enabled = scope_entries.iter().any(|(leaf, _, _)| {
            leaf.execution_authority_basis() == Some(ActionAuthorityBasisKindV1::BootstrapControlG0)
        });
        let capacity_root_id = CapacityRootIdV1::derive("stage3-ordinary-capacity").unwrap();
        let bootstrap_grant = GrantDefinitionV1 {
            id: GrantIdV1::derive("stage3-bootstrap-only-grant").unwrap(),
            context_id,
            grantee_principal_id: if bootstrap_execution_enabled {
                actor_principal
            } else {
                PrincipalIdV1::derive("stage3-g0-principal").unwrap()
            },
            parent_grant_id: None,
            delegation_id: None,
            terminal_scope: GrantScopeV1::new(bootstrap_terminal_scope).unwrap(),
            delegable_scope: GrantScopeV1::new(ordinary_scope.clone()).unwrap(),
            validity,
            delegation_depth_remaining: 1,
            authority_use_constraint: AuthorityUseConstraintV1::NoLocalBoundedRoot,
        }
        .validate()
        .unwrap();
        let bootstrap_grant_id = bootstrap_grant.id();
        let bootstrap_genesis_grant_id =
            GenesisGrantIdV1::derive(&bootstrap_grant_id.render()).unwrap();
        let bootstrap_path = BootstrapG0PathV1::new(
            bootstrap_genesis_grant_id,
            bootstrap_grant,
            if mode == AuthorityFixtureModeV1::MultiHopStaleRoot {
                0
            } else {
                1
            },
            7,
            11,
            true,
            vec![capacity_root_id],
        )
        .unwrap();

        let ordinary_parent_grant_id = GrantIdV1::derive("stage3-ordinary-parent-grant").unwrap();
        let ordinary_parent_delegation_id =
            DelegationIdV1::derive("stage3-ordinary-parent-delegation").unwrap();
        let ordinary_grant_id = GrantIdV1::derive("stage3-ordinary-grant").unwrap();
        let ordinary_delegation_id = DelegationIdV1::derive("stage3-ordinary-delegation").unwrap();
        let ordinary_parent_capacity_root_id =
            if mode == AuthorityFixtureModeV1::MultiHopForeignRoot {
                CapacityRootIdV1::derive("stage3-foreign-ordinary-capacity").unwrap()
            } else {
                capacity_root_id
            };
        let ordinary_parent = multi_hop.then(|| {
            OrdinaryBoundedGrantV1::new(
                GrantDefinitionV1 {
                    id: ordinary_parent_grant_id,
                    context_id,
                    grantee_principal_id: PrincipalIdV1::derive("stage3-intermediate-principal")
                        .unwrap(),
                    parent_grant_id: Some(if mode == AuthorityFixtureModeV1::MultiHopCycle {
                        ordinary_grant_id
                    } else {
                        bootstrap_grant_id
                    }),
                    delegation_id: Some(ordinary_parent_delegation_id),
                    terminal_scope: GrantScopeV1::new(vec![]).unwrap(),
                    delegable_scope: GrantScopeV1::new(ordinary_scope.clone()).unwrap(),
                    validity: if mode == AuthorityFixtureModeV1::MultiHopExpiredAncestor {
                        HalfOpenValidityV1::new(10, 20).unwrap()
                    } else {
                        validity
                    },
                    delegation_depth_remaining: if mode
                        == AuthorityFixtureModeV1::MultiHopNonAttenuated
                    {
                        0
                    } else {
                        1
                    },
                    authority_use_constraint: AuthorityUseConstraintV1::BoundedBy(
                        ordinary_parent_capacity_root_id,
                    ),
                }
                .validate()
                .unwrap(),
            )
            .unwrap()
        });
        let ordinary_parent_delegation = ordinary_parent.as_ref().map(|parent| {
            OrdinaryGrantDelegationV1::new(
                context_id,
                ordinary_parent_capacity_root_id,
                DelegationV1::new(
                    ordinary_parent_delegation_id,
                    if mode == AuthorityFixtureModeV1::MultiHopCycle {
                        ordinary_grant_id
                    } else {
                        bootstrap_grant_id
                    },
                    ordinary_parent_grant_id,
                ),
                parent,
            )
            .unwrap()
        });
        let ordinary_validity = if mode == AuthorityFixtureModeV1::ExpiredGrant {
            HalfOpenValidityV1::new(10, 20).unwrap()
        } else {
            validity
        };
        let ordinary_grant = GrantDefinitionV1 {
            id: ordinary_grant_id,
            context_id,
            grantee_principal_id: actor_principal,
            parent_grant_id: Some(if mode == AuthorityFixtureModeV1::OrphanedGrant {
                GrantIdV1::derive("stage3-orphaned-parent-grant").unwrap()
            } else {
                ordinary_parent
                    .as_ref()
                    .map_or(bootstrap_grant_id, |parent| parent.grant().id())
            }),
            delegation_id: Some(ordinary_delegation_id),
            terminal_scope: GrantScopeV1::new(ordinary_scope).unwrap(),
            delegable_scope: GrantScopeV1::new(vec![]).unwrap(),
            validity: ordinary_validity,
            delegation_depth_remaining: 0,
            authority_use_constraint: AuthorityUseConstraintV1::BoundedBy(capacity_root_id),
        }
        .validate()
        .unwrap();
        let ordinary_grant = OrdinaryBoundedGrantV1::new(ordinary_grant).unwrap();
        let ordinary_delegation = OrdinaryGrantDelegationV1::new(
            context_id,
            capacity_root_id,
            DelegationV1::new(
                ordinary_delegation_id,
                if mode == AuthorityFixtureModeV1::OrphanedGrant {
                    GrantIdV1::derive("stage3-orphaned-parent-grant").unwrap()
                } else {
                    ordinary_parent
                        .as_ref()
                        .map_or(bootstrap_grant_id, |parent| parent.grant().id())
                },
                ordinary_grant_id,
            ),
            &ordinary_grant,
        )
        .unwrap();
        let revocations = match mode {
            AuthorityFixtureModeV1::RevokedGrant => {
                RevocationSetV1::new(vec![RevocationTargetV1::Grant(ordinary_grant_id)]).unwrap()
            }
            AuthorityFixtureModeV1::MultiHopRevokedAncestor => {
                RevocationSetV1::new(vec![RevocationTargetV1::Grant(ordinary_parent_grant_id)])
                    .unwrap()
            }
            _ => RevocationSetV1::empty(),
        };
        let revocations = AuthorityRevocationSetV1::new(context_id, revocations);
        let facts = BootstrapAuthoritySnapshotV1::new(
            context,
            AuthoritySnapshotV1::new(
                context_id,
                1,
                7,
                11,
                1,
                TrustedTimeV1::verified(trusted_time_lower, trusted_time_upper).unwrap(),
            ),
            actor_binding,
            actor_session,
            responder_binding,
            responder_session,
            vec![bootstrap_path],
            revocations,
            Some(interaction),
            procedure,
            target,
            target_head,
            consent,
            BootstrapContinuityTransitionProofV1::new(
                context_id,
                1,
                7,
                11,
                manifest.id(),
                state.guard_kind(),
                state.state_token(),
                validity,
            ),
        )
        .unwrap();

        let context_object = authority_object(
            AuthoritySchemaV1::AuthorityContext,
            facts.context().schema_value().unwrap(),
            vec![],
        )
        .unwrap();
        let actor_binding_object = authority_object(
            AuthoritySchemaV1::PrincipalBinding,
            facts.actor_binding().schema_value().unwrap(),
            vec![],
        )
        .unwrap();
        let responder_binding_object = authority_object(
            AuthoritySchemaV1::PrincipalBinding,
            facts.responder_binding().schema_value().unwrap(),
            vec![],
        )
        .unwrap();
        let actor_session_object = authority_object(
            AuthoritySchemaV1::Session,
            facts.actor_session().schema_value().unwrap(),
            vec![actor_binding_object.id()],
        )
        .unwrap();
        let responder_session_object = authority_object(
            AuthoritySchemaV1::Session,
            facts.responder_session().schema_value().unwrap(),
            vec![responder_binding_object.id()],
        )
        .unwrap();
        let bootstrap_grant_object = authority_object(
            AuthoritySchemaV1::BootstrapGenesisGrant,
            facts.g0_candidate_paths()[0]
                .genesis_grant()
                .schema_value()
                .unwrap(),
            vec![],
        )
        .unwrap();
        let ordinary_parent_grant_object = ordinary_parent.as_ref().map(|parent| {
            authority_object(
                AuthoritySchemaV1::OrdinaryBoundedGrant,
                parent.schema_value().unwrap(),
                vec![bootstrap_grant_object.id()],
            )
            .unwrap()
        });
        let ordinary_parent_delegation_object = ordinary_parent_delegation.as_ref().map(|entry| {
            authority_object(
                AuthoritySchemaV1::OrdinaryGrantDelegation,
                entry.schema_value().unwrap(),
                vec![
                    ordinary_parent_grant_object.as_ref().unwrap().id(),
                    bootstrap_grant_object.id(),
                ],
            )
            .unwrap()
        });
        let revocations_object = authority_object(
            AuthoritySchemaV1::RevocationSet,
            facts.revocations().schema_value().unwrap(),
            vec![],
        )
        .unwrap();
        let interaction_object = authority_object(
            AuthoritySchemaV1::BootstrapMandateInteractionObservationJoin,
            facts.interaction_join().unwrap().schema_value().unwrap(),
            vec![responder_session_object.id()],
        )
        .unwrap();
        let consent_object = authority_object(
            AuthoritySchemaV1::ConsentSlotBindingParameter,
            facts.consent_slot().binding().schema_value().unwrap(),
            vec![],
        )
        .unwrap();
        let ordinary_grant_object = authority_object(
            AuthoritySchemaV1::OrdinaryBoundedGrant,
            ordinary_grant.schema_value().unwrap(),
            vec![
                ordinary_parent_grant_object
                    .as_ref()
                    .map_or(bootstrap_grant_object.id(), StoreObjectV1::id),
            ],
        )
        .unwrap();
        let ordinary_delegation_object = authority_object(
            AuthoritySchemaV1::OrdinaryGrantDelegation,
            ordinary_delegation.schema_value().unwrap(),
            vec![
                ordinary_grant_object.id(),
                ordinary_parent_grant_object
                    .as_ref()
                    .map_or(bootstrap_grant_object.id(), StoreObjectV1::id),
            ],
        )
        .unwrap();
        let capacity_kind = governed_capacity_kind_for(role, external_effect_only);
        let capacity_root = GovernedCapacityRootV1::new(
            capacity_root_id,
            capacity_kind.context_kind(),
            context_id,
            capacity_kind,
            32,
        )
        .unwrap();
        let capacity_root_object = authority_object(
            AuthoritySchemaV1::GovernedCapacityRoot,
            capacity_root.schema_value().unwrap(),
            vec![],
        )
        .unwrap();
        let cma_branch_id = CmaBranchIdV1::derive(&format!(
            "stage4-cma-current-branch-{}",
            render_digest(*context_id.as_bytes())
        ))
        .unwrap();
        let continuity_state_token = state.state_token();
        let continuity_state_object_id = state_object.id();
        let guard_object_id = selected_guard.id();
        let authority_epoch = facts.snapshot().authority_epoch;
        let cma_carriers = scope_entries
            .iter()
            .enumerate()
            .filter(|(_, (leaf, _, _))| {
                leaf.execution_authority_basis()
                    == Some(ActionAuthorityBasisKindV1::ContinuityMaintenance)
            })
            .map(|(phase_ordinal, (action, _, subject_commitment))| {
                let seed = action.literal();
                let basis = ContinuityMaintenanceAuthorityBasisV1::new(
                    cma_branch_id,
                    SlotIdV1::derive(&format!("stage4-cma-slot-{seed}-{phase_ordinal}")).unwrap(),
                    ExecutorAssertionIdV1::derive(&format!(
                        "stage4-cma-executor-{seed}-{phase_ordinal}"
                    ))
                    .unwrap(),
                );
                let purpose = CmaObservationPublicationPurposeV1::MaintenanceExecutorCurrentness;
                let withdrawal_slot_family = (*action
                    == RepositoryActionLeafV1::WithdrawContinuityMaintenanceEffect)
                    .then_some(purpose.effect_withdrawal_slot_family());
                let carrier = ContinuityMaintenanceExecutionSlotV1::new(
                    context_id,
                    basis.cma_branch_id,
                    basis.slot_id,
                    basis.executor_assertion_id,
                    actor_principal,
                    purpose,
                    *action,
                    withdrawal_slot_family,
                    *subject_commitment,
                    continuity_state_token,
                    continuity_state_object_id,
                    guard_object_id,
                    authority_epoch,
                    capacity_root_id,
                )
                .unwrap()
                .store_object({
                    let mut references = vec![
                        capacity_root_object.id(),
                        state_object.id(),
                        selected_guard.id(),
                    ];
                    references.sort_unstable();
                    references
                })
                .unwrap();
                let slot = parse_cma_execution_slot(&carrier).unwrap();
                (
                    *action,
                    basis,
                    withdrawal_slot_family,
                    purpose,
                    slot.job_applicability_commitment,
                    carrier,
                )
            })
            .collect::<Vec<_>>();
        let mut authority_root_references = vec![
            manifest_object.id(),
            closure_object.id(),
            selected_guard.id(),
            state_object.id(),
            context_object.id(),
            actor_binding_object.id(),
            responder_binding_object.id(),
            actor_session_object.id(),
            responder_session_object.id(),
            bootstrap_grant_object.id(),
            revocations_object.id(),
            interaction_object.id(),
            consent_object.id(),
            ordinary_grant_object.id(),
            ordinary_delegation_object.id(),
            capacity_root_object.id(),
        ];
        authority_root_references.extend(
            ordinary_parent_grant_object
                .iter()
                .chain(ordinary_parent_delegation_object.iter())
                .map(StoreObjectV1::id),
        );
        authority_root_references.extend(
            cma_carriers
                .iter()
                .map(|(_, _, _, _, _, object)| object.id()),
        );
        let authority_root = authority_object(
            AuthoritySchemaV1::BootstrapAuthoritySnapshot,
            facts.schema_value().unwrap(),
            authority_root_references,
        )
        .unwrap();
        let mut objects = vec![
            manifest_object,
            closure_object,
            guard_object,
            selected_guard,
            state_object,
            context_object,
            actor_binding_object,
            responder_binding_object,
            actor_session_object,
            responder_session_object,
            bootstrap_grant_object,
            revocations_object,
            interaction_object,
            consent_object,
            authority_root.clone(),
            ordinary_grant_object.clone(),
            ordinary_delegation_object.clone(),
            capacity_root_object.clone(),
        ];
        objects.extend(ordinary_parent_grant_object);
        objects.extend(ordinary_parent_delegation_object);
        objects.extend(
            cma_carriers
                .iter()
                .map(|(_, _, _, _, _, object)| object.clone()),
        );
        objects.sort_by_key(StoreObjectV1::id);
        objects.dedup_by_key(|object| object.id());
        RepositoryAuthorityFixtureV1 {
            objects,
            authority_root_id: authority_root.id(),
            selection: RepositoryAuthoritySelectionV1::new(
                facts.actor_binding().id(),
                facts.actor_session().id(),
                ordinary_grant_id,
            ),
            actor_principal,
            bootstrap_basis: BootstrapControlG0AuthorityBasisV1::new(
                facts.actor_binding().id(),
                facts.actor_session().id(),
                bootstrap_genesis_grant_id,
            ),
            cma_bases: cma_carriers
                .into_iter()
                .map(
                    |(
                        action,
                        basis,
                        withdrawal_slot_family,
                        purpose,
                        job_applicability_commitment,
                        _,
                    )| {
                        (
                            action,
                            basis,
                            withdrawal_slot_family,
                            purpose,
                            job_applicability_commitment,
                        )
                    },
                )
                .collect(),
            continuity_state_token,
            continuity_state_object_id,
            guard_object_id,
            authority_epoch,
            authenticated_human: RepositoryAuthenticatedHumanV1::new(
                facts.responder_binding().id(),
                facts.responder_session().id(),
                facts.responder_session().request_commitment().as_bytes(),
            )
            .unwrap(),
            leaf_authority_expires_at: 190,
        }
    }

    fn continuity_generation(
        manifest: &AuthorityContinuityManifestV1,
        context_id: AuthorityContextIdV1,
        trusted_time_lower: u64,
        trusted_time_upper: u64,
    ) -> (
        AuthorityContinuityClosureV1,
        AdmittedTransitionGuardV1,
        SuccessVisibleAuthorityContinuityStateV1,
    ) {
        let accepted_time = AcceptedAuthorityTimeFloorV1::context_genesis(
            reference("stage3-stable-lineage"),
            reference("stage3-trusted-time-coordinate"),
            reference("stage3-trusted-time-stack"),
            reference("stage3-trusted-time-origin"),
            trusted_time_lower,
            trusted_time_upper,
        )
        .unwrap();
        let allocation = StoreAllocatedContinuityStateTokenV1::from_store_commitments(
            context_id,
            1,
            None,
            1,
            digest("stage3-state-token"),
            digest("stage3-allocation"),
        )
        .unwrap();
        let semantic_cut = AuthorityContinuitySemanticCutV1 {
            cut_sequence: 1,
            source_store_generation: 0,
            successor_store_generation: 1,
            authority_epoch: 7,
            stable_lineage: reference("stage3-stable-lineage"),
            selected_trusted_time_stack: reference("stage3-trusted-time-stack"),
            carrier_profile: ContinuityCarrierProfileStatusV1::Confirmed {
                profile: reference("stage3-carrier-profile"),
                accepted_prefix: reference("stage3-accepted-prefix"),
                handoff_state: reference("stage3-handoff-state"),
                fence: reference("stage3-carrier-fence"),
                currentness: reference("stage3-carrier-currentness"),
            },
            accepted_time,
            lane_state_closure_root: reference("stage3-lane-state-root"),
            source_floor_root: reference("stage3-source-floor-root"),
            gap_companions: vec![],
            floor_provenance: vec![],
            external_revision_cells: vec![],
            cma_remaining_root: reference("stage3-cma-remaining"),
            cma_spent_root: reference("stage3-cma-spent"),
            canonical_records: vec![reference("stage3-canonical-record")],
            graph_nodes: vec![],
            replay_items: vec![],
            historical_spend_items: vec![],
            unresolved_effects: vec![],
        };
        let closure = AuthorityContinuityClosureV1::prove(
            manifest,
            AuthorityContinuityClosureInputV1 {
                manifest_id: manifest.id(),
                context_kind: manifest.context_kind(),
                context_id,
                predecessor: AuthorityContinuityPredecessorV1::ContextGenesis {
                    origin_commitment: reference("stage3-context-genesis-origin"),
                },
                class_entries: continuity_class_entries(manifest, &semantic_cut),
                semantic_cut,
                graph_edges: vec![],
                protocol_version: 1,
            },
            &allocation,
        )
        .unwrap();
        let census = TransitionGuardOwnerCensusV1::externally_rooted_genesis(
            context_id,
            1,
            7,
            reference("stage3-context-genesis-origin"),
        )
        .unwrap();
        let guard = AdmittedTransitionGuardV1::evaluate(AuthorityTransitionGuardAdmissionInputV1 {
            kind: GuardAdmissionKindV1::ExternallyRootedContextGenesis,
            context_kind: manifest.context_kind(),
            context_id,
            store_generation: 1,
            authority_epoch: 7,
            manifest_id: manifest.id(),
            closure_id: closure.id(),
            predecessor_state_token: None,
            cut_sequence: 1,
            selected_trusted_time_stack: closure.selected_trusted_time_stack(),
            carrier_profile: closure.carrier_profile().clone(),
            accepted_time: closure.accepted_time().clone(),
            lane_state_closure_root: closure.lane_state_closure_root(),
            source_floor_root: closure.source_floor_root(),
            gap_companions: vec![],
            floor_provenance: vec![],
            external_revision_cells: vec![],
            cma_remaining_root: closure.cma_remaining_root(),
            cma_spent_root: closure.cma_spent_root(),
            unresolved_effects: vec![],
            term_facts: vec![],
            owner_census: census,
            disclosure: ContinuityDisclosureV1::ProtectedComplete,
            protocol_version: 1,
        })
        .unwrap();
        let state =
            SuccessVisibleAuthorityContinuityStateV1::construct(manifest, &closure, &guard, None)
                .unwrap();
        (closure, guard, state)
    }

    fn continuity_class_entries(
        manifest: &AuthorityContinuityManifestV1,
        cut: &AuthorityContinuitySemanticCutV1,
    ) -> Vec<AuthorityContinuityClassClosureV1> {
        let first_canonical = manifest
            .descriptors()
            .iter()
            .find(|descriptor| descriptor.disposition == ClassDispositionV1::CanonicalRecordClosure)
            .map(|descriptor| descriptor.class_id)
            .unwrap();
        manifest
            .descriptors()
            .iter()
            .map(|descriptor| {
                let facets = ContinuityClosureFacetV1::ALL
                    .into_iter()
                    .map(|facet| {
                        let disposition = match descriptor.disposition {
                            ClassDispositionV1::CanonicalRecordClosure => {
                                let items = if descriptor.class_id == first_canonical {
                                    match facet {
                                        ContinuityClosureFacetV1::CanonicalRecords => {
                                            cut.canonical_records.clone()
                                        }
                                        ContinuityClosureFacetV1::Graph => cut.graph_nodes.clone(),
                                        ContinuityClosureFacetV1::Replay => {
                                            cut.replay_items.clone()
                                        }
                                        ContinuityClosureFacetV1::HistoricalSpend => {
                                            cut.historical_spend_items.clone()
                                        }
                                        ContinuityClosureFacetV1::UnresolvedEffect => {
                                            cut.unresolved_effects.clone()
                                        }
                                    }
                                } else {
                                    vec![]
                                };
                                ClosureFacetDispositionKindV1::ContributesExactRoot(
                                    ContinuityExactRootV1::new(
                                        descriptor.class_id,
                                        facet,
                                        cut.cut_sequence,
                                        items,
                                    )
                                    .unwrap(),
                                )
                            }
                            ClassDispositionV1::DerivedOnly => {
                                ClosureFacetDispositionKindV1::DerivedCheck {
                                    invariant: class_facet_reference(
                                        "invariant",
                                        descriptor.class_id,
                                        facet,
                                        cut.cut_sequence,
                                    ),
                                    proof: class_facet_reference(
                                        "proof",
                                        descriptor.class_id,
                                        facet,
                                        cut.cut_sequence,
                                    ),
                                }
                            }
                        };
                        AuthorityContinuityFacetDispositionV1 { facet, disposition }
                    })
                    .collect();
                AuthorityContinuityClassClosureV1 {
                    class_id: descriptor.class_id,
                    owner: descriptor.owner,
                    facets,
                }
            })
            .collect()
    }

    fn class_facet_reference(
        purpose: &str,
        class_id: ContinuityClassIdV1,
        facet: ContinuityClosureFacetV1,
        cut_sequence: u64,
    ) -> ContinuityReferenceV1 {
        ContinuityReferenceV1::from_digest(
            hash(&CborValue::Array(vec![
                CborValue::text("maestro.vnext.continuity-class-facet-proof.v1").unwrap(),
                CborValue::text(purpose).unwrap(),
                class_id.schema_value(),
                CborValue::Unsigned(facet as u64),
                CborValue::Unsigned(cut_sequence),
            ]))
            .unwrap(),
        )
    }

    fn reference(seed: &str) -> ContinuityReferenceV1 {
        ContinuityReferenceV1::derive(seed).unwrap()
    }

    fn digest(seed: &str) -> [u8; 32] {
        Sha256::digest(seed.as_bytes()).into()
    }
}

#[cfg(test)]
mod ancestry_tests {
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use rusqlite::Connection;

    use super::test_support::{
        AuthorityFixtureModeV1, repository_authority_fixture,
        repository_owner_family_authority_fixture,
    };
    use super::*;
    use crate::domain::authority::IdempotencyKeyIdV1;
    use crate::domain::authority::{
        CoordinationRepositoryActionAuthorityV1, DistributionRepositoryActionAuthorityV1,
        IntakeRepositoryActionAuthorityV1, MemoryRepositoryActionAuthorityV1,
        PersistenceRepositoryActionAuthorityV1, PlanningRepositoryActionAuthorityV1,
        PrincipalBindingIdV1, RepositoryDownstreamActionLeafV1,
        ResearchRepositoryActionAuthorityV1, SearchMaintenanceRepositoryActionAuthorityV1,
    };
    use crate::domain::identity::ContractRootIdV1;
    use crate::domain::persistence::{
        PreparedPublicationError, StoreCompatibilityV1, StoreDomainV1, StoreGenerationV1, StoreV1,
    };
    use crate::domain::repository::{
        CancelWorkPublicationV1, CreateDraftWorkPublicationV1, RepositoryActionIdentityV1,
        RepositoryStoreBasisV1, RepositoryStoreV1,
    };
    use crate::domain::work::{WorkIdV1, WorkRecordV1, WorkRecordWriterV1, WorkTransitionReasonV1};

    fn resolve_fixture(
        mode: AuthorityFixtureModeV1,
    ) -> Result<ResolvedRepositoryAuthorityChainV1, RepositoryAuthorityAdmissionErrorV1> {
        let fixture = repository_authority_fixture(vec![("CancelWork", [41; 32])], mode);
        resolve_fixture_objects(&fixture)
    }

    fn resolve_fixture_objects(
        fixture: &super::test_support::RepositoryAuthorityFixtureV1,
    ) -> Result<ResolvedRepositoryAuthorityChainV1, RepositoryAuthorityAdmissionErrorV1> {
        let snapshot_object = fixture
            .objects
            .iter()
            .find(|object| object.id() == fixture.authority_root_id)
            .unwrap();
        let facts = BootstrapAuthoritySnapshotV1::from_canonical_bytes(
            &object_value_bytes(snapshot_object).unwrap(),
        )
        .unwrap();
        let authority_objects = fixture.objects.iter().collect::<Vec<_>>();
        resolve_repository_authority_chain(
            &facts,
            fixture.selection.terminal_grant_id(),
            &authority_objects,
        )
    }

    fn owner_family_authority_input(
        selection: RepositoryAuthoritySelectionV1,
        action: RepositoryDownstreamActionLeafV1,
        subject_commitment: [u8; 32],
        subject_basis_commitment: [u8; 32],
        exact_payload_commitment: [u8; 32],
    ) -> RepositoryLeafAuthorityInputV1 {
        match action.global_tag() {
            94..=102 => CoordinationRepositoryActionAuthorityV1::new(
                selection,
                action,
                subject_commitment,
                subject_basis_commitment,
                exact_payload_commitment,
            )
            .unwrap()
            .into(),
            103..=106 => PlanningRepositoryActionAuthorityV1::new(
                selection,
                action,
                subject_commitment,
                subject_basis_commitment,
                exact_payload_commitment,
            )
            .unwrap()
            .into(),
            107..=116 => PersistenceRepositoryActionAuthorityV1::new(
                selection,
                action,
                subject_commitment,
                subject_basis_commitment,
                exact_payload_commitment,
            )
            .unwrap()
            .into(),
            117..=129 => DistributionRepositoryActionAuthorityV1::new(
                selection,
                action,
                subject_commitment,
                subject_basis_commitment,
                exact_payload_commitment,
            )
            .unwrap()
            .into(),
            130..=131 => SearchMaintenanceRepositoryActionAuthorityV1::new(
                selection,
                action,
                subject_commitment,
                subject_basis_commitment,
                exact_payload_commitment,
            )
            .unwrap()
            .into(),
            132..=138 => MemoryRepositoryActionAuthorityV1::new(
                selection,
                action,
                subject_commitment,
                subject_basis_commitment,
                exact_payload_commitment,
            )
            .unwrap()
            .into(),
            139..=141 => IntakeRepositoryActionAuthorityV1::new(
                selection,
                action,
                subject_commitment,
                subject_basis_commitment,
                exact_payload_commitment,
            )
            .unwrap()
            .into(),
            142..=145 => ResearchRepositoryActionAuthorityV1::new(
                selection,
                action,
                subject_commitment,
                subject_basis_commitment,
                exact_payload_commitment,
            )
            .unwrap()
            .into(),
            _ => unreachable!("the downstream Action catalog is closed to tags 94..=145"),
        }
    }

    fn owner_family_digest(seed: &str) -> [u8; 32] {
        Sha256::digest(seed.as_bytes()).into()
    }

    #[test]
    fn every_owner_family_wrapper_enters_the_existing_ordinary_admission_pipeline() {
        let subject = owner_family_digest("owner-family-admission-subject");
        let owner_basis = owner_family_digest("owner-family-admission-current-owner-basis");
        let payload = owner_family_digest("owner-family-admission-exact-payload");
        let fixture = repository_owner_family_authority_fixture(
            RepositoryDownstreamActionLeafV1::all()
                .into_iter()
                .map(|action| (action.literal(), subject))
                .collect(),
            AuthorityFixtureModeV1::Valid,
        );
        let root = test_root();
        let domain =
            StoreDomainV1::derive(StoreRoleV1::Repository, b"owner-family-admission").unwrap();
        let mut store = StoreV1::create(&root, domain.clone()).unwrap();
        put_objects_in_reference_order(&mut store, fixture.objects);
        let generation = StoreGenerationV1::new(
            domain,
            1,
            None,
            ContractRootIdV1::parse(&render_digest([71; 32])).unwrap(),
            StoreCompatibilityV1::stage0_successor().unwrap(),
            vec![fixture.authority_root_id],
        )
        .unwrap();
        store.publish_generation(&generation, None).unwrap();
        activate_store(&root);

        for action in RepositoryDownstreamActionLeafV1::all() {
            let request_id = ActionRequestIdV1::derive(&format!(
                "owner-family-admission-request-{}",
                action.global_tag()
            ))
            .unwrap();
            let authority = owner_family_authority_input(
                fixture.selection,
                action,
                subject,
                owner_basis,
                payload,
            );
            let admitted = store
                .with_serialized_active_view(|view| {
                    admit_repository_action(
                        view,
                        &generation,
                        RepositoryActionAdmissionInputV1::new(request_id, authority),
                    )
                })
                .unwrap();

            assert_eq!(
                admitted.action(),
                RepositoryActionLeafV1::Downstream(action)
            );
            assert_eq!(admitted.request_id(), request_id);
            assert_eq!(
                admitted.authorization_receipt().basis_kind(),
                ActionAuthorityBasisKindV1::OrdinaryLiveRuntime
            );
            assert!(admitted.leaf_authority_carrier.is_none());
            assert!(admitted.leaf_authority_consumption.is_none());
        }

        drop(store);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn same_key_substitutions_do_not_alias_and_selection_substitution_fails_closed() {
        const FIRST_FAMILY_TAGS: [u64; 8] = [94, 103, 107, 117, 130, 132, 139, 142];

        let subject = owner_family_digest("owner-family-substitution-subject");
        let owner_basis = owner_family_digest("owner-family-substitution-current-owner-basis");
        let payload = owner_family_digest("owner-family-substitution-exact-payload");
        let fixture = repository_owner_family_authority_fixture(
            RepositoryDownstreamActionLeafV1::all()
                .into_iter()
                .map(|action| (action.literal(), subject))
                .collect(),
            AuthorityFixtureModeV1::Valid,
        );
        let root = test_root();
        let domain =
            StoreDomainV1::derive(StoreRoleV1::Repository, b"owner-family-substitution").unwrap();
        let mut store = StoreV1::create(&root, domain.clone()).unwrap();
        put_objects_in_reference_order(&mut store, fixture.objects);
        let generation = StoreGenerationV1::new(
            domain,
            1,
            None,
            ContractRootIdV1::parse(&render_digest([72; 32])).unwrap(),
            StoreCompatibilityV1::stage0_successor().unwrap(),
            vec![fixture.authority_root_id],
        )
        .unwrap();
        store.publish_generation(&generation, None).unwrap();
        activate_store(&root);

        for first_global_tag in FIRST_FAMILY_TAGS {
            let action =
                RepositoryDownstreamActionLeafV1::from_global_tag(first_global_tag).unwrap();
            let substituted_action =
                RepositoryDownstreamActionLeafV1::from_global_tag(first_global_tag + 1).unwrap();
            let request_id = ActionRequestIdV1::derive(&format!(
                "owner-family-same-key-request-{first_global_tag}"
            ))
            .unwrap();

            let admit = |store: &mut StoreV1,
                         selection: RepositoryAuthoritySelectionV1,
                         action: RepositoryDownstreamActionLeafV1,
                         basis: [u8; 32],
                         payload: [u8; 32]| {
                let authority =
                    owner_family_authority_input(selection, action, subject, basis, payload);
                store.with_serialized_active_view(|view| {
                    admit_repository_action(
                        view,
                        &generation,
                        RepositoryActionAdmissionInputV1::new(request_id, authority),
                    )
                })
            };

            let baseline =
                admit(&mut store, fixture.selection, action, owner_basis, payload).unwrap();
            let replay =
                admit(&mut store, fixture.selection, action, owner_basis, payload).unwrap();
            assert_eq!(baseline.basis_object().id(), replay.basis_object().id());

            let substituted_owner_basis = admit(
                &mut store,
                fixture.selection,
                action,
                owner_family_digest("substituted-current-owner-basis"),
                payload,
            )
            .unwrap();
            assert_ne!(
                baseline.basis_object().id(),
                substituted_owner_basis.basis_object().id()
            );

            let substituted_payload = admit(
                &mut store,
                fixture.selection,
                action,
                owner_basis,
                owner_family_digest("substituted-exact-payload"),
            )
            .unwrap();
            assert_ne!(
                baseline.basis_object().id(),
                substituted_payload.basis_object().id()
            );

            let substituted_leaf = admit(
                &mut store,
                fixture.selection,
                substituted_action,
                owner_basis,
                payload,
            )
            .unwrap();
            assert_ne!(
                baseline.basis_object().id(),
                substituted_leaf.basis_object().id()
            );

            let hostile_selection = RepositoryAuthoritySelectionV1::new(
                PrincipalBindingIdV1::derive(&format!(
                    "owner-family-hostile-binding-{first_global_tag}"
                ))
                .unwrap(),
                fixture.selection.actor_session_id(),
                fixture.selection.terminal_grant_id(),
            );
            assert!(matches!(
                admit(&mut store, hostile_selection, action, owner_basis, payload,),
                Err(PreparedPublicationError::Prepare(
                    RepositoryAuthorityAdmissionErrorV1::AuthoritySelectionMismatch
                ))
            ));
        }

        drop(store);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn store_loaded_chain_accepts_g0_through_two_ordinary_grants() {
        let resolved = resolve_fixture(AuthorityFixtureModeV1::MultiHop).unwrap();

        assert_eq!(
            resolved.terminal_grant.grant().id(),
            GrantIdV1::derive("stage3-ordinary-grant").unwrap()
        );
    }

    #[test]
    fn successor_snapshot_preserves_multi_hop_ancestry_for_a_second_repository_admission() {
        let work_id = WorkIdV1::derive("successive-admission-work").unwrap();
        let subject_commitment = hash(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.repository-work-subject.v1").unwrap(),
            bytes(work_id.as_bytes()),
        ]))
        .unwrap();
        let fixture = repository_authority_fixture(
            vec![
                ("CreateDraftWork", subject_commitment),
                ("CancelWork", subject_commitment),
            ],
            AuthorityFixtureModeV1::MultiHop,
        );
        let root = test_root();
        let domain =
            StoreDomainV1::derive(StoreRoleV1::Repository, b"successive-admission").unwrap();
        let mut store = StoreV1::create(&root, domain.clone()).unwrap();
        put_objects_in_reference_order(&mut store, fixture.objects);
        let contract_root = ContractRootIdV1::parse(&render_digest([55; 32])).unwrap();
        let generation = StoreGenerationV1::new(
            domain,
            1,
            None,
            contract_root,
            StoreCompatibilityV1::stage0_successor().unwrap(),
            vec![fixture.authority_root_id],
        )
        .unwrap();
        store.publish_generation(&generation, None).unwrap();
        activate_store(&root);
        let head = store.active_head().unwrap().unwrap();
        let current = WorkRecordV1::create_draft(WorkRecordWriterV1::Work, work_id).unwrap();
        let create = CreateDraftWorkPublicationV1::new(
            RepositoryActionIdentityV1::new(
                ActionRequestIdV1::derive("successive-admission-create-request").unwrap(),
                IdempotencyKeyIdV1::derive("successive-admission-create-key").unwrap(),
            ),
            store_basis(&head, &generation),
            fixture.selection,
            work_id,
        )
        .unwrap();
        RepositoryStoreV1::new(&mut store)
            .create_draft_work(create)
            .unwrap();

        let head = store.active_head().unwrap().unwrap();
        let generation = store.publication_generation(head.id()).unwrap();
        let cancel = CancelWorkPublicationV1::new(
            RepositoryActionIdentityV1::new(
                ActionRequestIdV1::derive("successive-admission-cancel-request").unwrap(),
                IdempotencyKeyIdV1::derive("successive-admission-cancel-key").unwrap(),
            ),
            store_basis(&head, &generation),
            fixture.selection,
            current,
            WorkTransitionReasonV1::new("cancel after preserved ancestry").unwrap(),
        )
        .unwrap();
        let outcome = RepositoryStoreV1::new(&mut store)
            .cancel_work(cancel)
            .unwrap();

        assert_eq!(outcome.head().generation_ordinal(), 3);
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }

    fn store_basis(
        head: &crate::domain::persistence::StoreHeadV1,
        generation: &StoreGenerationV1,
    ) -> RepositoryStoreBasisV1 {
        RepositoryStoreBasisV1::new(
            head.id(),
            generation.id(),
            generation.ordinal(),
            generation.contract_root_id(),
        )
        .unwrap()
    }

    fn put_objects_in_reference_order(store: &mut StoreV1, objects: Vec<StoreObjectV1>) {
        let mut pending = objects;
        let mut inserted = BTreeSet::new();
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
        assert_eq!(
            connection
                .execute(
                    "UPDATE store_state SET state = 'active', state_revision = state_revision + 1 WHERE singleton = 1",
                    [],
                )
                .unwrap(),
            1
        );
    }

    fn test_root() -> std::path::PathBuf {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "maestro-vnext-successive-admission-{}-{nonce}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&path).unwrap();
        fs::canonicalize(path).unwrap()
    }

    #[test]
    fn store_loaded_chain_rejects_cycle_orphan_cross_root_non_attenuation_staleness_and_revocation()
    {
        for mode in [
            AuthorityFixtureModeV1::MultiHopCycle,
            AuthorityFixtureModeV1::MultiHopExpiredAncestor,
            AuthorityFixtureModeV1::MultiHopForeignRoot,
            AuthorityFixtureModeV1::MultiHopNonAttenuated,
            AuthorityFixtureModeV1::MultiHopRevokedAncestor,
            AuthorityFixtureModeV1::MultiHopStaleRoot,
            AuthorityFixtureModeV1::OrphanedGrant,
        ] {
            assert!(matches!(
                resolve_fixture(mode),
                Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier)
                    | Err(RepositoryAuthorityAdmissionErrorV1::AuthorityValidation(_))
            ));
        }
    }

    #[test]
    fn store_loaded_chain_rejects_a_missing_ordinary_ancestor_or_delegation_edge() {
        let parent_id = GrantIdV1::derive("stage3-ordinary-parent-grant").unwrap();
        let fixture = repository_authority_fixture(
            vec![("CancelWork", [41; 32])],
            AuthorityFixtureModeV1::MultiHop,
        );
        let (parent_object_id, parent) = fixture
            .objects
            .iter()
            .filter(|object| {
                object.schema_id() == OrdinaryBoundedGrantV1::store_schema_id().unwrap()
            })
            .find_map(|object| {
                OrdinaryBoundedGrantV1::from_canonical_bytes(&object_value_bytes(object).unwrap())
                    .ok()
                    .filter(|grant| grant.grant().id() == parent_id)
                    .map(|grant| (object.id(), grant))
            })
            .unwrap();
        let parent_delegation_object_id = fixture
            .objects
            .iter()
            .filter(|object| {
                object.schema_id() == OrdinaryGrantDelegationV1::store_schema_id().unwrap()
            })
            .find(|object| {
                OrdinaryGrantDelegationV1::from_canonical_bytes(
                    &object_value_bytes(object).unwrap(),
                    &parent,
                )
                .is_ok()
            })
            .map(StoreObjectV1::id)
            .unwrap();

        for missing_object_id in [parent_object_id, parent_delegation_object_id] {
            let mut missing_ancestor = repository_authority_fixture(
                vec![("CancelWork", [41; 32])],
                AuthorityFixtureModeV1::MultiHop,
            );
            missing_ancestor
                .objects
                .retain(|object| object.id() != missing_object_id);
            assert!(matches!(
                resolve_fixture_objects(&missing_ancestor),
                Err(RepositoryAuthorityAdmissionErrorV1::InvalidAuthorityCarrier)
            ));
        }
    }

    #[test]
    fn store_loaded_chain_rejects_duplicate_grant_and_delegation_carriers() {
        let mut duplicate_grant = repository_authority_fixture(
            vec![("CancelWork", [41; 32])],
            AuthorityFixtureModeV1::MultiHop,
        );
        let parent_id = GrantIdV1::derive("stage3-ordinary-parent-grant").unwrap();
        let parent = duplicate_grant
            .objects
            .iter()
            .filter(|object| {
                object.schema_id() == OrdinaryBoundedGrantV1::store_schema_id().unwrap()
            })
            .find_map(|object| {
                OrdinaryBoundedGrantV1::from_canonical_bytes(&object_value_bytes(object).unwrap())
                    .ok()
                    .filter(|grant| grant.grant().id() == parent_id)
            })
            .unwrap();
        let mut duplicate_definition = parent.grant().definition();
        duplicate_definition.grantee_principal_id =
            super::super::super::PrincipalIdV1::derive("stage3-duplicate-parent-principal")
                .unwrap();
        let duplicate_parent =
            OrdinaryBoundedGrantV1::new(duplicate_definition.validate().unwrap()).unwrap();
        duplicate_grant.objects.push(
            authority_object(
                AuthoritySchemaV1::OrdinaryBoundedGrant,
                duplicate_parent.schema_value().unwrap(),
                vec![],
            )
            .unwrap(),
        );
        assert!(resolve_fixture_objects(&duplicate_grant).is_err());

        let mut duplicate_delegation = repository_authority_fixture(
            vec![("CancelWork", [41; 32])],
            AuthorityFixtureModeV1::MultiHop,
        );
        let terminal_id = duplicate_delegation.selection.terminal_grant_id();
        let terminal = duplicate_delegation
            .objects
            .iter()
            .filter(|object| {
                object.schema_id() == OrdinaryBoundedGrantV1::store_schema_id().unwrap()
            })
            .find_map(|object| {
                OrdinaryBoundedGrantV1::from_canonical_bytes(&object_value_bytes(object).unwrap())
                    .ok()
                    .filter(|grant| grant.grant().id() == terminal_id)
            })
            .unwrap();
        let duplicate_value = duplicate_delegation
            .objects
            .iter()
            .filter(|object| {
                object.schema_id() == OrdinaryGrantDelegationV1::store_schema_id().unwrap()
            })
            .find(|object| {
                OrdinaryGrantDelegationV1::from_canonical_bytes(
                    &object_value_bytes(object).unwrap(),
                    &terminal,
                )
                .is_ok()
            })
            .unwrap()
            .value()
            .clone();
        duplicate_delegation.objects.push(
            authority_object(
                AuthoritySchemaV1::OrdinaryGrantDelegation,
                duplicate_value,
                vec![],
            )
            .unwrap(),
        );
        assert!(resolve_fixture_objects(&duplicate_delegation).is_err());
    }
}
