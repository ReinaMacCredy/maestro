use std::cell::Cell;
use std::marker::PhantomData;
use std::rc::Rc;

use sha2::{Digest, Sha256};
use thiserror::Error;

use super::materialization::{SchedulingPolicyMeaningV1, policy_commitment};

#[derive(Clone, Copy)]
pub(in crate::domain::vnext) struct PlanningSchedulingPolicyInputV1 {
    current_policy: [u64; 4],
    candidate_policy: [u64; 4],
    safety_floor: [u64; 4],
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
        reason = "the Stage 7 Planning input is one closed typed Scheduling namespace and contains no Authority-owned governance value"
    )]
    pub(in crate::domain::vnext) fn from_stage7_planning(
        current_policy: [u64; 4],
        candidate_policy: [u64; 4],
        safety_floor: [u64; 4],
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
            || current_policy == [0; 4]
            || candidate_policy == [0; 4]
            || safety_floor == [0; 4]
        {
            return Err(GovernanceAttestationErrorV1::InvalidPlanningInput);
        }
        Ok(Self {
            current_policy,
            candidate_policy,
            safety_floor,
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

    pub(super) const fn safety_floor(self) -> [u64; 4] {
        self.safety_floor
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

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum PolicyDiffClassV1 {
    Equivalent,
    Strengthening,
    Weakening,
    Incomparable,
}

impl PolicyDiffClassV1 {
    pub(super) const fn requires_supplemental_mandate(self) -> bool {
        matches!(self, Self::Weakening | Self::Incomparable)
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) struct GovernanceOwnerSnapshotV1 {
    pub(super) repository: [u8; 32],
    pub(super) store_instance: [u8; 32],
    pub(super) activation_incarnation: [u8; 32],
    pub(super) generation: [u8; 32],
    pub(super) generation_ordinal: u64,
    pub(super) head: [u8; 32],
    pub(super) head_revision: u64,
    pub(super) authority_context: [u8; 32],
    pub(super) trust_root: [u8; 32],
    pub(super) epoch: u64,
    pub(super) state_token: [u8; 32],
    pub(super) fence: [u8; 32],
    pub(super) trusted_time: u64,
    pub(super) revocation_revision: u64,
    pub(super) governance_floor_identity: [u8; 32],
    pub(super) governance_floor_schema: [u8; 32],
    pub(super) governance_floor_version: u64,
    pub(super) governance_floor_revision: u64,
    pub(super) governance_floor_semantic_hash: [u8; 32],
    pub(super) governance_floor: [u64; 4],
    pub(super) action_requirement: [u8; 32],
    pub(super) requirement_grammar: [u8; 32],
    pub(super) requirement_evaluator: [u8; 32],
    pub(super) classifier_identity: [u8; 32],
    pub(super) classifier_semantic_hash: [u8; 32],
    pub(super) classifier_revision: u64,
    pub(super) safety_floor_identity: [u8; 32],
    pub(super) safety_floor_version: u64,
    pub(super) safety_floor_semantic_hash: [u8; 32],
    pub(super) evaluator_identity: [u8; 32],
    pub(super) evaluator_revision: u64,
    pub(super) evaluator_compatibility: [u8; 32],
    pub(super) authority_witness: [u8; 32],
    pub(super) debit_map: [u8; 32],
    pub(super) root_use_atoms: [u8; 32],
    pub(super) transaction_occurrence: [u8; 32],
}

impl GovernanceOwnerSnapshotV1 {
    fn validate(self) -> Result<(), GovernanceAttestationErrorV1> {
        if [
            self.repository,
            self.store_instance,
            self.activation_incarnation,
            self.generation,
            self.head,
            self.authority_context,
            self.trust_root,
            self.state_token,
            self.fence,
            self.governance_floor_identity,
            self.governance_floor_schema,
            self.governance_floor_semantic_hash,
            self.action_requirement,
            self.requirement_grammar,
            self.requirement_evaluator,
            self.classifier_identity,
            self.classifier_semantic_hash,
            self.safety_floor_identity,
            self.safety_floor_semantic_hash,
            self.evaluator_identity,
            self.evaluator_compatibility,
            self.authority_witness,
            self.debit_map,
            self.root_use_atoms,
            self.transaction_occurrence,
        ]
        .contains(&[0; 32])
            || [
                self.generation_ordinal,
                self.head_revision,
                self.epoch,
                self.trusted_time,
                self.revocation_revision,
                self.governance_floor_version,
                self.governance_floor_revision,
                self.classifier_revision,
                self.safety_floor_version,
                self.evaluator_revision,
            ]
            .contains(&0)
            || self.governance_floor == [0; 4]
        {
            return Err(GovernanceAttestationErrorV1::InvalidAuthorityView);
        }
        Ok(())
    }
}

pub(super) struct GovernanceAttestationV1<'tx> {
    planning: PlanningSchedulingPolicyInputV1,
    snapshot: GovernanceOwnerSnapshotV1,
    relation: PolicyDiffClassV1,
    commitment: [u8; 32],
    consumed: Cell<bool>,
    _transaction: PhantomData<&'tx mut ()>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

pub(super) struct GovernanceAttestedPolicyV1<'tx> {
    policy: SchedulingPolicyMeaningV1,
    commitment: [u8; 32],
    relation: PolicyDiffClassV1,
    _transaction: PhantomData<&'tx mut ()>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl GovernanceAttestedPolicyV1<'_> {
    pub(super) const fn policy(&self) -> SchedulingPolicyMeaningV1 {
        self.policy
    }

    pub(super) const fn relation(&self) -> PolicyDiffClassV1 {
        self.relation
    }

    pub(super) const fn commitment(&self) -> [u8; 32] {
        self.commitment
    }
}

impl<'tx> GovernanceAttestationV1<'tx> {
    pub(super) fn derive(
        planning: PlanningSchedulingPolicyInputV1,
        snapshot: GovernanceOwnerSnapshotV1,
    ) -> Result<Self, GovernanceAttestationErrorV1> {
        snapshot.validate()?;
        let relation = classify_policy(planning, snapshot)?;
        let commitment = governance_commitment(planning, snapshot, relation);
        Ok(Self {
            planning,
            snapshot,
            relation,
            commitment,
            consumed: Cell::new(false),
            _transaction: PhantomData,
            _not_send_or_sync: PhantomData,
        })
    }

    pub(super) fn consume(
        self,
        policy: SchedulingPolicyMeaningV1,
    ) -> Result<GovernanceAttestedPolicyV1<'tx>, GovernanceAttestationErrorV1> {
        if self.consumed.replace(true)
            || policy.current_rules() != self.planning.current_policy
            || policy.candidate_rules() != self.planning.candidate_policy
            || policy.safety_floor() != self.planning.safety_floor
            || policy.governance_floor() != self.snapshot.governance_floor
            || policy.evaluator_revision() != self.snapshot.evaluator_revision
            || policy.classifier_revision() != self.snapshot.classifier_revision
            || policy_commitment(
                b"maestro.authority.scheduling-safety-floor.v1\0",
                &policy.safety_floor(),
            ) != self.snapshot.safety_floor_semantic_hash
            || policy_commitment(
                b"maestro.authority.scheduling-governance-floor.v1\0",
                &policy.governance_floor(),
            ) != self.snapshot.governance_floor_semantic_hash
        {
            return Err(GovernanceAttestationErrorV1::CapabilityMismatch);
        }
        Ok(GovernanceAttestedPolicyV1 {
            policy,
            commitment: self.commitment,
            relation: self.relation,
            _transaction: PhantomData,
            _not_send_or_sync: PhantomData,
        })
    }
}

fn classify_policy(
    planning: PlanningSchedulingPolicyInputV1,
    snapshot: GovernanceOwnerSnapshotV1,
) -> Result<PolicyDiffClassV1, GovernanceAttestationErrorV1> {
    if planning
        .candidate_policy
        .iter()
        .zip(planning.safety_floor)
        .any(|(candidate, floor)| candidate < &floor)
        || planning
            .candidate_policy
            .iter()
            .zip(snapshot.governance_floor)
            .any(|(candidate, floor)| candidate < &floor)
    {
        return Err(GovernanceAttestationErrorV1::FloorViolation);
    }
    let greater = planning
        .candidate_policy
        .iter()
        .zip(planning.current_policy)
        .any(|(candidate, current)| candidate > &current);
    let lower = planning
        .candidate_policy
        .iter()
        .zip(planning.current_policy)
        .any(|(candidate, current)| candidate < &current);
    Ok(match (greater, lower) {
        (false, false) => PolicyDiffClassV1::Equivalent,
        (true, false) => PolicyDiffClassV1::Strengthening,
        (false, true) => PolicyDiffClassV1::Weakening,
        (true, true) => PolicyDiffClassV1::Incomparable,
    })
}

fn governance_commitment(
    planning: PlanningSchedulingPolicyInputV1,
    snapshot: GovernanceOwnerSnapshotV1,
    relation: PolicyDiffClassV1,
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"maestro.authority.governance-attestation.v1\0");
    for rows in [
        planning.current_policy,
        planning.candidate_policy,
        planning.safety_floor,
        snapshot.governance_floor,
    ] {
        for row in rows {
            digest.update(row.to_be_bytes());
        }
    }
    for field in [
        planning.expected_binding,
        planning.candidate_binding,
        planning.request,
        planning.payload,
        planning.idempotency_key,
        planning.idempotency_meaning,
        snapshot.repository,
        snapshot.store_instance,
        snapshot.activation_incarnation,
        snapshot.generation,
        snapshot.head,
        snapshot.authority_context,
        snapshot.trust_root,
        snapshot.state_token,
        snapshot.fence,
        snapshot.governance_floor_identity,
        snapshot.governance_floor_schema,
        snapshot.governance_floor_semantic_hash,
        snapshot.action_requirement,
        snapshot.requirement_grammar,
        snapshot.requirement_evaluator,
        snapshot.classifier_identity,
        snapshot.classifier_semantic_hash,
        snapshot.safety_floor_identity,
        snapshot.safety_floor_semantic_hash,
        snapshot.evaluator_identity,
        snapshot.evaluator_compatibility,
        snapshot.authority_witness,
        snapshot.debit_map,
        snapshot.root_use_atoms,
        snapshot.transaction_occurrence,
    ] {
        digest.update(field);
    }
    for scalar in [
        snapshot.generation_ordinal,
        snapshot.head_revision,
        snapshot.epoch,
        snapshot.trusted_time,
        snapshot.revocation_revision,
        snapshot.governance_floor_version,
        snapshot.governance_floor_revision,
        snapshot.classifier_revision,
        snapshot.safety_floor_version,
        snapshot.evaluator_revision,
    ] {
        digest.update(scalar.to_be_bytes());
    }
    digest.update([relation as u8]);
    digest.finalize().into()
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub(in crate::domain::vnext) enum GovernanceAttestationErrorV1 {
    #[error("planning input is invalid")]
    InvalidPlanningInput,
    #[error("Authority current view is invalid")]
    InvalidAuthorityView,
    #[error("policy violates a current floor")]
    FloorViolation,
    #[error("governance capability does not match the publication")]
    CapabilityMismatch,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> GovernanceOwnerSnapshotV1 {
        GovernanceOwnerSnapshotV1 {
            repository: [1; 32],
            store_instance: [2; 32],
            activation_incarnation: [3; 32],
            generation: [4; 32],
            generation_ordinal: 5,
            head: [6; 32],
            head_revision: 7,
            authority_context: [8; 32],
            trust_root: [9; 32],
            epoch: 10,
            state_token: [11; 32],
            fence: [12; 32],
            trusted_time: 13,
            revocation_revision: 14,
            governance_floor_identity: [15; 32],
            governance_floor_schema: [16; 32],
            governance_floor_version: 17,
            governance_floor_revision: 18,
            governance_floor_semantic_hash: policy_commitment(
                b"maestro.authority.scheduling-governance-floor.v1\0",
                &[2, 2, 2, 2],
            ),
            governance_floor: [2, 2, 2, 2],
            action_requirement: [19; 32],
            requirement_grammar: [20; 32],
            requirement_evaluator: [21; 32],
            classifier_identity: [22; 32],
            classifier_semantic_hash: [23; 32],
            classifier_revision: 24,
            safety_floor_identity: [25; 32],
            safety_floor_version: 26,
            safety_floor_semantic_hash: policy_commitment(
                b"maestro.authority.scheduling-safety-floor.v1\0",
                &[1, 1, 1, 1],
            ),
            evaluator_identity: [27; 32],
            evaluator_revision: 28,
            evaluator_compatibility: [29; 32],
            authority_witness: [30; 32],
            debit_map: [31; 32],
            root_use_atoms: [32; 32],
            transaction_occurrence: [33; 32],
        }
    }

    fn planning(candidate: [u64; 4]) -> PlanningSchedulingPolicyInputV1 {
        PlanningSchedulingPolicyInputV1::from_stage7_planning(
            [4; 4], candidate, [1; 4], [34; 32], [35; 32], [36; 32], [37; 32],
            [38; 32], [39; 32],
        )
        .unwrap()
    }

    #[test]
    fn authority_derives_noninterchangeable_governance_and_scheduling_namespaces() {
        let snapshot = snapshot();
        let attestation =
            GovernanceAttestationV1::derive(planning([5; 4]), snapshot).unwrap();
        let policy = SchedulingPolicyMeaningV1::new(
            [4; 4],
            [5; 4],
            [1; 4],
            [2; 4],
            snapshot.evaluator_revision,
            snapshot.classifier_revision,
        )
        .unwrap();
        let consumed = attestation.consume(policy).unwrap();
        assert_eq!(consumed.policy(), policy);
        assert_eq!(consumed.relation(), PolicyDiffClassV1::Strengthening);
        assert_ne!(consumed.commitment(), [0; 32]);
    }

    #[test]
    fn wrong_floor_classifier_or_namespace_substitution_refuses() {
        let snapshot = snapshot();
        let mut wrong = snapshot;
        wrong.safety_floor_semantic_hash = [0xA1; 32];
        let policy = SchedulingPolicyMeaningV1::new(
            [4; 4],
            [5; 4],
            [1; 4],
            [2; 4],
            snapshot.evaluator_revision,
            snapshot.classifier_revision,
        )
        .unwrap();
        assert_eq!(
            GovernanceAttestationV1::derive(planning([5; 4]), wrong)
                .and_then(|attestation| attestation.consume(policy))
                .err(),
            Some(GovernanceAttestationErrorV1::CapabilityMismatch)
        );

        let weakening = GovernanceAttestationV1::derive(planning([3; 4]), snapshot).unwrap();
        assert!(weakening.relation.requires_supplemental_mandate());
    }
}
