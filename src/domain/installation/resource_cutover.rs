use std::collections::BTreeSet;
use std::marker::PhantomData;
use std::rc::Rc;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::capability::literals::CanonicalAgentResourceInventoryV1;
use crate::domain::distribution::runtime::{
    DistributionDomainKindV1, DistributionMutationKindV1, DistributionPlanV1,
    DistributionTransactionPhaseV1, DistributionTransactionV1,
};
use crate::domain::distribution::{CommitmentV1, ReleaseIdV1};

use super::consumer_snapshot::AgentResourceReleaseConsumerSealV1;
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
pub(crate) struct AgentResourceJournalBindingV1 {
    pub kind: AgentResourceTargetKindV1,
    pub target_tag: u64,
    pub expected_preimage: CommitmentV1,
    pub candidate: Option<CommitmentV1>,
}

pub(crate) struct AgentResourceReleaseAdmissionV1 {
    release_id: ReleaseIdV1,
    resource_inventory_closure: [u8; 32],
    legacy_ledger_closure: [u8; 32],
    consumer_closure: [u8; 32],
    journal_closure: [u8; 32],
    journal: [AgentResourceJournalBindingV1; 7],
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl AgentResourceReleaseAdmissionV1 {
    pub(crate) fn new(
        release_id: ReleaseIdV1,
        consumer_seal: AgentResourceReleaseConsumerSealV1,
        journal: [AgentResourceJournalBindingV1; 7],
    ) -> Result<Self, AgentResourceCutoverErrorV1> {
        if release_id.as_bytes() == &[0; 32] {
            return Err(AgentResourceCutoverErrorV1::InvalidAdmission);
        }
        let inventory = CanonicalAgentResourceInventoryV1::load_embedded()
            .map_err(|_| AgentResourceCutoverErrorV1::InvalidCanonicalInventory)?;
        validate_agent_resource_journal(&journal)?;
        let journal_closure = agent_resource_journal_closure(&journal);
        Ok(Self {
            release_id,
            resource_inventory_closure: inventory.resource_closure(),
            legacy_ledger_closure: inventory.legacy_ledger_closure(),
            consumer_closure: consumer_seal.closure(),
            journal_closure,
            journal,
            _not_send_or_sync: PhantomData,
        })
    }

    pub(crate) fn validate_plan(
        &self,
        plan: &DistributionPlanV1,
    ) -> Result<(), AgentResourceCutoverErrorV1> {
        if plan.domain().kind() != DistributionDomainKindV1::InstallationDomain
            || plan.mutation_kind() != DistributionMutationKindV1::Migrate
            || plan.release_id() != Some(self.release_id)
            || plan.targets().len() != self.journal.len()
        {
            return Err(AgentResourceCutoverErrorV1::PlanMismatch);
        }
        for (target, binding) in plan.targets().iter().zip(&self.journal) {
            if target.target_tag != binding.target_tag
                || target.expected_preimage_commitment != binding.expected_preimage
                || target.candidate_commitment != binding.candidate
            {
                return Err(AgentResourceCutoverErrorV1::PlanMismatch);
            }
        }
        Ok(())
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
}

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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RepositoryBootstrapTargetKindV1 {
    MaestroBootstrapFile,
    AgentsManagedPointer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RepositoryBootstrapAuthorizationV1 {
    Apply,
    Force,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RepositoryBootstrapBindingV1 {
    pub kind: RepositoryBootstrapTargetKindV1,
    pub target_tag: u64,
    pub expected_preimage: CommitmentV1,
    pub candidate: CommitmentV1,
    pub shown_diff: CommitmentV1,
    pub backup: CommitmentV1,
    pub authorization: RepositoryBootstrapAuthorizationV1,
}

pub(crate) struct RepositoryBootstrapAdmissionV1 {
    installation_release_id: ReleaseIdV1,
    installation_result_closure: [u8; 32],
    reconnect_closure: [u8; 32],
    bootstrap_closure: [u8; 32],
    bindings: [RepositoryBootstrapBindingV1; 2],
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl RepositoryBootstrapAdmissionV1 {
    pub(crate) fn after_agent_resource_release(
        release: &CommittedAgentResourceReleaseV1,
        bindings: [RepositoryBootstrapBindingV1; 2],
    ) -> Result<Self, AgentResourceCutoverErrorV1> {
        let kinds = bindings.map(|binding| binding.kind);
        if kinds
            != [
                RepositoryBootstrapTargetKindV1::MaestroBootstrapFile,
                RepositoryBootstrapTargetKindV1::AgentsManagedPointer,
            ]
            || bindings.iter().any(|binding| {
                binding.target_tag == 0
                    || [
                        binding.expected_preimage,
                        binding.candidate,
                        binding.shown_diff,
                        binding.backup,
                    ]
                    .iter()
                    .any(|value| value.as_bytes() == &[0; 32])
            })
        {
            return Err(AgentResourceCutoverErrorV1::InvalidRepositoryBootstrap);
        }
        let bootstrap_closure = commitment(
            b"maestro.vnext.repository-bootstrap.v1",
            &bindings
                .iter()
                .flat_map(repository_binding_parts)
                .collect::<Vec<_>>(),
        );
        Ok(Self {
            installation_release_id: release.release_id,
            installation_result_closure: release.installation_result_closure,
            reconnect_closure: release.reconnect_closure,
            bootstrap_closure,
            bindings,
            _not_send_or_sync: PhantomData,
        })
    }

    pub(crate) fn validate_plan(
        &self,
        plan: &DistributionPlanV1,
    ) -> Result<(), AgentResourceCutoverErrorV1> {
        if plan.domain().kind() != DistributionDomainKindV1::RepositoryDomain
            || plan.mutation_kind() != DistributionMutationKindV1::Migrate
            || plan.release_id().is_some()
            || plan.targets().len() != self.bindings.len()
        {
            return Err(AgentResourceCutoverErrorV1::PlanMismatch);
        }
        for (target, binding) in plan.targets().iter().zip(&self.bindings) {
            if target.target_tag != binding.target_tag
                || target.expected_preimage_commitment != binding.expected_preimage
                || target.candidate_commitment != Some(binding.candidate)
            {
                return Err(AgentResourceCutoverErrorV1::PlanMismatch);
            }
        }
        Ok(())
    }

    pub(crate) const fn installation_release_id(&self) -> ReleaseIdV1 {
        self.installation_release_id
    }

    pub(crate) const fn bootstrap_closure(&self) -> [u8; 32] {
        self.bootstrap_closure
    }
}

fn validate_agent_resource_journal(
    journal: &[AgentResourceJournalBindingV1; 7],
) -> Result<(), AgentResourceCutoverErrorV1> {
    let kinds = journal
        .iter()
        .map(|binding| binding.kind)
        .collect::<BTreeSet<_>>();
    let tags = journal
        .iter()
        .map(|binding| binding.target_tag)
        .collect::<BTreeSet<_>>();
    if journal.map(|binding| binding.kind) != AgentResourceTargetKindV1::ALL
        || kinds != AgentResourceTargetKindV1::ALL.into_iter().collect()
        || tags.len() != journal.len()
        || journal.iter().any(|binding| {
            binding.target_tag == 0
                || binding.expected_preimage.as_bytes() == &[0; 32]
                || (binding.kind == AgentResourceTargetKindV1::LegacyDiscoveryDeactivation)
                    != binding.candidate.is_none()
        })
    {
        return Err(AgentResourceCutoverErrorV1::InvalidJournal);
    }
    Ok(())
}

fn agent_resource_journal_closure(journal: &[AgentResourceJournalBindingV1; 7]) -> [u8; 32] {
    let parts = journal
        .iter()
        .flat_map(|binding| {
            [
                u64_commitment(binding.kind as u64 + 1),
                u64_commitment(binding.target_tag),
                *binding.expected_preimage.as_bytes(),
                binding.candidate.map_or([0; 32], |value| *value.as_bytes()),
            ]
        })
        .collect::<Vec<_>>();
    commitment(b"maestro.vnext.agent-resource-journal.v1", &parts)
}

fn repository_binding_parts(binding: &RepositoryBootstrapBindingV1) -> [[u8; 32]; 7] {
    [
        u64_commitment(binding.kind as u64 + 1),
        u64_commitment(binding.target_tag),
        *binding.expected_preimage.as_bytes(),
        *binding.candidate.as_bytes(),
        *binding.shown_diff.as_bytes(),
        *binding.backup.as_bytes(),
        u64_commitment(binding.authorization as u64 + 1),
    ]
}

fn u64_commitment(value: u64) -> [u8; 32] {
    Sha256::digest(value.to_be_bytes()).into()
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
    #[error("the Agent Resource journal is incomplete or inconsistent")]
    InvalidJournal,
    #[error("the Distribution plan does not match the exact domain-local cutover")]
    PlanMismatch,
    #[error("the Agent Resource release is not committed and coherently reconnected")]
    ReconnectNotCurrent,
    #[error("the Repository bootstrap lacks exact targets, diff, backup, or authorization")]
    InvalidRepositoryBootstrap,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commitment_value(value: u8) -> CommitmentV1 {
        CommitmentV1::from_bytes([value; 32])
    }

    fn journal() -> [AgentResourceJournalBindingV1; 7] {
        AgentResourceTargetKindV1::ALL.map(|kind| {
            let target_tag = kind as u64 + 1;
            AgentResourceJournalBindingV1 {
                kind,
                target_tag,
                expected_preimage: commitment_value(target_tag as u8),
                candidate: (kind != AgentResourceTargetKindV1::LegacyDiscoveryDeactivation)
                    .then(|| commitment_value(target_tag as u8 + 10)),
            }
        })
    }

    #[test]
    fn installation_release_admission_binds_exact_inventory_ledger_and_journal() {
        let admission = AgentResourceReleaseAdmissionV1::new(
            commitment_value(90),
            AgentResourceReleaseConsumerSealV1::test_seal([91; 32]),
            journal(),
        )
        .unwrap();

        assert_ne!(admission.resource_inventory_closure(), [0; 32]);
        assert_ne!(admission.legacy_ledger_closure(), [0; 32]);
        assert_ne!(admission.finality_closure(), [0; 32]);

        let mut reordered = journal();
        reordered.swap(0, 1);
        assert_eq!(
            AgentResourceReleaseAdmissionV1::new(
                commitment_value(90),
                AgentResourceReleaseConsumerSealV1::test_seal([91; 32]),
                reordered,
            )
            .err(),
            Some(AgentResourceCutoverErrorV1::InvalidJournal)
        );
    }

    #[test]
    fn repository_bootstrap_requires_prior_reconnected_installation_result() {
        let committed = CommittedAgentResourceReleaseV1 {
            release_id: commitment_value(80),
            installation_result_closure: [81; 32],
            reconnect_closure: [82; 32],
            _not_send_or_sync: PhantomData,
        };
        let bindings = [
            RepositoryBootstrapBindingV1 {
                kind: RepositoryBootstrapTargetKindV1::MaestroBootstrapFile,
                target_tag: 1,
                expected_preimage: commitment_value(1),
                candidate: commitment_value(2),
                shown_diff: commitment_value(3),
                backup: commitment_value(4),
                authorization: RepositoryBootstrapAuthorizationV1::Apply,
            },
            RepositoryBootstrapBindingV1 {
                kind: RepositoryBootstrapTargetKindV1::AgentsManagedPointer,
                target_tag: 2,
                expected_preimage: commitment_value(5),
                candidate: commitment_value(6),
                shown_diff: commitment_value(7),
                backup: commitment_value(8),
                authorization: RepositoryBootstrapAuthorizationV1::Force,
            },
        ];
        let admission =
            RepositoryBootstrapAdmissionV1::after_agent_resource_release(&committed, bindings)
                .unwrap();

        assert_eq!(admission.installation_release_id(), committed.release_id());
        assert_ne!(admission.bootstrap_closure(), [0; 32]);

        let mut missing_diff = bindings;
        missing_diff[1].shown_diff = CommitmentV1::from_bytes([0; 32]);
        assert_eq!(
            RepositoryBootstrapAdmissionV1::after_agent_resource_release(&committed, missing_diff)
                .err(),
            Some(AgentResourceCutoverErrorV1::InvalidRepositoryBootstrap)
        );
    }
}
