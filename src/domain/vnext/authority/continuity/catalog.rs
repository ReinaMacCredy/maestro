use std::fmt;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::super::closed::{AuthorityContextKindV1, AuthorityTagError};

const MAX_REFERENCE_SEED_BYTES: usize = 192;

macro_rules! closed_continuity_enum {
    ($name:ident, $error:ident, $len:literal, [$($tag:literal => $variant:ident),+ $(,)?]) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #[repr(u8)]
        pub enum $name { $($variant = $tag),+ }

        impl $name {
            pub const ALL: [Self; $len] = [$(Self::$variant),+];
        }

        impl TryFrom<u8> for $name {
            type Error = AuthorityTagError;

            fn try_from(tag: u8) -> Result<Self, Self::Error> {
                match tag {
                    $($tag => Ok(Self::$variant),)+
                    value => Err(AuthorityTagError::$error(value)),
                }
            }
        }
    };
}

closed_continuity_enum!(RepositoryAuthorityContinuityClassV1, UnknownRepositoryContinuityClass, 35, [
    1 => RepositoryOrdinaryMutationCapacityState,
    2 => RepositoryAuthorityAdministrationCapacityState,
    3 => RepositoryEvidenceAcquisitionCapacityState,
    4 => RepositoryPlanningPublicationCapacityState,
    5 => RepositoryExternalEffectCapacityState,
    6 => RepositoryPersistenceMaintenanceCapacityState,
    7 => RepositoryStoreGenerationCurrentness,
    8 => RepositoryGovernanceHead,
    9 => RepositoryAuthorityEpochState,
    10 => RepositoryTrustRootState,
    11 => RepositoryPrincipalBindingState,
    12 => RepositorySessionState,
    13 => RepositoryGrantState,
    14 => RepositoryDelegationState,
    15 => RepositoryMandateState,
    16 => RepositoryRevocationState,
    17 => RepositoryAuthorizationReceiptState,
    18 => RepositoryConsumptionCellState,
    19 => RepositoryContinuityState,
    20 => RepositoryTrustedTimeState,
    21 => RepositoryRecoveryCommitmentState,
    22 => RepositoryRecoveryAdmissionState,
    23 => RepositoryStepExecutionState,
    24 => RepositoryEffectIntentState,
    25 => RepositoryEvidenceState,
    26 => RepositoryGateSnapshot,
    27 => RepositoryPlanningState,
    28 => RepositoryCoordinationState,
    29 => RepositoryDesignDecisionState,
    30 => RepositoryContractState,
    31 => RepositoryWorkState,
    32 => RepositoryPersistenceRetentionState,
    33 => RepositoryMemoryState,
    34 => RepositoryIntakeState,
    35 => RepositoryResearchState,
]);

closed_continuity_enum!(InstallationAuthorityContinuityClassV1, UnknownInstallationContinuityClass, 30, [
    1 => InstallationAuthorityAdministrationCapacityState,
    2 => InstallationDistributionMutationCapacityState,
    3 => InstallationGovernedReviewPublicationCapacityState,
    4 => InstallationExternalEffectCapacityState,
    5 => InstallationWriterAdministrationCapacityState,
    6 => InstallationPersistenceMaintenanceCapacityState,
    7 => InstallationLocatorCurrentness,
    8 => InstallationStoreGenerationCurrentness,
    9 => InstallationGovernanceHead,
    10 => InstallationAuthorityEpochState,
    11 => InstallationTrustRootState,
    12 => InstallationPrincipalBindingState,
    13 => InstallationGrantState,
    14 => InstallationMandateState,
    15 => InstallationRevocationState,
    16 => InstallationAuthorizationReceiptState,
    17 => InstallationConsumptionCellState,
    18 => InstallationContinuityState,
    19 => InstallationRecoveryCommitmentState,
    20 => InstallationRecoveryAdmissionState,
    21 => InstallationWriterCohortState,
    22 => InstallationClientCompatibilityState,
    23 => InstallationDistributionTargetState,
    24 => InstallationDistributionTransactionState,
    25 => InstallationBinarySlotState,
    26 => InstallationResourceManifestState,
    27 => InstallationGovernedReviewPublicationState,
    28 => InstallationEffectIntentState,
    29 => InstallationEvidenceState,
    30 => InstallationPersistenceRetentionState,
]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ContinuityClassIdV1 {
    Repository(RepositoryAuthorityContinuityClassV1),
    Installation(InstallationAuthorityContinuityClassV1),
}

impl ContinuityClassIdV1 {
    pub const fn context_kind(self) -> AuthorityContextKindV1 {
        match self {
            Self::Repository(_) => AuthorityContextKindV1::RepositoryAuthorityContext,
            Self::Installation(_) => AuthorityContextKindV1::InstallationAuthorityContext,
        }
    }

    pub const fn tag(self) -> u8 {
        match self {
            Self::Repository(class) => class as u8,
            Self::Installation(class) => class as u8,
        }
    }

    pub(crate) fn schema_value(self) -> CborValue {
        CborValue::Array(vec![
            CborValue::Unsigned(self.context_kind() as u64),
            CborValue::Unsigned(self.tag() as u64),
        ])
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum ContinuitySemanticOwnerV1 {
    Authority = 1,
    Work = 2,
    Contract = 3,
    Design = 4,
    Execution = 5,
    Evidence = 6,
    Planning = 7,
    Coordination = 8,
    Memory = 9,
    Intake = 10,
    Research = 11,
    Distribution = 12,
    Installation = 13,
    Persistence = 14,
}

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ContinuityReferenceV1([u8; 32]);

impl ContinuityReferenceV1 {
    pub fn derive(seed: &str) -> Result<Self, ContinuityReferenceError> {
        if seed.is_empty()
            || seed.len() > MAX_REFERENCE_SEED_BYTES
            || !seed.as_bytes().iter().all(u8::is_ascii_graphic)
        {
            return Err(ContinuityReferenceError::InvalidSeed);
        }
        let value = CborValue::Array(vec![
            CborValue::text("maestro.vnext.authority-continuity-reference.v1")?,
            CborValue::text(seed)?,
        ]);
        Ok(Self(
            Sha256::digest(deterministic_cbor::encode(&value)?).into(),
        ))
    }

    pub const fn from_digest(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn render(&self) -> String {
        let mut rendered = String::with_capacity(71);
        rendered.push_str("sha256:");
        for byte in self.0 {
            use std::fmt::Write;
            write!(&mut rendered, "{byte:02x}")
                .expect("invariant: writing hexadecimal into String cannot fail");
        }
        rendered
    }
}

impl fmt::Debug for ContinuityReferenceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("ContinuityReferenceV1")
            .field(&self.render())
            .finish()
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ContinuityReferenceError {
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error("continuity references require 1..=192 printable ASCII seed bytes")]
    InvalidSeed,
}

macro_rules! bounded_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(u16);

        impl $name {
            pub const fn new(value: u16) -> Option<Self> {
                if value == 0 { None } else { Some(Self(value)) }
            }

            pub const fn get(self) -> u16 {
                self.0
            }
        }
    };
}

bounded_id!(CoverageObligationIdV1);
bounded_id!(OwnerContributionIdV1);

pub(crate) fn required_class_ids(context_kind: AuthorityContextKindV1) -> Vec<ContinuityClassIdV1> {
    match context_kind {
        AuthorityContextKindV1::RepositoryAuthorityContext => {
            RepositoryAuthorityContinuityClassV1::ALL
                .map(ContinuityClassIdV1::Repository)
                .to_vec()
        }
        AuthorityContextKindV1::InstallationAuthorityContext => {
            InstallationAuthorityContinuityClassV1::ALL
                .map(ContinuityClassIdV1::Installation)
                .to_vec()
        }
    }
}

pub(crate) const fn canonical_owner(class_id: ContinuityClassIdV1) -> ContinuitySemanticOwnerV1 {
    use ContinuitySemanticOwnerV1 as Owner;
    use InstallationAuthorityContinuityClassV1 as I;
    use RepositoryAuthorityContinuityClassV1 as R;

    match class_id {
        ContinuityClassIdV1::Repository(
            R::RepositoryOrdinaryMutationCapacityState
            | R::RepositoryAuthorityAdministrationCapacityState
            | R::RepositoryEvidenceAcquisitionCapacityState
            | R::RepositoryPlanningPublicationCapacityState
            | R::RepositoryExternalEffectCapacityState
            | R::RepositoryPersistenceMaintenanceCapacityState
            | R::RepositoryStoreGenerationCurrentness
            | R::RepositoryGovernanceHead
            | R::RepositoryAuthorityEpochState
            | R::RepositoryTrustRootState
            | R::RepositoryPrincipalBindingState
            | R::RepositorySessionState
            | R::RepositoryGrantState
            | R::RepositoryDelegationState
            | R::RepositoryMandateState
            | R::RepositoryContinuityState
            | R::RepositoryRevocationState
            | R::RepositoryAuthorizationReceiptState
            | R::RepositoryConsumptionCellState
            | R::RepositoryTrustedTimeState
            | R::RepositoryRecoveryCommitmentState
            | R::RepositoryRecoveryAdmissionState,
        ) => Owner::Authority,
        ContinuityClassIdV1::Repository(
            R::RepositoryStepExecutionState | R::RepositoryEffectIntentState,
        ) => Owner::Execution,
        ContinuityClassIdV1::Repository(R::RepositoryEvidenceState | R::RepositoryGateSnapshot) => {
            Owner::Evidence
        }
        ContinuityClassIdV1::Repository(R::RepositoryPlanningState) => Owner::Planning,
        ContinuityClassIdV1::Repository(R::RepositoryCoordinationState) => Owner::Coordination,
        ContinuityClassIdV1::Repository(R::RepositoryDesignDecisionState) => Owner::Design,
        ContinuityClassIdV1::Repository(R::RepositoryContractState) => Owner::Contract,
        ContinuityClassIdV1::Repository(R::RepositoryWorkState) => Owner::Work,
        ContinuityClassIdV1::Repository(R::RepositoryPersistenceRetentionState) => {
            Owner::Persistence
        }
        ContinuityClassIdV1::Repository(R::RepositoryMemoryState) => Owner::Memory,
        ContinuityClassIdV1::Repository(R::RepositoryIntakeState) => Owner::Intake,
        ContinuityClassIdV1::Repository(R::RepositoryResearchState) => Owner::Research,
        ContinuityClassIdV1::Installation(
            I::InstallationAuthorityAdministrationCapacityState
            | I::InstallationDistributionMutationCapacityState
            | I::InstallationGovernedReviewPublicationCapacityState
            | I::InstallationExternalEffectCapacityState
            | I::InstallationWriterAdministrationCapacityState
            | I::InstallationPersistenceMaintenanceCapacityState
            | I::InstallationLocatorCurrentness
            | I::InstallationStoreGenerationCurrentness
            | I::InstallationGovernanceHead
            | I::InstallationAuthorityEpochState
            | I::InstallationTrustRootState
            | I::InstallationPrincipalBindingState
            | I::InstallationGrantState
            | I::InstallationMandateState
            | I::InstallationContinuityState
            | I::InstallationRevocationState
            | I::InstallationAuthorizationReceiptState
            | I::InstallationConsumptionCellState
            | I::InstallationRecoveryCommitmentState
            | I::InstallationRecoveryAdmissionState,
        ) => Owner::Authority,
        ContinuityClassIdV1::Installation(
            I::InstallationWriterCohortState
            | I::InstallationClientCompatibilityState
            | I::InstallationBinarySlotState,
        ) => Owner::Installation,
        ContinuityClassIdV1::Installation(
            I::InstallationDistributionTargetState
            | I::InstallationDistributionTransactionState
            | I::InstallationResourceManifestState
            | I::InstallationGovernedReviewPublicationState,
        ) => Owner::Distribution,
        ContinuityClassIdV1::Installation(I::InstallationEffectIntentState) => Owner::Execution,
        ContinuityClassIdV1::Installation(I::InstallationEvidenceState) => Owner::Evidence,
        ContinuityClassIdV1::Installation(I::InstallationPersistenceRetentionState) => {
            Owner::Persistence
        }
    }
}
