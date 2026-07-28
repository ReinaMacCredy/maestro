use std::marker::PhantomData;
use std::rc::Rc;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::authority::LegacyRemovalGuardV3;
use crate::domain::capability::literals::{
    CanonicalAgentResourceInventoryV1, LegacySkillDispositionV1,
};
use crate::domain::distribution::runtime::{
    CustodyAssessmentV1, CutoverPlanOwnerFactsV1, DistributionDomainKindV1,
    DistributionDomainRefV1, DistributionMutationKindV1, DistributionPlanTargetV1,
    DistributionPlanV1, DistributionScopedObjectRefV1, DistributionTransactionPhaseV1,
    DistributionTransactionV1, TargetCustodyClassV1, TargetEffectKindV1,
};
use crate::domain::distribution::{CommitmentV1, ReleaseIdV1};
use crate::domain::migration::runtime::{
    DeclaredOverlapManifestV2, LegacyQuarantineEpochV3, LegacyQuarantineEpochV4,
    LegacyRollbackAssessmentV4, LegacySourceCaseManifestV3, MigrationClassificationManifestV3,
    MigrationDigestV1, SealedQuarantineManifestV3, Stage12SightingManifestV2,
    UnavailablePreexistingLossManifestV4,
};
use crate::foundation::core::FoundationLegacyQuarantineClosureV2;

use super::consumer_snapshot::{
    AgentResourceReleaseConsumerSealV1, ConsumerClosureReceiptV1, PhysicalPruningConsumerStageV1,
    PreCurrentnessConsumerStageV1, ProtectedRetentionConsumerStageV1,
};
use super::{
    DomainCurrentnessV1, ObservedInstallationClosureV1, UserAgentInstallationClosureV1,
    assess_user_agent_currentness,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum AgentResourceTargetKindV1 {
    GlobalSkillsLock,
    CanonicalSkillCache,
    AgentsActivationLink,
    ClaudeActivationLink,
    HostRegistration,
    DiscoveryRecord,
    LegacyDiscoveryDeactivation,
}

impl AgentResourceTargetKindV1 {
    const ALL: [Self; 7] = [
        Self::GlobalSkillsLock,
        Self::CanonicalSkillCache,
        Self::AgentsActivationLink,
        Self::ClaudeActivationLink,
        Self::HostRegistration,
        Self::DiscoveryRecord,
        Self::LegacyDiscoveryDeactivation,
    ];
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AgentResourceTargetOwnerFactsV1 {
    pub target_identity: CommitmentV1,
    pub expected_preimage: CommitmentV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentResourceReleaseOwnerFactsV1 {
    plan: CutoverPlanOwnerFactsV1<7>,
    targets: [AgentResourceTargetOwnerFactsV1; 7],
}

impl AgentResourceReleaseOwnerFactsV1 {
    pub(crate) fn new(
        plan: CutoverPlanOwnerFactsV1<7>,
        targets: [AgentResourceTargetOwnerFactsV1; 7],
    ) -> Result<Self, AgentResourceCutoverErrorV1> {
        if plan.domain.kind() != DistributionDomainKindV1::InstallationDomain
            || plan
                .target_custodies
                .iter()
                .any(|custody| custody.class() != TargetCustodyClassV1::MaestroOwnedTarget)
            || targets.iter().any(|facts| {
                facts.target_identity.as_bytes() == &[0; 32]
                    || facts.expected_preimage.as_bytes() == &[0; 32]
            })
        {
            return Err(AgentResourceCutoverErrorV1::InvalidOwnerFacts);
        }
        if targets
            .iter()
            .map(|facts| facts.target_identity)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != targets.len()
        {
            return Err(AgentResourceCutoverErrorV1::InvalidOwnerFacts);
        }
        Ok(Self { plan, targets })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AgentResourceJournalBindingV1 {
    kind: AgentResourceTargetKindV1,
    target_tag: u64,
    target_identity: CommitmentV1,
    expected_preimage: CommitmentV1,
    candidate: Option<CommitmentV1>,
    effect_kind: TargetEffectKindV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AgentResourceJournalRowV1 {
    ordinal: u64,
    logical_path: &'static str,
    embedded_source_path: &'static str,
    content_sha256: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LegacyDispositionJournalRowV1 {
    ordinal: u64,
    source_path: &'static str,
    disposition: LegacySkillDispositionV1,
    active_destination: Option<&'static str>,
}

#[derive(Debug)]
pub(crate) struct AgentResourceReleaseAdmissionV1 {
    release_id: ReleaseIdV1,
    resource_inventory_closure: [u8; 32],
    legacy_ledger_closure: [u8; 32],
    consumer_closure: [u8; 32],
    journal_closure: [u8; 32],
    resources: [AgentResourceJournalRowV1; 31],
    dispositions: [LegacyDispositionJournalRowV1; 35],
    targets: [AgentResourceJournalBindingV1; 7],
    plan: DistributionPlanV1,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl AgentResourceReleaseAdmissionV1 {
    pub(crate) fn new(
        release_id: ReleaseIdV1,
        consumer_seal: AgentResourceReleaseConsumerSealV1,
        owner_facts: AgentResourceReleaseOwnerFactsV1,
    ) -> Result<Self, AgentResourceCutoverErrorV1> {
        if release_id.as_bytes() == &[0; 32] {
            return Err(AgentResourceCutoverErrorV1::InvalidAdmission);
        }
        let inventory = CanonicalAgentResourceInventoryV1::load_embedded()
            .map_err(|_| AgentResourceCutoverErrorV1::InvalidCanonicalInventory)?;
        let resources = std::array::from_fn(|index| {
            let row = inventory.resources()[index];
            AgentResourceJournalRowV1 {
                ordinal: index as u64 + 1,
                logical_path: row.logical_path,
                embedded_source_path: row.embedded_source_path,
                content_sha256: row.content_sha256,
            }
        });
        let dispositions = std::array::from_fn(|index| {
            let row = inventory.legacy_ledger()[index];
            LegacyDispositionJournalRowV1 {
                ordinal: index as u64 + 1,
                source_path: row.source_path,
                disposition: row.disposition,
                active_destination: row.active_destination,
            }
        });
        let journal = owner_derived_agent_resource_journal(
            release_id,
            consumer_seal.closure(),
            inventory.resource_closure(),
            inventory.legacy_ledger_closure(),
            &owner_facts,
        );
        let plan = build_agent_resource_plan(release_id, &owner_facts.plan, &journal)?;
        let journal_closure = agent_resource_journal_closure(&resources, &dispositions, &journal);
        Ok(Self {
            release_id,
            resource_inventory_closure: inventory.resource_closure(),
            legacy_ledger_closure: inventory.legacy_ledger_closure(),
            consumer_closure: consumer_seal.closure(),
            journal_closure,
            resources,
            dispositions,
            targets: journal,
            plan,
            _not_send_or_sync: PhantomData,
        })
    }

    pub(crate) fn validate_plan(
        &self,
        plan: &DistributionPlanV1,
    ) -> Result<(), AgentResourceCutoverErrorV1> {
        if plan != &self.plan
            || !self.has_exact_embedded_journal()?
            || self.journal_closure
                != agent_resource_journal_closure(
                    &self.resources,
                    &self.dispositions,
                    &self.targets,
                )
        {
            return Err(AgentResourceCutoverErrorV1::PlanMismatch);
        }
        Ok(())
    }

    pub(crate) fn plan(&self) -> &DistributionPlanV1 {
        &self.plan
    }

    pub(crate) const fn release_id(&self) -> ReleaseIdV1 {
        self.release_id
    }

    pub(crate) const fn resource_inventory_closure(&self) -> [u8; 32] {
        self.resource_inventory_closure
    }

    pub(crate) const fn legacy_ledger_closure(&self) -> [u8; 32] {
        self.legacy_ledger_closure
    }

    pub(crate) const fn finality_closure(&self) -> [u8; 32] {
        self.journal_closure
    }

    pub(crate) fn stage12_deletion_plan_v2(
        &self,
        epoch: &LegacyQuarantineEpochV3,
        rollback: &Stage12RollbackRehearsalV2,
    ) -> Result<InstallationLegacyDeletionPlanV2, AgentResourceCutoverErrorV1> {
        let release_id = migration_release_id(self.release_id)?;
        if epoch.release_id() != release_id
            || rollback.release_id() != release_id
            || rollback.legacy_quarantine_epoch_id() != epoch.identity()
        {
            return Err(AgentResourceCutoverErrorV1::Stage12BindingMismatch);
        }
        let rollback_rehearsal_id = rollback.identity();
        let identity = migration_commitment(
            b"maestro.vnext.installation-legacy-deletion-plan.v2",
            &[
                *release_id.as_bytes(),
                *epoch.identity().as_bytes(),
                *rollback_rehearsal_id.as_bytes(),
                self.legacy_ledger_closure,
                self.journal_closure,
                *self.plan.meaning_digest().as_bytes(),
            ],
        )?;
        Ok(InstallationLegacyDeletionPlanV2 {
            identity,
            release_id,
            legacy_quarantine_epoch_id: epoch.identity(),
            rollback_rehearsal_id,
            _not_send_or_sync: PhantomData,
        })
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "the V4 deletion plan binds each independently owner-issued final-cut fact"
    )]
    pub(crate) fn stage12_deletion_plan_v3(
        &self,
        epoch: &LegacyQuarantineEpochV4,
        foundation: &FoundationLegacyQuarantineClosureV2,
        rollback_assessment: &LegacyRollbackAssessmentV4,
        rollback_rehearsal: &Stage12RollbackRehearsalV3,
    ) -> Result<InstallationLegacyDeletionPlanV3, AgentResourceCutoverErrorV1> {
        let release_id = migration_release_id(self.release_id)?;
        let foundation_closure_id = migration_digest(foundation.identity())?;
        if epoch.release_id() != release_id
            || epoch.foundation_closure_id() != foundation_closure_id
            || epoch.rollback_plan_id() != rollback_assessment.identity()
            || rollback_rehearsal.release_id() != release_id
            || rollback_rehearsal.legacy_quarantine_epoch_id() != epoch.identity()
            || rollback_rehearsal.foundation_closure_id() != foundation_closure_id
            || rollback_rehearsal.rollback_assessment_id() != rollback_assessment.identity()
        {
            return Err(AgentResourceCutoverErrorV1::Stage12BindingMismatch);
        }
        let rollback_rehearsal_id = rollback_rehearsal.identity();
        let identity = migration_commitment(
            b"maestro.vnext.installation-legacy-deletion-plan.v3",
            &[
                *release_id.as_bytes(),
                *epoch.identity().as_bytes(),
                *foundation_closure_id.as_bytes(),
                *rollback_assessment.identity().as_bytes(),
                *rollback_rehearsal_id.as_bytes(),
                self.legacy_ledger_closure,
                self.journal_closure,
                *self.plan.meaning_digest().as_bytes(),
            ],
        )?;
        Ok(InstallationLegacyDeletionPlanV3 {
            identity,
            release_id,
            legacy_quarantine_epoch_id: epoch.identity(),
            foundation_closure_id,
            rollback_assessment_id: rollback_assessment.identity(),
            rollback_rehearsal_id,
            _not_send_or_sync: PhantomData,
        })
    }

    fn has_exact_embedded_journal(&self) -> Result<bool, AgentResourceCutoverErrorV1> {
        let inventory = CanonicalAgentResourceInventoryV1::load_embedded()
            .map_err(|_| AgentResourceCutoverErrorV1::InvalidCanonicalInventory)?;
        Ok(self.resources
            == std::array::from_fn(|index| {
                let row = inventory.resources()[index];
                AgentResourceJournalRowV1 {
                    ordinal: index as u64 + 1,
                    logical_path: row.logical_path,
                    embedded_source_path: row.embedded_source_path,
                    content_sha256: row.content_sha256,
                }
            })
            && self.dispositions
                == std::array::from_fn(|index| {
                    let row = inventory.legacy_ledger()[index];
                    LegacyDispositionJournalRowV1 {
                        ordinal: index as u64 + 1,
                        source_path: row.source_path,
                        disposition: row.disposition,
                        active_destination: row.active_destination,
                    }
                }))
    }
}

#[derive(Debug)]
pub(crate) struct CommittedAgentResourceReleaseV1 {
    release_id: ReleaseIdV1,
    installation_result_closure: [u8; 32],
    reconnect_closure: [u8; 32],
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl CommittedAgentResourceReleaseV1 {
    pub(crate) fn confirm(
        admission: AgentResourceReleaseAdmissionV1,
        transaction: &DistributionTransactionV1,
        closure: &UserAgentInstallationClosureV1,
        observed: &ObservedInstallationClosureV1,
    ) -> Result<Self, AgentResourceCutoverErrorV1> {
        admission.validate_plan(transaction.plan())?;
        if transaction.phase() != DistributionTransactionPhaseV1::Committed
            || closure.release_id != admission.release_id
            || observed.release_id != admission.release_id
            || assess_user_agent_currentness(closure, observed) != DomainCurrentnessV1::Coherent
        {
            return Err(AgentResourceCutoverErrorV1::ReconnectNotCurrent);
        }
        let installation_result_closure = commitment(
            b"maestro.vnext.agent-resource-installation-result.v1",
            &[
                *admission.release_id.as_bytes(),
                admission.resource_inventory_closure,
                admission.legacy_ledger_closure,
                admission.consumer_closure,
                admission.journal_closure,
                *transaction.plan().meaning_digest().as_bytes(),
            ],
        );
        let reconnect_closure = commitment(
            b"maestro.vnext.agent-resource-host-reconnect.v1",
            &[
                installation_result_closure,
                *closure.release_id.as_bytes(),
                *observed.release_id.as_bytes(),
            ],
        );
        Ok(Self {
            release_id: admission.release_id,
            installation_result_closure,
            reconnect_closure,
            _not_send_or_sync: PhantomData,
        })
    }

    pub(crate) const fn release_id(&self) -> ReleaseIdV1 {
        self.release_id
    }

    pub(crate) const fn installation_result_closure(&self) -> [u8; 32] {
        self.installation_result_closure
    }

    pub(crate) const fn reconnect_closure(&self) -> [u8; 32] {
        self.reconnect_closure
    }

    pub(in crate::domain::installation) fn stage12_replacement_activation_v2(
        &self,
        epoch: &LegacyQuarantineEpochV3,
        sightings: &Stage12SightingManifestV2,
    ) -> Result<Stage12ReplacementActivationV2, AgentResourceCutoverErrorV1> {
        let release_id = migration_release_id(self.release_id)?;
        if epoch.release_id() != release_id || epoch.sighting_manifest_id() != sightings.identity()
        {
            return Err(AgentResourceCutoverErrorV1::Stage12BindingMismatch);
        }
        let identity = migration_commitment(
            b"maestro.vnext.stage12-replacement-activation.v2",
            &[
                *release_id.as_bytes(),
                *epoch.identity().as_bytes(),
                *sightings.identity().as_bytes(),
                self.installation_result_closure,
                self.reconnect_closure,
            ],
        )?;
        Ok(Stage12ReplacementActivationV2 {
            identity,
            release_id,
            legacy_quarantine_epoch_id: epoch.identity(),
            sighting_manifest_id: sightings.identity(),
            _not_send_or_sync: PhantomData,
        })
    }

    pub(in crate::domain::installation) fn stage12_replacement_activation_v3(
        &self,
        epoch: &LegacyQuarantineEpochV4,
        sightings: &Stage12SightingManifestV2,
        foundation: &FoundationLegacyQuarantineClosureV2,
        rollback_assessment: &LegacyRollbackAssessmentV4,
    ) -> Result<Stage12ReplacementActivationV3, AgentResourceCutoverErrorV1> {
        let release_id = migration_release_id(self.release_id)?;
        let foundation_closure_id = migration_digest(foundation.identity())?;
        if epoch.release_id() != release_id
            || epoch.sighting_manifest_id() != sightings.identity()
            || epoch.foundation_closure_id() != foundation_closure_id
            || epoch.rollback_plan_id() != rollback_assessment.identity()
        {
            return Err(AgentResourceCutoverErrorV1::Stage12BindingMismatch);
        }
        let identity = migration_commitment(
            b"maestro.vnext.stage12-replacement-activation.v3",
            &[
                *release_id.as_bytes(),
                *epoch.identity().as_bytes(),
                *sightings.identity().as_bytes(),
                *foundation_closure_id.as_bytes(),
                *rollback_assessment.identity().as_bytes(),
                self.installation_result_closure,
                self.reconnect_closure,
            ],
        )?;
        Ok(Stage12ReplacementActivationV3 {
            identity,
            release_id,
            legacy_quarantine_epoch_id: epoch.identity(),
            sighting_manifest_id: sightings.identity(),
            foundation_closure_id,
            rollback_assessment_id: rollback_assessment.identity(),
            _not_send_or_sync: PhantomData,
        })
    }

    #[cfg(test)]
    pub(in crate::domain) fn test_committed(
        release_id: ReleaseIdV1,
        installation_result_closure: [u8; 32],
        reconnect_closure: [u8; 32],
    ) -> Self {
        Self {
            release_id,
            installation_result_closure,
            reconnect_closure,
            _not_send_or_sync: PhantomData,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Stage12ReplacementActivationV2 {
    identity: MigrationDigestV1,
    release_id: MigrationDigestV1,
    legacy_quarantine_epoch_id: MigrationDigestV1,
    sighting_manifest_id: MigrationDigestV1,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl Stage12ReplacementActivationV2 {
    pub(crate) const fn identity(&self) -> MigrationDigestV1 {
        self.identity
    }

    pub(crate) const fn release_id(&self) -> MigrationDigestV1 {
        self.release_id
    }

    pub(crate) const fn legacy_quarantine_epoch_id(&self) -> MigrationDigestV1 {
        self.legacy_quarantine_epoch_id
    }

    pub(crate) const fn sighting_manifest_id(&self) -> MigrationDigestV1 {
        self.sighting_manifest_id
    }
}

pub(crate) struct Stage12ReplacementActivationV3 {
    identity: MigrationDigestV1,
    release_id: MigrationDigestV1,
    legacy_quarantine_epoch_id: MigrationDigestV1,
    sighting_manifest_id: MigrationDigestV1,
    foundation_closure_id: MigrationDigestV1,
    rollback_assessment_id: MigrationDigestV1,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl Stage12ReplacementActivationV3 {
    pub(crate) const fn identity(&self) -> MigrationDigestV1 {
        self.identity
    }

    pub(crate) const fn release_id(&self) -> MigrationDigestV1 {
        self.release_id
    }

    pub(crate) const fn legacy_quarantine_epoch_id(&self) -> MigrationDigestV1 {
        self.legacy_quarantine_epoch_id
    }

    pub(crate) const fn sighting_manifest_id(&self) -> MigrationDigestV1 {
        self.sighting_manifest_id
    }

    pub(crate) const fn foundation_closure_id(&self) -> MigrationDigestV1 {
        self.foundation_closure_id
    }

    pub(crate) const fn rollback_assessment_id(&self) -> MigrationDigestV1 {
        self.rollback_assessment_id
    }
}

#[derive(Debug)]
pub(crate) struct Stage12RollbackRehearsalV2 {
    identity: MigrationDigestV1,
    release_id: MigrationDigestV1,
    legacy_quarantine_epoch_id: MigrationDigestV1,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl Stage12RollbackRehearsalV2 {
    pub(crate) fn confirm(
        release_id: ReleaseIdV1,
        epoch: &LegacyQuarantineEpochV3,
        transaction: &DistributionTransactionV1,
    ) -> Result<Self, AgentResourceCutoverErrorV1> {
        let release_id = migration_release_id(release_id)?;
        if epoch.release_id() != release_id
            || transaction.phase() != DistributionTransactionPhaseV1::RolledBack
        {
            return Err(AgentResourceCutoverErrorV1::Stage12BindingMismatch);
        }
        let identity = migration_commitment(
            b"maestro.vnext.stage12-rollback-rehearsal.v2",
            &[
                *release_id.as_bytes(),
                *epoch.identity().as_bytes(),
                *transaction.plan().meaning_digest().as_bytes(),
            ],
        )?;
        Ok(Self {
            identity,
            release_id,
            legacy_quarantine_epoch_id: epoch.identity(),
            _not_send_or_sync: PhantomData,
        })
    }

    pub(crate) const fn identity(&self) -> MigrationDigestV1 {
        self.identity
    }

    pub(crate) const fn release_id(&self) -> MigrationDigestV1 {
        self.release_id
    }

    pub(crate) const fn legacy_quarantine_epoch_id(&self) -> MigrationDigestV1 {
        self.legacy_quarantine_epoch_id
    }
}

pub(crate) struct Stage12RollbackRehearsalV3 {
    identity: MigrationDigestV1,
    release_id: MigrationDigestV1,
    legacy_quarantine_epoch_id: MigrationDigestV1,
    foundation_closure_id: MigrationDigestV1,
    rollback_assessment_id: MigrationDigestV1,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl Stage12RollbackRehearsalV3 {
    pub(crate) fn confirm(
        release_id: ReleaseIdV1,
        epoch: &LegacyQuarantineEpochV4,
        foundation: &FoundationLegacyQuarantineClosureV2,
        rollback_assessment: &LegacyRollbackAssessmentV4,
        transaction: &DistributionTransactionV1,
    ) -> Result<Self, AgentResourceCutoverErrorV1> {
        let release_id = migration_release_id(release_id)?;
        let foundation_closure_id = migration_digest(foundation.identity())?;
        if epoch.release_id() != release_id
            || epoch.foundation_closure_id() != foundation_closure_id
            || epoch.rollback_plan_id() != rollback_assessment.identity()
            || transaction.phase() != DistributionTransactionPhaseV1::RolledBack
        {
            return Err(AgentResourceCutoverErrorV1::Stage12BindingMismatch);
        }
        let identity = migration_commitment(
            b"maestro.vnext.stage12-rollback-rehearsal.v3",
            &[
                *release_id.as_bytes(),
                *epoch.identity().as_bytes(),
                *foundation_closure_id.as_bytes(),
                *rollback_assessment.identity().as_bytes(),
                *transaction.plan().meaning_digest().as_bytes(),
            ],
        )?;
        Ok(Self {
            identity,
            release_id,
            legacy_quarantine_epoch_id: epoch.identity(),
            foundation_closure_id,
            rollback_assessment_id: rollback_assessment.identity(),
            _not_send_or_sync: PhantomData,
        })
    }

    pub(crate) const fn identity(&self) -> MigrationDigestV1 {
        self.identity
    }

    pub(crate) const fn release_id(&self) -> MigrationDigestV1 {
        self.release_id
    }

    pub(crate) const fn legacy_quarantine_epoch_id(&self) -> MigrationDigestV1 {
        self.legacy_quarantine_epoch_id
    }

    pub(crate) const fn foundation_closure_id(&self) -> MigrationDigestV1 {
        self.foundation_closure_id
    }

    pub(crate) const fn rollback_assessment_id(&self) -> MigrationDigestV1 {
        self.rollback_assessment_id
    }
}

#[derive(Debug)]
pub(crate) struct InstallationLegacyDeletionPlanV2 {
    identity: MigrationDigestV1,
    release_id: MigrationDigestV1,
    legacy_quarantine_epoch_id: MigrationDigestV1,
    rollback_rehearsal_id: MigrationDigestV1,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl InstallationLegacyDeletionPlanV2 {
    pub(crate) const fn identity(&self) -> MigrationDigestV1 {
        self.identity
    }

    pub(crate) const fn release_id(&self) -> MigrationDigestV1 {
        self.release_id
    }

    pub(crate) const fn legacy_quarantine_epoch_id(&self) -> MigrationDigestV1 {
        self.legacy_quarantine_epoch_id
    }

    pub(crate) const fn rollback_rehearsal_id(&self) -> MigrationDigestV1 {
        self.rollback_rehearsal_id
    }
}

pub(crate) struct InstallationLegacyDeletionPlanV3 {
    identity: MigrationDigestV1,
    release_id: MigrationDigestV1,
    legacy_quarantine_epoch_id: MigrationDigestV1,
    foundation_closure_id: MigrationDigestV1,
    rollback_assessment_id: MigrationDigestV1,
    rollback_rehearsal_id: MigrationDigestV1,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl InstallationLegacyDeletionPlanV3 {
    pub(crate) const fn identity(&self) -> MigrationDigestV1 {
        self.identity
    }

    pub(crate) const fn release_id(&self) -> MigrationDigestV1 {
        self.release_id
    }

    pub(crate) const fn legacy_quarantine_epoch_id(&self) -> MigrationDigestV1 {
        self.legacy_quarantine_epoch_id
    }

    pub(crate) const fn foundation_closure_id(&self) -> MigrationDigestV1 {
        self.foundation_closure_id
    }

    pub(crate) const fn rollback_assessment_id(&self) -> MigrationDigestV1 {
        self.rollback_assessment_id
    }

    pub(crate) const fn rollback_rehearsal_id(&self) -> MigrationDigestV1 {
        self.rollback_rehearsal_id
    }
}

#[derive(Debug)]
pub(crate) struct Stage12ConsumerReaderHoldClosureV2 {
    identity: MigrationDigestV1,
    release_id: MigrationDigestV1,
    legacy_quarantine_epoch_id: MigrationDigestV1,
    sighting_manifest_id: MigrationDigestV1,
    replacement_activation_id: MigrationDigestV1,
    rollback_rehearsal_id: MigrationDigestV1,
    deletion_plan_id: MigrationDigestV1,
    physical_pruning_reader_zero_id: MigrationDigestV1,
    physical_pruning_hold_zero_id: MigrationDigestV1,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl Stage12ConsumerReaderHoldClosureV2 {
    #[expect(
        clippy::too_many_arguments,
        reason = "Stage 12 closure binds every independently owned finality receipt"
    )]
    pub(in crate::domain::installation) fn seal(
        epoch: &LegacyQuarantineEpochV3,
        sightings: &Stage12SightingManifestV2,
        activation: &Stage12ReplacementActivationV2,
        rollback: &Stage12RollbackRehearsalV2,
        deletion_plan: &InstallationLegacyDeletionPlanV2,
        pre_currentness: ConsumerClosureReceiptV1<'_, '_, PreCurrentnessConsumerStageV1>,
        protected_retention: ConsumerClosureReceiptV1<'_, '_, ProtectedRetentionConsumerStageV1>,
        physical_pruning: ConsumerClosureReceiptV1<'_, '_, PhysicalPruningConsumerStageV1>,
    ) -> Result<Self, AgentResourceCutoverErrorV1> {
        let release_id = epoch.release_id();
        if epoch.sighting_manifest_id() != sightings.identity()
            || activation.release_id() != release_id
            || activation.legacy_quarantine_epoch_id() != epoch.identity()
            || activation.sighting_manifest_id() != sightings.identity()
            || rollback.release_id() != release_id
            || rollback.legacy_quarantine_epoch_id() != epoch.identity()
            || deletion_plan.release_id() != release_id
            || deletion_plan.legacy_quarantine_epoch_id() != epoch.identity()
            || deletion_plan.rollback_rehearsal_id() != rollback.identity()
        {
            return Err(AgentResourceCutoverErrorV1::Stage12BindingMismatch);
        }
        let pre_currentness_id = migration_digest(pre_currentness.finality_commitment())?;
        let protected_retention_id = migration_digest(protected_retention.finality_commitment())?;
        let physical_pruning_id = migration_digest(physical_pruning.finality_commitment())?;
        let physical_pruning_reader_zero_id = migration_commitment(
            b"maestro.vnext.physical-pruning-reader-zero.v2",
            &[*physical_pruning_id.as_bytes()],
        )?;
        let physical_pruning_hold_zero_id = migration_commitment(
            b"maestro.vnext.physical-pruning-hold-zero.v2",
            &[*physical_pruning_id.as_bytes()],
        )?;
        let identity = migration_commitment(
            b"maestro.vnext.stage12-consumer-reader-hold-closure.v2",
            &[
                *release_id.as_bytes(),
                *epoch.identity().as_bytes(),
                *sightings.identity().as_bytes(),
                *activation.identity().as_bytes(),
                *rollback.identity().as_bytes(),
                *deletion_plan.identity().as_bytes(),
                *pre_currentness_id.as_bytes(),
                *protected_retention_id.as_bytes(),
                *physical_pruning_reader_zero_id.as_bytes(),
                *physical_pruning_hold_zero_id.as_bytes(),
            ],
        )?;
        Ok(Self {
            identity,
            release_id,
            legacy_quarantine_epoch_id: epoch.identity(),
            sighting_manifest_id: sightings.identity(),
            replacement_activation_id: activation.identity(),
            rollback_rehearsal_id: rollback.identity(),
            deletion_plan_id: deletion_plan.identity(),
            physical_pruning_reader_zero_id,
            physical_pruning_hold_zero_id,
            _not_send_or_sync: PhantomData,
        })
    }

    pub(crate) const fn identity(&self) -> MigrationDigestV1 {
        self.identity
    }

    pub(crate) const fn release_id(&self) -> MigrationDigestV1 {
        self.release_id
    }

    pub(crate) const fn legacy_quarantine_epoch_id(&self) -> MigrationDigestV1 {
        self.legacy_quarantine_epoch_id
    }

    pub(crate) const fn sighting_manifest_id(&self) -> MigrationDigestV1 {
        self.sighting_manifest_id
    }

    pub(crate) const fn replacement_activation_id(&self) -> MigrationDigestV1 {
        self.replacement_activation_id
    }

    pub(crate) const fn rollback_rehearsal_id(&self) -> MigrationDigestV1 {
        self.rollback_rehearsal_id
    }

    pub(crate) const fn deletion_plan_id(&self) -> MigrationDigestV1 {
        self.deletion_plan_id
    }

    pub(crate) const fn physical_pruning_reader_zero_id(&self) -> MigrationDigestV1 {
        self.physical_pruning_reader_zero_id
    }

    pub(crate) const fn physical_pruning_hold_zero_id(&self) -> MigrationDigestV1 {
        self.physical_pruning_hold_zero_id
    }
}

pub(crate) struct Stage12ConsumerReaderHoldClosureV3 {
    identity: MigrationDigestV1,
    release_id: MigrationDigestV1,
    source_case_manifest_id: MigrationDigestV1,
    sighting_manifest_id: MigrationDigestV1,
    classification_manifest_id: MigrationDigestV1,
    overlap_manifest_id: MigrationDigestV1,
    loss_manifest_id: MigrationDigestV1,
    quarantine_manifest_id: MigrationDigestV1,
    foundation_closure_id: MigrationDigestV1,
    rollback_assessment_id: MigrationDigestV1,
    legacy_quarantine_epoch_id: MigrationDigestV1,
    replacement_activation_id: MigrationDigestV1,
    rollback_rehearsal_id: MigrationDigestV1,
    deletion_plan_id: MigrationDigestV1,
    physical_pruning_reader_zero_id: MigrationDigestV1,
    physical_pruning_hold_zero_id: MigrationDigestV1,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl Stage12ConsumerReaderHoldClosureV3 {
    #[expect(
        clippy::too_many_arguments,
        reason = "the V3 Stage 12 closure binds every V4 owner, migration, and consumer receipt"
    )]
    pub(in crate::domain::installation) fn seal(
        source_cases: &LegacySourceCaseManifestV3,
        sightings: &Stage12SightingManifestV2,
        classifications: &MigrationClassificationManifestV3,
        overlaps: &DeclaredOverlapManifestV2,
        losses: &UnavailablePreexistingLossManifestV4,
        quarantine: &SealedQuarantineManifestV3,
        foundation: &FoundationLegacyQuarantineClosureV2,
        rollback_assessment: &LegacyRollbackAssessmentV4,
        epoch: &LegacyQuarantineEpochV4,
        activation: &Stage12ReplacementActivationV3,
        rollback_rehearsal: &Stage12RollbackRehearsalV3,
        deletion_plan: &InstallationLegacyDeletionPlanV3,
        pre_currentness: ConsumerClosureReceiptV1<'_, '_, PreCurrentnessConsumerStageV1>,
        protected_retention: ConsumerClosureReceiptV1<'_, '_, ProtectedRetentionConsumerStageV1>,
        physical_pruning: ConsumerClosureReceiptV1<'_, '_, PhysicalPruningConsumerStageV1>,
    ) -> Result<Self, AgentResourceCutoverErrorV1> {
        let release_id = epoch.release_id();
        let foundation_closure_id = migration_digest(foundation.identity())?;
        let rollback_assessment_id = rollback_assessment.identity();
        if epoch.source_case_manifest_id() != source_cases.identity()
            || epoch.sighting_manifest_id() != sightings.identity()
            || epoch.classification_manifest_id() != classifications.identity()
            || epoch.overlap_manifest_id() != overlaps.identity()
            || epoch.loss_manifest_id() != losses.identity()
            || epoch.quarantine_manifest_id() != quarantine.identity()
            || epoch.foundation_closure_id() != foundation_closure_id
            || epoch.rollback_plan_id() != rollback_assessment_id
            || activation.release_id() != release_id
            || activation.legacy_quarantine_epoch_id() != epoch.identity()
            || activation.sighting_manifest_id() != sightings.identity()
            || activation.foundation_closure_id() != foundation_closure_id
            || activation.rollback_assessment_id() != rollback_assessment_id
            || rollback_rehearsal.release_id() != release_id
            || rollback_rehearsal.legacy_quarantine_epoch_id() != epoch.identity()
            || rollback_rehearsal.foundation_closure_id() != foundation_closure_id
            || rollback_rehearsal.rollback_assessment_id() != rollback_assessment_id
            || deletion_plan.release_id() != release_id
            || deletion_plan.legacy_quarantine_epoch_id() != epoch.identity()
            || deletion_plan.foundation_closure_id() != foundation_closure_id
            || deletion_plan.rollback_assessment_id() != rollback_assessment_id
            || deletion_plan.rollback_rehearsal_id() != rollback_rehearsal.identity()
        {
            return Err(AgentResourceCutoverErrorV1::Stage12BindingMismatch);
        }
        let pre_currentness_id = migration_digest(pre_currentness.finality_commitment())?;
        let protected_retention_id = migration_digest(protected_retention.finality_commitment())?;
        let physical_pruning_id = migration_digest(physical_pruning.finality_commitment())?;
        let physical_pruning_reader_zero_id = migration_commitment(
            b"maestro.vnext.physical-pruning-reader-zero.v3",
            &[
                *physical_pruning_id.as_bytes(),
                *foundation_closure_id.as_bytes(),
                *rollback_assessment_id.as_bytes(),
                *epoch.identity().as_bytes(),
            ],
        )?;
        let physical_pruning_hold_zero_id = migration_commitment(
            b"maestro.vnext.physical-pruning-hold-zero.v3",
            &[
                *physical_pruning_id.as_bytes(),
                *foundation_closure_id.as_bytes(),
                *rollback_assessment_id.as_bytes(),
                *epoch.identity().as_bytes(),
            ],
        )?;
        let identity = migration_commitment(
            b"maestro.vnext.stage12-consumer-reader-hold-closure.v3",
            &[
                *release_id.as_bytes(),
                *source_cases.identity().as_bytes(),
                *sightings.identity().as_bytes(),
                *classifications.identity().as_bytes(),
                *overlaps.identity().as_bytes(),
                *losses.identity().as_bytes(),
                *quarantine.identity().as_bytes(),
                *foundation_closure_id.as_bytes(),
                *rollback_assessment_id.as_bytes(),
                *epoch.identity().as_bytes(),
                *activation.identity().as_bytes(),
                *rollback_rehearsal.identity().as_bytes(),
                *deletion_plan.identity().as_bytes(),
                *pre_currentness_id.as_bytes(),
                *protected_retention_id.as_bytes(),
                *physical_pruning_reader_zero_id.as_bytes(),
                *physical_pruning_hold_zero_id.as_bytes(),
            ],
        )?;
        Ok(Self {
            identity,
            release_id,
            source_case_manifest_id: source_cases.identity(),
            sighting_manifest_id: sightings.identity(),
            classification_manifest_id: classifications.identity(),
            overlap_manifest_id: overlaps.identity(),
            loss_manifest_id: losses.identity(),
            quarantine_manifest_id: quarantine.identity(),
            foundation_closure_id,
            rollback_assessment_id,
            legacy_quarantine_epoch_id: epoch.identity(),
            replacement_activation_id: activation.identity(),
            rollback_rehearsal_id: rollback_rehearsal.identity(),
            deletion_plan_id: deletion_plan.identity(),
            physical_pruning_reader_zero_id,
            physical_pruning_hold_zero_id,
            _not_send_or_sync: PhantomData,
        })
    }

    pub(crate) const fn identity(&self) -> MigrationDigestV1 {
        self.identity
    }

    pub(crate) const fn release_id(&self) -> MigrationDigestV1 {
        self.release_id
    }

    pub(crate) const fn source_case_manifest_id(&self) -> MigrationDigestV1 {
        self.source_case_manifest_id
    }

    pub(crate) const fn sighting_manifest_id(&self) -> MigrationDigestV1 {
        self.sighting_manifest_id
    }

    pub(crate) const fn classification_manifest_id(&self) -> MigrationDigestV1 {
        self.classification_manifest_id
    }

    pub(crate) const fn overlap_manifest_id(&self) -> MigrationDigestV1 {
        self.overlap_manifest_id
    }

    pub(crate) const fn loss_manifest_id(&self) -> MigrationDigestV1 {
        self.loss_manifest_id
    }

    pub(crate) const fn quarantine_manifest_id(&self) -> MigrationDigestV1 {
        self.quarantine_manifest_id
    }

    pub(crate) const fn foundation_closure_id(&self) -> MigrationDigestV1 {
        self.foundation_closure_id
    }

    pub(crate) const fn rollback_assessment_id(&self) -> MigrationDigestV1 {
        self.rollback_assessment_id
    }

    pub(crate) const fn legacy_quarantine_epoch_id(&self) -> MigrationDigestV1 {
        self.legacy_quarantine_epoch_id
    }

    pub(crate) const fn replacement_activation_id(&self) -> MigrationDigestV1 {
        self.replacement_activation_id
    }

    pub(crate) const fn rollback_rehearsal_id(&self) -> MigrationDigestV1 {
        self.rollback_rehearsal_id
    }

    pub(crate) const fn deletion_plan_id(&self) -> MigrationDigestV1 {
        self.deletion_plan_id
    }

    pub(crate) const fn physical_pruning_reader_zero_id(&self) -> MigrationDigestV1 {
        self.physical_pruning_reader_zero_id
    }

    pub(crate) const fn physical_pruning_hold_zero_id(&self) -> MigrationDigestV1 {
        self.physical_pruning_hold_zero_id
    }
}

pub(in crate::domain) trait InstallationPhysicalPruningEffectPortV2 {
    fn compare_expected_old_and_prune(
        &mut self,
        deletion_plan_id: MigrationDigestV1,
        expected_old_state_id: MigrationDigestV1,
    ) -> Result<MigrationDigestV1, AgentResourceCutoverErrorV1>;
}

pub(in crate::domain) struct InstallationPhysicalPruningContinuationV2 {
    deletion_plan_id: MigrationDigestV1,
    expected_old_state_id: MigrationDigestV1,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl InstallationPhysicalPruningContinuationV2 {
    pub(in crate::domain::installation) fn prepare_from_owner(
        closure: &Stage12ConsumerReaderHoldClosureV2,
        expected_old_state_id: MigrationDigestV1,
    ) -> Self {
        Self {
            deletion_plan_id: closure.deletion_plan_id(),
            expected_old_state_id,
            _not_send_or_sync: PhantomData,
        }
    }
}

pub(in crate::domain) struct CommittedLegacyPhysicalPruningV2 {
    identity: MigrationDigestV1,
    deletion_plan_id: MigrationDigestV1,
    effect_receipt_id: MigrationDigestV1,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl CommittedLegacyPhysicalPruningV2 {
    pub(in crate::domain) const fn identity(&self) -> MigrationDigestV1 {
        self.identity
    }

    pub(in crate::domain) const fn deletion_plan_id(&self) -> MigrationDigestV1 {
        self.deletion_plan_id
    }

    pub(in crate::domain) const fn effect_receipt_id(&self) -> MigrationDigestV1 {
        self.effect_receipt_id
    }
}

pub(in crate::domain) trait InstallationPhysicalPruningEffectPortV3 {
    fn compare_expected_old_and_prune(
        &mut self,
        deletion_plan_id: MigrationDigestV1,
        expected_old_state_id: MigrationDigestV1,
    ) -> Result<MigrationDigestV1, AgentResourceCutoverErrorV1>;
}

pub(in crate::domain) struct InstallationPhysicalPruningContinuationV3 {
    deletion_plan_id: MigrationDigestV1,
    expected_old_state_id: MigrationDigestV1,
    foundation_closure_id: MigrationDigestV1,
    rollback_assessment_id: MigrationDigestV1,
    legacy_quarantine_epoch_id: MigrationDigestV1,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl InstallationPhysicalPruningContinuationV3 {
    pub(in crate::domain::installation) fn prepare_from_owner(
        closure: &Stage12ConsumerReaderHoldClosureV3,
        expected_old_state_id: MigrationDigestV1,
    ) -> Self {
        Self {
            deletion_plan_id: closure.deletion_plan_id(),
            expected_old_state_id,
            foundation_closure_id: closure.foundation_closure_id(),
            rollback_assessment_id: closure.rollback_assessment_id(),
            legacy_quarantine_epoch_id: closure.legacy_quarantine_epoch_id(),
            _not_send_or_sync: PhantomData,
        }
    }
}

pub(in crate::domain) struct CommittedLegacyPhysicalPruningV3 {
    identity: MigrationDigestV1,
    deletion_plan_id: MigrationDigestV1,
    effect_receipt_id: MigrationDigestV1,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl CommittedLegacyPhysicalPruningV3 {
    pub(in crate::domain) const fn identity(&self) -> MigrationDigestV1 {
        self.identity
    }

    pub(in crate::domain) const fn deletion_plan_id(&self) -> MigrationDigestV1 {
        self.deletion_plan_id
    }

    pub(in crate::domain) const fn effect_receipt_id(&self) -> MigrationDigestV1 {
        self.effect_receipt_id
    }
}

pub(in crate::domain) fn execute_stage12_product_pruning(
    guard: LegacyRemovalGuardV3<'_>,
    closure: Stage12ConsumerReaderHoldClosureV3,
    continuation: InstallationPhysicalPruningContinuationV3,
    effects: &mut dyn InstallationPhysicalPruningEffectPortV3,
) -> Result<CommittedLegacyPhysicalPruningV3, AgentResourceCutoverErrorV1> {
    if continuation.deletion_plan_id != closure.deletion_plan_id()
        || continuation.foundation_closure_id != closure.foundation_closure_id()
        || continuation.rollback_assessment_id != closure.rollback_assessment_id()
        || continuation.legacy_quarantine_epoch_id != closure.legacy_quarantine_epoch_id()
        || continuation.expected_old_state_id.as_bytes() == &[0; 32]
    {
        return Err(AgentResourceCutoverErrorV1::Stage12BindingMismatch);
    }
    let effect_receipt_id = guard
        .consume_with_linearization(&closure, || {
            effects.compare_expected_old_and_prune(
                continuation.deletion_plan_id,
                continuation.expected_old_state_id,
            )
        })
        .map_err(|_| AgentResourceCutoverErrorV1::Stage12BindingMismatch)??;
    let identity = migration_commitment(
        b"maestro.vnext.committed-legacy-physical-pruning.v3",
        &[
            *closure.identity().as_bytes(),
            *closure.loss_manifest_id().as_bytes(),
            *closure.foundation_closure_id().as_bytes(),
            *closure.rollback_assessment_id().as_bytes(),
            *closure.legacy_quarantine_epoch_id().as_bytes(),
            *continuation.deletion_plan_id.as_bytes(),
            *continuation.expected_old_state_id.as_bytes(),
            *effect_receipt_id.as_bytes(),
        ],
    )?;
    Ok(CommittedLegacyPhysicalPruningV3 {
        identity,
        deletion_plan_id: continuation.deletion_plan_id,
        effect_receipt_id,
        _not_send_or_sync: PhantomData,
    })
}

fn migration_release_id(
    release_id: ReleaseIdV1,
) -> Result<MigrationDigestV1, AgentResourceCutoverErrorV1> {
    migration_digest(*release_id.as_bytes())
}

fn migration_digest(digest: [u8; 32]) -> Result<MigrationDigestV1, AgentResourceCutoverErrorV1> {
    MigrationDigestV1::from_digest(digest)
        .map_err(|_| AgentResourceCutoverErrorV1::Stage12BindingMismatch)
}

fn migration_commitment(
    domain: &'static [u8],
    parts: &[[u8; 32]],
) -> Result<MigrationDigestV1, AgentResourceCutoverErrorV1> {
    migration_digest(commitment(domain, parts))
}

fn owner_derived_agent_resource_journal(
    release_id: ReleaseIdV1,
    consumer_closure: [u8; 32],
    resource_inventory_closure: [u8; 32],
    legacy_ledger_closure: [u8; 32],
    owner_facts: &AgentResourceReleaseOwnerFactsV1,
) -> [AgentResourceJournalBindingV1; 7] {
    std::array::from_fn(|index| {
        let kind = AgentResourceTargetKindV1::ALL[index];
        let target_tag = index as u64 + 1;
        let target_identity = owner_facts.targets[index].target_identity;
        let effect_kind = if kind == AgentResourceTargetKindV1::LegacyDiscoveryDeactivation {
            TargetEffectKindV1::RemoveOwnedTarget
        } else {
            TargetEffectKindV1::RewriteOwnedTarget
        };
        let candidate =
            (kind != AgentResourceTargetKindV1::LegacyDiscoveryDeactivation).then(|| {
                CommitmentV1::from_bytes(commitment(
                    b"maestro.vnext.agent-resource-target-candidate.v1",
                    &[
                        u64_commitment(kind as u64 + 1),
                        *release_id.as_bytes(),
                        resource_inventory_closure,
                        legacy_ledger_closure,
                        consumer_closure,
                    ],
                ))
            });
        AgentResourceJournalBindingV1 {
            kind,
            target_tag,
            target_identity,
            expected_preimage: owner_facts.targets[index].expected_preimage,
            candidate,
            effect_kind,
        }
    })
}

fn build_agent_resource_plan(
    release_id: ReleaseIdV1,
    plan: &CutoverPlanOwnerFactsV1<7>,
    journal: &[AgentResourceJournalBindingV1; 7],
) -> Result<DistributionPlanV1, AgentResourceCutoverErrorV1> {
    let targets = journal
        .iter()
        .enumerate()
        .map(|(index, binding)| DistributionPlanTargetV1 {
            target_tag: binding.target_tag,
            target_identity_ref: plan.target_identity_refs[index].clone(),
            target_identity: binding.target_identity,
            custody: plan.target_custodies[index].clone(),
            expected_preimage_commitment: binding.expected_preimage,
            candidate_commitment: binding.candidate,
            effect_kind: binding.effect_kind,
            outside_prefix_commitment: None,
            outside_suffix_commitment: None,
        })
        .collect();
    DistributionPlanV1::new(
        plan.domain.clone(),
        DistributionMutationKindV1::Migrate,
        plan.request_id,
        plan.request_or_ceremony_ref.clone(),
        plan.plan_ref.clone(),
        plan.idempotency_key_ref.clone(),
        Some(release_id),
        plan.prior_commit_ref.clone(),
        plan.prior_receipt_ref.clone(),
        None,
        targets,
    )
    .map_err(|_| AgentResourceCutoverErrorV1::InvalidOwnerFacts)
}

fn agent_resource_journal_closure(
    resources: &[AgentResourceJournalRowV1; 31],
    dispositions: &[LegacyDispositionJournalRowV1; 35],
    targets: &[AgentResourceJournalBindingV1; 7],
) -> [u8; 32] {
    let resource_parts = resources.iter().flat_map(|row| {
        [
            u64_commitment(row.ordinal),
            string_commitment(row.logical_path),
            string_commitment(row.embedded_source_path),
            row.content_sha256,
        ]
    });
    let disposition_parts = dispositions.iter().flat_map(|row| {
        [
            u64_commitment(row.ordinal),
            string_commitment(row.source_path),
            u64_commitment(match row.disposition {
                LegacySkillDispositionV1::Rewrite => 1,
                LegacySkillDispositionV1::Replace => 2,
                LegacySkillDispositionV1::MigrationOnly => 3,
            }),
            row.active_destination.map_or([0; 32], string_commitment),
        ]
    });
    let target_parts = targets.iter().flat_map(|binding| {
        [
            u64_commitment(binding.kind as u64 + 1),
            u64_commitment(binding.target_tag),
            *binding.target_identity.as_bytes(),
            *binding.expected_preimage.as_bytes(),
            binding.candidate.map_or([0; 32], |value| *value.as_bytes()),
            u64_commitment(binding.effect_kind.numeric_tag()),
        ]
    });
    let parts = resource_parts
        .chain(disposition_parts)
        .chain(target_parts)
        .collect::<Vec<_>>();
    commitment(b"maestro.vnext.agent-resource-journal.v1", &parts)
}

fn u64_commitment(value: u64) -> [u8; 32] {
    Sha256::digest(value.to_be_bytes()).into()
}

fn string_commitment(value: &str) -> [u8; 32] {
    Sha256::digest(value.as_bytes()).into()
}

fn commitment(domain: &[u8], parts: &[[u8; 32]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update((domain.len() as u64).to_be_bytes());
    hasher.update(domain);
    for part in parts {
        hasher.update(part);
    }
    hasher.finalize().into()
}

#[derive(Debug, Error, Eq, PartialEq)]
pub(crate) enum AgentResourceCutoverErrorV1 {
    #[error("the canonical Agent Resource inventory is invalid")]
    InvalidCanonicalInventory,
    #[error("the Agent Resource release admission is invalid")]
    InvalidAdmission,
    #[error("the Installation owner facts are incomplete or invalid")]
    InvalidOwnerFacts,
    #[error("the Distribution plan does not match the exact domain-local cutover")]
    PlanMismatch,
    #[error("the Agent Resource release is not committed and coherently reconnected")]
    ReconnectNotCurrent,
    #[error("the Stage 12 replacement, rollback, deletion, or consumer closure binding mismatches")]
    Stage12BindingMismatch,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::authority::ActionRequestIdV1;
    use crate::domain::distribution::runtime::{
        CustodyBasisV1, DistributionDomainRefV1, DistributionRuntimeObjectKindV1,
        DistributionScopedObjectRefV1, ManagedBlockBoundaryV1,
    };
    use crate::domain::identity::StoreObjectIdV1;

    fn commitment_value(value: u8) -> CommitmentV1 {
        CommitmentV1::from_bytes([value; 32])
    }

    fn object_id(value: u8) -> StoreObjectIdV1 {
        StoreObjectIdV1::parse(&format!("sha256:{}", format!("{value:02x}").repeat(32))).unwrap()
    }

    fn domain(kind: DistributionDomainKindV1) -> DistributionDomainRefV1 {
        DistributionDomainRefV1::new(
            kind,
            commitment_value(1),
            commitment_value(2),
            commitment_value(3),
        )
        .unwrap()
    }

    fn scoped(
        domain: &DistributionDomainRefV1,
        kind: DistributionRuntimeObjectKindV1,
        value: u8,
    ) -> DistributionScopedObjectRefV1 {
        DistributionScopedObjectRefV1::new(domain.clone(), kind, object_id(value)).unwrap()
    }

    fn custody(
        domain: &DistributionDomainRefV1,
        target_identity: CommitmentV1,
        value: u8,
        shared_block: bool,
    ) -> CustodyAssessmentV1 {
        CustodyAssessmentV1::assess(&CustodyBasisV1 {
            domain: domain.clone(),
            target_identity,
            alias_closure_id: commitment_value(value.wrapping_add(1)),
            receipt_ref: Some(scoped(
                domain,
                DistributionRuntimeObjectKindV1::DistributionReceipt,
                value.wrapping_add(2),
            )),
            claim_ref: Some(scoped(
                domain,
                DistributionRuntimeObjectKindV1::InstalledResourceClaim,
                value.wrapping_add(3),
            )),
            claimed_target_identity: Some(target_identity),
            resource_id: Some(commitment_value(value.wrapping_add(4))),
            bundle_id: Some(commitment_value(value.wrapping_add(5))),
            release_id: Some(commitment_value(value.wrapping_add(6))),
            claimed_content_sha256: Some(commitment_value(value.wrapping_add(7))),
            observed_content_sha256: Some(commitment_value(value.wrapping_add(7))),
            managed_block: shared_block.then(|| ManagedBlockBoundaryV1 {
                start_marker: b"<!-- maestro:start -->".to_vec(),
                end_marker: b"<!-- maestro:end -->".to_vec(),
                block_sha256: commitment_value(value.wrapping_add(8)),
                outside_prefix_sha256: commitment_value(value.wrapping_add(9)),
                outside_suffix_sha256: commitment_value(value.wrapping_add(10)),
            }),
            foreign_owner_observed: false,
            external_manager_observed: false,
            alias_ambiguous: false,
            unsafe_path_state: false,
        })
        .unwrap()
    }

    fn plan_facts<const TARGETS: usize>(
        kind: DistributionDomainKindV1,
        identities: [CommitmentV1; TARGETS],
        shared_target: Option<usize>,
    ) -> CutoverPlanOwnerFactsV1<TARGETS> {
        let domain = domain(kind);
        let target_identity_refs = std::array::from_fn(|index| {
            scoped(
                &domain,
                DistributionRuntimeObjectKindV1::CanonicalTargetIdentity,
                30 + index as u8,
            )
        });
        let target_custodies = std::array::from_fn(|index| {
            custody(
                &domain,
                identities[index],
                100 + index as u8 * 12,
                shared_target == Some(index),
            )
        });
        CutoverPlanOwnerFactsV1::new(
            domain.clone(),
            ActionRequestIdV1::derive("stage9-resource-cutover").unwrap(),
            scoped(
                &domain,
                DistributionRuntimeObjectKindV1::ActionRequestOrCeremony,
                10,
            ),
            scoped(
                &domain,
                DistributionRuntimeObjectKindV1::DistributionPlan,
                11,
            ),
            scoped(&domain, DistributionRuntimeObjectKindV1::IdempotencyKey, 12),
            None,
            None,
            target_identity_refs,
            target_custodies,
        )
    }

    fn agent_owner_facts(seed: u8) -> AgentResourceReleaseOwnerFactsV1 {
        let targets = std::array::from_fn(|index| AgentResourceTargetOwnerFactsV1 {
            target_identity: commitment_value(seed + index as u8),
            expected_preimage: commitment_value(seed + 20 + index as u8),
        });
        AgentResourceReleaseOwnerFactsV1::new(
            plan_facts(
                DistributionDomainKindV1::InstallationDomain,
                targets.map(|facts| facts.target_identity),
                None,
            ),
            targets,
        )
        .unwrap()
    }

    #[test]
    fn physical_pruning_continuation_is_prepared_from_the_owner_closure() {
        let deletion_plan_id = MigrationDigestV1::from_digest([71; 32]).unwrap();
        let expected_old_state_id = MigrationDigestV1::from_digest([72; 32]).unwrap();
        let closure = Stage12ConsumerReaderHoldClosureV2 {
            identity: MigrationDigestV1::from_digest([61; 32]).unwrap(),
            release_id: MigrationDigestV1::from_digest([62; 32]).unwrap(),
            legacy_quarantine_epoch_id: MigrationDigestV1::from_digest([63; 32]).unwrap(),
            sighting_manifest_id: MigrationDigestV1::from_digest([64; 32]).unwrap(),
            replacement_activation_id: MigrationDigestV1::from_digest([65; 32]).unwrap(),
            rollback_rehearsal_id: MigrationDigestV1::from_digest([66; 32]).unwrap(),
            deletion_plan_id,
            physical_pruning_reader_zero_id: MigrationDigestV1::from_digest([67; 32]).unwrap(),
            physical_pruning_hold_zero_id: MigrationDigestV1::from_digest([68; 32]).unwrap(),
            _not_send_or_sync: PhantomData,
        };

        let continuation = InstallationPhysicalPruningContinuationV2::prepare_from_owner(
            &closure,
            expected_old_state_id,
        );

        assert_eq!(continuation.deletion_plan_id, deletion_plan_id);
        assert_eq!(continuation.expected_old_state_id, expected_old_state_id);
    }

    #[test]
    fn installation_owner_derives_exact_inventory_ledger_plan_and_refuses_tampering() {
        let inventory = CanonicalAgentResourceInventoryV1::load_embedded().unwrap();
        assert_eq!(inventory.resources().len(), 31);
        assert_eq!(inventory.legacy_ledger().len(), 35);

        let admission = AgentResourceReleaseAdmissionV1::new(
            commitment_value(90),
            AgentResourceReleaseConsumerSealV1::test_seal([91; 32]),
            agent_owner_facts(20),
        )
        .unwrap();

        assert_ne!(admission.resource_inventory_closure(), [0; 32]);
        assert_ne!(admission.legacy_ledger_closure(), [0; 32]);
        assert_ne!(admission.finality_closure(), [0; 32]);
        assert_eq!(admission.resources.len(), 31);
        assert_eq!(admission.dispositions.len(), 35);
        assert_eq!(
            admission
                .resources
                .map(|row| (row.logical_path, row.content_sha256)),
            inventory
                .resources()
                .map(|row| (row.logical_path, row.content_sha256))
        );
        assert_eq!(
            admission.dispositions.map(|row| (
                row.source_path,
                row.disposition,
                row.active_destination
            )),
            inventory.legacy_ledger().map(|row| (
                row.source_path,
                row.disposition,
                row.active_destination
            ))
        );
        assert_eq!(admission.plan().targets().len(), 7);
        assert_eq!(admission.plan().targets()[6].candidate_commitment, None);

        let candidate_tamper = AgentResourceReleaseAdmissionV1::new(
            commitment_value(90),
            AgentResourceReleaseConsumerSealV1::test_seal([92; 32]),
            agent_owner_facts(20),
        )
        .unwrap();
        assert_eq!(
            admission.validate_plan(candidate_tamper.plan()),
            Err(AgentResourceCutoverErrorV1::PlanMismatch)
        );

        let mut preimage_tamper = agent_owner_facts(20);
        preimage_tamper.targets[0].expected_preimage = commitment_value(99);
        let preimage_tamper = AgentResourceReleaseAdmissionV1::new(
            commitment_value(90),
            AgentResourceReleaseConsumerSealV1::test_seal([91; 32]),
            preimage_tamper,
        )
        .unwrap();
        assert_eq!(
            admission.validate_plan(preimage_tamper.plan()),
            Err(AgentResourceCutoverErrorV1::PlanMismatch)
        );

        let mut row_tamper = AgentResourceReleaseAdmissionV1::new(
            commitment_value(90),
            AgentResourceReleaseConsumerSealV1::test_seal([91; 32]),
            agent_owner_facts(20),
        )
        .unwrap();
        row_tamper.resources[0].content_sha256 = [0; 32];
        assert_eq!(
            row_tamper.validate_plan(row_tamper.plan()),
            Err(AgentResourceCutoverErrorV1::PlanMismatch)
        );

        let mut closure_tamper = AgentResourceReleaseAdmissionV1::new(
            commitment_value(90),
            AgentResourceReleaseConsumerSealV1::test_seal([91; 32]),
            agent_owner_facts(20),
        )
        .unwrap();
        closure_tamper.journal_closure = [0; 32];
        assert_eq!(
            closure_tamper.validate_plan(closure_tamper.plan()),
            Err(AgentResourceCutoverErrorV1::PlanMismatch)
        );
    }
}
