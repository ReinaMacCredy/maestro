use thiserror::Error;

use super::effect_home::EffectIntentHomeKindV1;

pub const EFFECT_ORIGIN_COUNT_V1: usize = 23;
pub const EFFECT_ORIGIN_ROUTE_COUNT_V1: usize = 139;
pub const ACTION_EFFECT_BRANCH_COUNT_V1: usize = 19;
pub const CEREMONY_EFFECT_BRANCH_COUNT_V1: usize = 11;
pub const ACTION_EFFECT_ROUTE_COUNT_V1: usize = 95;
pub const CEREMONY_EFFECT_ROUTE_COUNT_V1: usize = 44;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DispatchReservationModeV1 {
    InitiateNew,
    RecoverReserved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CeremonyRequestModeV1 {
    Initiate,
    RecoverReserved,
    ResolveResult,
    Withdraw,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectOriginRouteRoleV1 {
    ActionReserve,
    ActionRecoverReserved,
    ActionOutcome,
    ActionReconcile,
    ActionWithdraw,
    CeremonyInitiate,
    CeremonyRecoverReserved,
    CeremonyResolveResult,
    CeremonyWithdraw,
}

impl EffectOriginRouteRoleV1 {
    pub const ALL: [Self; 9] = [
        Self::ActionReserve,
        Self::ActionRecoverReserved,
        Self::ActionOutcome,
        Self::ActionReconcile,
        Self::ActionWithdraw,
        Self::CeremonyInitiate,
        Self::CeremonyRecoverReserved,
        Self::CeremonyResolveResult,
        Self::CeremonyWithdraw,
    ];

    pub const fn is_action(self) -> bool {
        matches!(
            self,
            Self::ActionReserve
                | Self::ActionRecoverReserved
                | Self::ActionOutcome
                | Self::ActionReconcile
                | Self::ActionWithdraw
        )
    }

    pub const fn creates_no_effect_records(self) -> bool {
        matches!(self, Self::CeremonyResolveResult | Self::CeremonyWithdraw)
    }
}

pub struct EffectOriginHomeCompatibilityV1;

impl EffectOriginHomeCompatibilityV1 {
    pub const fn counts_are_exact() -> bool {
        EFFECT_ORIGIN_COUNT_V1 == 23
            && EFFECT_ORIGIN_ROUTE_COUNT_V1
                == ACTION_EFFECT_BRANCH_COUNT_V1 * 5 + CEREMONY_EFFECT_BRANCH_COUNT_V1 * 4
            && ACTION_EFFECT_ROUTE_COUNT_V1 == ACTION_EFFECT_BRANCH_COUNT_V1 * 5
            && CEREMONY_EFFECT_ROUTE_COUNT_V1 == CEREMONY_EFFECT_BRANCH_COUNT_V1 * 4
    }

    pub fn validate(
        role: EffectOriginRouteRoleV1,
        home: EffectIntentHomeKindV1,
        reservation_mode: Option<DispatchReservationModeV1>,
        ceremony_mode: Option<CeremonyRequestModeV1>,
        ceremony_branch_is_installation_context_genesis: bool,
    ) -> Result<(), RouteCompatibilityError> {
        if role.is_action() {
            if home != EffectIntentHomeKindV1::ActiveStore || ceremony_mode.is_some() {
                return Err(RouteCompatibilityError::ActionRouteNotActiveStore);
            }
            let expected = match role {
                EffectOriginRouteRoleV1::ActionReserve => {
                    Some(DispatchReservationModeV1::InitiateNew)
                }
                EffectOriginRouteRoleV1::ActionRecoverReserved => {
                    Some(DispatchReservationModeV1::RecoverReserved)
                }
                _ => None,
            };
            if reservation_mode != expected {
                return Err(RouteCompatibilityError::ReservationModeMismatch);
            }
            return Ok(());
        }
        if reservation_mode.is_some() {
            return Err(RouteCompatibilityError::CeremonyHasActionReservationMode);
        }
        let expected = match role {
            EffectOriginRouteRoleV1::CeremonyInitiate => CeremonyRequestModeV1::Initiate,
            EffectOriginRouteRoleV1::CeremonyRecoverReserved => {
                CeremonyRequestModeV1::RecoverReserved
            }
            EffectOriginRouteRoleV1::CeremonyResolveResult => CeremonyRequestModeV1::ResolveResult,
            EffectOriginRouteRoleV1::CeremonyWithdraw => CeremonyRequestModeV1::Withdraw,
            _ => return Err(RouteCompatibilityError::UnknownRole),
        };
        if ceremony_mode != Some(expected) {
            return Err(RouteCompatibilityError::CeremonyModeMismatch);
        }
        let expected_home = if ceremony_branch_is_installation_context_genesis {
            EffectIntentHomeKindV1::NoStoreCeremony
        } else {
            EffectIntentHomeKindV1::PreStoreCeremony
        };
        if home != expected_home {
            return Err(RouteCompatibilityError::CeremonyHomeMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RouteCompatibilityError {
    #[error("Action route must use ActiveStoreHomeV1 and no Ceremony mode")]
    ActionRouteNotActiveStore,
    #[error("Action route has the wrong DispatchReservationModeV1")]
    ReservationModeMismatch,
    #[error("Ceremony route may not carry an Action reservation mode")]
    CeremonyHasActionReservationMode,
    #[error("Ceremony route has the wrong CeremonyRequestModeV1")]
    CeremonyModeMismatch,
    #[error("Ceremony route Home does not match InstallationContextGenesis versus PreStore")]
    CeremonyHomeMismatch,
    #[error("route role is outside the frozen nine-member union")]
    UnknownRole,
}
