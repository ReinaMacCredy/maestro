use std::collections::BTreeSet;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::vnext::distribution::{BundleIdV1, CommitmentV1, ReleaseIdV1, ResourceIdV1};
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::{
    DistributionDomainRefV1, DistributionRuntimeObjectKindV1, DistributionScopedObjectRefV1,
};

const MAX_LOCATOR_BYTES_V1: usize = 4_096;
const MAX_ALIAS_COUNT_V1: usize = 4_096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetCustodyClassV1 {
    MaestroOwnedTarget,
    SharedManagedBlock,
    Unmanaged,
}

impl TargetCustodyClassV1 {
    pub const fn numeric_tag(self) -> u64 {
        match self {
            Self::MaestroOwnedTarget => 1,
            Self::SharedManagedBlock => 2,
            Self::Unmanaged => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagedTargetCustodyClassV1 {
    MaestroOwnedTarget,
    SharedManagedBlock,
}

impl ManagedTargetCustodyClassV1 {
    pub const fn numeric_tag(self) -> u64 {
        match self {
            Self::MaestroOwnedTarget => 1,
            Self::SharedManagedBlock => 2,
        }
    }

    pub const fn target_class(self) -> TargetCustodyClassV1 {
        match self {
            Self::MaestroOwnedTarget => TargetCustodyClassV1::MaestroOwnedTarget,
            Self::SharedManagedBlock => TargetCustodyClassV1::SharedManagedBlock,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnmanagedReasonV1 {
    Unclaimed,
    Foreign,
    ExternallyManaged,
    Ambiguous,
    Unsafe,
}

impl UnmanagedReasonV1 {
    pub const fn numeric_tag(self) -> u64 {
        match self {
            Self::Unclaimed => 1,
            Self::Foreign => 2,
            Self::ExternallyManaged => 3,
            Self::Ambiguous => 4,
            Self::Unsafe => 5,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetIdentityPartsV1 {
    pub display_locator: String,
    pub resolved_locator: String,
    pub declared_root_id: CommitmentV1,
    pub parent_identity_id: CommitmentV1,
    pub mount_identity_id: CommitmentV1,
    pub manager_realm_id: CommitmentV1,
    pub security_realm_id: CommitmentV1,
    pub observed_object_identity_id: Option<CommitmentV1>,
    pub vacant_slot: bool,
    pub aliases: BTreeSet<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalTargetIdentityV1 {
    domain: DistributionDomainRefV1,
    parts: TargetIdentityPartsV1,
    identity: CommitmentV1,
}

impl CanonicalTargetIdentityV1 {
    pub fn new(
        domain: DistributionDomainRefV1,
        parts: TargetIdentityPartsV1,
    ) -> Result<Self, CustodyErrorV1> {
        validate_locator(&parts.display_locator)?;
        validate_locator(&parts.resolved_locator)?;
        if parts.aliases.len() > MAX_ALIAS_COUNT_V1 {
            return Err(CustodyErrorV1::TooManyAliases);
        }
        for alias in &parts.aliases {
            validate_locator(alias)?;
        }
        let fixed = [
            parts.declared_root_id,
            parts.parent_identity_id,
            parts.mount_identity_id,
            parts.manager_realm_id,
            parts.security_realm_id,
        ];
        if fixed.iter().any(|value| value.as_bytes() == &[0; 32])
            || parts
                .observed_object_identity_id
                .is_some_and(|value| value.as_bytes() == &[0; 32])
        {
            return Err(CustodyErrorV1::ZeroCommitment);
        }
        if parts.vacant_slot == parts.observed_object_identity_id.is_some() {
            return Err(CustodyErrorV1::InvalidVacantSlot);
        }
        if parts.aliases.contains(&parts.resolved_locator) {
            return Err(CustodyErrorV1::ResolvedLocatorRepeatedAsAlias);
        }
        let value = identity_value(&domain, &parts)?;
        let identity =
            CommitmentV1::from_bytes(Sha256::digest(deterministic_cbor::encode(&value)?).into());
        Ok(Self {
            domain,
            parts,
            identity,
        })
    }

    pub const fn domain(&self) -> &DistributionDomainRefV1 {
        &self.domain
    }

    pub const fn identity(&self) -> CommitmentV1 {
        self.identity
    }

    pub const fn parts(&self) -> &TargetIdentityPartsV1 {
        &self.parts
    }

    pub fn canonical_value(&self) -> Result<CborValue, CborError> {
        identity_value(&self.domain, &self.parts)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagedBlockBoundaryV1 {
    pub start_marker: Vec<u8>,
    pub end_marker: Vec<u8>,
    pub block_sha256: CommitmentV1,
    pub outside_prefix_sha256: CommitmentV1,
    pub outside_suffix_sha256: CommitmentV1,
}

impl ManagedBlockBoundaryV1 {
    pub fn validate(&self) -> Result<(), CustodyErrorV1> {
        if self.start_marker.is_empty()
            || self.end_marker.is_empty()
            || self.start_marker == self.end_marker
            || self
                .start_marker
                .windows(self.end_marker.len())
                .any(|window| window == self.end_marker)
            || self
                .end_marker
                .windows(self.start_marker.len())
                .any(|window| window == self.start_marker)
        {
            return Err(CustodyErrorV1::InvalidManagedBlockBoundary);
        }
        if [
            self.block_sha256,
            self.outside_prefix_sha256,
            self.outside_suffix_sha256,
        ]
        .iter()
        .any(|value| value.as_bytes() == &[0; 32])
        {
            return Err(CustodyErrorV1::ZeroCommitment);
        }
        Ok(())
    }

    pub fn outside_bytes_match(&self, prefix: CommitmentV1, suffix: CommitmentV1) -> bool {
        self.outside_prefix_sha256 == prefix && self.outside_suffix_sha256 == suffix
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CustodyBasisV1 {
    pub domain: DistributionDomainRefV1,
    pub target_identity: CommitmentV1,
    pub alias_closure_id: CommitmentV1,
    pub receipt_ref: Option<DistributionScopedObjectRefV1>,
    pub claim_ref: Option<DistributionScopedObjectRefV1>,
    pub claimed_target_identity: Option<CommitmentV1>,
    pub resource_id: Option<ResourceIdV1>,
    pub bundle_id: Option<BundleIdV1>,
    pub release_id: Option<ReleaseIdV1>,
    pub claimed_content_sha256: Option<CommitmentV1>,
    pub observed_content_sha256: Option<CommitmentV1>,
    pub managed_block: Option<ManagedBlockBoundaryV1>,
    pub foreign_owner_observed: bool,
    pub external_manager_observed: bool,
    pub alias_ambiguous: bool,
    pub unsafe_path_state: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CustodyAssessmentV1 {
    class: TargetCustodyClassV1,
    unmanaged_reason: Option<UnmanagedReasonV1>,
    preexisting_drift: bool,
}

impl CustodyAssessmentV1 {
    pub fn assess(basis: &CustodyBasisV1) -> Result<Self, CustodyErrorV1> {
        if basis.target_identity.as_bytes() == &[0; 32]
            || basis.alias_closure_id.as_bytes() == &[0; 32]
        {
            return Err(CustodyErrorV1::ZeroCommitment);
        }
        if basis.unsafe_path_state {
            return Ok(Self::unmanaged(UnmanagedReasonV1::Unsafe));
        }
        if basis.alias_ambiguous {
            return Ok(Self::unmanaged(UnmanagedReasonV1::Ambiguous));
        }
        if basis.external_manager_observed {
            return Ok(Self::unmanaged(UnmanagedReasonV1::ExternallyManaged));
        }
        if basis.foreign_owner_observed {
            return Ok(Self::unmanaged(UnmanagedReasonV1::Foreign));
        }
        let (Some(receipt_ref), Some(claim_ref)) = (&basis.receipt_ref, &basis.claim_ref) else {
            return Ok(Self::unmanaged(UnmanagedReasonV1::Unclaimed));
        };
        receipt_ref.require_same_domain(&basis.domain)?;
        receipt_ref.require_kind(DistributionRuntimeObjectKindV1::DistributionReceipt)?;
        claim_ref.require_same_domain(&basis.domain)?;
        claim_ref.require_kind(DistributionRuntimeObjectKindV1::InstalledResourceClaim)?;
        let complete_claim = basis.claimed_target_identity == Some(basis.target_identity)
            && basis.resource_id.is_some()
            && basis.bundle_id.is_some()
            && basis.release_id.is_some()
            && basis.claimed_content_sha256.is_some();
        if !complete_claim {
            return Ok(Self::unmanaged(UnmanagedReasonV1::Unclaimed));
        }
        let preexisting_drift = basis.claimed_content_sha256 != basis.observed_content_sha256;
        let class = if let Some(block) = &basis.managed_block {
            block.validate()?;
            TargetCustodyClassV1::SharedManagedBlock
        } else {
            TargetCustodyClassV1::MaestroOwnedTarget
        };
        Ok(Self {
            class,
            unmanaged_reason: None,
            preexisting_drift,
        })
    }

    const fn unmanaged(reason: UnmanagedReasonV1) -> Self {
        Self {
            class: TargetCustodyClassV1::Unmanaged,
            unmanaged_reason: Some(reason),
            preexisting_drift: false,
        }
    }

    pub const fn class(&self) -> TargetCustodyClassV1 {
        self.class
    }

    pub const fn unmanaged_reason(&self) -> Option<UnmanagedReasonV1> {
        self.unmanaged_reason
    }

    pub const fn has_preexisting_drift(&self) -> bool {
        self.preexisting_drift
    }

    pub const fn permits_mutation(&self) -> bool {
        !matches!(self.class, TargetCustodyClassV1::Unmanaged)
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CustodyErrorV1 {
    #[error("target locator must be non-empty bounded ASCII")]
    InvalidLocator,
    #[error("canonical target identity exceeds the finite alias limit")]
    TooManyAliases,
    #[error("canonical target identity commitments must be non-zero")]
    ZeroCommitment,
    #[error("a vacant target slot must not also carry an observed object identity")]
    InvalidVacantSlot,
    #[error("the resolved locator must not be duplicated in its alias set")]
    ResolvedLocatorRepeatedAsAlias,
    #[error("managed block markers do not define one unambiguous bounded region")]
    InvalidManagedBlockBoundary,
    #[error(transparent)]
    DistributionModel(#[from] super::DistributionModelErrorV1),
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
}

fn validate_locator(locator: &str) -> Result<(), CustodyErrorV1> {
    if locator.is_empty() || locator.len() > MAX_LOCATOR_BYTES_V1 || !locator.is_ascii() {
        return Err(CustodyErrorV1::InvalidLocator);
    }
    Ok(())
}

fn identity_value(
    domain: &DistributionDomainRefV1,
    parts: &TargetIdentityPartsV1,
) -> Result<CborValue, CborError> {
    Ok(CborValue::Array(vec![
        CborValue::text("maestro.vnext.canonical-target-identity.v1")?,
        domain.canonical_value(),
        CborValue::text(parts.display_locator.clone())?,
        CborValue::text(parts.resolved_locator.clone())?,
        bytes(parts.declared_root_id),
        bytes(parts.parent_identity_id),
        bytes(parts.mount_identity_id),
        bytes(parts.manager_realm_id),
        bytes(parts.security_realm_id),
        CborValue::optional(parts.observed_object_identity_id.map(bytes)),
        CborValue::Bool(parts.vacant_slot),
        CborValue::Array(
            parts
                .aliases
                .iter()
                .map(|alias| CborValue::text(alias.clone()))
                .collect::<Result<Vec<_>, _>>()?,
        ),
    ]))
}

fn bytes(value: CommitmentV1) -> CborValue {
    CborValue::Bytes(value.as_bytes().to_vec())
}
