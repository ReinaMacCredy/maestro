use std::marker::PhantomData;
use std::rc::Rc;

use sha2::{Digest, Sha256};
use thiserror::Error;

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
