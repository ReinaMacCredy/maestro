use std::cell::Cell;
use std::marker::PhantomData;
use std::rc::Rc;

use sha2::{Digest, Sha256};
use thiserror::Error;

use super::facade::MaterializationAuthorityAdmissionV1;
use super::facade::{StoreGenerationV1, StoreObjectV1};
use super::governance_floor::{
    RepositoryGovernanceFloorCurrentViewV1, RepositoryGovernanceFloorErrorV1,
};
use super::materialization::{
    AuthorityMaterializationErrorV1, SchedulingPolicyDiffClassV1, SchedulingPolicyMeaningV1,
    derive_policy_relation,
};
use crate::domain::identity::StoreObjectIdV1;
use crate::domain::planning::{SchedulingPolicySnapshotV1, SchedulingSafetyFloorV1};
use crate::foundation::core::deterministic_cbor::{self, CborValue};

#[derive(Clone)]
pub(in crate::domain) struct PlanningSchedulingPolicyInputV1 {
    candidate_policy: SchedulingPolicySnapshotV1,
    scheduling_safety_floor: SchedulingSafetyFloorV1,
}

impl PlanningSchedulingPolicyInputV1 {
    pub(super) fn from_stage7_planning(
        candidate_policy: &SchedulingPolicySnapshotV1,
        scheduling_safety_floor: &SchedulingSafetyFloorV1,
    ) -> Result<Self, GovernanceAttestationErrorV1> {
        if candidate_policy.strength() == [0; 4] || scheduling_safety_floor.strength() == [0; 4] {
            return Err(GovernanceAttestationErrorV1::InvalidPlanningInput);
        }
        Ok(Self {
            candidate_policy: candidate_policy.clone(),
            scheduling_safety_floor: scheduling_safety_floor.clone(),
        })
    }

    pub(super) const fn candidate_policy(&self) -> &SchedulingPolicySnapshotV1 {
        &self.candidate_policy
    }

    pub(super) const fn scheduling_safety_floor(&self) -> &SchedulingSafetyFloorV1 {
        &self.scheduling_safety_floor
    }
}

pub(super) struct SchedulingPolicyBindingResolutionV1 {
    current_root: Option<StoreObjectIdV1>,
    current_policy: Option<[u64; 4]>,
    candidate_relation: SchedulingPolicyDiffClassV1,
}

impl SchedulingPolicyBindingResolutionV1 {
    pub(super) const fn current_root(&self) -> Option<StoreObjectIdV1> {
        self.current_root
    }

    pub(super) const fn current_policy(&self) -> Option<[u64; 4]> {
        self.current_policy
    }

    pub(super) const fn candidate_relation(&self) -> SchedulingPolicyDiffClassV1 {
        self.candidate_relation
    }
}

struct ParsedSchedulingPolicyBindingV1 {
    revision: u64,
    expected_old_binding_hash: Option<[u8; 32]>,
    policy_rules: [u64; 4],
    policy_hash: [u8; 32],
    old_policy_hash: Option<[u8; 32]>,
    relation: SchedulingPolicyDiffClassV1,
    semantic_hash: [u8; 32],
}

pub(super) fn resolve_scheduling_policy_binding(
    generation: &StoreGenerationV1,
    active_objects: &[StoreObjectV1],
    candidate_object: &StoreObjectV1,
    planning: &PlanningSchedulingPolicyInputV1,
) -> Result<SchedulingPolicyBindingResolutionV1, GovernanceAttestationErrorV1> {
    let candidate = parse_scheduling_policy_binding(candidate_object)?;
    if candidate.policy_rules != planning.candidate_policy().strength()
        || candidate.policy_hash != planning.candidate_policy().semantic_hash()
    {
        return Err(GovernanceAttestationErrorV1::InvalidPlanningInput);
    }
    let parseable_roots = generation
        .roots()
        .iter()
        .filter_map(|root| {
            active_objects
                .iter()
                .find(|object| object.id() == *root)
                .filter(|object| object.schema_id() == candidate_object.schema_id())
                .and_then(|object| {
                    parse_scheduling_policy_binding(object)
                        .ok()
                        .map(|binding| (*root, binding))
                })
        })
        .collect::<Vec<_>>();
    let (current_root, current_policy) = match parseable_roots.as_slice() {
        [] if candidate.revision == 1
            && candidate.expected_old_binding_hash.is_none()
            && candidate.old_policy_hash.is_none()
            && candidate_object.references().is_empty() =>
        {
            (None, None)
        }
        [(root, current)]
            if candidate.revision == current.revision.saturating_add(1)
                && candidate.expected_old_binding_hash == Some(current.semantic_hash)
                && candidate.old_policy_hash == Some(current.policy_hash)
                && candidate_object.references() == [*root] =>
        {
            (Some(*root), Some(current.policy_rules))
        }
        _ => return Err(GovernanceAttestationErrorV1::CurrentSchedulingBindingMismatch),
    };
    Ok(SchedulingPolicyBindingResolutionV1 {
        current_root,
        current_policy,
        candidate_relation: candidate.relation,
    })
}

fn parse_scheduling_policy_binding(
    object: &StoreObjectV1,
) -> Result<ParsedSchedulingPolicyBindingV1, GovernanceAttestationErrorV1> {
    let CborValue::Array(fields) = object.value() else {
        return Err(GovernanceAttestationErrorV1::InvalidSchedulingBinding);
    };
    let [
        CborValue::Text(repository_installation_ref),
        CborValue::Text(store_generation_ref),
        CborValue::Unsigned(revision),
        expected_old_binding_hash,
        CborValue::Array(policy_fields),
        CborValue::Array(diff_fields),
        CborValue::Bytes(semantic_hash),
    ] = fields.as_slice()
    else {
        return Err(GovernanceAttestationErrorV1::InvalidSchedulingBinding);
    };
    let [
        CborValue::Text(policy_ref),
        CborValue::Text(evaluator_ref),
        CborValue::Unsigned(evaluator_revision),
        CborValue::Text(core_compatibility_ref),
        CborValue::Bool(true),
        CborValue::Bool(true),
        CborValue::Unsigned(foundation_maximum_total_time),
        CborValue::Unsigned(fairness_maximum_deferral),
        CborValue::Unsigned(hysteresis_window),
        CborValue::Unsigned(overload_opportunity_limit),
        CborValue::Bytes(policy_hash),
    ] = policy_fields.as_slice()
    else {
        return Err(GovernanceAttestationErrorV1::InvalidSchedulingBinding);
    };
    let [
        old_policy_hash,
        CborValue::Bytes(candidate_policy_hash),
        CborValue::Bytes(classifier_hash),
        CborValue::Unsigned(relation_tag),
    ] = diff_fields.as_slice()
    else {
        return Err(GovernanceAttestationErrorV1::InvalidSchedulingBinding);
    };
    if repository_installation_ref.is_empty()
        || store_generation_ref.is_empty()
        || policy_ref.is_empty()
        || evaluator_ref.is_empty()
        || core_compatibility_ref.is_empty()
        || *revision == 0
        || *evaluator_revision == 0
        || [
            *foundation_maximum_total_time,
            *fairness_maximum_deferral,
            *overload_opportunity_limit,
        ]
        .contains(&0)
    {
        return Err(GovernanceAttestationErrorV1::InvalidSchedulingBinding);
    }
    let policy_hash = exact_digest(policy_hash)?;
    let candidate_policy_hash = exact_digest(candidate_policy_hash)?;
    let classifier_hash = exact_digest(classifier_hash)?;
    let semantic_hash = exact_digest(semantic_hash)?;
    let expected_old_binding_hash = optional_digest(expected_old_binding_hash)?;
    let old_policy_hash = optional_digest(old_policy_hash)?;
    if candidate_policy_hash != policy_hash
        || classifier_hash == [0; 32]
        || old_policy_hash.is_some() != expected_old_binding_hash.is_some()
        || policy_hash
            != scheduling_domain_hash(
                "maestro.vnext.scheduling-policy-snapshot.v1",
                &CborValue::Array(policy_fields[..10].to_vec()),
            )?
        || semantic_hash
            != scheduling_domain_hash(
                "maestro.vnext.scheduling-policy-binding.v1",
                &CborValue::Array(fields[..6].to_vec()),
            )?
    {
        return Err(GovernanceAttestationErrorV1::InvalidSchedulingBinding);
    }
    let relation = match relation_tag {
        1 => SchedulingPolicyDiffClassV1::Equivalent,
        2 => SchedulingPolicyDiffClassV1::Strengthening,
        3 => SchedulingPolicyDiffClassV1::Weakening,
        4 => SchedulingPolicyDiffClassV1::Incomparable,
        _ => return Err(GovernanceAttestationErrorV1::InvalidSchedulingBinding),
    };
    Ok(ParsedSchedulingPolicyBindingV1 {
        revision: *revision,
        expected_old_binding_hash,
        policy_rules: [
            u64::MAX - foundation_maximum_total_time,
            u64::MAX - fairness_maximum_deferral,
            u64::MAX - hysteresis_window,
            u64::MAX - overload_opportunity_limit,
        ],
        policy_hash,
        old_policy_hash,
        relation,
        semantic_hash,
    })
}

fn optional_digest(value: &CborValue) -> Result<Option<[u8; 32]>, GovernanceAttestationErrorV1> {
    match value {
        CborValue::Array(fields) if fields == &[CborValue::Unsigned(0)] => Ok(None),
        CborValue::Array(fields) => match fields.as_slice() {
            [CborValue::Unsigned(1), CborValue::Bytes(bytes)] => Ok(Some(exact_digest(bytes)?)),
            _ => Err(GovernanceAttestationErrorV1::InvalidSchedulingBinding),
        },
        _ => Err(GovernanceAttestationErrorV1::InvalidSchedulingBinding),
    }
}

fn exact_digest(bytes: &[u8]) -> Result<[u8; 32], GovernanceAttestationErrorV1> {
    let digest: [u8; 32] = bytes
        .try_into()
        .map_err(|_| GovernanceAttestationErrorV1::InvalidSchedulingBinding)?;
    if digest == [0; 32] {
        return Err(GovernanceAttestationErrorV1::InvalidSchedulingBinding);
    }
    Ok(digest)
}

fn scheduling_domain_hash(
    domain: &str,
    value: &CborValue,
) -> Result<[u8; 32], GovernanceAttestationErrorV1> {
    let domain = CborValue::text(domain)
        .map_err(|_| GovernanceAttestationErrorV1::InvalidSchedulingBinding)?;
    let bytes = deterministic_cbor::encode(&CborValue::Array(vec![domain, value.clone()]))
        .map_err(|_| GovernanceAttestationErrorV1::InvalidSchedulingBinding)?;
    Ok(Sha256::digest(bytes).into())
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
    direct_floor_root: crate::domain::identity::StoreObjectIdV1,
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

    pub(super) const fn direct_floor_root(&self) -> crate::domain::identity::StoreObjectIdV1 {
        self.direct_floor_root
    }
}

impl<'tx> GovernanceAttestationV1<'tx> {
    pub(super) fn derive(
        planning: PlanningSchedulingPolicyInputV1,
        current_policy: [u64; 4],
        current_view: &'tx RepositoryGovernanceFloorCurrentViewV1<'tx>,
        admission: MaterializationAuthorityAdmissionV1,
    ) -> Result<Self, GovernanceAttestationErrorV1> {
        let selection = admission
            .selection
            .ok_or(GovernanceAttestationErrorV1::AuthorityMismatch)?;
        let requirement = current_view.snapshot().action_105_requirement()?;
        if !current_view.retained_tuple_is_current()
            || planning.scheduling_safety_floor().strength()
                != current_view.scheduling_safety_floor()
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
        let policy = SchedulingPolicyMeaningV1::new(
            current_policy,
            planning.candidate_policy().strength(),
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
                &planning_commitment(&planning),
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
        current_policy: [u64; 4],
        current_view: &'tx RepositoryGovernanceFloorCurrentViewV1<'tx>,
        admission: MaterializationAuthorityAdmissionV1,
    ) -> Result<GovernanceAttestedPolicyV1<'tx>, GovernanceAttestationErrorV1> {
        let rederived = Self::derive(
            self.planning.clone(),
            current_policy,
            current_view,
            admission,
        )?;
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

fn planning_commitment(planning: &PlanningSchedulingPolicyInputV1) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"maestro.authority.planning-scheduling-policy.v1\0");
    digest.update(planning.candidate_policy().semantic_hash());
    digest.update(planning.scheduling_safety_floor().semantic_hash());
    for group in [
        planning.candidate_policy().strength(),
        planning.scheduling_safety_floor().strength(),
    ] {
        for value in group {
            digest.update(value.to_be_bytes());
        }
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
pub(in crate::domain) enum GovernanceAttestationErrorV1 {
    #[error("the Planning scheduling input is invalid")]
    InvalidPlanningInput,
    #[error("the Scheduling Binding object is not the exact canonical Planning value")]
    InvalidSchedulingBinding,
    #[error("the unique current Scheduling Binding root or expected-old lineage does not match")]
    CurrentSchedulingBindingMismatch,
    #[error("the live Authority governance view does not match the admitted Action")]
    AuthorityMismatch,
    #[error("the governance capability is stale, replayed, or substituted")]
    CapabilityMismatch,
    #[error(transparent)]
    GovernanceFloor(#[from] RepositoryGovernanceFloorErrorV1),
    #[error(transparent)]
    Materialization(#[from] AuthorityMaterializationErrorV1),
}
