use std::collections::BTreeSet;

use thiserror::Error;

use crate::domain::distribution::runtime::{
    DistributionDomainKindV1, DistributionDomainRefV1, DistributionModelErrorV1,
    DistributionRuntimeObjectKindV1, DistributionScopedObjectRefV1,
};
use crate::domain::distribution::{CommitmentV1, ReleaseIdV1};
use crate::domain::identity::StoreObjectIdV1;
use crate::domain::persistence::{StoreObjectError, StoreObjectV1};
use crate::foundation::core::deterministic_cbor::{CborError, CborValue};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostAdmissionStateV1 {
    Absent,
    Admitted,
    RequiredBlocked,
}

impl HostAdmissionStateV1 {
    pub const fn numeric_tag(self) -> u64 {
        1 + self as u64
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostActivationEntryV1 {
    pub host_tag: u64,
    pub domain: DistributionDomainRefV1,
    pub host_adapter_id: CommitmentV1,
    pub admission_state: HostAdmissionStateV1,
    pub skill_activation_claim_ref: Option<DistributionScopedObjectRefV1>,
    pub mcp_packet_descriptor_claim_ref: Option<DistributionScopedObjectRefV1>,
    pub mcp_cli_search_descriptor_claim_ref: Option<DistributionScopedObjectRefV1>,
    pub running_catalog_observation_ref: Option<DistributionScopedObjectRefV1>,
}

impl HostActivationEntryV1 {
    pub fn validate(&self) -> Result<(), InstallationClosureErrorV1> {
        if self.host_tag == 0 || self.host_adapter_id.as_bytes() == &[0; 32] {
            return Err(InstallationClosureErrorV1::InvalidHostEntry);
        }
        let activation_refs = [
            self.skill_activation_claim_ref.as_ref(),
            self.mcp_packet_descriptor_claim_ref.as_ref(),
            self.mcp_cli_search_descriptor_claim_ref.as_ref(),
        ];
        let running_ref = self.running_catalog_observation_ref.as_ref();
        match self.admission_state {
            HostAdmissionStateV1::Admitted
                if activation_refs.iter().all(|reference| reference.is_some())
                    && running_ref.is_some() => {}
            HostAdmissionStateV1::Absent | HostAdmissionStateV1::RequiredBlocked
                if activation_refs.iter().all(|reference| reference.is_none())
                    && running_ref.is_none() => {}
            _ => return Err(InstallationClosureErrorV1::HostAdmissionClosureMismatch),
        }
        for reference in activation_refs.into_iter().flatten() {
            require_ref(
                reference,
                &self.domain,
                DistributionRuntimeObjectKindV1::InstalledResourceClaim,
            )?;
        }
        if let Some(reference) = running_ref {
            require_ref(
                reference,
                &self.domain,
                DistributionRuntimeObjectKindV1::RunningCatalogObservation,
            )?;
        }
        if self.mcp_packet_descriptor_claim_ref == self.mcp_cli_search_descriptor_claim_ref
            && self.admission_state == HostAdmissionStateV1::Admitted
        {
            return Err(InstallationClosureErrorV1::McpDescriptorClaimsNotDistinct);
        }
        Ok(())
    }

    pub fn canonical_value(&self) -> Result<CborValue, InstallationClosureErrorV1> {
        self.validate()?;
        Ok(CborValue::Array(vec![
            CborValue::Unsigned(self.host_tag),
            self.domain.canonical_value(),
            bytes(self.host_adapter_id),
            CborValue::Unsigned(self.admission_state.numeric_tag()),
            optional_ref(self.skill_activation_claim_ref.as_ref()),
            optional_ref(self.mcp_packet_descriptor_claim_ref.as_ref()),
            optional_ref(self.mcp_cli_search_descriptor_claim_ref.as_ref()),
            optional_ref(self.running_catalog_observation_ref.as_ref()),
            CborValue::Unsigned(1),
        ]))
    }

    fn add_references(&self, references: &mut BTreeSet<StoreObjectIdV1>) {
        for reference in [
            self.skill_activation_claim_ref.as_ref(),
            self.mcp_packet_descriptor_claim_ref.as_ref(),
            self.mcp_cli_search_descriptor_claim_ref.as_ref(),
            self.running_catalog_observation_ref.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            references.insert(reference.object_id());
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserAgentInstallationClosureV1 {
    pub domain: DistributionDomainRefV1,
    pub release_id: ReleaseIdV1,
    pub binary_claim_ref: DistributionScopedObjectRefV1,
    pub tui_closure_ref: Option<DistributionScopedObjectRefV1>,
    pub capability_catalog_id: CommitmentV1,
    pub maestro_skill_claim_ref: DistributionScopedObjectRefV1,
    pub agents_activation_claim_ref: DistributionScopedObjectRefV1,
    pub claude_activation_claim_ref: DistributionScopedObjectRefV1,
    pub host_entries: Vec<HostActivationEntryV1>,
    pub claim_set_ref: DistributionScopedObjectRefV1,
    pub receipt_ref: DistributionScopedObjectRefV1,
    pub snapshot_catalog_ref: DistributionScopedObjectRefV1,
    pub recovery_root_set_ref: DistributionScopedObjectRefV1,
    pub verification_result_ref: DistributionScopedObjectRefV1,
}

impl UserAgentInstallationClosureV1 {
    pub fn validate(&self) -> Result<(), InstallationClosureErrorV1> {
        if self.domain.kind() != DistributionDomainKindV1::InstallationDomain
            || self.release_id.as_bytes() == &[0; 32]
            || self.capability_catalog_id.as_bytes() == &[0; 32]
        {
            return Err(InstallationClosureErrorV1::WrongClosureDomain);
        }
        for (reference, kind) in [
            (
                &self.binary_claim_ref,
                DistributionRuntimeObjectKindV1::InstalledResourceClaim,
            ),
            (
                &self.maestro_skill_claim_ref,
                DistributionRuntimeObjectKindV1::InstalledResourceClaim,
            ),
            (
                &self.agents_activation_claim_ref,
                DistributionRuntimeObjectKindV1::InstalledResourceClaim,
            ),
            (
                &self.claude_activation_claim_ref,
                DistributionRuntimeObjectKindV1::InstalledResourceClaim,
            ),
            (
                &self.claim_set_ref,
                DistributionRuntimeObjectKindV1::InstalledResourceClaimSet,
            ),
            (
                &self.receipt_ref,
                DistributionRuntimeObjectKindV1::DistributionReceipt,
            ),
            (
                &self.snapshot_catalog_ref,
                DistributionRuntimeObjectKindV1::OrdinarySnapshotCatalog,
            ),
            (
                &self.recovery_root_set_ref,
                DistributionRuntimeObjectKindV1::RecoveryRootSet,
            ),
            (
                &self.verification_result_ref,
                DistributionRuntimeObjectKindV1::VerificationResult,
            ),
        ] {
            require_ref(reference, &self.domain, kind)?;
        }
        if let Some(tui_closure) = &self.tui_closure_ref {
            require_ref(
                tui_closure,
                &self.domain,
                DistributionRuntimeObjectKindV1::TuiClosure,
            )?;
        }
        let mut prior_key = None;
        let mut adapters = BTreeSet::new();
        for entry in &self.host_entries {
            entry.validate()?;
            if entry.domain != self.domain
                || !adapters.insert(entry.host_adapter_id)
                || prior_key.is_some_and(|prior| prior >= entry.host_tag)
            {
                return Err(InstallationClosureErrorV1::InvalidHostEntry);
            }
            if entry.admission_state == HostAdmissionStateV1::Admitted
                && entry.skill_activation_claim_ref.as_ref() != Some(&self.maestro_skill_claim_ref)
            {
                return Err(InstallationClosureErrorV1::HostSkillClaimMismatch);
            }
            prior_key = Some(entry.host_tag);
        }
        Ok(())
    }

    pub fn to_store_object(&self) -> Result<StoreObjectV1, InstallationClosureErrorV1> {
        self.validate()?;
        let mut references = BTreeSet::from([
            self.binary_claim_ref.object_id(),
            self.maestro_skill_claim_ref.object_id(),
            self.agents_activation_claim_ref.object_id(),
            self.claude_activation_claim_ref.object_id(),
            self.claim_set_ref.object_id(),
            self.receipt_ref.object_id(),
            self.snapshot_catalog_ref.object_id(),
            self.recovery_root_set_ref.object_id(),
            self.verification_result_ref.object_id(),
        ]);
        if let Some(tui_closure) = &self.tui_closure_ref {
            references.insert(tui_closure.object_id());
        }
        for entry in &self.host_entries {
            entry.add_references(&mut references);
        }
        store_object(
            DistributionRuntimeObjectKindV1::UserAgentInstallationClosure,
            CborValue::Array(vec![
                self.domain.canonical_value(),
                CborValue::Unsigned(1),
                bytes(self.release_id),
                self.binary_claim_ref.canonical_value(),
                optional_ref(self.tui_closure_ref.as_ref()),
                bytes(self.capability_catalog_id),
                self.maestro_skill_claim_ref.canonical_value(),
                self.agents_activation_claim_ref.canonical_value(),
                self.claude_activation_claim_ref.canonical_value(),
                CborValue::Array(
                    self.host_entries
                        .iter()
                        .map(HostActivationEntryV1::canonical_value)
                        .collect::<Result<Vec<_>, _>>()?,
                ),
                self.claim_set_ref.canonical_value(),
                self.receipt_ref.canonical_value(),
                self.snapshot_catalog_ref.canonical_value(),
                self.recovery_root_set_ref.canonical_value(),
                self.verification_result_ref.canonical_value(),
                CborValue::Unsigned(1),
            ]),
            references,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryInstallationClosureV1 {
    pub domain: DistributionDomainRefV1,
    pub agent_bootstrap_claim_ref: DistributionScopedObjectRefV1,
    pub root_agents_managed_block_claim_ref: DistributionScopedObjectRefV1,
    pub adapt_resource_claim_refs: Vec<(u64, DistributionScopedObjectRefV1)>,
    pub claim_set_ref: DistributionScopedObjectRefV1,
    pub receipt_ref: DistributionScopedObjectRefV1,
    pub snapshot_catalog_ref: DistributionScopedObjectRefV1,
    pub recovery_root_set_ref: DistributionScopedObjectRefV1,
    pub verification_result_ref: DistributionScopedObjectRefV1,
}

impl RepositoryInstallationClosureV1 {
    pub fn validate(&self) -> Result<(), InstallationClosureErrorV1> {
        if self.domain.kind() != DistributionDomainKindV1::RepositoryDomain {
            return Err(InstallationClosureErrorV1::WrongClosureDomain);
        }
        for (reference, kind) in [
            (
                &self.agent_bootstrap_claim_ref,
                DistributionRuntimeObjectKindV1::InstalledResourceClaim,
            ),
            (
                &self.root_agents_managed_block_claim_ref,
                DistributionRuntimeObjectKindV1::InstalledResourceClaim,
            ),
            (
                &self.claim_set_ref,
                DistributionRuntimeObjectKindV1::InstalledResourceClaimSet,
            ),
            (
                &self.receipt_ref,
                DistributionRuntimeObjectKindV1::DistributionReceipt,
            ),
            (
                &self.snapshot_catalog_ref,
                DistributionRuntimeObjectKindV1::OrdinarySnapshotCatalog,
            ),
            (
                &self.recovery_root_set_ref,
                DistributionRuntimeObjectKindV1::RecoveryRootSet,
            ),
            (
                &self.verification_result_ref,
                DistributionRuntimeObjectKindV1::VerificationResult,
            ),
        ] {
            require_ref(reference, &self.domain, kind)?;
        }
        let mut prior_tag = 0;
        let mut identities = BTreeSet::new();
        for (tag, reference) in &self.adapt_resource_claim_refs {
            require_ref(
                reference,
                &self.domain,
                DistributionRuntimeObjectKindV1::InstalledResourceClaim,
            )?;
            if *tag <= prior_tag || !identities.insert(reference.object_id()) {
                return Err(InstallationClosureErrorV1::InvalidAdaptClaimSet);
            }
            prior_tag = *tag;
        }
        Ok(())
    }

    pub fn to_store_object(&self) -> Result<StoreObjectV1, InstallationClosureErrorV1> {
        self.validate()?;
        let mut references = BTreeSet::from([
            self.agent_bootstrap_claim_ref.object_id(),
            self.root_agents_managed_block_claim_ref.object_id(),
            self.claim_set_ref.object_id(),
            self.receipt_ref.object_id(),
            self.snapshot_catalog_ref.object_id(),
            self.recovery_root_set_ref.object_id(),
            self.verification_result_ref.object_id(),
        ]);
        references.extend(
            self.adapt_resource_claim_refs
                .iter()
                .map(|(_, reference)| reference.object_id()),
        );
        store_object(
            DistributionRuntimeObjectKindV1::RepositoryInstallationClosure,
            CborValue::Array(vec![
                self.domain.canonical_value(),
                CborValue::Unsigned(1),
                self.agent_bootstrap_claim_ref.canonical_value(),
                self.root_agents_managed_block_claim_ref.canonical_value(),
                CborValue::Array(
                    self.adapt_resource_claim_refs
                        .iter()
                        .map(|(tag, reference)| {
                            CborValue::Array(vec![
                                CborValue::Unsigned(*tag),
                                reference.canonical_value(),
                            ])
                        })
                        .collect(),
                ),
                self.claim_set_ref.canonical_value(),
                self.receipt_ref.canonical_value(),
                self.snapshot_catalog_ref.canonical_value(),
                self.recovery_root_set_ref.canonical_value(),
                self.verification_result_ref.canonical_value(),
                CborValue::Unsigned(1),
            ]),
            references,
        )
    }
}

#[derive(Debug, Error)]
pub enum InstallationClosureErrorV1 {
    #[error("Installation closure is bound to the wrong Distribution domain")]
    WrongClosureDomain,
    #[error("host entry tag, adapter identity, domain, or order is invalid")]
    InvalidHostEntry,
    #[error("host admission state does not carry its exact activation closure")]
    HostAdmissionClosureMismatch,
    #[error("one admitted host must carry two distinct MCP descriptor claims")]
    McpDescriptorClaimsNotDistinct,
    #[error("admitted host does not reference the closure's single Maestro Skill claim")]
    HostSkillClaimMismatch,
    #[error("Repository adapt Resource claims must be strictly ordered and unique")]
    InvalidAdaptClaimSet,
    #[error(transparent)]
    Model(#[from] DistributionModelErrorV1),
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error(transparent)]
    StoreObject(#[from] StoreObjectError),
}

fn require_ref(
    reference: &DistributionScopedObjectRefV1,
    domain: &DistributionDomainRefV1,
    kind: DistributionRuntimeObjectKindV1,
) -> Result<(), InstallationClosureErrorV1> {
    reference.require_same_domain(domain)?;
    reference.require_kind(kind)?;
    Ok(())
}

fn optional_ref(reference: Option<&DistributionScopedObjectRefV1>) -> CborValue {
    CborValue::optional(reference.map(DistributionScopedObjectRefV1::canonical_value))
}

fn bytes(value: CommitmentV1) -> CborValue {
    CborValue::Bytes(value.as_bytes().to_vec())
}

fn store_object(
    kind: DistributionRuntimeObjectKindV1,
    value: CborValue,
    references: BTreeSet<StoreObjectIdV1>,
) -> Result<StoreObjectV1, InstallationClosureErrorV1> {
    let schema_id = kind
        .schema_id()
        .expect("invariant: C868 Installation closure kind has a frozen SchemaId");
    Ok(StoreObjectV1::new(
        schema_id,
        value,
        references.into_iter().collect(),
    )?)
}
