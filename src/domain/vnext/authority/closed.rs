use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum AuthorityContextKindV1 {
    RepositoryAuthorityContext = 1,
    InstallationAuthorityContext = 2,
}

impl TryFrom<u8> for AuthorityContextKindV1 {
    type Error = AuthorityTagError;

    fn try_from(tag: u8) -> Result<Self, Self::Error> {
        match tag {
            1 => Ok(Self::RepositoryAuthorityContext),
            2 => Ok(Self::InstallationAuthorityContext),
            value => Err(AuthorityTagError::UnknownAuthorityContextKind(value)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ActionAuthorityBasisKindV1 {
    OrdinaryLiveRuntime = 1,
    BootstrapControlG0 = 2,
    ContinuityMaintenance = 3,
}

impl TryFrom<u8> for ActionAuthorityBasisKindV1 {
    type Error = AuthorityTagError;

    fn try_from(tag: u8) -> Result<Self, Self::Error> {
        match tag {
            1 => Ok(Self::OrdinaryLiveRuntime),
            2 => Ok(Self::BootstrapControlG0),
            3 => Ok(Self::ContinuityMaintenance),
            value => Err(AuthorityTagError::UnknownActionAuthorityBasisKind(value)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum TransitionGuardKindV1 {
    RepositoryWorkAuthorityPolicyTransition = 1,
    RepositoryFirstWorkPublication = 2,
    RepositoryFloorOrTrustRootRotation = 3,
    InstallationPolicyBindingReplacement = 4,
    InstallationStructuralRootFloorReplacement = 5,
    TrustedTimePolicyStackRotation = 6,
    ExternalLogicalCarrierProfileRotation = 7,
    PlannedEpochTurnoverPreparation = 8,
}

impl TransitionGuardKindV1 {
    pub const ALL: [Self; 8] = [
        Self::RepositoryWorkAuthorityPolicyTransition,
        Self::RepositoryFirstWorkPublication,
        Self::RepositoryFloorOrTrustRootRotation,
        Self::InstallationPolicyBindingReplacement,
        Self::InstallationStructuralRootFloorReplacement,
        Self::TrustedTimePolicyStackRotation,
        Self::ExternalLogicalCarrierProfileRotation,
        Self::PlannedEpochTurnoverPreparation,
    ];
}

impl TryFrom<u8> for TransitionGuardKindV1 {
    type Error = AuthorityTagError;

    fn try_from(tag: u8) -> Result<Self, Self::Error> {
        match tag {
            1 => Ok(Self::RepositoryWorkAuthorityPolicyTransition),
            2 => Ok(Self::RepositoryFirstWorkPublication),
            3 => Ok(Self::RepositoryFloorOrTrustRootRotation),
            4 => Ok(Self::InstallationPolicyBindingReplacement),
            5 => Ok(Self::InstallationStructuralRootFloorReplacement),
            6 => Ok(Self::TrustedTimePolicyStackRotation),
            7 => Ok(Self::ExternalLogicalCarrierProfileRotation),
            8 => Ok(Self::PlannedEpochTurnoverPreparation),
            value => Err(AuthorityTagError::UnknownTransitionGuardKind(value)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum AuthorityTagError {
    #[error("unknown Authority Context kind tag {0}")]
    UnknownAuthorityContextKind(u8),
    #[error("unknown Action authority-basis kind tag {0}")]
    UnknownActionAuthorityBasisKind(u8),
    #[error("unknown transition-guard kind tag {0}")]
    UnknownTransitionGuardKind(u8),
    #[error("unknown Repository governed-capacity kind tag {0}")]
    UnknownRepositoryGovernedCapacityKind(u8),
    #[error("unknown Installation governed-capacity kind tag {0}")]
    UnknownInstallationGovernedCapacityKind(u8),
    #[error("unknown CMA Observation-publication purpose tag {0}")]
    UnknownCmaObservationPublicationPurpose(u8),
    #[error("unknown CMA EffectWithdrawal slot-family tag {0}")]
    UnknownCmaEffectWithdrawalSlotFamily(u8),
    #[error("unknown Bootstrap target tag {0}")]
    UnknownBootstrapTarget(u8),
    #[error("unknown Action outcome tag {0}")]
    UnknownActionOutcome(u8),
    #[error("unknown Repository Authority continuity class tag {0}")]
    UnknownRepositoryContinuityClass(u8),
    #[error("unknown Installation Authority continuity class tag {0}")]
    UnknownInstallationContinuityClass(u8),
}
