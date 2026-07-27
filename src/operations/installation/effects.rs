use crate::domain::distribution::CommitmentV1;
use crate::domain::distribution::runtime::{
    CapturedTargetPreimageV1, DistributionPlanTargetV1, DistributionPlanV1,
    DistributionTransactionV1, EffectCrossingObservationV1, VerificationDispositionV1,
};
use crate::domain::execution::EffectIntentIdV1;

use super::InstallationOperationErrorV1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Stage4EffectReservationBatchV1 {
    reservations: Vec<(u64, EffectIntentIdV1)>,
}

impl Stage4EffectReservationBatchV1 {
    pub fn new(
        plan: &DistributionPlanV1,
        captures: &[CapturedTargetPreimageV1],
        reservations: Vec<(u64, EffectIntentIdV1)>,
    ) -> Result<Self, InstallationOperationErrorV1> {
        if captures.len() != plan.targets().len()
            || reservations.len() != plan.targets().len()
            || plan.targets().iter().zip(captures).zip(&reservations).any(
                |((target, capture), (target_tag, _))| {
                    target.target_tag != capture.target_tag || target.target_tag != *target_tag
                },
            )
        {
            return Err(InstallationOperationErrorV1::IncompleteEffectReservationBatch);
        }
        let unique_intents = reservations
            .iter()
            .map(|(_, intent)| *intent)
            .collect::<std::collections::BTreeSet<_>>();
        if unique_intents.len() != reservations.len() {
            return Err(InstallationOperationErrorV1::IncompleteEffectReservationBatch);
        }
        Ok(Self { reservations })
    }

    pub(crate) fn into_reservations(self) -> Vec<(u64, EffectIntentIdV1)> {
        self.reservations
    }
}

/// The single Stage-9 boundary allowed to cross from durable intent into
/// target effects. Implementations must use the Stage-4 Execution carriers;
/// returning an `EffectIntentIdV1` is evidence of reservation, not permission
/// to perform an unrecorded effect.
pub trait DistributionEffectPortV1 {
    /// Compares the live target with the plan fence and captures the exact
    /// target-owned preimage without reserving or crossing an Effect.
    fn compare_and_capture(
        &mut self,
        target: &DistributionPlanTargetV1,
    ) -> Result<CapturedTargetPreimageV1, InstallationOperationErrorV1>;

    fn stage_candidate(
        &mut self,
        plan: &DistributionPlanV1,
    ) -> Result<CommitmentV1, InstallationOperationErrorV1>;

    /// Publishes every target reservation through the Stage-4 effect seam as
    /// one all-or-none batch. An error must mean that no reservation in the
    /// requested batch became durable; partial durable success is forbidden.
    /// Every Intent must bind its capture's effect fence, and replaying the
    /// exact plan and captures must return the same content-addressed IDs.
    fn reserve_all_effects_atomically(
        &mut self,
        plan: &DistributionPlanV1,
        captures: &[CapturedTargetPreimageV1],
    ) -> Result<Stage4EffectReservationBatchV1, InstallationOperationErrorV1>;

    fn persist_checkpoint(
        &mut self,
        transaction: &DistributionTransactionV1,
    ) -> Result<(), InstallationOperationErrorV1>;

    fn reconcile_and_apply(
        &mut self,
        target: &DistributionPlanTargetV1,
        effect_intent_id: EffectIntentIdV1,
    ) -> Result<EffectCrossingObservationV1, InstallationOperationErrorV1>;

    fn verify_target(
        &mut self,
        target: &DistributionPlanTargetV1,
    ) -> Result<VerificationDispositionV1, InstallationOperationErrorV1>;

    /// Restores only the target-owned preimage. For a managed-block target,
    /// the implementation must retain the currently observed outside bytes
    /// and refuse rather than replace the whole containing file.
    fn restore_exact_preimage(
        &mut self,
        target: &DistributionPlanTargetV1,
        capture: &CapturedTargetPreimageV1,
    ) -> Result<(), InstallationOperationErrorV1>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::authority::ActionRequestIdV1;
    use crate::domain::distribution::runtime::{
        CustodyAssessmentV1, CustodyBasisV1, DistributionDomainKindV1, DistributionDomainRefV1,
        DistributionMutationKindV1, DistributionRuntimeObjectKindV1, DistributionScopedObjectRefV1,
        DistributionSnapshotTargetV1, TargetEffectKindV1,
    };
    use crate::domain::identity::StoreObjectIdV1;

    fn commitment(byte: u8) -> CommitmentV1 {
        CommitmentV1::from_bytes([byte; 32])
    }

    fn object_id(byte: u8) -> StoreObjectIdV1 {
        StoreObjectIdV1::parse(&format!("sha256:{}", format!("{byte:02x}").repeat(32))).unwrap()
    }

    fn domain() -> DistributionDomainRefV1 {
        DistributionDomainRefV1::new(
            DistributionDomainKindV1::InstallationDomain,
            commitment(1),
            commitment(2),
            commitment(3),
        )
        .unwrap()
    }

    fn scoped(
        domain: &DistributionDomainRefV1,
        kind: DistributionRuntimeObjectKindV1,
        byte: u8,
    ) -> DistributionScopedObjectRefV1 {
        DistributionScopedObjectRefV1::new(domain.clone(), kind, object_id(byte)).unwrap()
    }

    fn plan_and_capture() -> (DistributionPlanV1, CapturedTargetPreimageV1) {
        let domain = domain();
        let custody = CustodyAssessmentV1::assess(&CustodyBasisV1 {
            domain: domain.clone(),
            target_identity: commitment(10),
            alias_closure_id: commitment(11),
            receipt_ref: Some(scoped(
                &domain,
                DistributionRuntimeObjectKindV1::DistributionReceipt,
                12,
            )),
            claim_ref: Some(scoped(
                &domain,
                DistributionRuntimeObjectKindV1::InstalledResourceClaim,
                13,
            )),
            claimed_target_identity: Some(commitment(10)),
            resource_id: Some(commitment(14)),
            bundle_id: Some(commitment(15)),
            release_id: Some(commitment(16)),
            claimed_content_sha256: Some(commitment(17)),
            observed_content_sha256: Some(commitment(17)),
            managed_block: None,
            foreign_owner_observed: false,
            external_manager_observed: false,
            alias_ambiguous: false,
            unsafe_path_state: false,
        })
        .unwrap();
        let target_ref = scoped(
            &domain,
            DistributionRuntimeObjectKindV1::CanonicalTargetIdentity,
            20,
        );
        let plan = DistributionPlanV1::new(
            domain.clone(),
            DistributionMutationKindV1::Update,
            ActionRequestIdV1::derive("stage9-effect-batch").unwrap(),
            scoped(
                &domain,
                DistributionRuntimeObjectKindV1::ActionRequestOrCeremony,
                21,
            ),
            scoped(
                &domain,
                DistributionRuntimeObjectKindV1::DistributionPlan,
                22,
            ),
            scoped(&domain, DistributionRuntimeObjectKindV1::IdempotencyKey, 23),
            Some(commitment(16)),
            None,
            None,
            None,
            vec![DistributionPlanTargetV1 {
                target_tag: 1,
                target_identity_ref: target_ref.clone(),
                target_identity: commitment(10),
                custody,
                expected_preimage_commitment: commitment(24),
                candidate_commitment: Some(commitment(25)),
                effect_kind: TargetEffectKindV1::RewriteOwnedTarget,
                outside_prefix_commitment: None,
                outside_suffix_commitment: None,
            }],
        )
        .unwrap();
        let capture = CapturedTargetPreimageV1 {
            target_tag: 1,
            compared_preimage_commitment: commitment(24),
            snapshot_target: DistributionSnapshotTargetV1 {
                target_tag: 1,
                domain,
                canonical_target_identity_ref: target_ref,
                prior_claim_ref: None,
                content_object_ref: Some(scoped(
                    plan.domain(),
                    DistributionRuntimeObjectKindV1::ContentObject,
                    26,
                )),
                content_sha256: Some(commitment(17)),
                prior_absence: false,
                permissions_commitment_id: commitment(27),
                owner_metadata_commitment_id: commitment(28),
                managed_block_ref: None,
                restore_profile_id: commitment(29),
            },
            effect_fence_commitment: commitment(30),
        };
        (plan, capture)
    }

    #[test]
    fn reservation_batch_covers_every_target_exactly_once() {
        let (plan, capture) = plan_and_capture();
        let effect = EffectIntentIdV1::derive("stage9-batched-effect").unwrap();
        assert!(
            Stage4EffectReservationBatchV1::new(
                &plan,
                std::slice::from_ref(&capture),
                vec![(1, effect)],
            )
            .is_ok()
        );
        assert!(matches!(
            Stage4EffectReservationBatchV1::new(&plan, &[], vec![(1, effect)]),
            Err(InstallationOperationErrorV1::IncompleteEffectReservationBatch)
        ));
        assert!(matches!(
            Stage4EffectReservationBatchV1::new(
                &plan,
                std::slice::from_ref(&capture),
                vec![(2, effect)],
            ),
            Err(InstallationOperationErrorV1::IncompleteEffectReservationBatch)
        ));
    }
}
