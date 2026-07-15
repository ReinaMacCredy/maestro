use thiserror::Error;

use super::effect_home::EffectIntentHomeKindV1;

pub const WITHDRAWN_LOCALLY_RENDERING_V1: &str =
    "withdrawn locally; no provider cancellation performed";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectIntentLiveDispatchV1 {
    None,
    Reserved,
    Sealed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RemoteClassificationV1 {
    Prepared,
    Dispatching,
    Pending,
    InDoubt,
    ConfirmedApplied,
    ConfirmedNotApplied,
    PartiallyApplied,
    Conflicted,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectWithdrawalSlotFamilyV1 {
    MaintenanceExecutorCurrentness,
    ProspectiveContinuityCarrier,
    PlannedTurnoverHighWater,
    RepositoryRecoveryAdmission,
    InstallationRecoveryAdmission,
}

impl EffectWithdrawalSlotFamilyV1 {
    pub const ALL: [Self; 5] = [
        Self::MaintenanceExecutorCurrentness,
        Self::ProspectiveContinuityCarrier,
        Self::PlannedTurnoverHighWater,
        Self::RepositoryRecoveryAdmission,
        Self::InstallationRecoveryAdmission,
    ];
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WithdrawalAuthorityPathV1 {
    Ordinary,
    BootstrapG0,
    ContinuityMaintenance(EffectWithdrawalSlotFamilyV1),
    Ceremony,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WithdrawalRequestV1 {
    pub home: EffectIntentHomeKindV1,
    pub path: WithdrawalAuthorityPathV1,
    pub live_dispatch: EffectIntentLiveDispatchV1,
    pub classification: RemoteClassificationV1,
    pub has_live_attempt: bool,
    pub has_dispatch_fence: bool,
    pub has_seal: bool,
    pub has_release_capability: bool,
    pub runs_closed: bool,
    pub same_home_current: bool,
    pub authority_current: bool,
    pub capacity_current: bool,
    pub expected_old_head: bool,
    pub expected_old_carrier: bool,
}

/// Validated literal shape. It deliberately provides no writer, I/O, or
/// mutation method; Stage 4 is the only future implementation owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WithdrawalLegalityV1 {
    pub next_live_dispatch: EffectIntentLiveDispatchV1,
    pub next_classification: RemoteClassificationV1,
    pub creates_intent: bool,
    pub creates_attempt: bool,
    pub creates_run: bool,
    pub performs_provider_io: bool,
    pub refunds_or_remints: bool,
}

pub fn validate_withdrawal(
    request: WithdrawalRequestV1,
) -> Result<WithdrawalLegalityV1, WithdrawalError> {
    if request.live_dispatch != EffectIntentLiveDispatchV1::None {
        return Err(WithdrawalError::LiveDispatch);
    }
    if !matches!(
        request.classification,
        RemoteClassificationV1::Prepared | RemoteClassificationV1::ConfirmedNotApplied
    ) {
        return Err(WithdrawalError::Classification);
    }
    if request.has_live_attempt
        || request.has_dispatch_fence
        || request.has_seal
        || request.has_release_capability
    {
        return Err(WithdrawalError::LiveAttemptFenceSealOrCapability);
    }
    if !request.runs_closed {
        return Err(WithdrawalError::OpenRuns);
    }
    if !request.same_home_current || !request.authority_current || !request.capacity_current {
        return Err(WithdrawalError::StaleHomeAuthorityOrCapacity);
    }
    if !request.expected_old_head || !request.expected_old_carrier {
        return Err(WithdrawalError::ExpectedOldMismatch);
    }
    match (request.home, request.path) {
        (
            EffectIntentHomeKindV1::ActiveStore,
            WithdrawalAuthorityPathV1::Ordinary
            | WithdrawalAuthorityPathV1::BootstrapG0
            | WithdrawalAuthorityPathV1::ContinuityMaintenance(_),
        ) => {}
        (
            EffectIntentHomeKindV1::NoStoreCeremony | EffectIntentHomeKindV1::PreStoreCeremony,
            WithdrawalAuthorityPathV1::Ceremony,
        ) => {}
        _ => return Err(WithdrawalError::CrossHomeBasisDonation),
    }
    Ok(WithdrawalLegalityV1 {
        next_live_dispatch: EffectIntentLiveDispatchV1::None,
        next_classification: RemoteClassificationV1::Cancelled,
        creates_intent: false,
        creates_attempt: false,
        creates_run: false,
        performs_provider_io: false,
        refunds_or_remints: false,
    })
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum WithdrawalError {
    #[error("withdrawal requires None live dispatch")]
    LiveDispatch,
    #[error("withdrawal requires prepared or confirmed_not_applied")]
    Classification,
    #[error("withdrawal may not have a live Attempt, fence, seal, or release capability")]
    LiveAttemptFenceSealOrCapability,
    #[error("withdrawal requires a closed Run set")]
    OpenRuns,
    #[error("withdrawal requires current same-Home authority and capacity")]
    StaleHomeAuthorityOrCapacity,
    #[error("withdrawal requires the exact expected-old Head and carrier")]
    ExpectedOldMismatch,
    #[error(
        "withdrawal may not donate an ordinary, Bootstrap, CMA, or Ceremony basis across Homes"
    )]
    CrossHomeBasisDonation,
}
