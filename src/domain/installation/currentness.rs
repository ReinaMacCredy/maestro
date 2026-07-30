use std::collections::{BTreeMap, BTreeSet};

use crate::domain::distribution::ReleaseIdV1;
use crate::domain::distribution::runtime::DistributionScopedObjectRefV1;

use super::{HostActivationEntryV1, HostAdmissionStateV1, UserAgentInstallationClosureV1};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedHostActivationV1 {
    pub host_tag: u64,
    pub host_adapter_id: crate::domain::distribution::CommitmentV1,
    pub skill_activation_claim_ref: Option<DistributionScopedObjectRefV1>,
    pub mcp_packet_descriptor_claim_ref: Option<DistributionScopedObjectRefV1>,
    pub mcp_cli_search_descriptor_claim_ref: Option<DistributionScopedObjectRefV1>,
    pub running_catalog_observation_ref: Option<DistributionScopedObjectRefV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedInstallationClosureV1 {
    pub release_id: ReleaseIdV1,
    pub binary_claim_ref: DistributionScopedObjectRefV1,
    pub claim_set_ref: DistributionScopedObjectRefV1,
    pub receipt_ref: DistributionScopedObjectRefV1,
    pub snapshot_catalog_ref: DistributionScopedObjectRefV1,
    pub recovery_root_set_ref: DistributionScopedObjectRefV1,
    pub verification_result_ref: DistributionScopedObjectRefV1,
    pub hosts: Vec<ObservedHostActivationV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DomainCurrentnessV1 {
    Coherent,
    StoreClosureDrift,
    HostLocalStale { host_tags: Vec<u64> },
}

impl DomainCurrentnessV1 {
    pub const fn grants_mutation_authority(&self) -> bool {
        false
    }

    pub const fn domain_head_remains_current(&self) -> bool {
        matches!(self, Self::Coherent | Self::HostLocalStale { .. })
    }
}

pub fn assess_user_agent_currentness(
    closure: &UserAgentInstallationClosureV1,
    observed: &ObservedInstallationClosureV1,
) -> DomainCurrentnessV1 {
    if closure.release_id != observed.release_id
        || closure.binary_claim_ref != observed.binary_claim_ref
        || closure.claim_set_ref != observed.claim_set_ref
        || closure.receipt_ref != observed.receipt_ref
        || closure.snapshot_catalog_ref != observed.snapshot_catalog_ref
        || closure.recovery_root_set_ref != observed.recovery_root_set_ref
        || closure.verification_result_ref != observed.verification_result_ref
    {
        return DomainCurrentnessV1::StoreClosureDrift;
    }
    let observed_by_tag = observed
        .hosts
        .iter()
        .map(|host| (host.host_tag, host))
        .collect::<BTreeMap<_, _>>();
    if observed_by_tag.len() != observed.hosts.len() {
        return DomainCurrentnessV1::StoreClosureDrift;
    }
    let expected_tags = closure
        .host_entries
        .iter()
        .map(|host| host.host_tag)
        .collect::<BTreeSet<_>>();
    if observed_by_tag
        .keys()
        .any(|tag| !expected_tags.contains(tag))
    {
        return DomainCurrentnessV1::StoreClosureDrift;
    }
    let mut stale = Vec::new();
    for expected in &closure.host_entries {
        match expected.admission_state {
            HostAdmissionStateV1::Absent => {
                if observed_by_tag.contains_key(&expected.host_tag) {
                    stale.push(expected.host_tag);
                }
            }
            HostAdmissionStateV1::RequiredBlocked => stale.push(expected.host_tag),
            HostAdmissionStateV1::Admitted => {
                if observed_by_tag
                    .get(&expected.host_tag)
                    .is_none_or(|observed| !host_matches(expected, observed))
                {
                    stale.push(expected.host_tag);
                }
            }
        }
    }
    if stale.is_empty() {
        DomainCurrentnessV1::Coherent
    } else {
        DomainCurrentnessV1::HostLocalStale { host_tags: stale }
    }
}

fn host_matches(expected: &HostActivationEntryV1, observed: &ObservedHostActivationV1) -> bool {
    expected.host_adapter_id == observed.host_adapter_id
        && expected.skill_activation_claim_ref == observed.skill_activation_claim_ref
        && expected.mcp_packet_descriptor_claim_ref == observed.mcp_packet_descriptor_claim_ref
        && expected.mcp_cli_search_descriptor_claim_ref
            == observed.mcp_cli_search_descriptor_claim_ref
        && expected.running_catalog_observation_ref == observed.running_catalog_observation_ref
}
