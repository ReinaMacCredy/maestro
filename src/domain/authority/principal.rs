use std::collections::BTreeSet;

use super::grant::{AuthorityValidationError, GrantV1, HalfOpenValidityV1, ScopeAtomV1};
use super::identity::{
    AuthorityContextIdV1, GrantIdV1, MandateIdV1, PrincipalBindingIdV1, PrincipalIdV1, SessionIdV1,
};

const MAX_REQUEST_COMMITMENT_BYTES: usize = 256;
const MAX_REVOCATIONS: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrustedTimeV1 {
    Verified { lower_bound: u64, upper_bound: u64 },
    Unavailable,
}

impl TrustedTimeV1 {
    pub fn verified(lower_bound: u64, upper_bound: u64) -> Result<Self, AuthorityValidationError> {
        if lower_bound > upper_bound {
            return Err(AuthorityValidationError::InvalidTrustedTimeWindow);
        }
        Ok(Self::Verified {
            lower_bound,
            upper_bound,
        })
    }

    pub(crate) fn is_within(
        self,
        validity: HalfOpenValidityV1,
    ) -> Result<bool, AuthorityValidationError> {
        match self {
            Self::Verified {
                lower_bound,
                upper_bound,
            } => Ok(validity.contains(lower_bound) && validity.contains(upper_bound)),
            Self::Unavailable => Err(AuthorityValidationError::TrustedTimeUnavailable),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrincipalBindingV1 {
    id: PrincipalBindingIdV1,
    principal_id: PrincipalIdV1,
    context_id: AuthorityContextIdV1,
    trust_root_revision: u64,
    assurance_revision: u64,
    validity: HalfOpenValidityV1,
    human_capable: bool,
}

impl PrincipalBindingV1 {
    pub fn new(
        id: PrincipalBindingIdV1,
        principal_id: PrincipalIdV1,
        context_id: AuthorityContextIdV1,
        trust_root_revision: u64,
        assurance_revision: u64,
        validity: HalfOpenValidityV1,
        human_capable: bool,
    ) -> Result<Self, AuthorityValidationError> {
        if trust_root_revision == 0 || assurance_revision == 0 {
            return Err(AuthorityValidationError::ZeroProtocolRevision);
        }
        Ok(Self {
            id,
            principal_id,
            context_id,
            trust_root_revision,
            assurance_revision,
            validity,
            human_capable,
        })
    }

    pub const fn id(&self) -> PrincipalBindingIdV1 {
        self.id
    }

    pub const fn principal_id(&self) -> PrincipalIdV1 {
        self.principal_id
    }

    pub const fn context_id(&self) -> AuthorityContextIdV1 {
        self.context_id
    }

    pub const fn trust_root_revision(&self) -> u64 {
        self.trust_root_revision
    }

    pub const fn assurance_revision(&self) -> u64 {
        self.assurance_revision
    }

    pub const fn human_capable(&self) -> bool {
        self.human_capable
    }

    pub const fn validity(&self) -> HalfOpenValidityV1 {
        self.validity
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionV1 {
    id: SessionIdV1,
    binding_id: PrincipalBindingIdV1,
    context_id: AuthorityContextIdV1,
    store_generation: u64,
    authority_epoch: u64,
    request_commitment: String,
    validity: HalfOpenValidityV1,
}

impl SessionV1 {
    pub fn new(
        id: SessionIdV1,
        binding_id: PrincipalBindingIdV1,
        context_id: AuthorityContextIdV1,
        store_generation: u64,
        authority_epoch: u64,
        request_commitment: &str,
        validity: HalfOpenValidityV1,
    ) -> Result<Self, AuthorityValidationError> {
        if request_commitment.is_empty()
            || request_commitment.len() > MAX_REQUEST_COMMITMENT_BYTES
            || !request_commitment.is_ascii()
        {
            return Err(AuthorityValidationError::InvalidRequestCommitment);
        }
        Ok(Self {
            id,
            binding_id,
            context_id,
            store_generation,
            authority_epoch,
            request_commitment: request_commitment.to_owned(),
            validity,
        })
    }

    pub const fn id(&self) -> SessionIdV1 {
        self.id
    }

    pub const fn binding_id(&self) -> PrincipalBindingIdV1 {
        self.binding_id
    }

    pub const fn context_id(&self) -> AuthorityContextIdV1 {
        self.context_id
    }

    pub const fn store_generation(&self) -> u64 {
        self.store_generation
    }

    pub const fn authority_epoch(&self) -> u64 {
        self.authority_epoch
    }

    pub fn request_commitment(&self) -> &str {
        &self.request_commitment
    }

    pub const fn validity(&self) -> HalfOpenValidityV1 {
        self.validity
    }

    pub(crate) fn continue_at_store_generation(&self, store_generation: u64) -> Self {
        Self {
            id: self.id,
            binding_id: self.binding_id,
            context_id: self.context_id,
            store_generation,
            authority_epoch: self.authority_epoch,
            request_commitment: self.request_commitment.clone(),
            validity: self.validity,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RevocationTargetV1 {
    TrustRoot(u64),
    PrincipalBinding(PrincipalBindingIdV1),
    Session(SessionIdV1),
    Grant(GrantIdV1),
    Mandate(MandateIdV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevocationSetV1(BTreeSet<RevocationTargetV1>);

impl RevocationSetV1 {
    pub fn empty() -> Self {
        Self(BTreeSet::new())
    }

    pub fn new(targets: Vec<RevocationTargetV1>) -> Result<Self, AuthorityValidationError> {
        if targets.len() > MAX_REVOCATIONS {
            return Err(AuthorityValidationError::RevocationBoundExceeded);
        }
        Ok(Self(targets.into_iter().collect()))
    }

    pub fn contains(&self, target: RevocationTargetV1) -> bool {
        self.0.contains(&target)
    }

    pub fn targets(&self) -> impl ExactSizeIterator<Item = RevocationTargetV1> + '_ {
        self.0.iter().copied()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthoritySnapshotV1 {
    pub context_id: AuthorityContextIdV1,
    pub store_generation: u64,
    pub authority_epoch: u64,
    pub trust_root_revision: u64,
    pub subject_revision: u64,
    pub trusted_time: TrustedTimeV1,
}

impl AuthoritySnapshotV1 {
    pub const fn new(
        context_id: AuthorityContextIdV1,
        store_generation: u64,
        authority_epoch: u64,
        trust_root_revision: u64,
        subject_revision: u64,
        trusted_time: TrustedTimeV1,
    ) -> Self {
        Self {
            context_id,
            store_generation,
            authority_epoch,
            trust_root_revision,
            subject_revision,
            trusted_time,
        }
    }
}

pub fn validate_ordinary_authority(
    snapshot: &AuthoritySnapshotV1,
    binding: &PrincipalBindingV1,
    session: &SessionV1,
    grant: &GrantV1,
    required_atom: &ScopeAtomV1,
    revocations: &RevocationSetV1,
) -> Result<(), AuthorityValidationError> {
    if snapshot.context_id != binding.context_id
        || snapshot.context_id != session.context_id
        || snapshot.context_id != grant.context_id()
    {
        return Err(AuthorityValidationError::CrossContextAuthority);
    }
    if snapshot.store_generation != session.store_generation
        || snapshot.authority_epoch != session.authority_epoch
        || snapshot.trust_root_revision != binding.trust_root_revision
    {
        return Err(AuthorityValidationError::StaleAuthorityBasis);
    }
    if snapshot.subject_revision != required_atom.protocol_revision() {
        return Err(AuthorityValidationError::StaleSubjectRevision);
    }
    if binding.principal_id != grant.grantee_principal_id() || session.binding_id != binding.id {
        return Err(AuthorityValidationError::PrincipalSessionGrantMismatch);
    }
    if revocations.contains(RevocationTargetV1::TrustRoot(snapshot.trust_root_revision))
        || revocations.contains(RevocationTargetV1::PrincipalBinding(binding.id))
        || revocations.contains(RevocationTargetV1::Session(session.id))
        || revocations.contains(RevocationTargetV1::Grant(grant.id()))
    {
        return Err(AuthorityValidationError::Revoked);
    }
    if !snapshot.trusted_time.is_within(binding.validity)?
        || !snapshot.trusted_time.is_within(session.validity)?
        || !snapshot.trusted_time.is_within(grant.validity())?
    {
        return Err(AuthorityValidationError::ExpiredOrNotYetValid);
    }
    if !grant.terminal_scope().contains(required_atom) {
        return Err(AuthorityValidationError::MissingTerminalScope);
    }
    Ok(())
}
