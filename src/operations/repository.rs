use std::fmt;

use thiserror::Error;

use crate::domain::distribution::CommitmentV1;
use crate::domain::distribution::runtime::{
    CapturedTargetPreimageV1, DistributionPhaseAuthorizationV1, DistributionPlanTargetV1,
    DistributionPlanV1, DistributionTransactionV1, EffectCrossingObservationV1,
    TargetPlanObservationV1, VerificationDispositionV1,
};
use crate::domain::execution::EffectIntentIdV1;
use crate::domain::installation::RepositoryInstallationClosureV1;
use crate::domain::persistence::{StorePublicationOutcomeV1, StoreRoleV1, StoreV1};
use crate::domain::repository::{
    CommittedRepositoryBootstrapV1, RepositoryBootstrapAdmissionV1,
    RepositoryBootstrapAuthorizationV1, RepositoryBootstrapEffectObservationV1,
    RepositoryBootstrapEffectPermitV1, RepositoryBootstrapErrorV1, RepositoryBootstrapOwnerFactsV1,
    RepositoryBootstrapReadbackV1, RepositoryBootstrapTargetFactsV1,
};

use super::installation::{
    ActiveDistributionTransactionV1, ActiveInstallationFacadeV1, ActivePublicationObjectsV1,
    DistributionEffectPortV1, InstallationOperationErrorV1, Stage4EffectReservationBatchV1,
};

#[derive(Debug)]
pub(crate) struct ActiveRepositoryBootstrapV1 {
    admission: RepositoryBootstrapAdmissionV1,
    transaction: ActiveDistributionTransactionV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RepositoryBootstrapBackupReceiptV1 {
    pub target_tag: u64,
    pub effect_fence_commitment: CommitmentV1,
    pub backup_object_commitment: CommitmentV1,
}

pub(crate) trait RepositoryBootstrapEffectPortV1: DistributionEffectPortV1 {
    fn persist_exact_backups(
        &mut self,
        plan: &DistributionPlanV1,
        captures: &[CapturedTargetPreimageV1],
    ) -> Result<Vec<RepositoryBootstrapBackupReceiptV1>, InstallationOperationErrorV1>;

    fn readback_exact_targets(
        &mut self,
        plan: &DistributionPlanV1,
    ) -> Result<RepositoryBootstrapReadbackV1, InstallationOperationErrorV1>;
}

pub(crate) trait RepositoryBootstrapBackupPortV1 {
    fn persist_exact_backups(
        &mut self,
        plan: &DistributionPlanV1,
        captures: &[CapturedTargetPreimageV1],
    ) -> Result<Vec<RepositoryBootstrapBackupReceiptV1>, InstallationOperationErrorV1>;
}

pub(crate) trait RepositoryBootstrapReadbackPortV1 {
    fn readback_exact_targets(
        &mut self,
        plan: &DistributionPlanV1,
    ) -> Result<RepositoryBootstrapReadbackV1, InstallationOperationErrorV1>;
}

pub(crate) struct RepositoryBootstrapEffectAdapterV1<'ports> {
    distribution: &'ports mut dyn DistributionEffectPortV1,
    backups: &'ports mut dyn RepositoryBootstrapBackupPortV1,
    readback: &'ports mut dyn RepositoryBootstrapReadbackPortV1,
}

impl fmt::Debug for RepositoryBootstrapEffectAdapterV1<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RepositoryBootstrapEffectAdapterV1")
            .finish_non_exhaustive()
    }
}

impl<'ports> RepositoryBootstrapEffectAdapterV1<'ports> {
    pub(crate) fn new(
        distribution: &'ports mut dyn DistributionEffectPortV1,
        backups: &'ports mut dyn RepositoryBootstrapBackupPortV1,
        readback: &'ports mut dyn RepositoryBootstrapReadbackPortV1,
    ) -> Self {
        Self {
            distribution,
            backups,
            readback,
        }
    }
}

impl DistributionEffectPortV1 for RepositoryBootstrapEffectAdapterV1<'_> {
    fn compare_and_capture(
        &mut self,
        target: &DistributionPlanTargetV1,
    ) -> Result<CapturedTargetPreimageV1, InstallationOperationErrorV1> {
        self.distribution.compare_and_capture(target)
    }

    fn stage_candidate(
        &mut self,
        plan: &DistributionPlanV1,
    ) -> Result<CommitmentV1, InstallationOperationErrorV1> {
        self.distribution.stage_candidate(plan)
    }

    fn reserve_all_effects_atomically(
        &mut self,
        plan: &DistributionPlanV1,
        captures: &[CapturedTargetPreimageV1],
    ) -> Result<Stage4EffectReservationBatchV1, InstallationOperationErrorV1> {
        self.distribution
            .reserve_all_effects_atomically(plan, captures)
    }

    fn persist_checkpoint(
        &mut self,
        transaction: &DistributionTransactionV1,
    ) -> Result<(), InstallationOperationErrorV1> {
        self.distribution.persist_checkpoint(transaction)
    }

    fn reconcile_and_apply(
        &mut self,
        target: &DistributionPlanTargetV1,
        effect_intent_id: EffectIntentIdV1,
    ) -> Result<EffectCrossingObservationV1, InstallationOperationErrorV1> {
        self.distribution
            .reconcile_and_apply(target, effect_intent_id)
    }

    fn verify_target(
        &mut self,
        target: &DistributionPlanTargetV1,
    ) -> Result<VerificationDispositionV1, InstallationOperationErrorV1> {
        self.distribution.verify_target(target)
    }

    fn restore_exact_preimage(
        &mut self,
        target: &DistributionPlanTargetV1,
        capture: &CapturedTargetPreimageV1,
    ) -> Result<(), InstallationOperationErrorV1> {
        self.distribution.restore_exact_preimage(target, capture)
    }
}

impl RepositoryBootstrapEffectPortV1 for RepositoryBootstrapEffectAdapterV1<'_> {
    fn persist_exact_backups(
        &mut self,
        plan: &DistributionPlanV1,
        captures: &[CapturedTargetPreimageV1],
    ) -> Result<Vec<RepositoryBootstrapBackupReceiptV1>, InstallationOperationErrorV1> {
        self.backups.persist_exact_backups(plan, captures)
    }

    fn readback_exact_targets(
        &mut self,
        plan: &DistributionPlanV1,
    ) -> Result<RepositoryBootstrapReadbackV1, InstallationOperationErrorV1> {
        self.readback.readback_exact_targets(plan)
    }
}

pub(crate) struct ActiveRepositoryFacadeV1<'store> {
    distribution: ActiveInstallationFacadeV1<'store>,
}

impl fmt::Debug for ActiveRepositoryFacadeV1<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ActiveRepositoryFacadeV1")
            .finish_non_exhaustive()
    }
}

impl<'store> ActiveRepositoryFacadeV1<'store> {
    pub(crate) fn new(
        store: &'store mut StoreV1,
    ) -> Result<Self, RepositoryBootstrapOperationErrorV1> {
        if store.role() != StoreRoleV1::Repository {
            return Err(RepositoryBootstrapOperationErrorV1::WrongOwnerDomain);
        }
        Ok(Self {
            distribution: ActiveInstallationFacadeV1::new(store),
        })
    }

    pub(crate) fn begin_bootstrap(
        &mut self,
        admission: RepositoryBootstrapAdmissionV1,
        phase_authorizations: Vec<DistributionPhaseAuthorizationV1>,
        observations: Vec<TargetPlanObservationV1>,
    ) -> Result<ActiveRepositoryBootstrapV1, RepositoryBootstrapOperationErrorV1> {
        let plan = admission.plan().clone();
        let transaction = self
            .distribution
            .begin(plan, phase_authorizations, observations)?;
        Ok(ActiveRepositoryBootstrapV1 {
            admission,
            transaction,
        })
    }

    pub(crate) fn authorize_bootstrap(
        &self,
        active: &ActiveRepositoryBootstrapV1,
        observations: [RepositoryBootstrapEffectObservationV1; 2],
    ) -> Result<RepositoryBootstrapEffectPermitV1, RepositoryBootstrapOperationErrorV1> {
        Ok(active.admission.authorize_effects(observations)?)
    }

    pub(crate) fn drive_bootstrap(
        &mut self,
        active: &mut ActiveRepositoryBootstrapV1,
        permit: RepositoryBootstrapEffectPermitV1,
        effects: &mut dyn RepositoryBootstrapEffectPortV1,
    ) -> Result<(), RepositoryBootstrapOperationErrorV1> {
        active.admission.validate_effect_permit(&permit)?;
        self.distribution
            .capture_preimages(&mut active.transaction, effects)?;
        let captures = active.transaction.transaction().captures();
        let receipts = effects.persist_exact_backups(active.admission.plan(), captures)?;
        validate_backup_receipts(captures, &receipts)?;
        self.distribution
            .drive_captured_to_verification(&mut active.transaction, effects)?;
        Ok(())
    }

    pub(crate) fn restore_bootstrap(
        &mut self,
        active: &mut ActiveRepositoryBootstrapV1,
        effects: &mut dyn RepositoryBootstrapEffectPortV1,
    ) -> Result<(), RepositoryBootstrapOperationErrorV1> {
        self.distribution
            .restore_from_captures(&mut active.transaction, effects)?;
        Ok(())
    }

    pub(crate) fn publish_bootstrap(
        &mut self,
        active: &mut ActiveRepositoryBootstrapV1,
        publication_objects: ActivePublicationObjectsV1,
    ) -> Result<StorePublicationOutcomeV1, RepositoryBootstrapOperationErrorV1> {
        Ok(self
            .distribution
            .publish(&mut active.transaction, publication_objects)?)
    }

    pub(crate) fn confirm_currentness(
        &self,
        active: ActiveRepositoryBootstrapV1,
        closure: &RepositoryInstallationClosureV1,
        effects: &mut dyn RepositoryBootstrapEffectPortV1,
    ) -> Result<CommittedRepositoryBootstrapV1, RepositoryBootstrapOperationErrorV1> {
        if !self
            .distribution
            .coherent_repository_closure_is_current(closure)?
        {
            return Err(RepositoryBootstrapOperationErrorV1::RepositoryNotCurrent);
        }
        let readback = effects.readback_exact_targets(active.admission.plan())?;
        Ok(CommittedRepositoryBootstrapV1::confirm(
            active.admission,
            active.transaction.transaction(),
            closure,
            &readback,
        )?)
    }
}

fn validate_backup_receipts(
    captures: &[CapturedTargetPreimageV1],
    receipts: &[RepositoryBootstrapBackupReceiptV1],
) -> Result<(), RepositoryBootstrapOperationErrorV1> {
    if captures.len() != receipts.len()
        || captures.iter().zip(receipts).any(|(capture, receipt)| {
            receipt.target_tag != capture.target_tag
                || receipt.effect_fence_commitment != capture.effect_fence_commitment
                || receipt.backup_object_commitment.as_bytes() == &[0; 32]
        })
    {
        return Err(RepositoryBootstrapOperationErrorV1::InvalidBackupReceipts);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RepositoryBootstrapOwnerSurfaceV1 {
    pub operation_name: &'static str,
}

pub(crate) fn repository_bootstrap_owner_surface() -> RepositoryBootstrapOwnerSurfaceV1 {
    let _ = ActiveRepositoryFacadeV1::new;
    let _ = ActiveRepositoryFacadeV1::begin_bootstrap;
    let _ = ActiveRepositoryFacadeV1::authorize_bootstrap;
    let _ = ActiveRepositoryFacadeV1::drive_bootstrap;
    let _ = ActiveRepositoryFacadeV1::restore_bootstrap;
    let _ = ActiveRepositoryFacadeV1::publish_bootstrap;
    let _ = ActiveRepositoryFacadeV1::confirm_currentness;
    let _ = RepositoryBootstrapAdmissionV1::after_agent_resource_release;
    let _ = RepositoryBootstrapAdmissionV1::installation_release_id;
    let _ = RepositoryBootstrapAdmissionV1::bootstrap_closure;
    let _ = RepositoryBootstrapOwnerFactsV1::new;
    let _ = RepositoryBootstrapTargetFactsV1::new;
    let _ = RepositoryBootstrapReadbackV1::new;
    let _ = CommittedRepositoryBootstrapV1::installation_release_id;
    let _ = CommittedRepositoryBootstrapV1::installation_result_closure;
    let _ = CommittedRepositoryBootstrapV1::repository_result_closure;
    let _ = RepositoryBootstrapEffectAdapterV1::new;
    let _ = RepositoryBootstrapAuthorizationV1::Apply;
    let _ = RepositoryBootstrapAuthorizationV1::Force;
    RepositoryBootstrapOwnerSurfaceV1 {
        operation_name: "RepositoryV1Cutover",
    }
}

#[derive(Debug, Error)]
pub(crate) enum RepositoryBootstrapOperationErrorV1 {
    #[error(transparent)]
    Bootstrap(#[from] RepositoryBootstrapErrorV1),
    #[error(transparent)]
    Distribution(#[from] InstallationOperationErrorV1),
    #[error("the Repository bootstrap operation requires the RepositoryDomain owner")]
    WrongOwnerDomain,
    #[error("Repository bootstrap backup receipts do not match exact captured preimages")]
    InvalidBackupReceipts,
    #[error("the Repository closure is absent from the coherent active Store snapshot")]
    RepositoryNotCurrent,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::distribution::runtime::{
        DistributionDomainKindV1, DistributionDomainRefV1, DistributionRuntimeObjectKindV1,
        DistributionScopedObjectRefV1, DistributionSnapshotTargetV1,
    };
    use crate::domain::identity::StoreObjectIdV1;

    fn commitment(byte: u8) -> CommitmentV1 {
        CommitmentV1::from_bytes([byte; 32])
    }

    fn object(byte: u8) -> StoreObjectIdV1 {
        StoreObjectIdV1::parse(&format!("sha256:{}", format!("{byte:02x}").repeat(32))).unwrap()
    }

    fn capture() -> CapturedTargetPreimageV1 {
        let domain = DistributionDomainRefV1::new(
            DistributionDomainKindV1::RepositoryDomain,
            commitment(1),
            commitment(2),
            commitment(3),
        )
        .unwrap();
        let target_ref = DistributionScopedObjectRefV1::new(
            domain.clone(),
            DistributionRuntimeObjectKindV1::CanonicalTargetIdentity,
            object(4),
        )
        .unwrap();
        CapturedTargetPreimageV1 {
            target_tag: 1,
            compared_preimage_commitment: commitment(5),
            snapshot_target: DistributionSnapshotTargetV1 {
                target_tag: 1,
                domain,
                canonical_target_identity_ref: target_ref,
                prior_claim_ref: None,
                content_object_ref: None,
                content_sha256: Some(commitment(5)),
                prior_absence: false,
                permissions_commitment_id: commitment(6),
                owner_metadata_commitment_id: commitment(7),
                managed_block_ref: None,
                restore_profile_id: commitment(8),
            },
            effect_fence_commitment: commitment(9),
        }
    }

    #[test]
    fn backup_receipts_must_bind_exact_captured_effect_fence_before_crossing() {
        let capture = capture();
        let exact = RepositoryBootstrapBackupReceiptV1 {
            target_tag: capture.target_tag,
            effect_fence_commitment: capture.effect_fence_commitment,
            backup_object_commitment: commitment(10),
        };
        assert!(validate_backup_receipts(std::slice::from_ref(&capture), &[exact]).is_ok());

        let wrong_fence = RepositoryBootstrapBackupReceiptV1 {
            target_tag: capture.target_tag,
            effect_fence_commitment: commitment(11),
            backup_object_commitment: commitment(10),
        };
        assert!(matches!(
            validate_backup_receipts(std::slice::from_ref(&capture), &[wrong_fence]),
            Err(RepositoryBootstrapOperationErrorV1::InvalidBackupReceipts)
        ));

        let missing_backup = RepositoryBootstrapBackupReceiptV1 {
            target_tag: capture.target_tag,
            effect_fence_commitment: capture.effect_fence_commitment,
            backup_object_commitment: CommitmentV1::from_bytes([0; 32]),
        };
        assert!(matches!(
            validate_backup_receipts(&[capture], &[missing_backup]),
            Err(RepositoryBootstrapOperationErrorV1::InvalidBackupReceipts)
        ));
    }

    #[test]
    fn repository_operation_orders_backup_rollback_publication_and_currentness_gates() {
        let source = include_str!("repository.rs");
        let capture = source.find(".capture_preimages").unwrap();
        let backup = source.find("effects.persist_exact_backups").unwrap();
        let validate = source.find("validate_backup_receipts(captures").unwrap();
        let cross = source.find(".drive_captured_to_verification").unwrap();
        assert!(capture < backup && backup < validate && validate < cross);

        let restore = source.find(".restore_from_captures").unwrap();
        let publish = source.find(".publish(&mut active.transaction").unwrap();
        assert!(restore < publish);

        let coherent = source
            .find(".coherent_repository_closure_is_current")
            .unwrap();
        let readback = source
            .find("effects.readback_exact_targets(active.admission.plan())")
            .unwrap();
        let confirm = source
            .find("CommittedRepositoryBootstrapV1::confirm")
            .unwrap();
        assert!(coherent < readback && readback < confirm);
    }
}
