use super::closed::AuthorityTagError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BootstrapTargetDispositionV1 {
    Admitted,
    Excluded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum BootstrapMandateTargetV1 {
    EnrollRecoveryCommitmentSelection = 1,
    RotateRecoveryCommitmentSelection = 2,
    RevokeRecoveryCommitmentSelection = 3,
    FirstHumanBindingEnrollment = 4,
    ReserveBootstrapMandateInteractionEffect = 5,
    PublishBootstrapMandateInteractionOutcome = 6,
    PublishBootstrapMandatePresentationObservation = 7,
    PublishBootstrapMandateResponseObservation = 8,
    ReconcileBootstrapMandateInteractionEffect = 9,
    IssueBootstrapMandate = 10,
    WithdrawBootstrapMandateInteractionEffect = 11,
}

impl BootstrapMandateTargetV1 {
    pub const ALL: [Self; 11] = [
        Self::EnrollRecoveryCommitmentSelection,
        Self::RotateRecoveryCommitmentSelection,
        Self::RevokeRecoveryCommitmentSelection,
        Self::FirstHumanBindingEnrollment,
        Self::ReserveBootstrapMandateInteractionEffect,
        Self::PublishBootstrapMandateInteractionOutcome,
        Self::PublishBootstrapMandatePresentationObservation,
        Self::PublishBootstrapMandateResponseObservation,
        Self::ReconcileBootstrapMandateInteractionEffect,
        Self::IssueBootstrapMandate,
        Self::WithdrawBootstrapMandateInteractionEffect,
    ];

    pub const fn disposition(self) -> BootstrapTargetDispositionV1 {
        match self {
            Self::EnrollRecoveryCommitmentSelection
            | Self::RotateRecoveryCommitmentSelection
            | Self::RevokeRecoveryCommitmentSelection => BootstrapTargetDispositionV1::Admitted,
            _ => BootstrapTargetDispositionV1::Excluded,
        }
    }

    pub const fn action_name(self) -> &'static str {
        match self {
            Self::EnrollRecoveryCommitmentSelection => "EnrollRecoveryCommitmentSelection",
            Self::RotateRecoveryCommitmentSelection => "RotateRecoveryCommitmentSelection",
            Self::RevokeRecoveryCommitmentSelection => "RevokeRecoveryCommitmentSelection",
            Self::FirstHumanBindingEnrollment => "FirstHumanBindingEnrollment",
            Self::ReserveBootstrapMandateInteractionEffect => {
                "ReserveBootstrapMandateInteractionEffect"
            }
            Self::PublishBootstrapMandateInteractionOutcome => {
                "PublishBootstrapMandateInteractionOutcome"
            }
            Self::PublishBootstrapMandatePresentationObservation => {
                "PublishBootstrapMandatePresentationObservation"
            }
            Self::PublishBootstrapMandateResponseObservation => {
                "PublishBootstrapMandateResponseObservation"
            }
            Self::ReconcileBootstrapMandateInteractionEffect => {
                "ReconcileBootstrapMandateInteractionEffect"
            }
            Self::IssueBootstrapMandate => "IssueBootstrapMandate",
            Self::WithdrawBootstrapMandateInteractionEffect => {
                "WithdrawBootstrapMandateInteractionEffect"
            }
        }
    }
}

impl TryFrom<u8> for BootstrapMandateTargetV1 {
    type Error = AuthorityTagError;

    fn try_from(tag: u8) -> Result<Self, Self::Error> {
        match tag {
            1 => Ok(Self::EnrollRecoveryCommitmentSelection),
            2 => Ok(Self::RotateRecoveryCommitmentSelection),
            3 => Ok(Self::RevokeRecoveryCommitmentSelection),
            4 => Ok(Self::FirstHumanBindingEnrollment),
            5 => Ok(Self::ReserveBootstrapMandateInteractionEffect),
            6 => Ok(Self::PublishBootstrapMandateInteractionOutcome),
            7 => Ok(Self::PublishBootstrapMandatePresentationObservation),
            8 => Ok(Self::PublishBootstrapMandateResponseObservation),
            9 => Ok(Self::ReconcileBootstrapMandateInteractionEffect),
            10 => Ok(Self::IssueBootstrapMandate),
            11 => Ok(Self::WithdrawBootstrapMandateInteractionEffect),
            value => Err(AuthorityTagError::UnknownBootstrapTarget(value)),
        }
    }
}

pub const fn bootstrap_mandate_target_catalog() -> [BootstrapMandateTargetV1; 11] {
    BootstrapMandateTargetV1::ALL
}
