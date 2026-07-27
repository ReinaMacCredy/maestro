use thiserror::Error;

use crate::domain::vnext::distribution::CommitmentV1;
use crate::domain::vnext::identity::{SchemaIdV1, StoreObjectIdV1};
use crate::domain::vnext::persistence::{StoreDomainV1, StoreRoleV1};
use crate::foundation::core::deterministic_cbor::CborValue;

pub const DISTRIBUTION_DOMAIN_REF_SCHEMA_ID_V1: &str =
    "87ee40e772df0915f86568d2c85e29eb06882d2e2c9f68ec3fbbdb3d46a9e792";
pub const DISTRIBUTION_SCOPED_OBJECT_REF_SCHEMA_ID_V1: &str =
    "01534e1c233047cb4ce25ddf0317cccacd230d244876d6ff7b8c9d6f98062c0a";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DistributionDomainKindV1 {
    RepositoryDomain,
    InstallationDomain,
}

impl DistributionDomainKindV1 {
    pub const fn numeric_tag(self) -> u64 {
        match self {
            Self::RepositoryDomain => 1,
            Self::InstallationDomain => 2,
        }
    }

    pub const fn store_role(self) -> StoreRoleV1 {
        match self {
            Self::RepositoryDomain => StoreRoleV1::Repository,
            Self::InstallationDomain => StoreRoleV1::Installation,
        }
    }
}

impl From<StoreRoleV1> for DistributionDomainKindV1 {
    fn from(role: StoreRoleV1) -> Self {
        match role {
            StoreRoleV1::Repository => Self::RepositoryDomain,
            StoreRoleV1::Installation => Self::InstallationDomain,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DistributionDomainRefV1 {
    kind: DistributionDomainKindV1,
    domain_id: CommitmentV1,
    store_generation_id: CommitmentV1,
    authority_epoch_id: CommitmentV1,
}

impl DistributionDomainRefV1 {
    pub fn new(
        kind: DistributionDomainKindV1,
        domain_id: CommitmentV1,
        store_generation_id: CommitmentV1,
        authority_epoch_id: CommitmentV1,
    ) -> Result<Self, DistributionModelErrorV1> {
        if [domain_id, store_generation_id, authority_epoch_id]
            .iter()
            .any(|commitment| commitment.as_bytes() == &[0; 32])
        {
            return Err(DistributionModelErrorV1::ZeroCommitment);
        }
        Ok(Self {
            kind,
            domain_id,
            store_generation_id,
            authority_epoch_id,
        })
    }

    pub fn from_store(
        domain: &StoreDomainV1,
        store_generation_id: [u8; 32],
        authority_epoch_id: [u8; 32],
    ) -> Result<Self, DistributionModelErrorV1> {
        Self::new(
            domain.role().into(),
            CommitmentV1::from_bytes(*domain.id().as_bytes()),
            CommitmentV1::from_bytes(store_generation_id),
            CommitmentV1::from_bytes(authority_epoch_id),
        )
    }

    pub const fn kind(&self) -> DistributionDomainKindV1 {
        self.kind
    }

    pub const fn domain_id(&self) -> CommitmentV1 {
        self.domain_id
    }

    pub const fn store_generation_id(&self) -> CommitmentV1 {
        self.store_generation_id
    }

    pub const fn authority_epoch_id(&self) -> CommitmentV1 {
        self.authority_epoch_id
    }

    pub fn matches_store(&self, domain: &StoreDomainV1, generation_id: [u8; 32]) -> bool {
        self.kind.store_role() == domain.role()
            && self.domain_id.as_bytes() == domain.id().as_bytes()
            && self.store_generation_id.as_bytes() == &generation_id
    }

    pub fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            CborValue::Unsigned(self.kind.numeric_tag()),
            bytes(self.domain_id),
            bytes(self.store_generation_id),
            bytes(self.authority_epoch_id),
        ])
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DistributionRuntimeObjectKindV1 {
    InstalledResourceClaim,
    InstalledResourceClaimSet,
    DistributionSnapshot,
    OrdinarySnapshotCatalog,
    DistributionReceipt,
    DistributionCommitRecord,
    HostActivation,
    UserAgentInstallationClosure,
    RepositoryInstallationClosure,
    InstallationCensus,
    ActionRequestOrCeremony,
    DistributionPlan,
    EffectIntentSet,
    VerificationResult,
    AuthorizationReceiptSet,
    OperationResult,
    RecoveryRootSet,
    CanonicalTargetIdentity,
    AliasClosure,
    RetentionPinSet,
    CleanupDebtSet,
    ContentObject,
    ManagedBlock,
    IdempotencyKey,
    DeclaredRootSet,
    HostAdapterSet,
    LegacyLocatorSet,
    ObservedDistributionState,
    TuiClosure,
    RunningCatalogObservation,
    EffectIntent,
}

impl DistributionRuntimeObjectKindV1 {
    pub const ALL: [Self; 31] = [
        Self::InstalledResourceClaim,
        Self::InstalledResourceClaimSet,
        Self::DistributionSnapshot,
        Self::OrdinarySnapshotCatalog,
        Self::DistributionReceipt,
        Self::DistributionCommitRecord,
        Self::HostActivation,
        Self::UserAgentInstallationClosure,
        Self::RepositoryInstallationClosure,
        Self::InstallationCensus,
        Self::ActionRequestOrCeremony,
        Self::DistributionPlan,
        Self::EffectIntentSet,
        Self::VerificationResult,
        Self::AuthorizationReceiptSet,
        Self::OperationResult,
        Self::RecoveryRootSet,
        Self::CanonicalTargetIdentity,
        Self::AliasClosure,
        Self::RetentionPinSet,
        Self::CleanupDebtSet,
        Self::ContentObject,
        Self::ManagedBlock,
        Self::IdempotencyKey,
        Self::DeclaredRootSet,
        Self::HostAdapterSet,
        Self::LegacyLocatorSet,
        Self::ObservedDistributionState,
        Self::TuiClosure,
        Self::RunningCatalogObservation,
        Self::EffectIntent,
    ];

    pub const fn numeric_tag(self) -> u64 {
        self as u64 + 1
    }

    pub fn schema_id(self) -> Option<SchemaIdV1> {
        let digest = match self {
            Self::InstalledResourceClaim => {
                "d85f58640e2ca307400fe2994939536e7496a8db158a0a32e710c033cbec6103"
            }
            Self::InstalledResourceClaimSet => {
                "f5cf0c77d42e954ba6f1239f9ec0110ef9f5b099f7df485769f3e7728ad99de6"
            }
            Self::DistributionSnapshot => {
                "c2bdb12fa79377d4b1c3dfff65f1e38909907e052ee01be0bf21cad6b57cfa9c"
            }
            Self::OrdinarySnapshotCatalog => {
                "3d9e05ded75660352700c1f8b9940fa1ee170c2d768caccb33c087023d43c366"
            }
            Self::DistributionReceipt => {
                "c493d2cf85aa68dd8b23c3555afe71f74566fe8aace9fc4d1e6a25169700be2b"
            }
            Self::DistributionCommitRecord => {
                "7f5b3401095de19458598b7bf778fdc19334cdc1c68834d05525430bb6eb7caf"
            }
            Self::HostActivation => {
                "f66e7e341f2a5cebe91468d9323254400dbbd2388109e4250ab899c4ceda4f86"
            }
            Self::UserAgentInstallationClosure => {
                "65e911797ccc2589890ee80b244cf0b52ee927071645b6593cca0ef46deb7831"
            }
            Self::RepositoryInstallationClosure => {
                "dbf2db3b16cfc2891846e559fb8d6e2238f9d1945594b18bcbabc0909995fe0c"
            }
            Self::InstallationCensus => {
                "37f954d258ef478d442b1358f3ffce380270f8a2dc2a42072b964f304dbb0ad5"
            }
            _ => return None,
        };
        Some(
            SchemaIdV1::parse(&format!("sha256:{digest}"))
                .expect("invariant: frozen C868 runtime SchemaId is canonical"),
        )
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DistributionScopedObjectRefV1 {
    domain: DistributionDomainRefV1,
    object_kind: DistributionRuntimeObjectKindV1,
    object_id: StoreObjectIdV1,
}

impl DistributionScopedObjectRefV1 {
    pub fn new(
        domain: DistributionDomainRefV1,
        object_kind: DistributionRuntimeObjectKindV1,
        object_id: StoreObjectIdV1,
    ) -> Result<Self, DistributionModelErrorV1> {
        if object_id.as_bytes() == &[0; 32] {
            return Err(DistributionModelErrorV1::ZeroCommitment);
        }
        Ok(Self {
            domain,
            object_kind,
            object_id,
        })
    }

    pub const fn domain(&self) -> &DistributionDomainRefV1 {
        &self.domain
    }

    pub const fn object_kind(&self) -> DistributionRuntimeObjectKindV1 {
        self.object_kind
    }

    pub const fn object_id(&self) -> StoreObjectIdV1 {
        self.object_id
    }

    pub fn require_kind(
        &self,
        expected: DistributionRuntimeObjectKindV1,
    ) -> Result<(), DistributionModelErrorV1> {
        if self.object_kind != expected {
            return Err(DistributionModelErrorV1::ObjectKindMismatch {
                expected,
                observed: self.object_kind,
            });
        }
        Ok(())
    }

    pub fn require_same_domain(
        &self,
        expected: &DistributionDomainRefV1,
    ) -> Result<(), DistributionModelErrorV1> {
        if &self.domain != expected {
            return Err(DistributionModelErrorV1::CrossDomainReference);
        }
        Ok(())
    }

    pub fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            self.domain.canonical_value(),
            CborValue::Unsigned(self.object_kind.numeric_tag()),
            CborValue::Bytes(self.object_id.as_bytes().to_vec()),
        ])
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum DistributionModelErrorV1 {
    #[error("distribution identity commitments must be non-zero")]
    ZeroCommitment,
    #[error("distribution reference crosses an exact domain, Generation, or Authority epoch")]
    CrossDomainReference,
    #[error("distribution reference kind mismatch: expected {expected:?}, observed {observed:?}")]
    ObjectKindMismatch {
        expected: DistributionRuntimeObjectKindV1,
        observed: DistributionRuntimeObjectKindV1,
    },
}

fn bytes(commitment: CommitmentV1) -> CborValue {
    CborValue::Bytes(commitment.as_bytes().to_vec())
}
