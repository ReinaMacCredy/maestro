use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::vnext::identity::StoreObjectIdV1;
#[cfg(test)]
use crate::domain::vnext::integration::TrustedHostDiagnosticTestConnectionV1;
use crate::domain::vnext::integration::{
    TrustedHostDiagnosticConnectionPortV1, TrustedHostDiagnosticPresentationPortV1,
};
#[cfg(test)]
use crate::domain::vnext::persistence::ProtectedDiagnosticTestCurrentViewProviderV1;
use crate::domain::vnext::persistence::{
    AtomicGenerationPublicationV1, AtomicPublicationError, GenerationError,
    PreparedPublicationError, StoreCompatibilityV1, StoreGenerationV1, StoreIdempotencyProbeV1,
    StoreIdempotencyV1, StoreObjectError, StoreObjectV1, StorePublicationAllocationV1,
    StorePublicationOutcomeV1, StorePublicationViewV1, StoreRoleV1, StoreStateV1, StoreV1,
};
use crate::domain::vnext::persistence::{
    ProtectedDiagnosticCurrentViewAnchorV1, ProtectedDiagnosticCurrentViewProviderV1,
};
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

mod repository_admission;
mod repository_leaf_authority;

use super::governance_attestation::{GovernanceAttestationV1, PlanningSchedulingPolicyInputV1};
use super::governance_floor::{
    RepositoryGovernanceAuthorityCurrentnessV1, resolve_repository_governance_floor_current_view,
};
use super::protected_diagnostic_envelope::{
    ProtectedContinuityDiagnosticAssemblerModeV1, ProtectedContinuityDiagnosticEnvelopeInputV1,
    ProtectedContinuityDiagnosticPreparedCarrierV1, ProtectedContinuityDiagnosticReadGuardMarkerV1,
    ProtectedContinuityDiagnosticReleasedEnvelopeV1, prepare_current_protected_snapshot,
};
#[cfg(test)]
use super::protected_diagnostic_envelope::{
    protected_diagnostic_envelope_test_observation,
    reset_protected_diagnostic_envelope_test_observation,
};

pub(in crate::domain::vnext::authority) use repository_admission::MaterializationAuthorityAdmissionV1;
#[cfg(test)]
pub(crate) use repository_admission::test_support;
pub(crate) use repository_admission::{
    AdmittedRepositoryActionV1, ContinuedRepositoryActionV1,
    PersistedEvidenceMutationAuthorityExpectationV1, RepositoryActionAdmissionInputV1,
    RepositoryAuthorityAdmissionErrorV1, RepositoryAuthorityArtifactsV1, admit_repository_action,
    admit_repository_authority_candidate, continue_durably_admitted_repository_action_attempt,
    continue_repository_action_attempt, current_authorization_receipt_is_persisted,
    current_repository_authority_time, validate_persisted_evidence_mutation_authority,
    validate_persisted_repository_action_basis,
};
pub use repository_leaf_authority::{
    AbsorbWorkAuthorityV1, AmendContractAuthorityV1, AppendDesignRevisionAuthorityV1,
    BootstrapExecutionAuthorityV1, CancelWorkAuthorityV1,
    ContinuityMaintenanceExecutionAuthorityV1, CreateDraftWorkAuthorityV1, ExecutionAuthorityV1,
    ExecutionProducerV1, GenericExecutionAuthorityV1, PublishInitialContractAuthorityV1,
    RepositoryAuthenticatedHumanV1, RepositoryAuthoritySelectionV1,
    RepositoryDecisionAuthorityCarrierV1, RepositoryDecisionOptionMappingV1,
    RepositoryDecisionPresentationV1, RepositoryLeafAuthorityErrorV1,
    RepositoryPolicyComponentSetV1, RepositoryPolicySnapshotV1, RepositoryPolicyStrengthV1,
    RepositoryPolicyTransitionAuthorityV1, RepositoryPolicyTransitionKindV1,
    RepositoryPolicyTransitionV1, ResolveDecisionAuthorityV1, SubmitStepAuthorityV1,
    SubmitWorkCompletionAuthorityV1,
};
pub use repository_leaf_authority::{
    CoordinationRepositoryActionAuthorityV1, DistributionRepositoryActionAuthorityV1,
    IntakeRepositoryActionAuthorityV1, MemoryRepositoryActionAuthorityV1,
    PersistenceRepositoryActionAuthorityV1, PlanningRepositoryActionAuthorityV1,
    ResearchRepositoryActionAuthorityV1, SearchMaintenanceRepositoryActionAuthorityV1,
};

use super::continuity::{StoreAllocatedContinuityStateTokenV1, StoreAllocationBindingErrorV1};
use super::materialization::{
    AdmittedRepositoryActionBindingV1, AuthorityMaterializationErrorV1,
    AuthorityMaterializationTransactionV1, MaterializationAtomicPublicationPortV1,
    MaterializationStoreViewPortV1, RepositoryActionBindingFactsV1, RepositoryActionBindingKindV1,
    RepositoryActionCommitFactsV1, SchedulingPolicyBindingOwnerV1, SchedulingPolicyDiffClassV1,
    SchedulingPolicyDowngradeMandateFactsV1, SchedulingPolicyMeaningV1,
    VerifiedSchedulingPolicyDowngradeMandateUseV1, policy_commitment,
};
use super::publication::{
    AuthorityPublicationKindV1, AuthorityPublicationOutcomeV1, AuthoritySchemaV1,
    ISSUE_BOOTSTRAP_MANDATE_IDEMPOTENCY_NAMESPACE_V1,
    ISSUE_ROOT_ATTACHED_BOUNDED_GRANT_IDEMPOTENCY_NAMESPACE_V1, IssueBootstrapMandatePublicationV1,
    IssueRootAttachedBoundedGrantPublicationV1,
    REISSUE_ROOT_ATTACHED_GRANT_ONE_TO_ONE_IDEMPOTENCY_NAMESPACE_V1,
    REVOKE_GRANT_IDEMPOTENCY_NAMESPACE_V1, ReissueRootAttachedGrantOneToOnePublicationV1,
    RevokeGrantPublicationV1,
};
use super::{
    AcceptedAuthorityTimeFloorV1, ActionAuthorityBasisKindV1, ActionOutcomeV1, ActionRequestIdV1,
    ActionResultError, ActionResultIdV1, ActionResultV1, AdmittedTransitionGuardV1,
    AuthorityContextIdV1, AuthorityContextKindV1, AuthorityContinuityClassClosureV1,
    AuthorityContinuityClosureError, AuthorityContinuityClosureInputV1,
    AuthorityContinuityClosureV1, AuthorityContinuityError, AuthorityContinuityFacetDispositionV1,
    AuthorityContinuityManifestV1, AuthorityContinuityPostCutConsequenceSetV1,
    AuthorityContinuityPredecessorV1, AuthorityContinuitySemanticCutV1,
    AuthorityContinuityStateError, AuthorityEvaluationErrorV1, AuthorityEvaluatorV1,
    AuthorityPostCutErrorV1, AuthorityTransitionGuardAdmissionInputV1, AuthorizationReceiptV1,
    BootstrapAuthoritySnapshotErrorV1, BootstrapAuthoritySnapshotV1, CapacityUseDispositionV1,
    ClassDispositionV1, ClosureFacetDispositionKindV1, ContinuityCarrierProfileStatusV1,
    ContinuityClosureFacetV1, ContinuityDisclosureV1, ContinuityExactRootV1, ContinuityReferenceV1,
    DelegationAncestryV1, GovernedCapacityKindV1, GovernedCapacityRootV1,
    GrantAdministrationAuthorityV1, GuardAdmissionKindV1, HTimeAcceptanceErrorV1,
    HTimeCarryBasisV1, HTimeContinuationContributionV1, IdempotencyKeyIdV1,
    IssueBootstrapMandateError, LinearizationCoverageWitnessV1, LinearizationFenceCarrierV1,
    MandateIdV1, RepositoryActionLeafV1, RepositoryGovernedCapacitySlotKindV1, RevocationTargetV1,
    StateTokenIdV1, SuccessVisibleAuthorityContinuityStateV1, TransitionGuardOwnerCensusV1,
    TransitionGuardTermFactV1, TrustedTimeV1, issue_bootstrap_mandate, validate_delegation,
    validate_ordinary_authority,
};

pub struct AuthorityFacadeV1<'store> {
    store: &'store mut StoreV1,
}

// TODO(Authority Stage 7/8): Remove this expectation on or after 2026-07-24
// when the first materialization caller handles the typed publication failure.
#[expect(
    dead_code,
    reason = "the owner-private materialization publication error is frozen before its Stage 7/8 consumers integrate"
)]
#[derive(Debug)]
pub(in crate::domain::vnext) enum AuthorityMaterializationPublicationErrorV1<E> {
    Store(crate::domain::vnext::persistence::StoreError),
    Prepare(E),
}

#[derive(Debug, Error)]
pub(crate) enum SchedulingPolicyMaterializationErrorV1 {
    #[error("the Planning scheduling materialization input is not the exact admitted Action 105")]
    InvalidPlanningInput,
    #[error(transparent)]
    Admission(#[from] RepositoryAuthorityAdmissionErrorV1),
    #[error(transparent)]
    Materialization(#[from] AuthorityMaterializationErrorV1),
    #[error(transparent)]
    Generation(#[from] GenerationError),
    #[error(transparent)]
    AtomicPublication(#[from] AtomicPublicationError),
    #[error(transparent)]
    Identity(#[from] crate::domain::vnext::identity::IdentityError),
    #[error(transparent)]
    Store(#[from] crate::domain::vnext::persistence::StoreError),
    #[error(transparent)]
    Governance(#[from] super::governance_attestation::GovernanceAttestationErrorV1),
    #[error(transparent)]
    GovernanceFloor(#[from] super::governance_floor::RepositoryGovernanceFloorErrorV1),
}

struct SchedulingPolicyOwnerPublicationV1 {
    admission_input: RepositoryActionAdmissionInputV1,
    request_object: StoreObjectV1,
    binding_object: StoreObjectV1,
    current_binding_root: Option<StoreObjectIdV1>,
    current_owner_basis_commitment: [u8; 32],
    planning: PlanningSchedulingPolicyInputV1,
}

pub(in crate::domain::vnext) struct SchedulingPolicyPublicationInputV1 {
    request_id: ActionRequestIdV1,
    request_object: StoreObjectV1,
    binding_object: StoreObjectV1,
    current_binding_root: Option<StoreObjectIdV1>,
    planning: PlanningSchedulingPolicyInputV1,
}

impl SchedulingPolicyPublicationInputV1 {
    // TODO(Planning Stage 7): Remove this expectation when Planning constructs
    // the frozen typed scheduling publication input.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "Stage 5 freezes the typed Stage 7 scheduling publication input before Planning integrates"
        )
    )]
    pub(in crate::domain::vnext) fn new(
        request_id: ActionRequestIdV1,
        request_object: StoreObjectV1,
        binding_object: StoreObjectV1,
        current_binding_root: Option<StoreObjectIdV1>,
        planning: PlanningSchedulingPolicyInputV1,
    ) -> Self {
        Self {
            request_id,
            request_object,
            binding_object,
            current_binding_root,
            planning,
        }
    }
}

struct SchedulingPolicyDowngradeMaterializationV1 {
    mandate_id: MandateIdV1,
    human_principal_id: crate::domain::vnext::authority::PrincipalIdV1,
    human_binding_id: crate::domain::vnext::authority::PrincipalBindingIdV1,
    human_session_id: crate::domain::vnext::authority::SessionIdV1,
    mandate_body_object_id: StoreObjectIdV1,
    mandate_carrier_object_id: StoreObjectIdV1,
    mandate_atom_object_id: StoreObjectIdV1,
    mandate_nonce_commitment: [u8; 32],
    valid_from: u64,
    valid_until: u64,
    revocation_revision: u64,
}

struct AuthorityMaterializationPublicationViewV1<'a> {
    publication: &'a AtomicGenerationPublicationV1,
    probe: &'a StoreIdempotencyProbeV1,
}

impl MaterializationAtomicPublicationPortV1 for AuthorityMaterializationPublicationViewV1<'_> {
    fn expected_old(&self) -> Option<crate::domain::vnext::identity::StoreHeadIdV1> {
        self.publication.expected_old()
    }

    fn generation_ordinal(&self) -> u64 {
        self.publication.generation().ordinal()
    }

    fn generation_previous(&self) -> Option<crate::domain::vnext::identity::StoreGenerationIdV1> {
        self.publication.generation().previous()
    }

    fn probe_key_digest(&self) -> [u8; 32] {
        *self.probe.key_digest()
    }

    fn probe_meaning_digest(&self) -> [u8; 32] {
        *self.probe.meaning_digest()
    }

    fn idempotency_key_digest(&self) -> [u8; 32] {
        *self.publication.idempotency().key_digest()
    }

    fn idempotency_meaning_digest(&self) -> [u8; 32] {
        *self.publication.idempotency().meaning_digest()
    }

    fn idempotency_result_object_id(&self) -> StoreObjectIdV1 {
        self.publication.idempotency().result_object_id()
    }

    fn object_ids(&self) -> Vec<StoreObjectIdV1> {
        self.publication
            .objects()
            .iter()
            .map(|object| object.id())
            .collect()
    }
}

pub(crate) struct AuthorityMaterializationPortV1<'tx> {
    view: &'tx StorePublicationViewV1<'tx>,
    transaction: AuthorityMaterializationTransactionV1<'tx>,
}

impl MaterializationStoreViewPortV1 for StorePublicationViewV1<'_> {
    fn active_head_id(
        &self,
    ) -> Result<
        Option<crate::domain::vnext::identity::StoreHeadIdV1>,
        AuthorityMaterializationErrorV1,
    > {
        self.active_head()
            .map(|head| head.map(|head| head.id()))
            .map_err(|_| AuthorityMaterializationErrorV1::StoreCurrentness)
    }

    fn active_generation_id(
        &self,
    ) -> Result<crate::domain::vnext::identity::StoreGenerationIdV1, AuthorityMaterializationErrorV1>
    {
        self.active_generation()
            .map_err(|_| AuthorityMaterializationErrorV1::StoreCurrentness)?
            .map(|generation| generation.id())
            .ok_or(AuthorityMaterializationErrorV1::StoreCurrentness)
    }

    fn active_generation_object_ids(
        &self,
    ) -> Result<Vec<StoreObjectIdV1>, AuthorityMaterializationErrorV1> {
        self.active_generation_objects()
            .map(|objects| objects.into_iter().map(|object| object.id()).collect())
            .map_err(|_| AuthorityMaterializationErrorV1::StoreCurrentness)
    }
}

// TODO(Authority Stage 7/8): Remove this expectation on or after 2026-07-24
// when the first Planning/repository-owner consumer calls these operations.
#[expect(
    dead_code,
    reason = "the owner-private Authority materialization operations are frozen before their Stage 7/8 consumers integrate"
)]
impl<'tx> AuthorityMaterializationPortV1<'tx> {
    pub(in crate::domain::vnext::authority) fn mint_mandate(
        &'tx self,
        facts: SchedulingPolicyDowngradeMandateFactsV1,
    ) -> Result<VerifiedSchedulingPolicyDowngradeMandateUseV1<'tx>, AuthorityMaterializationErrorV1>
    {
        self.transaction.mint_mandate(facts)
    }

    pub(in crate::domain::vnext::authority) fn admit_binding<K: RepositoryActionBindingKindV1>(
        &'tx self,
        input: RepositoryActionAdmissionInputV1,
        facts: RepositoryActionBindingFactsV1,
    ) -> Result<AdmittedRepositoryActionBindingV1<'tx, K>, AuthorityMaterializationErrorV1> {
        let generation = self
            .view
            .active_generation()
            .map_err(|_| AuthorityMaterializationErrorV1::StoreCurrentness)?
            .ok_or(AuthorityMaterializationErrorV1::StoreCurrentness)?;
        let admitted = admit_repository_action(self.view, &generation, input)
            .map_err(|_| AuthorityMaterializationErrorV1::BindingMismatch)?;
        self.transaction.mint_binding::<K>(&admitted, facts)
    }

    pub(in crate::domain::vnext::authority) fn execute_owner_materialization<
        K: RepositoryActionBindingKindV1,
    >(
        &'tx self,
        input: RepositoryActionAdmissionInputV1,
        facts: RepositoryActionBindingFactsV1,
        commit: RepositoryActionCommitFactsV1,
        publication: AtomicGenerationPublicationV1,
    ) -> Result<AtomicGenerationPublicationV1, AuthorityMaterializationErrorV1> {
        let binding = self.admit_binding::<K>(input, facts)?;
        self.transaction
            .consume_binding_without_mandate(binding, commit)?;
        Ok(publication)
    }

    pub(in crate::domain::vnext::authority) fn execute_scheduling_downgrade_materialization(
        &'tx self,
        input: RepositoryActionAdmissionInputV1,
        mandate: SchedulingPolicyDowngradeMandateFactsV1,
        facts: RepositoryActionBindingFactsV1,
        commit: RepositoryActionCommitFactsV1,
        publication: AtomicGenerationPublicationV1,
    ) -> Result<AtomicGenerationPublicationV1, AuthorityMaterializationErrorV1> {
        let mandate_use = self.mint_mandate(mandate)?;
        let binding = self.admit_binding::<SchedulingPolicyBindingOwnerV1>(input, facts)?;
        mandate_use.consume_with_action_binding(binding, commit)?;
        Ok(publication)
    }

    fn execute_scheduling_policy_materialization(
        &'tx self,
        probe: &StoreIdempotencyProbeV1,
        owner: SchedulingPolicyOwnerPublicationV1,
        requires_downgrade_mandate: bool,
    ) -> Result<AtomicGenerationPublicationV1, SchedulingPolicyMaterializationErrorV1> {
        let current_head = self
            .view
            .active_head()?
            .ok_or(SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput)?;
        let current_generation = self
            .view
            .active_generation()?
            .ok_or(SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput)?;
        let active_objects = self.view.active_generation_objects()?;
        if owner
            .current_binding_root
            .is_some_and(|root| !current_generation.roots().contains(&root))
        {
            return Err(SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput);
        }
        let admitted = admit_repository_action(
            self.view,
            &current_generation,
            owner.admission_input.clone(),
        )?;
        let admission = admitted.materialization_admission();
        if admission.action != SchedulingPolicyBindingOwnerV1::ACTION
            || admission.exact_payload_commitment != Some(*owner.binding_object.id().as_bytes())
        {
            return Err(SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput);
        }
        let authority_root = select_current_authority_root(&current_generation, &active_objects)
            .map_err(|_| SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput)?;
        let current_authority = load_current_authority(
            self.view,
            &current_head,
            &current_generation,
            authority_root,
            &active_objects,
        )
        .map_err(|_| SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput)?;
        let authority_currentness =
            repository_governance_currentness(&current_authority, admission)?;
        let governance_view = resolve_repository_governance_floor_current_view(
            self.view,
            &current_head,
            &current_generation,
            &active_objects,
            authority_currentness,
            owner.planning.safety_floor(),
        )?;
        let attestation =
            GovernanceAttestationV1::derive(owner.planning, &governance_view, admission)?;
        let attested = attestation.consume(&governance_view, admission)?;
        let policy = attested.policy();
        let relation = attested.relation();
        if relation.requires_downgrade_mandate() != requires_downgrade_mandate {
            return Err(SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput);
        }
        let downgrade = requires_downgrade_mandate
            .then(|| {
                resolve_scheduling_policy_downgrade_mandate(
                    self.view,
                    &current_head,
                    &current_generation,
                    &active_objects,
                    &owner,
                    policy,
                    admission,
                )
            })
            .transpose()?;
        let produced_objects = vec![owner.binding_object.clone()];
        let artifacts =
            admitted.issue_committed_artifacts(&owner.request_object, &produced_objects)?;

        let mut roots = current_generation.roots().to_vec();
        if let Some(current) = owner.current_binding_root {
            replace_materialization_root(&mut roots, current, owner.binding_object.id())?;
        } else {
            roots.push(owner.binding_object.id());
        }
        if let Some(mandate) = downgrade.as_ref() {
            consume_materialization_root(&mut roots, mandate.mandate_atom_object_id)?;
        }
        replace_materialization_root(
            &mut roots,
            admitted.current_snapshot_id(),
            admitted.successor_snapshot().id(),
        )?;
        if roots.contains(&admitted.current_capacity_root_id()) {
            replace_materialization_root(
                &mut roots,
                admitted.current_capacity_root_id(),
                admitted.successor_capacity_root().id(),
            )?;
        }
        roots.push(artifacts.result_object().id());
        roots.sort_unstable();
        roots.dedup();
        if !governance_view.preserved_by_roots(&roots)
            || roots
                .iter()
                .filter(|root| **root == attested.direct_floor_root())
                .count()
                != 1
        {
            return Err(SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput);
        }
        let generation = StoreGenerationV1::new(
            self.view.domain().clone(),
            admission.successor_store_generation,
            Some(current_generation.id()),
            current_generation.contract_root_id(),
            StoreCompatibilityV1::stage0_successor()?,
            roots,
        )?;
        let idempotency = StoreIdempotencyV1::new(
            probe.namespace(),
            *probe.key_digest(),
            *probe.meaning_digest(),
            artifacts.result_object().id(),
        )?;
        let mut objects = active_objects;
        objects.extend(produced_objects);
        objects.extend([
            owner.request_object,
            admitted.basis_object().clone(),
            admitted.successor_snapshot().clone(),
            admitted.successor_capacity_root().clone(),
            admitted.capacity_debit().clone(),
            artifacts.receipt_object().clone(),
            artifacts.result_object().clone(),
        ]);
        objects.extend(artifacts.leaf_authority_objects().iter().cloned());
        objects.sort_by_key(StoreObjectV1::id);
        objects.dedup_by_key(|object| object.id());
        let publication = AtomicGenerationPublicationV1::new_from_object_superset(
            generation,
            Some(current_head.id()),
            objects,
            idempotency,
        )?;
        let binding_facts =
            derive_scheduling_policy_binding_facts(SchedulingPolicyBindingDerivationV1 {
                admission,
                policy,
                relation,
                governance_attestation_commitment: attested.governance_commitment(),
                governance_current_view_commitment: attested.current_view_commitment(),
                probe,
                publication: &publication,
                receipt_object_id: artifacts.receipt_object().id(),
                downgrade: downgrade.as_ref(),
            })?;
        if let Some(mandate) = downgrade {
            let mandate_facts = derive_scheduling_policy_mandate_facts(binding_facts, mandate);
            let mandate_use = self.transaction.mint_mandate(mandate_facts)?;
            let binding = self
                .transaction
                .mint_binding::<SchedulingPolicyBindingOwnerV1>(&admitted, binding_facts)?;
            mandate_use.consume_with_action_binding(
                binding,
                RepositoryActionCommitFactsV1 {
                    binding: binding_facts,
                },
            )?;
            Ok(publication)
        } else {
            let binding = self
                .transaction
                .mint_binding::<SchedulingPolicyBindingOwnerV1>(&admitted, binding_facts)?;
            self.transaction.consume_binding_without_mandate(
                binding,
                RepositoryActionCommitFactsV1 {
                    binding: binding_facts,
                },
            )?;
            Ok(publication)
        }
    }
}

fn repository_governance_currentness(
    current: &CurrentAuthorityV1,
    admission: MaterializationAuthorityAdmissionV1,
) -> Result<RepositoryGovernanceAuthorityCurrentnessV1, SchedulingPolicyMaterializationErrorV1> {
    let selection = admission
        .selection
        .ok_or(SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput)?;
    let mut operators = [
        (current.facts.actor_binding(), current.facts.actor_session()),
        (
            current.facts.responder_binding(),
            current.facts.responder_session(),
        ),
    ]
    .into_iter()
    .filter(|(binding, session)| {
        binding.id() == selection.actor_binding_id()
            && session.id() == selection.actor_session_id()
            && session.binding_id() == binding.id()
            && binding.principal_id() == admission.principal_id
            && binding.context_id() == admission.authority_context_id
            && session.context_id() == admission.authority_context_id
            && binding.validity().contains(admission.accepted_h_time)
            && session.validity().contains(admission.accepted_h_time)
    });
    let Some((binding, session)) = operators.next() else {
        return Err(SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput);
    };
    if operators.next().is_some() {
        return Err(SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput);
    }
    let trust_root_binding_commitment = materialization_commitment(
        b"maestro.authority.repository-governance-trust-root-binding.v1\0",
        &[
            admission.authority_context_id.as_bytes(),
            binding.id().as_bytes(),
            session.id().as_bytes(),
            &binding.trust_root_revision().to_be_bytes(),
        ],
    );
    Ok(RepositoryGovernanceAuthorityCurrentnessV1 {
        authority_context: *admission.authority_context_id.as_bytes(),
        authority_epoch: admission.authority_epoch,
        trust_root_revision: binding.trust_root_revision(),
        trust_root_binding_commitment,
        authority_state_token: *current.state.state_token().as_bytes(),
        authority_fence: *current.state.carrier_fence().as_bytes(),
        revocation_revision: current.state.authority_epoch(),
        principal: *binding.principal_id().as_bytes(),
        binding: *binding.id().as_bytes(),
        session: *session.id().as_bytes(),
        assurance_revision: binding.assurance_revision(),
        trusted_time: admission.accepted_h_time,
    })
}

fn replace_materialization_root(
    roots: &mut [StoreObjectIdV1],
    current: StoreObjectIdV1,
    successor: StoreObjectIdV1,
) -> Result<(), SchedulingPolicyMaterializationErrorV1> {
    let mut matches = roots
        .iter()
        .enumerate()
        .filter_map(|(index, root)| (*root == current).then_some(index));
    let Some(index) = matches.next() else {
        return Err(SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput);
    };
    if matches.next().is_some() {
        return Err(SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput);
    }
    roots[index] = successor;
    Ok(())
}

fn consume_materialization_root(
    roots: &mut Vec<StoreObjectIdV1>,
    consumed: StoreObjectIdV1,
) -> Result<(), SchedulingPolicyMaterializationErrorV1> {
    let original_len = roots.len();
    roots.retain(|root| *root != consumed);
    if roots.len() + 1 != original_len {
        return Err(SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput);
    }
    Ok(())
}

fn authority_successor_roots_preserving_governance(
    primary_root: StoreObjectIdV1,
    current_generation: &StoreGenerationV1,
    active_objects: &[StoreObjectV1],
) -> Result<Vec<StoreObjectIdV1>, AuthorityPublicationError> {
    let governance_schema = AuthoritySchemaV1::RepositoryGovernanceFloorSnapshot.id()?;
    let mut governance_roots = current_generation
        .roots()
        .iter()
        .filter(|root| {
            active_objects
                .iter()
                .any(|object| object.id() == **root && object.schema_id() == governance_schema)
        })
        .copied()
        .collect::<Vec<_>>();
    if governance_roots.len() > 1 {
        return Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot);
    }
    governance_roots.push(primary_root);
    governance_roots.sort_unstable();
    governance_roots.dedup();
    Ok(governance_roots)
}

fn resolve_scheduling_policy_downgrade_mandate(
    view: &StorePublicationViewV1<'_>,
    current_head: &crate::domain::vnext::persistence::StoreHeadV1,
    current_generation: &StoreGenerationV1,
    active_objects: &[StoreObjectV1],
    owner: &SchedulingPolicyOwnerPublicationV1,
    policy: SchedulingPolicyMeaningV1,
    admission: MaterializationAuthorityAdmissionV1,
) -> Result<SchedulingPolicyDowngradeMaterializationV1, SchedulingPolicyMaterializationErrorV1> {
    let authority_root = select_current_authority_root(current_generation, active_objects)
        .map_err(|_| SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput)?;
    let current = load_current_authority(
        view,
        current_head,
        current_generation,
        authority_root,
        active_objects,
    )
    .map_err(|_| SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput)?;
    let expected_subject = scheduling_policy_mandate_subject(owner.binding_object.id());
    let expected_action_commitment = materialization_commitment(
        b"maestro.authority.scheduling-action-spec.v1\0",
        &[SchedulingPolicyBindingOwnerV1::ACTION.literal().as_bytes()],
    );
    let binding_schema = AuthoritySchemaV1::BootstrapMandateIssuanceBinding
        .id()
        .map_err(SchedulingPolicyMaterializationErrorV1::Identity)?;
    let mandate_schema = AuthoritySchemaV1::AuthorityMandate
        .id()
        .map_err(SchedulingPolicyMaterializationErrorV1::Identity)?;
    let consent_schema = AuthoritySchemaV1::ConsentSlotBindingParameter
        .id()
        .map_err(SchedulingPolicyMaterializationErrorV1::Identity)?;
    let request_schema = owner.request_object.schema_id();
    let mut matches = Vec::new();

    for atom_id in current_generation.roots() {
        let Some(carrier) = active_objects
            .iter()
            .find(|object| object.id() == *atom_id && object.schema_id() == binding_schema)
        else {
            continue;
        };
        let carrier_fields = exact_current_fields(carrier, 5)
            .map_err(|_| SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput)?;
        let carrier_references = direct_reference_objects(carrier, active_objects)
            .map_err(|_| SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput)?;
        let mut mandate_objects = carrier_references
            .iter()
            .filter(|object| object.schema_id() == mandate_schema);
        let Some(mandate_object) = mandate_objects.next() else {
            continue;
        };
        if mandate_objects.next().is_some() {
            return Err(SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput);
        }
        let mandate_fields = exact_current_fields(mandate_object, 14)
            .map_err(|_| SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput)?;
        if !matches!(
            &mandate_fields[2],
            CborValue::Text(action) if action == SchedulingPolicyBindingOwnerV1::ACTION.literal()
        ) {
            continue;
        }
        let mut consent_objects = carrier_references
            .iter()
            .filter(|object| object.schema_id() == consent_schema);
        let Some(consent_object) = consent_objects.next() else {
            return Err(SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput);
        };
        if consent_objects.next().is_some()
            || carrier_references.len() != 3
            || !carrier_references.iter().any(|object| {
                object.id() == owner.request_object.id() && object.schema_id() == request_schema
            })
            || mandate_object.references().len() != 2
            || !mandate_object.references().contains(&consent_object.id())
            || !mandate_object.references().contains(&authority_root)
        {
            return Err(SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput);
        }

        let mandate_bytes = object_value_bytes(mandate_object)
            .map_err(|_| SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput)?;
        let mandate_id = MandateIdV1::from_digest(Sha256::digest(&mandate_bytes).into());
        let context_id = exact_current_digest(&mandate_fields[1])
            .map_err(|_| SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput)?;
        let responder_binding = exact_current_digest(&mandate_fields[6])
            .map_err(|_| SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput)?;
        let mut operators = [
            (current.facts.actor_binding(), current.facts.actor_session()),
            (
                current.facts.responder_binding(),
                current.facts.responder_session(),
            ),
        ]
        .into_iter()
        .filter(|(binding, session)| {
            *binding.id().as_bytes() == responder_binding
                && session.binding_id() == binding.id()
                && binding.context_id() == admission.authority_context_id
                && session.context_id() == admission.authority_context_id
                && binding.human_capable()
                && binding.validity().contains(admission.accepted_h_time)
                && session.validity().contains(admission.accepted_h_time)
        });
        let Some((binding, session)) = operators.next() else {
            return Err(SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput);
        };
        if operators.next().is_some() {
            return Err(SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput);
        }
        let interaction_closure = exact_current_digest(&mandate_fields[8])
            .map_err(|_| SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput)?;
        let authority_basis = exact_current_digest(&mandate_fields[9])
            .map_err(|_| SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput)?;
        let valid_from = exact_current_unsigned(&mandate_fields[10])?;
        let valid_until = exact_current_unsigned(&mandate_fields[11])?;
        let logical_mandate_id = exact_current_digest(&carrier_fields[1])
            .map_err(|_| SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput)?;
        let target_action_commitment = exact_current_digest(&carrier_fields[2])
            .map_err(|_| SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput)?;
        let consent_commitment = exact_current_digest(&carrier_fields[3])
            .map_err(|_| SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput)?;
        let carrier_interaction_closure = exact_current_digest(&carrier_fields[4])
            .map_err(|_| SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput)?;
        let subject = match &mandate_fields[3] {
            CborValue::Text(value) => value,
            _ => return Err(SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput),
        };
        if logical_mandate_id != *mandate_id.as_bytes()
            || context_id != *admission.authority_context_id.as_bytes()
            || subject != &expected_subject
            || exact_current_unsigned(&mandate_fields[4])? != policy.classifier_revision()
            || mandate_fields[5] != *consent_object.value()
            || exact_current_unsigned(&mandate_fields[7])? != binding.assurance_revision()
            || authority_basis != owner.current_owner_basis_commitment
            || valid_from >= valid_until
            || !(valid_from <= admission.accepted_h_time && admission.accepted_h_time < valid_until)
            || exact_current_unsigned(&mandate_fields[12])? != 1
            || exact_current_unsigned(&mandate_fields[13])? != 0
            || target_action_commitment != expected_action_commitment
            || consent_commitment
                != digest_value(consent_object.value())
                    .map_err(|_| SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput)?
            || carrier_interaction_closure != interaction_closure
            || current
                .facts
                .revocations()
                .revocations()
                .contains(RevocationTargetV1::Mandate(mandate_id))
        {
            return Err(SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput);
        }
        matches.push(SchedulingPolicyDowngradeMaterializationV1 {
            mandate_id,
            human_principal_id: binding.principal_id(),
            human_binding_id: binding.id(),
            human_session_id: session.id(),
            mandate_body_object_id: mandate_object.id(),
            mandate_carrier_object_id: carrier.id(),
            mandate_atom_object_id: carrier.id(),
            mandate_nonce_commitment: materialization_commitment(
                b"maestro.authority.scheduling-mandate-use.v1\0",
                &[
                    interaction_closure.as_slice(),
                    binding.principal_id().as_bytes(),
                    binding.id().as_bytes(),
                    session.id().as_bytes(),
                    admission.request_id.as_bytes(),
                    owner.request_object.id().as_bytes(),
                    owner.binding_object.id().as_bytes(),
                ],
            ),
            valid_from,
            valid_until,
            revocation_revision: current.facts.snapshot().authority_epoch,
        });
    }
    if matches.len() != 1 {
        return Err(SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput);
    }
    Ok(matches
        .pop()
        .expect("invariant: exact one applicable scheduling Mandate"))
}

fn exact_current_unsigned(
    value: &CborValue,
) -> Result<u64, SchedulingPolicyMaterializationErrorV1> {
    match value {
        CborValue::Unsigned(value) => Ok(*value),
        _ => Err(SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput),
    }
}

fn scheduling_policy_mandate_subject(binding_id: StoreObjectIdV1) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut subject = String::with_capacity(71);
    subject.push_str("sha256:");
    for byte in binding_id.as_bytes() {
        subject.push(HEX[(byte >> 4) as usize] as char);
        subject.push(HEX[(byte & 0x0f) as usize] as char);
    }
    subject
}

struct SchedulingPolicyBindingDerivationV1<'a> {
    admission: MaterializationAuthorityAdmissionV1,
    policy: SchedulingPolicyMeaningV1,
    relation: SchedulingPolicyDiffClassV1,
    governance_attestation_commitment: [u8; 32],
    governance_current_view_commitment: [u8; 32],
    probe: &'a StoreIdempotencyProbeV1,
    publication: &'a AtomicGenerationPublicationV1,
    receipt_object_id: StoreObjectIdV1,
    downgrade: Option<&'a SchedulingPolicyDowngradeMaterializationV1>,
}

fn derive_scheduling_policy_binding_facts(
    derivation: SchedulingPolicyBindingDerivationV1<'_>,
) -> Result<RepositoryActionBindingFactsV1, SchedulingPolicyMaterializationErrorV1> {
    let SchedulingPolicyBindingDerivationV1 {
        admission,
        policy,
        relation,
        governance_attestation_commitment,
        governance_current_view_commitment,
        probe,
        publication,
        receipt_object_id,
        downgrade,
    } = derivation;
    let selection = admission
        .selection
        .ok_or(SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput)?;
    let exact_payload_commitment = admission
        .exact_payload_commitment
        .ok_or(SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput)?;
    let active_generation_id = publication
        .generation()
        .previous()
        .ok_or(SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput)?;
    let expected_old = publication
        .expected_old()
        .ok_or(SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput)?;
    let object_ids = publication
        .objects()
        .iter()
        .map(StoreObjectV1::id)
        .collect::<Vec<_>>();
    let mut write_set = Sha256::new();
    write_set.update(b"maestro.authority.materialization-write-set.v1\0");
    for object_id in &object_ids {
        write_set.update(object_id.as_bytes());
    }
    let write_set_commitment = write_set.finalize().into();
    let owner_publication_commitment = materialization_commitment(
        b"maestro.authority.scheduling-owner-publication.v1\0",
        &[
            publication.generation().id().as_bytes(),
            publication.idempotency().result_object_id().as_bytes(),
            exact_payload_commitment.as_slice(),
        ],
    );
    let currentness_commitment = materialization_commitment(
        b"maestro.authority.scheduling-currentness.v1\0",
        &[
            expected_old.as_bytes(),
            active_generation_id.as_bytes(),
            admission.current_snapshot_id.as_bytes(),
            admission.state_object_id.as_bytes(),
            &governance_attestation_commitment,
            &governance_current_view_commitment,
        ],
    );
    let authority_fence_commitment = materialization_commitment(
        b"maestro.authority.scheduling-fence.v1\0",
        &[
            expected_old.as_bytes(),
            active_generation_id.as_bytes(),
            admission.guard_object_id.as_bytes(),
        ],
    );
    let normalized_witness_commitment = materialization_commitment(
        b"maestro.authority.scheduling-normalized-witness.v1\0",
        &[
            admission.current_snapshot_id.as_bytes(),
            admission.current_capacity_root_id.as_bytes(),
            admission.guard_object_id.as_bytes(),
            admission.state_object_id.as_bytes(),
        ],
    );
    let root_use_atoms_commitment = materialization_commitment(
        b"maestro.authority.scheduling-root-use-atoms.v1\0",
        &[
            admission.current_capacity_root_id.as_bytes(),
            admission.successor_capacity_root_id.as_bytes(),
            admission.capacity_debit_id.as_bytes(),
        ],
    );
    let invocation_commitment = materialization_invocation_commitment(
        admission.request_id,
        active_generation_id,
        exact_payload_commitment,
    );
    let current_policy_commitment = policy_commitment(
        b"maestro.authority.scheduling-current-policy.v1\0",
        &policy.current_rules(),
    );
    let candidate_policy_commitment = policy_commitment(
        b"maestro.authority.scheduling-candidate-policy.v1\0",
        &policy.candidate_rules(),
    );
    let complete_diff_commitment = policy_commitment(
        b"maestro.authority.scheduling-complete-diff.v1\0",
        &[
            policy.current_rules().as_slice(),
            policy.candidate_rules().as_slice(),
        ]
        .concat(),
    );
    let no_mandate = [0xA5; 32];
    let (
        supplemental_mandate_id,
        supplemental_mandate_schema_version,
        supplemental_mandate_valid_from,
        supplemental_mandate_valid_until,
        supplemental_mandate_revocation_revision,
        supplemental_mandate_atom,
        supplemental_mandate_body_commitment,
        supplemental_mandate_carrier_commitment,
        supplemental_mandate_nonce_commitment,
    ) = downgrade.map_or(
        (
            None, 0, 0, 0, 0, no_mandate, no_mandate, no_mandate, no_mandate,
        ),
        |mandate| {
            (
                Some(mandate.mandate_id),
                1,
                mandate.valid_from,
                mandate.valid_until,
                mandate.revocation_revision,
                *mandate.mandate_atom_object_id.as_bytes(),
                *mandate.mandate_body_object_id.as_bytes(),
                *mandate.mandate_carrier_object_id.as_bytes(),
                mandate.mandate_nonce_commitment,
            )
        },
    );
    let planned_consumption_id = admission
        .leaf_authority_consumption_id
        .or(admission.leaf_authority_carrier_id)
        .unwrap_or(admission.capacity_debit_id);
    let idempotency_mapping_commitment = materialization_commitment(
        b"maestro.authority.scheduling-idempotency-mapping.v1\0",
        &[
            probe.key_digest(),
            probe.meaning_digest(),
            publication.idempotency().result_object_id().as_bytes(),
        ],
    );
    Ok(RepositoryActionBindingFactsV1 {
        policy_meaning: policy,
        authority_selection: selection,
        request_id: admission.request_id,
        action: admission.action,
        principal_id: admission.principal_id,
        binding_id: selection.actor_binding_id(),
        session_id: selection.actor_session_id(),
        repository_generation_id: active_generation_id,
        authority_context_id: admission.authority_context_id,
        state_token_id: admission.state_token,
        authority_epoch: admission.authority_epoch,
        trusted_time: admission.accepted_h_time,
        subject_commitment: admission.subject_commitment,
        subject_basis_commitment: admission.subject_basis_commitment,
        exact_payload_commitment,
        action_spec_commitment: materialization_commitment(
            b"maestro.authority.scheduling-action-spec.v1\0",
            &[admission.action.literal().as_bytes()],
        ),
        repository_commitment: materialization_commitment(
            b"maestro.authority.scheduling-repository.v1\0",
            &[publication.generation().domain().id().as_bytes()],
        ),
        store_instance_commitment: materialization_commitment(
            b"maestro.authority.scheduling-store-instance.v1\0",
            &[
                publication.generation().domain().id().as_bytes(),
                active_generation_id.as_bytes(),
            ],
        ),
        head_commitment: materialization_commitment(
            b"maestro.authority.scheduling-head.v1\0",
            &[expected_old.as_bytes()],
        ),
        expected_old_owner_state_commitment: admission.subject_basis_commitment,
        participant_set_commitment: materialization_commitment(
            b"maestro.authority.scheduling-participants.v1\0",
            &[b"Planning", b"Authority", b"Persistence"],
        ),
        owner_publication_commitment,
        write_set_commitment,
        output_commitment: materialization_commitment(
            b"maestro.authority.scheduling-output.v1\0",
            &[publication.idempotency().result_object_id().as_bytes()],
        ),
        authority_basis_commitment: *admission.basis_object_id.as_bytes(),
        authority_snapshot_commitment: *admission.current_snapshot_id.as_bytes(),
        authority_fence_commitment,
        currentness_commitment,
        revocation_commitment: materialization_commitment(
            b"maestro.authority.scheduling-revocation.v1\0",
            &[
                admission.current_snapshot_id.as_bytes(),
                admission.receipt_id.as_bytes(),
            ],
        ),
        normalized_witness_commitment,
        debit_map_commitment: *admission.capacity_debit_id.as_bytes(),
        root_use_atoms_commitment,
        supplemental_mandate_atom,
        planned_debit_commitment: *admission.capacity_debit_id.as_bytes(),
        planned_consumption_commitment: *planned_consumption_id.as_bytes(),
        idempotency_key: IdempotencyKeyIdV1::from_digest(*probe.key_digest()),
        idempotency_mapping_commitment,
        successor_capacity_commitment: *admission.successor_capacity_root_id.as_bytes(),
        receipt_id: admission.receipt_id,
        authorization_receipt_object_commitment: *receipt_object_id.as_bytes(),
        basis_object_commitment: *admission.basis_object_id.as_bytes(),
        current_snapshot_object_commitment: *admission.current_snapshot_id.as_bytes(),
        successor_snapshot_object_commitment: *admission.successor_snapshot_id.as_bytes(),
        current_capacity_root_object_commitment: *admission.current_capacity_root_id.as_bytes(),
        successor_capacity_root_object_commitment: *admission.successor_capacity_root_id.as_bytes(),
        capacity_debit_object_commitment: *admission.capacity_debit_id.as_bytes(),
        leaf_authority_carrier_object_commitment: admission
            .leaf_authority_carrier_id
            .map(|id| *id.as_bytes()),
        leaf_authority_consumption_object_commitment: admission
            .leaf_authority_consumption_id
            .map(|id| *id.as_bytes()),
        guard_object_commitment: *admission.guard_object_id.as_bytes(),
        state_object_commitment: *admission.state_object_id.as_bytes(),
        result_commitment: *publication.idempotency().result_object_id().as_bytes(),
        invocation_commitment,
        supplemental_mandate_body_commitment,
        supplemental_mandate_carrier_commitment,
        supplemental_mandate_nonce_commitment,
        current_policy_commitment,
        candidate_policy_commitment,
        evaluator_commitment: policy_commitment(
            b"maestro.authority.scheduling-evaluator.v1\0",
            &[policy.evaluator_revision()],
        ),
        complete_diff_commitment,
        classifier_commitment: policy_commitment(
            b"maestro.authority.scheduling-classifier.v1\0",
            &[policy.classifier_revision()],
        ),
        classifier_revision_commitment: policy_commitment(
            b"maestro.authority.scheduling-classifier-revision.v1\0",
            &[policy.classifier_revision()],
        ),
        safety_floor_commitment: policy_commitment(
            b"maestro.authority.scheduling-safety-floor.v1\0",
            &policy.safety_floor(),
        ),
        governance_floor_commitment: policy.governance_floor_binding(),
        request_payload_commitment: exact_payload_commitment,
        idempotency_meaning_commitment: *probe.meaning_digest(),
        trust_root_commitment: materialization_commitment(
            b"maestro.authority.scheduling-trust-root.v1\0",
            &[
                admission.current_snapshot_id.as_bytes(),
                admission.state_object_id.as_bytes(),
            ],
        ),
        supplemental_mandate_id,
        supplemental_mandate_schema_version,
        supplemental_mandate_valid_from,
        supplemental_mandate_valid_until,
        supplemental_mandate_revocation_revision,
        supplemental_mandate_diff_class: relation,
    })
}

fn derive_scheduling_policy_mandate_facts(
    binding: RepositoryActionBindingFactsV1,
    mandate: SchedulingPolicyDowngradeMaterializationV1,
) -> SchedulingPolicyDowngradeMandateFactsV1 {
    SchedulingPolicyDowngradeMandateFactsV1 {
        policy_meaning: binding.policy_meaning,
        mandate_id: mandate.mandate_id,
        action_request_id: binding.request_id,
        idempotency_key: binding.idempotency_key,
        repository_generation_id: binding.repository_generation_id,
        principal_id: mandate.human_principal_id,
        human_binding_id: mandate.human_binding_id,
        human_session_id: mandate.human_session_id,
        authority_context_id: binding.authority_context_id,
        state_token_id: binding.state_token_id,
        mandate_schema_version: binding.supplemental_mandate_schema_version,
        authority_epoch: binding.authority_epoch,
        valid_from: mandate.valid_from,
        valid_until: mandate.valid_until,
        trusted_time: binding.trusted_time,
        revocation_revision: mandate.revocation_revision,
        diff_class: binding.supplemental_mandate_diff_class,
        mandate_body_commitment: binding.supplemental_mandate_body_commitment,
        mandate_carrier_commitment: binding.supplemental_mandate_carrier_commitment,
        mandate_nonce_commitment: binding.supplemental_mandate_nonce_commitment,
        repository_commitment: binding.repository_commitment,
        store_instance_commitment: binding.store_instance_commitment,
        head_commitment: binding.head_commitment,
        expected_old_binding_commitment: binding.expected_old_owner_state_commitment,
        current_policy_commitment: binding.current_policy_commitment,
        candidate_policy_commitment: binding.candidate_policy_commitment,
        evaluator_commitment: binding.evaluator_commitment,
        complete_diff_commitment: binding.complete_diff_commitment,
        classifier_commitment: binding.classifier_commitment,
        classifier_revision_commitment: binding.classifier_revision_commitment,
        safety_floor_commitment: binding.safety_floor_commitment,
        governance_floor_commitment: binding.governance_floor_commitment,
        request_payload_commitment: binding.request_payload_commitment,
        idempotency_meaning_commitment: binding.idempotency_meaning_commitment,
        idempotency_mapping_commitment: binding.idempotency_mapping_commitment,
        action_spec_commitment: binding.action_spec_commitment,
        subject_commitment: binding.subject_commitment,
        subject_basis_commitment: binding.subject_basis_commitment,
        exact_payload_commitment: binding.exact_payload_commitment,
        participant_set_commitment: binding.participant_set_commitment,
        owner_publication_commitment: binding.owner_publication_commitment,
        write_set_commitment: binding.write_set_commitment,
        output_commitment: binding.output_commitment,
        planned_debit_commitment: binding.planned_debit_commitment,
        planned_consumption_commitment: binding.planned_consumption_commitment,
        result_commitment: binding.result_commitment,
        invocation_commitment: binding.invocation_commitment,
        authority_snapshot_commitment: binding.authority_snapshot_commitment,
        authority_fence_commitment: binding.authority_fence_commitment,
        authority_basis_commitment: binding.authority_basis_commitment,
        currentness_commitment: binding.currentness_commitment,
        revocation_commitment: binding.revocation_commitment,
        authorization_receipt_id: binding.receipt_id,
        trust_root_commitment: binding.trust_root_commitment,
        normalized_witness_commitment: binding.normalized_witness_commitment,
        debit_map_commitment: binding.debit_map_commitment,
        root_use_atoms_commitment: binding.root_use_atoms_commitment,
        mandate_atom_commitment: binding.supplemental_mandate_atom,
        successor_capacity_commitment: binding.successor_capacity_commitment,
    }
}

fn materialization_invocation_commitment(
    request_id: ActionRequestIdV1,
    generation_id: crate::domain::vnext::identity::StoreGenerationIdV1,
    payload: [u8; 32],
) -> [u8; 32] {
    let entropy = protected_diagnostic_random_entropy(
        b"maestro.authority.scheduling-materialization-entropy.v1\0",
    );
    materialization_commitment(
        b"maestro.authority.scheduling-materialization-invocation.v1\0",
        &[
            &entropy,
            request_id.as_bytes(),
            generation_id.as_bytes(),
            &payload,
        ],
    )
}

fn materialization_commitment(domain: &[u8], fields: &[&[u8]]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update((domain.len() as u64).to_be_bytes());
    digest.update(domain);
    for field in fields {
        digest.update((field.len() as u64).to_be_bytes());
        digest.update(field);
    }
    digest.finalize().into()
}

impl<'store> AuthorityFacadeV1<'store> {
    pub fn new(store: &'store mut StoreV1) -> Self {
        let _ = Self::publish_scheduling_policy_without_downgrade;
        let _ = Self::publish_scheduling_policy_with_downgrade;
        Self { store }
    }

    pub(in crate::domain::vnext::authority) fn publish_repository_materialization<E>(
        &mut self,
        probe: &StoreIdempotencyProbeV1,
        prepare: impl for<'tx> FnOnce(
            &'tx AuthorityMaterializationPortV1<'tx>,
        ) -> Result<AtomicGenerationPublicationV1, E>,
    ) -> Result<StorePublicationOutcomeV1, AuthorityMaterializationPublicationErrorV1<E>>
    where
        E: From<AuthorityMaterializationErrorV1>,
    {
        self.store
            .publish_generation_atomically_with_prepare(probe, |view| {
                let port = AuthorityMaterializationPortV1 {
                    view,
                    transaction: AuthorityMaterializationTransactionV1::from_live_store_transaction(
                        view,
                    ),
                };
                let publication = prepare(&port)?;
                if !port.transaction.is_consumed() {
                    return Err(E::from(
                        AuthorityMaterializationErrorV1::IncompleteTransaction,
                    ));
                }
                port.transaction
                    .validate_atomic_publication(&AuthorityMaterializationPublicationViewV1 {
                        publication: &publication,
                        probe,
                    })
                    .map_err(E::from)?;
                Ok(publication)
            })
            .map_err(|error| match error {
                PreparedPublicationError::Store(error) => {
                    AuthorityMaterializationPublicationErrorV1::Store(error)
                }
                PreparedPublicationError::Prepare(error) => {
                    AuthorityMaterializationPublicationErrorV1::Prepare(error)
                }
            })
    }

    pub(in crate::domain::vnext) fn publish_scheduling_policy_without_downgrade(
        &mut self,
        probe: &StoreIdempotencyProbeV1,
        authority: PlanningRepositoryActionAuthorityV1,
        input: SchedulingPolicyPublicationInputV1,
    ) -> Result<
        StorePublicationOutcomeV1,
        AuthorityMaterializationPublicationErrorV1<SchedulingPolicyMaterializationErrorV1>,
    > {
        let SchedulingPolicyPublicationInputV1 {
            request_id,
            request_object,
            binding_object,
            current_binding_root,
            planning,
        } = input;
        let current_owner_basis_commitment = authority.current_semantic_owner_basis_commitment();
        if authority.action()
            != match SchedulingPolicyBindingOwnerV1::ACTION {
                RepositoryActionLeafV1::Downstream(action) => action,
                _ => unreachable!("Scheduling owner is one exact downstream Action"),
            }
            || authority.exact_payload_commitment() != *binding_object.id().as_bytes()
            || planning.request() != *request_id.as_bytes()
            || planning.payload() != *binding_object.id().as_bytes()
            || planning.candidate_binding() != *binding_object.id().as_bytes()
            || planning.expected_binding()
                != current_binding_root.map_or([0xA5; 32], |root| *root.as_bytes())
            || planning.idempotency_key() != *probe.key_digest()
            || planning.idempotency_meaning() != *probe.meaning_digest()
        {
            return Err(AuthorityMaterializationPublicationErrorV1::Prepare(
                SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput,
            ));
        }
        self.publish_scheduling_policy_materialization(
            probe,
            SchedulingPolicyOwnerPublicationV1 {
                admission_input: RepositoryActionAdmissionInputV1::new(request_id, authority),
                request_object,
                binding_object,
                current_binding_root,
                current_owner_basis_commitment,
                planning,
            },
            false,
        )
    }

    pub(in crate::domain::vnext) fn publish_scheduling_policy_with_downgrade(
        &mut self,
        probe: &StoreIdempotencyProbeV1,
        authority: PlanningRepositoryActionAuthorityV1,
        input: SchedulingPolicyPublicationInputV1,
    ) -> Result<
        StorePublicationOutcomeV1,
        AuthorityMaterializationPublicationErrorV1<SchedulingPolicyMaterializationErrorV1>,
    > {
        let SchedulingPolicyPublicationInputV1 {
            request_id,
            request_object,
            binding_object,
            current_binding_root,
            planning,
        } = input;
        let current_owner_basis_commitment = authority.current_semantic_owner_basis_commitment();
        if authority.action()
            != match SchedulingPolicyBindingOwnerV1::ACTION {
                RepositoryActionLeafV1::Downstream(action) => action,
                _ => unreachable!("Scheduling owner is one exact downstream Action"),
            }
            || authority.exact_payload_commitment() != *binding_object.id().as_bytes()
            || planning.request() != *request_id.as_bytes()
            || planning.payload() != *binding_object.id().as_bytes()
            || planning.candidate_binding() != *binding_object.id().as_bytes()
            || planning.expected_binding()
                != current_binding_root.map_or([0xA5; 32], |root| *root.as_bytes())
            || planning.idempotency_key() != *probe.key_digest()
            || planning.idempotency_meaning() != *probe.meaning_digest()
        {
            return Err(AuthorityMaterializationPublicationErrorV1::Prepare(
                SchedulingPolicyMaterializationErrorV1::InvalidPlanningInput,
            ));
        }
        self.publish_scheduling_policy_materialization(
            probe,
            SchedulingPolicyOwnerPublicationV1 {
                admission_input: RepositoryActionAdmissionInputV1::new(request_id, authority),
                request_object,
                binding_object,
                current_binding_root,
                current_owner_basis_commitment,
                planning,
            },
            true,
        )
    }

    fn publish_scheduling_policy_materialization(
        &mut self,
        probe: &StoreIdempotencyProbeV1,
        owner: SchedulingPolicyOwnerPublicationV1,
        requires_downgrade_mandate: bool,
    ) -> Result<
        StorePublicationOutcomeV1,
        AuthorityMaterializationPublicationErrorV1<SchedulingPolicyMaterializationErrorV1>,
    > {
        self.publish_repository_materialization(probe, move |port| {
            port.execute_scheduling_policy_materialization(probe, owner, requires_downgrade_mandate)
        })
    }

    pub(crate) fn protected_continuity_diagnostic_with_ports(
        &mut self,
        connection: &mut dyn TrustedHostDiagnosticConnectionPortV1,
        current_view_provider: &mut dyn ProtectedDiagnosticCurrentViewProviderV1,
        requested_subject: ContinuityReferenceV1,
    ) -> Result<ProtectedContinuityDiagnosticReleasedEnvelopeV1, AuthorityPublicationError> {
        self.protected_continuity_diagnostic_with_mode(
            connection,
            current_view_provider,
            requested_subject,
            ProtectedContinuityDiagnosticAssemblerModeV1::Canonical,
        )
    }

    fn protected_continuity_diagnostic_with_mode(
        &mut self,
        connection: &mut dyn TrustedHostDiagnosticConnectionPortV1,
        current_view_provider: &mut dyn ProtectedDiagnosticCurrentViewProviderV1,
        requested_subject: ContinuityReferenceV1,
        assembler_mode: ProtectedContinuityDiagnosticAssemblerModeV1,
    ) -> Result<ProtectedContinuityDiagnosticReleasedEnvelopeV1, AuthorityPublicationError> {
        let invocation_issuer = ProtectedDiagnosticInvocationIssuerV1::fresh()?;
        let outcome = self.store.with_serialized_active_view(move |view| {
            build_protected_continuity_diagnostic(
                view,
                connection,
                current_view_provider,
                requested_subject,
                &invocation_issuer,
                assembler_mode,
            )
        });
        match outcome {
            Ok(value) => Ok(value),
            Err(PreparedPublicationError::Store(_)) | Err(PreparedPublicationError::Prepare(_)) => {
                Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot)
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn protected_continuity_diagnostic_reference_envelope(
        &mut self,
        connection: &mut TrustedHostDiagnosticTestConnectionV1,
        current_view_provider: &mut ProtectedDiagnosticTestCurrentViewProviderV1,
        requested_subject: ContinuityReferenceV1,
    ) -> Result<ProtectedContinuityDiagnosticReferenceEnvelopeV1, AuthorityPublicationError> {
        let released = self.protected_continuity_diagnostic_with_ports(
            connection,
            current_view_provider,
            requested_subject,
        )?;
        if released.into_bytes().is_empty() {
            return Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot);
        }
        Ok(ProtectedContinuityDiagnosticReferenceEnvelopeV1 {
            disposition: ProtectedContinuityDiagnosticDispositionV1::CurrentProtectedSnapshot,
        })
    }

    #[cfg(test)]
    fn protected_continuity_diagnostic_reference_envelope_with_assembler_mutation(
        &mut self,
        connection: &mut TrustedHostDiagnosticTestConnectionV1,
        current_view_provider: &mut ProtectedDiagnosticTestCurrentViewProviderV1,
        requested_subject: ContinuityReferenceV1,
        assembler_mode: ProtectedContinuityDiagnosticAssemblerModeV1,
    ) -> Result<ProtectedContinuityDiagnosticReferenceEnvelopeV1, AuthorityPublicationError> {
        let released = self.protected_continuity_diagnostic_with_mode(
            connection,
            current_view_provider,
            requested_subject,
            assembler_mode,
        )?;
        if released.into_bytes().is_empty() {
            return Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot);
        }
        Ok(ProtectedContinuityDiagnosticReferenceEnvelopeV1 {
            disposition: ProtectedContinuityDiagnosticDispositionV1::CurrentProtectedSnapshot,
        })
    }

    pub fn issue_bootstrap_mandate(
        &mut self,
        plan: IssueBootstrapMandatePublicationV1,
    ) -> Result<AuthorityPublicationOutcomeV1, AuthorityPublicationError> {
        if self.store.state()?.0 != StoreStateV1::Active {
            return Err(AuthorityPublicationError::InactiveStore);
        }
        let prepared = PreparedBootstrapRequestV1::new(&plan)?;
        let probe = StoreIdempotencyProbeV1::new(
            ISSUE_BOOTSTRAP_MANDATE_IDEMPOTENCY_NAMESPACE_V1,
            *plan.request.idempotency_key().as_bytes(),
            prepared.meaning_digest,
        )?;
        let outcome = self
            .store
            .publish_generation_atomically_with_prepare(&probe, |view| {
                prepare_bootstrap_publication(view, plan, prepared)
            });
        match outcome {
            Ok(outcome) => publication_outcome(self.store, outcome),
            Err(PreparedPublicationError::Store(error)) => Err(error.into()),
            Err(PreparedPublicationError::Prepare(error)) => Err(error),
        }
    }

    pub fn issue_root_attached_bounded_grant(
        &mut self,
        plan: IssueRootAttachedBoundedGrantPublicationV1,
    ) -> Result<AuthorityPublicationOutcomeV1, AuthorityPublicationError> {
        if self.store.state()?.0 != StoreStateV1::Active {
            return Err(AuthorityPublicationError::InactiveStore);
        }
        let meaning_digest = grant_issue_meaning_digest(&plan)?;
        let probe = StoreIdempotencyProbeV1::new(
            ISSUE_ROOT_ATTACHED_BOUNDED_GRANT_IDEMPOTENCY_NAMESPACE_V1,
            *plan.identity.idempotency_key().as_bytes(),
            meaning_digest,
        )?;
        let outcome = self
            .store
            .publish_generation_atomically_with_prepare(&probe, |view| {
                prepare_root_attached_grant_issue(view, plan, meaning_digest)
            });
        match outcome {
            Ok(outcome) => grant_publication_outcome(outcome),
            Err(PreparedPublicationError::Store(error)) => Err(error.into()),
            Err(PreparedPublicationError::Prepare(error)) => Err(error),
        }
    }

    pub fn reissue_root_attached_grant_one_to_one(
        &mut self,
        plan: ReissueRootAttachedGrantOneToOnePublicationV1,
    ) -> Result<AuthorityPublicationOutcomeV1, AuthorityPublicationError> {
        self.publish_ordinary_grant_mutation(OrdinaryGrantMutationV1::Reissue(Box::new(plan)))
    }

    pub fn revoke_grant(
        &mut self,
        plan: RevokeGrantPublicationV1,
    ) -> Result<AuthorityPublicationOutcomeV1, AuthorityPublicationError> {
        self.publish_ordinary_grant_mutation(OrdinaryGrantMutationV1::Revoke(Box::new(plan)))
    }

    fn publish_ordinary_grant_mutation(
        &mut self,
        mutation: OrdinaryGrantMutationV1,
    ) -> Result<AuthorityPublicationOutcomeV1, AuthorityPublicationError> {
        if self.store.state()?.0 != StoreStateV1::Active {
            return Err(AuthorityPublicationError::InactiveStore);
        }
        let meaning_digest = mutation.meaning_digest()?;
        let probe = StoreIdempotencyProbeV1::new(
            mutation.idempotency_namespace(),
            *mutation.idempotency_key().as_bytes(),
            meaning_digest,
        )?;
        let outcome = self
            .store
            .publish_generation_atomically_with_prepare(&probe, |view| {
                prepare_ordinary_grant_mutation(view, mutation, meaning_digest)
            });
        match outcome {
            Ok(outcome) => grant_publication_outcome(outcome),
            Err(PreparedPublicationError::Store(error)) => Err(error.into()),
            Err(PreparedPublicationError::Prepare(error)) => Err(error),
        }
    }
}

fn prepare_bootstrap_publication(
    view: &StorePublicationViewV1<'_>,
    plan: IssueBootstrapMandatePublicationV1,
    prepared: PreparedBootstrapRequestV1,
) -> Result<AtomicGenerationPublicationV1, AuthorityPublicationError> {
    let current_head = view
        .active_head()?
        .ok_or(AuthorityPublicationError::MissingAuthorityPredecessor)?;
    let current_generation = view
        .active_generation()?
        .ok_or(AuthorityPublicationError::MissingAuthorityPredecessor)?;
    if plan.expected_old != current_head.id()
        || plan.previous_generation_id != current_generation.id()
        || plan.contract_root_id != current_generation.contract_root_id()
        || current_generation.roots() != [plan.prior_authority_root]
    {
        return Err(AuthorityPublicationError::AuthorityPredecessorMismatch);
    }
    let active_objects = view.active_generation_objects()?;
    let current = load_current_authority(
        view,
        &current_head,
        &current_generation,
        plan.prior_authority_root,
        &active_objects,
    )?;
    let evaluation =
        AuthorityEvaluatorV1::evaluate_bootstrap_mandate(plan.request.clone(), &current.facts)?;
    let issuance = issue_bootstrap_mandate(evaluation.clone())?;
    let next_generation = current_generation
        .ordinal()
        .checked_add(1)
        .ok_or(AuthorityPublicationError::AuthorityPredecessorMismatch)?;
    let allocation = view.allocate_continuity_state_token(
        next_generation,
        Some(*current.state.state_token().as_bytes()),
        prepared.meaning_digest,
    )?;
    let allocation = bind_store_allocation(current.facts.context().context_id(), allocation)?;
    let continuity = build_successor_continuity(
        &current,
        &active_objects,
        &current_head,
        &current_generation,
        &allocation,
        prepared.meaning_digest,
    )?;
    let resulting_state_token = continuity.state.state_token();
    let successor_facts = current.facts.continue_at_store_generation(
        next_generation,
        current.manifest.id(),
        continuity.state.guard_kind(),
        resulting_state_token,
    )?;

    let context_object = authority_object(
        AuthoritySchemaV1::AuthorityContext,
        current.facts.context().schema_value()?,
        vec![],
    )?;
    let consent_slot_object = authority_object(
        AuthoritySchemaV1::ConsentSlotBindingParameter,
        plan.request.consent_slot().schema_value()?,
        vec![],
    )?;
    let manifest_object = authority_object(
        AuthoritySchemaV1::AuthorityContinuityManifest,
        current.manifest.schema_value()?,
        vec![],
    )?;
    let successor_context_object = authority_object(
        AuthoritySchemaV1::AuthorityContext,
        successor_facts.context().schema_value()?,
        vec![],
    )?;
    let actor_binding_object = authority_object(
        AuthoritySchemaV1::PrincipalBinding,
        successor_facts.actor_binding().schema_value()?,
        vec![],
    )?;
    let responder_binding_object = authority_object(
        AuthoritySchemaV1::PrincipalBinding,
        successor_facts.responder_binding().schema_value()?,
        vec![],
    )?;
    let successor_actor_session_object = authority_object(
        AuthoritySchemaV1::Session,
        successor_facts.actor_session().schema_value()?,
        vec![actor_binding_object.id()],
    )?;
    let successor_responder_session_object = authority_object(
        AuthoritySchemaV1::Session,
        successor_facts.responder_session().schema_value()?,
        vec![responder_binding_object.id()],
    )?;
    let successor_grant_objects = successor_facts
        .g0_candidate_paths()
        .iter()
        .map(|path| {
            authority_object(
                AuthoritySchemaV1::BootstrapGenesisGrant,
                path.genesis_grant().schema_value()?,
                vec![],
            )
        })
        .collect::<Result<Vec<_>, AuthorityPublicationError>>()?;
    let successor_revocations_object = authority_object(
        AuthoritySchemaV1::RevocationSet,
        successor_facts.revocations().schema_value()?,
        vec![],
    )?;
    let successor_interaction_object = authority_object(
        AuthoritySchemaV1::BootstrapMandateInteractionObservationJoin,
        successor_facts
            .interaction_join()
            .ok_or(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot)?
            .schema_value()?,
        vec![successor_responder_session_object.id()],
    )?;
    let mut successor_snapshot_references = vec![
        manifest_object.id(),
        continuity.closure_object.id(),
        continuity.guard_object.id(),
        continuity.state_object.id(),
        successor_context_object.id(),
        actor_binding_object.id(),
        responder_binding_object.id(),
        successor_actor_session_object.id(),
        successor_responder_session_object.id(),
        successor_revocations_object.id(),
        successor_interaction_object.id(),
        consent_slot_object.id(),
    ];
    successor_snapshot_references.extend(successor_grant_objects.iter().map(StoreObjectV1::id));
    let successor_snapshot_object = authority_object(
        AuthoritySchemaV1::BootstrapAuthoritySnapshot,
        successor_facts.schema_value()?,
        successor_snapshot_references,
    )?;
    let basis_object = authority_object(
        AuthoritySchemaV1::ActionAuthorityBasis,
        CborValue::Array(vec![
            CborValue::Unsigned(ActionAuthorityBasisKindV1::BootstrapControlG0 as u64),
            bytes(current.facts.context().context_id().as_bytes()),
            bytes(evaluation.authority_basis_commitment_id().as_bytes()),
        ]),
        vec![],
    )?;
    let request_object = authority_object(
        AuthoritySchemaV1::IssueBootstrapMandateRequest,
        plan.request.schema_value()?,
        vec![
            context_object.id(),
            consent_slot_object.id(),
            basis_object.id(),
        ],
    )?;
    let mandate_object = authority_object(
        AuthoritySchemaV1::AuthorityMandate,
        issuance.mandate.schema_value()?,
        vec![consent_slot_object.id(), basis_object.id()],
    )?;
    let newly_minted = validate_mandate_issuance_cardinality(
        &active_objects,
        &mandate_object,
        &issuance.issuance_binding.canonical_bytes()?,
    )?;

    let binding_object = newly_minted
        .then(|| {
            authority_object(
                AuthoritySchemaV1::BootstrapMandateIssuanceBinding,
                issuance.issuance_binding.schema_value()?,
                vec![
                    mandate_object.id(),
                    request_object.id(),
                    consent_slot_object.id(),
                ],
            )
        })
        .transpose()?;

    let outcome = if newly_minted {
        ActionOutcomeV1::Committed
    } else {
        ActionOutcomeV1::NoOp
    };
    let receipt = AuthorizationReceiptV1::new(
        plan.request.request_id(),
        current.facts.context().context_id(),
        ActionAuthorityBasisKindV1::BootstrapControlG0,
        current.state.state_token(),
        resulting_state_token,
    )?;
    let result = ActionResultV1::new(
        plan.request.request_id(),
        outcome,
        Some(receipt.clone()),
        None,
    )?;
    let receipt_object = authority_object(
        AuthoritySchemaV1::AuthorizationReceipt,
        CborValue::Array(vec![
            bytes(receipt.id().as_bytes()),
            bytes(current.facts.context().context_id().as_bytes()),
            bytes(receipt.request_id().as_bytes()),
            bytes(basis_object.id().as_bytes()),
            CborValue::Unsigned(1),
            CborValue::Bool(true),
            bytes(result.id().as_bytes()),
        ]),
        vec![context_object.id(), request_object.id(), basis_object.id()],
    )?;
    let produced = CborValue::Array(
        std::iter::once(bytes(issuance.mandate.id().as_bytes()))
            .chain(
                binding_object
                    .as_ref()
                    .map(|_| bytes(issuance.issuance_binding.id().as_bytes())),
            )
            .collect(),
    );
    let result_object = authority_object(
        AuthoritySchemaV1::ActionResult,
        CborValue::Array(vec![
            bytes(result.id().as_bytes()),
            bytes(result.request_id().as_bytes()),
            CborValue::Unsigned(result.outcome() as u64),
            CborValue::Unsigned(1),
            CborValue::Array(vec![bytes(current.state.state_token().as_bytes())]),
            CborValue::Array(vec![bytes(resulting_state_token.as_bytes())]),
            CborValue::Array(vec![bytes(receipt.id().as_bytes())]),
            produced,
            CborValue::Array(Vec::new()),
            CborValue::optional(None),
            CborValue::optional(plan.next_or_inspect_ref.map(|value| bytes(&value))),
        ]),
        std::iter::once(request_object.id())
            .chain(std::iter::once(receipt_object.id()))
            .chain(std::iter::once(mandate_object.id()))
            .chain(binding_object.as_ref().map(StoreObjectV1::id))
            .chain(std::iter::once(continuity.closure_object.id()))
            .chain(std::iter::once(continuity.state_object.id()))
            .collect(),
    )?;

    let action_request_commitment = reference_from_bytes(&plan.request.canonical_bytes()?);
    let closure_ref = ContinuityReferenceV1::from_digest(*continuity.closure.id().as_bytes());
    let state_ref = reference_from_object(&continuity.state_object);
    let receipt_ref = ContinuityReferenceV1::from_digest(*receipt.id().as_bytes());
    let result_ref = ContinuityReferenceV1::from_digest(*result.id().as_bytes());
    let phase_mutation_ref = ContinuityReferenceV1::from_digest(*issuance.mandate.id().as_bytes());
    let idempotency_ref = hash_reference(&CborValue::Array(vec![
        CborValue::text(ISSUE_BOOTSTRAP_MANDATE_IDEMPOTENCY_NAMESPACE_V1)?,
        bytes(plan.request.idempotency_key().as_bytes()),
        bytes(&prepared.meaning_digest),
        bytes(result.id().as_bytes()),
    ]))?;
    let context_current_ref = hash_reference(&CborValue::Array(vec![
        CborValue::text("maestro.vnext.context-current-continuity-relation.v1")?,
        bytes(current.facts.context().context_id().as_bytes()),
        bytes(continuity.closure.id().as_bytes()),
        bytes(resulting_state_token.as_bytes()),
    ]))?;
    let witness = LinearizationCoverageWitnessV1::new(
        action_request_commitment,
        LinearizationFenceCarrierV1::SameStoreCommit,
        ContinuityReferenceV1::from_digest(*current_head.id().as_bytes()),
        ContinuityReferenceV1::from_digest(*plan.request.request_id().as_bytes()),
        allocation.allocation_commitment(),
        closure_ref,
        ContinuityReferenceV1::from_digest(prepared.meaning_digest),
        ContinuityReferenceV1::from_digest(*current_generation.id().as_bytes()),
    )?;
    let witness_object = authority_object(
        AuthoritySchemaV1::LinearizationCoverageWitness,
        witness.schema_value()?,
        vec![continuity.closure_object.id()],
    )?;
    let consumption_refs = current
        .facts
        .g0_candidate_paths()
        .iter()
        .map(|path| ContinuityReferenceV1::from_digest(*path.genesis_grant_id().as_bytes()))
        .collect();
    let post_cut = AuthorityContinuityPostCutConsequenceSetV1::new(
        closure_ref,
        continuity.closure.id(),
        resulting_state_token,
        action_request_commitment,
        state_ref,
        consumption_refs,
        phase_mutation_ref,
        receipt_ref,
        result_ref,
        idempotency_ref,
        reference_from_object(&witness_object),
        context_current_ref,
    )?;
    let mut post_cut_references = vec![
        plan.prior_authority_root,
        successor_snapshot_object.id(),
        context_object.id(),
        consent_slot_object.id(),
        basis_object.id(),
        request_object.id(),
        receipt_object.id(),
        result_object.id(),
        mandate_object.id(),
        continuity.closure_object.id(),
        continuity.guard_object.id(),
        continuity.state_object.id(),
        witness_object.id(),
    ];
    post_cut_references.extend(binding_object.as_ref().map(StoreObjectV1::id));
    let post_cut_object = authority_object(
        AuthoritySchemaV1::AuthorityContinuityPostCutConsequenceSet,
        post_cut.schema_value()?,
        post_cut_references,
    )?;
    let generation_roots = authority_successor_roots_preserving_governance(
        post_cut_object.id(),
        &current_generation,
        &active_objects,
    )?;

    let mut objects = active_objects;
    objects.extend([
        context_object,
        consent_slot_object,
        manifest_object,
        successor_context_object,
        actor_binding_object,
        responder_binding_object,
        successor_actor_session_object,
        successor_responder_session_object,
        successor_revocations_object,
        successor_interaction_object,
        successor_snapshot_object,
        basis_object,
        request_object,
        receipt_object,
        result_object.clone(),
        continuity.closure_object,
        continuity.guard_object,
        continuity.state_object,
        witness_object,
        post_cut_object.clone(),
    ]);
    objects.extend(successor_grant_objects);
    if newly_minted {
        objects.push(mandate_object);
        objects.extend(binding_object);
    }
    let generation = StoreGenerationV1::new(
        view.domain().clone(),
        next_generation,
        Some(plan.previous_generation_id),
        plan.contract_root_id,
        StoreCompatibilityV1::stage0_successor()?,
        generation_roots,
    )?;
    let idempotency = StoreIdempotencyV1::new(
        ISSUE_BOOTSTRAP_MANDATE_IDEMPOTENCY_NAMESPACE_V1,
        *plan.request.idempotency_key().as_bytes(),
        prepared.meaning_digest,
        result_object.id(),
    )?;
    objects.sort_by_key(StoreObjectV1::id);
    objects.dedup_by_key(|object| object.id());
    let publication = AtomicGenerationPublicationV1::new_from_object_superset(
        generation,
        Some(plan.expected_old),
        objects,
        idempotency,
    )?;
    Ok(publication)
}

enum OrdinaryGrantMutationV1 {
    Reissue(Box<ReissueRootAttachedGrantOneToOnePublicationV1>),
    Revoke(Box<RevokeGrantPublicationV1>),
}

impl OrdinaryGrantMutationV1 {
    fn idempotency_namespace(&self) -> &'static str {
        match self {
            Self::Reissue(_) => REISSUE_ROOT_ATTACHED_GRANT_ONE_TO_ONE_IDEMPOTENCY_NAMESPACE_V1,
            Self::Revoke(_) => REVOKE_GRANT_IDEMPOTENCY_NAMESPACE_V1,
        }
    }

    fn identity(&self) -> super::GrantActionIdentityV1 {
        match self {
            Self::Reissue(plan) => plan.identity,
            Self::Revoke(plan) => plan.identity,
        }
    }

    fn idempotency_key(&self) -> super::IdempotencyKeyIdV1 {
        self.identity().idempotency_key()
    }

    fn lineage(&self) -> super::AuthorityPublicationLineageV1 {
        match self {
            Self::Reissue(plan) => plan.lineage,
            Self::Revoke(plan) => plan.lineage,
        }
    }

    fn authority(&self) -> GrantAdministrationAuthorityV1 {
        match self {
            Self::Reissue(plan) => plan.authority,
            Self::Revoke(plan) => plan.authority,
        }
    }

    fn target_grant_id(&self) -> super::GrantIdV1 {
        match self {
            Self::Reissue(plan) => plan.retired_grant_id,
            Self::Revoke(plan) => plan.target_grant_id,
        }
    }

    fn action(&self) -> &'static str {
        match self {
            Self::Reissue(_) => "ReissueRootAttachedGrantOneToOne",
            Self::Revoke(_) => "RevokeGrant",
        }
    }

    fn meaning_digest(&self) -> Result<[u8; 32], AuthorityPublicationError> {
        let mut fields = vec![
            CborValue::text("maestro.vnext.ordinary-grant-mutation-meaning.v1")?,
            CborValue::text(self.action())?,
            bytes(self.identity().request_id().as_bytes()),
            bytes(self.identity().idempotency_key().as_bytes()),
            bytes(self.lineage().contract_root_id().as_bytes()),
            bytes(self.target_grant_id().as_bytes()),
            bytes(self.authority().terminal_grant_id().as_bytes()),
        ];
        if let Self::Reissue(plan) = self {
            fields.push(plan.grant.schema_value()?);
            fields.push(plan.delegation.schema_value()?);
        }
        Ok(Sha256::digest(deterministic_cbor::encode(&CborValue::Array(fields))?).into())
    }
}

fn grant_issue_meaning_digest(
    plan: &IssueRootAttachedBoundedGrantPublicationV1,
) -> Result<[u8; 32], AuthorityPublicationError> {
    let value = CborValue::Array(vec![
        CborValue::text("maestro.vnext.issue-root-attached-bounded-grant-meaning.v1")?,
        bytes(plan.identity.request_id().as_bytes()),
        bytes(plan.identity.idempotency_key().as_bytes()),
        bytes(plan.lineage.contract_root_id().as_bytes()),
        bytes(
            plan.lineage
                .previous_generation_id()
                .ok_or(AuthorityPublicationError::AuthorityPredecessorMismatch)?
                .as_bytes(),
        ),
        bytes(
            plan.lineage
                .expected_old()
                .ok_or(AuthorityPublicationError::AuthorityPredecessorMismatch)?
                .as_bytes(),
        ),
        bytes(
            plan.lineage
                .prior_authority_root()
                .ok_or(AuthorityPublicationError::AuthorityPredecessorMismatch)?
                .as_bytes(),
        ),
        bytes(plan.parent_genesis_grant_id.as_bytes()),
        plan.grant.schema_value()?,
        plan.delegation.schema_value()?,
    ]);
    Ok(Sha256::digest(deterministic_cbor::encode(&value)?).into())
}

fn prepare_root_attached_grant_issue(
    view: &StorePublicationViewV1<'_>,
    plan: IssueRootAttachedBoundedGrantPublicationV1,
    meaning_digest: [u8; 32],
) -> Result<AtomicGenerationPublicationV1, AuthorityPublicationError> {
    let current_head = view
        .active_head()?
        .ok_or(AuthorityPublicationError::MissingAuthorityPredecessor)?;
    let current_generation = view
        .active_generation()?
        .ok_or(AuthorityPublicationError::MissingAuthorityPredecessor)?;
    let previous_generation_id = plan
        .lineage
        .previous_generation_id()
        .ok_or(AuthorityPublicationError::AuthorityPredecessorMismatch)?;
    let expected_old = plan
        .lineage
        .expected_old()
        .ok_or(AuthorityPublicationError::AuthorityPredecessorMismatch)?;
    let prior_authority_root = plan
        .lineage
        .prior_authority_root()
        .ok_or(AuthorityPublicationError::AuthorityPredecessorMismatch)?;
    if current_head.id() != expected_old
        || current_generation.id() != previous_generation_id
        || current_generation.contract_root_id() != plan.lineage.contract_root_id()
        || !current_generation.roots().contains(&prior_authority_root)
    {
        return Err(AuthorityPublicationError::AuthorityPredecessorMismatch);
    }
    let active_objects = view.active_generation_objects()?;
    let current = load_current_authority(
        view,
        &current_head,
        &current_generation,
        prior_authority_root,
        &active_objects,
    )?;
    let mut parent_paths = current
        .facts
        .g0_candidate_paths()
        .iter()
        .filter(|path| {
            path.genesis_grant_id() == plan.parent_genesis_grant_id
                && path.grant().id()
                    == plan.grant.grant().parent_grant_id().unwrap_or_else(|| {
                        // The ordinary carrier constructor makes this branch unreachable.
                        path.grant().id()
                    })
                && path.store_generation() == current_generation.ordinal()
                && path.complete()
        })
        .collect::<Vec<_>>();
    if parent_paths.len() != 1 {
        return Err(AuthorityPublicationError::InvalidBootstrapGrantAuthority);
    }
    let parent = parent_paths
        .pop()
        .expect("invariant: exact one-element G0 parent check");
    if parent.grant().context_id() != plan.grant.grant().context_id()
        || parent.grant().context_id() != current.facts.context().context_id()
    {
        return Err(AuthorityPublicationError::InvalidBootstrapGrantAuthority);
    }
    let issue_scope = super::ScopeAtomV1::new(
        "IssueRootAttachedBoundedGrant",
        &plan.grant.capacity_root_id().render(),
        current.facts.snapshot().subject_revision,
    )?;
    validate_ordinary_authority(
        current.facts.snapshot(),
        current.facts.actor_binding(),
        current.facts.actor_session(),
        parent.grant(),
        &issue_scope,
        current.facts.revocations().revocations(),
    )
    .map_err(|_| AuthorityPublicationError::InvalidBootstrapGrantAuthority)?;
    require_established_capacity_root(
        &active_objects,
        current.facts.context().context_id(),
        plan.grant.capacity_root_id(),
    )?;
    let ancestry = DelegationAncestryV1::new(
        vec![parent.grant().id()],
        vec![parent.grant().grantee_principal_id()],
        false,
    )?;
    let delegation = plan.delegation.delegation();
    let mut structural_g0_definition = parent.grant().definition();
    structural_g0_definition.delegation_depth_remaining = u8::MAX;
    let structural_g0 = structural_g0_definition.validate()?;
    validate_delegation(&structural_g0, plan.grant.grant(), &delegation, &ancestry)?;

    let next_generation = current_generation
        .ordinal()
        .checked_add(1)
        .ok_or(AuthorityPublicationError::AuthorityPredecessorMismatch)?;
    let allocation = view.allocate_continuity_state_token(
        next_generation,
        Some(*current.state.state_token().as_bytes()),
        meaning_digest,
    )?;
    let allocation = bind_store_allocation(current.facts.context().context_id(), allocation)?;
    let continuity = build_successor_continuity(
        &current,
        &active_objects,
        &current_head,
        &current_generation,
        &allocation,
        meaning_digest,
    )?;
    let successor_facts = current.facts.continue_at_store_generation(
        next_generation,
        current.manifest.id(),
        continuity.state.guard_kind(),
        continuity.state.state_token(),
    )?;
    let referenced = direct_reference_objects(&current.snapshot_object, &active_objects)?;
    let successor_context = authority_object(
        AuthoritySchemaV1::AuthorityContext,
        successor_facts.context().schema_value()?,
        vec![],
    )?;
    let successor_actor_session = authority_object(
        AuthoritySchemaV1::Session,
        successor_facts.actor_session().schema_value()?,
        vec![],
    )?;
    let successor_responder_session = authority_object(
        AuthoritySchemaV1::Session,
        successor_facts.responder_session().schema_value()?,
        vec![],
    )?;
    let grant_object = authority_object(
        AuthoritySchemaV1::OrdinaryBoundedGrant,
        plan.grant.schema_value()?,
        vec![],
    )?;
    let delegation_object = authority_object(
        AuthoritySchemaV1::OrdinaryGrantDelegation,
        plan.delegation.schema_value()?,
        vec![grant_object.id()],
    )?;
    let basis_commitment: [u8; 32] =
        Sha256::digest(deterministic_cbor::encode(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.issue-root-attached-bounded-grant-basis.v1")?,
            bytes(current.facts.context().context_id().as_bytes()),
            bytes(parent.genesis_grant_id().as_bytes()),
            bytes(grant_object.id().as_bytes()),
            bytes(delegation_object.id().as_bytes()),
            bytes(continuity.state.state_token().as_bytes()),
        ]))?)
        .into();
    let basis_object = authority_object(
        AuthoritySchemaV1::ActionAuthorityBasis,
        CborValue::Array(vec![
            CborValue::Unsigned(ActionAuthorityBasisKindV1::BootstrapControlG0 as u64),
            bytes(current.facts.context().context_id().as_bytes()),
            bytes(&basis_commitment),
        ]),
        vec![grant_object.id(), delegation_object.id()],
    )?;
    let receipt = AuthorizationReceiptV1::new(
        plan.identity.request_id(),
        current.facts.context().context_id(),
        ActionAuthorityBasisKindV1::BootstrapControlG0,
        current.state.state_token(),
        continuity.state.state_token(),
    )?;
    let logical_result = ActionResultV1::new(
        plan.identity.request_id(),
        ActionOutcomeV1::Committed,
        Some(receipt.clone()),
        None,
    )?;
    let receipt_object = authority_object(
        AuthoritySchemaV1::AuthorizationReceipt,
        CborValue::Array(vec![
            bytes(receipt.id().as_bytes()),
            bytes(receipt.context_id().as_bytes()),
            bytes(receipt.request_id().as_bytes()),
            bytes(basis_object.id().as_bytes()),
            CborValue::Unsigned(1),
            CborValue::Bool(true),
            bytes(logical_result.id().as_bytes()),
        ]),
        vec![basis_object.id()],
    )?;
    let result_object = authority_object(
        AuthoritySchemaV1::ActionResult,
        CborValue::Array(vec![
            bytes(logical_result.id().as_bytes()),
            bytes(logical_result.request_id().as_bytes()),
            CborValue::Unsigned(logical_result.outcome() as u64),
            CborValue::Unsigned(1),
            CborValue::Array(vec![bytes(current.state.state_token().as_bytes())]),
            CborValue::Array(vec![bytes(continuity.state.state_token().as_bytes())]),
            CborValue::Array(vec![bytes(receipt.id().as_bytes())]),
            CborValue::Array(vec![
                bytes(plan.grant.grant().id().as_bytes()),
                bytes(plan.delegation.delegation().id.as_bytes()),
            ]),
            CborValue::Array(Vec::new()),
            CborValue::optional(None),
            CborValue::optional(None),
        ]),
        vec![
            grant_object.id(),
            delegation_object.id(),
            basis_object.id(),
            receipt_object.id(),
        ],
    )?;
    let mut snapshot_references = retained_snapshot_references(&referenced)?;
    snapshot_references.extend([
        continuity.closure_object.id(),
        continuity.guard_object.id(),
        continuity.state_object.id(),
        successor_context.id(),
        successor_actor_session.id(),
        successor_responder_session.id(),
        grant_object.id(),
        delegation_object.id(),
        basis_object.id(),
        receipt_object.id(),
        result_object.id(),
    ]);
    let successor_snapshot = authority_object(
        AuthoritySchemaV1::BootstrapAuthoritySnapshot,
        successor_facts.schema_value()?,
        snapshot_references,
    )?;
    let generation_roots = authority_successor_roots_preserving_governance(
        successor_snapshot.id(),
        &current_generation,
        &active_objects,
    )?;
    let generation = StoreGenerationV1::new(
        view.domain().clone(),
        next_generation,
        Some(previous_generation_id),
        plan.lineage.contract_root_id(),
        StoreCompatibilityV1::stage0_successor()?,
        generation_roots,
    )?;
    let idempotency = StoreIdempotencyV1::new(
        ISSUE_ROOT_ATTACHED_BOUNDED_GRANT_IDEMPOTENCY_NAMESPACE_V1,
        *plan.identity.idempotency_key().as_bytes(),
        meaning_digest,
        result_object.id(),
    )?;
    let mut objects = active_objects;
    objects.extend([
        successor_context,
        successor_actor_session,
        successor_responder_session,
        continuity.closure_object,
        continuity.guard_object,
        continuity.state_object,
        grant_object,
        delegation_object,
        basis_object,
        receipt_object,
        result_object,
        successor_snapshot,
    ]);
    objects.sort_by_key(StoreObjectV1::id);
    objects.dedup_by_key(|object| object.id());
    AtomicGenerationPublicationV1::new_from_object_superset(
        generation,
        Some(expected_old),
        objects,
        idempotency,
    )
    .map_err(Into::into)
}

fn prepare_ordinary_grant_mutation(
    view: &StorePublicationViewV1<'_>,
    mutation: OrdinaryGrantMutationV1,
    meaning_digest: [u8; 32],
) -> Result<AtomicGenerationPublicationV1, AuthorityPublicationError> {
    let current_head = view
        .active_head()?
        .ok_or(AuthorityPublicationError::MissingAuthorityPredecessor)?;
    let current_generation = view
        .active_generation()?
        .ok_or(AuthorityPublicationError::MissingAuthorityPredecessor)?;
    let lineage = mutation.lineage();
    let previous_generation_id = lineage
        .previous_generation_id()
        .ok_or(AuthorityPublicationError::AuthorityPredecessorMismatch)?;
    let expected_old = lineage
        .expected_old()
        .ok_or(AuthorityPublicationError::AuthorityPredecessorMismatch)?;
    let prior_authority_root = lineage
        .prior_authority_root()
        .ok_or(AuthorityPublicationError::AuthorityPredecessorMismatch)?;
    if current_head.id() != expected_old
        || current_generation.id() != previous_generation_id
        || current_generation.contract_root_id() != lineage.contract_root_id()
        || !current_generation.roots().contains(&prior_authority_root)
    {
        return Err(AuthorityPublicationError::AuthorityPredecessorMismatch);
    }
    let active_objects = view.active_generation_objects()?;
    let current = load_current_authority(
        view,
        &current_head,
        &current_generation,
        prior_authority_root,
        &active_objects,
    )?;
    let referenced = direct_reference_objects(&current.snapshot_object, &active_objects)?;
    let grants = load_ordinary_grants(&referenced)?;
    let target = one_grant(&grants, mutation.target_grant_id())?;
    let authority = mutation.authority();
    let administrator = one_grant(&grants, authority.terminal_grant_id())?;
    if super::grant_is_revoked_by_closure(
        administrator,
        &grants,
        current.facts.revocations().revocations(),
    )? {
        return Err(AuthorityPublicationError::InvalidGrantAdministrationAuthority);
    }
    let (binding, session) = if current.facts.actor_binding().id() == authority.actor_binding_id()
        && current.facts.actor_session().id() == authority.actor_session_id()
    {
        (current.facts.actor_binding(), current.facts.actor_session())
    } else if current.facts.responder_binding().id() == authority.actor_binding_id()
        && current.facts.responder_session().id() == authority.actor_session_id()
    {
        (
            current.facts.responder_binding(),
            current.facts.responder_session(),
        )
    } else {
        return Err(AuthorityPublicationError::InvalidGrantAdministrationAuthority);
    };
    let (capacity_root_object, capacity_root) = current_repository_admin_capacity_root(
        &referenced,
        current.facts.context().context_id(),
        administrator.capacity_root_id(),
    )?;
    let required_scope = super::ScopeAtomV1::new(
        mutation.action(),
        &target.capacity_root_id().render(),
        current.facts.snapshot().subject_revision,
    )?;
    validate_ordinary_authority(
        current.facts.snapshot(),
        binding,
        session,
        administrator.grant(),
        &required_scope,
        current.facts.revocations().revocations(),
    )?;
    let transition = capacity_root.transition(
        current.facts.context().context_id(),
        GovernedCapacityKindV1::Repository(
            RepositoryGovernedCapacitySlotKindV1::RepositoryAuthorityAdministration,
        ),
        capacity_root.spent(),
        CapacityUseDispositionV1::FreshCommit,
    )?;
    let successor_capacity_root = authority_object(
        AuthoritySchemaV1::GovernedCapacityRoot,
        transition.root().schema_value()?,
        vec![capacity_root_object.id()],
    )?;
    let debit = authority_object(
        AuthoritySchemaV1::GovernedCapacityDebit,
        transition
            .debit()
            .ok_or(AuthorityPublicationError::InvalidGrantAdministrationAuthority)?
            .schema_value()?,
        vec![capacity_root_object.id(), successor_capacity_root.id()],
    )?;

    let candidate = match &mutation {
        OrdinaryGrantMutationV1::Reissue(plan) => {
            if plan.grant.capacity_root_id() != target.capacity_root_id()
                || !plan
                    .grant
                    .grant()
                    .terminal_scope()
                    .is_subset(target.grant().terminal_scope())
                || !plan
                    .grant
                    .grant()
                    .delegable_scope()
                    .is_subset(target.grant().delegable_scope())
                || !target
                    .grant()
                    .validity()
                    .contains_interval(plan.grant.grant().validity())
                || plan.grant.grant().delegation_depth_remaining()
                    > target.grant().delegation_depth_remaining()
            {
                return Err(AuthorityPublicationError::GrantReissueWidening);
            }
            super::admit_repository_authority_candidate(
                &current.facts,
                target.capacity_root_id(),
                &plan.grant,
                &plan.delegation,
            )
            .map_err(|_| AuthorityPublicationError::GrantReissueWidening)?;
            Some((&plan.grant, &plan.delegation))
        }
        OrdinaryGrantMutationV1::Revoke(_) => None,
    };
    reject_administrator_ancestor_mutation(
        mutation.action(),
        administrator,
        target.grant().id(),
        &grants,
    )?;
    if !has_independently_live_repository_administrator(
        IndependentRepositoryAdministratorCheckV1 {
            grants: &grants,
            candidate: candidate.map(|(grant, _)| grant),
            target_grant_id: target.grant().id(),
            current_revocations: current.facts.revocations().revocations(),
            capacity_root_id: administrator.capacity_root_id(),
            action: mutation.action(),
            protocol_revision: current.facts.snapshot().subject_revision,
            trusted_time: current.facts.snapshot().trusted_time,
        },
    )? {
        return Err(AuthorityPublicationError::LastAdministrator);
    }

    let next_generation = current_generation
        .ordinal()
        .checked_add(1)
        .ok_or(AuthorityPublicationError::AuthorityPredecessorMismatch)?;
    let allocation = view.allocate_continuity_state_token(
        next_generation,
        Some(*current.state.state_token().as_bytes()),
        meaning_digest,
    )?;
    let allocation = bind_store_allocation(current.facts.context().context_id(), allocation)?;
    let continuity = build_successor_continuity(
        &current,
        &active_objects,
        &current_head,
        &current_generation,
        &allocation,
        meaning_digest,
    )?;
    let successor_facts = current
        .facts
        .continue_at_store_generation_with_revoked_grant(
            next_generation,
            current.manifest.id(),
            continuity.state.guard_kind(),
            continuity.state.state_token(),
            mutation.target_grant_id(),
        )?;
    let successor_context = authority_object(
        AuthoritySchemaV1::AuthorityContext,
        successor_facts.context().schema_value()?,
        vec![],
    )?;
    let successor_actor_session = authority_object(
        AuthoritySchemaV1::Session,
        successor_facts.actor_session().schema_value()?,
        vec![],
    )?;
    let successor_responder_session = authority_object(
        AuthoritySchemaV1::Session,
        successor_facts.responder_session().schema_value()?,
        vec![],
    )?;
    let revocations = authority_object(
        AuthoritySchemaV1::RevocationSet,
        successor_facts.revocations().schema_value()?,
        vec![],
    )?;
    let candidate_objects = candidate
        .map(|(grant, delegation)| {
            let grant_object = authority_object(
                AuthoritySchemaV1::OrdinaryBoundedGrant,
                grant.schema_value()?,
                vec![],
            )?;
            let delegation_object = authority_object(
                AuthoritySchemaV1::OrdinaryGrantDelegation,
                delegation.schema_value()?,
                vec![grant_object.id()],
            )?;
            Ok::<_, AuthorityPublicationError>((grant_object, delegation_object))
        })
        .transpose()?;
    let basis_commitment: [u8; 32] =
        Sha256::digest(deterministic_cbor::encode(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.ordinary-grant-administration-basis.v1")?,
            CborValue::text(mutation.action())?,
            bytes(administrator.grant().id().as_bytes()),
            bytes(mutation.target_grant_id().as_bytes()),
            bytes(debit.id().as_bytes()),
            bytes(continuity.state.state_token().as_bytes()),
        ]))?)
        .into();
    let basis = authority_object(
        AuthoritySchemaV1::ActionAuthorityBasis,
        CborValue::Array(vec![
            CborValue::Unsigned(ActionAuthorityBasisKindV1::OrdinaryLiveRuntime as u64),
            bytes(current.facts.context().context_id().as_bytes()),
            bytes(&basis_commitment),
        ]),
        vec![capacity_root_object.id(), debit.id()],
    )?;
    let receipt = AuthorizationReceiptV1::new(
        mutation.identity().request_id(),
        current.facts.context().context_id(),
        ActionAuthorityBasisKindV1::OrdinaryLiveRuntime,
        current.state.state_token(),
        continuity.state.state_token(),
    )?;
    let logical_result = ActionResultV1::new(
        mutation.identity().request_id(),
        ActionOutcomeV1::Committed,
        Some(receipt.clone()),
        None,
    )?;
    let receipt_object = authority_object(
        AuthoritySchemaV1::AuthorizationReceipt,
        CborValue::Array(vec![
            bytes(receipt.id().as_bytes()),
            bytes(receipt.context_id().as_bytes()),
            bytes(receipt.request_id().as_bytes()),
            bytes(basis.id().as_bytes()),
            CborValue::Unsigned(1),
            CborValue::Bool(true),
            bytes(logical_result.id().as_bytes()),
        ]),
        vec![basis.id()],
    )?;
    let mut produced = vec![
        bytes(revocations.id().as_bytes()),
        bytes(debit.id().as_bytes()),
    ];
    if let Some((grant, delegation)) = candidate_objects.as_ref() {
        produced.extend([
            bytes(grant.id().as_bytes()),
            bytes(delegation.id().as_bytes()),
        ]);
    }
    let mut result_references = vec![
        basis.id(),
        receipt_object.id(),
        revocations.id(),
        successor_capacity_root.id(),
        debit.id(),
    ];
    if let Some((grant, delegation)) = candidate_objects.as_ref() {
        result_references.extend([grant.id(), delegation.id()]);
    }
    let result = authority_object(
        AuthoritySchemaV1::ActionResult,
        CborValue::Array(vec![
            bytes(logical_result.id().as_bytes()),
            bytes(logical_result.request_id().as_bytes()),
            CborValue::Unsigned(logical_result.outcome() as u64),
            CborValue::Unsigned(1),
            CborValue::Array(vec![bytes(current.state.state_token().as_bytes())]),
            CborValue::Array(vec![bytes(continuity.state.state_token().as_bytes())]),
            CborValue::Array(vec![bytes(receipt.id().as_bytes())]),
            CborValue::Array(produced),
            CborValue::Array(Vec::new()),
            CborValue::optional(None),
            CborValue::optional(None),
        ]),
        result_references,
    )?;
    let mut snapshot_references = retained_snapshot_references(&referenced)?;
    let replaced_revocation_id = AuthoritySchemaV1::RevocationSet.id()?;
    let replaced_capacity_id = AuthoritySchemaV1::GovernedCapacityRoot.id()?;
    snapshot_references.retain(|id| {
        referenced
            .iter()
            .find(|object| object.id() == *id)
            .is_none_or(|object| {
                object.schema_id() != replaced_revocation_id
                    && !(object.schema_id() == replaced_capacity_id
                        && object.id() == capacity_root_object.id())
            })
    });
    snapshot_references.extend([
        continuity.closure_object.id(),
        continuity.guard_object.id(),
        continuity.state_object.id(),
        successor_context.id(),
        successor_actor_session.id(),
        successor_responder_session.id(),
        revocations.id(),
        successor_capacity_root.id(),
        debit.id(),
        basis.id(),
        receipt_object.id(),
        result.id(),
    ]);
    if let Some((grant, delegation)) = candidate_objects.as_ref() {
        snapshot_references.extend([grant.id(), delegation.id()]);
    }
    let successor_snapshot = authority_object(
        AuthoritySchemaV1::BootstrapAuthoritySnapshot,
        successor_facts.schema_value()?,
        snapshot_references,
    )?;
    let generation_roots = authority_successor_roots_preserving_governance(
        successor_snapshot.id(),
        &current_generation,
        &active_objects,
    )?;
    let generation = StoreGenerationV1::new(
        view.domain().clone(),
        next_generation,
        Some(previous_generation_id),
        lineage.contract_root_id(),
        StoreCompatibilityV1::stage0_successor()?,
        generation_roots,
    )?;
    let idempotency = StoreIdempotencyV1::new(
        mutation.idempotency_namespace(),
        *mutation.idempotency_key().as_bytes(),
        meaning_digest,
        result.id(),
    )?;
    let mut objects = active_objects;
    objects.extend([
        successor_context,
        successor_actor_session,
        successor_responder_session,
        revocations,
        successor_capacity_root,
        debit,
        basis,
        receipt_object,
        result,
        continuity.closure_object,
        continuity.guard_object,
        continuity.state_object,
        successor_snapshot,
    ]);
    if let Some((grant, delegation)) = candidate_objects {
        objects.extend([grant, delegation]);
    }
    objects.sort_by_key(StoreObjectV1::id);
    objects.dedup_by_key(|object| object.id());
    AtomicGenerationPublicationV1::new_from_object_superset(
        generation,
        Some(expected_old),
        objects,
        idempotency,
    )
    .map_err(Into::into)
}

fn reject_administrator_ancestor_mutation(
    action: &str,
    administrator: &super::OrdinaryBoundedGrantV1,
    target_grant_id: super::GrantIdV1,
    grants: &[super::OrdinaryBoundedGrantV1],
) -> Result<(), AuthorityPublicationError> {
    if !matches!(action, "ReissueRootAttachedGrantOneToOne" | "RevokeGrant") {
        return Err(AuthorityPublicationError::InvalidGrantAdministrationAuthority);
    }
    let target_revocation =
        super::RevocationSetV1::new(vec![super::RevocationTargetV1::Grant(target_grant_id)])?;
    if super::grant_is_revoked_by_closure(administrator, grants, &target_revocation)? {
        return Err(AuthorityPublicationError::InvalidGrantAdministrationAuthority);
    }
    Ok(())
}

struct IndependentRepositoryAdministratorCheckV1<'a> {
    grants: &'a [super::OrdinaryBoundedGrantV1],
    candidate: Option<&'a super::OrdinaryBoundedGrantV1>,
    target_grant_id: super::GrantIdV1,
    current_revocations: &'a super::RevocationSetV1,
    capacity_root_id: super::CapacityRootIdV1,
    action: &'a str,
    protocol_revision: u64,
    trusted_time: TrustedTimeV1,
}

fn has_independently_live_repository_administrator(
    check: IndependentRepositoryAdministratorCheckV1<'_>,
) -> Result<bool, AuthorityPublicationError> {
    let mut targets = check.current_revocations.targets().collect::<Vec<_>>();
    targets.push(super::RevocationTargetV1::Grant(check.target_grant_id));
    let post_mutation_revocations = super::RevocationSetV1::new(targets)?;
    if !matches!(
        check.action,
        "ReissueRootAttachedGrantOneToOne" | "RevokeGrant"
    ) {
        return Err(AuthorityPublicationError::InvalidGrantAdministrationAuthority);
    }
    let required_scope = super::ScopeAtomV1::new(
        check.action,
        &check.capacity_root_id.render(),
        check.protocol_revision,
    )?;
    let post_mutation_grants = check
        .grants
        .iter()
        .chain(check.candidate)
        .cloned()
        .collect::<Vec<_>>();
    for grant in &post_mutation_grants {
        if grant.capacity_root_id() != check.capacity_root_id
            || !grant.grant().terminal_scope().contains(&required_scope)
            || !check.trusted_time.is_within(grant.grant().validity())?
        {
            continue;
        }
        if !super::grant_is_revoked_by_closure(
            grant,
            &post_mutation_grants,
            &post_mutation_revocations,
        )? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn load_ordinary_grants(
    referenced: &[StoreObjectV1],
) -> Result<Vec<super::OrdinaryBoundedGrantV1>, AuthorityPublicationError> {
    schema_objects(referenced, AuthoritySchemaV1::OrdinaryBoundedGrant)?
        .into_iter()
        .map(|object| {
            super::OrdinaryBoundedGrantV1::from_canonical_bytes(&object_value_bytes(&object)?)
                .map_err(Into::into)
        })
        .collect()
}

fn one_grant(
    grants: &[super::OrdinaryBoundedGrantV1],
    id: super::GrantIdV1,
) -> Result<&super::OrdinaryBoundedGrantV1, AuthorityPublicationError> {
    let mut matches = grants
        .iter()
        .filter(|grant| grant.grant().id() == id)
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(AuthorityPublicationError::InvalidGrantAdministrationAuthority);
    }
    Ok(matches
        .pop()
        .expect("invariant: exact one-element Grant check"))
}

fn current_repository_admin_capacity_root(
    referenced: &[StoreObjectV1],
    context_id: AuthorityContextIdV1,
    root_id: super::CapacityRootIdV1,
) -> Result<(StoreObjectV1, GovernedCapacityRootV1), AuthorityPublicationError> {
    let mut matches = schema_objects(referenced, AuthoritySchemaV1::GovernedCapacityRoot)?
        .into_iter()
        .filter_map(|object| {
            let CborValue::Array(fields) = object.value() else {
                return None;
            };
            if fields.len() != 7
                || exact_digest(&fields[1]).ok()? != *root_id.as_bytes()
                || exact_digest(&fields[3]).ok()? != *context_id.as_bytes()
                || fields[2]
                    != CborValue::Unsigned(
                        AuthorityContextKindV1::RepositoryAuthorityContext as u64,
                    )
                || fields[4]
                    != CborValue::Unsigned(
                        RepositoryGovernedCapacitySlotKindV1::RepositoryAuthorityAdministration
                            as u64,
                    )
            {
                return None;
            }
            let (CborValue::Unsigned(initial), CborValue::Unsigned(spent)) =
                (&fields[5], &fields[6])
            else {
                return None;
            };
            Some(
                GovernedCapacityRootV1::from_persisted_state(
                    root_id,
                    AuthorityContextKindV1::RepositoryAuthorityContext,
                    context_id,
                    GovernedCapacityKindV1::Repository(
                        RepositoryGovernedCapacitySlotKindV1::RepositoryAuthorityAdministration,
                    ),
                    u32::try_from(*initial).ok()?,
                    u32::try_from(*spent).ok()?,
                )
                .ok()
                .map(|root| (object, root)),
            )
            .flatten()
        })
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(AuthorityPublicationError::InvalidGrantAdministrationAuthority);
    }
    Ok(matches
        .pop()
        .expect("invariant: exact one-element admin capacity root check"))
}

fn require_established_capacity_root(
    active_objects: &[StoreObjectV1],
    context_id: AuthorityContextIdV1,
    root_id: super::CapacityRootIdV1,
) -> Result<(), AuthorityPublicationError> {
    let matches = schema_objects(active_objects, AuthoritySchemaV1::GovernedCapacityRoot)?
        .into_iter()
        .filter(|object| {
            let CborValue::Array(fields) = object.value() else {
                return false;
            };
            fields.len() == 7
                && exact_digest(&fields[1]).is_ok_and(|id| id == *root_id.as_bytes())
                && exact_digest(&fields[3]).is_ok_and(|id| id == *context_id.as_bytes())
                && matches!((&fields[5], &fields[6]), (CborValue::Unsigned(maximum), CborValue::Unsigned(spent)) if *maximum > 0 && spent <= maximum)
        })
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(AuthorityPublicationError::UnestablishedCapacityRoot);
    }
    Ok(())
}

fn retained_snapshot_references(
    referenced: &[StoreObjectV1],
) -> Result<Vec<StoreObjectIdV1>, AuthorityPublicationError> {
    let retained = [
        AuthoritySchemaV1::AuthorityContinuityManifest.id()?,
        AuthoritySchemaV1::PrincipalBinding.id()?,
        AuthoritySchemaV1::BootstrapGenesisGrant.id()?,
        AuthoritySchemaV1::BootstrapMandateInteractionObservationJoin.id()?,
        AuthoritySchemaV1::ConsentSlotBindingParameter.id()?,
        AuthoritySchemaV1::RevocationSet.id()?,
        AuthoritySchemaV1::GovernedCapacityRoot.id()?,
        AuthoritySchemaV1::OrdinaryBoundedGrant.id()?,
        AuthoritySchemaV1::OrdinaryGrantDelegation.id()?,
    ];
    Ok(referenced
        .iter()
        .filter(|object| retained.contains(&object.schema_id()))
        .map(StoreObjectV1::id)
        .collect())
}

fn grant_publication_outcome(
    outcome: StorePublicationOutcomeV1,
) -> Result<AuthorityPublicationOutcomeV1, AuthorityPublicationError> {
    let (kind, head, result) = match outcome {
        StorePublicationOutcomeV1::Committed { head, result } => {
            (AuthorityPublicationKindV1::Committed, head, result)
        }
        StorePublicationOutcomeV1::Replayed { head, result } => {
            (AuthorityPublicationKindV1::Replayed, head, result)
        }
    };
    if result.schema_id() != AuthoritySchemaV1::ActionResult.id()? {
        return Err(AuthorityPublicationError::InvalidPublishedResult);
    }
    let CborValue::Array(fields) = result.value() else {
        return Err(AuthorityPublicationError::InvalidPublishedResult);
    };
    let logical_result_id = fields
        .first()
        .ok_or(AuthorityPublicationError::InvalidPublishedResult)
        .and_then(exact_digest)?;
    Ok(AuthorityPublicationOutcomeV1 {
        kind,
        head,
        result,
        logical_result_id: ActionResultIdV1::from_digest(logical_result_id),
    })
}

struct PreparedBootstrapRequestV1 {
    meaning_digest: [u8; 32],
}

impl PreparedBootstrapRequestV1 {
    fn new(plan: &IssueBootstrapMandatePublicationV1) -> Result<Self, AuthorityPublicationError> {
        let meaning_value = CborValue::Array(vec![
            CborValue::text("maestro.vnext.issue-bootstrap-mandate-meaning.v1")?,
            plan.request.schema_value()?,
            bytes(plan.contract_root_id.as_bytes()),
            bytes(plan.previous_generation_id.as_bytes()),
            bytes(plan.expected_old.as_bytes()),
            bytes(plan.prior_authority_root.as_bytes()),
            CborValue::optional(plan.next_or_inspect_ref.map(|value| bytes(&value))),
        ]);
        let meaning_digest = Sha256::digest(deterministic_cbor::encode(&meaning_value)?).into();
        Ok(Self { meaning_digest })
    }
}

struct CurrentAuthorityV1 {
    facts: BootstrapAuthoritySnapshotV1,
    snapshot_object: StoreObjectV1,
    manifest: AuthorityContinuityManifestV1,
    closure: AuthorityContinuityClosureV1,
    state: SuccessVisibleAuthorityContinuityStateV1,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProtectedContinuityDiagnosticDispositionV1 {
    CurrentProtectedSnapshot,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProtectedContinuityDiagnosticReferenceEnvelopeV1 {
    disposition: ProtectedContinuityDiagnosticDispositionV1,
}

#[cfg(test)]
impl ProtectedContinuityDiagnosticReferenceEnvelopeV1 {
    pub(crate) const fn disposition(self) -> ProtectedContinuityDiagnosticDispositionV1 {
        self.disposition
    }
}

const PROTECTED_DIAGNOSTIC_CHALLENGE_DOMAIN_V1: &[u8] =
    b"maestro.vnext.trusted-host-diagnostic-challenge.v1";
const PROTECTED_DIAGNOSTIC_ATTESTATION_DOMAIN_V1: &[u8] =
    b"maestro.vnext.trusted-host-diagnostic-attestation.v1";

#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "Stage 5 freezes challenge accessors before the Stage 10 producer"
    )
)]
pub(crate) struct TrustedHostDiagnosticChallengeV1<'anchor, 'view> {
    current_view_anchor: &'anchor ProtectedDiagnosticCurrentViewAnchorV1<'view>,
    anchor_commitment: [u8; 32],
    authority_commitment: [u8; 32],
    protected_subject_commitment: [u8; 32],
    invocation_nonce: [u8; 32],
    commitment: [u8; 32],
}

#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "Stage 5 freezes challenge accessors before the Stage 10 producer"
    )
)]
impl<'anchor, 'view> TrustedHostDiagnosticChallengeV1<'anchor, 'view> {
    fn from_authority_issuance(
        current_view_anchor: &'anchor ProtectedDiagnosticCurrentViewAnchorV1<'view>,
        authority_commitment: [u8; 32],
        protected_subject_commitment: [u8; 32],
        invocation_nonce: [u8; 32],
    ) -> Result<Self, AuthorityPublicationError> {
        let anchor_commitment = current_view_anchor.commitment();
        if [anchor_commitment, authority_commitment, invocation_nonce].contains(&[0; 32]) {
            return Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot);
        }
        let commitment = protected_diagnostic_tuple_commitment(
            PROTECTED_DIAGNOSTIC_CHALLENGE_DOMAIN_V1,
            &[
                &anchor_commitment,
                &authority_commitment,
                &protected_subject_commitment,
                &invocation_nonce,
            ],
        );
        Ok(Self {
            current_view_anchor,
            anchor_commitment,
            authority_commitment,
            protected_subject_commitment,
            invocation_nonce,
            commitment,
        })
    }

    pub(crate) const fn current_view_anchor(
        &self,
    ) -> &'anchor ProtectedDiagnosticCurrentViewAnchorV1<'view> {
        self.current_view_anchor
    }

    pub(crate) const fn anchor_commitment(&self) -> [u8; 32] {
        self.anchor_commitment
    }

    pub(crate) const fn authority_commitment(&self) -> [u8; 32] {
        self.authority_commitment
    }

    pub(crate) const fn protected_subject_commitment(&self) -> [u8; 32] {
        self.protected_subject_commitment
    }

    pub(crate) const fn invocation_nonce(&self) -> [u8; 32] {
        self.invocation_nonce
    }

    pub(crate) const fn commitment(&self) -> [u8; 32] {
        self.commitment
    }
}

struct ProtectedContinuityDiagnosticReadGuardV1<'anchor, 'view> {
    _witness: LinearizationCoverageWitnessV1,
    _current_view_anchor: &'anchor ProtectedDiagnosticCurrentViewAnchorV1<'view>,
}

impl ProtectedContinuityDiagnosticReadGuardMarkerV1
    for ProtectedContinuityDiagnosticReadGuardV1<'_, '_>
{
}

struct ProtectedDiagnosticInvocationIssuerV1 {
    process_incarnation: [u8; 32],
    invocation_entropy: [u8; 32],
    sequence: u64,
}

impl ProtectedDiagnosticInvocationIssuerV1 {
    fn fresh() -> Result<Self, AuthorityPublicationError> {
        let process_incarnation = protected_diagnostic_process_incarnation()?;
        let invocation_entropy = protected_diagnostic_random_entropy(
            b"maestro.vnext.protected-diagnostic-invocation-entropy.v1",
        );
        if invocation_entropy == [0; 32] {
            return Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot);
        }
        let sequence = protected_diagnostic_invocation_sequence()?;
        Ok(Self {
            process_incarnation,
            invocation_entropy,
            sequence,
        })
    }
}

struct SuccessorContinuityV1 {
    closure: AuthorityContinuityClosureV1,
    state: SuccessVisibleAuthorityContinuityStateV1,
    closure_object: StoreObjectV1,
    guard_object: StoreObjectV1,
    state_object: StoreObjectV1,
}

fn load_current_authority(
    view: &StorePublicationViewV1<'_>,
    current_head: &crate::domain::vnext::persistence::StoreHeadV1,
    current_generation: &StoreGenerationV1,
    prior_authority_root: StoreObjectIdV1,
    active_objects: &[StoreObjectV1],
) -> Result<CurrentAuthorityV1, AuthorityPublicationError> {
    debug_assert_eq!(AuthoritySchemaV1::ALL.len(), 25);
    let role = view.role();
    let root = active_objects
        .iter()
        .find(|object| object.id() == prior_authority_root)
        .ok_or(AuthorityPublicationError::MissingAuthorityPredecessor)?;
    let snapshot_schema = AuthoritySchemaV1::BootstrapAuthoritySnapshot.id()?;
    let post_cut_schema = AuthoritySchemaV1::AuthorityContinuityPostCutConsequenceSet.id()?;
    if root.schema_id() != snapshot_schema && root.schema_id() != post_cut_schema {
        return Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot);
    }
    let post_cut = if root.schema_id() == post_cut_schema {
        Some(AuthorityContinuityPostCutConsequenceSetV1::decode(
            &object_value_bytes(root)?,
        )?)
    } else {
        None
    };
    let root_references = direct_reference_objects(root, active_objects)?;
    let mut snapshot_objects = if root.schema_id() == snapshot_schema {
        vec![root.clone()]
    } else {
        root_references
            .iter()
            .filter(|object| object.schema_id() == snapshot_schema)
            .cloned()
            .collect::<Vec<_>>()
    };
    snapshot_objects.retain(|object| {
        BootstrapAuthoritySnapshotV1::from_canonical_bytes(
            &object_value_bytes(object).unwrap_or_default(),
        )
        .is_ok_and(|facts| {
            facts.context().store_generation() == current_generation.ordinal()
                && facts.snapshot().store_generation == current_generation.ordinal()
        })
    });
    if snapshot_objects.len() != 1 {
        return Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot);
    }
    let snapshot_object = snapshot_objects
        .pop()
        .expect("invariant: exact one-element check");
    let facts =
        BootstrapAuthoritySnapshotV1::from_canonical_bytes(&object_value_bytes(&snapshot_object)?)?;
    ensure_context_role(role, facts.context().kind())?;
    if facts.context().store_generation() != current_generation.ordinal()
        || facts.snapshot().store_generation != current_generation.ordinal()
        || facts.context().authority_epoch() != facts.snapshot().authority_epoch
        || facts.context().trust_root_revision() != facts.snapshot().trust_root_revision
        || current_head.generation_id() != current_generation.id()
    {
        return Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot);
    }

    let referenced_facts = direct_reference_objects(&snapshot_object, active_objects)?;
    let manifest = match role {
        StoreRoleV1::Repository => AuthorityContinuityManifestV1::repository()?,
        StoreRoleV1::Installation => AuthorityContinuityManifestV1::installation()?,
    };
    let manifest_object = find_exact_object(
        &referenced_facts,
        AuthoritySchemaV1::AuthorityContinuityManifest,
        &manifest.canonical_bytes()?,
    )?;
    let closure_object = one_schema_object(
        &referenced_facts,
        AuthoritySchemaV1::AuthorityContinuityClosure,
    )?;
    let closure =
        AuthorityContinuityClosureV1::decode(&object_value_bytes(&closure_object)?, &manifest)?;
    let state_object = one_schema_object(
        &referenced_facts,
        AuthoritySchemaV1::SuccessVisibleAuthorityContinuityState,
    )?;
    let guard_object = one_schema_object(
        &referenced_facts,
        AuthoritySchemaV1::AdmittedTransitionGuard,
    )?;
    let state = SuccessVisibleAuthorityContinuityStateV1::decode(
        &object_value_bytes(&state_object)?,
        &manifest,
    )?;
    let proof = facts.continuity();
    if closure.store_generation() != current_generation.ordinal()
        || state.store_generation() != current_generation.ordinal()
        || closure.id() != state.closure_id()
        || closure.successor_state_token() != state.state_token()
        || proof.context_id() != facts.context().context_id()
        || proof.store_generation() != current_generation.ordinal()
        || proof.authority_epoch() != state.authority_epoch()
        || proof.manifest_id() != manifest.id()
        || proof.guard_kind() != state.guard_kind()
        || proof.state_token() != state.state_token()
    {
        return Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot);
    }

    let mut required_references = vec![
        manifest_object.id(),
        closure_object.id(),
        guard_object.id(),
        state_object.id(),
    ];
    required_references.push(
        find_exact_object(
            &referenced_facts,
            AuthoritySchemaV1::AuthorityContext,
            &facts.context().canonical_bytes()?,
        )?
        .id(),
    );
    for binding in [facts.actor_binding(), facts.responder_binding()] {
        required_references.push(
            find_exact_object(
                &referenced_facts,
                AuthoritySchemaV1::PrincipalBinding,
                &binding.canonical_bytes()?,
            )?
            .id(),
        );
    }
    for session in [facts.actor_session(), facts.responder_session()] {
        required_references.push(
            find_exact_object(
                &referenced_facts,
                AuthoritySchemaV1::Session,
                &session.canonical_bytes()?,
            )?
            .id(),
        );
    }
    let mut expected_grants = facts
        .g0_candidate_paths()
        .iter()
        .map(|path| path.genesis_grant().canonical_bytes())
        .collect::<Result<Vec<_>, _>>()?;
    expected_grants.sort_unstable();
    let grant_objects =
        schema_objects(&referenced_facts, AuthoritySchemaV1::BootstrapGenesisGrant)?;
    let mut stored_grants = grant_objects
        .iter()
        .map(object_value_bytes)
        .collect::<Result<Vec<_>, _>>()?;
    stored_grants.sort_unstable();
    if expected_grants != stored_grants {
        return Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot);
    }
    required_references.extend(grant_objects.iter().map(StoreObjectV1::id));
    required_references.push(
        find_exact_object(
            &referenced_facts,
            AuthoritySchemaV1::RevocationSet,
            &facts.revocations().canonical_bytes()?,
        )?
        .id(),
    );
    required_references.push(
        find_exact_object(
            &referenced_facts,
            AuthoritySchemaV1::ConsentSlotBindingParameter,
            &facts.consent_slot().binding().canonical_bytes()?,
        )?
        .id(),
    );
    let interaction = facts
        .interaction_join()
        .ok_or(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot)?;
    required_references.push(
        find_exact_object(
            &referenced_facts,
            AuthoritySchemaV1::BootstrapMandateInteractionObservationJoin,
            &interaction.canonical_bytes()?,
        )?
        .id(),
    );
    required_references.sort_unstable();
    required_references.dedup();
    if required_references
        .iter()
        .any(|object_id| !snapshot_object.references().contains(object_id))
    {
        return Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot);
    }

    if let Some(post_cut) = post_cut.as_ref() {
        validate_post_cut_current_authority(
            view,
            current_head,
            current_generation,
            root,
            &root_references,
            &snapshot_object,
            &facts,
            &closure,
            &state,
            closure_object.id(),
            guard_object.id(),
            state_object.id(),
            post_cut,
        )?;
    }

    Ok(CurrentAuthorityV1 {
        facts,
        snapshot_object,
        manifest,
        closure,
        state,
    })
}

fn select_current_authority_root(
    current_generation: &StoreGenerationV1,
    active_objects: &[StoreObjectV1],
) -> Result<StoreObjectIdV1, AuthorityPublicationError> {
    let snapshot_schema = AuthoritySchemaV1::BootstrapAuthoritySnapshot.id()?;
    let post_cut_schema = AuthoritySchemaV1::AuthorityContinuityPostCutConsequenceSet.id()?;
    let mut authority_roots = Vec::new();
    for root_id in current_generation.roots() {
        let root = active_objects
            .iter()
            .find(|object| object.id() == *root_id)
            .ok_or(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot)?;
        if root.schema_id() == snapshot_schema || root.schema_id() == post_cut_schema {
            authority_roots.push(root.id());
        }
    }
    let [authority_root] = authority_roots.as_slice() else {
        return Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot);
    };
    Ok(*authority_root)
}

fn build_protected_continuity_diagnostic(
    view: &StorePublicationViewV1<'_>,
    connection: &mut dyn TrustedHostDiagnosticConnectionPortV1,
    current_view_provider: &mut dyn ProtectedDiagnosticCurrentViewProviderV1,
    requested_subject: ContinuityReferenceV1,
    invocation_issuer: &ProtectedDiagnosticInvocationIssuerV1,
    assembler_mode: ProtectedContinuityDiagnosticAssemblerModeV1,
) -> Result<ProtectedContinuityDiagnosticReleasedEnvelopeV1, AuthorityPublicationError> {
    if view.role() != StoreRoleV1::Repository {
        return Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot);
    }
    let current_view_anchor = view
        .protected_diagnostic_current_view_anchor(current_view_provider)
        .map_err(|_| AuthorityPublicationError::InvalidCurrentAuthoritySnapshot)?;
    let current_head = view
        .active_head()?
        .ok_or(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot)?;
    let current_generation = view
        .active_generation()?
        .ok_or(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot)?;
    if current_head.generation_id() != current_generation.id()
        || current_generation.domain() != view.domain()
    {
        return Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot);
    }
    let active_objects = view.active_generation_objects()?;
    let current_root = select_current_authority_root(&current_generation, &active_objects)?;
    let current = load_current_authority(
        view,
        &current_head,
        &current_generation,
        current_root,
        &active_objects,
    )?;
    let facts = &current.facts;
    let snapshot = facts.snapshot();
    if facts.continuity().context_id() != facts.context().context_id()
        || facts.continuity().store_generation() != current_generation.ordinal()
        || facts.continuity().authority_epoch() != snapshot.authority_epoch
        || facts.continuity().trust_root_revision() != snapshot.trust_root_revision
        || facts.continuity().state_token() != current.state.state_token()
    {
        return Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot);
    }
    let TrustedTimeV1::Verified {
        lower_bound,
        upper_bound,
    } = snapshot.trusted_time
    else {
        return Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot);
    };
    if lower_bound > upper_bound
        || !snapshot
            .trusted_time
            .is_within(facts.continuity().validity())?
    {
        return Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot);
    }

    let referenced = direct_reference_objects(&current.snapshot_object, &active_objects)?;
    let state_object = one_schema_object(
        &referenced,
        AuthoritySchemaV1::SuccessVisibleAuthorityContinuityState,
    )?;
    let guard_object = one_schema_object(&referenced, AuthoritySchemaV1::AdmittedTransitionGuard)?;
    let authority_commitment = protected_diagnostic_authority_commitment(
        view,
        &current_head,
        &current_generation,
        current_root,
        &current,
        state_object.id(),
        guard_object.id(),
    )?;
    let invocation_nonce = protected_diagnostic_invocation_nonce(
        invocation_issuer,
        current_view_anchor.commitment(),
        authority_commitment,
        *requested_subject.as_bytes(),
    )?;
    let challenge = TrustedHostDiagnosticChallengeV1::from_authority_issuance(
        &current_view_anchor,
        authority_commitment,
        *requested_subject.as_bytes(),
        invocation_nonce,
    )?;
    let challenge_commitment = challenge.commitment();
    let mut attestation = connection
        .attest_in_current_view(challenge)
        .ok_or(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot)?;

    let mut presented_validation = Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot);
    let presented = attestation.present_once(
        &mut |presentation: &dyn TrustedHostDiagnosticPresentationPortV1| {
            presented_validation = (|| {
                let presented_host_facts =
                    ProtectedDiagnosticPresentedHostFactsV1::capture(presentation);
                let recomputed_attestation_commitment =
                    presented_host_facts.attestation_commitment();
                if presented_host_facts.anchor_commitment != current_view_anchor.commitment()
                    || presented_host_facts.authority_commitment != authority_commitment
                    || presented_host_facts.protected_subject_commitment
                        != *requested_subject.as_bytes()
                    || presented_host_facts.protected_subject_commitment == [0; 32]
                    || presented_host_facts.invocation_nonce != invocation_nonce
                    || presented_host_facts.challenge_commitment != challenge_commitment
                    || presented_host_facts.presented_attestation_commitment == [0; 32]
                    || presented_host_facts.presented_attestation_commitment
                        != recomputed_attestation_commitment
                {
                    return Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot);
                }
                let mut matches = 0usize;
                for (binding, session) in [
                    (facts.actor_binding(), facts.actor_session()),
                    (facts.responder_binding(), facts.responder_session()),
                ] {
                    if protected_diagnostic_operator_is_current(
                        facts,
                        binding,
                        session,
                        &current_generation,
                    )? && presented_host_facts.principal_identity
                        == *binding.principal_id().as_bytes()
                        && presented_host_facts.binding_identity == *binding.id().as_bytes()
                        && presented_host_facts.session_identity == *session.id().as_bytes()
                        && presented_host_facts.context_identity == *binding.context_id().as_bytes()
                        && presented_host_facts.trust_root_revision == binding.trust_root_revision()
                        && presented_host_facts.assurance_revision == binding.assurance_revision()
                        && presented_host_facts.human_capable == binding.human_capable()
                        && presented_host_facts.binding_not_before
                            == binding.validity().not_before()
                        && presented_host_facts.binding_expires_at
                            == binding.validity().expires_at()
                        && presented_host_facts.session_not_before
                            == session.validity().not_before()
                        && presented_host_facts.session_expires_at
                            == session.validity().expires_at()
                        && presented_host_facts.store_generation == session.store_generation()
                        && presented_host_facts.authority_epoch == session.authority_epoch()
                        && presented_host_facts.domain_identity == *view.domain().id().as_bytes()
                        && presented_host_facts.domain_role == view.role().tag()
                    {
                        let binding_object = find_exact_object(
                            &referenced,
                            AuthoritySchemaV1::PrincipalBinding,
                            &binding.canonical_bytes()?,
                        )?;
                        let session_object = find_exact_object(
                            &referenced,
                            AuthoritySchemaV1::Session,
                            &session.canonical_bytes()?,
                        )?;
                        require_exact_object_references(&session_object, &[binding_object.id()])?;
                        matches += 1;
                    }
                }
                Ok((matches, recomputed_attestation_commitment))
            })();
            presented_validation.is_ok()
        },
    );
    if !presented {
        return Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot);
    }
    let (live_operator_matches, verified_attestation_commitment) = presented_validation?;
    if live_operator_matches != 1 {
        return Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot);
    }

    let protected_snapshot_subject =
        ContinuityReferenceV1::from_digest(*current.snapshot_object.id().as_bytes());
    if requested_subject != protected_snapshot_subject {
        return Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot);
    }

    let authentication_carrier_ref =
        ContinuityReferenceV1::from_digest(verified_attestation_commitment);
    let currentness_and_fence_ref = hash_reference(&CborValue::Array(vec![
        CborValue::text("maestro.vnext.protected-diagnostic-currentness-and-fence.v1")?,
        bytes(current_head.id().as_bytes()),
        bytes(current_generation.id().as_bytes()),
        bytes(current_root.as_bytes()),
        bytes(current.snapshot_object.id().as_bytes()),
        bytes(state_object.id().as_bytes()),
        bytes(guard_object.id().as_bytes()),
        bytes(current.state.state_token().as_bytes()),
        bytes(current.closure.id().as_bytes()),
    ]))?;
    let carrier_revision_ref = hash_reference(&CborValue::Array(vec![
        CborValue::text("maestro.vnext.protected-diagnostic-carrier-revision.v1")?,
        bytes(current_generation.id().as_bytes()),
        bytes(current_generation.contract_root_id().as_bytes()),
        CborValue::Unsigned(snapshot.subject_revision),
        bytes(current.manifest.id().as_bytes()),
    ]))?;
    let witness = LinearizationCoverageWitnessV1::new(
        requested_subject,
        LinearizationFenceCarrierV1::ProtectedSnapshot,
        ContinuityReferenceV1::from_digest(*state_object.id().as_bytes()),
        authentication_carrier_ref,
        ContinuityReferenceV1::from_digest(*current.state.state_token().as_bytes()),
        ContinuityReferenceV1::from_digest(*current.snapshot_object.id().as_bytes()),
        currentness_and_fence_ref,
        carrier_revision_ref,
    )?;
    let guard = ProtectedContinuityDiagnosticReadGuardV1 {
        _witness: witness,
        _current_view_anchor: &current_view_anchor,
    };
    let envelope_input = ProtectedContinuityDiagnosticEnvelopeInputV1::new(
        &guard,
        ContinuityReferenceV1::from_digest(*guard_object.id().as_bytes()),
        guard._witness.attempt_ref(),
        ContinuityReferenceV1::from_digest(*current_generation.id().as_bytes()),
        guard._witness.fence_subject_ref(),
        guard._witness.fence_carrier_ref(),
        guard._witness.semantic_point_ref(),
        guard._witness.covered_closure_ref(),
        guard._witness.conservative_point_envelope_ref(),
        guard._witness.carrier_revision_ref(),
        ContinuityReferenceV1::from_digest(current_view_anchor.commitment()),
        ContinuityReferenceV1::from_digest(authority_commitment),
        authentication_carrier_ref,
    )
    .ok_or(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot)?;
    let prepared: ProtectedContinuityDiagnosticPreparedCarrierV1 =
        prepare_current_protected_snapshot(&envelope_input, assembler_mode)
            .ok_or(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot)?;
    let final_attestation_commitment = attestation
        .final_recheck()
        .ok_or(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot)?;
    if final_attestation_commitment != verified_attestation_commitment {
        return Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot);
    }
    if view
        .consume_protected_diagnostic_current_view_anchor(current_view_anchor)
        .is_err()
    {
        return Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot);
    }
    Ok(prepared.release())
}

struct ProtectedDiagnosticPresentedHostFactsV1 {
    anchor_commitment: [u8; 32],
    authority_commitment: [u8; 32],
    protected_subject_commitment: [u8; 32],
    invocation_nonce: [u8; 32],
    challenge_commitment: [u8; 32],
    presented_attestation_commitment: [u8; 32],
    provider_identity: [u8; 32],
    profile_identity: [u8; 32],
    profile_revision: u64,
    process_incarnation: [u8; 32],
    connection_incarnation: [u8; 32],
    channel_incarnation: [u8; 32],
    issuer_identity: [u8; 32],
    realm_identity: [u8; 32],
    audience_identity: [u8; 32],
    authentication_event_identity: [u8; 32],
    host_currentness_revision: u64,
    revocation_revision: u64,
    freshness_identity: [u8; 32],
    carrier_commitment: [u8; 32],
    principal_identity: [u8; 32],
    binding_identity: [u8; 32],
    session_identity: [u8; 32],
    context_identity: [u8; 32],
    trust_root_revision: u64,
    assurance_revision: u64,
    human_capable: bool,
    binding_not_before: u64,
    binding_expires_at: u64,
    session_not_before: u64,
    session_expires_at: u64,
    store_generation: u64,
    authority_epoch: u64,
    domain_identity: [u8; 32],
    domain_role: u64,
    incarnation_revision: u64,
}

impl ProtectedDiagnosticPresentedHostFactsV1 {
    fn capture(presentation: &dyn TrustedHostDiagnosticPresentationPortV1) -> Self {
        Self {
            anchor_commitment: presentation.anchor_commitment(),
            authority_commitment: presentation.authority_commitment(),
            protected_subject_commitment: presentation.protected_subject_commitment(),
            invocation_nonce: presentation.invocation_nonce(),
            challenge_commitment: presentation.challenge_commitment(),
            presented_attestation_commitment: presentation.attestation_commitment(),
            provider_identity: presentation.provider_identity(),
            profile_identity: presentation.profile_identity(),
            profile_revision: presentation.profile_revision(),
            process_incarnation: presentation.process_incarnation(),
            connection_incarnation: presentation.connection_incarnation(),
            channel_incarnation: presentation.channel_incarnation(),
            issuer_identity: presentation.issuer_identity(),
            realm_identity: presentation.realm_identity(),
            audience_identity: presentation.audience_identity(),
            authentication_event_identity: presentation.authentication_event_identity(),
            host_currentness_revision: presentation.host_currentness_revision(),
            revocation_revision: presentation.revocation_revision(),
            freshness_identity: presentation.freshness_identity(),
            carrier_commitment: presentation.carrier_commitment(),
            principal_identity: presentation.principal_identity(),
            binding_identity: presentation.binding_identity(),
            session_identity: presentation.session_identity(),
            context_identity: presentation.context_identity(),
            trust_root_revision: presentation.trust_root_revision(),
            assurance_revision: presentation.assurance_revision(),
            human_capable: presentation.human_capable(),
            binding_not_before: presentation.binding_not_before(),
            binding_expires_at: presentation.binding_expires_at(),
            session_not_before: presentation.session_not_before(),
            session_expires_at: presentation.session_expires_at(),
            store_generation: presentation.store_generation(),
            authority_epoch: presentation.authority_epoch(),
            domain_identity: presentation.domain_identity(),
            domain_role: presentation.domain_role(),
            incarnation_revision: presentation.incarnation_revision(),
        }
    }

    fn attestation_commitment(&self) -> [u8; 32] {
        let profile_revision = self.profile_revision.to_be_bytes();
        let host_currentness_revision = self.host_currentness_revision.to_be_bytes();
        let revocation_revision = self.revocation_revision.to_be_bytes();
        let trust_root_revision = self.trust_root_revision.to_be_bytes();
        let assurance_revision = self.assurance_revision.to_be_bytes();
        let human_capable = [u8::from(self.human_capable)];
        let binding_not_before = self.binding_not_before.to_be_bytes();
        let binding_expires_at = self.binding_expires_at.to_be_bytes();
        let session_not_before = self.session_not_before.to_be_bytes();
        let session_expires_at = self.session_expires_at.to_be_bytes();
        let store_generation = self.store_generation.to_be_bytes();
        let authority_epoch = self.authority_epoch.to_be_bytes();
        let domain_role = self.domain_role.to_be_bytes();
        let incarnation_revision = self.incarnation_revision.to_be_bytes();
        protected_diagnostic_tuple_commitment(
            PROTECTED_DIAGNOSTIC_ATTESTATION_DOMAIN_V1,
            &[
                &self.challenge_commitment,
                &self.provider_identity,
                &self.profile_identity,
                &profile_revision,
                &self.process_incarnation,
                &self.connection_incarnation,
                &self.channel_incarnation,
                &self.issuer_identity,
                &self.realm_identity,
                &self.audience_identity,
                &self.authentication_event_identity,
                &host_currentness_revision,
                &revocation_revision,
                &self.freshness_identity,
                &self.carrier_commitment,
                &self.principal_identity,
                &self.binding_identity,
                &self.session_identity,
                &self.context_identity,
                &trust_root_revision,
                &assurance_revision,
                &human_capable,
                &binding_not_before,
                &binding_expires_at,
                &session_not_before,
                &session_expires_at,
                &store_generation,
                &authority_epoch,
                &self.domain_identity,
                &domain_role,
                &incarnation_revision,
            ],
        )
    }
}

fn protected_diagnostic_invocation_nonce(
    issuer: &ProtectedDiagnosticInvocationIssuerV1,
    anchor_commitment: [u8; 32],
    authority_commitment: [u8; 32],
    protected_subject_commitment: [u8; 32],
) -> Result<[u8; 32], AuthorityPublicationError> {
    let nonce = protected_diagnostic_nonce_derivation(
        issuer.process_incarnation,
        issuer.invocation_entropy,
        issuer.sequence,
        anchor_commitment,
        authority_commitment,
        protected_subject_commitment,
    )?;
    if nonce == [0; 32] {
        return Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot);
    }
    Ok(nonce)
}

fn protected_diagnostic_nonce_derivation(
    process_incarnation: [u8; 32],
    invocation_entropy: [u8; 32],
    sequence: u64,
    anchor_commitment: [u8; 32],
    authority_commitment: [u8; 32],
    protected_subject_commitment: [u8; 32],
) -> Result<[u8; 32], AuthorityPublicationError> {
    Ok(
        Sha256::digest(deterministic_cbor::encode(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.protected-diagnostic-invocation-nonce.v1")?,
            bytes(&process_incarnation),
            bytes(&invocation_entropy),
            CborValue::Unsigned(sequence),
            bytes(&anchor_commitment),
            bytes(&authority_commitment),
            bytes(&protected_subject_commitment),
        ]))?)
        .into(),
    )
}

fn protected_diagnostic_process_incarnation() -> Result<[u8; 32], AuthorityPublicationError> {
    static PROCESS_INCARNATION: OnceLock<[u8; 32]> = OnceLock::new();
    let candidate = protected_diagnostic_random_entropy(
        b"maestro.vnext.protected-diagnostic-process-incarnation.v1",
    );
    let incarnation = *PROCESS_INCARNATION.get_or_init(|| candidate);
    if incarnation == [0; 32] {
        return Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot);
    }
    Ok(incarnation)
}

fn protected_diagnostic_random_entropy(domain: &[u8]) -> [u8; 32] {
    let state = RandomState::new();
    let mut entropy = [0u8; 32];
    for lane in 0..4u64 {
        let mut hasher = state.build_hasher();
        hasher.write(domain);
        hasher.write_u64(lane);
        entropy[(lane as usize) * 8..(lane as usize + 1) * 8]
            .copy_from_slice(&hasher.finish().to_be_bytes());
    }
    entropy
}

fn protected_diagnostic_invocation_sequence() -> Result<u64, AuthorityPublicationError> {
    static NEXT_INVOCATION_SEQUENCE: AtomicU64 = AtomicU64::new(1);
    NEXT_INVOCATION_SEQUENCE
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |sequence| {
            sequence.checked_add(1)
        })
        .map_err(|_| AuthorityPublicationError::InvalidCurrentAuthoritySnapshot)
}

fn protected_diagnostic_tuple_commitment(domain: &[u8], fields: &[&[u8]]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update((domain.len() as u64).to_be_bytes());
    digest.update(domain);
    for field in fields {
        digest.update((field.len() as u64).to_be_bytes());
        digest.update(field);
    }
    digest.finalize().into()
}

fn protected_diagnostic_operator_is_current(
    facts: &BootstrapAuthoritySnapshotV1,
    binding: &super::PrincipalBindingV1,
    session: &super::SessionV1,
    current_generation: &StoreGenerationV1,
) -> Result<bool, AuthorityPublicationError> {
    let snapshot = facts.snapshot();
    let revocations = facts.revocations().revocations();
    Ok(binding.human_capable()
        && binding.context_id() == facts.context().context_id()
        && binding.trust_root_revision() == snapshot.trust_root_revision
        && session.binding_id() == binding.id()
        && session.context_id() == facts.context().context_id()
        && session.store_generation() == current_generation.ordinal()
        && session.authority_epoch() == snapshot.authority_epoch
        && !revocations.contains(super::RevocationTargetV1::TrustRoot(
            snapshot.trust_root_revision,
        ))
        && !revocations.contains(super::RevocationTargetV1::PrincipalBinding(binding.id()))
        && !revocations.contains(super::RevocationTargetV1::Session(session.id()))
        && snapshot.trusted_time.is_within(binding.validity())?
        && snapshot.trusted_time.is_within(session.validity())?)
}

fn protected_diagnostic_authority_commitment(
    view: &StorePublicationViewV1<'_>,
    current_head: &crate::domain::vnext::persistence::StoreHeadV1,
    current_generation: &StoreGenerationV1,
    current_root: StoreObjectIdV1,
    current: &CurrentAuthorityV1,
    state_object_id: StoreObjectIdV1,
    guard_object_id: StoreObjectIdV1,
) -> Result<[u8; 32], AuthorityPublicationError> {
    let facts_digest: [u8; 32] = Sha256::digest(current.facts.canonical_bytes()?).into();
    Ok(
        Sha256::digest(deterministic_cbor::encode(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.protected-diagnostic-authority-currentness.v1")?,
            bytes(view.domain().id().as_bytes()),
            CborValue::Unsigned(view.role().tag()),
            bytes(current_head.id().as_bytes()),
            CborValue::Unsigned(current_head.revision()),
            bytes(current_generation.id().as_bytes()),
            CborValue::Unsigned(current_generation.ordinal()),
            bytes(current_generation.contract_root_id().as_bytes()),
            bytes(current_root.as_bytes()),
            bytes(current.snapshot_object.id().as_bytes()),
            bytes(current.manifest.id().as_bytes()),
            bytes(current.closure.id().as_bytes()),
            bytes(current.state.state_token().as_bytes()),
            bytes(state_object_id.as_bytes()),
            bytes(guard_object_id.as_bytes()),
            bytes(current.facts.current_carrier_procedure_ref().as_bytes()),
            bytes(&facts_digest),
            CborValue::Unsigned(LinearizationFenceCarrierV1::ProtectedSnapshot as u64),
        ]))?)
        .into(),
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "the closed post-cut carrier must be checked against every coupled Store fact"
)]
fn validate_post_cut_current_authority(
    view: &StorePublicationViewV1<'_>,
    current_head: &crate::domain::vnext::persistence::StoreHeadV1,
    current_generation: &StoreGenerationV1,
    root: &StoreObjectV1,
    root_references: &[StoreObjectV1],
    snapshot_object: &StoreObjectV1,
    facts: &BootstrapAuthoritySnapshotV1,
    closure: &AuthorityContinuityClosureV1,
    state: &SuccessVisibleAuthorityContinuityStateV1,
    closure_object_id: StoreObjectIdV1,
    guard_object_id: StoreObjectIdV1,
    state_object_id: StoreObjectIdV1,
    post_cut: &AuthorityContinuityPostCutConsequenceSetV1,
) -> Result<(), AuthorityPublicationError> {
    let prior_generation_id = current_generation
        .previous()
        .ok_or(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot)?;
    let prior_generation = view.generation(prior_generation_id)?;
    let [prior_root_id] = prior_generation.roots() else {
        return Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot);
    };
    let prior_head_id = current_head
        .previous_head_id()
        .ok_or(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot)?;

    let context_object = one_schema_object(root_references, AuthoritySchemaV1::AuthorityContext)?;
    let consent_object = one_schema_object(
        root_references,
        AuthoritySchemaV1::ConsentSlotBindingParameter,
    )?;
    let basis_object = one_schema_object(root_references, AuthoritySchemaV1::ActionAuthorityBasis)?;
    let request_object = one_schema_object(
        root_references,
        AuthoritySchemaV1::IssueBootstrapMandateRequest,
    )?;
    let receipt_object =
        one_schema_object(root_references, AuthoritySchemaV1::AuthorizationReceipt)?;
    let result_object = one_schema_object(root_references, AuthoritySchemaV1::ActionResult)?;
    let mandate_object = one_schema_object(root_references, AuthoritySchemaV1::AuthorityMandate)?;
    let witness_object = one_schema_object(
        root_references,
        AuthoritySchemaV1::LinearizationCoverageWitness,
    )?;
    let binding_objects = schema_objects(
        root_references,
        AuthoritySchemaV1::BootstrapMandateIssuanceBinding,
    )?;

    let request_fields = exact_current_fields(&request_object, 14)?;
    let request_id = exact_current_digest(&request_fields[1])?;
    let request_context_id = exact_current_digest(&request_fields[2])?;
    let idempotency_key = exact_current_digest(&request_fields[12])?;
    let request_commitment = reference_from_bytes(&object_value_bytes(&request_object)?);

    let receipt_fields = exact_current_fields(&receipt_object, 7)?;
    let receipt_id = exact_current_digest(&receipt_fields[0])?;
    let receipt_context_id = exact_current_digest(&receipt_fields[1])?;
    let receipt_request_id = exact_current_digest(&receipt_fields[2])?;
    let receipt_basis_object_id = exact_current_digest(&receipt_fields[3])?;
    let receipt_result_id = exact_current_digest(&receipt_fields[6])?;

    let result_fields = exact_current_fields(&result_object, 11)?;
    let result_id = exact_current_digest(&result_fields[0])?;
    let result_request_id = exact_current_digest(&result_fields[1])?;
    let outcome = match result_fields[2] {
        CborValue::Unsigned(1) => ActionOutcomeV1::Committed,
        CborValue::Unsigned(2) => ActionOutcomeV1::NoOp,
        _ => return Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot),
    };
    let CborValue::Array(prior_state_tokens) = &result_fields[4] else {
        return Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot);
    };
    let CborValue::Array(resulting_state_tokens) = &result_fields[5] else {
        return Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot);
    };
    let CborValue::Array(receipt_ids) = &result_fields[6] else {
        return Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot);
    };
    let CborValue::Array(produced_ids) = &result_fields[7] else {
        return Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot);
    };
    if prior_state_tokens.len() != 1
        || resulting_state_tokens.len() != 1
        || receipt_ids.len() != 1
        || result_fields[3] != CborValue::Unsigned(1)
        || !(1..=2).contains(&produced_ids.len())
        || !matches!(&result_fields[8], CborValue::Array(values) if values.is_empty())
        || result_fields[9] != CborValue::optional(None)
        || !valid_optional_digest(&result_fields[10])
    {
        return Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot);
    }
    let prior_state_token =
        StateTokenIdV1::from_digest(exact_current_digest(&prior_state_tokens[0])?);
    let resulting_state_token =
        StateTokenIdV1::from_digest(exact_current_digest(&resulting_state_tokens[0])?);
    let result_receipt_id = exact_current_digest(&receipt_ids[0])?;
    let produced_mandate_id = exact_current_digest(&produced_ids[0])?;
    let mandate_id = Sha256::digest(object_value_bytes(&mandate_object)?).into();

    let binding_object = match (produced_ids.len(), binding_objects.as_slice()) {
        (1, []) => None,
        (2, [binding]) => {
            let binding_id: [u8; 32] = Sha256::digest(object_value_bytes(binding)?).into();
            if binding_id != exact_current_digest(&produced_ids[1])? {
                return Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot);
            }
            let binding_fields = exact_current_fields(binding, 5)?;
            if exact_current_digest(&binding_fields[1])? != mandate_id {
                return Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot);
            }
            Some(binding)
        }
        _ => return Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot),
    };

    let closure_ref = ContinuityReferenceV1::from_digest(*closure.id().as_bytes());
    let state_ref = ContinuityReferenceV1::from_digest(*state_object_id.as_bytes());
    let mandate_ref = ContinuityReferenceV1::from_digest(mandate_id);
    let receipt_ref = ContinuityReferenceV1::from_digest(receipt_id);
    let result_ref = ContinuityReferenceV1::from_digest(result_id);
    let reconstructed_receipt = AuthorizationReceiptV1::new(
        ActionRequestIdV1::from_digest(request_id),
        AuthorityContextIdV1::from_digest(request_context_id),
        ActionAuthorityBasisKindV1::BootstrapControlG0,
        prior_state_token,
        resulting_state_token,
    )?;
    let reconstructed_result = ActionResultV1::new(
        ActionRequestIdV1::from_digest(request_id),
        outcome,
        Some(reconstructed_receipt.clone()),
        None,
    )?;
    let mut expected_consumptions = facts
        .g0_candidate_paths()
        .iter()
        .map(|path| ContinuityReferenceV1::from_digest(*path.genesis_grant_id().as_bytes()))
        .collect::<Vec<_>>();
    expected_consumptions.sort_unstable();

    let mut idempotency_records = view.generation_idempotency(current_generation.id())?;
    if idempotency_records.len() != 1 {
        return Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot);
    }
    let idempotency = idempotency_records
        .pop()
        .expect("invariant: exact one-element check");
    let idempotency_ref = hash_reference(&CborValue::Array(vec![
        CborValue::text(idempotency.namespace())?,
        bytes(idempotency.key_digest()),
        bytes(idempotency.meaning_digest()),
        bytes(&result_id),
    ]))?;
    let context_current_ref = hash_reference(&CborValue::Array(vec![
        CborValue::text("maestro.vnext.context-current-continuity-relation.v1")?,
        bytes(facts.context().context_id().as_bytes()),
        bytes(closure.id().as_bytes()),
        bytes(state.state_token().as_bytes()),
    ]))?;

    let witness = LinearizationCoverageWitnessV1::decode(&object_value_bytes(&witness_object)?)?;
    if witness.fence_subject_ref() != request_commitment
        || witness.fence_carrier() != LinearizationFenceCarrierV1::SameStoreCommit
        || witness.fence_carrier_ref()
            != ContinuityReferenceV1::from_digest(*prior_head_id.as_bytes())
        || witness.attempt_ref() != ContinuityReferenceV1::from_digest(request_id)
        || witness.semantic_point_ref() != closure.store_allocation_commitment()
        || witness.covered_closure_ref() != closure_ref
        || witness.conservative_point_envelope_ref()
            != ContinuityReferenceV1::from_digest(*idempotency.meaning_digest())
        || witness.carrier_revision_ref()
            != ContinuityReferenceV1::from_digest(*prior_generation_id.as_bytes())
    {
        return Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot);
    }

    if post_cut.authority_continuity_closure_ref() != closure_ref
        || post_cut.closure_id() != closure.id()
        || post_cut.successor_state_token() != state.state_token()
        || post_cut.action_request_commitment() != request_commitment
        || post_cut.success_visible_continuity_state_ref() != state_ref
        || post_cut.selected_authority_consumption_refs() != expected_consumptions
        || post_cut.phase_owned_semantic_mutation_ref() != mandate_ref
        || post_cut.primary_authorization_receipt_ref() != receipt_ref
        || post_cut.action_result_ref() != result_ref
        || post_cut.active_idempotency_mapping_ref() != idempotency_ref
        || post_cut.linearization_coverage_witness_ref()
            != ContinuityReferenceV1::from_digest(*witness_object.id().as_bytes())
        || post_cut.context_current_continuity_relation_ref() != context_current_ref
        || request_context_id != *facts.context().context_id().as_bytes()
        || receipt_context_id != *facts.context().context_id().as_bytes()
        || receipt_request_id != request_id
        || receipt_fields[4] != CborValue::Unsigned(1)
        || receipt_fields[5] != CborValue::Bool(true)
        || result_request_id != request_id
        || result_receipt_id != receipt_id
        || receipt_result_id != result_id
        || receipt_basis_object_id != *basis_object.id().as_bytes()
        || reconstructed_receipt.id().as_bytes() != &receipt_id
        || reconstructed_result.id().as_bytes() != &result_id
        || closure.predecessor_state_token() != Some(prior_state_token)
        || resulting_state_token != state.state_token()
        || produced_mandate_id != mandate_id
        || idempotency.namespace() != ISSUE_BOOTSTRAP_MANDATE_IDEMPOTENCY_NAMESPACE_V1
        || idempotency.key_digest() != &idempotency_key
        || idempotency.result_object_id() != result_object.id()
        || idempotency.head_id() != current_head.id()
        || closure_object_id
            != one_schema_object(
                root_references,
                AuthoritySchemaV1::AuthorityContinuityClosure,
            )?
            .id()
        || guard_object_id
            != one_schema_object(root_references, AuthoritySchemaV1::AdmittedTransitionGuard)?.id()
        || state_object_id
            != one_schema_object(
                root_references,
                AuthoritySchemaV1::SuccessVisibleAuthorityContinuityState,
            )?
            .id()
    {
        return Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot);
    }

    require_exact_object_references(
        &request_object,
        &[context_object.id(), consent_object.id(), basis_object.id()],
    )?;
    require_exact_object_references(
        &receipt_object,
        &[context_object.id(), request_object.id(), basis_object.id()],
    )?;
    require_exact_object_references(&mandate_object, &[consent_object.id(), basis_object.id()])?;
    require_exact_object_references(&witness_object, &[closure_object_id])?;
    if let Some(binding) = binding_object {
        require_exact_object_references(
            binding,
            &[
                mandate_object.id(),
                request_object.id(),
                consent_object.id(),
            ],
        )?;
    }
    let mut result_references = vec![
        request_object.id(),
        receipt_object.id(),
        mandate_object.id(),
        closure_object_id,
        state_object_id,
    ];
    result_references.extend(binding_object.map(StoreObjectV1::id));
    require_exact_object_references(&result_object, &result_references)?;

    let mut expected_root_references = vec![
        *prior_root_id,
        snapshot_object.id(),
        context_object.id(),
        consent_object.id(),
        basis_object.id(),
        request_object.id(),
        receipt_object.id(),
        result_object.id(),
        mandate_object.id(),
        closure_object_id,
        guard_object_id,
        state_object_id,
        witness_object.id(),
    ];
    expected_root_references.extend(binding_object.map(StoreObjectV1::id));
    require_exact_object_references(root, &expected_root_references)
}

fn exact_current_fields(
    object: &StoreObjectV1,
    expected_len: usize,
) -> Result<&[CborValue], AuthorityPublicationError> {
    match object.value() {
        CborValue::Array(fields) if fields.len() == expected_len => Ok(fields),
        _ => Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot),
    }
}

fn exact_current_digest(value: &CborValue) -> Result<[u8; 32], AuthorityPublicationError> {
    match value {
        CborValue::Bytes(bytes) => bytes
            .as_slice()
            .try_into()
            .map_err(|_| AuthorityPublicationError::InvalidCurrentAuthoritySnapshot),
        _ => Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot),
    }
}

fn require_exact_object_references(
    object: &StoreObjectV1,
    expected: &[StoreObjectIdV1],
) -> Result<(), AuthorityPublicationError> {
    let mut expected = expected.to_vec();
    expected.sort_unstable();
    expected.dedup();
    if object.references() == expected {
        Ok(())
    } else {
        Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot)
    }
}

fn build_successor_continuity(
    current: &CurrentAuthorityV1,
    active_objects: &[StoreObjectV1],
    current_head: &crate::domain::vnext::persistence::StoreHeadV1,
    current_generation: &StoreGenerationV1,
    allocation: &StoreAllocatedContinuityStateTokenV1,
    meaning_digest: [u8; 32],
) -> Result<SuccessorContinuityV1, AuthorityPublicationError> {
    let accepted_time = AcceptedAuthorityTimeFloorV1::continue_from(
        current.state.accepted_time(),
        current.state.accepted_time().stable_lineage(),
        current.state.accepted_time().coordinate(),
        current.state.accepted_time().policy_stack(),
        HTimeCarryBasisV1::ExactNoLineageChange,
        HTimeContinuationContributionV1::CarryOnly,
    )?;
    let mut canonical_records = active_objects
        .iter()
        .map(reference_from_object)
        .collect::<Vec<_>>();
    canonical_records.sort_unstable();
    canonical_records.dedup();
    let cut_sequence = current
        .state
        .cut_sequence()
        .checked_add(1)
        .ok_or(AuthorityPublicationError::ContinuityOverflow)?;
    let semantic_cut = AuthorityContinuitySemanticCutV1 {
        cut_sequence,
        source_store_generation: current_generation.ordinal(),
        successor_store_generation: allocation.store_generation(),
        authority_epoch: current.state.authority_epoch(),
        stable_lineage: current.state.accepted_time().stable_lineage(),
        selected_trusted_time_stack: current.state.selected_trusted_time_stack(),
        carrier_profile: ContinuityCarrierProfileStatusV1::Confirmed {
            profile: current.state.carrier_profile(),
            accepted_prefix: current.state.accepted_external_prefix(),
            handoff_state: current.state.carrier_handoff_state(),
            fence: current.state.carrier_fence(),
            currentness: current.state.carrier_currentness(),
        },
        accepted_time,
        lane_state_closure_root: current.state.lane_state_closure_root(),
        source_floor_root: current.state.source_floor_root(),
        gap_companions: current.state.gap_companions().to_vec(),
        floor_provenance: current.state.floor_provenance().to_vec(),
        external_revision_cells: current.state.external_revision_cells().to_vec(),
        cma_remaining_root: current.state.cma_remaining_root(),
        cma_spent_root: current.state.cma_spent_root(),
        canonical_records,
        graph_nodes: current.closure.graph_nodes().to_vec(),
        replay_items: current.closure.replay_items().to_vec(),
        historical_spend_items: current.closure.historical_spend_items().to_vec(),
        unresolved_effects: current.state.unresolved_effects().to_vec(),
    };
    let governance_schema = AuthoritySchemaV1::RepositoryGovernanceFloorSnapshot.id()?;
    let mut governance_records = active_objects
        .iter()
        .filter(|object| object.schema_id() == governance_schema)
        .map(reference_from_object)
        .collect::<Vec<_>>();
    governance_records.sort_unstable();
    governance_records.dedup();
    let class_entries =
        continuity_class_entries(&current.manifest, &semantic_cut, &governance_records)?;
    let closure = AuthorityContinuityClosureV1::prove(
        &current.manifest,
        AuthorityContinuityClosureInputV1 {
            manifest_id: current.manifest.id(),
            context_kind: current.manifest.context_kind(),
            context_id: current.state.context_id(),
            predecessor: AuthorityContinuityPredecessorV1::PriorClosure {
                closure_id: current.closure.id(),
                state_token: current.state.state_token(),
            },
            semantic_cut,
            class_entries,
            graph_edges: current.closure.graph_edges().to_vec(),
            protocol_version: 1,
        },
        allocation,
    )?;
    let GuardAdmissionKindV1::Established(kind) = current.state.guard_kind() else {
        return Err(AuthorityPublicationError::UnestablishedContinuityGuard);
    };
    let object_set = CborValue::Array(
        active_objects
            .iter()
            .map(|object| bytes(object.id().as_bytes()))
            .collect(),
    );
    let source_cut_commitment = hash_reference(&CborValue::Array(vec![
        CborValue::text("maestro.vnext.transition-guard-owner-source-cut.v1")?,
        bytes(current_head.id().as_bytes()),
        bytes(current_generation.id().as_bytes()),
        bytes(&meaning_digest),
        object_set.clone(),
    ]))?;
    let term_facts = kind
        .term_bundle()
        .terms()
        .iter()
        .copied()
        .map(|term| {
            let owner_fact = hash_reference(&CborValue::Array(vec![
                CborValue::text("maestro.vnext.transition-guard-owner-fact.v1")?,
                CborValue::Unsigned(term as u64),
                object_set.clone(),
            ]))?;
            let owner_revision = hash_reference(&CborValue::Array(vec![
                CborValue::text("maestro.vnext.transition-guard-owner-revision.v1")?,
                CborValue::Unsigned(term as u64),
                bytes(current_head.id().as_bytes()),
                bytes(current_generation.id().as_bytes()),
                bytes(current_generation.contract_root_id().as_bytes()),
            ]))?;
            Ok(TransitionGuardTermFactV1::owner_confirmed(
                term,
                owner_fact,
                owner_revision,
            )?)
        })
        .collect::<Result<Vec<_>, AuthorityPublicationError>>()?;
    let owner_census = TransitionGuardOwnerCensusV1::from_owner_sources(
        kind,
        current.state.context_id(),
        closure.store_generation(),
        closure.authority_epoch(),
        source_cut_commitment,
        term_facts.clone(),
    )?;
    let guard = AdmittedTransitionGuardV1::evaluate(AuthorityTransitionGuardAdmissionInputV1 {
        kind: GuardAdmissionKindV1::Established(kind),
        context_kind: closure.context_kind(),
        context_id: closure.context_id(),
        store_generation: closure.store_generation(),
        authority_epoch: closure.authority_epoch(),
        manifest_id: current.manifest.id(),
        closure_id: closure.id(),
        predecessor_state_token: Some(current.state.state_token()),
        cut_sequence: closure.cut_sequence(),
        selected_trusted_time_stack: closure.selected_trusted_time_stack(),
        carrier_profile: closure.carrier_profile().clone(),
        accepted_time: closure.accepted_time().clone(),
        lane_state_closure_root: closure.lane_state_closure_root(),
        source_floor_root: closure.source_floor_root(),
        gap_companions: closure.gap_companions().to_vec(),
        floor_provenance: closure.floor_provenance().to_vec(),
        external_revision_cells: closure.external_revision_cells().to_vec(),
        cma_remaining_root: closure.cma_remaining_root(),
        cma_spent_root: closure.cma_spent_root(),
        unresolved_effects: closure.unresolved_effects().to_vec(),
        term_facts,
        owner_census,
        disclosure: ContinuityDisclosureV1::ProtectedComplete,
        protocol_version: 1,
    })?;
    let state = SuccessVisibleAuthorityContinuityStateV1::construct(
        &current.manifest,
        &closure,
        &guard,
        Some(&current.state),
    )?;
    let closure_object = authority_object(
        AuthoritySchemaV1::AuthorityContinuityClosure,
        closure.schema_value()?,
        vec![],
    )?;
    let guard_object = authority_object(
        AuthoritySchemaV1::AdmittedTransitionGuard,
        guard.schema_value()?,
        vec![closure_object.id()],
    )?;
    let state_object = authority_object(
        AuthoritySchemaV1::SuccessVisibleAuthorityContinuityState,
        state.schema_value()?,
        vec![closure_object.id(), guard_object.id()],
    )?;
    Ok(SuccessorContinuityV1 {
        closure,
        state,
        closure_object,
        guard_object,
        state_object,
    })
}

fn continuity_class_entries(
    manifest: &AuthorityContinuityManifestV1,
    cut: &AuthorityContinuitySemanticCutV1,
    governance_records: &[ContinuityReferenceV1],
) -> Result<Vec<AuthorityContinuityClassClosureV1>, AuthorityPublicationError> {
    let first_canonical = manifest
        .descriptors()
        .iter()
        .find(|descriptor| descriptor.disposition == ClassDispositionV1::CanonicalRecordClosure)
        .map(|descriptor| descriptor.class_id)
        .ok_or(AuthorityPublicationError::InvalidContinuityClassClosure)?;
    manifest
        .descriptors()
        .iter()
        .map(|descriptor| {
            let facets = ContinuityClosureFacetV1::ALL
                .into_iter()
                .map(|facet| {
                    let disposition = match descriptor.disposition {
                        ClassDispositionV1::CanonicalRecordClosure => {
                            let repository_governance_head = matches!(
                                descriptor.class_id,
                                super::ContinuityClassIdV1::Repository(
                                    super::RepositoryAuthorityContinuityClassV1::RepositoryGovernanceHead
                                )
                            );
                            let items = if repository_governance_head {
                                match facet {
                                    ContinuityClosureFacetV1::CanonicalRecords => {
                                        governance_records.to_vec()
                                    }
                                    _ => Vec::new(),
                                }
                            } else if descriptor.class_id == first_canonical {
                                match facet {
                                    ContinuityClosureFacetV1::CanonicalRecords => {
                                        cut.canonical_records
                                            .iter()
                                            .copied()
                                            .filter(|item| !governance_records.contains(item))
                                            .collect()
                                    }
                                    ContinuityClosureFacetV1::Graph => cut.graph_nodes.clone(),
                                    ContinuityClosureFacetV1::Replay => cut.replay_items.clone(),
                                    ContinuityClosureFacetV1::HistoricalSpend => {
                                        cut.historical_spend_items.clone()
                                    }
                                    ContinuityClosureFacetV1::UnresolvedEffect => {
                                        cut.unresolved_effects.clone()
                                    }
                                }
                            } else {
                                Vec::new()
                            };
                            ClosureFacetDispositionKindV1::ContributesExactRoot(
                                ContinuityExactRootV1::new(
                                    descriptor.class_id,
                                    facet,
                                    cut.cut_sequence,
                                    items,
                                )?,
                            )
                        }
                        ClassDispositionV1::DerivedOnly => {
                            ClosureFacetDispositionKindV1::DerivedCheck {
                                invariant: class_facet_reference(
                                    "invariant",
                                    descriptor.class_id,
                                    facet,
                                    cut.cut_sequence,
                                )?,
                                proof: class_facet_reference(
                                    "proof",
                                    descriptor.class_id,
                                    facet,
                                    cut.cut_sequence,
                                )?,
                            }
                        }
                    };
                    Ok(AuthorityContinuityFacetDispositionV1 { facet, disposition })
                })
                .collect::<Result<Vec<_>, AuthorityPublicationError>>()?;
            Ok(AuthorityContinuityClassClosureV1 {
                class_id: descriptor.class_id,
                owner: descriptor.owner,
                facets,
            })
        })
        .collect()
}

fn class_facet_reference(
    purpose: &str,
    class_id: super::ContinuityClassIdV1,
    facet: ContinuityClosureFacetV1,
    cut_sequence: u64,
) -> Result<ContinuityReferenceV1, AuthorityPublicationError> {
    hash_reference(&CborValue::Array(vec![
        CborValue::text("maestro.vnext.continuity-class-facet-proof.v1")?,
        CborValue::text(purpose)?,
        class_id.schema_value(),
        CborValue::Unsigned(facet as u64),
        CborValue::Unsigned(cut_sequence),
    ]))
}

fn bind_store_allocation(
    context_id: AuthorityContextIdV1,
    allocation: StorePublicationAllocationV1,
) -> Result<StoreAllocatedContinuityStateTokenV1, AuthorityPublicationError> {
    Ok(
        StoreAllocatedContinuityStateTokenV1::from_store_commitments(
            context_id,
            allocation.store_generation(),
            allocation
                .expected_predecessor()
                .map(StateTokenIdV1::from_digest),
            allocation.publication_clock(),
            allocation.token_commitment(),
            allocation.allocation_commitment(),
        )?,
    )
}

fn ensure_context_role(
    role: StoreRoleV1,
    context: AuthorityContextKindV1,
) -> Result<(), AuthorityPublicationError> {
    if matches!(
        (role, context),
        (
            StoreRoleV1::Repository,
            AuthorityContextKindV1::RepositoryAuthorityContext
        ) | (
            StoreRoleV1::Installation,
            AuthorityContextKindV1::InstallationAuthorityContext
        )
    ) {
        Ok(())
    } else {
        Err(AuthorityPublicationError::ContextRoleMismatch)
    }
}

fn one_schema_object(
    objects: &[StoreObjectV1],
    schema: AuthoritySchemaV1,
) -> Result<StoreObjectV1, AuthorityPublicationError> {
    let mut matches = schema_objects(objects, schema)?;
    if matches.len() != 1 {
        return Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot);
    }
    Ok(matches.pop().expect("invariant: exact one-element check"))
}

fn direct_reference_objects(
    owner: &StoreObjectV1,
    objects: &[StoreObjectV1],
) -> Result<Vec<StoreObjectV1>, AuthorityPublicationError> {
    owner
        .references()
        .iter()
        .map(|reference| {
            let mut matches = objects
                .iter()
                .filter(|object| object.id() == *reference)
                .cloned()
                .collect::<Vec<_>>();
            if matches.len() != 1 {
                return Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot);
            }
            Ok(matches.pop().expect("invariant: exact one-element check"))
        })
        .collect()
}

fn schema_objects(
    objects: &[StoreObjectV1],
    schema: AuthoritySchemaV1,
) -> Result<Vec<StoreObjectV1>, AuthorityPublicationError> {
    let schema_id = schema.id()?;
    Ok(objects
        .iter()
        .filter(|object| object.schema_id() == schema_id)
        .cloned()
        .collect())
}

fn find_exact_object(
    objects: &[StoreObjectV1],
    schema: AuthoritySchemaV1,
    exact_value_bytes: &[u8],
) -> Result<StoreObjectV1, AuthorityPublicationError> {
    let mut matches = schema_objects(objects, schema)?
        .into_iter()
        .filter(|object| object_value_bytes(object).is_ok_and(|bytes| bytes == exact_value_bytes))
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(AuthorityPublicationError::InvalidCurrentAuthoritySnapshot);
    }
    Ok(matches.pop().expect("invariant: exact one-element check"))
}

fn validate_mandate_issuance_cardinality(
    active_objects: &[StoreObjectV1],
    expected_mandate: &StoreObjectV1,
    expected_binding_value: &[u8],
) -> Result<bool, AuthorityPublicationError> {
    let mandate_matches = active_objects
        .iter()
        .filter(|object| object.id() == expected_mandate.id())
        .collect::<Vec<_>>();
    if mandate_matches.len() > 1
        || mandate_matches
            .first()
            .is_some_and(|object| **object != *expected_mandate)
    {
        return Err(AuthorityPublicationError::StoredMandateMismatch);
    }

    let mandate_id: [u8; 32] = Sha256::digest(object_value_bytes(expected_mandate)?).into();
    let mut associated_bindings = Vec::new();
    for binding in schema_objects(
        active_objects,
        AuthoritySchemaV1::BootstrapMandateIssuanceBinding,
    )? {
        let fields = exact_current_fields(&binding, 5)
            .map_err(|_| AuthorityPublicationError::StoredMandateBindingMismatch)?;
        if binding.references().contains(&expected_mandate.id())
            || exact_current_digest(&fields[1])
                .map_err(|_| AuthorityPublicationError::StoredMandateBindingMismatch)?
                == mandate_id
        {
            associated_bindings.push(binding);
        }
    }

    match (mandate_matches.is_empty(), associated_bindings.as_slice()) {
        (true, []) => Ok(true),
        (false, [binding]) => {
            if object_value_bytes(binding)? != expected_binding_value {
                return Err(AuthorityPublicationError::StoredMandateBindingMismatch);
            }
            let references = direct_reference_objects(binding, active_objects)
                .map_err(|_| AuthorityPublicationError::StoredMandateBindingMismatch)?;
            let mandate_references = direct_reference_objects(expected_mandate, active_objects)
                .map_err(|_| AuthorityPublicationError::StoredMandateBindingMismatch)?;
            let consent = one_schema_object(
                &mandate_references,
                AuthoritySchemaV1::ConsentSlotBindingParameter,
            )
            .map_err(|_| AuthorityPublicationError::StoredMandateBindingMismatch)?;
            if references.len() != 3
                || schema_objects(&references, AuthoritySchemaV1::AuthorityMandate)?.len() != 1
                || schema_objects(&references, AuthoritySchemaV1::IssueBootstrapMandateRequest)?
                    .len()
                    != 1
                || schema_objects(&references, AuthoritySchemaV1::ConsentSlotBindingParameter)?
                    .len()
                    != 1
                || !binding.references().contains(&expected_mandate.id())
                || !binding.references().contains(&consent.id())
            {
                return Err(AuthorityPublicationError::StoredMandateBindingMismatch);
            }
            Ok(false)
        }
        _ => Err(AuthorityPublicationError::StoredMandateBindingMismatch),
    }
}

fn object_value_bytes(object: &StoreObjectV1) -> Result<Vec<u8>, AuthorityPublicationError> {
    Ok(deterministic_cbor::encode(object.value())?)
}

fn reference_from_object(object: &StoreObjectV1) -> ContinuityReferenceV1 {
    ContinuityReferenceV1::from_digest(*object.id().as_bytes())
}

fn reference_from_bytes(value: &[u8]) -> ContinuityReferenceV1 {
    ContinuityReferenceV1::from_digest(Sha256::digest(value).into())
}

fn hash_reference(value: &CborValue) -> Result<ContinuityReferenceV1, AuthorityPublicationError> {
    Ok(reference_from_bytes(&deterministic_cbor::encode(value)?))
}

fn authority_object(
    schema: AuthoritySchemaV1,
    value: CborValue,
    mut references: Vec<StoreObjectIdV1>,
) -> Result<StoreObjectV1, AuthorityPublicationError> {
    if !schema.accepts_value(&value) {
        return Err(AuthorityPublicationError::InvalidSchemaCarrier);
    }
    references.sort_unstable();
    references.dedup();
    Ok(StoreObjectV1::new(schema.id()?, value, references)?)
}

fn publication_outcome(
    store: &StoreV1,
    outcome: StorePublicationOutcomeV1,
) -> Result<AuthorityPublicationOutcomeV1, AuthorityPublicationError> {
    let (kind, head, result) = match outcome {
        StorePublicationOutcomeV1::Committed { head, result } => {
            (AuthorityPublicationKindV1::Committed, head, result)
        }
        StorePublicationOutcomeV1::Replayed { head, result } => {
            (AuthorityPublicationKindV1::Replayed, head, result)
        }
    };
    if result.schema_id() != AuthoritySchemaV1::ActionResult.id()? {
        return Err(AuthorityPublicationError::InvalidPublishedResult);
    }
    let CborValue::Array(fields) = result.value() else {
        return Err(AuthorityPublicationError::InvalidPublishedResult);
    };
    let [
        logical_result_id,
        request_id,
        CborValue::Unsigned(outcome_tag),
        CborValue::Unsigned(receipt_count),
        CborValue::Array(prior_tokens),
        CborValue::Array(resulting_tokens),
        CborValue::Array(receipt_ids),
        CborValue::Array(produced_ids),
        CborValue::Array(effect_ids),
        error_detail,
        next_or_inspect,
    ] = fields.as_slice()
    else {
        return Err(AuthorityPublicationError::InvalidPublishedResult);
    };
    let logical_result_id = exact_digest(logical_result_id)?;
    let request_id = ActionRequestIdV1::from_digest(exact_digest(request_id)?);
    let outcome_value = ActionOutcomeV1::try_from(
        u8::try_from(*outcome_tag)
            .map_err(|_| AuthorityPublicationError::InvalidPublishedResult)?,
    )
    .map_err(|_| AuthorityPublicationError::InvalidPublishedResult)?;
    if *receipt_count != 1
        || receipt_ids.len() != 1
        || prior_tokens.len() != 1
        || resulting_tokens.len() != 1
        || !(1..=2).contains(&produced_ids.len())
        || !effect_ids.is_empty()
        || *error_detail != CborValue::optional(None)
        || !valid_optional_digest(next_or_inspect)
    {
        return Err(AuthorityPublicationError::InvalidPublishedResult);
    }
    let prior_state_token = StateTokenIdV1::from_digest(exact_digest(&prior_tokens[0])?);
    let resulting_state_token = StateTokenIdV1::from_digest(exact_digest(&resulting_tokens[0])?);
    let logical_receipt_id = exact_digest(&receipt_ids[0])?;
    let request_object = one_referenced_object(
        store,
        &result,
        AuthoritySchemaV1::IssueBootstrapMandateRequest,
    )?;
    let receipt_object =
        one_referenced_object(store, &result, AuthoritySchemaV1::AuthorizationReceipt)?;
    let mandate_object =
        one_referenced_object(store, &result, AuthoritySchemaV1::AuthorityMandate)?;
    validate_request_object(&request_object, request_id)?;
    let receipt = validate_receipt_object(
        &receipt_object,
        request_id,
        prior_state_token,
        resulting_state_token,
        logical_result_id,
    )?;
    if receipt.id().as_bytes() != &logical_receipt_id
        || exact_digest(&produced_ids[0])? != digest_value(mandate_object.value())?
    {
        return Err(AuthorityPublicationError::InvalidPublishedResult);
    }
    if produced_ids.len() == 2 {
        let binding = one_referenced_object(
            store,
            &result,
            AuthoritySchemaV1::BootstrapMandateIssuanceBinding,
        )?;
        if exact_digest(&produced_ids[1])? != digest_value(binding.value())? {
            return Err(AuthorityPublicationError::InvalidPublishedResult);
        }
    } else if !referenced_objects_by_schema(
        store,
        &result,
        AuthoritySchemaV1::BootstrapMandateIssuanceBinding,
    )?
    .is_empty()
    {
        return Err(AuthorityPublicationError::InvalidPublishedResult);
    }
    let reconstructed = ActionResultV1::new(request_id, outcome_value, Some(receipt), None)?;
    if reconstructed.id().as_bytes() != &logical_result_id {
        return Err(AuthorityPublicationError::InvalidPublishedResult);
    }
    let generation = store.publication_generation(head.id())?;
    let [post_cut_id] = generation.roots() else {
        return Err(AuthorityPublicationError::InvalidPublishedResult);
    };
    let post_cut_object = store.read_object(*post_cut_id)?;
    if post_cut_object.schema_id()
        != AuthoritySchemaV1::AuthorityContinuityPostCutConsequenceSet.id()?
        || !post_cut_object.references().contains(&result.id())
    {
        return Err(AuthorityPublicationError::InvalidPublishedResult);
    }
    let post_cut =
        AuthorityContinuityPostCutConsequenceSetV1::decode(&object_value_bytes(&post_cut_object)?)?;
    if post_cut.successor_state_token() != resulting_state_token
        || post_cut.action_result_ref() != ContinuityReferenceV1::from_digest(logical_result_id)
    {
        return Err(AuthorityPublicationError::InvalidPublishedResult);
    }
    Ok(AuthorityPublicationOutcomeV1 {
        kind,
        head,
        result,
        logical_result_id: ActionResultIdV1::from_digest(logical_result_id),
    })
}

fn validate_request_object(
    object: &StoreObjectV1,
    request_id: ActionRequestIdV1,
) -> Result<(), AuthorityPublicationError> {
    let CborValue::Array(fields) = object.value() else {
        return Err(AuthorityPublicationError::InvalidPublishedResult);
    };
    if fields.len() != 14 || exact_digest(&fields[1])? != *request_id.as_bytes() {
        return Err(AuthorityPublicationError::InvalidPublishedResult);
    }
    Ok(())
}

fn validate_receipt_object(
    object: &StoreObjectV1,
    request_id: ActionRequestIdV1,
    prior_state_token: StateTokenIdV1,
    resulting_state_token: StateTokenIdV1,
    logical_result_id: [u8; 32],
) -> Result<AuthorizationReceiptV1, AuthorityPublicationError> {
    let CborValue::Array(fields) = object.value() else {
        return Err(AuthorityPublicationError::InvalidPublishedResult);
    };
    let [
        receipt_id,
        context_id,
        receipt_request_id,
        _basis_object_id,
        CborValue::Unsigned(1),
        CborValue::Bool(true),
        result_id,
    ] = fields.as_slice()
    else {
        return Err(AuthorityPublicationError::InvalidPublishedResult);
    };
    if exact_digest(receipt_request_id)? != *request_id.as_bytes()
        || exact_digest(result_id)? != logical_result_id
    {
        return Err(AuthorityPublicationError::InvalidPublishedResult);
    }
    let receipt = AuthorizationReceiptV1::new(
        request_id,
        AuthorityContextIdV1::from_digest(exact_digest(context_id)?),
        ActionAuthorityBasisKindV1::BootstrapControlG0,
        prior_state_token,
        resulting_state_token,
    )?;
    if receipt.id().as_bytes() != &exact_digest(receipt_id)? {
        return Err(AuthorityPublicationError::InvalidPublishedResult);
    }
    Ok(receipt)
}

fn one_referenced_object(
    store: &StoreV1,
    result: &StoreObjectV1,
    schema: AuthoritySchemaV1,
) -> Result<StoreObjectV1, AuthorityPublicationError> {
    let mut matches = referenced_objects_by_schema(store, result, schema)?;
    if matches.len() != 1 {
        return Err(AuthorityPublicationError::InvalidPublishedResult);
    }
    Ok(matches.pop().expect("invariant: exact one-element check"))
}

fn referenced_objects_by_schema(
    store: &StoreV1,
    result: &StoreObjectV1,
    schema: AuthoritySchemaV1,
) -> Result<Vec<StoreObjectV1>, AuthorityPublicationError> {
    let schema_id = schema.id()?;
    result
        .references()
        .iter()
        .map(|object_id| store.read_object(*object_id))
        .filter_map(|object| match object {
            Ok(object) if object.schema_id() == schema_id => Some(Ok(object)),
            Ok(_) => None,
            Err(error) => Some(Err(error.into())),
        })
        .collect()
}

fn digest_value(value: &CborValue) -> Result<[u8; 32], AuthorityPublicationError> {
    Ok(Sha256::digest(deterministic_cbor::encode(value)?).into())
}

fn exact_digest(value: &CborValue) -> Result<[u8; 32], AuthorityPublicationError> {
    let CborValue::Bytes(bytes) = value else {
        return Err(AuthorityPublicationError::InvalidPublishedResult);
    };
    bytes
        .as_slice()
        .try_into()
        .map_err(|_| AuthorityPublicationError::InvalidPublishedResult)
}

fn valid_optional_digest(value: &CborValue) -> bool {
    match value {
        CborValue::Array(fields) if fields == &[CborValue::Unsigned(0)] => true,
        CborValue::Array(fields) if matches!(fields.as_slice(), [CborValue::Unsigned(1), CborValue::Bytes(bytes)] if bytes.len() == 32) => {
            true
        }
        _ => false,
    }
}

fn bytes(value: &[u8]) -> CborValue {
    CborValue::Bytes(value.to_vec())
}

#[derive(Debug, Error)]
pub enum AuthorityPublicationError {
    #[error("Authority mutation is unavailable while the Store is inactive")]
    InactiveStore,
    #[error("Authority Context kind does not match the Store role")]
    ContextRoleMismatch,
    #[error("Authority mutation requires one exact current predecessor Authority root")]
    MissingAuthorityPredecessor,
    #[error("Authority mutation does not extend the exact current Authority lineage")]
    AuthorityPredecessorMismatch,
    #[error("an existing semantic Mandate does not match its canonical Store Object")]
    StoredMandateMismatch,
    #[error("semantic Mandate convergence requires exactly one canonical issuance binding")]
    StoredMandateBindingMismatch,
    #[error("stored Authority Action Result is not the exact Stage 2 carrier")]
    InvalidPublishedResult,
    #[error("Authority Store Object does not match its exact Stage 2 schema carrier")]
    InvalidSchemaCarrier,
    #[error("the current Store does not contain one exact current Authority snapshot closure")]
    InvalidCurrentAuthoritySnapshot,
    #[error("the current Authority continuity guard is not established")]
    UnestablishedContinuityGuard,
    #[error("root-attached Grant issuance requires one exact current complete G0 parent")]
    InvalidBootstrapGrantAuthority,
    #[error("the Grant BoundedBy root is not one exact already-established capacity root")]
    UnestablishedCapacityRoot,
    #[error(
        "ordinary Grant administration requires a separate current RepositoryAuthorityAdministration Grant"
    )]
    InvalidGrantAdministrationAuthority,
    #[error("one-to-one Grant reissue must preserve its root and cannot widen authority")]
    GrantReissueWidening,
    #[error("the last live Repository Authority administrator requires a surviving handoff")]
    LastAdministrator,
    #[error("the Authority continuity counter overflowed")]
    ContinuityOverflow,
    #[error("the frozen continuity class closure has no canonical-record owner")]
    InvalidContinuityClassClosure,
    #[error(transparent)]
    Identity(#[from] crate::domain::vnext::identity::IdentityError),
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error(transparent)]
    StoreObject(#[from] StoreObjectError),
    #[error(transparent)]
    Generation(#[from] GenerationError),
    #[error(transparent)]
    AtomicPublication(#[from] AtomicPublicationError),
    #[error(transparent)]
    AuthorityEvaluation(#[from] IssueBootstrapMandateError),
    #[error(transparent)]
    Evaluator(#[from] AuthorityEvaluationErrorV1),
    #[error(transparent)]
    BootstrapSnapshot(#[from] BootstrapAuthoritySnapshotErrorV1),
    #[error(transparent)]
    AuthorityValidation(#[from] super::AuthorityValidationError),
    #[error(transparent)]
    Capacity(#[from] super::CapacityError),
    #[error(transparent)]
    ContinuityManifest(#[from] AuthorityContinuityError),
    #[error(transparent)]
    ContinuityClosure(#[from] AuthorityContinuityClosureError),
    #[error(transparent)]
    ContinuityState(#[from] AuthorityContinuityStateError),
    #[error(transparent)]
    ContinuityAllocation(#[from] StoreAllocationBindingErrorV1),
    #[error(transparent)]
    TrustedTime(#[from] HTimeAcceptanceErrorV1),
    #[error(transparent)]
    PostCut(#[from] AuthorityPostCutErrorV1),
    #[error(transparent)]
    ActionResult(#[from] ActionResultError),
    #[error(transparent)]
    Store(#[from] crate::domain::vnext::persistence::StoreError),
}

#[cfg(test)]
#[path = "facade_tests.rs"]
mod tests;
