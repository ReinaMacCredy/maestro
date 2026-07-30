use super::closed::TransitionGuardKindV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum TransitionGuardTermV1 {
    RepositoryGovernanceFloor,
    CurrentWorkRootOutgoingRequirement,
    CurrentWorkRootAbsence,
    CandidateWorkRootIncomingRequirement,
    CurrentRepositoryGovernanceFloor,
    ProposedRepositoryGovernanceFloor,
    CurrentPredeclaredContinuityOrRootRotationRequirement,
    CurrentCorePolicyTransitionProtocol,
    CurrentFloorPolicyTransitionRequirement,
    CurrentPolicyOutgoingPolicyTransitionRequirement,
    CandidatePolicyIncomingActivationRequirement,
    CurrentOwnedPolicyTransitionRequirement,
    CurrentCoreStructuralTransitionProtocol,
    CurrentFloorOutgoingStructuralTransitionRequirement,
    CandidateFloorIncomingStructuralActivationRequirement,
    CurrentPolicyOutgoingStructuralTransitionRequirement,
    CandidatePolicyIncomingStructuralActivationRequirement,
    CurrentPredeclaredStructuralContinuityRequirement,
    CurrentOwnedStructuralTransitionRequirement,
    CoreTrustedTimeTransitionFloor,
    CurrentStackOutgoingRequirement,
    CandidateStackIncomingRequirement,
    CurrentContinuityOwnerRotationRequirement,
    CurrentPredeclaredSourceContinuityRequirement,
    CoreExternalHighWaterTransitionFloor,
    CurrentLogicalCarrierProfileOutgoingRequirement,
    CandidateLogicalCarrierProfileIncomingRequirement,
    CurrentContinuityOwnerCarrierRotationRequirement,
    CurrentPredeclaredCarrierContinuityRequirement,
    CorePlannedTurnoverPreparationFloor,
    CurrentEpochOutgoingRequirement,
    CandidateEpochIncomingRequirement,
    CurrentContinuityOwnerPreparationRequirement,
    CurrentPredeclaredExternalContinuityRequirement,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransitionGuardTermBundleV1 {
    kind: TransitionGuardKindV1,
    terms: &'static [TransitionGuardTermV1],
}

impl TransitionGuardTermBundleV1 {
    pub const fn kind(self) -> TransitionGuardKindV1 {
        self.kind
    }

    pub const fn terms(self) -> &'static [TransitionGuardTermV1] {
        self.terms
    }
}

impl TransitionGuardKindV1 {
    pub const fn term_bundle(self) -> TransitionGuardTermBundleV1 {
        use TransitionGuardTermV1 as Term;

        let terms: &'static [TransitionGuardTermV1] = match self {
            Self::RepositoryWorkAuthorityPolicyTransition => &[
                Term::RepositoryGovernanceFloor,
                Term::CurrentWorkRootOutgoingRequirement,
                Term::CandidateWorkRootIncomingRequirement,
            ],
            Self::RepositoryFirstWorkPublication => &[
                Term::RepositoryGovernanceFloor,
                Term::CurrentWorkRootAbsence,
                Term::CandidateWorkRootIncomingRequirement,
            ],
            Self::RepositoryFloorOrTrustRootRotation => &[
                Term::CurrentRepositoryGovernanceFloor,
                Term::ProposedRepositoryGovernanceFloor,
                Term::CurrentPredeclaredContinuityOrRootRotationRequirement,
            ],
            Self::InstallationPolicyBindingReplacement => &[
                Term::CurrentCorePolicyTransitionProtocol,
                Term::CurrentFloorPolicyTransitionRequirement,
                Term::CurrentPolicyOutgoingPolicyTransitionRequirement,
                Term::CandidatePolicyIncomingActivationRequirement,
                Term::CurrentOwnedPolicyTransitionRequirement,
            ],
            Self::InstallationStructuralRootFloorReplacement => &[
                Term::CurrentCoreStructuralTransitionProtocol,
                Term::CurrentFloorOutgoingStructuralTransitionRequirement,
                Term::CandidateFloorIncomingStructuralActivationRequirement,
                Term::CurrentPolicyOutgoingStructuralTransitionRequirement,
                Term::CandidatePolicyIncomingStructuralActivationRequirement,
                Term::CurrentPredeclaredStructuralContinuityRequirement,
                Term::CurrentOwnedStructuralTransitionRequirement,
            ],
            Self::TrustedTimePolicyStackRotation => &[
                Term::CoreTrustedTimeTransitionFloor,
                Term::CurrentStackOutgoingRequirement,
                Term::CandidateStackIncomingRequirement,
                Term::CurrentContinuityOwnerRotationRequirement,
                Term::CurrentPredeclaredSourceContinuityRequirement,
            ],
            Self::ExternalLogicalCarrierProfileRotation => &[
                Term::CoreExternalHighWaterTransitionFloor,
                Term::CurrentLogicalCarrierProfileOutgoingRequirement,
                Term::CandidateLogicalCarrierProfileIncomingRequirement,
                Term::CurrentContinuityOwnerCarrierRotationRequirement,
                Term::CurrentPredeclaredCarrierContinuityRequirement,
            ],
            Self::PlannedEpochTurnoverPreparation => &[
                Term::CorePlannedTurnoverPreparationFloor,
                Term::CurrentEpochOutgoingRequirement,
                Term::CandidateEpochIncomingRequirement,
                Term::CurrentContinuityOwnerPreparationRequirement,
                Term::CurrentPredeclaredExternalContinuityRequirement,
            ],
        };
        TransitionGuardTermBundleV1 { kind: self, terms }
    }
}
