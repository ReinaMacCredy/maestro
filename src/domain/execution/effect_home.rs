use std::fmt;

use thiserror::Error;

/// An opaque immutable reference used by the Stage-0 literal schema.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HomeTokenV1([u8; 32]);

impl fmt::Debug for HomeTokenV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("HomeTokenV1(<redacted>)")
    }
}

impl HomeTokenV1 {
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectIntentHomeKindV1 {
    ActiveStore,
    NoStoreCeremony,
    PreStoreCeremony,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectIntentDomainKindV1 {
    RepositoryDomain,
    InstallationDomain,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActiveStoreHomeV1 {
    pub domain_kind: EffectIntentDomainKindV1,
    pub stable_domain_id: HomeTokenV1,
    pub realm: HomeTokenV1,
    pub semantic_namespace: HomeTokenV1,
    pub home_qualified_semantic_uniqueness_namespace: HomeTokenV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NoStoreCeremonyHomeV1 {
    pub protected_installation_realm: HomeTokenV1,
    pub locator_candidate_branch: HomeTokenV1,
    pub installation_context_genesis_ceremony: HomeTokenV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreStoreCeremonyHomeV1 {
    pub allowed_pre_store_ceremony: HomeTokenV1,
    pub destination_domain_kind: EffectIntentDomainKindV1,
    pub candidate_branch_or_destination: HomeTokenV1,
    pub inactive_destination_lineage: HomeTokenV1,
}

/// Stable identity for one Effect Intent. No-store deliberately has no domain,
/// generation, or epoch field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectIntentHomeV1 {
    ActiveStore(ActiveStoreHomeV1),
    NoStoreCeremony(NoStoreCeremonyHomeV1),
    PreStoreCeremony(PreStoreCeremonyHomeV1),
}

impl EffectIntentHomeV1 {
    pub const fn kind(&self) -> EffectIntentHomeKindV1 {
        match self {
            Self::ActiveStore(_) => EffectIntentHomeKindV1::ActiveStore,
            Self::NoStoreCeremony(_) => EffectIntentHomeKindV1::NoStoreCeremony,
            Self::PreStoreCeremony(_) => EffectIntentHomeKindV1::PreStoreCeremony,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActiveStoreOriginationFenceV1 {
    pub store: HomeTokenV1,
    pub generation: HomeTokenV1,
    pub epoch: HomeTokenV1,
    pub namespace: HomeTokenV1,
    pub material_token: HomeTokenV1,
    pub action_request: HomeTokenV1,
    pub action_authority_basis: HomeTokenV1,
    pub receipt: HomeTokenV1,
    pub result: HomeTokenV1,
    pub effect_origin: HomeTokenV1,
    pub current_authority_commitment: HomeTokenV1,
    pub credential_commitment: HomeTokenV1,
    pub dispatch_reservation_or_fence: HomeTokenV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NoStoreCeremonyOriginationFenceV1 {
    pub ceremony_spec: HomeTokenV1,
    pub ceremony_manifest: HomeTokenV1,
    pub initiate_mode: HomeTokenV1,
    pub sealed_ceremony_attempt_commitment: HomeTokenV1,
    pub attempt_id: HomeTokenV1,
    pub protected_realm: HomeTokenV1,
    pub locator_candidate_bundle: HomeTokenV1,
    pub carrier_identity: HomeTokenV1,
    pub carrier_incarnation: HomeTokenV1,
    pub expected_old_token: HomeTokenV1,
    pub candidate_seal: HomeTokenV1,
    pub external_anchor: HomeTokenV1,
    pub idempotency_identity: HomeTokenV1,
    pub dispatch_fence: HomeTokenV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreStoreCeremonyOriginationFenceV1 {
    pub ceremony_spec: HomeTokenV1,
    pub ceremony_manifest: HomeTokenV1,
    pub initiate_mode: HomeTokenV1,
    pub sealed_ceremony_attempt_commitment: HomeTokenV1,
    pub attempt_id: HomeTokenV1,
    pub branch_bundle: HomeTokenV1,
    pub inactive_destination: HomeTokenV1,
    pub candidate_seal: HomeTokenV1,
    pub carrier_identity: HomeTokenV1,
    pub carrier_incarnation: HomeTokenV1,
    pub expected_old_token: HomeTokenV1,
    pub external_basis: HomeTokenV1,
    pub idempotency_identity: HomeTokenV1,
    pub dispatch_fence: HomeTokenV1,
}

/// Immutable fence committed when the Intent originates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectIntentOriginationFenceV1 {
    ActiveStore(ActiveStoreOriginationFenceV1),
    NoStoreCeremony(NoStoreCeremonyOriginationFenceV1),
    PreStoreCeremony(PreStoreCeremonyOriginationFenceV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActiveStoreUseFenceV1 {
    pub same_stable_home: HomeTokenV1,
    pub generation: HomeTokenV1,
    pub epoch: HomeTokenV1,
    pub namespace: HomeTokenV1,
    pub material_token: HomeTokenV1,
    pub authority: HomeTokenV1,
    pub credentials: HomeTokenV1,
    pub attempt_fence: HomeTokenV1,
    pub idempotency_binding: HomeTokenV1,
    pub provider_contract_guards: HomeTokenV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NoStoreCeremonyUseFenceV1 {
    pub same_home: HomeTokenV1,
    pub branch_authority: HomeTokenV1,
    pub carrier_incarnation: HomeTokenV1,
    pub expected_old_token: HomeTokenV1,
    pub attempt_id: HomeTokenV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreStoreCeremonyUseFenceV1 {
    pub same_home: HomeTokenV1,
    pub branch_authority: HomeTokenV1,
    pub carrier: HomeTokenV1,
    pub expected_old_token: HomeTokenV1,
    pub attempt_id: HomeTokenV1,
}

/// A fresh fence for every later DispatchAttempt or Action reconciliation use.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectIntentUseFenceV1 {
    ActiveStore(ActiveStoreUseFenceV1),
    NoStoreCeremony(NoStoreCeremonyUseFenceV1),
    PreStoreCeremony(PreStoreCeremonyUseFenceV1),
}

impl EffectIntentOriginationFenceV1 {
    pub const fn home_kind(&self) -> EffectIntentHomeKindV1 {
        match self {
            Self::ActiveStore(_) => EffectIntentHomeKindV1::ActiveStore,
            Self::NoStoreCeremony(_) => EffectIntentHomeKindV1::NoStoreCeremony,
            Self::PreStoreCeremony(_) => EffectIntentHomeKindV1::PreStoreCeremony,
        }
    }
}

impl EffectIntentUseFenceV1 {
    pub const fn home_kind(&self) -> EffectIntentHomeKindV1 {
        match self {
            Self::ActiveStore(_) => EffectIntentHomeKindV1::ActiveStore,
            Self::NoStoreCeremony(_) => EffectIntentHomeKindV1::NoStoreCeremony,
            Self::PreStoreCeremony(_) => EffectIntentHomeKindV1::PreStoreCeremony,
        }
    }
}

pub fn validate_fence_home(
    home: EffectIntentHomeV1,
    origination: EffectIntentOriginationFenceV1,
    use_fence: EffectIntentUseFenceV1,
) -> Result<(), EffectIntentHomeError> {
    let kind = home.kind();
    if origination.home_kind() != kind {
        return Err(EffectIntentHomeError::OriginationFenceHomeMismatch);
    }
    if use_fence.home_kind() != kind {
        return Err(EffectIntentHomeError::UseFenceHomeMismatch);
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum EffectIntentHomeError {
    #[error("Effect Intent origination fence is not for the immutable Home")]
    OriginationFenceHomeMismatch,
    #[error("Effect Intent use fence is not fresh same-home material")]
    UseFenceHomeMismatch,
}
