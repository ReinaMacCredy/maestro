#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Stage 5 freezes the sealed diagnostic port without a production producer"
    )
)]

use crate::domain::vnext::authority::TrustedHostDiagnosticChallengeV1;
#[cfg(test)]
use crate::domain::vnext::persistence::ProtectedDiagnosticCurrentViewAnchorV1;
use sha2::{Digest, Sha256};

const ATTESTATION_DOMAIN_V1: &[u8] = b"maestro.vnext.trusted-host-diagnostic-attestation.v1";

pub(super) mod sealed {
    pub(crate) trait Connection {}
    pub(crate) trait Attestation {}
    pub(crate) trait Presentation {}
}

pub(crate) use sealed::{
    Attestation as TrustedHostDiagnosticAttestationPortSealedV1,
    Connection as TrustedHostDiagnosticConnectionPortSealedV1,
    Presentation as TrustedHostDiagnosticPresentationPortSealedV1,
};

pub(crate) trait TrustedHostDiagnosticConnectionPortV1: sealed::Connection {
    fn attest_in_current_view<'scope, 'view>(
        &'scope mut self,
        challenge: TrustedHostDiagnosticChallengeV1<'scope, 'view>,
    ) -> Option<Box<dyn TrustedHostDiagnosticAttestationPortV1 + 'scope>>
    where
        'view: 'scope;
}

pub(crate) trait TrustedHostDiagnosticAttestationPortV1: sealed::Attestation {
    fn witness_carrier_commitment(&self) -> [u8; 32];

    fn present_once(
        &mut self,
        inspect: &mut dyn FnMut(&dyn TrustedHostDiagnosticPresentationPortV1) -> bool,
    ) -> bool;

    fn final_recheck(self: Box<Self>) -> bool;
}

pub(crate) trait TrustedHostDiagnosticPresentationPortV1: sealed::Presentation {
    fn anchor_commitment(&self) -> [u8; 32];
    fn authority_commitment(&self) -> [u8; 32];
    fn protected_subject_commitment(&self) -> [u8; 32];
    fn invocation_nonce(&self) -> [u8; 32];
    fn challenge_commitment(&self) -> [u8; 32];
    fn claims_commitment(&self) -> [u8; 32];
    fn principal_identity(&self) -> [u8; 32];
    fn binding_identity(&self) -> [u8; 32];
    fn session_identity(&self) -> [u8; 32];
    fn context_identity(&self) -> [u8; 32];
    fn trust_root_revision(&self) -> u64;
    fn assurance_revision(&self) -> u64;
    fn human_capable(&self) -> bool;
    fn binding_not_before(&self) -> u64;
    fn binding_expires_at(&self) -> u64;
    fn session_not_before(&self) -> u64;
    fn session_expires_at(&self) -> u64;
    fn store_generation(&self) -> u64;
    fn authority_epoch(&self) -> u64;
    fn domain_identity(&self) -> [u8; 32];
    fn domain_role(&self) -> u64;
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

    fn bind_to_attestation(&self) -> Self {
        Self {
            principal_identity: self.principal_identity,
            binding_identity: self.binding_identity,
            session_identity: self.session_identity,
            context_identity: self.context_identity,
            trust_root_revision: self.trust_root_revision,
            assurance_revision: self.assurance_revision,
            human_capable: self.human_capable,
            binding_not_before: self.binding_not_before,
            binding_expires_at: self.binding_expires_at,
            session_not_before: self.session_not_before,
            session_expires_at: self.session_expires_at,
            store_generation: self.store_generation,
            authority_epoch: self.authority_epoch,
            domain_identity: self.domain_identity,
            domain_role: self.domain_role,
        }
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

    pub(crate) fn attest_in_current_view<'connection, 'anchor, 'view>(
        &'connection mut self,
        challenge: TrustedHostDiagnosticChallengeV1<'anchor, 'view>,
    ) -> Option<TrustedHostDiagnosticAttestationV1<'connection, 'anchor, 'view>> {
        let mut state = self.state.borrow_mut();
        if !state.connected
            || state.authentication_event_consumed
            || state.issued_invocation_nonce.is_some()
        {
            state.authentication_event_consumed = true;
            state.issued_invocation_nonce = None;
            return None;
        }
        state.issued_invocation_nonce = Some(challenge.invocation_nonce());
        if !state.connected
            || state.authentication_event_consumed
            || state.issued_invocation_nonce != Some(challenge.invocation_nonce())
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
        let operator_identity = state.claims.operator_identity.bind_to_attestation();
        let claims_commitment = claims_commitment(
            &state.claims,
            challenge.commitment(),
            host_currentness_revision,
            revocation_revision,
            incarnation_revision,
        );
        drop(state);
        Some(TrustedHostDiagnosticAttestationV1 {
            connection: self,
            _current_view_anchor: challenge.current_view_anchor(),
            anchor_commitment: challenge.anchor_commitment(),
            authority_commitment: challenge.authority_commitment(),
            protected_subject_commitment: challenge.protected_subject_commitment(),
            invocation_nonce: challenge.invocation_nonce(),
            challenge_commitment: challenge.commitment(),
            claims_commitment,
            operator_identity,
            operator_presented: false,
            host_currentness_revision,
            revocation_revision,
            incarnation_revision,
            consumed: false,
        })
    }
}

#[cfg(test)]
impl sealed::Connection for TrustedHostDiagnosticTestConnectionV1 {}

#[cfg(test)]
impl TrustedHostDiagnosticConnectionPortV1 for TrustedHostDiagnosticTestConnectionV1 {
    fn attest_in_current_view<'scope, 'view>(
        &'scope mut self,
        challenge: TrustedHostDiagnosticChallengeV1<'scope, 'view>,
    ) -> Option<Box<dyn TrustedHostDiagnosticAttestationPortV1 + 'scope>>
    where
        'view: 'scope,
    {
        TrustedHostDiagnosticTestConnectionV1::attest_in_current_view(self, challenge)
            .map(|attestation| Box::new(attestation) as Box<_>)
    }
}

#[cfg(test)]
pub(crate) struct TrustedHostDiagnosticAttestationV1<'connection, 'anchor, 'view> {
    connection: &'connection mut TrustedHostDiagnosticTestConnectionV1,
    _current_view_anchor: &'anchor ProtectedDiagnosticCurrentViewAnchorV1<'view>,
    anchor_commitment: [u8; 32],
    authority_commitment: [u8; 32],
    protected_subject_commitment: [u8; 32],
    invocation_nonce: [u8; 32],
    challenge_commitment: [u8; 32],
    claims_commitment: [u8; 32],
    operator_identity: TrustedHostDiagnosticTestOperatorIdentityV1,
    operator_presented: bool,
    host_currentness_revision: u64,
    revocation_revision: u64,
    incarnation_revision: u64,
    consumed: bool,
}

#[cfg(test)]
pub(crate) struct TrustedHostDiagnosticPresentationV1<'presentation, 'connection, 'anchor, 'view> {
    attestation: &'presentation TrustedHostDiagnosticAttestationV1<'connection, 'anchor, 'view>,
}

#[cfg(test)]
impl TrustedHostDiagnosticPresentationV1<'_, '_, '_, '_> {
    pub(crate) const fn anchor_commitment(&self) -> [u8; 32] {
        self.attestation.anchor_commitment
    }

    pub(crate) const fn authority_commitment(&self) -> [u8; 32] {
        self.attestation.authority_commitment
    }

    pub(crate) const fn protected_subject_commitment(&self) -> [u8; 32] {
        self.attestation.protected_subject_commitment
    }

    pub(crate) const fn invocation_nonce(&self) -> [u8; 32] {
        self.attestation.invocation_nonce
    }

    pub(crate) const fn challenge_commitment(&self) -> [u8; 32] {
        self.attestation.challenge_commitment
    }

    pub(crate) const fn claims_commitment(&self) -> [u8; 32] {
        self.attestation.claims_commitment
    }

    pub(crate) const fn principal_identity(&self) -> [u8; 32] {
        self.attestation.operator_identity.principal_identity
    }

    pub(crate) const fn binding_identity(&self) -> [u8; 32] {
        self.attestation.operator_identity.binding_identity
    }

    pub(crate) const fn session_identity(&self) -> [u8; 32] {
        self.attestation.operator_identity.session_identity
    }

    pub(crate) const fn context_identity(&self) -> [u8; 32] {
        self.attestation.operator_identity.context_identity
    }

    pub(crate) const fn trust_root_revision(&self) -> u64 {
        self.attestation.operator_identity.trust_root_revision
    }

    pub(crate) const fn assurance_revision(&self) -> u64 {
        self.attestation.operator_identity.assurance_revision
    }

    pub(crate) const fn human_capable(&self) -> bool {
        self.attestation.operator_identity.human_capable
    }

    pub(crate) const fn binding_not_before(&self) -> u64 {
        self.attestation.operator_identity.binding_not_before
    }

    pub(crate) const fn binding_expires_at(&self) -> u64 {
        self.attestation.operator_identity.binding_expires_at
    }

    pub(crate) const fn session_not_before(&self) -> u64 {
        self.attestation.operator_identity.session_not_before
    }

    pub(crate) const fn session_expires_at(&self) -> u64 {
        self.attestation.operator_identity.session_expires_at
    }

    pub(crate) const fn store_generation(&self) -> u64 {
        self.attestation.operator_identity.store_generation
    }

    pub(crate) const fn authority_epoch(&self) -> u64 {
        self.attestation.operator_identity.authority_epoch
    }

    pub(crate) const fn domain_identity(&self) -> [u8; 32] {
        self.attestation.operator_identity.domain_identity
    }

    pub(crate) const fn domain_role(&self) -> u64 {
        self.attestation.operator_identity.domain_role
    }
}

#[cfg(test)]
impl sealed::Presentation for TrustedHostDiagnosticPresentationV1<'_, '_, '_, '_> {}

#[cfg(test)]
impl TrustedHostDiagnosticPresentationPortV1
    for TrustedHostDiagnosticPresentationV1<'_, '_, '_, '_>
{
    fn anchor_commitment(&self) -> [u8; 32] {
        self.anchor_commitment()
    }

    fn authority_commitment(&self) -> [u8; 32] {
        self.authority_commitment()
    }

    fn protected_subject_commitment(&self) -> [u8; 32] {
        self.protected_subject_commitment()
    }

    fn invocation_nonce(&self) -> [u8; 32] {
        self.invocation_nonce()
    }

    fn challenge_commitment(&self) -> [u8; 32] {
        self.challenge_commitment()
    }

    fn claims_commitment(&self) -> [u8; 32] {
        self.claims_commitment()
    }

    fn principal_identity(&self) -> [u8; 32] {
        self.principal_identity()
    }

    fn binding_identity(&self) -> [u8; 32] {
        self.binding_identity()
    }

    fn session_identity(&self) -> [u8; 32] {
        self.session_identity()
    }

    fn context_identity(&self) -> [u8; 32] {
        self.context_identity()
    }

    fn trust_root_revision(&self) -> u64 {
        self.trust_root_revision()
    }

    fn assurance_revision(&self) -> u64 {
        self.assurance_revision()
    }

    fn human_capable(&self) -> bool {
        self.human_capable()
    }

    fn binding_not_before(&self) -> u64 {
        self.binding_not_before()
    }

    fn binding_expires_at(&self) -> u64 {
        self.binding_expires_at()
    }

    fn session_not_before(&self) -> u64 {
        self.session_not_before()
    }

    fn session_expires_at(&self) -> u64 {
        self.session_expires_at()
    }

    fn store_generation(&self) -> u64 {
        self.store_generation()
    }

    fn authority_epoch(&self) -> u64 {
        self.authority_epoch()
    }

    fn domain_identity(&self) -> [u8; 32] {
        self.domain_identity()
    }

    fn domain_role(&self) -> u64 {
        self.domain_role()
    }
}

#[cfg(test)]
impl<'connection, 'anchor, 'view> TrustedHostDiagnosticAttestationV1<'connection, 'anchor, 'view> {
    pub(crate) const fn witness_carrier_commitment(&self) -> [u8; 32] {
        self.claims_commitment
    }

    pub(crate) fn present_once<'presentation>(
        &'presentation mut self,
    ) -> Option<TrustedHostDiagnosticPresentationV1<'presentation, 'connection, 'anchor, 'view>>
    {
        if self.consumed || self.operator_presented {
            return None;
        }
        self.operator_presented = true;
        Some(TrustedHostDiagnosticPresentationV1 { attestation: self })
    }

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

#[cfg(test)]
impl sealed::Attestation for TrustedHostDiagnosticAttestationV1<'_, '_, '_> {}

#[cfg(test)]
impl TrustedHostDiagnosticAttestationPortV1 for TrustedHostDiagnosticAttestationV1<'_, '_, '_> {
    fn witness_carrier_commitment(&self) -> [u8; 32] {
        self.witness_carrier_commitment()
    }

    fn present_once(
        &mut self,
        inspect: &mut dyn FnMut(&dyn TrustedHostDiagnosticPresentationPortV1) -> bool,
    ) -> bool {
        TrustedHostDiagnosticAttestationV1::present_once(self)
            .is_some_and(|presentation| inspect(&presentation))
    }

    fn final_recheck(self: Box<Self>) -> bool {
        TrustedHostDiagnosticAttestationV1::final_recheck(*self)
    }
}

#[cfg(test)]
impl Drop for TrustedHostDiagnosticAttestationV1<'_, '_, '_> {
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
