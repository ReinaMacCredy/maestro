use std::collections::BTreeSet;

use thiserror::Error;

use crate::domain::identity::SchemaIdV1;
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::identity::{
    AuthorityContextIdV1, CapacityRootIdV1, DelegationIdV1, GenesisGrantIdV1, GrantIdV1,
    PrincipalIdV1,
};
use super::principal::{RevocationSetV1, RevocationTargetV1};

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
        if matches!(
            self.authority_use_constraint,
            AuthorityUseConstraintV1::BoundedBy(_)
        ) && self.parent_grant_id.is_none()
        {
            return Err(AuthorityValidationError::ParentlessBoundedGrant);
        }
        if self.parent_grant_id == Some(self.id) {
            return Err(AuthorityValidationError::GrantCycle);
        }
        Ok(GrantV1(self))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrdinaryBoundedGrantV1(GrantV1);

impl OrdinaryBoundedGrantV1 {
    pub const SCHEMA_DOMAIN: &'static str = "maestro.vnext.ordinary-bounded-grant.v1";
    pub const STORE_SCHEMA_ID: &'static str =
        "sha256:93a4227d4bd0e1d4204a87dc92fb688838b649c1bffe6b1fd87cfdc5287d50c7";

    pub fn new(grant: GrantV1) -> Result<Self, AuthorityValidationError> {
        if grant.parent_grant_id().is_none()
            || grant.delegation_id().is_none()
            || !matches!(
                grant.authority_use_constraint(),
                AuthorityUseConstraintV1::BoundedBy(_)
            )
        {
            return Err(AuthorityValidationError::InvalidOrdinaryBoundedGrant);
        }
        Ok(Self(grant))
    }

    pub fn store_schema_id() -> Result<SchemaIdV1, crate::domain::identity::IdentityError> {
        SchemaIdV1::parse(Self::STORE_SCHEMA_ID)
    }

    pub const fn grant(&self) -> &GrantV1 {
        &self.0
    }

    pub fn capacity_root_id(&self) -> CapacityRootIdV1 {
        match self.0.authority_use_constraint() {
            AuthorityUseConstraintV1::BoundedBy(root_id) => root_id,
            AuthorityUseConstraintV1::NoLocalBoundedRoot => {
                unreachable!("invariant: ordinary bounded Grant constructor enforces BoundedBy")
            }
        }
    }

    pub fn schema_value(&self) -> Result<CborValue, CborError> {
        grant_schema_value(Self::SCHEMA_DOMAIN, &self.0)
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CborError> {
        deterministic_cbor::encode(&self.schema_value()?)
    }

    pub fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, AuthorityValidationError> {
        let value = deterministic_cbor::decode(bytes)?;
        let grant = parse_grant_schema_value(&value, Self::SCHEMA_DOMAIN)?;
        let ordinary = Self::new(grant)?;
        if ordinary.canonical_bytes()? != bytes {
            return Err(AuthorityValidationError::InvalidCanonicalGrantCarrier);
        }
        Ok(ordinary)
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
pub struct OrdinaryGrantDelegationV1 {
    context_id: AuthorityContextIdV1,
    capacity_root_id: CapacityRootIdV1,
    delegation: DelegationV1,
}

impl OrdinaryGrantDelegationV1 {
    pub const SCHEMA_DOMAIN: &'static str = "maestro.vnext.ordinary-grant-delegation.v1";
    pub const STORE_SCHEMA_ID: &'static str =
        "sha256:5f22c4aabdb6b8bf0a4f3f25c2dac5d693be43f8becb243f7cb73e9a028a29ac";

    pub fn new(
        context_id: AuthorityContextIdV1,
        capacity_root_id: CapacityRootIdV1,
        delegation: DelegationV1,
        child: &OrdinaryBoundedGrantV1,
    ) -> Result<Self, AuthorityValidationError> {
        if child.grant().context_id() != context_id
            || child.capacity_root_id() != capacity_root_id
            || child.grant().parent_grant_id() != Some(delegation.parent_grant_id)
            || child.grant().delegation_id() != Some(delegation.id)
            || child.grant().id() != delegation.child_grant_id
        {
            return Err(AuthorityValidationError::DelegationBindingMismatch);
        }
        Ok(Self {
            context_id,
            capacity_root_id,
            delegation,
        })
    }

    pub fn store_schema_id() -> Result<SchemaIdV1, crate::domain::identity::IdentityError> {
        SchemaIdV1::parse(Self::STORE_SCHEMA_ID)
    }

    pub const fn context_id(&self) -> AuthorityContextIdV1 {
        self.context_id
    }

    pub const fn capacity_root_id(&self) -> CapacityRootIdV1 {
        self.capacity_root_id
    }

    pub const fn delegation(&self) -> DelegationV1 {
        self.delegation
    }

    pub fn schema_value(&self) -> Result<CborValue, CborError> {
        Ok(CborValue::Array(vec![
            CborValue::text(Self::SCHEMA_DOMAIN)?,
            bytes(self.delegation.id.as_bytes()),
            bytes(self.context_id.as_bytes()),
            bytes(self.delegation.parent_grant_id.as_bytes()),
            bytes(self.delegation.child_grant_id.as_bytes()),
            bytes(self.capacity_root_id.as_bytes()),
        ]))
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CborError> {
        deterministic_cbor::encode(&self.schema_value()?)
    }

    pub fn from_canonical_bytes(
        bytes: &[u8],
        child: &OrdinaryBoundedGrantV1,
    ) -> Result<Self, AuthorityValidationError> {
        let value = deterministic_cbor::decode(bytes)?;
        let fields = exact_array(&value, 6)?;
        require_domain(&fields[0], Self::SCHEMA_DOMAIN)?;
        let carrier = Self::new(
            authority_id(&fields[2])?,
            authority_id(&fields[5])?,
            DelegationV1::new(
                authority_id(&fields[1])?,
                authority_id(&fields[3])?,
                authority_id(&fields[4])?,
            ),
            child,
        )?;
        if carrier.canonical_bytes()? != bytes {
            return Err(AuthorityValidationError::InvalidCanonicalGrantCarrier);
        }
        Ok(carrier)
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

pub fn grant_is_revoked_by_closure(
    grant: &OrdinaryBoundedGrantV1,
    grants: &[OrdinaryBoundedGrantV1],
    revocations: &RevocationSetV1,
) -> Result<bool, AuthorityValidationError> {
    let mut current = Some(grant.grant().id());
    let mut visited = BTreeSet::new();
    while let Some(grant_id) = current {
        if !visited.insert(grant_id) {
            return Err(AuthorityValidationError::GrantCycle);
        }
        if revocations.contains(RevocationTargetV1::Grant(grant_id)) {
            return Ok(true);
        }
        current = grants
            .iter()
            .find(|candidate| candidate.grant().id() == grant_id)
            .and_then(|candidate| candidate.grant().parent_grant_id());
        if visited.len() > MAX_DELEGATION_ANCESTRY {
            return Err(AuthorityValidationError::AncestryBoundExceeded);
        }
    }
    Ok(false)
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AuthorityValidationError {
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
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
    #[error("a BoundedBy Grant must have exactly one parent and Delegation")]
    ParentlessBoundedGrant,
    #[error("ordinary Grant carrier must be parented, delegated, and BoundedBy one root")]
    InvalidOrdinaryBoundedGrant,
    #[error("ordinary Grant or Delegation carrier is not canonical")]
    InvalidCanonicalGrantCarrier,
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

fn grant_schema_value(domain: &str, grant: &GrantV1) -> Result<CborValue, CborError> {
    let definition = grant.definition();
    Ok(CborValue::Array(vec![
        CborValue::text(domain)?,
        bytes(definition.id.as_bytes()),
        bytes(definition.context_id.as_bytes()),
        bytes(definition.grantee_principal_id.as_bytes()),
        CborValue::optional(definition.parent_grant_id.map(|id| bytes(id.as_bytes()))),
        CborValue::optional(definition.delegation_id.map(|id| bytes(id.as_bytes()))),
        scope_value(&definition.terminal_scope)?,
        scope_value(&definition.delegable_scope)?,
        CborValue::Array(vec![
            CborValue::Unsigned(definition.validity.not_before()),
            CborValue::Unsigned(definition.validity.expires_at()),
        ]),
        CborValue::Unsigned(u64::from(definition.delegation_depth_remaining)),
        match definition.authority_use_constraint {
            AuthorityUseConstraintV1::NoLocalBoundedRoot => {
                CborValue::Array(vec![CborValue::Unsigned(1)])
            }
            AuthorityUseConstraintV1::BoundedBy(root_id) => {
                CborValue::Array(vec![CborValue::Unsigned(2), bytes(root_id.as_bytes())])
            }
        },
    ]))
}

fn parse_grant_schema_value(
    value: &CborValue,
    domain: &str,
) -> Result<GrantV1, AuthorityValidationError> {
    let fields = exact_array(value, 11)?;
    require_domain(&fields[0], domain)?;
    let validity = exact_array(&fields[8], 2)?;
    let constraint = match exact_array_any(&fields[10])? {
        [CborValue::Unsigned(1)] => AuthorityUseConstraintV1::NoLocalBoundedRoot,
        [CborValue::Unsigned(2), root] => AuthorityUseConstraintV1::BoundedBy(authority_id(root)?),
        _ => return Err(AuthorityValidationError::InvalidCanonicalGrantCarrier),
    };
    GrantDefinitionV1 {
        id: authority_id(&fields[1])?,
        context_id: authority_id(&fields[2])?,
        grantee_principal_id: authority_id(&fields[3])?,
        parent_grant_id: optional_authority_id(&fields[4])?,
        delegation_id: optional_authority_id(&fields[5])?,
        terminal_scope: parse_scope(&fields[6])?,
        delegable_scope: parse_scope(&fields[7])?,
        validity: HalfOpenValidityV1::new(unsigned(&validity[0])?, unsigned(&validity[1])?)?,
        delegation_depth_remaining: u8::try_from(unsigned(&fields[9])?)
            .map_err(|_| AuthorityValidationError::InvalidCanonicalGrantCarrier)?,
        authority_use_constraint: constraint,
    }
    .validate()
}

fn scope_value(scope: &GrantScopeV1) -> Result<CborValue, CborError> {
    Ok(CborValue::Array(
        scope
            .atoms()
            .map(|atom| {
                Ok(CborValue::Array(vec![
                    CborValue::text(atom.action())?,
                    CborValue::text(atom.subject())?,
                    CborValue::Unsigned(atom.protocol_revision()),
                ]))
            })
            .collect::<Result<Vec<_>, CborError>>()?,
    ))
}

fn parse_scope(value: &CborValue) -> Result<GrantScopeV1, AuthorityValidationError> {
    let CborValue::Array(atoms) = value else {
        return Err(AuthorityValidationError::InvalidCanonicalGrantCarrier);
    };
    GrantScopeV1::new(
        atoms
            .iter()
            .map(|value| {
                let fields = exact_array(value, 3)?;
                ScopeAtomV1::new(text(&fields[0])?, text(&fields[1])?, unsigned(&fields[2])?)
            })
            .collect::<Result<Vec<_>, AuthorityValidationError>>()?,
    )
}

fn exact_array(
    value: &CborValue,
    expected: usize,
) -> Result<&[CborValue], AuthorityValidationError> {
    let fields = exact_array_any(value)?;
    if fields.len() != expected {
        return Err(AuthorityValidationError::InvalidCanonicalGrantCarrier);
    }
    Ok(fields)
}

fn exact_array_any(value: &CborValue) -> Result<&[CborValue], AuthorityValidationError> {
    let CborValue::Array(fields) = value else {
        return Err(AuthorityValidationError::InvalidCanonicalGrantCarrier);
    };
    Ok(fields)
}

fn require_domain(value: &CborValue, expected: &str) -> Result<(), AuthorityValidationError> {
    if matches!(value, CborValue::Text(actual) if actual == expected) {
        Ok(())
    } else {
        Err(AuthorityValidationError::InvalidCanonicalGrantCarrier)
    }
}

fn authority_id<K: super::identity::AuthorityIdentityKindV1>(
    value: &CborValue,
) -> Result<super::identity::AuthorityIdV1<K>, AuthorityValidationError> {
    let CborValue::Bytes(bytes) = value else {
        return Err(AuthorityValidationError::InvalidCanonicalGrantCarrier);
    };
    let digest: [u8; 32] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| AuthorityValidationError::InvalidCanonicalGrantCarrier)?;
    Ok(super::identity::AuthorityIdV1::from_digest(digest))
}

fn optional_authority_id<K: super::identity::AuthorityIdentityKindV1>(
    value: &CborValue,
) -> Result<Option<super::identity::AuthorityIdV1<K>>, AuthorityValidationError> {
    match exact_array_any(value)? {
        [CborValue::Unsigned(0)] => Ok(None),
        [CborValue::Unsigned(1), value] => Ok(Some(authority_id(value)?)),
        _ => Err(AuthorityValidationError::InvalidCanonicalGrantCarrier),
    }
}

fn unsigned(value: &CborValue) -> Result<u64, AuthorityValidationError> {
    match value {
        CborValue::Unsigned(value) => Ok(*value),
        _ => Err(AuthorityValidationError::InvalidCanonicalGrantCarrier),
    }
}

fn text(value: &CborValue) -> Result<&str, AuthorityValidationError> {
    match value {
        CborValue::Text(value) => Ok(value),
        _ => Err(AuthorityValidationError::InvalidCanonicalGrantCarrier),
    }
}

fn bytes(value: &[u8]) -> CborValue {
    CborValue::Bytes(value.to_vec())
}

fn valid_scope_text(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_SCOPE_TEXT_BYTES && value.is_ascii()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ordinary_grant(
        seed: &str,
        parent: GrantIdV1,
        root: CapacityRootIdV1,
    ) -> (OrdinaryBoundedGrantV1, OrdinaryGrantDelegationV1) {
        let id = GrantIdV1::derive(&format!("{seed}-grant")).unwrap();
        let delegation_id = DelegationIdV1::derive(&format!("{seed}-delegation")).unwrap();
        let grant = GrantDefinitionV1 {
            id,
            context_id: AuthorityContextIdV1::derive("context").unwrap(),
            grantee_principal_id: PrincipalIdV1::derive(&format!("{seed}-principal")).unwrap(),
            parent_grant_id: Some(parent),
            delegation_id: Some(delegation_id),
            terminal_scope: GrantScopeV1::new(vec![
                ScopeAtomV1::new("RevokeGrant", &root.render(), 1).unwrap(),
            ])
            .unwrap(),
            delegable_scope: GrantScopeV1::new(vec![]).unwrap(),
            validity: HalfOpenValidityV1::new(1, 10).unwrap(),
            delegation_depth_remaining: 2,
            authority_use_constraint: AuthorityUseConstraintV1::BoundedBy(root),
        }
        .validate()
        .unwrap();
        let grant = OrdinaryBoundedGrantV1::new(grant).unwrap();
        let delegation = OrdinaryGrantDelegationV1::new(
            grant.grant().context_id(),
            root,
            DelegationV1::new(delegation_id, parent, id),
            &grant,
        )
        .unwrap();
        (grant, delegation)
    }

    #[test]
    fn ordinary_carriers_round_trip_and_parentless_bounded_grant_is_refused() {
        let parent = GrantIdV1::derive("parent").unwrap();
        let root = CapacityRootIdV1::derive("root").unwrap();
        let (grant, delegation) = ordinary_grant("child", parent, root);
        let grant_bytes = grant.canonical_bytes().unwrap();
        let decoded = OrdinaryBoundedGrantV1::from_canonical_bytes(&grant_bytes).unwrap();
        assert_eq!(decoded, grant);
        let delegation_bytes = delegation.canonical_bytes().unwrap();
        assert_eq!(
            OrdinaryGrantDelegationV1::from_canonical_bytes(&delegation_bytes, &decoded).unwrap(),
            delegation
        );
        assert_eq!(
            OrdinaryBoundedGrantV1::store_schema_id().unwrap().render(),
            OrdinaryBoundedGrantV1::STORE_SCHEMA_ID
        );

        let mut invalid = grant.grant().definition();
        invalid.parent_grant_id = None;
        invalid.delegation_id = None;
        assert_eq!(
            invalid.validate(),
            Err(AuthorityValidationError::ParentlessBoundedGrant)
        );
    }

    #[test]
    fn ancestor_revocation_invalidates_descendant_grant_closure() {
        let root = CapacityRootIdV1::derive("root").unwrap();
        let parent_id = GrantIdV1::derive("g0").unwrap();
        let (parent, _) = ordinary_grant("parent", parent_id, root);
        let (child, _) = ordinary_grant("child", parent.grant().id(), root);
        let revocations =
            RevocationSetV1::new(vec![RevocationTargetV1::Grant(parent.grant().id())]).unwrap();
        assert!(
            grant_is_revoked_by_closure(&child, &[parent, child.clone()], &revocations).unwrap()
        );
    }
}
