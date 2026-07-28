use crate::domain::distribution::runtime::{
    CapturedTargetPreimageV1, DistributionPhaseAuthorizationV1, DistributionPlanTargetV1,
    DistributionPlanV1, DistributionTransactionV1, EffectCrossingObservationV1,
    TargetPlanObservationV1, VerificationDispositionV1,
};
use crate::domain::distribution::{CommitmentV1, ReleaseIdV1};
use crate::domain::execution::EffectIntentIdV1;
use crate::domain::installation::{
    AgentResourceCutoverErrorV1, AgentResourceReleaseAdmissionV1,
    AgentResourceReleaseConsumerSealV1, AgentResourceReleaseOwnerFactsV1,
    CommittedAgentResourceReleaseV1, InstallationLegacyDeletionPlanV3,
    ObservedInstallationClosureV1, Stage12RollbackRehearsalV3, UserAgentInstallationClosureV1,
};
use crate::domain::migration::runtime::{LegacyQuarantineEpochV4, LegacyRollbackAssessmentV4};
use crate::domain::persistence::StorePublicationOutcomeV1;
use crate::foundation::core::legacy_quarantine::FoundationLegacyQuarantineClosureV2;

use super::{
    ActiveDistributionTransactionV1, ActiveInstallationFacadeV1, ActivePublicationObjectsV1,
    DistributionEffectPortV1, InstallationOperationErrorV1,
};

#[derive(Debug)]
pub(crate) struct ActiveAgentResourceReleaseV1 {
    admission: AgentResourceReleaseAdmissionV1,
    transaction: ActiveDistributionTransactionV1,
}

#[derive(Debug)]
pub(crate) struct AgentResourceReleaseCeremonyV1 {
    release_id: ReleaseIdV1,
    consumer_seal: AgentResourceReleaseConsumerSealV1,
    owner_facts: AgentResourceReleaseOwnerFactsV1,
    phase_authorizations: Vec<DistributionPhaseAuthorizationV1>,
    plan_observations: Vec<TargetPlanObservationV1>,
    publication_objects: ActivePublicationObjectsV1,
    closure: UserAgentInstallationClosureV1,
    observed: ObservedInstallationClosureV1,
}

impl AgentResourceReleaseCeremonyV1 {
    #[expect(
        clippy::too_many_arguments,
        reason = "the frozen cutover ceremony binds admission, publication, and reconnect facts"
    )]
    #[expect(
        dead_code,
        reason = "the admitted ceremony provider constructs the exact release input"
    )]
    pub(crate) fn new(
        release_id: ReleaseIdV1,
        consumer_seal: AgentResourceReleaseConsumerSealV1,
        owner_facts: AgentResourceReleaseOwnerFactsV1,
        phase_authorizations: Vec<DistributionPhaseAuthorizationV1>,
        plan_observations: Vec<TargetPlanObservationV1>,
        publication_objects: ActivePublicationObjectsV1,
        closure: UserAgentInstallationClosureV1,
        observed: ObservedInstallationClosureV1,
    ) -> Self {
        Self {
            release_id,
            consumer_seal,
            owner_facts,
            phase_authorizations,
            plan_observations,
            publication_objects,
            closure,
            observed,
        }
    }
}

pub(crate) struct AgentResourceReleaseEffectAdapterV1<'effects> {
    distribution: &'effects mut dyn DistributionEffectPortV1,
}

impl std::fmt::Debug for AgentResourceReleaseEffectAdapterV1<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AgentResourceReleaseEffectAdapterV1")
            .finish_non_exhaustive()
    }
}

impl<'effects> AgentResourceReleaseEffectAdapterV1<'effects> {
    pub(crate) fn new(distribution: &'effects mut dyn DistributionEffectPortV1) -> Self {
        Self { distribution }
    }
}

impl DistributionEffectPortV1 for AgentResourceReleaseEffectAdapterV1<'_> {
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
    ) -> Result<super::Stage4EffectReservationBatchV1, InstallationOperationErrorV1> {
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

impl ActiveInstallationFacadeV1<'_> {
    pub(crate) fn begin_agent_resource_release(
        &mut self,
        admission: AgentResourceReleaseAdmissionV1,
        phase_authorizations: Vec<DistributionPhaseAuthorizationV1>,
        observations: Vec<TargetPlanObservationV1>,
    ) -> Result<ActiveAgentResourceReleaseV1, AgentResourceReleaseOperationErrorV1> {
        let plan = admission.plan().clone();
        let transaction = self.begin(plan, phase_authorizations, observations)?;
        Ok(ActiveAgentResourceReleaseV1 {
            admission,
            transaction,
        })
    }

    pub(crate) fn execute_agent_resource_release(
        &mut self,
        ceremony: AgentResourceReleaseCeremonyV1,
        effects: &mut AgentResourceReleaseEffectAdapterV1<'_>,
    ) -> Result<CommittedAgentResourceReleaseV1, AgentResourceReleaseOperationErrorV1> {
        let admission = AgentResourceReleaseAdmissionV1::new(
            ceremony.release_id,
            ceremony.consumer_seal,
            ceremony.owner_facts,
        )?;
        let mut active = self.begin_agent_resource_release(
            admission,
            ceremony.phase_authorizations,
            ceremony.plan_observations,
        )?;
        self.drive_agent_resource_release(&mut active, effects)?;
        self.publish_agent_resource_release(&mut active, ceremony.publication_objects)?;
        self.confirm_agent_resource_reconnect(active, &ceremony.closure, &ceremony.observed)
    }

    pub(crate) fn drive_agent_resource_release(
        &mut self,
        active: &mut ActiveAgentResourceReleaseV1,
        effects: &mut impl DistributionEffectPortV1,
    ) -> Result<(), AgentResourceReleaseOperationErrorV1> {
        self.drive_to_verification(&mut active.transaction, effects)?;
        Ok(())
    }

    #[expect(
        dead_code,
        reason = "the frozen recovery entrypoint remains available to a recovery-mode owner"
    )]
    pub(crate) fn restore_agent_resource_release(
        &mut self,
        active: &mut ActiveAgentResourceReleaseV1,
        effects: &mut impl DistributionEffectPortV1,
    ) -> Result<(), AgentResourceReleaseOperationErrorV1> {
        self.restore_from_captures(&mut active.transaction, effects)?;
        Ok(())
    }

    pub(crate) fn rehearse_stage12_agent_resource_rollback(
        &mut self,
        active: &mut ActiveAgentResourceReleaseV1,
        effects: &mut impl DistributionEffectPortV1,
        epoch: &LegacyQuarantineEpochV4,
        foundation: &FoundationLegacyQuarantineClosureV2,
        rollback_assessment: &LegacyRollbackAssessmentV4,
    ) -> Result<
        (Stage12RollbackRehearsalV3, InstallationLegacyDeletionPlanV3),
        AgentResourceReleaseOperationErrorV1,
    > {
        self.restore_from_captures(&mut active.transaction, effects)?;
        let rollback = Stage12RollbackRehearsalV3::confirm(
            active.admission.release_id(),
            epoch,
            foundation,
            rollback_assessment,
            active.transaction.transaction(),
        )?;
        let deletion_plan = active.admission.stage12_deletion_plan_v3(
            epoch,
            foundation,
            rollback_assessment,
            &rollback,
        )?;
        Ok((rollback, deletion_plan))
    }

    pub(crate) fn publish_agent_resource_release(
        &mut self,
        active: &mut ActiveAgentResourceReleaseV1,
        publication_objects: ActivePublicationObjectsV1,
    ) -> Result<StorePublicationOutcomeV1, AgentResourceReleaseOperationErrorV1> {
        Ok(self.publish(&mut active.transaction, publication_objects)?)
    }

    pub(crate) fn confirm_agent_resource_reconnect(
        &self,
        active: ActiveAgentResourceReleaseV1,
        closure: &UserAgentInstallationClosureV1,
        observed: &ObservedInstallationClosureV1,
    ) -> Result<CommittedAgentResourceReleaseV1, AgentResourceReleaseOperationErrorV1> {
        Ok(CommittedAgentResourceReleaseV1::confirm(
            active.admission,
            active.transaction.transaction(),
            closure,
            observed,
        )?)
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum AgentResourceReleaseOperationErrorV1 {
    #[error(transparent)]
    Cutover(#[from] AgentResourceCutoverErrorV1),
    #[error(transparent)]
    Installation(#[from] InstallationOperationErrorV1),
}
