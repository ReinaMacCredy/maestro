#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u64)]
pub enum ContractComponentKindV1 {
    IntendedOutcome = 1,
    AcceptanceBoundary = 2,
    MaterialScope = 3,
    AffectedSurfaces = 4,
    NonGoals = 5,
    StepDefinitions = 6,
    StepGraphSnapshot = 7,
    GateSnapshot = 8,
    PolicyProfileProvenance = 9,
    PublicationAuthorityRequirement = 10,
    CompletionAuthorityRequirement = 11,
    NormativeInputs = 12,
    ResourceLimits = 13,
    ExternalTargets = 14,
    OperatingConstraints = 15,
    CapabilityCensus = 16,
    LiteralSchemaClosure = 17,
    LiteralManifestClosure = 18,
    ResourceClosure = 19,
    BundleClosure = 20,
    ReleaseResourceCensus = 21,
    ReleaseClosure = 22,
    MigrationRollbackRemoval = 23,
    StageProofMatrix = 24,
}

impl ContractComponentKindV1 {
    pub const ALL: [Self; 24] = [
        Self::IntendedOutcome,
        Self::AcceptanceBoundary,
        Self::MaterialScope,
        Self::AffectedSurfaces,
        Self::NonGoals,
        Self::StepDefinitions,
        Self::StepGraphSnapshot,
        Self::GateSnapshot,
        Self::PolicyProfileProvenance,
        Self::PublicationAuthorityRequirement,
        Self::CompletionAuthorityRequirement,
        Self::NormativeInputs,
        Self::ResourceLimits,
        Self::ExternalTargets,
        Self::OperatingConstraints,
        Self::CapabilityCensus,
        Self::LiteralSchemaClosure,
        Self::LiteralManifestClosure,
        Self::ResourceClosure,
        Self::BundleClosure,
        Self::ReleaseResourceCensus,
        Self::ReleaseClosure,
        Self::MigrationRollbackRemoval,
        Self::StageProofMatrix,
    ];

    pub const fn tag(self) -> u64 {
        self as u64
    }
}

impl TryFrom<u64> for ContractComponentKindV1 {
    type Error = ComponentKindError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.tag() == value)
            .ok_or(ComponentKindError::UnknownTag(value))
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ComponentKindError {
    #[error("unknown ContractComponentKindV1 tag {0}")]
    UnknownTag(u64),
}
use thiserror::Error;
