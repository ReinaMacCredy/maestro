use crate::domain::distribution::runtime::{
    DistributionPhaseAuthorizationV1, DistributionPlanV1, TargetPlanObservationV1,
};
use crate::domain::installation::{
    AgentResourceCutoverErrorV1, AgentResourceReleaseAdmissionV1, CommittedAgentResourceReleaseV1,
    ObservedInstallationClosureV1, RepositoryBootstrapAdmissionV1, UserAgentInstallationClosureV1,
};
use crate::domain::persistence::StorePublicationOutcomeV1;

use super::{
    ActiveDistributionTransactionV1, ActiveInstallationFacadeV1, ActivePublicationObjectsV1,
    DistributionEffectPortV1, InstallationOperationErrorV1,
};

pub(crate) struct ActiveAgentResourceReleaseV1 {
    admission: AgentResourceReleaseAdmissionV1,
    transaction: ActiveDistributionTransactionV1,
}

impl ActiveAgentResourceReleaseV1 {
    pub(crate) fn transaction(&self) -> &ActiveDistributionTransactionV1 {
        &self.transaction
    }
}

impl ActiveInstallationFacadeV1<'_> {
    pub(crate) fn begin_agent_resource_release(
        &mut self,
        admission: AgentResourceReleaseAdmissionV1,
        plan: DistributionPlanV1,
        phase_authorizations: Vec<DistributionPhaseAuthorizationV1>,
        observations: Vec<TargetPlanObservationV1>,
    ) -> Result<ActiveAgentResourceReleaseV1, AgentResourceReleaseOperationErrorV1> {
        admission.validate_plan(&plan)?;
        let transaction = self.begin(plan, phase_authorizations, observations)?;
        Ok(ActiveAgentResourceReleaseV1 {
            admission,
            transaction,
        })
    }

    pub(crate) fn drive_agent_resource_release(
        &mut self,
        active: &mut ActiveAgentResourceReleaseV1,
        effects: &mut impl DistributionEffectPortV1,
    ) -> Result<(), AgentResourceReleaseOperationErrorV1> {
        self.drive_to_verification(&mut active.transaction, effects)?;
        Ok(())
    }

    pub(crate) fn restore_agent_resource_release(
        &mut self,
        active: &mut ActiveAgentResourceReleaseV1,
        effects: &mut impl DistributionEffectPortV1,
    ) -> Result<(), AgentResourceReleaseOperationErrorV1> {
        self.restore_from_captures(&mut active.transaction, effects)?;
        Ok(())
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

    pub(crate) fn begin_repository_bootstrap(
        &mut self,
        admission: &RepositoryBootstrapAdmissionV1,
        plan: DistributionPlanV1,
        phase_authorizations: Vec<DistributionPhaseAuthorizationV1>,
        observations: Vec<TargetPlanObservationV1>,
    ) -> Result<ActiveDistributionTransactionV1, AgentResourceReleaseOperationErrorV1> {
        admission.validate_plan(&plan)?;
        Ok(self.begin(plan, phase_authorizations, observations)?)
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum AgentResourceReleaseOperationErrorV1 {
    #[error(transparent)]
    Cutover(#[from] AgentResourceCutoverErrorV1),
    #[error(transparent)]
    Installation(#[from] InstallationOperationErrorV1),
}
