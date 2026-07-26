use std::cell::Cell;
use std::marker::PhantomData;
use std::rc::Rc;

use sha2::{Digest, Sha256};
use thiserror::Error;

use super::materialization::{
    AdmittedRepositoryActionBindingV1, AuthorityMaterializationTransactionV1,
    RepositoryActionCommitFactsV1, SchedulingPolicyBindingOwnerV1,
    VerifiedSchedulingPolicyDowngradeMandateUseV1,
};

#[derive(Clone, Copy)]
pub(in crate::domain::vnext) struct PlanningSchedulingPolicyInputV1 {
    pub(super) current_policy: [u64; 4],
    pub(super) candidate_policy: [u64; 4],
    pub(super) safety_floor: [u64; 4],
    pub(super) expected_binding: [u8; 32],
    pub(super) candidate_binding: [u8; 32],
    pub(super) request: [u8; 32],
    pub(super) payload: [u8; 32],
    pub(super) idempotency_key: [u8; 32],
    pub(super) idempotency_meaning: [u8; 32],
}

impl PlanningSchedulingPolicyInputV1 {
    #[expect(
        clippy::too_many_arguments,
        reason = "the Stage 7 Planning input is a closed typed tuple and contains no Authority-owned governance values"
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
        let commitments = [
            expected_binding,
            candidate_binding,
            request,
            payload,
            idempotency_key,
            idempotency_meaning,
        ];
        if commitments.contains(&[0; 32])
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
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum PolicyDiffClassV1 {
    Equivalent,
    Strengthening,
    Weakening,
    Incomparable,
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
    pub(super) governance_floor: [u64; 4],
    pub(super) action_requirement: [u8; 32],
    pub(super) requirement_grammar: [u8; 32],
    pub(super) requirement_evaluator: [u8; 32],
    pub(super) classifier: [u8; 32],
    pub(super) classifier_revision: u64,
    pub(super) authority_witness: [u8; 32],
    pub(super) debit_map: [u8; 32],
    pub(super) root_use_atoms: [u8; 32],
    pub(super) transaction_occurrence: [u8; 32],
}

impl GovernanceOwnerSnapshotV1 {
    fn validate(self) -> Result<(), GovernanceAttestationErrorV1> {
        let commitments = [
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
            self.action_requirement,
            self.requirement_grammar,
            self.requirement_evaluator,
            self.classifier,
            self.authority_witness,
            self.debit_map,
            self.root_use_atoms,
            self.transaction_occurrence,
        ];
        if commitments.contains(&[0; 32])
            || self.generation_ordinal == 0
            || self.head_revision == 0
            || self.epoch == 0
            || self.trusted_time == 0
            || self.revocation_revision == 0
            || self.governance_floor_version == 0
            || self.governance_floor_revision == 0
            || self.classifier_revision == 0
            || self.governance_floor == [0; 4]
        {
            return Err(GovernanceAttestationErrorV1::InvalidAuthorityView);
        }
        Ok(())
    }
}

pub(super) trait GovernanceAttestationOwnerPortV1 {
    fn same_key_committed_result(
        &mut self,
        key: [u8; 32],
        meaning: [u8; 32],
    ) -> Result<Option<[u8; 32]>, GovernanceAttestationErrorV1>;

    fn retained_snapshot(
        &mut self,
    ) -> Result<GovernanceOwnerSnapshotV1, GovernanceAttestationErrorV1>;

    fn consume_joined_publication(
        &mut self,
        request: GovernanceJoinedPublicationV1,
    ) -> Result<[u8; 32], GovernanceAttestationErrorV1>;
}

pub(super) enum GovernanceMaterializationUseV1<'tx> {
    #[cfg_attr(
        test,
        expect(
            dead_code,
            reason = "the production no-Mandate carrier is exercised by the Stage 7 owner integration"
        )
    )]
    NoMandate(Box<GovernanceNoMandateUseV1<'tx>>),
    #[cfg_attr(
        test,
        expect(
            dead_code,
            reason = "the production downgrade carrier is exercised by the Stage 7 owner integration"
        )
    )]
    Downgrade(Box<GovernanceDowngradeUseV1<'tx>>),
    #[cfg(test)]
    Conformance {
        supplemental: bool,
        _lifetime: PhantomData<&'tx mut ()>,
    },
}

pub(super) struct GovernanceNoMandateUseV1<'tx> {
    pub(super) transaction: &'tx AuthorityMaterializationTransactionV1<'tx>,
    pub(super) binding: AdmittedRepositoryActionBindingV1<'tx, SchedulingPolicyBindingOwnerV1>,
    pub(super) commit: RepositoryActionCommitFactsV1,
}

pub(super) struct GovernanceDowngradeUseV1<'tx> {
    pub(super) mandate: VerifiedSchedulingPolicyDowngradeMandateUseV1<'tx>,
    pub(super) binding: AdmittedRepositoryActionBindingV1<'tx, SchedulingPolicyBindingOwnerV1>,
    pub(super) commit: RepositoryActionCommitFactsV1,
}

impl GovernanceMaterializationUseV1<'_> {
    const fn is_supplemental(&self) -> bool {
        match self {
            Self::NoMandate(_) => false,
            Self::Downgrade(_) => true,
            #[cfg(test)]
            Self::Conformance { supplemental, .. } => *supplemental,
        }
    }

    fn consume_existing_materialization(
        self,
    ) -> Result<(), super::materialization::AuthorityMaterializationErrorV1> {
        match self {
            Self::NoMandate(use_token) => {
                let use_token = *use_token;
                use_token
                    .transaction
                    .consume_binding_without_mandate(use_token.binding, use_token.commit)
                    .map(|_| ())
            }
            Self::Downgrade(use_token) => {
                let use_token = *use_token;
                use_token
                    .mandate
                    .consume_with_action_binding(use_token.binding, use_token.commit)
                    .map(|_| ())
            }
            #[cfg(test)]
            Self::Conformance { .. } => Ok(()),
        }
    }
}

pub(super) struct GovernanceJoinedPublicationV1 {
    pub(super) attestation_commitment: [u8; 32],
    pub(super) expected_snapshot: GovernanceOwnerSnapshotV1,
    pub(super) planning: PlanningSchedulingPolicyInputV1,
    pub(super) relation: PolicyDiffClassV1,
}

struct GovernanceAttestationV1<'tx, P: GovernanceAttestationOwnerPortV1> {
    owner: &'tx mut P,
    planning: PlanningSchedulingPolicyInputV1,
    snapshot: GovernanceOwnerSnapshotV1,
    relation: PolicyDiffClassV1,
    commitment: [u8; 32],
    consumed: Cell<bool>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl<'tx, P: GovernanceAttestationOwnerPortV1> GovernanceAttestationV1<'tx, P> {
    fn derive(
        owner: &'tx mut P,
        planning: PlanningSchedulingPolicyInputV1,
    ) -> Result<Self, GovernanceAttestationErrorV1> {
        let snapshot = owner.retained_snapshot()?;
        snapshot.validate()?;
        let relation = classify_policy(planning, snapshot)?;
        let commitment = governance_commitment(planning, snapshot, relation);
        Ok(Self {
            owner,
            planning,
            snapshot,
            relation,
            commitment,
            consumed: Cell::new(false),
            _not_send_or_sync: PhantomData,
        })
    }

    fn consume(
        self,
        materialization: GovernanceMaterializationUseV1<'tx>,
    ) -> Result<[u8; 32], GovernanceAttestationErrorV1> {
        if self.consumed.replace(true)
            || materialization.is_supplemental() != requires_supplemental_mandate(self.relation)
        {
            return Err(GovernanceAttestationErrorV1::CapabilityMismatch);
        }
        materialization
            .consume_existing_materialization()
            .map_err(|_| GovernanceAttestationErrorV1::CapabilityMismatch)?;
        self.owner
            .consume_joined_publication(GovernanceJoinedPublicationV1 {
                attestation_commitment: self.commitment,
                expected_snapshot: self.snapshot,
                planning: self.planning,
                relation: self.relation,
            })
    }
}

pub(super) fn publish_with_governance<'tx, P: GovernanceAttestationOwnerPortV1>(
    owner: &'tx mut P,
    planning: PlanningSchedulingPolicyInputV1,
    materialization: GovernanceMaterializationUseV1<'tx>,
) -> Result<[u8; 32], GovernanceAttestationErrorV1> {
    if let Some(result) =
        owner.same_key_committed_result(planning.idempotency_key, planning.idempotency_meaning)?
    {
        return Ok(result);
    }
    GovernanceAttestationV1::derive(owner, planning)?.consume(materialization)
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

const fn requires_supplemental_mandate(relation: PolicyDiffClassV1) -> bool {
    matches!(
        relation,
        PolicyDiffClassV1::Weakening | PolicyDiffClassV1::Incomparable
    )
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
        snapshot.action_requirement,
        snapshot.requirement_grammar,
        snapshot.requirement_evaluator,
        snapshot.classifier,
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
    #[error("governance currentness changed")]
    Changed,
    #[error("idempotency meaning conflicts")]
    IdempotencyConflict,
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestOwnerV1 {
        initial: GovernanceOwnerSnapshotV1,
        final_snapshot: GovernanceOwnerSnapshotV1,
        writes: u64,
        spends: u64,
        prior: Option<([u8; 32], [u8; 32], [u8; 32])>,
    }

    impl GovernanceAttestationOwnerPortV1 for TestOwnerV1 {
        fn same_key_committed_result(
            &mut self,
            key: [u8; 32],
            meaning: [u8; 32],
        ) -> Result<Option<[u8; 32]>, GovernanceAttestationErrorV1> {
            match self.prior {
                Some((stored_key, stored_meaning, result))
                    if stored_key == key && stored_meaning == meaning =>
                {
                    Ok(Some(result))
                }
                Some((stored_key, _, _)) if stored_key == key => {
                    Err(GovernanceAttestationErrorV1::IdempotencyConflict)
                }
                _ => Ok(None),
            }
        }

        fn retained_snapshot(
            &mut self,
        ) -> Result<GovernanceOwnerSnapshotV1, GovernanceAttestationErrorV1> {
            Ok(self.initial)
        }

        fn consume_joined_publication(
            &mut self,
            request: GovernanceJoinedPublicationV1,
        ) -> Result<[u8; 32], GovernanceAttestationErrorV1> {
            if self.final_snapshot != request.expected_snapshot
                || request.attestation_commitment
                    != governance_commitment(
                        request.planning,
                        request.expected_snapshot,
                        request.relation,
                    )
            {
                return Err(GovernanceAttestationErrorV1::Changed);
            }
            self.writes += 1;
            self.spends += u64::from(requires_supplemental_mandate(request.relation));
            Ok([0xE1; 32])
        }
    }

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
            governance_floor: [2, 2, 2, 2],
            action_requirement: [19; 32],
            requirement_grammar: [20; 32],
            requirement_evaluator: [21; 32],
            classifier: [22; 32],
            classifier_revision: 23,
            authority_witness: [24; 32],
            debit_map: [25; 32],
            root_use_atoms: [26; 32],
            transaction_occurrence: [27; 32],
        }
    }

    fn planning(candidate_policy: [u64; 4]) -> PlanningSchedulingPolicyInputV1 {
        PlanningSchedulingPolicyInputV1::from_stage7_planning(
            [4, 4, 4, 4],
            candidate_policy,
            [1, 1, 1, 1],
            [31; 32],
            [32; 32],
            [33; 32],
            [34; 32],
            [35; 32],
            [36; 32],
        )
        .unwrap()
    }

    #[test]
    fn authority_derives_governance_and_classifier_and_consumes_with_publication() {
        let current = snapshot();
        let mut owner = TestOwnerV1 {
            initial: current,
            final_snapshot: current,
            writes: 0,
            spends: 0,
            prior: None,
        };

        let result = publish_with_governance(
            &mut owner,
            planning([3, 3, 3, 3]),
            GovernanceMaterializationUseV1::Conformance {
                supplemental: true,
                _lifetime: PhantomData,
            },
        )
        .unwrap();

        assert_eq!(result, [0xE1; 32]);
        assert_eq!(owner.writes, 1);
        assert_eq!(owner.spends, 1);
    }

    #[test]
    fn raw_floor_stale_view_and_wrong_mandate_route_refuse_zero_write_zero_spend() {
        let current = snapshot();
        let mut changed = current;
        changed.governance_floor_revision += 1;
        let mut owner = TestOwnerV1 {
            initial: current,
            final_snapshot: changed,
            writes: 0,
            spends: 0,
            prior: None,
        };
        assert_eq!(
            publish_with_governance(
                &mut owner,
                planning([3, 3, 3, 3]),
                GovernanceMaterializationUseV1::Conformance {
                    supplemental: true,
                    _lifetime: PhantomData,
                },
            ),
            Err(GovernanceAttestationErrorV1::Changed)
        );
        assert_eq!((owner.writes, owner.spends), (0, 0));

        owner.final_snapshot = owner.initial;
        assert_eq!(
            publish_with_governance(
                &mut owner,
                planning([3, 3, 3, 3]),
                GovernanceMaterializationUseV1::Conformance {
                    supplemental: false,
                    _lifetime: PhantomData,
                },
            ),
            Err(GovernanceAttestationErrorV1::CapabilityMismatch)
        );
        assert_eq!((owner.writes, owner.spends), (0, 0));
    }

    #[test]
    fn exact_committed_result_replays_without_capability_or_spend() {
        let current = snapshot();
        let planning = planning([3, 3, 3, 3]);
        let mut owner = TestOwnerV1 {
            initial: current,
            final_snapshot: current,
            writes: 0,
            spends: 0,
            prior: Some((
                planning.idempotency_key,
                planning.idempotency_meaning,
                [0xA1; 32],
            )),
        };
        assert_eq!(
            publish_with_governance(
                &mut owner,
                planning,
                GovernanceMaterializationUseV1::Conformance {
                    supplemental: true,
                    _lifetime: PhantomData,
                },
            ),
            Ok([0xA1; 32])
        );
        assert_eq!((owner.writes, owner.spends), (0, 0));
    }
}
