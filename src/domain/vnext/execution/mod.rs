//! Candidate-only Stage-0 Execution literals.
//!
//! These values freeze schema and refusal boundaries. They do not register a
//! runtime handler, perform I/O, or mutate a Store.

pub mod bootstrap;
pub mod control_head;
pub mod dispatch_state;
pub mod effect_home;
pub mod effect_routes;
pub mod withdrawal;

pub use bootstrap::{
    BootstrapControlTargetV1, BootstrapTargetDispositionV1, bootstrap_target_census,
};
pub use control_head::{
    EffectIntentControlConsumerDispositionV1, EffectIntentControlHeadV1,
    EffectIntentControlReadWriteCohortDescriptorV1, EffectIntentControlRevisionV1,
    EffectIntentControlTokenV1, EffectIntentControlTransitionContenderV1,
    EffectIntentControlTransitionV1, EffectIntentControlWriterTermKindV1,
    EffectIntentControlWriterTermV1,
};
pub use effect_home::{
    ActiveStoreHomeV1, EffectIntentDomainKindV1, EffectIntentHomeError, EffectIntentHomeKindV1,
    EffectIntentHomeV1, EffectIntentOriginationFenceV1, EffectIntentUseFenceV1, HomeTokenV1,
    NoStoreCeremonyHomeV1, PreStoreCeremonyHomeV1,
};
pub use effect_routes::{
    CeremonyRequestModeV1, DispatchReservationModeV1, EffectOriginHomeCompatibilityV1,
    EffectOriginRouteRoleV1, RouteCompatibilityError,
};
pub use withdrawal::{
    EffectIntentLiveDispatchV1, EffectWithdrawalSlotFamilyV1, RemoteClassificationV1,
    WITHDRAWN_LOCALLY_RENDERING_V1, WithdrawalAuthorityPathV1, WithdrawalError,
    WithdrawalLegalityV1, WithdrawalRequestV1, validate_withdrawal,
};
