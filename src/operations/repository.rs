use std::cell::RefCell;
use std::collections::BTreeMap;
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
use crate::domain::integration::public_literals::{
    CeremonyRequestModeV1, OperationRequestV1, OperationResultBodyV1, OperationSemanticOutcomeV1,
};
use crate::domain::persistence::{StorePublicationOutcomeV1, StoreRoleV1, StoreV1};
use crate::domain::repository::{
    CommittedRepositoryBootstrapV1, RepositoryBootstrapAdmissionV1,
    RepositoryBootstrapDescriptorObservationV1, RepositoryBootstrapDescriptorReadPortV1,
    RepositoryBootstrapEffectObservationV1, RepositoryBootstrapEffectPermitV1,
    RepositoryBootstrapErrorV1, RepositoryBootstrapOwnerFactsV1,
};

use super::action::{
    ActionSubmissionErrorV1, GovernedOperationPortV1, OwnerAdmissionV1, OwnerDurableResultV1,
    OwnerSubmissionOutcomeV1,
};
use super::installation::{
    ActiveDistributionTransactionV1, ActiveInstallationFacadeV1, ActivePublicationObjectsV1,
    AgentResourceReleaseCeremonyV1, AgentResourceReleaseEffectAdapterV1, DistributionEffectPortV1,
    InstallationOperationErrorV1, Stage4EffectReservationBatchV1,
};

#[derive(Debug)]
pub(crate) struct ActiveRepositoryBootstrapV1 {
    admission: RepositoryBootstrapAdmissionV1,
    transaction: ActiveDistributionTransactionV1,
}

#[derive(Debug)]
pub(crate) struct RepositoryBootstrapCeremonyV1 {
    owner_facts: RepositoryBootstrapOwnerFactsV1,
    phase_authorizations: Vec<DistributionPhaseAuthorizationV1>,
    plan_observations: Vec<TargetPlanObservationV1>,
    effect_observations: [RepositoryBootstrapEffectObservationV1; 2],
    publication_objects: ActivePublicationObjectsV1,
    closure: RepositoryInstallationClosureV1,
}

impl RepositoryBootstrapCeremonyV1 {
    #[expect(
        dead_code,
        reason = "the admitted ceremony provider constructs the exact Repository input"
    )]
    pub(crate) fn new(
        owner_facts: RepositoryBootstrapOwnerFactsV1,
        phase_authorizations: Vec<DistributionPhaseAuthorizationV1>,
        plan_observations: Vec<TargetPlanObservationV1>,
        effect_observations: [RepositoryBootstrapEffectObservationV1; 2],
        publication_objects: ActivePublicationObjectsV1,
        closure: RepositoryInstallationClosureV1,
    ) -> Self {
        Self {
            owner_facts,
            phase_authorizations,
            plan_observations,
            effect_observations,
            publication_objects,
            closure,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RepositoryBootstrapBackupReceiptV1 {
    pub target_tag: u64,
    pub effect_fence_commitment: CommitmentV1,
    pub backup_object_commitment: CommitmentV1,
}

pub(crate) trait RepositoryBootstrapEffectPortV1:
    DistributionEffectPortV1 + RepositoryBootstrapDescriptorReadPortV1
{
    fn persist_exact_backups(
        &mut self,
        plan: &DistributionPlanV1,
        captures: &[CapturedTargetPreimageV1],
    ) -> Result<Vec<RepositoryBootstrapBackupReceiptV1>, InstallationOperationErrorV1>;
}

pub(crate) trait RepositoryBootstrapBackupPortV1 {
    fn persist_exact_backups(
        &mut self,
        plan: &DistributionPlanV1,
        captures: &[CapturedTargetPreimageV1],
    ) -> Result<Vec<RepositoryBootstrapBackupReceiptV1>, InstallationOperationErrorV1>;
}

pub(crate) trait RepositoryBootstrapDescriptorPortV1 {
    fn read_exact_targets(
        &mut self,
        plan: &DistributionPlanV1,
    ) -> Result<[RepositoryBootstrapDescriptorObservationV1; 2], InstallationOperationErrorV1>;
}

pub(crate) struct RepositoryBootstrapEffectAdapterV1<'ports> {
    distribution: &'ports mut dyn DistributionEffectPortV1,
    backups: &'ports mut dyn RepositoryBootstrapBackupPortV1,
    descriptors: &'ports mut dyn RepositoryBootstrapDescriptorPortV1,
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
        descriptors: &'ports mut dyn RepositoryBootstrapDescriptorPortV1,
    ) -> Self {
        Self {
            distribution,
            backups,
            descriptors,
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

impl RepositoryBootstrapDescriptorReadPortV1 for RepositoryBootstrapEffectAdapterV1<'_> {
    fn read_exact_targets(
        &mut self,
        plan: &DistributionPlanV1,
    ) -> Result<[RepositoryBootstrapDescriptorObservationV1; 2], RepositoryBootstrapErrorV1> {
        self.descriptors
            .read_exact_targets(plan)
            .map_err(|error| RepositoryBootstrapErrorV1::DescriptorRead(error.to_string()))
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
}

pub(crate) struct ActiveRepositoryFacadeV1<'store> {
    store: &'store mut StoreV1,
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
        Ok(Self { store })
    }

    pub(crate) fn begin_bootstrap(
        &mut self,
        admission: RepositoryBootstrapAdmissionV1,
        phase_authorizations: Vec<DistributionPhaseAuthorizationV1>,
        observations: Vec<TargetPlanObservationV1>,
    ) -> Result<ActiveRepositoryBootstrapV1, RepositoryBootstrapOperationErrorV1> {
        let plan = admission.plan().clone();
        let transaction = ActiveInstallationFacadeV1::new(self.store).begin(
            plan,
            phase_authorizations,
            observations,
        )?;
        Ok(ActiveRepositoryBootstrapV1 {
            admission,
            transaction,
        })
    }

    pub(crate) fn execute_bootstrap(
        &mut self,
        release: crate::domain::installation::CommittedAgentResourceReleaseV1,
        ceremony: RepositoryBootstrapCeremonyV1,
        effects: &mut dyn RepositoryBootstrapEffectPortV1,
    ) -> Result<CommittedRepositoryBootstrapV1, RepositoryBootstrapOperationErrorV1> {
        let admission = RepositoryBootstrapAdmissionV1::after_agent_resource_release(
            release,
            ceremony.owner_facts,
        )?;
        let mut active = self.begin_bootstrap(
            admission,
            ceremony.phase_authorizations,
            ceremony.plan_observations,
        )?;
        let permit = self.authorize_bootstrap(&active, ceremony.effect_observations)?;
        self.drive_bootstrap(&mut active, permit, effects)?;
        self.publish_bootstrap(&mut active, ceremony.publication_objects)?;
        self.confirm_currentness(active, &ceremony.closure, effects)
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
        ActiveInstallationFacadeV1::new(self.store)
            .capture_preimages(&mut active.transaction, effects)?;
        let captures = active.transaction.transaction().captures();
        let receipts = effects.persist_exact_backups(active.admission.plan(), captures)?;
        validate_backup_receipts(captures, &receipts)?;
        ActiveInstallationFacadeV1::new(self.store)
            .drive_captured_to_verification(&mut active.transaction, effects)?;
        Ok(())
    }

    #[expect(
        dead_code,
        reason = "the frozen recovery entrypoint remains available to a recovery-mode owner"
    )]
    pub(crate) fn restore_bootstrap(
        &mut self,
        active: &mut ActiveRepositoryBootstrapV1,
        effects: &mut dyn RepositoryBootstrapEffectPortV1,
    ) -> Result<(), RepositoryBootstrapOperationErrorV1> {
        ActiveInstallationFacadeV1::new(self.store)
            .restore_from_captures(&mut active.transaction, effects)?;
        Ok(())
    }

    pub(crate) fn publish_bootstrap(
        &mut self,
        active: &mut ActiveRepositoryBootstrapV1,
        publication_objects: ActivePublicationObjectsV1,
    ) -> Result<StorePublicationOutcomeV1, RepositoryBootstrapOperationErrorV1> {
        Ok(ActiveInstallationFacadeV1::new(self.store)
            .publish(&mut active.transaction, publication_objects)?)
    }

    pub(crate) fn confirm_currentness(
        &self,
        active: ActiveRepositoryBootstrapV1,
        closure: &RepositoryInstallationClosureV1,
        effects: &mut dyn RepositoryBootstrapEffectPortV1,
    ) -> Result<CommittedRepositoryBootstrapV1, RepositoryBootstrapOperationErrorV1> {
        let readback = active
            .admission
            .acquire_coherent_readback(self.store, closure, effects)?;
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

pub(crate) struct CutoverGovernedOperationPortV1<'owner> {
    state: RefCell<CutoverGovernedOperationStateV1<'owner>>,
}

pub(crate) struct CutoverGovernedOperationAssemblyV1<'owner> {
    pub installation_store: &'owner mut StoreV1,
    pub repository_store: &'owner mut StoreV1,
    pub installation_distribution: &'owner mut dyn DistributionEffectPortV1,
    pub repository_distribution: &'owner mut dyn DistributionEffectPortV1,
    pub repository_backups: &'owner mut dyn RepositoryBootstrapBackupPortV1,
    pub repository_descriptors: &'owner mut dyn RepositoryBootstrapDescriptorPortV1,
    pub installation_ceremony: AgentResourceReleaseCeremonyV1,
    pub repository_ceremony: RepositoryBootstrapCeremonyV1,
}

impl<'owner> CutoverGovernedOperationAssemblyV1<'owner> {
    pub(crate) fn into_port(
        self,
    ) -> Result<CutoverGovernedOperationPortV1<'owner>, RepositoryBootstrapOperationErrorV1> {
        CutoverGovernedOperationPortV1::new(
            self.installation_store,
            self.repository_store,
            self.installation_distribution,
            self.repository_distribution,
            self.repository_backups,
            self.repository_descriptors,
            self.installation_ceremony,
            self.repository_ceremony,
        )
    }
}

struct CutoverGovernedOperationStateV1<'owner> {
    installation: ActiveInstallationFacadeV1<'owner>,
    repository: ActiveRepositoryFacadeV1<'owner>,
    installation_effects: AgentResourceReleaseEffectAdapterV1<'owner>,
    repository_effects: RepositoryBootstrapEffectAdapterV1<'owner>,
    installation_ceremony: Option<AgentResourceReleaseCeremonyV1>,
    repository_ceremony: Option<RepositoryBootstrapCeremonyV1>,
    installation_result: Option<crate::domain::installation::CommittedAgentResourceReleaseV1>,
    durable_results: BTreeMap<String, ([u8; 32], OperationResultBodyV1)>,
}

impl fmt::Debug for CutoverGovernedOperationPortV1<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CutoverGovernedOperationPortV1")
            .finish_non_exhaustive()
    }
}

impl<'owner> CutoverGovernedOperationPortV1<'owner> {
    #[expect(
        clippy::too_many_arguments,
        reason = "the concrete governed port owns both Store and effect boundaries for two frozen Ceremonies"
    )]
    pub(crate) fn new(
        installation_store: &'owner mut StoreV1,
        repository_store: &'owner mut StoreV1,
        installation_distribution: &'owner mut dyn DistributionEffectPortV1,
        repository_distribution: &'owner mut dyn DistributionEffectPortV1,
        repository_backups: &'owner mut dyn RepositoryBootstrapBackupPortV1,
        repository_descriptors: &'owner mut dyn RepositoryBootstrapDescriptorPortV1,
        installation_ceremony: AgentResourceReleaseCeremonyV1,
        repository_ceremony: RepositoryBootstrapCeremonyV1,
    ) -> Result<Self, RepositoryBootstrapOperationErrorV1> {
        if installation_store.role() != StoreRoleV1::Installation {
            return Err(RepositoryBootstrapOperationErrorV1::WrongOwnerDomain);
        }
        Ok(Self {
            state: RefCell::new(CutoverGovernedOperationStateV1 {
                installation: ActiveInstallationFacadeV1::new(installation_store),
                repository: ActiveRepositoryFacadeV1::new(repository_store)?,
                installation_effects: AgentResourceReleaseEffectAdapterV1::new(
                    installation_distribution,
                ),
                repository_effects: RepositoryBootstrapEffectAdapterV1::new(
                    repository_distribution,
                    repository_backups,
                    repository_descriptors,
                ),
                installation_ceremony: Some(installation_ceremony),
                repository_ceremony: Some(repository_ceremony),
                installation_result: None,
                durable_results: BTreeMap::new(),
            }),
        })
    }

    fn submit_installation(
        state: &mut CutoverGovernedOperationStateV1<'_>,
        admission: &OwnerAdmissionV1,
        request: &OperationRequestV1,
    ) -> Result<OwnerSubmissionOutcomeV1, ActionSubmissionErrorV1> {
        let ceremony = state.installation_ceremony.take().ok_or_else(|| {
            ActionSubmissionErrorV1::Owner(
                "InstallationV1Cutover has no unused admitted ceremony input".to_owned(),
            )
        })?;
        let committed = state
            .installation
            .execute_agent_resource_release(ceremony, &mut state.installation_effects)
            .map_err(|error| ActionSubmissionErrorV1::Owner(error.to_string()))?;
        let produced_ref = digest_ref(committed.installation_result_closure());
        state.installation_result = Some(committed);
        Ok(durable_cutover_result(
            state,
            admission,
            request,
            produced_ref,
        ))
    }

    fn submit_repository(
        state: &mut CutoverGovernedOperationStateV1<'_>,
        admission: &OwnerAdmissionV1,
        request: &OperationRequestV1,
    ) -> Result<OwnerSubmissionOutcomeV1, ActionSubmissionErrorV1> {
        let release = state.installation_result.take().ok_or_else(|| {
            ActionSubmissionErrorV1::Owner(
                "RepositoryV1Cutover requires the committed InstallationV1Cutover result"
                    .to_owned(),
            )
        })?;
        let ceremony = state.repository_ceremony.take().ok_or_else(|| {
            ActionSubmissionErrorV1::Owner(
                "RepositoryV1Cutover has no unused admitted ceremony input".to_owned(),
            )
        })?;
        let committed = state
            .repository
            .execute_bootstrap(release, ceremony, &mut state.repository_effects)
            .map_err(|error| ActionSubmissionErrorV1::Owner(error.to_string()))?;
        Ok(durable_cutover_result(
            state,
            admission,
            request,
            digest_ref(committed.repository_result_closure()),
        ))
    }
}

impl GovernedOperationPortV1 for CutoverGovernedOperationPortV1<'_> {
    fn submit(
        &self,
        request: &OperationRequestV1,
        admission: &OwnerAdmissionV1,
    ) -> Result<OwnerSubmissionOutcomeV1, ActionSubmissionErrorV1> {
        let mut state = self.state.borrow_mut();
        if let Some((semantic_hash, body)) = state.durable_results.get(&admission.idempotency_key) {
            return if semantic_hash == &admission.semantic_request_hash {
                Ok(OwnerSubmissionOutcomeV1::Durable(Box::new(
                    OwnerDurableResultV1::replay(admission.clone(), body.clone()),
                )))
            } else {
                Ok(OwnerSubmissionOutcomeV1::SameKeyDifferentMeaning)
            };
        }
        if !matches!(
            request,
            OperationRequestV1::Ceremony(ceremony)
                if ceremony.request_mode == CeremonyRequestModeV1::Initiate
        ) {
            return Ok(OwnerSubmissionOutcomeV1::OwnerUnavailable { inspect_ref: None });
        }
        match admission.operation_name.as_str() {
            "InstallationV1Cutover" => Self::submit_installation(&mut state, admission, request),
            "RepositoryV1Cutover" => Self::submit_repository(&mut state, admission, request),
            _ => Ok(OwnerSubmissionOutcomeV1::OwnerUnavailable { inspect_ref: None }),
        }
    }
}

fn durable_cutover_result(
    state: &mut CutoverGovernedOperationStateV1<'_>,
    admission: &OwnerAdmissionV1,
    request: &OperationRequestV1,
    produced_ref: String,
) -> OwnerSubmissionOutcomeV1 {
    let OperationRequestV1::Ceremony(ceremony) = request else {
        unreachable!("invariant: cutover dispatch admits only frozen Ceremonies");
    };
    let body = OperationResultBodyV1 {
        schema_version: 1,
        request_id: ceremony.request_id.clone(),
        operation_spec_ref: ceremony.ceremony_spec.exact_ceremony_spec_ref.clone(),
        outcome: OperationSemanticOutcomeV1::Committed,
        before_revision_refs: Vec::new(),
        after_revision_refs: Vec::new(),
        transition_receipt_refs: Vec::new(),
        produced_record_refs: vec![produced_ref],
        next_packet: None,
        inspect_ref: None,
        replayed_delivery: false,
    };
    state.durable_results.insert(
        admission.idempotency_key.clone(),
        (admission.semantic_request_hash, body.clone()),
    );
    OwnerSubmissionOutcomeV1::Durable(Box::new(OwnerDurableResultV1::fresh(
        admission.clone(),
        body,
    )))
}

fn digest_ref(value: [u8; 32]) -> String {
    let mut encoded = String::from("sha256:");
    for byte in value {
        use std::fmt::Write;
        write!(&mut encoded, "{byte:02x}")
            .expect("invariant: writing hexadecimal into a String cannot fail");
    }
    encoded
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
        let execute = source.find("pub(crate) fn execute_bootstrap").unwrap();
        let admission = source[execute..]
            .find("RepositoryBootstrapAdmissionV1::after_agent_resource_release")
            .unwrap();
        let begin = source[execute..].find("self.begin_bootstrap").unwrap();
        let drive = source[execute..].find("self.drive_bootstrap").unwrap();
        let publish = source[execute..].find("self.publish_bootstrap").unwrap();
        let confirm = source[execute..].find("self.confirm_currentness").unwrap();
        assert!(admission < begin && begin < drive && drive < publish && publish < confirm);

        let capture = source.find(".capture_preimages").unwrap();
        let backup = source.find("effects.persist_exact_backups").unwrap();
        let validate = source.find("validate_backup_receipts(captures").unwrap();
        let cross = source.find(".drive_captured_to_verification").unwrap();
        assert!(capture < backup && backup < validate && validate < cross);

        let restore = source.find(".restore_from_captures").unwrap();
        let publish_transaction = source.find(".publish(&mut active.transaction").unwrap();
        assert!(restore < publish_transaction);

        let bootstrap = include_str!("../domain/repository/bootstrap.rs");
        let before = bootstrap
            .find("let before = repository_snapshot_binding")
            .unwrap();
        let read = bootstrap
            .find("let observations = reader.read_exact_targets")
            .unwrap();
        let after = bootstrap
            .find("let after = repository_snapshot_binding")
            .unwrap();
        let compare = bootstrap.find("if before != after").unwrap();
        let validate_readback = bootstrap
            .find("self.validated_readback(observations)")
            .unwrap();
        assert!(before < read && read < after && after < compare && compare < validate_readback);
        assert!(!bootstrap.contains("RepositoryBootstrapReadbackV1::new"));
        assert!(!bootstrap.contains("pub targets:"));

        let adapter = include_str!("../interfaces/cli/adapter.rs");
        assert!(adapter.contains("operation_assembly.into_port()"));
        assert!(adapter.contains("service.submit(operation_port, &request)"));
        let installation_dispatch = source
            .find("\"InstallationV1Cutover\" => Self::submit_installation")
            .unwrap();
        let repository_dispatch = source
            .find("\"RepositoryV1Cutover\" => Self::submit_repository")
            .unwrap();
        let installation_execute = source.find(".execute_agent_resource_release").unwrap();
        let repository_execute = source.find(".execute_bootstrap").unwrap();
        assert!(installation_dispatch < repository_dispatch);
        assert!(installation_execute < repository_execute);
    }
}
