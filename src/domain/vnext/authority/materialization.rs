use std::cell::Cell;
use std::marker::PhantomData;
use std::rc::Rc;

use thiserror::Error;

#[cfg(test)]
use super::GrantIdV1;
use super::facade::construct_owner_local_repository_authority_input;
use super::{
    ActionRequestIdV1, AuthorityContextIdV1, AuthorizationReceiptIdV1, IdempotencyKeyIdV1,
    MandateIdV1, PrincipalBindingIdV1, PrincipalIdV1, RepositoryActionLeafV1,
    RepositoryAuthoritySelectionV1, RepositoryDownstreamActionLeafV1, SessionIdV1, StateTokenIdV1,
};
use crate::domain::vnext::identity::StoreGenerationIdV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::domain::vnext) enum SchedulingPolicyDiffClassV1 {
    Equivalent,
    Strengthening,
    Weakening,
    Incomparable,
}

impl SchedulingPolicyDiffClassV1 {
    const fn requires_downgrade_mandate(self) -> bool {
        matches!(self, Self::Weakening | Self::Incomparable)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::domain::vnext) struct SchedulingPolicyDowngradeMandateFactsV1 {
    mandate_id: MandateIdV1,
    repository_generation_id: StoreGenerationIdV1,
    principal_id: PrincipalIdV1,
    human_binding_id: PrincipalBindingIdV1,
    human_session_id: SessionIdV1,
    authority_context_id: AuthorityContextIdV1,
    state_token_id: StateTokenIdV1,
    mandate_schema_version: u64,
    authority_epoch: u64,
    valid_from: u64,
    valid_until: u64,
    trusted_time: u64,
    revocation_revision: u64,
    diff_class: SchedulingPolicyDiffClassV1,
    mandate_body_commitment: [u8; 32],
    mandate_carrier_commitment: [u8; 32],
    mandate_nonce_commitment: [u8; 32],
    repository_commitment: [u8; 32],
    store_instance_commitment: [u8; 32],
    head_commitment: [u8; 32],
    expected_old_binding_commitment: [u8; 32],
    current_policy_commitment: [u8; 32],
    candidate_policy_commitment: [u8; 32],
    evaluator_commitment: [u8; 32],
    complete_diff_commitment: [u8; 32],
    classifier_commitment: [u8; 32],
    classifier_revision_commitment: [u8; 32],
    safety_floor_commitment: [u8; 32],
    governance_floor_commitment: [u8; 32],
    request_payload_commitment: [u8; 32],
    idempotency_meaning_commitment: [u8; 32],
    invocation_commitment: [u8; 32],
    authority_snapshot_commitment: [u8; 32],
    authority_fence_commitment: [u8; 32],
    trust_root_commitment: [u8; 32],
    normalized_witness_commitment: [u8; 32],
    debit_map_commitment: [u8; 32],
    root_use_atoms_commitment: [u8; 32],
    mandate_atom_commitment: [u8; 32],
    successor_capacity_commitment: [u8; 32],
}

pub(in crate::domain::vnext) struct SchedulingPolicyDowngradeMandateUseCellV1 {
    consumed: Cell<bool>,
}

impl SchedulingPolicyDowngradeMandateUseCellV1 {
    pub(in crate::domain::vnext::authority) const fn new() -> Self {
        Self {
            consumed: Cell::new(false),
        }
    }
}

pub(in crate::domain::vnext) struct VerifiedSchedulingPolicyDowngradeMandateUseV1<'tx> {
    facts: SchedulingPolicyDowngradeMandateFactsV1,
    cell: &'tx SchedulingPolicyDowngradeMandateUseCellV1,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::domain::vnext) struct RepositoryActionCommitFactsV1 {
    binding: RepositoryActionBindingFactsV1,
}

pub(in crate::domain::vnext) struct ConsumedSchedulingPolicyDowngradeMandateV1<'tx> {
    _transaction: PhantomData<&'tx mut ()>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl<'tx> VerifiedSchedulingPolicyDowngradeMandateUseV1<'tx> {
    pub(in crate::domain::vnext) fn consume_with_action_binding(
        self,
        binding: AdmittedRepositoryActionBindingV1<'tx, SchedulingPolicyBindingOwnerV1>,
        commit: RepositoryActionCommitFactsV1,
    ) -> Result<ConsumedSchedulingPolicyDowngradeMandateV1<'tx>, AuthorityMaterializationErrorV1>
    {
        if self.cell.consumed.get()
            || binding.cell.consumed.get()
            || binding.facts != commit.binding
            || self.facts.repository_generation_id != commit.binding.repository_generation_id
            || self.facts.principal_id != commit.binding.principal_id
            || self.facts.human_binding_id != commit.binding.binding_id
            || self.facts.human_session_id != commit.binding.session_id
            || self.facts.authority_context_id != commit.binding.authority_context_id
            || self.facts.state_token_id != commit.binding.state_token_id
            || self.facts.authority_epoch != commit.binding.authority_epoch
            || self.facts.trusted_time != commit.binding.trusted_time
            || self.facts.repository_commitment != commit.binding.repository_commitment
            || self.facts.store_instance_commitment != commit.binding.store_instance_commitment
            || self.facts.head_commitment != commit.binding.head_commitment
            || self.facts.expected_old_binding_commitment
                != commit.binding.expected_old_owner_state_commitment
            || self.facts.authority_snapshot_commitment
                != commit.binding.authority_snapshot_commitment
            || self.facts.authority_fence_commitment != commit.binding.authority_fence_commitment
            || self.facts.normalized_witness_commitment
                != commit.binding.normalized_witness_commitment
            || self.facts.mandate_atom_commitment != commit.binding.supplemental_mandate_atom
            || self.facts.debit_map_commitment != commit.binding.debit_map_commitment
            || self.facts.root_use_atoms_commitment != commit.binding.root_use_atoms_commitment
            || self.facts.invocation_commitment != commit.binding.invocation_commitment
            || self.facts.successor_capacity_commitment
                != commit.binding.successor_capacity_commitment
        {
            return Err(AuthorityMaterializationErrorV1::BindingMismatch);
        }
        self.cell.consumed.set(true);
        binding.cell.consumed.set(true);
        Ok(ConsumedSchedulingPolicyDowngradeMandateV1 {
            _transaction: PhantomData,
            _not_send_or_sync: PhantomData,
        })
    }
}

mod owner_sealed {
    pub trait Sealed {}
}

pub(in crate::domain::vnext) trait RepositoryActionBindingKindV1:
    owner_sealed::Sealed
{
    const ACTION: RepositoryActionLeafV1;
}

pub(in crate::domain::vnext) struct SchedulingPolicyBindingOwnerV1;

impl owner_sealed::Sealed for SchedulingPolicyBindingOwnerV1 {}

impl RepositoryActionBindingKindV1 for SchedulingPolicyBindingOwnerV1 {
    const ACTION: RepositoryActionLeafV1 = RepositoryActionLeafV1::Downstream(
        RepositoryDownstreamActionLeafV1::PUBLISH_SCHEDULING_POLICY_BINDING,
    );
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::domain::vnext) struct RepositoryActionBindingFactsV1 {
    authority_selection: RepositoryAuthoritySelectionV1,
    request_id: ActionRequestIdV1,
    action: RepositoryActionLeafV1,
    principal_id: PrincipalIdV1,
    binding_id: PrincipalBindingIdV1,
    session_id: SessionIdV1,
    repository_generation_id: StoreGenerationIdV1,
    authority_context_id: AuthorityContextIdV1,
    state_token_id: StateTokenIdV1,
    authority_epoch: u64,
    trusted_time: u64,
    subject_commitment: [u8; 32],
    subject_basis_commitment: [u8; 32],
    exact_payload_commitment: [u8; 32],
    action_spec_commitment: [u8; 32],
    repository_commitment: [u8; 32],
    store_instance_commitment: [u8; 32],
    head_commitment: [u8; 32],
    expected_old_owner_state_commitment: [u8; 32],
    participant_set_commitment: [u8; 32],
    owner_publication_commitment: [u8; 32],
    write_set_commitment: [u8; 32],
    output_commitment: [u8; 32],
    authority_basis_commitment: [u8; 32],
    authority_snapshot_commitment: [u8; 32],
    authority_fence_commitment: [u8; 32],
    currentness_commitment: [u8; 32],
    revocation_commitment: [u8; 32],
    normalized_witness_commitment: [u8; 32],
    debit_map_commitment: [u8; 32],
    root_use_atoms_commitment: [u8; 32],
    supplemental_mandate_atom: [u8; 32],
    planned_debit_commitment: [u8; 32],
    planned_consumption_commitment: [u8; 32],
    idempotency_key: IdempotencyKeyIdV1,
    idempotency_mapping_commitment: [u8; 32],
    successor_capacity_commitment: [u8; 32],
    receipt_id: AuthorizationReceiptIdV1,
    result_commitment: [u8; 32],
    invocation_commitment: [u8; 32],
}

pub(in crate::domain::vnext) struct RepositoryActionBindingUseCellV1 {
    consumed: Cell<bool>,
}

impl RepositoryActionBindingUseCellV1 {
    pub(in crate::domain::vnext::authority) const fn new() -> Self {
        Self {
            consumed: Cell::new(false),
        }
    }
}

pub(in crate::domain::vnext) struct AdmittedRepositoryActionBindingV1<'tx, K> {
    facts: RepositoryActionBindingFactsV1,
    cell: &'tx RepositoryActionBindingUseCellV1,
    _transaction: PhantomData<&'tx mut K>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl<'tx, K: RepositoryActionBindingKindV1> AdmittedRepositoryActionBindingV1<'tx, K> {
    pub(in crate::domain::vnext::authority) fn from_admitted_facts(
        facts: RepositoryActionBindingFactsV1,
        cell: &'tx RepositoryActionBindingUseCellV1,
    ) -> Result<Self, AuthorityMaterializationErrorV1> {
        let commitments = [
            facts.subject_commitment,
            facts.subject_basis_commitment,
            facts.exact_payload_commitment,
            facts.action_spec_commitment,
            facts.repository_commitment,
            facts.store_instance_commitment,
            facts.head_commitment,
            facts.expected_old_owner_state_commitment,
            facts.participant_set_commitment,
            facts.owner_publication_commitment,
            facts.write_set_commitment,
            facts.output_commitment,
            facts.authority_basis_commitment,
            facts.authority_snapshot_commitment,
            facts.authority_fence_commitment,
            facts.currentness_commitment,
            facts.revocation_commitment,
            facts.normalized_witness_commitment,
            facts.debit_map_commitment,
            facts.root_use_atoms_commitment,
            facts.supplemental_mandate_atom,
            facts.planned_debit_commitment,
            facts.planned_consumption_commitment,
            facts.idempotency_mapping_commitment,
            facts.successor_capacity_commitment,
            facts.result_commitment,
            facts.invocation_commitment,
        ];
        if facts.action != K::ACTION
            || commitments.contains(&[0; 32])
            || facts.authority_epoch == 0
            || facts.trusted_time == 0
            || facts.authority_selection.actor_binding_id() != facts.binding_id
            || facts.authority_selection.actor_session_id() != facts.session_id
            || cell.consumed.get()
        {
            return Err(AuthorityMaterializationErrorV1::BindingMismatch);
        }
        let RepositoryActionLeafV1::Downstream(downstream) = facts.action else {
            return Err(AuthorityMaterializationErrorV1::BindingMismatch);
        };
        construct_owner_local_repository_authority_input(
            facts.authority_selection,
            downstream,
            facts.subject_commitment,
            facts.subject_basis_commitment,
            facts.exact_payload_commitment,
        )
        .map_err(|_| AuthorityMaterializationErrorV1::BindingMismatch)?;
        Ok(Self {
            facts,
            cell,
            _transaction: PhantomData,
            _not_send_or_sync: PhantomData,
        })
    }
}

pub(in crate::domain::vnext::authority) fn verify_scheduling_policy_downgrade_mandate_use<'tx>(
    facts: SchedulingPolicyDowngradeMandateFactsV1,
    cell: &'tx SchedulingPolicyDowngradeMandateUseCellV1,
) -> Result<VerifiedSchedulingPolicyDowngradeMandateUseV1<'tx>, AuthorityMaterializationErrorV1> {
    let commitments = [
        facts.mandate_body_commitment,
        facts.mandate_carrier_commitment,
        facts.mandate_nonce_commitment,
        facts.repository_commitment,
        facts.store_instance_commitment,
        facts.head_commitment,
        facts.expected_old_binding_commitment,
        facts.current_policy_commitment,
        facts.candidate_policy_commitment,
        facts.evaluator_commitment,
        facts.complete_diff_commitment,
        facts.classifier_commitment,
        facts.classifier_revision_commitment,
        facts.safety_floor_commitment,
        facts.governance_floor_commitment,
        facts.request_payload_commitment,
        facts.idempotency_meaning_commitment,
        facts.invocation_commitment,
        facts.authority_snapshot_commitment,
        facts.authority_fence_commitment,
        facts.trust_root_commitment,
        facts.normalized_witness_commitment,
        facts.debit_map_commitment,
        facts.root_use_atoms_commitment,
        facts.mandate_atom_commitment,
        facts.successor_capacity_commitment,
    ];
    if !facts.diff_class.requires_downgrade_mandate()
        || commitments.contains(&[0; 32])
        || facts.current_policy_commitment == facts.candidate_policy_commitment
        || facts.mandate_schema_version == 0
        || facts.authority_epoch == 0
        || facts.revocation_revision == 0
        || facts.valid_from >= facts.valid_until
        || !(facts.valid_from..facts.valid_until).contains(&facts.trusted_time)
        || cell.consumed.get()
    {
        return Err(AuthorityMaterializationErrorV1::InvalidMandate);
    }
    Ok(VerifiedSchedulingPolicyDowngradeMandateUseV1 {
        facts,
        cell,
        _not_send_or_sync: PhantomData,
    })
}

type VerifySchedulingPolicyDowngradeMandateUseFnV1 = for<'tx> fn(
    SchedulingPolicyDowngradeMandateFactsV1,
    &'tx SchedulingPolicyDowngradeMandateUseCellV1,
) -> Result<
    VerifiedSchedulingPolicyDowngradeMandateUseV1<'tx>,
    AuthorityMaterializationErrorV1,
>;

type AdmitSchedulingPolicyBindingFnV1 = for<'tx> fn(
    RepositoryActionBindingFactsV1,
    &'tx RepositoryActionBindingUseCellV1,
) -> Result<
    AdmittedRepositoryActionBindingV1<'tx, SchedulingPolicyBindingOwnerV1>,
    AuthorityMaterializationErrorV1,
>;

type ConsumeSchedulingPolicyDowngradeFnV1 = for<'tx> fn(
    VerifiedSchedulingPolicyDowngradeMandateUseV1<'tx>,
    AdmittedRepositoryActionBindingV1<'tx, SchedulingPolicyBindingOwnerV1>,
    RepositoryActionCommitFactsV1,
) -> Result<
    ConsumedSchedulingPolicyDowngradeMandateV1<'tx>,
    AuthorityMaterializationErrorV1,
>;

fn admit_scheduling_policy_binding<'tx>(
    facts: RepositoryActionBindingFactsV1,
    cell: &'tx RepositoryActionBindingUseCellV1,
) -> Result<
    AdmittedRepositoryActionBindingV1<'tx, SchedulingPolicyBindingOwnerV1>,
    AuthorityMaterializationErrorV1,
> {
    AdmittedRepositoryActionBindingV1::<SchedulingPolicyBindingOwnerV1>::from_admitted_facts(
        facts, cell,
    )
}

fn consume_scheduling_policy_downgrade<'tx>(
    mandate: VerifiedSchedulingPolicyDowngradeMandateUseV1<'tx>,
    binding: AdmittedRepositoryActionBindingV1<'tx, SchedulingPolicyBindingOwnerV1>,
    commit: RepositoryActionCommitFactsV1,
) -> Result<ConsumedSchedulingPolicyDowngradeMandateV1<'tx>, AuthorityMaterializationErrorV1> {
    mandate.consume_with_action_binding(binding, commit)
}

pub(in crate::domain::vnext::authority) struct MaterializationAuthoritySeedV1 {
    _diff_classes: [SchedulingPolicyDiffClassV1; 4],
    _new_mandate_use_cell: fn() -> SchedulingPolicyDowngradeMandateUseCellV1,
    _new_action_binding_cell: fn() -> RepositoryActionBindingUseCellV1,
    _verify_downgrade_mandate: VerifySchedulingPolicyDowngradeMandateUseFnV1,
    _admit_scheduling_binding: AdmitSchedulingPolicyBindingFnV1,
    _consume_scheduling_downgrade: ConsumeSchedulingPolicyDowngradeFnV1,
}

pub(in crate::domain::vnext::authority) const fn authority_materialization_seed_v1()
-> MaterializationAuthoritySeedV1 {
    MaterializationAuthoritySeedV1 {
        _diff_classes: [
            SchedulingPolicyDiffClassV1::Equivalent,
            SchedulingPolicyDiffClassV1::Strengthening,
            SchedulingPolicyDiffClassV1::Weakening,
            SchedulingPolicyDiffClassV1::Incomparable,
        ],
        _new_mandate_use_cell: SchedulingPolicyDowngradeMandateUseCellV1::new,
        _new_action_binding_cell: RepositoryActionBindingUseCellV1::new,
        _verify_downgrade_mandate: verify_scheduling_policy_downgrade_mandate_use,
        _admit_scheduling_binding: admit_scheduling_policy_binding,
        _consume_scheduling_downgrade: consume_scheduling_policy_downgrade,
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub(in crate::domain::vnext) enum AuthorityMaterializationErrorV1 {
    #[error("scheduling-policy downgrade Mandate is unavailable")]
    InvalidMandate,
    #[error("repository Action binding does not match the admitted transaction")]
    BindingMismatch,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> ActionRequestIdV1 {
        ActionRequestIdV1::derive("materialization-request").unwrap()
    }

    fn selection() -> RepositoryAuthoritySelectionV1 {
        RepositoryAuthoritySelectionV1::new(
            PrincipalBindingIdV1::derive("materialization-human").unwrap(),
            SessionIdV1::derive("materialization-human-session").unwrap(),
            GrantIdV1::derive("materialization-selection-grant").unwrap(),
        )
    }

    fn facts() -> SchedulingPolicyDowngradeMandateFactsV1 {
        SchedulingPolicyDowngradeMandateFactsV1 {
            mandate_id: MandateIdV1::derive("materialization-mandate").unwrap(),
            repository_generation_id: StoreGenerationIdV1::from_digest([13; 32]),
            principal_id: PrincipalIdV1::derive("materialization-principal").unwrap(),
            human_binding_id: PrincipalBindingIdV1::derive("materialization-human").unwrap(),
            human_session_id: SessionIdV1::derive("materialization-human-session").unwrap(),
            authority_context_id: AuthorityContextIdV1::derive("materialization-context").unwrap(),
            state_token_id: StateTokenIdV1::derive("materialization-state-token").unwrap(),
            mandate_schema_version: 1,
            authority_epoch: 8,
            valid_from: 10,
            valid_until: 20,
            trusted_time: 15,
            revocation_revision: 7,
            diff_class: SchedulingPolicyDiffClassV1::Weakening,
            mandate_body_commitment: [1; 32],
            mandate_carrier_commitment: [2; 32],
            mandate_nonce_commitment: [3; 32],
            repository_commitment: [4; 32],
            store_instance_commitment: [5; 32],
            head_commitment: [6; 32],
            expected_old_binding_commitment: [7; 32],
            current_policy_commitment: [8; 32],
            candidate_policy_commitment: [9; 32],
            evaluator_commitment: [10; 32],
            complete_diff_commitment: [11; 32],
            classifier_commitment: [12; 32],
            classifier_revision_commitment: [13; 32],
            safety_floor_commitment: [14; 32],
            governance_floor_commitment: [15; 32],
            request_payload_commitment: [16; 32],
            idempotency_meaning_commitment: [17; 32],
            invocation_commitment: [18; 32],
            authority_snapshot_commitment: [19; 32],
            authority_fence_commitment: [20; 32],
            trust_root_commitment: [21; 32],
            normalized_witness_commitment: [22; 32],
            debit_map_commitment: [23; 32],
            root_use_atoms_commitment: [24; 32],
            mandate_atom_commitment: [25; 32],
            successor_capacity_commitment: [26; 32],
        }
    }

    fn binding_facts() -> RepositoryActionBindingFactsV1 {
        RepositoryActionBindingFactsV1 {
            authority_selection: selection(),
            request_id: request(),
            action: SchedulingPolicyBindingOwnerV1::ACTION,
            principal_id: facts().principal_id,
            binding_id: facts().human_binding_id,
            session_id: facts().human_session_id,
            repository_generation_id: facts().repository_generation_id,
            authority_context_id: facts().authority_context_id,
            state_token_id: facts().state_token_id,
            authority_epoch: facts().authority_epoch,
            trusted_time: facts().trusted_time,
            subject_commitment: [30; 32],
            subject_basis_commitment: [31; 32],
            exact_payload_commitment: [32; 32],
            action_spec_commitment: [33; 32],
            repository_commitment: facts().repository_commitment,
            store_instance_commitment: facts().store_instance_commitment,
            head_commitment: facts().head_commitment,
            expected_old_owner_state_commitment: facts().expected_old_binding_commitment,
            participant_set_commitment: [34; 32],
            owner_publication_commitment: [35; 32],
            write_set_commitment: [36; 32],
            output_commitment: [37; 32],
            authority_basis_commitment: [38; 32],
            authority_snapshot_commitment: facts().authority_snapshot_commitment,
            authority_fence_commitment: facts().authority_fence_commitment,
            currentness_commitment: [39; 32],
            revocation_commitment: [40; 32],
            normalized_witness_commitment: facts().normalized_witness_commitment,
            debit_map_commitment: facts().debit_map_commitment,
            root_use_atoms_commitment: facts().root_use_atoms_commitment,
            supplemental_mandate_atom: facts().mandate_atom_commitment,
            planned_debit_commitment: [41; 32],
            planned_consumption_commitment: [42; 32],
            idempotency_key: IdempotencyKeyIdV1::derive("materialization-idempotency").unwrap(),
            idempotency_mapping_commitment: [43; 32],
            successor_capacity_commitment: facts().successor_capacity_commitment,
            receipt_id: AuthorizationReceiptIdV1::derive("materialization-receipt").unwrap(),
            result_commitment: [44; 32],
            invocation_commitment: facts().invocation_commitment,
        }
    }

    #[test]
    fn downgrade_mandate_and_action_binding_are_one_use_and_exact() {
        let cell = SchedulingPolicyDowngradeMandateUseCellV1::new();
        let use_token = verify_scheduling_policy_downgrade_mandate_use(facts(), &cell).unwrap();
        let binding_cell = RepositoryActionBindingUseCellV1::new();
        let binding =
            AdmittedRepositoryActionBindingV1::from_admitted_facts(binding_facts(), &binding_cell)
                .unwrap();
        let commit = RepositoryActionCommitFactsV1 {
            binding: binding_facts(),
        };
        use_token
            .consume_with_action_binding(binding, commit)
            .unwrap();
        assert!(matches!(
            verify_scheduling_policy_downgrade_mandate_use(facts(), &cell),
            Err(AuthorityMaterializationErrorV1::InvalidMandate)
        ));
        assert!(matches!(
            AdmittedRepositoryActionBindingV1::<SchedulingPolicyBindingOwnerV1>::from_admitted_facts(
                binding_facts(),
                &binding_cell,
            ),
            Err(AuthorityMaterializationErrorV1::BindingMismatch)
        ));
    }

    #[test]
    fn equivalent_policy_and_cross_transaction_substitution_refuse_without_consumption() {
        let cell = SchedulingPolicyDowngradeMandateUseCellV1::new();
        let mut equivalent = facts();
        equivalent.diff_class = SchedulingPolicyDiffClassV1::Equivalent;
        assert!(matches!(
            verify_scheduling_policy_downgrade_mandate_use(equivalent, &cell),
            Err(AuthorityMaterializationErrorV1::InvalidMandate)
        ));

        let use_token = verify_scheduling_policy_downgrade_mandate_use(facts(), &cell).unwrap();
        let binding_cell = RepositoryActionBindingUseCellV1::new();
        let binding =
            AdmittedRepositoryActionBindingV1::from_admitted_facts(binding_facts(), &binding_cell)
                .unwrap();
        let mut substituted = binding_facts();
        substituted.request_id = ActionRequestIdV1::derive("other-request").unwrap();
        let wrong = RepositoryActionCommitFactsV1 {
            binding: substituted,
        };
        assert!(matches!(
            use_token.consume_with_action_binding(binding, wrong),
            Err(AuthorityMaterializationErrorV1::BindingMismatch)
        ));
        assert!(verify_scheduling_policy_downgrade_mandate_use(facts(), &cell).is_ok());

        for diff_class in [
            SchedulingPolicyDiffClassV1::Strengthening,
            SchedulingPolicyDiffClassV1::Incomparable,
        ] {
            let mut variant = facts();
            variant.diff_class = diff_class;
            assert_eq!(
                verify_scheduling_policy_downgrade_mandate_use(
                    variant,
                    &SchedulingPolicyDowngradeMandateUseCellV1::new()
                )
                .is_ok(),
                diff_class == SchedulingPolicyDiffClassV1::Incomparable
            );
        }
    }

    #[test]
    fn wrong_owner_action_cannot_enter_the_scheduling_binding() {
        let mut facts = binding_facts();
        facts.action = RepositoryActionLeafV1::CreateDraftWork;
        let cell = RepositoryActionBindingUseCellV1::new();
        assert!(matches!(
            AdmittedRepositoryActionBindingV1::<SchedulingPolicyBindingOwnerV1>::from_admitted_facts(
                facts,
                &cell,
            ),
            Err(AuthorityMaterializationErrorV1::BindingMismatch)
        ));
    }
}
