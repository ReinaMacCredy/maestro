use std::cell::Cell;
use std::marker::PhantomData;
use std::rc::Rc;

use sha2::{Digest, Sha256};
use thiserror::Error;

use super::facade::MaterializationAuthorityAdmissionV1;
use super::governance_floor::{
    RepositoryGovernanceFloorCurrentViewV1, RepositoryGovernanceFloorErrorV1,
};
use super::materialization::{
    AuthorityMaterializationErrorV1, SchedulingPolicyDiffClassV1, SchedulingPolicyMeaningV1,
    derive_policy_relation,
};

#[derive(Clone, Copy)]
pub(in crate::domain::vnext) struct PlanningSchedulingPolicyInputV1 {
    current_policy: [u64; 4],
    candidate_policy: [u64; 4],
    expected_binding: [u8; 32],
    candidate_binding: [u8; 32],
    request: [u8; 32],
    payload: [u8; 32],
    idempotency_key: [u8; 32],
    idempotency_meaning: [u8; 32],
}

impl PlanningSchedulingPolicyInputV1 {
    #[expect(
        clippy::too_many_arguments,
        reason = "the Stage 7 Planning input is one closed Scheduling namespace and contains no Authority-owned governance value"
    )]
    pub(in crate::domain::vnext) fn from_stage7_planning(
        current_policy: [u64; 4],
        candidate_policy: [u64; 4],
        expected_binding: [u8; 32],
        candidate_binding: [u8; 32],
        request: [u8; 32],
        payload: [u8; 32],
        idempotency_key: [u8; 32],
        idempotency_meaning: [u8; 32],
    ) -> Result<Self, GovernanceAttestationErrorV1> {
        if [
            expected_binding,
            candidate_binding,
            request,
            payload,
            idempotency_key,
            idempotency_meaning,
        ]
        .contains(&[0; 32])
            || (current_policy == [0; 4] && expected_binding != [0xA5; 32])
            || candidate_policy == [0; 4]
        {
            return Err(GovernanceAttestationErrorV1::InvalidPlanningInput);
        }
        Ok(Self {
            current_policy,
            candidate_policy,
            expected_binding,
            candidate_binding,
            request,
            payload,
            idempotency_key,
            idempotency_meaning,
        })
    }

    pub(super) const fn current_policy(self) -> [u64; 4] {
        self.current_policy
    }

    pub(super) const fn candidate_policy(self) -> [u64; 4] {
        self.candidate_policy
    }

    pub(super) fn is_initial_policy(self) -> bool {
        self.current_policy == [0; 4] && self.expected_binding == [0xA5; 32]
    }

    pub(super) const fn expected_binding(self) -> [u8; 32] {
        self.expected_binding
    }

    pub(super) const fn candidate_binding(self) -> [u8; 32] {
        self.candidate_binding
    }

    pub(super) const fn request(self) -> [u8; 32] {
        self.request
    }

    pub(super) const fn payload(self) -> [u8; 32] {
        self.payload
    }

    pub(super) const fn idempotency_key(self) -> [u8; 32] {
        self.idempotency_key
    }

    pub(super) const fn idempotency_meaning(self) -> [u8; 32] {
        self.idempotency_meaning
    }
}

pub(super) struct GovernanceAttestationV1<'tx> {
    current_view: &'tx RepositoryGovernanceFloorCurrentViewV1<'tx>,
    planning: PlanningSchedulingPolicyInputV1,
    policy: SchedulingPolicyMeaningV1,
    relation: SchedulingPolicyDiffClassV1,
    admission_commitment: [u8; 32],
    commitment: [u8; 32],
    consumed: Cell<bool>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

pub(super) struct GovernanceAttestedPolicyV1<'tx> {
    policy: SchedulingPolicyMeaningV1,
    relation: SchedulingPolicyDiffClassV1,
    governance_commitment: [u8; 32],
    current_view_commitment: [u8; 32],
    direct_floor_root: crate::domain::vnext::identity::StoreObjectIdV1,
    _transaction: PhantomData<&'tx mut ()>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl GovernanceAttestedPolicyV1<'_> {
    pub(super) const fn policy(&self) -> SchedulingPolicyMeaningV1 {
        self.policy
    }

    pub(super) const fn relation(&self) -> SchedulingPolicyDiffClassV1 {
        self.relation
    }

    pub(super) const fn governance_commitment(&self) -> [u8; 32] {
        self.governance_commitment
    }

    pub(super) const fn current_view_commitment(&self) -> [u8; 32] {
        self.current_view_commitment
    }

    pub(super) const fn direct_floor_root(
        &self,
    ) -> crate::domain::vnext::identity::StoreObjectIdV1 {
        self.direct_floor_root
    }
}

impl<'tx> GovernanceAttestationV1<'tx> {
    pub(super) fn derive(
        planning: PlanningSchedulingPolicyInputV1,
        current_view: &'tx RepositoryGovernanceFloorCurrentViewV1<'tx>,
        admission: MaterializationAuthorityAdmissionV1,
    ) -> Result<Self, GovernanceAttestationErrorV1> {
        let selection = admission
            .selection
            .ok_or(GovernanceAttestationErrorV1::AuthorityMismatch)?;
        let requirement = current_view.snapshot().action_105_requirement()?;
        if !current_view.retained_tuple_is_current()
            || planning.request() != *admission.request_id.as_bytes()
            || planning.payload() != admission.exact_payload_commitment.unwrap_or([0; 32])
            || *admission.authority_context_id.as_bytes()
                != current_view.snapshot().authority_context()
            || admission.authority_epoch != current_view.snapshot().authority_epoch()
            || admission.principal_id.as_bytes() != &current_view.principal()
            || selection.actor_binding_id().as_bytes() != &current_view.binding()
            || selection.actor_session_id().as_bytes() != &current_view.session()
            || admission.accepted_h_time != current_view.trusted_time()
            || current_view.assurance_revision() < requirement.minimum_assurance()
        {
            return Err(GovernanceAttestationErrorV1::AuthorityMismatch);
        }
        let current_policy = if planning.is_initial_policy() {
            current_view.scheduling_safety_floor()
        } else {
            planning.current_policy()
        };
        let policy = SchedulingPolicyMeaningV1::new(
            current_policy,
            planning.candidate_policy(),
            current_view.scheduling_safety_floor(),
            current_view.snapshot().semantic_hash(),
            current_view.scheduling_evaluator_revision(),
            current_view.scheduling_classifier_revision(),
        )?;
        let relation = derive_policy_relation(policy)?;
        let admission_commitment = hash_fields(
            b"maestro.authority.governance-admission.v1\0",
            &[
                admission.request_id.as_bytes(),
                admission.principal_id.as_bytes(),
                selection.actor_binding_id().as_bytes(),
                selection.actor_session_id().as_bytes(),
                admission.current_snapshot_id.as_bytes(),
                admission.successor_snapshot_id.as_bytes(),
                admission.current_capacity_root_id.as_bytes(),
                admission.successor_capacity_root_id.as_bytes(),
                admission.capacity_debit_id.as_bytes(),
                admission.guard_object_id.as_bytes(),
                admission.state_object_id.as_bytes(),
                admission.state_token.as_bytes(),
                &admission.subject_commitment,
                &admission.subject_basis_commitment,
                &admission.exact_payload_commitment.unwrap_or([0; 32]),
            ],
        );
        let commitment = hash_fields(
            b"maestro.authority.governance-attestation.v1\0",
            &[
                &current_view.commitment(),
                &current_view.snapshot().semantic_hash(),
                &planning_commitment(planning),
                &admission_commitment,
                &policy_commitment(policy),
                &[relation as u8],
            ],
        );
        Ok(Self {
            current_view,
            planning,
            policy,
            relation,
            admission_commitment,
            commitment,
            consumed: Cell::new(false),
            _not_send_or_sync: PhantomData,
        })
    }

    pub(super) fn consume(
        self,
        current_view: &'tx RepositoryGovernanceFloorCurrentViewV1<'tx>,
        admission: MaterializationAuthorityAdmissionV1,
    ) -> Result<GovernanceAttestedPolicyV1<'tx>, GovernanceAttestationErrorV1> {
        let rederived = Self::derive(self.planning, current_view, admission)?;
        if self.consumed.replace(true)
            || current_view.commitment() != self.current_view.commitment()
            || rederived.admission_commitment != self.admission_commitment
            || rederived.commitment != self.commitment
            || rederived.policy != self.policy
            || rederived.relation != self.relation
        {
            return Err(GovernanceAttestationErrorV1::CapabilityMismatch);
        }
        Ok(GovernanceAttestedPolicyV1 {
            policy: self.policy,
            relation: self.relation,
            governance_commitment: self.commitment,
            current_view_commitment: current_view.commitment(),
            direct_floor_root: current_view.direct_root(),
            _transaction: PhantomData,
            _not_send_or_sync: PhantomData,
        })
    }
}

fn planning_commitment(planning: PlanningSchedulingPolicyInputV1) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"maestro.authority.planning-scheduling-policy.v1\0");
    for group in [planning.current_policy, planning.candidate_policy] {
        for value in group {
            digest.update(value.to_be_bytes());
        }
    }
    for field in [
        planning.expected_binding,
        planning.candidate_binding,
        planning.request,
        planning.payload,
        planning.idempotency_key,
        planning.idempotency_meaning,
    ] {
        digest.update(field);
    }
    digest.finalize().into()
}

fn policy_commitment(policy: SchedulingPolicyMeaningV1) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"maestro.authority.scheduling-policy-meaning.v1\0");
    for group in [
        policy.current_rules(),
        policy.candidate_rules(),
        policy.safety_floor(),
    ] {
        for value in group {
            digest.update(value.to_be_bytes());
        }
    }
    digest.update(policy.governance_floor_binding());
    digest.update(policy.evaluator_revision().to_be_bytes());
    digest.update(policy.classifier_revision().to_be_bytes());
    digest.finalize().into()
}

fn hash_fields(domain: &[u8], fields: &[&[u8]]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update((domain.len() as u64).to_be_bytes());
    digest.update(domain);
    for field in fields {
        digest.update((field.len() as u64).to_be_bytes());
        digest.update(field);
    }
    digest.finalize().into()
}

#[derive(Debug, Error)]
pub(in crate::domain::vnext) enum GovernanceAttestationErrorV1 {
    #[error("the Planning scheduling input is invalid")]
    InvalidPlanningInput,
    #[error("the live Authority governance view does not match the admitted Action")]
    AuthorityMismatch,
    #[error("the governance capability is stale, replayed, or substituted")]
    CapabilityMismatch,
    #[error(transparent)]
    GovernanceFloor(#[from] RepositoryGovernanceFloorErrorV1),
    #[error(transparent)]
    Materialization(#[from] AuthorityMaterializationErrorV1),
}
