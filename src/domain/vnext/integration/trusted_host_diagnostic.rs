#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Stage 5 freezes the sealed diagnostic port without a production producer"
    )
)]

#[cfg(test)]
use crate::domain::vnext::persistence::ProtectedDiagnosticCurrentViewAnchorV1;
use sha2::{Digest, Sha256};

const CHALLENGE_DOMAIN_V1: &[u8] = b"maestro.vnext.trusted-host-diagnostic-challenge.v1";
const ATTESTATION_DOMAIN_V1: &[u8] = b"maestro.vnext.trusted-host-diagnostic-attestation.v1";

pub(crate) struct TrustedHostDiagnosticChallengeV1<'view> {
    #[cfg(test)]
    current_view_anchor: &'view ProtectedDiagnosticCurrentViewAnchorV1<'view>,
    #[cfg(not(test))]
    current_view_anchor: &'view (),
    anchor_commitment: [u8; 32],
    authority_commitment: [u8; 32],
    protected_subject_commitment: [u8; 32],
    invocation_nonce: [u8; 32],
    commitment: [u8; 32],
}

impl<'view> TrustedHostDiagnosticChallengeV1<'view> {
    #[cfg(test)]
    fn from_authority_issuance(
        current_view_anchor: &'view ProtectedDiagnosticCurrentViewAnchorV1<'view>,
        authority_commitment: [u8; 32],
        protected_subject_commitment: [u8; 32],
        invocation_nonce: [u8; 32],
    ) -> Option<Self> {
        let anchor_commitment = current_view_anchor.commitment();
        if [
            anchor_commitment,
            authority_commitment,
            protected_subject_commitment,
            invocation_nonce,
        ]
        .contains(&[0; 32])
        {
            return None;
        }
        let commitment = tuple_commitment(
            CHALLENGE_DOMAIN_V1,
            &[
                &anchor_commitment,
                &authority_commitment,
                &protected_subject_commitment,
                &invocation_nonce,
            ],
        );
        Some(Self {
            current_view_anchor,
            anchor_commitment,
            authority_commitment,
            protected_subject_commitment,
            invocation_nonce,
            commitment,
        })
    }
}

#[cfg(test)]
struct TrustedHostDiagnosticTestStateV1 {
    claims: TrustedHostDiagnosticTestClaimsV1,
    connected: bool,
    authentication_event_consumed: bool,
    issued_invocation_nonce: Option<[u8; 32]>,
    host_currentness_revision: u64,
    revocation_revision: u64,
    incarnation_revision: u64,
    final_recheck_mutation: Option<TrustedHostDiagnosticTestFinalRecheckMutationV1>,
}

#[cfg(test)]
enum TrustedHostDiagnosticTestFinalRecheckMutationV1 {
    Disconnect,
    Revocation,
    Currentness,
    Incarnation,
    ClaimDimension(usize),
    OperatorIdentityDimension(usize),
}

#[cfg(test)]
pub(crate) struct TrustedHostDiagnosticTestOperatorIdentityV1 {
    principal_identity: [u8; 32],
    binding_identity: [u8; 32],
    session_identity: [u8; 32],
    context_identity: [u8; 32],
    trust_root_revision: u64,
    assurance_revision: u64,
    human_capable: bool,
    binding_not_before: u64,
    binding_expires_at: u64,
    session_not_before: u64,
    session_expires_at: u64,
    store_generation: u64,
    authority_epoch: u64,
    domain_identity: [u8; 32],
    domain_role: u64,
}

#[cfg(test)]
impl TrustedHostDiagnosticTestOperatorIdentityV1 {
    #[expect(
        clippy::too_many_arguments,
        reason = "the test host identity must independently carry every Authority join dimension"
    )]
    pub(crate) const fn authenticated_human(
        principal_identity: [u8; 32],
        binding_identity: [u8; 32],
        session_identity: [u8; 32],
        context_identity: [u8; 32],
        trust_root_revision: u64,
        assurance_revision: u64,
        binding_not_before: u64,
        binding_expires_at: u64,
        session_not_before: u64,
        session_expires_at: u64,
        store_generation: u64,
        authority_epoch: u64,
        domain_identity: [u8; 32],
        domain_role: u64,
    ) -> Self {
        Self {
            principal_identity,
            binding_identity,
            session_identity,
            context_identity,
            trust_root_revision,
            assurance_revision,
            human_capable: true,
            binding_not_before,
            binding_expires_at,
            session_not_before,
            session_expires_at,
            store_generation,
            authority_epoch,
            domain_identity,
            domain_role,
        }
    }

    fn is_complete(&self) -> bool {
        ![
            self.principal_identity,
            self.binding_identity,
            self.session_identity,
            self.context_identity,
            self.domain_identity,
        ]
        .contains(&[0; 32])
            && self.trust_root_revision != 0
            && self.assurance_revision != 0
            && self.human_capable
            && self.binding_not_before < self.binding_expires_at
            && self.session_not_before < self.session_expires_at
            && self.store_generation != 0
            && self.authority_epoch != 0
            && self.domain_role != 0
    }

    fn substitute_dimension(&mut self, dimension: usize) {
        match dimension {
            0 => mutate_digest(&mut self.principal_identity),
            1 => mutate_digest(&mut self.binding_identity),
            2 => mutate_digest(&mut self.session_identity),
            3 => mutate_digest(&mut self.context_identity),
            4 => self.trust_root_revision = self.trust_root_revision.saturating_add(1),
            5 => self.assurance_revision = self.assurance_revision.saturating_add(1),
            6 => self.human_capable = false,
            7 => self.binding_not_before = self.binding_not_before.saturating_add(1),
            8 => self.binding_expires_at = self.binding_expires_at.saturating_add(1),
            9 => self.session_not_before = self.session_not_before.saturating_add(1),
            10 => self.session_expires_at = self.session_expires_at.saturating_add(1),
            11 => self.store_generation = self.store_generation.saturating_add(1),
            12 => self.authority_epoch = self.authority_epoch.saturating_add(1),
            13 => mutate_digest(&mut self.domain_identity),
            14 => self.domain_role = self.domain_role.saturating_add(1),
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
#[derive(Clone)]
pub(crate) struct TrustedHostDiagnosticTestControlV1(
    std::rc::Rc<std::cell::RefCell<TrustedHostDiagnosticTestStateV1>>,
);

#[cfg(test)]
impl TrustedHostDiagnosticTestControlV1 {
    pub(crate) fn disconnect_on_final_recheck(&self) {
        self.0.borrow_mut().final_recheck_mutation =
            Some(TrustedHostDiagnosticTestFinalRecheckMutationV1::Disconnect);
    }

    pub(crate) fn revoke_on_final_recheck(&self) {
        self.0.borrow_mut().final_recheck_mutation =
            Some(TrustedHostDiagnosticTestFinalRecheckMutationV1::Revocation);
    }

    pub(crate) fn advance_currentness_on_final_recheck(&self) {
        self.0.borrow_mut().final_recheck_mutation =
            Some(TrustedHostDiagnosticTestFinalRecheckMutationV1::Currentness);
    }

    pub(crate) fn replace_incarnation_on_final_recheck(&self) {
        self.0.borrow_mut().final_recheck_mutation =
            Some(TrustedHostDiagnosticTestFinalRecheckMutationV1::Incarnation);
    }

    pub(crate) fn substitute_claim_dimension_on_final_recheck(&self, dimension: usize) {
        self.0.borrow_mut().final_recheck_mutation =
            Some(TrustedHostDiagnosticTestFinalRecheckMutationV1::ClaimDimension(dimension));
    }

    pub(crate) fn substitute_operator_identity_dimension_on_final_recheck(&self, dimension: usize) {
        self.0.borrow_mut().final_recheck_mutation = Some(
            TrustedHostDiagnosticTestFinalRecheckMutationV1::OperatorIdentityDimension(dimension),
        );
    }

    pub(crate) fn substitute_operator_identity_dimension(&self, dimension: usize) {
        self.0
            .borrow_mut()
            .claims
            .operator_identity
            .substitute_dimension(dimension);
    }

    pub(crate) fn invocation_is_pending(&self) -> bool {
        self.0.borrow().issued_invocation_nonce.is_some()
    }
}

#[cfg(test)]
pub(crate) struct TrustedHostDiagnosticTestClaimsV1 {
    provider_identity: [u8; 32],
    profile_identity: [u8; 32],
    profile_revision: u64,
    process_incarnation: [u8; 32],
    connection_incarnation: [u8; 32],
    channel_incarnation: [u8; 32],
    issuer_identity: [u8; 32],
    realm_identity: [u8; 32],
    audience_identity: [u8; 32],
    authentication_event_identity: [u8; 32],
    freshness_identity: [u8; 32],
    carrier_commitment: [u8; 32],
    operator_identity: TrustedHostDiagnosticTestOperatorIdentityV1,
}

#[cfg(test)]
impl TrustedHostDiagnosticTestClaimsV1 {
    pub(crate) fn from_seed(
        seed: &[u8],
        operator_identity: TrustedHostDiagnosticTestOperatorIdentityV1,
    ) -> Option<Self> {
        if seed.is_empty() || !operator_identity.is_complete() {
            return None;
        }
        let derive = |label: &[u8]| tuple_commitment(label, &[seed]);
        Some(Self {
            provider_identity: derive(b"provider"),
            profile_identity: derive(b"profile"),
            profile_revision: 1,
            process_incarnation: derive(b"process"),
            connection_incarnation: derive(b"connection"),
            channel_incarnation: derive(b"channel"),
            issuer_identity: derive(b"issuer"),
            realm_identity: derive(b"realm"),
            audience_identity: derive(b"audience"),
            authentication_event_identity: derive(b"authentication-event"),
            freshness_identity: derive(b"freshness"),
            carrier_commitment: derive(b"carrier"),
            operator_identity,
        })
    }
}

#[cfg(test)]
pub(crate) struct TrustedHostDiagnosticTestConnectionV1 {
    state: std::rc::Rc<std::cell::RefCell<TrustedHostDiagnosticTestStateV1>>,
}

#[cfg(test)]
impl TrustedHostDiagnosticTestConnectionV1 {
    pub(crate) fn authenticated(claims: TrustedHostDiagnosticTestClaimsV1) -> Self {
        Self {
            state: std::rc::Rc::new(std::cell::RefCell::new(TrustedHostDiagnosticTestStateV1 {
                claims,
                connected: true,
                authentication_event_consumed: false,
                issued_invocation_nonce: None,
                host_currentness_revision: 1,
                revocation_revision: 1,
                incarnation_revision: 1,
                final_recheck_mutation: None,
            })),
        }
    }

    pub(crate) fn control(&self) -> TrustedHostDiagnosticTestControlV1 {
        TrustedHostDiagnosticTestControlV1(std::rc::Rc::clone(&self.state))
    }

    pub(crate) fn attest_in_current_view<'connection, 'view>(
        &'connection mut self,
        current_view_anchor: &'view ProtectedDiagnosticCurrentViewAnchorV1<'view>,
        authority_commitment: [u8; 32],
        protected_subject_commitment: [u8; 32],
        invocation_nonce: [u8; 32],
    ) -> Option<TrustedHostDiagnosticAttestationV1<'connection, 'view>> {
        let challenge = match TrustedHostDiagnosticChallengeV1::from_authority_issuance(
            current_view_anchor,
            authority_commitment,
            protected_subject_commitment,
            invocation_nonce,
        ) {
            Some(challenge) => challenge,
            None => {
                let mut state = self.state.borrow_mut();
                state.authentication_event_consumed = true;
                state.issued_invocation_nonce = None;
                return None;
            }
        };
        let current_view_anchor_commitment = challenge.current_view_anchor.commitment();
        let mut state = self.state.borrow_mut();
        if !state.connected
            || state.authentication_event_consumed
            || state.issued_invocation_nonce.is_some()
            || challenge.protected_subject_commitment == [0; 32]
            || current_view_anchor_commitment == [0; 32]
            || challenge.authority_commitment == [0; 32]
            || challenge.anchor_commitment != current_view_anchor_commitment
        {
            state.authentication_event_consumed = true;
            state.issued_invocation_nonce = None;
            return None;
        }
        state.issued_invocation_nonce = Some(challenge.invocation_nonce);
        if !state.connected
            || state.authentication_event_consumed
            || state.issued_invocation_nonce != Some(challenge.invocation_nonce)
        {
            state.authentication_event_consumed = true;
            state.issued_invocation_nonce = None;
            return None;
        }
        state.authentication_event_consumed = true;
        state.issued_invocation_nonce = None;
        let host_currentness_revision = state.host_currentness_revision;
        let revocation_revision = state.revocation_revision;
        let incarnation_revision = state.incarnation_revision;
        let claims_commitment = claims_commitment(
            &state.claims,
            challenge.commitment,
            host_currentness_revision,
            revocation_revision,
            incarnation_revision,
        );
        drop(state);
        Some(TrustedHostDiagnosticAttestationV1 {
            connection: self,
            _current_view_anchor: challenge.current_view_anchor,
            anchor_commitment: challenge.anchor_commitment,
            authority_commitment: challenge.authority_commitment,
            protected_subject_commitment: challenge.protected_subject_commitment,
            invocation_nonce: challenge.invocation_nonce,
            challenge_commitment: challenge.commitment,
            claims_commitment,
            host_currentness_revision,
            revocation_revision,
            incarnation_revision,
            consumed: false,
        })
    }
}

pub(crate) struct TrustedHostDiagnosticAttestationV1<'connection, 'view> {
    #[cfg(test)]
    connection: &'connection mut TrustedHostDiagnosticTestConnectionV1,
    #[cfg(not(test))]
    connection: &'connection mut (),
    #[cfg(test)]
    _current_view_anchor: &'view ProtectedDiagnosticCurrentViewAnchorV1<'view>,
    #[cfg(not(test))]
    _current_view_anchor: &'view (),
    anchor_commitment: [u8; 32],
    authority_commitment: [u8; 32],
    protected_subject_commitment: [u8; 32],
    invocation_nonce: [u8; 32],
    challenge_commitment: [u8; 32],
    claims_commitment: [u8; 32],
    host_currentness_revision: u64,
    revocation_revision: u64,
    incarnation_revision: u64,
    consumed: bool,
}

impl TrustedHostDiagnosticAttestationV1<'_, '_> {
    pub(crate) const fn witness_carrier_commitment(&self) -> [u8; 32] {
        self.claims_commitment
    }

    #[cfg(test)]
    #[expect(
        clippy::too_many_arguments,
        reason = "Authority must equality-join every independent host identity dimension"
    )]
    pub(crate) fn matches_operator(
        &self,
        principal_identity: [u8; 32],
        binding_identity: [u8; 32],
        session_identity: [u8; 32],
        context_identity: [u8; 32],
        trust_root_revision: u64,
        assurance_revision: u64,
        human_capable: bool,
        binding_not_before: u64,
        binding_expires_at: u64,
        session_not_before: u64,
        session_expires_at: u64,
        store_generation: u64,
        authority_epoch: u64,
        domain_identity: [u8; 32],
        domain_role: u64,
    ) -> bool {
        let state = self.connection.state.borrow();
        let identity = &state.claims.operator_identity;
        identity.principal_identity == principal_identity
            && identity.binding_identity == binding_identity
            && identity.session_identity == session_identity
            && identity.context_identity == context_identity
            && identity.trust_root_revision == trust_root_revision
            && identity.assurance_revision == assurance_revision
            && identity.human_capable == human_capable
            && identity.binding_not_before == binding_not_before
            && identity.binding_expires_at == binding_expires_at
            && identity.session_not_before == session_not_before
            && identity.session_expires_at == session_expires_at
            && identity.store_generation == store_generation
            && identity.authority_epoch == authority_epoch
            && identity.domain_identity == domain_identity
            && identity.domain_role == domain_role
    }

    pub(crate) fn binds(
        &self,
        anchor_commitment: [u8; 32],
        authority_commitment: [u8; 32],
        protected_subject_commitment: [u8; 32],
    ) -> bool {
        self.anchor_commitment == anchor_commitment
            && self.authority_commitment == authority_commitment
            && self.protected_subject_commitment == protected_subject_commitment
            && self.challenge_commitment
                == tuple_commitment(
                    CHALLENGE_DOMAIN_V1,
                    &[
                        &anchor_commitment,
                        &authority_commitment,
                        &protected_subject_commitment,
                        &self.invocation_nonce,
                    ],
                )
            && self.claims_commitment != [0; 32]
    }

    #[cfg(test)]
    pub(crate) fn final_recheck(mut self) -> bool {
        let mut state = self.connection.state.borrow_mut();
        match state.final_recheck_mutation.take() {
            Some(TrustedHostDiagnosticTestFinalRecheckMutationV1::Disconnect) => {
                state.connected = false
            }
            Some(TrustedHostDiagnosticTestFinalRecheckMutationV1::Revocation) => {
                state.revocation_revision = state.revocation_revision.saturating_add(1)
            }
            Some(TrustedHostDiagnosticTestFinalRecheckMutationV1::Currentness) => {
                state.host_currentness_revision = state.host_currentness_revision.saturating_add(1)
            }
            Some(TrustedHostDiagnosticTestFinalRecheckMutationV1::Incarnation) => {
                state.incarnation_revision = state.incarnation_revision.saturating_add(1)
            }
            Some(TrustedHostDiagnosticTestFinalRecheckMutationV1::ClaimDimension(dimension)) => {
                let claims = &mut state.claims;
                match dimension {
                    0 => mutate_digest(&mut claims.provider_identity),
                    1 => mutate_digest(&mut claims.profile_identity),
                    2 => claims.profile_revision = claims.profile_revision.saturating_add(1),
                    3 => mutate_digest(&mut claims.process_incarnation),
                    4 => mutate_digest(&mut claims.connection_incarnation),
                    5 => mutate_digest(&mut claims.channel_incarnation),
                    6 => mutate_digest(&mut claims.issuer_identity),
                    7 => mutate_digest(&mut claims.realm_identity),
                    8 => mutate_digest(&mut claims.audience_identity),
                    9 => mutate_digest(&mut claims.authentication_event_identity),
                    10 => mutate_digest(&mut claims.freshness_identity),
                    11 => mutate_digest(&mut claims.carrier_commitment),
                    _ => unreachable!(),
                }
            }
            Some(TrustedHostDiagnosticTestFinalRecheckMutationV1::OperatorIdentityDimension(
                dimension,
            )) => state
                .claims
                .operator_identity
                .substitute_dimension(dimension),
            None => {}
        }
        let valid = state.connected
            && state.authentication_event_consumed
            && state.host_currentness_revision == self.host_currentness_revision
            && state.revocation_revision == self.revocation_revision
            && state.incarnation_revision == self.incarnation_revision
            && claims_commitment(
                &state.claims,
                self.challenge_commitment,
                self.host_currentness_revision,
                self.revocation_revision,
                self.incarnation_revision,
            ) == self.claims_commitment;
        drop(state);
        self.consumed = true;
        valid
    }
}

impl Drop for TrustedHostDiagnosticAttestationV1<'_, '_> {
    fn drop(&mut self) {
        self.consumed = true;
    }
}

#[cfg(test)]
fn mutate_digest(value: &mut [u8; 32]) {
    value[0] ^= 0xff;
}

#[cfg(test)]
fn claims_commitment(
    claims: &TrustedHostDiagnosticTestClaimsV1,
    challenge_commitment: [u8; 32],
    host_currentness_revision: u64,
    revocation_revision: u64,
    incarnation_revision: u64,
) -> [u8; 32] {
    let revision = claims.profile_revision.to_be_bytes();
    let currentness = host_currentness_revision.to_be_bytes();
    let revocation = revocation_revision.to_be_bytes();
    let incarnation = incarnation_revision.to_be_bytes();
    tuple_commitment(
        ATTESTATION_DOMAIN_V1,
        &[
            &challenge_commitment,
            &claims.provider_identity,
            &claims.profile_identity,
            &revision,
            &claims.process_incarnation,
            &claims.connection_incarnation,
            &claims.channel_incarnation,
            &claims.issuer_identity,
            &claims.realm_identity,
            &claims.audience_identity,
            &claims.authentication_event_identity,
            &currentness,
            &revocation,
            &claims.freshness_identity,
            &claims.carrier_commitment,
            &claims.operator_identity.principal_identity,
            &claims.operator_identity.binding_identity,
            &claims.operator_identity.session_identity,
            &claims.operator_identity.context_identity,
            &claims.operator_identity.trust_root_revision.to_be_bytes(),
            &claims.operator_identity.assurance_revision.to_be_bytes(),
            &[u8::from(claims.operator_identity.human_capable)],
            &claims.operator_identity.binding_not_before.to_be_bytes(),
            &claims.operator_identity.binding_expires_at.to_be_bytes(),
            &claims.operator_identity.session_not_before.to_be_bytes(),
            &claims.operator_identity.session_expires_at.to_be_bytes(),
            &claims.operator_identity.store_generation.to_be_bytes(),
            &claims.operator_identity.authority_epoch.to_be_bytes(),
            &claims.operator_identity.domain_identity,
            &claims.operator_identity.domain_role.to_be_bytes(),
            &incarnation,
        ],
    )
}

fn tuple_commitment(domain: &[u8], fields: &[&[u8]]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update((domain.len() as u64).to_be_bytes());
    digest.update(domain);
    for field in fields {
        digest.update((field.len() as u64).to_be_bytes());
        digest.update(field);
    }
    digest.finalize().into()
}
