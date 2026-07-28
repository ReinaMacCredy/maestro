use std::collections::BTreeSet;

use sha2::Digest;
use thiserror::Error;

use crate::domain::distribution::runtime::{
    DistributionDomainRefV1, DistributionModelErrorV1, DistributionRuntimeObjectKindV1,
    DistributionScopedObjectRefV1, TargetCustodyClassV1, UnmanagedReasonV1,
};
use crate::domain::distribution::{
    BundleIdV1, CommitmentV1, ReleaseIdV1, ResourceDispositionV1, ResourceIdV1,
};
use crate::domain::identity::StoreObjectIdV1;
use crate::domain::persistence::{StoreObjectError, StoreObjectV1};
use crate::foundation::core::deterministic_cbor::{CborError, CborValue};

#[path = "legacy_quarantine.rs"]
mod legacy_quarantine;

const MAX_CENSUS_ENTRIES_V1: usize = 1_048_576;
const MAX_CONSUMER_REFS_V1: usize = 65_535;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstallationCensusClassV1 {
    Active,
    Inactive,
    Stale,
    Legacy,
    ModifiedManaged,
    Foreign,
    Ambiguous,
    Unsafe,
    Snapshot,
    Cache,
    Archive,
    RemovalCandidate,
}

impl InstallationCensusClassV1 {
    pub const fn numeric_tag(self) -> u64 {
        self as u64 + 1
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallationCensusEntryV1 {
    pub entry_tag: u64,
    pub display_locator: String,
    pub resolved_locator: String,
    pub classification: InstallationCensusClassV1,
    pub custody_class: TargetCustodyClassV1,
    pub unmanaged_reason: Option<UnmanagedReasonV1>,
    pub resource_id: Option<ResourceIdV1>,
    pub bundle_id: Option<BundleIdV1>,
    pub release_id: Option<ReleaseIdV1>,
    pub claim_ref: Option<DistributionScopedObjectRefV1>,
    pub receipt_ref: Option<DistributionScopedObjectRefV1>,
    pub content_sha256: Option<CommitmentV1>,
    pub alias_closure_ref: DistributionScopedObjectRefV1,
    pub consumer_refs: Vec<(u64, CommitmentV1)>,
    pub disposition: ResourceDispositionV1,
}

impl InstallationCensusEntryV1 {
    pub fn validate(
        &self,
        domain: &DistributionDomainRefV1,
    ) -> Result<(), InstallationCensusErrorV1> {
        if self.entry_tag == 0 || self.entry_tag > MAX_CENSUS_ENTRIES_V1 as u64 {
            return Err(InstallationCensusErrorV1::InvalidEntryTag);
        }
        validate_locator(&self.display_locator)?;
        validate_locator(&self.resolved_locator)?;
        require_ref(
            &self.alias_closure_ref,
            domain,
            DistributionRuntimeObjectKindV1::AliasClosure,
        )?;
        require_optional_ref(
            self.claim_ref.as_ref(),
            domain,
            DistributionRuntimeObjectKindV1::InstalledResourceClaim,
        )?;
        require_optional_ref(
            self.receipt_ref.as_ref(),
            domain,
            DistributionRuntimeObjectKindV1::DistributionReceipt,
        )?;
        if self.consumer_refs.len() > MAX_CONSUMER_REFS_V1
            || self
                .consumer_refs
                .iter()
                .any(|(tag, id)| *tag == 0 || id.as_bytes() == &[0; 32])
            || self.consumer_refs.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(InstallationCensusErrorV1::InvalidConsumerRefs);
        }
        let managed_identity_complete = self.resource_id.is_some()
            && self.bundle_id.is_some()
            && self.release_id.is_some()
            && self.claim_ref.is_some()
            && self.receipt_ref.is_some()
            && self.content_sha256.is_some();
        match self.custody_class {
            TargetCustodyClassV1::MaestroOwnedTarget | TargetCustodyClassV1::SharedManagedBlock
                if self.unmanaged_reason.is_none() && managed_identity_complete => {}
            TargetCustodyClassV1::Unmanaged
                if self.unmanaged_reason.is_some()
                    && self.claim_ref.is_none()
                    && self.receipt_ref.is_none() => {}
            _ => return Err(InstallationCensusErrorV1::CustodyClosureMismatch),
        }
        match self.classification {
            InstallationCensusClassV1::Active | InstallationCensusClassV1::ModifiedManaged
                if self.custody_class == TargetCustodyClassV1::Unmanaged =>
            {
                return Err(InstallationCensusErrorV1::ClassificationCustodyMismatch);
            }
            InstallationCensusClassV1::Foreign
                if self.unmanaged_reason != Some(UnmanagedReasonV1::Foreign) =>
            {
                return Err(InstallationCensusErrorV1::ClassificationCustodyMismatch);
            }
            InstallationCensusClassV1::Ambiguous
                if self.unmanaged_reason != Some(UnmanagedReasonV1::Ambiguous) =>
            {
                return Err(InstallationCensusErrorV1::ClassificationCustodyMismatch);
            }
            InstallationCensusClassV1::Unsafe
                if self.unmanaged_reason != Some(UnmanagedReasonV1::Unsafe) =>
            {
                return Err(InstallationCensusErrorV1::ClassificationCustodyMismatch);
            }
            _ => {}
        }
        for value in [
            self.resource_id,
            self.bundle_id,
            self.release_id,
            self.content_sha256,
        ]
        .into_iter()
        .flatten()
        {
            if value.as_bytes() == &[0; 32] {
                return Err(InstallationCensusErrorV1::ZeroCommitment);
            }
        }
        Ok(())
    }

    fn canonical_value(
        &self,
        domain: &DistributionDomainRefV1,
    ) -> Result<CborValue, InstallationCensusErrorV1> {
        self.validate(domain)?;
        Ok(CborValue::Array(vec![
            CborValue::Unsigned(self.entry_tag),
            CborValue::text(self.display_locator.clone())?,
            CborValue::text(self.resolved_locator.clone())?,
            CborValue::Unsigned(self.classification.numeric_tag()),
            CborValue::Unsigned(self.custody_class.numeric_tag()),
            CborValue::optional(
                self.unmanaged_reason
                    .map(|reason| CborValue::Unsigned(reason.numeric_tag())),
            ),
            optional_bytes(self.resource_id),
            optional_bytes(self.bundle_id),
            optional_bytes(self.release_id),
            optional_ref(self.claim_ref.as_ref()),
            optional_ref(self.receipt_ref.as_ref()),
            optional_bytes(self.content_sha256),
            self.alias_closure_ref.canonical_value(),
            CborValue::Array(
                self.consumer_refs
                    .iter()
                    .map(|(tag, id)| CborValue::Array(vec![CborValue::Unsigned(*tag), bytes(*id)]))
                    .collect(),
            ),
            CborValue::Unsigned(self.disposition.numeric_tag()),
            CborValue::Unsigned(1),
        ]))
    }

    fn add_references(&self, references: &mut BTreeSet<StoreObjectIdV1>) {
        references.insert(self.alias_closure_ref.object_id());
        for reference in [self.claim_ref.as_ref(), self.receipt_ref.as_ref()]
            .into_iter()
            .flatten()
        {
            references.insert(reference.object_id());
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallationCensusHeaderV1 {
    pub domain: DistributionDomainRefV1,
    pub inspection_request_ref: DistributionScopedObjectRefV1,
    pub declared_root_set_ref: DistributionScopedObjectRefV1,
    pub host_adapter_set_ref: DistributionScopedObjectRefV1,
    pub legacy_locator_set_ref: DistributionScopedObjectRefV1,
    pub observed_state_ref: DistributionScopedObjectRefV1,
    pub proof_profile_id: CommitmentV1,
}

impl InstallationCensusHeaderV1 {
    fn validate(&self) -> Result<(), InstallationCensusErrorV1> {
        for (reference, kind) in [
            (
                &self.inspection_request_ref,
                DistributionRuntimeObjectKindV1::ActionRequestOrCeremony,
            ),
            (
                &self.declared_root_set_ref,
                DistributionRuntimeObjectKindV1::DeclaredRootSet,
            ),
            (
                &self.host_adapter_set_ref,
                DistributionRuntimeObjectKindV1::HostAdapterSet,
            ),
            (
                &self.legacy_locator_set_ref,
                DistributionRuntimeObjectKindV1::LegacyLocatorSet,
            ),
            (
                &self.observed_state_ref,
                DistributionRuntimeObjectKindV1::ObservedDistributionState,
            ),
        ] {
            require_ref(reference, &self.domain, kind)?;
        }
        if self.proof_profile_id.as_bytes() == &[0; 32] {
            return Err(InstallationCensusErrorV1::ZeroCommitment);
        }
        Ok(())
    }

    fn canonical_value(&self, entry_count: usize) -> CborValue {
        CborValue::Array(vec![
            self.domain.canonical_value(),
            self.inspection_request_ref.canonical_value(),
            self.declared_root_set_ref.canonical_value(),
            self.host_adapter_set_ref.canonical_value(),
            self.legacy_locator_set_ref.canonical_value(),
            self.observed_state_ref.canonical_value(),
            CborValue::Unsigned(entry_count as u64),
            bytes(self.proof_profile_id),
            CborValue::Unsigned(1),
        ])
    }

    fn references(&self) -> BTreeSet<StoreObjectIdV1> {
        BTreeSet::from([
            self.inspection_request_ref.object_id(),
            self.declared_root_set_ref.object_id(),
            self.host_adapter_set_ref.object_id(),
            self.legacy_locator_set_ref.object_id(),
            self.observed_state_ref.object_id(),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallationCensusV1 {
    pub header: InstallationCensusHeaderV1,
    pub rows: Vec<(u64, CommitmentV1, InstallationCensusEntryV1)>,
}

impl InstallationCensusV1 {
    pub fn validate(&self) -> Result<(), InstallationCensusErrorV1> {
        self.header.validate()?;
        if self.rows.len() > MAX_CENSUS_ENTRIES_V1 {
            return Err(InstallationCensusErrorV1::TooManyEntries);
        }
        let mut prior_key = None;
        for (tag, row_id, entry) in &self.rows {
            entry.validate(&self.header.domain)?;
            let key = (*tag, *row_id);
            if *tag != entry.entry_tag
                || row_id.as_bytes() == &[0; 32]
                || prior_key.is_some_and(|prior| prior >= key)
            {
                return Err(InstallationCensusErrorV1::RowsNotStrictlySorted);
            }
            prior_key = Some(key);
        }
        Ok(())
    }

    pub fn to_store_object(&self) -> Result<StoreObjectV1, InstallationCensusErrorV1> {
        self.validate()?;
        let mut references = self.header.references();
        let rows = self
            .rows
            .iter()
            .map(|(tag, row_id, entry)| {
                entry.add_references(&mut references);
                Ok(CborValue::Array(vec![
                    CborValue::Unsigned(*tag),
                    bytes(*row_id),
                    entry.canonical_value(&self.header.domain)?,
                ]))
            })
            .collect::<Result<Vec<_>, InstallationCensusErrorV1>>()?;
        let schema_id = DistributionRuntimeObjectKindV1::InstallationCensus
            .schema_id()
            .expect("invariant: frozen InstallationCensusV1 SchemaId exists");
        Ok(StoreObjectV1::new(
            schema_id,
            CborValue::Array(vec![
                self.header.canonical_value(self.rows.len()),
                CborValue::Array(rows),
            ]),
            references.into_iter().collect(),
        )?)
    }

    pub(crate) fn admit_legacy_quarantine_roots_v3(
        &self,
        expected_sources: crate::foundation::core::legacy_quarantine::LegacyQuarantineExpectedSourceSetV3,
    ) -> Result<
        impl crate::foundation::core::legacy_quarantine::LegacyQuarantineRootAdmissionV3,
        InstallationCensusErrorV1,
    > {
        legacy_quarantine::InstallationRootAdmissionV3::mint_from_live_registry(
            self,
            expected_sources,
        )
        .map_err(|_| InstallationCensusErrorV1::LegacyAdmissionUnavailable)
    }

    pub(super) fn legacy_quarantine_root_snapshot_v3(
        &self,
    ) -> Result<InstallationLegacyRootSnapshotV3, InstallationCensusErrorV1> {
        self.validate()?;
        let mut roots = self
            .rows
            .iter()
            .filter(|(_, _, entry)| {
                matches!(
                    entry.classification,
                    InstallationCensusClassV1::Legacy
                        | InstallationCensusClassV1::RemovalCandidate
                        | InstallationCensusClassV1::ModifiedManaged
                        | InstallationCensusClassV1::Stale
                )
            })
            .map(|(_, _, entry)| entry.resolved_locator.clone())
            .collect::<Vec<_>>();
        roots.sort();
        roots.dedup();
        if roots.is_empty() {
            return Err(InstallationCensusErrorV1::MissingLegacyRoot);
        }
        let census_object = self.to_store_object()?;
        Ok(InstallationLegacyRootSnapshotV3 {
            roots,
            owner_currentness: *census_object.id().as_bytes(),
            owner_attestation: sha2::Sha256::digest(census_object.canonical_bytes()).into(),
        })
    }

    pub(super) fn legacy_root_universe_comparison_identity_v1(
        &self,
    ) -> Result<[u8; 32], InstallationCensusErrorV1> {
        self.validate()?;
        Ok(*self.to_store_object()?.id().as_bytes())
    }
}

pub(super) struct InstallationLegacyRootSnapshotV3 {
    pub(super) roots: Vec<String>,
    pub(super) owner_currentness: [u8; 32],
    pub(super) owner_attestation: [u8; 32],
}

#[derive(Debug, Error)]
pub enum InstallationCensusErrorV1 {
    #[error("Installation census entry tag is outside the frozen range")]
    InvalidEntryTag,
    #[error("Installation census exceeds the frozen entry bound")]
    TooManyEntries,
    #[error("Installation census rows are not a strictly ordered canonical set")]
    RowsNotStrictlySorted,
    #[error("Installation census locator must be non-empty bounded ASCII")]
    InvalidLocator,
    #[error("Installation census consumer references are not a canonical bounded set")]
    InvalidConsumerRefs,
    #[error("Installation census custody class, reason, claim, and Receipt disagree")]
    CustodyClosureMismatch,
    #[error("Installation census classification contradicts its custody class or reason")]
    ClassificationCustodyMismatch,
    #[error("Installation census commitment must be non-zero")]
    ZeroCommitment,
    #[error("Installation census has no live legacy root eligible for Stage 11 admission")]
    MissingLegacyRoot,
    #[error("Installation could not mint its live Stage 11 root admission")]
    LegacyAdmissionUnavailable,
    #[error(transparent)]
    Model(#[from] DistributionModelErrorV1),
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error(transparent)]
    StoreObject(#[from] StoreObjectError),
}

fn validate_locator(value: &str) -> Result<(), InstallationCensusErrorV1> {
    if value.is_empty() || value.len() > 4_096 || !value.is_ascii() {
        return Err(InstallationCensusErrorV1::InvalidLocator);
    }
    Ok(())
}

fn require_optional_ref(
    reference: Option<&DistributionScopedObjectRefV1>,
    domain: &DistributionDomainRefV1,
    kind: DistributionRuntimeObjectKindV1,
) -> Result<(), InstallationCensusErrorV1> {
    if let Some(reference) = reference {
        require_ref(reference, domain, kind)?;
    }
    Ok(())
}

fn require_ref(
    reference: &DistributionScopedObjectRefV1,
    domain: &DistributionDomainRefV1,
    kind: DistributionRuntimeObjectKindV1,
) -> Result<(), InstallationCensusErrorV1> {
    reference.require_same_domain(domain)?;
    reference.require_kind(kind)?;
    Ok(())
}

fn optional_ref(reference: Option<&DistributionScopedObjectRefV1>) -> CborValue {
    CborValue::optional(reference.map(DistributionScopedObjectRefV1::canonical_value))
}

fn optional_bytes(value: Option<CommitmentV1>) -> CborValue {
    CborValue::optional(value.map(bytes))
}

fn bytes(value: CommitmentV1) -> CborValue {
    CborValue::Bytes(value.as_bytes().to_vec())
}
