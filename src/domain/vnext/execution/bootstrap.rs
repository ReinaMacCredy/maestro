#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BootstrapTargetDispositionV1 {
    CandidateTarget,
    HardExclusion,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BootstrapControlTargetV1 {
    pub action: &'static str,
    pub disposition: BootstrapTargetDispositionV1,
}

pub const BOOTSTRAP_CONTROL_TERMINAL_SCOPE_ATOM_V1: &str = "BootstrapControlTerminalScopeAtomV1";
pub const SEVENTH_INTERACTION_HARD_EXCLUSION_V1: &str = "WithdrawBootstrapMandateInteractionEffect";

pub const fn bootstrap_target_census() -> [BootstrapControlTargetV1; 11] {
    [
        BootstrapControlTargetV1 {
            action: "EnrollRecoveryCommitmentSelection",
            disposition: BootstrapTargetDispositionV1::CandidateTarget,
        },
        BootstrapControlTargetV1 {
            action: "RotateRecoveryCommitmentSelection",
            disposition: BootstrapTargetDispositionV1::CandidateTarget,
        },
        BootstrapControlTargetV1 {
            action: "RevokeRecoveryCommitmentSelection",
            disposition: BootstrapTargetDispositionV1::CandidateTarget,
        },
        BootstrapControlTargetV1 {
            action: "FirstHumanBindingEnrollment",
            disposition: BootstrapTargetDispositionV1::HardExclusion,
        },
        BootstrapControlTargetV1 {
            action: "ReserveBootstrapMandateInteractionEffect",
            disposition: BootstrapTargetDispositionV1::HardExclusion,
        },
        BootstrapControlTargetV1 {
            action: "PublishBootstrapMandateInteractionEffectOutcome",
            disposition: BootstrapTargetDispositionV1::HardExclusion,
        },
        BootstrapControlTargetV1 {
            action: "PublishBootstrapMandatePresentationObservation",
            disposition: BootstrapTargetDispositionV1::HardExclusion,
        },
        BootstrapControlTargetV1 {
            action: "PublishBootstrapMandateResponseObservation",
            disposition: BootstrapTargetDispositionV1::HardExclusion,
        },
        BootstrapControlTargetV1 {
            action: "ReconcileBootstrapMandateInteractionEffect",
            disposition: BootstrapTargetDispositionV1::HardExclusion,
        },
        BootstrapControlTargetV1 {
            action: "IssueBootstrapMandate",
            disposition: BootstrapTargetDispositionV1::HardExclusion,
        },
        BootstrapControlTargetV1 {
            action: "WithdrawBootstrapMandateInteractionEffect",
            disposition: BootstrapTargetDispositionV1::HardExclusion,
        },
    ]
}
