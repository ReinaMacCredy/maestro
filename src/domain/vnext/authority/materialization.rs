use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::rc::Rc;

use thiserror::Error;

#[cfg(test)]
use super::GrantIdV1;
use super::facade::{
    AdmittedRepositoryActionV1, CoordinationRepositoryActionAuthorityV1,
    DistributionRepositoryActionAuthorityV1, IntakeRepositoryActionAuthorityV1,
    MaterializationAuthorityAdmissionV1, MemoryRepositoryActionAuthorityV1,
    PersistenceRepositoryActionAuthorityV1, PlanningRepositoryActionAuthorityV1,
    ResearchRepositoryActionAuthorityV1, SearchMaintenanceRepositoryActionAuthorityV1,
};
use super::{
    ActionRequestIdV1, AuthorityContextIdV1, AuthorizationReceiptIdV1, IdempotencyKeyIdV1,
    MandateIdV1, PrincipalBindingIdV1, PrincipalIdV1, RepositoryActionLeafV1,
    RepositoryAuthoritySelectionV1, RepositoryDownstreamActionLeafV1, SessionIdV1, StateTokenIdV1,
};
use crate::domain::vnext::identity::{StoreGenerationIdV1, StoreHeadIdV1, StoreObjectIdV1};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchedulingPolicyDiffClassV1 {
    Equivalent,
    Strengthening,
    Weakening,
    Incomparable,
}

const NO_SUPPLEMENTAL_MANDATE_COMMITMENT_V1: [u8; 32] = [0xA5; 32];

impl SchedulingPolicyDiffClassV1 {
    const fn requires_downgrade_mandate(self) -> bool {
        matches!(self, Self::Weakening | Self::Incomparable)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SchedulingPolicyMeaningV1 {
    current_rules: [u64; 4],
    candidate_rules: [u64; 4],
    safety_floor: [u64; 4],
    governance_floor: [u64; 4],
    evaluator_revision: u64,
    classifier_revision: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::domain::vnext) struct SchedulingPolicyDowngradeMandateFactsV1 {
    policy_meaning: SchedulingPolicyMeaningV1,
    pub(crate) mandate_id: MandateIdV1,
    pub(crate) action_request_id: ActionRequestIdV1,
    pub(crate) idempotency_key: IdempotencyKeyIdV1,
    pub(crate) repository_generation_id: StoreGenerationIdV1,
    pub(crate) principal_id: PrincipalIdV1,
    pub(crate) human_binding_id: PrincipalBindingIdV1,
    pub(crate) human_session_id: SessionIdV1,
    pub(crate) authority_context_id: AuthorityContextIdV1,
    pub(crate) state_token_id: StateTokenIdV1,
    pub(crate) mandate_schema_version: u64,
    pub(crate) authority_epoch: u64,
    pub(crate) valid_from: u64,
    pub(crate) valid_until: u64,
    pub(crate) trusted_time: u64,
    pub(crate) revocation_revision: u64,
    pub(crate) diff_class: SchedulingPolicyDiffClassV1,
    pub(crate) mandate_body_commitment: [u8; 32],
    pub(crate) mandate_carrier_commitment: [u8; 32],
    pub(crate) mandate_nonce_commitment: [u8; 32],
    pub(crate) repository_commitment: [u8; 32],
    pub(crate) store_instance_commitment: [u8; 32],
    pub(crate) head_commitment: [u8; 32],
    pub(crate) expected_old_binding_commitment: [u8; 32],
    pub(crate) current_policy_commitment: [u8; 32],
    pub(crate) candidate_policy_commitment: [u8; 32],
    pub(crate) evaluator_commitment: [u8; 32],
    pub(crate) complete_diff_commitment: [u8; 32],
    pub(crate) classifier_commitment: [u8; 32],
    pub(crate) classifier_revision_commitment: [u8; 32],
    pub(crate) safety_floor_commitment: [u8; 32],
    pub(crate) governance_floor_commitment: [u8; 32],
    pub(crate) request_payload_commitment: [u8; 32],
    pub(crate) idempotency_meaning_commitment: [u8; 32],
    pub(crate) idempotency_mapping_commitment: [u8; 32],
    pub(crate) action_spec_commitment: [u8; 32],
    pub(crate) subject_commitment: [u8; 32],
    pub(crate) subject_basis_commitment: [u8; 32],
    pub(crate) exact_payload_commitment: [u8; 32],
    pub(crate) participant_set_commitment: [u8; 32],
    pub(crate) owner_publication_commitment: [u8; 32],
    pub(crate) write_set_commitment: [u8; 32],
    pub(crate) output_commitment: [u8; 32],
    pub(crate) planned_debit_commitment: [u8; 32],
    pub(crate) planned_consumption_commitment: [u8; 32],
    pub(crate) result_commitment: [u8; 32],
    pub(crate) invocation_commitment: [u8; 32],
    pub(crate) authority_snapshot_commitment: [u8; 32],
    pub(crate) authority_fence_commitment: [u8; 32],
    pub(crate) authority_basis_commitment: [u8; 32],
    pub(crate) currentness_commitment: [u8; 32],
    pub(crate) revocation_commitment: [u8; 32],
    pub(crate) authorization_receipt_id: AuthorizationReceiptIdV1,
    pub(crate) trust_root_commitment: [u8; 32],
    pub(crate) normalized_witness_commitment: [u8; 32],
    pub(crate) debit_map_commitment: [u8; 32],
    pub(crate) root_use_atoms_commitment: [u8; 32],
    pub(crate) mandate_atom_commitment: [u8; 32],
    pub(crate) successor_capacity_commitment: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MaterializationTransactionStateV1 {
    Fresh,
    MandateMinted([u8; 32]),
    BindingMinted([u8; 32]),
    BothMinted {
        mandate_atom: [u8; 32],
        invocation: [u8; 32],
    },
    Consumed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ConsumedRepositoryActionBindingV1 {
    repository_generation_id: StoreGenerationIdV1,
    successor_store_generation: u64,
    idempotency_key: IdempotencyKeyIdV1,
    idempotency_meaning_commitment: [u8; 32],
    result_commitment: [u8; 32],
    write_set_commitment: [u8; 32],
    receipt_object_id: StoreObjectIdV1,
    basis_object_id: StoreObjectIdV1,
    current_snapshot_id: StoreObjectIdV1,
    successor_snapshot_id: StoreObjectIdV1,
    current_capacity_root_id: StoreObjectIdV1,
    successor_capacity_root_id: StoreObjectIdV1,
    capacity_debit_id: StoreObjectIdV1,
    leaf_authority_carrier_id: Option<StoreObjectIdV1>,
    leaf_authority_consumption_id: Option<StoreObjectIdV1>,
    guard_object_id: StoreObjectIdV1,
    state_object_id: StoreObjectIdV1,
    mandate_object_ids: Option<[StoreObjectIdV1; 4]>,
}

impl ConsumedRepositoryActionBindingV1 {
    fn new(
        facts: RepositoryActionBindingFactsV1,
        admission: MaterializationAuthorityAdmissionV1,
    ) -> Self {
        Self {
            repository_generation_id: facts.repository_generation_id,
            successor_store_generation: admission.successor_store_generation,
            idempotency_key: facts.idempotency_key,
            idempotency_meaning_commitment: facts.idempotency_meaning_commitment,
            result_commitment: facts.result_commitment,
            write_set_commitment: facts.write_set_commitment,
            receipt_object_id: StoreObjectIdV1::from_digest(
                facts.authorization_receipt_object_commitment,
            ),
            basis_object_id: admission.basis_object_id,
            current_snapshot_id: admission.current_snapshot_id,
            successor_snapshot_id: admission.successor_snapshot_id,
            current_capacity_root_id: admission.current_capacity_root_id,
            successor_capacity_root_id: admission.successor_capacity_root_id,
            capacity_debit_id: admission.capacity_debit_id,
            leaf_authority_carrier_id: admission.leaf_authority_carrier_id,
            leaf_authority_consumption_id: admission.leaf_authority_consumption_id,
            guard_object_id: admission.guard_object_id,
            state_object_id: admission.state_object_id,
            mandate_object_ids: facts.supplemental_mandate_id.map(|_| {
                [
                    StoreObjectIdV1::from_digest(facts.supplemental_mandate_atom),
                    StoreObjectIdV1::from_digest(facts.supplemental_mandate_body_commitment),
                    StoreObjectIdV1::from_digest(facts.supplemental_mandate_carrier_commitment),
                    StoreObjectIdV1::from_digest(facts.supplemental_mandate_nonce_commitment),
                ]
            }),
        }
    }
}

pub(in crate::domain::vnext::authority) struct AuthorityMaterializationTransactionV1<'tx> {
    view: MaterializationCurrentViewV1<'tx>,
    state: Cell<MaterializationTransactionStateV1>,
    consumed_binding: RefCell<Option<ConsumedRepositoryActionBindingV1>>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

pub(in crate::domain::vnext::authority) trait MaterializationStoreViewPortV1 {
    fn active_head_id(&self) -> Result<Option<StoreHeadIdV1>, AuthorityMaterializationErrorV1>;
    fn active_generation_id(&self) -> Result<StoreGenerationIdV1, AuthorityMaterializationErrorV1>;
    fn active_generation_object_ids(
        &self,
    ) -> Result<Vec<StoreObjectIdV1>, AuthorityMaterializationErrorV1>;
}

pub(in crate::domain::vnext::authority) trait MaterializationAtomicPublicationPortV1 {
    fn expected_old(&self) -> Option<StoreHeadIdV1>;
    fn generation_ordinal(&self) -> u64;
    fn generation_previous(&self) -> Option<StoreGenerationIdV1>;
    fn probe_key_digest(&self) -> [u8; 32];
    fn probe_meaning_digest(&self) -> [u8; 32];
    fn idempotency_key_digest(&self) -> [u8; 32];
    fn idempotency_meaning_digest(&self) -> [u8; 32];
    fn idempotency_result_object_id(&self) -> StoreObjectIdV1;
    fn object_ids(&self) -> Vec<StoreObjectIdV1>;
}

enum MaterializationCurrentViewV1<'tx> {
    Store(&'tx dyn MaterializationStoreViewPortV1),
    #[cfg(test)]
    Test(StoreGenerationIdV1),
}

impl<'tx> AuthorityMaterializationTransactionV1<'tx> {
    pub(in crate::domain::vnext::authority) fn from_live_store_transaction(
        view: &'tx dyn MaterializationStoreViewPortV1,
    ) -> Self {
        freeze_repository_owner_ports();
        Self {
            view: MaterializationCurrentViewV1::Store(view),
            state: Cell::new(MaterializationTransactionStateV1::Fresh),
            consumed_binding: RefCell::new(None),
            _not_send_or_sync: PhantomData,
        }
    }

    pub(in crate::domain::vnext::authority) fn mint_mandate(
        &'tx self,
        facts: SchedulingPolicyDowngradeMandateFactsV1,
    ) -> Result<VerifiedSchedulingPolicyDowngradeMandateUseV1<'tx>, AuthorityMaterializationErrorV1>
    {
        validate_scheduling_policy_downgrade_mandate(facts)?;
        self.require_live_generation(facts.repository_generation_id)?;
        self.state.set(match self.state.get() {
            MaterializationTransactionStateV1::Fresh => {
                MaterializationTransactionStateV1::MandateMinted(facts.mandate_atom_commitment)
            }
            MaterializationTransactionStateV1::BindingMinted(invocation) => {
                MaterializationTransactionStateV1::BothMinted {
                    mandate_atom: facts.mandate_atom_commitment,
                    invocation,
                }
            }
            _ => return Err(AuthorityMaterializationErrorV1::DuplicateUse),
        });
        Ok(VerifiedSchedulingPolicyDowngradeMandateUseV1 {
            facts,
            transaction: self,
            _not_send_or_sync: PhantomData,
        })
    }

    pub(in crate::domain::vnext::authority) fn mint_binding<K: RepositoryActionBindingKindV1>(
        &'tx self,
        admission: &AdmittedRepositoryActionV1,
        facts: RepositoryActionBindingFactsV1,
    ) -> Result<AdmittedRepositoryActionBindingV1<'tx, K>, AuthorityMaterializationErrorV1> {
        let admission = admission.materialization_admission();
        validate_repository_action_binding::<K>(facts, admission)?;
        self.require_live_generation(facts.repository_generation_id)?;
        self.state.set(match self.state.get() {
            MaterializationTransactionStateV1::Fresh => {
                MaterializationTransactionStateV1::BindingMinted(facts.invocation_commitment)
            }
            MaterializationTransactionStateV1::MandateMinted(mandate_atom) => {
                MaterializationTransactionStateV1::BothMinted {
                    mandate_atom,
                    invocation: facts.invocation_commitment,
                }
            }
            _ => return Err(AuthorityMaterializationErrorV1::DuplicateUse),
        });
        Ok(AdmittedRepositoryActionBindingV1 {
            facts,
            admission,
            transaction: self,
            _transaction: PhantomData,
            _not_send_or_sync: PhantomData,
        })
    }

    #[cfg(test)]
    fn mint_binding_from_test_admission<K: RepositoryActionBindingKindV1>(
        &'tx self,
        admission: MaterializationAuthorityAdmissionV1,
        facts: RepositoryActionBindingFactsV1,
    ) -> Result<AdmittedRepositoryActionBindingV1<'tx, K>, AuthorityMaterializationErrorV1> {
        validate_repository_action_binding::<K>(facts, admission)?;
        self.require_live_generation(facts.repository_generation_id)?;
        self.state.set(match self.state.get() {
            MaterializationTransactionStateV1::Fresh => {
                MaterializationTransactionStateV1::BindingMinted(facts.invocation_commitment)
            }
            MaterializationTransactionStateV1::MandateMinted(mandate_atom) => {
                MaterializationTransactionStateV1::BothMinted {
                    mandate_atom,
                    invocation: facts.invocation_commitment,
                }
            }
            _ => return Err(AuthorityMaterializationErrorV1::DuplicateUse),
        });
        Ok(AdmittedRepositoryActionBindingV1 {
            facts,
            admission,
            transaction: self,
            _transaction: PhantomData,
            _not_send_or_sync: PhantomData,
        })
    }

    pub(in crate::domain::vnext::authority) fn consume_binding_without_mandate<
        K: RepositoryActionBindingKindV1,
    >(
        &self,
        binding: AdmittedRepositoryActionBindingV1<'_, K>,
        commit: RepositoryActionCommitFactsV1,
    ) -> Result<ConsumedRepositoryActionMaterializationV1<'_>, AuthorityMaterializationErrorV1>
    {
        let facts = binding.facts;
        validate_repository_action_binding::<K>(facts, binding.admission)?;
        if K::ACTION == SchedulingPolicyBindingOwnerV1::ACTION {
            validate_scheduling_policy_binding(facts, binding.admission, false)?;
        }
        self.require_live_generation(facts.repository_generation_id)?;
        if !std::ptr::eq(self, binding.transaction)
            || self.state.get()
                != MaterializationTransactionStateV1::BindingMinted(facts.invocation_commitment)
            || facts != commit.binding
            || facts.supplemental_mandate_id.is_some()
            || facts.supplemental_mandate_atom != NO_SUPPLEMENTAL_MANDATE_COMMITMENT_V1
            || facts.supplemental_mandate_body_commitment != NO_SUPPLEMENTAL_MANDATE_COMMITMENT_V1
            || facts.supplemental_mandate_carrier_commitment
                != NO_SUPPLEMENTAL_MANDATE_COMMITMENT_V1
            || facts.supplemental_mandate_nonce_commitment != NO_SUPPLEMENTAL_MANDATE_COMMITMENT_V1
        {
            return Err(AuthorityMaterializationErrorV1::BindingMismatch);
        }
        self.consumed_binding
            .replace(Some(ConsumedRepositoryActionBindingV1::new(
                facts,
                binding.admission,
            )));
        self.state.set(MaterializationTransactionStateV1::Consumed);
        Ok(ConsumedRepositoryActionMaterializationV1 {
            _transaction: PhantomData,
            _not_send_or_sync: PhantomData,
        })
    }

    fn require_live_generation(
        &self,
        generation_id: StoreGenerationIdV1,
    ) -> Result<(), AuthorityMaterializationErrorV1> {
        let active_generation_id = match self.view {
            MaterializationCurrentViewV1::Store(view) => view.active_generation_id()?,
            #[cfg(test)]
            MaterializationCurrentViewV1::Test(generation_id) => generation_id,
        };
        if active_generation_id != generation_id {
            return Err(AuthorityMaterializationErrorV1::StoreCurrentness);
        }
        Ok(())
    }

    #[cfg(test)]
    fn for_test(generation_id: StoreGenerationIdV1) -> Self {
        Self {
            view: MaterializationCurrentViewV1::Test(generation_id),
            state: Cell::new(MaterializationTransactionStateV1::Fresh),
            consumed_binding: RefCell::new(None),
            _not_send_or_sync: PhantomData,
        }
    }

    pub(in crate::domain::vnext::authority) fn validate_atomic_publication(
        &self,
        publication: &dyn MaterializationAtomicPublicationPortV1,
    ) -> Result<(), AuthorityMaterializationErrorV1> {
        if self.state.get() != MaterializationTransactionStateV1::Consumed {
            return Err(AuthorityMaterializationErrorV1::IncompleteTransaction);
        }
        let binding = self
            .consumed_binding
            .borrow()
            .as_ref()
            .copied()
            .ok_or(AuthorityMaterializationErrorV1::IncompleteTransaction)?;
        let mut write_set = sha2::Sha256::new();
        use sha2::Digest;
        write_set.update(b"maestro.authority.materialization-write-set.v1\0");
        let object_ids = publication.object_ids();
        for object_id in &object_ids {
            write_set.update(object_id.as_bytes());
        }
        let write_set_commitment: [u8; 32] = write_set.finalize().into();
        let (expected_old, active_generation_id, active_objects) = match self.view {
            MaterializationCurrentViewV1::Store(view) => (
                view.active_head_id()?,
                view.active_generation_id()?,
                view.active_generation_object_ids()?,
            ),
            #[cfg(test)]
            MaterializationCurrentViewV1::Test(_) => (
                publication.expected_old(),
                binding.repository_generation_id,
                Vec::new(),
            ),
        };
        let current_object_ids = [
            binding.current_snapshot_id,
            binding.current_capacity_root_id,
            binding.guard_object_id,
            binding.state_object_id,
        ];
        if active_generation_id != binding.repository_generation_id
            || publication.expected_old() != expected_old
            || publication.generation_ordinal() != binding.successor_store_generation
            || publication.generation_previous() != Some(binding.repository_generation_id)
            || publication.probe_key_digest() != *binding.idempotency_key.as_bytes()
            || publication.probe_meaning_digest() != binding.idempotency_meaning_commitment
            || publication.idempotency_key_digest() != *binding.idempotency_key.as_bytes()
            || publication.idempotency_meaning_digest() != binding.idempotency_meaning_commitment
            || publication.idempotency_result_object_id().as_bytes() != &binding.result_commitment
            || write_set_commitment != binding.write_set_commitment
            || !object_ids.contains(&binding.receipt_object_id)
            || ![
                binding.basis_object_id,
                binding.successor_snapshot_id,
                binding.successor_capacity_root_id,
                binding.capacity_debit_id,
            ]
            .into_iter()
            .chain(binding.leaf_authority_carrier_id)
            .chain(binding.leaf_authority_consumption_id)
            .all(|required| object_ids.contains(&required))
            || binding
                .mandate_object_ids
                .is_some_and(|required| !required.into_iter().all(|id| object_ids.contains(&id)))
            || (!active_objects.is_empty()
                && !current_object_ids
                    .into_iter()
                    .all(|required| active_objects.contains(&required)))
        {
            return Err(AuthorityMaterializationErrorV1::BindingMismatch);
        }
        Ok(())
    }

    pub(in crate::domain::vnext::authority) fn is_consumed(&self) -> bool {
        self.state.get() == MaterializationTransactionStateV1::Consumed
    }
}

fn freeze_owner_port<K: RepositoryActionBindingKindV1>() {
    let _ = K::ACTION;
}

fn freeze_repository_owner_ports() {
    freeze_owner_port::<CoordinationRepositoryActionBindingOwnerV1<94>>();
    freeze_owner_port::<PlanningRepositoryActionBindingOwnerV1<103>>();
    freeze_owner_port::<PersistenceRepositoryActionBindingOwnerV1<107>>();
    freeze_owner_port::<DistributionRepositoryActionBindingOwnerV1<117>>();
    freeze_owner_port::<SearchMaintenanceRepositoryActionBindingOwnerV1<130>>();
    freeze_owner_port::<MemoryRepositoryActionBindingOwnerV1<132>>();
    freeze_owner_port::<IntakeRepositoryActionBindingOwnerV1<139>>();
    freeze_owner_port::<ResearchRepositoryActionBindingOwnerV1<142>>();
}

pub(in crate::domain::vnext::authority) struct VerifiedSchedulingPolicyDowngradeMandateUseV1<'tx> {
    facts: SchedulingPolicyDowngradeMandateFactsV1,
    transaction: &'tx AuthorityMaterializationTransactionV1<'tx>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::domain::vnext) struct RepositoryActionCommitFactsV1 {
    pub(crate) binding: RepositoryActionBindingFactsV1,
}

pub(in crate::domain::vnext::authority) struct ConsumedSchedulingPolicyDowngradeMandateV1<'tx> {
    _transaction: PhantomData<&'tx mut ()>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

pub(in crate::domain::vnext::authority) struct ConsumedRepositoryActionMaterializationV1<'tx> {
    _transaction: PhantomData<&'tx mut ()>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl<'tx> VerifiedSchedulingPolicyDowngradeMandateUseV1<'tx> {
    pub(in crate::domain::vnext::authority) fn consume_with_action_binding<
        K: RepositoryActionBindingKindV1,
    >(
        self,
        binding: AdmittedRepositoryActionBindingV1<'tx, K>,
        commit: RepositoryActionCommitFactsV1,
    ) -> Result<ConsumedSchedulingPolicyDowngradeMandateV1<'tx>, AuthorityMaterializationErrorV1>
    {
        if K::ACTION != SchedulingPolicyBindingOwnerV1::ACTION {
            return Err(AuthorityMaterializationErrorV1::BindingMismatch);
        }
        validate_scheduling_policy_binding(binding.facts, binding.admission, true)?;
        if !std::ptr::eq(self.transaction, binding.transaction)
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
            || self.facts.authority_basis_commitment != commit.binding.authority_basis_commitment
            || self.facts.currentness_commitment != commit.binding.currentness_commitment
            || self.facts.revocation_commitment != commit.binding.revocation_commitment
            || self.facts.authorization_receipt_id != commit.binding.receipt_id
            || self.facts.normalized_witness_commitment
                != commit.binding.normalized_witness_commitment
            || self.facts.mandate_atom_commitment != commit.binding.supplemental_mandate_atom
            || self.facts.debit_map_commitment != commit.binding.debit_map_commitment
            || self.facts.root_use_atoms_commitment != commit.binding.root_use_atoms_commitment
            || self.facts.invocation_commitment != commit.binding.invocation_commitment
            || self.facts.successor_capacity_commitment
                != commit.binding.successor_capacity_commitment
            || self.facts.mandate_body_commitment
                != commit.binding.supplemental_mandate_body_commitment
            || self.facts.mandate_carrier_commitment
                != commit.binding.supplemental_mandate_carrier_commitment
            || self.facts.mandate_nonce_commitment
                != commit.binding.supplemental_mandate_nonce_commitment
            || self.facts.current_policy_commitment != commit.binding.current_policy_commitment
            || self.facts.candidate_policy_commitment != commit.binding.candidate_policy_commitment
            || self.facts.evaluator_commitment != commit.binding.evaluator_commitment
            || self.facts.complete_diff_commitment != commit.binding.complete_diff_commitment
            || self.facts.classifier_commitment != commit.binding.classifier_commitment
            || self.facts.classifier_revision_commitment
                != commit.binding.classifier_revision_commitment
            || self.facts.safety_floor_commitment != commit.binding.safety_floor_commitment
            || self.facts.governance_floor_commitment != commit.binding.governance_floor_commitment
            || self.facts.request_payload_commitment != commit.binding.request_payload_commitment
            || self.facts.idempotency_meaning_commitment
                != commit.binding.idempotency_meaning_commitment
            || self.facts.trust_root_commitment != commit.binding.trust_root_commitment
            || self.facts.action_request_id != commit.binding.request_id
            || self.facts.idempotency_key != commit.binding.idempotency_key
            || self.facts.idempotency_mapping_commitment
                != commit.binding.idempotency_mapping_commitment
            || self.facts.action_spec_commitment != commit.binding.action_spec_commitment
            || self.facts.subject_commitment != commit.binding.subject_commitment
            || self.facts.subject_basis_commitment != commit.binding.subject_basis_commitment
            || self.facts.exact_payload_commitment != commit.binding.exact_payload_commitment
            || self.facts.participant_set_commitment != commit.binding.participant_set_commitment
            || self.facts.owner_publication_commitment
                != commit.binding.owner_publication_commitment
            || self.facts.write_set_commitment != commit.binding.write_set_commitment
            || self.facts.output_commitment != commit.binding.output_commitment
            || self.facts.planned_debit_commitment != commit.binding.planned_debit_commitment
            || self.facts.planned_consumption_commitment
                != commit.binding.planned_consumption_commitment
            || self.facts.result_commitment != commit.binding.result_commitment
            || Some(self.facts.mandate_id) != commit.binding.supplemental_mandate_id
            || self.facts.mandate_schema_version
                != commit.binding.supplemental_mandate_schema_version
            || self.facts.valid_from != commit.binding.supplemental_mandate_valid_from
            || self.facts.valid_until != commit.binding.supplemental_mandate_valid_until
            || self.facts.revocation_revision
                != commit.binding.supplemental_mandate_revocation_revision
            || self.facts.diff_class != commit.binding.supplemental_mandate_diff_class
        {
            return Err(AuthorityMaterializationErrorV1::BindingMismatch);
        }
        match self.transaction.state.get() {
            MaterializationTransactionStateV1::BothMinted {
                mandate_atom,
                invocation,
            } if mandate_atom == self.facts.mandate_atom_commitment
                && invocation == self.facts.invocation_commitment =>
            {
                self.transaction.consumed_binding.replace(Some(
                    ConsumedRepositoryActionBindingV1::new(binding.facts, binding.admission),
                ));
                self.transaction
                    .state
                    .set(MaterializationTransactionStateV1::Consumed);
            }
            _ => return Err(AuthorityMaterializationErrorV1::DuplicateUse),
        }
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

    fn validate_owner(
        selection: RepositoryAuthoritySelectionV1,
        subject_commitment: [u8; 32],
        current_semantic_owner_basis_commitment: [u8; 32],
        exact_payload_commitment: [u8; 32],
    ) -> Result<(), AuthorityMaterializationErrorV1>;
}

macro_rules! owner_binding_kind {
    ($type:ident, $authority:ty, $first:literal..=$last:literal, $future:meta) => {
        pub(in crate::domain::vnext) struct $type<const GLOBAL_TAG: u64>;

        impl<const GLOBAL_TAG: u64> owner_sealed::Sealed for $type<GLOBAL_TAG> {}

        impl<const GLOBAL_TAG: u64> RepositoryActionBindingKindV1 for $type<GLOBAL_TAG> {
            const ACTION: RepositoryActionLeafV1 = {
                assert!(GLOBAL_TAG >= $first && GLOBAL_TAG <= $last);
                RepositoryActionLeafV1::Downstream(if GLOBAL_TAG == 105 {
                    RepositoryDownstreamActionLeafV1::PUBLISH_SCHEDULING_POLICY_BINDING
                } else {
                    RepositoryDownstreamActionLeafV1::from_catalog_index((GLOBAL_TAG - 94) as u8)
                })
            };

            fn validate_owner(
                selection: RepositoryAuthoritySelectionV1,
                subject_commitment: [u8; 32],
                current_semantic_owner_basis_commitment: [u8; 32],
                exact_payload_commitment: [u8; 32],
            ) -> Result<(), AuthorityMaterializationErrorV1> {
                Self::authority(
                    selection,
                    subject_commitment,
                    current_semantic_owner_basis_commitment,
                    exact_payload_commitment,
                )
                .map(|_| ())
                .map_err(|_| AuthorityMaterializationErrorV1::BindingMismatch)
            }
        }

        impl<const GLOBAL_TAG: u64> $type<GLOBAL_TAG> {
            pub(in crate::domain::vnext) fn authority(
                selection: RepositoryAuthoritySelectionV1,
                subject_commitment: [u8; 32],
                current_semantic_owner_basis_commitment: [u8; 32],
                exact_payload_commitment: [u8; 32],
            ) -> Result<$authority, super::facade::RepositoryLeafAuthorityErrorV1> {
                <$authority>::new(
                    selection,
                    match <Self as RepositoryActionBindingKindV1>::ACTION {
                        RepositoryActionLeafV1::Downstream(action) => action,
                        _ => unreachable!(),
                    },
                    subject_commitment,
                    current_semantic_owner_basis_commitment,
                    exact_payload_commitment,
                )
            }
        }
    };
}

owner_binding_kind!(
    CoordinationRepositoryActionBindingOwnerV1,
    CoordinationRepositoryActionAuthorityV1,
    94..=102,
    expect(
        dead_code,
        reason = "Stage 7 Coordination binding port is frozen before integration"
    )
);
owner_binding_kind!(
    PlanningRepositoryActionBindingOwnerV1,
    PlanningRepositoryActionAuthorityV1,
    103..=106,
    cfg(all())
);
owner_binding_kind!(
    PersistenceRepositoryActionBindingOwnerV1,
    PersistenceRepositoryActionAuthorityV1,
    107..=116,
    expect(
        dead_code,
        reason = "Stage 7 Persistence binding port is frozen before integration"
    )
);
owner_binding_kind!(
    DistributionRepositoryActionBindingOwnerV1,
    DistributionRepositoryActionAuthorityV1,
    117..=129,
    expect(
        dead_code,
        reason = "Stage 7 Distribution binding port is frozen before integration"
    )
);
owner_binding_kind!(
    SearchMaintenanceRepositoryActionBindingOwnerV1,
    SearchMaintenanceRepositoryActionAuthorityV1,
    130..=131,
    expect(
        dead_code,
        reason = "Stage 7 Search binding port is frozen before integration"
    )
);
owner_binding_kind!(
    MemoryRepositoryActionBindingOwnerV1,
    MemoryRepositoryActionAuthorityV1,
    132..=138,
    expect(
        dead_code,
        reason = "Stage 7 Memory binding port is frozen before integration"
    )
);
owner_binding_kind!(
    IntakeRepositoryActionBindingOwnerV1,
    IntakeRepositoryActionAuthorityV1,
    139..=141,
    expect(
        dead_code,
        reason = "Stage 7 Intake binding port is frozen before integration"
    )
);
owner_binding_kind!(
    ResearchRepositoryActionBindingOwnerV1,
    ResearchRepositoryActionAuthorityV1,
    142..=145,
    expect(
        dead_code,
        reason = "Stage 7 Research binding port is frozen before integration"
    )
);

pub(in crate::domain::vnext) type SchedulingPolicyBindingOwnerV1 =
    PlanningRepositoryActionBindingOwnerV1<105>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::domain::vnext) struct RepositoryActionBindingFactsV1 {
    policy_meaning: SchedulingPolicyMeaningV1,
    pub(crate) authority_selection: RepositoryAuthoritySelectionV1,
    pub(crate) request_id: ActionRequestIdV1,
    pub(crate) action: RepositoryActionLeafV1,
    pub(crate) principal_id: PrincipalIdV1,
    pub(crate) binding_id: PrincipalBindingIdV1,
    pub(crate) session_id: SessionIdV1,
    pub(crate) repository_generation_id: StoreGenerationIdV1,
    pub(crate) authority_context_id: AuthorityContextIdV1,
    pub(crate) state_token_id: StateTokenIdV1,
    pub(crate) authority_epoch: u64,
    pub(crate) trusted_time: u64,
    pub(crate) subject_commitment: [u8; 32],
    pub(crate) subject_basis_commitment: [u8; 32],
    pub(crate) exact_payload_commitment: [u8; 32],
    pub(crate) action_spec_commitment: [u8; 32],
    pub(crate) repository_commitment: [u8; 32],
    pub(crate) store_instance_commitment: [u8; 32],
    pub(crate) head_commitment: [u8; 32],
    pub(crate) expected_old_owner_state_commitment: [u8; 32],
    pub(crate) participant_set_commitment: [u8; 32],
    pub(crate) owner_publication_commitment: [u8; 32],
    pub(crate) write_set_commitment: [u8; 32],
    pub(crate) output_commitment: [u8; 32],
    pub(crate) authority_basis_commitment: [u8; 32],
    pub(crate) authority_snapshot_commitment: [u8; 32],
    pub(crate) authority_fence_commitment: [u8; 32],
    pub(crate) currentness_commitment: [u8; 32],
    pub(crate) revocation_commitment: [u8; 32],
    pub(crate) normalized_witness_commitment: [u8; 32],
    pub(crate) debit_map_commitment: [u8; 32],
    pub(crate) root_use_atoms_commitment: [u8; 32],
    pub(crate) supplemental_mandate_atom: [u8; 32],
    pub(crate) planned_debit_commitment: [u8; 32],
    pub(crate) planned_consumption_commitment: [u8; 32],
    pub(crate) idempotency_key: IdempotencyKeyIdV1,
    pub(crate) idempotency_mapping_commitment: [u8; 32],
    pub(crate) successor_capacity_commitment: [u8; 32],
    pub(crate) receipt_id: AuthorizationReceiptIdV1,
    pub(crate) authorization_receipt_object_commitment: [u8; 32],
    pub(crate) basis_object_commitment: [u8; 32],
    pub(crate) current_snapshot_object_commitment: [u8; 32],
    pub(crate) successor_snapshot_object_commitment: [u8; 32],
    pub(crate) current_capacity_root_object_commitment: [u8; 32],
    pub(crate) successor_capacity_root_object_commitment: [u8; 32],
    pub(crate) capacity_debit_object_commitment: [u8; 32],
    pub(crate) leaf_authority_carrier_object_commitment: Option<[u8; 32]>,
    pub(crate) leaf_authority_consumption_object_commitment: Option<[u8; 32]>,
    pub(crate) guard_object_commitment: [u8; 32],
    pub(crate) state_object_commitment: [u8; 32],
    pub(crate) result_commitment: [u8; 32],
    pub(crate) invocation_commitment: [u8; 32],
    pub(crate) supplemental_mandate_body_commitment: [u8; 32],
    pub(crate) supplemental_mandate_carrier_commitment: [u8; 32],
    pub(crate) supplemental_mandate_nonce_commitment: [u8; 32],
    pub(crate) current_policy_commitment: [u8; 32],
    pub(crate) candidate_policy_commitment: [u8; 32],
    pub(crate) evaluator_commitment: [u8; 32],
    pub(crate) complete_diff_commitment: [u8; 32],
    pub(crate) classifier_commitment: [u8; 32],
    pub(crate) classifier_revision_commitment: [u8; 32],
    pub(crate) safety_floor_commitment: [u8; 32],
    pub(crate) governance_floor_commitment: [u8; 32],
    pub(crate) request_payload_commitment: [u8; 32],
    pub(crate) idempotency_meaning_commitment: [u8; 32],
    pub(crate) trust_root_commitment: [u8; 32],
    pub(crate) supplemental_mandate_id: Option<MandateIdV1>,
    pub(crate) supplemental_mandate_schema_version: u64,
    pub(crate) supplemental_mandate_valid_from: u64,
    pub(crate) supplemental_mandate_valid_until: u64,
    pub(crate) supplemental_mandate_revocation_revision: u64,
    pub(crate) supplemental_mandate_diff_class: SchedulingPolicyDiffClassV1,
}

pub(in crate::domain::vnext::authority) struct AdmittedRepositoryActionBindingV1<'tx, K> {
    facts: RepositoryActionBindingFactsV1,
    admission: MaterializationAuthorityAdmissionV1,
    transaction: &'tx AuthorityMaterializationTransactionV1<'tx>,
    _transaction: PhantomData<&'tx mut K>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl<'tx, K: RepositoryActionBindingKindV1> AdmittedRepositoryActionBindingV1<'tx, K> {
    fn validate_facts(
        facts: RepositoryActionBindingFactsV1,
    ) -> Result<(), AuthorityMaterializationErrorV1> {
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
            facts.authorization_receipt_object_commitment,
            facts.basis_object_commitment,
            facts.current_snapshot_object_commitment,
            facts.successor_snapshot_object_commitment,
            facts.current_capacity_root_object_commitment,
            facts.successor_capacity_root_object_commitment,
            facts.capacity_debit_object_commitment,
            facts.guard_object_commitment,
            facts.state_object_commitment,
            facts.result_commitment,
            facts.invocation_commitment,
            facts.supplemental_mandate_body_commitment,
            facts.supplemental_mandate_carrier_commitment,
            facts.supplemental_mandate_nonce_commitment,
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
            facts.trust_root_commitment,
        ];
        let supplemental_valid = if facts.supplemental_mandate_id.is_some() {
            facts
                .supplemental_mandate_diff_class
                .requires_downgrade_mandate()
                && facts.supplemental_mandate_schema_version > 0
                && facts.supplemental_mandate_revocation_revision > 0
                && facts.supplemental_mandate_valid_from < facts.supplemental_mandate_valid_until
                && (facts.supplemental_mandate_valid_from..facts.supplemental_mandate_valid_until)
                    .contains(&facts.trusted_time)
        } else {
            !facts
                .supplemental_mandate_diff_class
                .requires_downgrade_mandate()
                && facts.supplemental_mandate_schema_version == 0
                && facts.supplemental_mandate_revocation_revision == 0
                && facts.supplemental_mandate_valid_from == 0
                && facts.supplemental_mandate_valid_until == 0
        };
        if facts.action != K::ACTION
            || commitments.contains(&[0; 32])
            || facts
                .leaf_authority_carrier_object_commitment
                .is_some_and(|value| value == [0; 32])
            || facts
                .leaf_authority_consumption_object_commitment
                .is_some_and(|value| value == [0; 32])
            || facts.authority_epoch == 0
            || facts.trusted_time == 0
            || facts.authority_selection.actor_binding_id() != facts.binding_id
            || facts.authority_selection.actor_session_id() != facts.session_id
            || !supplemental_valid
        {
            return Err(AuthorityMaterializationErrorV1::BindingMismatch);
        }
        K::validate_owner(
            facts.authority_selection,
            facts.subject_commitment,
            facts.subject_basis_commitment,
            facts.exact_payload_commitment,
        )?;
        Ok(())
    }
}

fn validate_repository_action_binding<K: RepositoryActionBindingKindV1>(
    facts: RepositoryActionBindingFactsV1,
    admission: MaterializationAuthorityAdmissionV1,
) -> Result<(), AuthorityMaterializationErrorV1> {
    AdmittedRepositoryActionBindingV1::<K>::validate_facts(facts)?;
    if admission.request_id != facts.request_id
        || admission.action != facts.action
        || admission.receipt_id != facts.receipt_id
        || admission.authority_epoch != facts.authority_epoch
        || admission.accepted_h_time != facts.trusted_time
        || admission.state_token != facts.state_token_id
        || admission.basis_object_id.as_bytes() != &facts.basis_object_commitment
        || admission.current_snapshot_id.as_bytes() != &facts.current_snapshot_object_commitment
        || admission.successor_snapshot_id.as_bytes() != &facts.successor_snapshot_object_commitment
        || admission.current_capacity_root_id.as_bytes()
            != &facts.current_capacity_root_object_commitment
        || admission.successor_capacity_root_id.as_bytes()
            != &facts.successor_capacity_root_object_commitment
        || admission.capacity_debit_id.as_bytes() != &facts.capacity_debit_object_commitment
        || admission.leaf_authority_carrier_id.map(|id| *id.as_bytes())
            != facts.leaf_authority_carrier_object_commitment
        || admission
            .leaf_authority_consumption_id
            .map(|id| *id.as_bytes())
            != facts.leaf_authority_consumption_object_commitment
        || admission.guard_object_id.as_bytes() != &facts.guard_object_commitment
        || admission.state_object_id.as_bytes() != &facts.state_object_commitment
    {
        return Err(AuthorityMaterializationErrorV1::BindingMismatch);
    }
    Ok(())
}

fn validate_scheduling_policy_binding(
    facts: RepositoryActionBindingFactsV1,
    admission: MaterializationAuthorityAdmissionV1,
    mandate_required: bool,
) -> Result<(), AuthorityMaterializationErrorV1> {
    validate_repository_action_binding::<SchedulingPolicyBindingOwnerV1>(facts, admission)?;
    let derived = derive_policy_relation(facts.policy_meaning)?;
    if mandate_required != derived.requires_downgrade_mandate()
        || derived != facts.supplemental_mandate_diff_class
        || mandate_required != facts.supplemental_mandate_id.is_some()
        || !policy_commitments_match(
            facts.policy_meaning,
            facts.current_policy_commitment,
            facts.candidate_policy_commitment,
            facts.evaluator_commitment,
            facts.complete_diff_commitment,
            facts.classifier_commitment,
            facts.classifier_revision_commitment,
            facts.safety_floor_commitment,
            facts.governance_floor_commitment,
        )
    {
        return Err(AuthorityMaterializationErrorV1::BindingMismatch);
    }
    Ok(())
}

fn validate_scheduling_policy_downgrade_mandate(
    facts: SchedulingPolicyDowngradeMandateFactsV1,
) -> Result<(), AuthorityMaterializationErrorV1> {
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
        facts.idempotency_mapping_commitment,
        facts.action_spec_commitment,
        facts.subject_commitment,
        facts.subject_basis_commitment,
        facts.exact_payload_commitment,
        facts.participant_set_commitment,
        facts.owner_publication_commitment,
        facts.write_set_commitment,
        facts.output_commitment,
        facts.planned_debit_commitment,
        facts.planned_consumption_commitment,
        facts.result_commitment,
        facts.invocation_commitment,
        facts.authority_snapshot_commitment,
        facts.authority_fence_commitment,
        facts.authority_basis_commitment,
        facts.currentness_commitment,
        facts.revocation_commitment,
        facts.trust_root_commitment,
        facts.normalized_witness_commitment,
        facts.debit_map_commitment,
        facts.root_use_atoms_commitment,
        facts.mandate_atom_commitment,
        facts.successor_capacity_commitment,
    ];
    let derived = derive_policy_relation(facts.policy_meaning)?;
    if !derived.requires_downgrade_mandate()
        || derived != facts.diff_class
        || !policy_commitments_match(
            facts.policy_meaning,
            facts.current_policy_commitment,
            facts.candidate_policy_commitment,
            facts.evaluator_commitment,
            facts.complete_diff_commitment,
            facts.classifier_commitment,
            facts.classifier_revision_commitment,
            facts.safety_floor_commitment,
            facts.governance_floor_commitment,
        )
        || commitments.contains(&[0; 32])
        || facts.current_policy_commitment == facts.candidate_policy_commitment
        || facts.mandate_schema_version == 0
        || facts.authority_epoch == 0
        || facts.revocation_revision == 0
        || facts.valid_from >= facts.valid_until
        || !(facts.valid_from..facts.valid_until).contains(&facts.trusted_time)
    {
        return Err(AuthorityMaterializationErrorV1::InvalidMandate);
    }
    Ok(())
}

fn policy_commitment(domain: &[u8], rows: &[u64]) -> [u8; 32] {
    use sha2::{Digest, Sha256};

    let mut writer = Sha256::new();
    writer.update(domain);
    writer.update((rows.len() as u64).to_be_bytes());
    for row in rows {
        writer.update(row.to_be_bytes());
    }
    writer.finalize().into()
}

fn derive_policy_relation(
    meaning: SchedulingPolicyMeaningV1,
) -> Result<SchedulingPolicyDiffClassV1, AuthorityMaterializationErrorV1> {
    if meaning.evaluator_revision == 0
        || meaning.classifier_revision == 0
        || meaning
            .candidate_rules
            .iter()
            .zip(meaning.safety_floor)
            .any(|(candidate, floor)| *candidate < floor)
        || meaning
            .candidate_rules
            .iter()
            .zip(meaning.governance_floor)
            .any(|(candidate, floor)| *candidate < floor)
    {
        return Err(AuthorityMaterializationErrorV1::InvalidMandate);
    }
    let greater = meaning
        .candidate_rules
        .iter()
        .zip(meaning.current_rules)
        .any(|(candidate, current)| *candidate > current);
    let lower = meaning
        .candidate_rules
        .iter()
        .zip(meaning.current_rules)
        .any(|(candidate, current)| *candidate < current);
    Ok(match (greater, lower) {
        (false, false) => SchedulingPolicyDiffClassV1::Equivalent,
        (true, false) => SchedulingPolicyDiffClassV1::Strengthening,
        (false, true) => SchedulingPolicyDiffClassV1::Weakening,
        (true, true) => SchedulingPolicyDiffClassV1::Incomparable,
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "the closed Authority policy tuple is intentionally compared field-for-field"
)]
fn policy_commitments_match(
    meaning: SchedulingPolicyMeaningV1,
    current: [u8; 32],
    candidate: [u8; 32],
    evaluator: [u8; 32],
    complete_diff: [u8; 32],
    classifier: [u8; 32],
    classifier_revision: [u8; 32],
    safety_floor: [u8; 32],
    governance_floor: [u8; 32],
) -> bool {
    current
        == policy_commitment(
            b"maestro.authority.scheduling-current-policy.v1\0",
            &meaning.current_rules,
        )
        && candidate
            == policy_commitment(
                b"maestro.authority.scheduling-candidate-policy.v1\0",
                &meaning.candidate_rules,
            )
        && evaluator
            == policy_commitment(
                b"maestro.authority.scheduling-evaluator.v1\0",
                &[meaning.evaluator_revision],
            )
        && complete_diff
            == policy_commitment(
                b"maestro.authority.scheduling-complete-diff.v1\0",
                &[
                    meaning.current_rules.as_slice(),
                    meaning.candidate_rules.as_slice(),
                ]
                .concat(),
            )
        && classifier
            == policy_commitment(
                b"maestro.authority.scheduling-classifier.v1\0",
                &[meaning.classifier_revision],
            )
        && classifier_revision
            == policy_commitment(
                b"maestro.authority.scheduling-classifier-revision.v1\0",
                &[meaning.classifier_revision],
            )
        && safety_floor
            == policy_commitment(
                b"maestro.authority.scheduling-safety-floor.v1\0",
                &meaning.safety_floor,
            )
        && governance_floor
            == policy_commitment(
                b"maestro.authority.scheduling-governance-floor.v1\0",
                &meaning.governance_floor,
            )
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum AuthorityMaterializationErrorV1 {
    #[error("scheduling-policy downgrade Mandate is unavailable")]
    InvalidMandate,
    #[error("repository Action binding does not match the admitted transaction")]
    BindingMismatch,
    #[error("materialization transaction capability was already minted or consumed")]
    DuplicateUse,
    #[error("materialization transaction is not bound to the active Repository generation")]
    StoreCurrentness,
    #[error("materialization transaction did not atomically consume its exact Authority inputs")]
    IncompleteTransaction,
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

    fn policy_meaning() -> SchedulingPolicyMeaningV1 {
        SchedulingPolicyMeaningV1 {
            current_rules: [5, 5, 5, 5],
            candidate_rules: [4, 4, 4, 4],
            safety_floor: [1, 1, 1, 1],
            governance_floor: [2, 2, 2, 2],
            evaluator_revision: 3,
            classifier_revision: 4,
        }
    }

    fn facts() -> SchedulingPolicyDowngradeMandateFactsV1 {
        let policy_meaning = policy_meaning();
        SchedulingPolicyDowngradeMandateFactsV1 {
            policy_meaning,
            mandate_id: MandateIdV1::derive("materialization-mandate").unwrap(),
            action_request_id: request(),
            idempotency_key: IdempotencyKeyIdV1::derive("materialization-idempotency").unwrap(),
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
            current_policy_commitment: policy_commitment(
                b"maestro.authority.scheduling-current-policy.v1\0",
                &policy_meaning.current_rules,
            ),
            candidate_policy_commitment: policy_commitment(
                b"maestro.authority.scheduling-candidate-policy.v1\0",
                &policy_meaning.candidate_rules,
            ),
            evaluator_commitment: policy_commitment(
                b"maestro.authority.scheduling-evaluator.v1\0",
                &[policy_meaning.evaluator_revision],
            ),
            complete_diff_commitment: policy_commitment(
                b"maestro.authority.scheduling-complete-diff.v1\0",
                &[
                    policy_meaning.current_rules.as_slice(),
                    policy_meaning.candidate_rules.as_slice(),
                ]
                .concat(),
            ),
            classifier_commitment: policy_commitment(
                b"maestro.authority.scheduling-classifier.v1\0",
                &[policy_meaning.classifier_revision],
            ),
            classifier_revision_commitment: policy_commitment(
                b"maestro.authority.scheduling-classifier-revision.v1\0",
                &[policy_meaning.classifier_revision],
            ),
            safety_floor_commitment: policy_commitment(
                b"maestro.authority.scheduling-safety-floor.v1\0",
                &policy_meaning.safety_floor,
            ),
            governance_floor_commitment: policy_commitment(
                b"maestro.authority.scheduling-governance-floor.v1\0",
                &policy_meaning.governance_floor,
            ),
            request_payload_commitment: [16; 32],
            idempotency_meaning_commitment: [17; 32],
            idempotency_mapping_commitment: [43; 32],
            action_spec_commitment: [33; 32],
            subject_commitment: [30; 32],
            subject_basis_commitment: [31; 32],
            exact_payload_commitment: [32; 32],
            participant_set_commitment: [34; 32],
            owner_publication_commitment: [35; 32],
            write_set_commitment: [36; 32],
            output_commitment: [37; 32],
            planned_debit_commitment: [41; 32],
            planned_consumption_commitment: [42; 32],
            result_commitment: [44; 32],
            invocation_commitment: [18; 32],
            authority_snapshot_commitment: [19; 32],
            authority_fence_commitment: [20; 32],
            authority_basis_commitment: [38; 32],
            currentness_commitment: [39; 32],
            revocation_commitment: [40; 32],
            authorization_receipt_id: AuthorizationReceiptIdV1::derive("materialization-receipt")
                .unwrap(),
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
            policy_meaning: facts().policy_meaning,
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
            authorization_receipt_object_commitment: [50; 32],
            basis_object_commitment: [51; 32],
            current_snapshot_object_commitment: [52; 32],
            successor_snapshot_object_commitment: [53; 32],
            current_capacity_root_object_commitment: [54; 32],
            successor_capacity_root_object_commitment: [55; 32],
            capacity_debit_object_commitment: [56; 32],
            leaf_authority_carrier_object_commitment: Some([57; 32]),
            leaf_authority_consumption_object_commitment: Some([58; 32]),
            guard_object_commitment: [59; 32],
            state_object_commitment: [60; 32],
            result_commitment: [44; 32],
            invocation_commitment: facts().invocation_commitment,
            supplemental_mandate_body_commitment: facts().mandate_body_commitment,
            supplemental_mandate_carrier_commitment: facts().mandate_carrier_commitment,
            supplemental_mandate_nonce_commitment: facts().mandate_nonce_commitment,
            current_policy_commitment: facts().current_policy_commitment,
            candidate_policy_commitment: facts().candidate_policy_commitment,
            evaluator_commitment: facts().evaluator_commitment,
            complete_diff_commitment: facts().complete_diff_commitment,
            classifier_commitment: facts().classifier_commitment,
            classifier_revision_commitment: facts().classifier_revision_commitment,
            safety_floor_commitment: facts().safety_floor_commitment,
            governance_floor_commitment: facts().governance_floor_commitment,
            request_payload_commitment: facts().request_payload_commitment,
            idempotency_meaning_commitment: facts().idempotency_meaning_commitment,
            trust_root_commitment: facts().trust_root_commitment,
            supplemental_mandate_id: Some(facts().mandate_id),
            supplemental_mandate_schema_version: facts().mandate_schema_version,
            supplemental_mandate_valid_from: facts().valid_from,
            supplemental_mandate_valid_until: facts().valid_until,
            supplemental_mandate_revocation_revision: facts().revocation_revision,
            supplemental_mandate_diff_class: facts().diff_class,
        }
    }

    fn binding_facts_without_mandate(
        diff_class: SchedulingPolicyDiffClassV1,
    ) -> RepositoryActionBindingFactsV1 {
        let mut facts = binding_facts();
        facts.policy_meaning.candidate_rules = match diff_class {
            SchedulingPolicyDiffClassV1::Equivalent => facts.policy_meaning.current_rules,
            SchedulingPolicyDiffClassV1::Strengthening => [6, 6, 6, 6],
            SchedulingPolicyDiffClassV1::Weakening => [4, 4, 4, 4],
            SchedulingPolicyDiffClassV1::Incomparable => [6, 4, 6, 4],
        };
        facts.current_policy_commitment = policy_commitment(
            b"maestro.authority.scheduling-current-policy.v1\0",
            &facts.policy_meaning.current_rules,
        );
        facts.candidate_policy_commitment = policy_commitment(
            b"maestro.authority.scheduling-candidate-policy.v1\0",
            &facts.policy_meaning.candidate_rules,
        );
        facts.complete_diff_commitment = policy_commitment(
            b"maestro.authority.scheduling-complete-diff.v1\0",
            &[
                facts.policy_meaning.current_rules.as_slice(),
                facts.policy_meaning.candidate_rules.as_slice(),
            ]
            .concat(),
        );
        facts.supplemental_mandate_id = None;
        facts.supplemental_mandate_schema_version = 0;
        facts.supplemental_mandate_valid_from = 0;
        facts.supplemental_mandate_valid_until = 0;
        facts.supplemental_mandate_revocation_revision = 0;
        facts.supplemental_mandate_diff_class = diff_class;
        facts.supplemental_mandate_atom = NO_SUPPLEMENTAL_MANDATE_COMMITMENT_V1;
        facts.supplemental_mandate_body_commitment = NO_SUPPLEMENTAL_MANDATE_COMMITMENT_V1;
        facts.supplemental_mandate_carrier_commitment = NO_SUPPLEMENTAL_MANDATE_COMMITMENT_V1;
        facts.supplemental_mandate_nonce_commitment = NO_SUPPLEMENTAL_MANDATE_COMMITMENT_V1;
        facts
    }

    fn test_admission(
        binding: RepositoryActionBindingFactsV1,
    ) -> MaterializationAuthorityAdmissionV1 {
        MaterializationAuthorityAdmissionV1 {
            request_id: binding.request_id,
            action: binding.action,
            receipt_id: binding.receipt_id,
            authority_epoch: binding.authority_epoch,
            accepted_h_time: binding.trusted_time,
            basis_object_id: crate::domain::vnext::identity::StoreObjectIdV1::from_digest([51; 32]),
            current_snapshot_id: crate::domain::vnext::identity::StoreObjectIdV1::from_digest(
                [52; 32],
            ),
            successor_snapshot_id: crate::domain::vnext::identity::StoreObjectIdV1::from_digest(
                [53; 32],
            ),
            successor_store_generation: 2,
            current_capacity_root_id: crate::domain::vnext::identity::StoreObjectIdV1::from_digest(
                [54; 32],
            ),
            successor_capacity_root_id:
                crate::domain::vnext::identity::StoreObjectIdV1::from_digest([55; 32]),
            capacity_debit_id: crate::domain::vnext::identity::StoreObjectIdV1::from_digest(
                [56; 32],
            ),
            leaf_authority_carrier_id: Some(
                crate::domain::vnext::identity::StoreObjectIdV1::from_digest([57; 32]),
            ),
            leaf_authority_consumption_id: Some(
                crate::domain::vnext::identity::StoreObjectIdV1::from_digest([58; 32]),
            ),
            guard_object_id: crate::domain::vnext::identity::StoreObjectIdV1::from_digest([59; 32]),
            state_object_id: crate::domain::vnext::identity::StoreObjectIdV1::from_digest([60; 32]),
            state_token: binding.state_token_id,
        }
    }

    #[test]
    fn downgrade_mandate_and_action_binding_are_one_use_and_exact() {
        let transaction =
            AuthorityMaterializationTransactionV1::for_test(facts().repository_generation_id);
        let use_token = transaction.mint_mandate(facts()).unwrap();
        let binding = transaction
            .mint_binding_from_test_admission::<SchedulingPolicyBindingOwnerV1>(
                test_admission(binding_facts()),
                binding_facts(),
            )
            .unwrap();
        let commit = RepositoryActionCommitFactsV1 {
            binding: binding_facts(),
        };
        use_token
            .consume_with_action_binding(binding, commit)
            .unwrap();
        assert!(matches!(
            transaction.mint_mandate(facts()),
            Err(AuthorityMaterializationErrorV1::DuplicateUse)
        ));
        assert!(matches!(
            transaction.mint_binding_from_test_admission::<SchedulingPolicyBindingOwnerV1>(
                test_admission(binding_facts()),
                binding_facts(),
            ),
            Err(AuthorityMaterializationErrorV1::DuplicateUse)
        ));
    }

    #[test]
    fn equivalent_policy_and_cross_transaction_substitution_refuse_without_consumption() {
        let mut equivalent = facts();
        equivalent.diff_class = SchedulingPolicyDiffClassV1::Equivalent;
        let transaction =
            AuthorityMaterializationTransactionV1::for_test(facts().repository_generation_id);
        assert!(matches!(
            transaction.mint_mandate(equivalent),
            Err(AuthorityMaterializationErrorV1::InvalidMandate)
        ));

        let use_token = transaction.mint_mandate(facts()).unwrap();
        let binding = transaction
            .mint_binding_from_test_admission::<SchedulingPolicyBindingOwnerV1>(
                test_admission(binding_facts()),
                binding_facts(),
            )
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
        assert!(!transaction.is_consumed());

        for substituted in [
            {
                let mut value = binding_facts();
                value.authority_basis_commitment = [91; 32];
                value
            },
            {
                let mut value = binding_facts();
                value.currentness_commitment = [92; 32];
                value
            },
            {
                let mut value = binding_facts();
                value.revocation_commitment = [93; 32];
                value
            },
            {
                let mut value = binding_facts();
                value.receipt_id =
                    AuthorizationReceiptIdV1::derive("other-materialization-receipt").unwrap();
                value
            },
        ] {
            let transaction =
                AuthorityMaterializationTransactionV1::for_test(facts().repository_generation_id);
            let use_token = transaction.mint_mandate(facts()).unwrap();
            let binding = transaction
                .mint_binding_from_test_admission::<SchedulingPolicyBindingOwnerV1>(
                    test_admission(binding_facts()),
                    binding_facts(),
                )
                .unwrap();
            assert!(matches!(
                use_token.consume_with_action_binding(
                    binding,
                    RepositoryActionCommitFactsV1 {
                        binding: substituted
                    }
                ),
                Err(AuthorityMaterializationErrorV1::BindingMismatch)
            ));
            assert!(!transaction.is_consumed());
        }

        for diff_class in [
            SchedulingPolicyDiffClassV1::Strengthening,
            SchedulingPolicyDiffClassV1::Incomparable,
        ] {
            let mut variant = facts();
            variant.diff_class = diff_class;
            let transaction =
                AuthorityMaterializationTransactionV1::for_test(facts().repository_generation_id);
            assert!(matches!(
                transaction.mint_mandate(variant),
                Err(AuthorityMaterializationErrorV1::InvalidMandate)
            ));
        }
    }

    #[test]
    fn wrong_owner_action_cannot_enter_the_scheduling_binding() {
        let mut facts = binding_facts();
        facts.action = RepositoryActionLeafV1::CreateDraftWork;
        let transaction =
            AuthorityMaterializationTransactionV1::for_test(facts.repository_generation_id);
        assert!(matches!(
            transaction.mint_binding_from_test_admission::<SchedulingPolicyBindingOwnerV1>(
                test_admission(facts),
                facts,
            ),
            Err(AuthorityMaterializationErrorV1::BindingMismatch)
        ));
    }

    #[test]
    fn equivalent_and_strengthening_use_the_distinct_no_mandate_path() {
        for diff_class in [
            SchedulingPolicyDiffClassV1::Equivalent,
            SchedulingPolicyDiffClassV1::Strengthening,
        ] {
            let facts = binding_facts_without_mandate(diff_class);
            let transaction =
                AuthorityMaterializationTransactionV1::for_test(facts.repository_generation_id);
            let binding = transaction
                .mint_binding_from_test_admission::<SchedulingPolicyBindingOwnerV1>(
                    test_admission(facts),
                    facts,
                )
                .unwrap();
            transaction
                .consume_binding_without_mandate(
                    binding,
                    RepositoryActionCommitFactsV1 { binding: facts },
                )
                .unwrap();
            assert!(transaction.is_consumed());
        }

        let facts = binding_facts_without_mandate(SchedulingPolicyDiffClassV1::Weakening);
        let transaction =
            AuthorityMaterializationTransactionV1::for_test(facts.repository_generation_id);
        assert!(matches!(
            transaction.mint_binding_from_test_admission::<SchedulingPolicyBindingOwnerV1>(
                test_admission(facts),
                facts,
            ),
            Err(AuthorityMaterializationErrorV1::BindingMismatch)
        ));
    }

    #[test]
    fn every_frozen_repository_owner_kind_is_nominally_reachable() {
        macro_rules! assert_owner {
            ($owner:ty) => {
                assert!(<$owner>::authority(selection(), [30; 32], [31; 32], [32; 32]).is_ok());
            };
        }

        assert_owner!(CoordinationRepositoryActionBindingOwnerV1<94>);
        assert_owner!(PlanningRepositoryActionBindingOwnerV1<103>);
        assert_owner!(PersistenceRepositoryActionBindingOwnerV1<107>);
        assert_owner!(DistributionRepositoryActionBindingOwnerV1<117>);
        assert_owner!(SearchMaintenanceRepositoryActionBindingOwnerV1<130>);
        assert_owner!(MemoryRepositoryActionBindingOwnerV1<132>);
        assert_owner!(IntakeRepositoryActionBindingOwnerV1<139>);
        assert_owner!(ResearchRepositoryActionBindingOwnerV1<142>);
    }
}
