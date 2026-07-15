use std::collections::BTreeSet;

use thiserror::Error;

use super::identity::{
    AuthorityContextIdV1, CapacityRootIdV1, DelegationIdV1, GenesisGrantIdV1, GrantIdV1,
    PrincipalIdV1,
};

const MAX_SCOPE_ATOMS: usize = 64;
const MAX_DELEGATION_ANCESTRY: usize = 64;
const MAX_SCOPE_TEXT_BYTES: usize = 256;
const MAX_G0_ROOT_CONTRIBUTIONS: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HalfOpenValidityV1 {
    not_before: u64,
    expires_at: u64,
}

impl HalfOpenValidityV1 {
    pub fn new(not_before: u64, expires_at: u64) -> Result<Self, AuthorityValidationError> {
        if not_before >= expires_at {
            return Err(AuthorityValidationError::InvalidValidityInterval);
        }
        Ok(Self {
            not_before,
            expires_at,
        })
    }

    pub const fn contains(self, trusted_time: u64) -> bool {
        self.not_before <= trusted_time && trusted_time < self.expires_at
    }

    pub const fn contains_interval(self, child: Self) -> bool {
        self.not_before <= child.not_before && child.expires_at <= self.expires_at
    }

    pub const fn not_before(self) -> u64 {
        self.not_before
    }

    pub const fn expires_at(self) -> u64 {
        self.expires_at
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ScopeAtomV1 {
    action: String,
    subject: String,
    protocol_revision: u64,
}

impl ScopeAtomV1 {
    pub fn new(
        action: &str,
        subject: &str,
        protocol_revision: u64,
    ) -> Result<Self, AuthorityValidationError> {
        if !valid_scope_text(action) || !valid_scope_text(subject) {
            return Err(AuthorityValidationError::InvalidScopeText);
        }
        if protocol_revision == 0 {
            return Err(AuthorityValidationError::ZeroProtocolRevision);
        }
        Ok(Self {
            action: action.to_owned(),
            subject: subject.to_owned(),
            protocol_revision,
        })
    }

    pub fn action(&self) -> &str {
        &self.action
    }

    pub fn subject(&self) -> &str {
        &self.subject
    }

    pub const fn protocol_revision(&self) -> u64 {
        self.protocol_revision
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrantScopeV1(BTreeSet<ScopeAtomV1>);

impl GrantScopeV1 {
    pub fn new(atoms: Vec<ScopeAtomV1>) -> Result<Self, AuthorityValidationError> {
        if atoms.len() > MAX_SCOPE_ATOMS {
            return Err(AuthorityValidationError::ScopeBoundExceeded);
        }
        Ok(Self(atoms.into_iter().collect()))
    }

    pub fn is_subset(&self, other: &Self) -> bool {
        self.0.is_subset(&other.0)
    }

    pub fn contains(&self, atom: &ScopeAtomV1) -> bool {
        self.0.contains(atom)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn is_disjoint(&self, other: &Self) -> bool {
        self.0.is_disjoint(&other.0)
    }

    pub fn atoms(&self) -> impl ExactSizeIterator<Item = &ScopeAtomV1> {
        self.0.iter()
    }

    fn union(&self, other: &Self) -> BTreeSet<ScopeAtomV1> {
        self.0.union(&other.0).cloned().collect()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthorityUseConstraintV1 {
    NoLocalBoundedRoot,
    BoundedBy(CapacityRootIdV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrantDefinitionV1 {
    pub id: GrantIdV1,
    pub context_id: AuthorityContextIdV1,
    pub grantee_principal_id: PrincipalIdV1,
    pub parent_grant_id: Option<GrantIdV1>,
    pub delegation_id: Option<DelegationIdV1>,
    pub terminal_scope: GrantScopeV1,
    pub delegable_scope: GrantScopeV1,
    pub validity: HalfOpenValidityV1,
    pub delegation_depth_remaining: u8,
    pub authority_use_constraint: AuthorityUseConstraintV1,
}

impl GrantDefinitionV1 {
    pub fn validate(self) -> Result<GrantV1, AuthorityValidationError> {
        if self.parent_grant_id.is_some() != self.delegation_id.is_some() {
            return Err(AuthorityValidationError::ParentDelegationMismatch);
        }
        if self.parent_grant_id == Some(self.id) {
            return Err(AuthorityValidationError::GrantCycle);
        }
        Ok(GrantV1(self))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrantV1(GrantDefinitionV1);

impl GrantV1 {
    pub const fn id(&self) -> GrantIdV1 {
        self.0.id
    }

    pub const fn context_id(&self) -> AuthorityContextIdV1 {
        self.0.context_id
    }

    pub const fn grantee_principal_id(&self) -> PrincipalIdV1 {
        self.0.grantee_principal_id
    }

    pub fn terminal_scope(&self) -> &GrantScopeV1 {
        &self.0.terminal_scope
    }

    pub fn delegable_scope(&self) -> &GrantScopeV1 {
        &self.0.delegable_scope
    }

    pub const fn authority_use_constraint(&self) -> AuthorityUseConstraintV1 {
        self.0.authority_use_constraint
    }

    pub const fn validity(&self) -> HalfOpenValidityV1 {
        self.0.validity
    }

    pub const fn parent_grant_id(&self) -> Option<GrantIdV1> {
        self.0.parent_grant_id
    }

    pub const fn delegation_id(&self) -> Option<DelegationIdV1> {
        self.0.delegation_id
    }

    pub const fn delegation_depth_remaining(&self) -> u8 {
        self.0.delegation_depth_remaining
    }

    pub fn definition(&self) -> GrantDefinitionV1 {
        self.0.clone()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapGenesisGrantV1 {
    genesis_grant_id: GenesisGrantIdV1,
    grant: GrantV1,
    authority_epoch: u64,
    trust_root_revision: u64,
}

impl BootstrapGenesisGrantV1 {
    pub fn new(
        genesis_grant_id: GenesisGrantIdV1,
        grant: GrantV1,
        authority_epoch: u64,
        trust_root_revision: u64,
    ) -> Result<Self, AuthorityValidationError> {
        let expected_genesis_id = GenesisGrantIdV1::derive(&grant.id().render())
            .map_err(|_| AuthorityValidationError::InvalidBootstrapGenesisGrant)?;
        if authority_epoch == 0 || trust_root_revision == 0 {
            return Err(AuthorityValidationError::ZeroProtocolRevision);
        }
        if genesis_grant_id != expected_genesis_id
            || grant.parent_grant_id().is_some()
            || grant.delegation_id().is_some()
            || grant.authority_use_constraint() != AuthorityUseConstraintV1::NoLocalBoundedRoot
            || !grant.terminal_scope().is_disjoint(grant.delegable_scope())
        {
            return Err(AuthorityValidationError::InvalidBootstrapGenesisGrant);
        }
        Ok(Self {
            genesis_grant_id,
            grant,
            authority_epoch,
            trust_root_revision,
        })
    }

    pub const fn id(&self) -> GenesisGrantIdV1 {
        self.genesis_grant_id
    }

    pub const fn grant(&self) -> &GrantV1 {
        &self.grant
    }

    pub const fn authority_epoch(&self) -> u64 {
        self.authority_epoch
    }

    pub const fn trust_root_revision(&self) -> u64 {
        self.trust_root_revision
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapG0PathV1 {
    genesis_grant: BootstrapGenesisGrantV1,
    store_generation: u64,
    complete: bool,
    root_contributions: Vec<CapacityRootIdV1>,
}

impl BootstrapG0PathV1 {
    pub fn new(
        genesis_grant_id: GenesisGrantIdV1,
        grant: GrantV1,
        store_generation: u64,
        authority_epoch: u64,
        trust_root_revision: u64,
        complete: bool,
        mut root_contributions: Vec<CapacityRootIdV1>,
    ) -> Result<Self, AuthorityValidationError> {
        if root_contributions.len() > MAX_G0_ROOT_CONTRIBUTIONS {
            return Err(AuthorityValidationError::G0RootContributionBoundExceeded);
        }
        root_contributions.sort_unstable();
        root_contributions.dedup();
        Ok(Self {
            genesis_grant: BootstrapGenesisGrantV1::new(
                genesis_grant_id,
                grant,
                authority_epoch,
                trust_root_revision,
            )?,
            store_generation,
            complete,
            root_contributions,
        })
    }

    pub fn from_genesis_grant(
        genesis_grant: BootstrapGenesisGrantV1,
        store_generation: u64,
        complete: bool,
        mut root_contributions: Vec<CapacityRootIdV1>,
    ) -> Result<Self, AuthorityValidationError> {
        if root_contributions.len() > MAX_G0_ROOT_CONTRIBUTIONS {
            return Err(AuthorityValidationError::G0RootContributionBoundExceeded);
        }
        root_contributions.sort_unstable();
        root_contributions.dedup();
        Ok(Self {
            genesis_grant,
            store_generation,
            complete,
            root_contributions,
        })
    }

    pub const fn genesis_grant(&self) -> &BootstrapGenesisGrantV1 {
        &self.genesis_grant
    }

    pub const fn genesis_grant_id(&self) -> GenesisGrantIdV1 {
        self.genesis_grant.id()
    }

    pub const fn grant(&self) -> &GrantV1 {
        self.genesis_grant.grant()
    }

    pub const fn store_generation(&self) -> u64 {
        self.store_generation
    }

    pub const fn authority_epoch(&self) -> u64 {
        self.genesis_grant.authority_epoch()
    }

    pub const fn trust_root_revision(&self) -> u64 {
        self.genesis_grant.trust_root_revision()
    }

    pub const fn complete(&self) -> bool {
        self.complete
    }

    pub fn root_contributions(&self) -> &[CapacityRootIdV1] {
        &self.root_contributions
    }

    pub(crate) fn continue_at_store_generation(&self, store_generation: u64) -> Self {
        Self {
            genesis_grant: self.genesis_grant.clone(),
            store_generation,
            complete: self.complete,
            root_contributions: self.root_contributions.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DelegationV1 {
    pub id: DelegationIdV1,
    pub parent_grant_id: GrantIdV1,
    pub child_grant_id: GrantIdV1,
}

impl DelegationV1 {
    pub const fn new(
        id: DelegationIdV1,
        parent_grant_id: GrantIdV1,
        child_grant_id: GrantIdV1,
    ) -> Self {
        Self {
            id,
            parent_grant_id,
            child_grant_id,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DelegationAncestryV1 {
    grant_ids: BTreeSet<GrantIdV1>,
    principal_ids: BTreeSet<PrincipalIdV1>,
    has_bounded_root: bool,
}

impl DelegationAncestryV1 {
    pub fn new(
        grant_ids: Vec<GrantIdV1>,
        principal_ids: Vec<PrincipalIdV1>,
        has_bounded_root: bool,
    ) -> Result<Self, AuthorityValidationError> {
        if grant_ids.len() > MAX_DELEGATION_ANCESTRY
            || principal_ids.len() > MAX_DELEGATION_ANCESTRY
        {
            return Err(AuthorityValidationError::AncestryBoundExceeded);
        }
        Ok(Self {
            grant_ids: grant_ids.into_iter().collect(),
            principal_ids: principal_ids.into_iter().collect(),
            has_bounded_root,
        })
    }
}

pub fn validate_delegation(
    parent: &GrantV1,
    child: &GrantV1,
    delegation: &DelegationV1,
    ancestry: &DelegationAncestryV1,
) -> Result<(), AuthorityValidationError> {
    if parent.context_id() != child.context_id() {
        return Err(AuthorityValidationError::CrossContextDelegation);
    }
    if child.0.parent_grant_id != Some(parent.id())
        || child.0.delegation_id != Some(delegation.id)
        || delegation.parent_grant_id != parent.id()
        || delegation.child_grant_id != child.id()
    {
        return Err(AuthorityValidationError::DelegationBindingMismatch);
    }
    if !ancestry.grant_ids.contains(&parent.id()) || ancestry.grant_ids.contains(&child.id()) {
        return Err(AuthorityValidationError::GrantCycle);
    }
    if ancestry
        .principal_ids
        .contains(&child.grantee_principal_id())
    {
        return Err(AuthorityValidationError::PrincipalAlreadyInAncestry);
    }
    let child_envelope = child.0.terminal_scope.union(&child.0.delegable_scope);
    if !child_envelope.is_subset(&parent.0.delegable_scope.0) {
        return Err(AuthorityValidationError::ScopeWidening);
    }
    if !parent.0.validity.contains_interval(child.0.validity) {
        return Err(AuthorityValidationError::ValidityWidening);
    }
    if child.0.delegation_depth_remaining >= parent.0.delegation_depth_remaining {
        return Err(AuthorityValidationError::DelegationDepthNotAttenuated);
    }
    if child.0.authority_use_constraint == AuthorityUseConstraintV1::NoLocalBoundedRoot
        && !ancestry.has_bounded_root
    {
        return Err(AuthorityValidationError::RootlessChildWithoutBoundedAncestry);
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AuthorityValidationError {
    #[error("validity must be a nonempty half-open interval")]
    InvalidValidityInterval,
    #[error("scope text must contain between 1 and 256 ASCII bytes")]
    InvalidScopeText,
    #[error("scope protocol revision must be nonzero")]
    ZeroProtocolRevision,
    #[error("Grant scope exceeds the finite atom bound")]
    ScopeBoundExceeded,
    #[error("Grant parent and Delegation must both be absent or both be present")]
    ParentDelegationMismatch,
    #[error("Grant ancestry is cyclic")]
    GrantCycle,
    #[error("delegation ancestry exceeds its finite bound")]
    AncestryBoundExceeded,
    #[error("delegation cannot cross Authority Contexts")]
    CrossContextDelegation,
    #[error("Delegation does not bind the exact parent and child Grant")]
    DelegationBindingMismatch,
    #[error("Delegation cannot target a Principal already in its ancestry")]
    PrincipalAlreadyInAncestry,
    #[error("child terminal plus delegable scope widens beyond the parent delegable scope")]
    ScopeWidening,
    #[error("child validity widens beyond parent validity")]
    ValidityWidening,
    #[error("child delegation depth must strictly attenuate")]
    DelegationDepthNotAttenuated,
    #[error("a rootless child requires an already bounded ancestry")]
    RootlessChildWithoutBoundedAncestry,
    #[error("trusted-time lower bound must not exceed its upper bound")]
    InvalidTrustedTimeWindow,
    #[error("trusted time is unavailable")]
    TrustedTimeUnavailable,
    #[error("Session request commitment must contain between 1 and 256 ASCII bytes")]
    InvalidRequestCommitment,
    #[error("revocation set exceeds its finite bound")]
    RevocationBoundExceeded,
    #[error("authority-bearing objects cannot cross Authority Contexts")]
    CrossContextAuthority,
    #[error("Authority basis is stale for the current Generation, Epoch, or Trust Root")]
    StaleAuthorityBasis,
    #[error("Action subject revision is stale")]
    StaleSubjectRevision,
    #[error("Principal Binding, Session, and Grant do not bind one exact Principal")]
    PrincipalSessionGrantMismatch,
    #[error("Authority basis contains a revoked object")]
    Revoked,
    #[error("Authority object is outside its half-open validity interval")]
    ExpiredOrNotYetValid,
    #[error("Grant TerminalScope does not contain the exact Action subject")]
    MissingTerminalScope,
    #[error("bootstrap G0 path exceeds the finite root-contribution bound")]
    G0RootContributionBoundExceeded,
    #[error("bootstrap genesis Grant must be rootless, parentless, and scope-disjoint")]
    InvalidBootstrapGenesisGrant,
}

fn valid_scope_text(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_SCOPE_TEXT_BYTES && value.is_ascii()
}
