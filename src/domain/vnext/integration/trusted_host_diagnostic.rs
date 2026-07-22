#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Stage 5 freezes the sealed diagnostic port without a production producer"
    )
)]

use sha2::{Digest, Sha256};
#[cfg(test)]
use std::sync::atomic::{AtomicU64, Ordering};

const CHALLENGE_DOMAIN_V1: &[u8] = b"maestro.vnext.trusted-host-diagnostic-challenge.v1";
const ATTESTATION_DOMAIN_V1: &[u8] = b"maestro.vnext.trusted-host-diagnostic-attestation.v1";

pub(crate) struct TrustedHostDiagnosticChallengeV1 {
    anchor_commitment: [u8; 32],
    authority_commitment: [u8; 32],
    protected_subject_commitment: [u8; 32],
    invocation_nonce: [u8; 32],
    commitment: [u8; 32],
}

impl TrustedHostDiagnosticChallengeV1 {
    fn new(
        anchor_commitment: [u8; 32],
        authority_commitment: [u8; 32],
        protected_subject_commitment: [u8; 32],
        invocation_nonce: [u8; 32],
    ) -> Option<Self> {
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
}

#[cfg(test)]
#[derive(Clone)]
pub(crate) struct TrustedHostDiagnosticTestControlV1(
    std::rc::Rc<std::cell::RefCell<TrustedHostDiagnosticTestStateV1>>,
);

#[cfg(test)]
impl TrustedHostDiagnosticTestControlV1 {
    pub(crate) fn disconnect(&self) {
        self.0.borrow_mut().connected = false;
    }

    pub(crate) fn reconnect(&self) {
        let mut state = self.0.borrow_mut();
        state.connected = true;
        state.incarnation_revision = state.incarnation_revision.saturating_add(1);
    }

    pub(crate) fn revoke(&self) {
        let mut state = self.0.borrow_mut();
        state.revocation_revision = state.revocation_revision.saturating_add(1);
    }

    pub(crate) fn advance_currentness(&self) {
        let mut state = self.0.borrow_mut();
        state.host_currentness_revision = state.host_currentness_revision.saturating_add(1);
    }

    fn substitute_claim_dimension(&self, dimension: usize) {
        let mut state = self.0.borrow_mut();
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
            9 => mutate_digest(&mut claims.authenticated_subject_identity),
            10 => mutate_digest(&mut claims.role_identity),
            11 => mutate_digest(&mut claims.assurance_identity),
            12 => mutate_digest(&mut claims.authentication_event_identity),
            13 => mutate_digest(&mut claims.host_session_identity),
            14 => mutate_digest(&mut claims.freshness_identity),
            15 => mutate_digest(&mut claims.carrier_commitment),
            16 => mutate_digest(&mut claims.operator_mapping_commitment),
            _ => unreachable!(),
        }
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
    authenticated_subject_identity: [u8; 32],
    role_identity: [u8; 32],
    assurance_identity: [u8; 32],
    authentication_event_identity: [u8; 32],
    host_session_identity: [u8; 32],
    freshness_identity: [u8; 32],
    carrier_commitment: [u8; 32],
    operator_mapping_commitment: [u8; 32],
}

#[cfg(test)]
impl TrustedHostDiagnosticTestClaimsV1 {
    pub(crate) fn from_seed(seed: &[u8], operator_mapping_commitment: [u8; 32]) -> Option<Self> {
        if seed.is_empty() || operator_mapping_commitment == [0; 32] {
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
            authenticated_subject_identity: derive(b"authenticated-subject"),
            role_identity: derive(b"human-role"),
            assurance_identity: derive(b"assurance"),
            authentication_event_identity: derive(b"authentication-event"),
            host_session_identity: derive(b"host-session"),
            freshness_identity: derive(b"freshness"),
            carrier_commitment: derive(b"carrier"),
            operator_mapping_commitment,
        })
    }

    pub(crate) fn replace_operator_mapping_commitment(&mut self, value: [u8; 32]) {
        self.operator_mapping_commitment = value;
    }
}

#[cfg(test)]
pub(crate) struct TrustedHostDiagnosticTestConnectionV1 {
    state: std::rc::Rc<std::cell::RefCell<TrustedHostDiagnosticTestStateV1>>,
}

#[cfg(test)]
pub(crate) struct TrustedHostDiagnosticTestInvocationV1<'connection> {
    connection: Option<&'connection mut TrustedHostDiagnosticTestConnectionV1>,
    protected_subject_commitment: [u8; 32],
    invocation_nonce: [u8; 32],
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
            })),
        }
    }

    pub(crate) fn control(&self) -> TrustedHostDiagnosticTestControlV1 {
        TrustedHostDiagnosticTestControlV1(std::rc::Rc::clone(&self.state))
    }

    pub(crate) fn begin_invocation(
        &mut self,
        protected_subject_commitment: [u8; 32],
    ) -> Option<TrustedHostDiagnosticTestInvocationV1<'_>> {
        static SEQUENCE: AtomicU64 = AtomicU64::new(1);
        let mut state = self.state.borrow_mut();
        if !state.connected
            || state.authentication_event_consumed
            || state.issued_invocation_nonce.is_some()
            || protected_subject_commitment == [0; 32]
        {
            return None;
        }
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed).to_be_bytes();
        let nonce = tuple_commitment(
            b"maestro.vnext.trusted-host-diagnostic-invocation-nonce.v1",
            &[
                &state.claims.authentication_event_identity,
                &state.claims.carrier_commitment,
                &protected_subject_commitment,
                &sequence,
            ],
        );
        state.issued_invocation_nonce = Some(nonce);
        drop(state);
        Some(TrustedHostDiagnosticTestInvocationV1 {
            connection: Some(self),
            protected_subject_commitment,
            invocation_nonce: nonce,
        })
    }

    fn attest<'connection>(
        &'connection mut self,
        challenge: TrustedHostDiagnosticChallengeV1,
    ) -> Option<TrustedHostDiagnosticAttestationV1<'connection>> {
        let mut state = self.state.borrow_mut();
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
        let operator_mapping_commitment = state.claims.operator_mapping_commitment;
        drop(state);
        Some(TrustedHostDiagnosticAttestationV1 {
            connection: self,
            anchor_commitment: challenge.anchor_commitment,
            authority_commitment: challenge.authority_commitment,
            protected_subject_commitment: challenge.protected_subject_commitment,
            invocation_nonce: challenge.invocation_nonce,
            challenge_commitment: challenge.commitment,
            claims_commitment,
            operator_mapping_commitment,
            host_currentness_revision,
            revocation_revision,
            incarnation_revision,
            consumed: false,
        })
    }
}

#[cfg(test)]
impl<'connection> TrustedHostDiagnosticTestInvocationV1<'connection> {
    pub(crate) fn attest(
        mut self,
        anchor_commitment: [u8; 32],
        authority_commitment: [u8; 32],
    ) -> Option<TrustedHostDiagnosticAttestationV1<'connection>> {
        let connection = self.connection.take()?;
        let challenge = TrustedHostDiagnosticChallengeV1::new(
            anchor_commitment,
            authority_commitment,
            self.protected_subject_commitment,
            self.invocation_nonce,
        )?;
        connection.attest(challenge)
    }
}

#[cfg(test)]
impl Drop for TrustedHostDiagnosticTestInvocationV1<'_> {
    fn drop(&mut self) {
        let Some(connection) = self.connection.take() else {
            return;
        };
        let mut state = connection.state.borrow_mut();
        if state.issued_invocation_nonce == Some(self.invocation_nonce) {
            state.issued_invocation_nonce = None;
        }
        state.authentication_event_consumed = true;
    }
}

pub(crate) struct TrustedHostDiagnosticAttestationV1<'connection> {
    #[cfg(test)]
    connection: &'connection mut TrustedHostDiagnosticTestConnectionV1,
    #[cfg(not(test))]
    connection: &'connection mut (),
    anchor_commitment: [u8; 32],
    authority_commitment: [u8; 32],
    protected_subject_commitment: [u8; 32],
    invocation_nonce: [u8; 32],
    challenge_commitment: [u8; 32],
    claims_commitment: [u8; 32],
    operator_mapping_commitment: [u8; 32],
    host_currentness_revision: u64,
    revocation_revision: u64,
    incarnation_revision: u64,
    consumed: bool,
}

impl TrustedHostDiagnosticAttestationV1<'_> {
    pub(crate) const fn operator_mapping_commitment(&self) -> [u8; 32] {
        self.operator_mapping_commitment
    }

    pub(crate) const fn witness_carrier_commitment(&self) -> [u8; 32] {
        self.claims_commitment
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
        let state = self.connection.state.borrow();
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

impl Drop for TrustedHostDiagnosticAttestationV1<'_> {
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
            &claims.authenticated_subject_identity,
            &claims.role_identity,
            &claims.assurance_identity,
            &claims.authentication_event_identity,
            &claims.host_session_identity,
            &currentness,
            &revocation,
            &claims.freshness_identity,
            &claims.carrier_commitment,
            &claims.operator_mapping_commitment,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_is_one_shot_and_final_recheck_rejects_turnover() {
        let claims = TrustedHostDiagnosticTestClaimsV1::from_seed(b"host", [7; 32]).unwrap();
        let mut connection = TrustedHostDiagnosticTestConnectionV1::authenticated(claims);
        let control = connection.control();
        let invocation = connection.begin_invocation([3; 32]).unwrap();
        let attestation = invocation.attest([1; 32], [2; 32]).unwrap();
        control.disconnect();
        assert!(!attestation.final_recheck());
        control.reconnect();
        assert!(connection.begin_invocation([3; 32]).is_none());
    }

    #[test]
    fn challenge_refuses_zero_dimensions() {
        assert!(
            TrustedHostDiagnosticChallengeV1::new([0; 32], [2; 32], [3; 32], [4; 32]).is_none()
        );
    }

    #[test]
    fn final_recheck_refuses_revocation_and_currentness_turnover() {
        for mutate in [
            TrustedHostDiagnosticTestControlV1::revoke,
            TrustedHostDiagnosticTestControlV1::advance_currentness,
        ] {
            let claims =
                TrustedHostDiagnosticTestClaimsV1::from_seed(b"host-turnover", [8; 32]).unwrap();
            let mut connection = TrustedHostDiagnosticTestConnectionV1::authenticated(claims);
            let control = connection.control();
            let invocation = connection.begin_invocation([3; 32]).unwrap();
            let attestation = invocation.attest([1; 32], [2; 32]).unwrap();
            mutate(&control);
            assert!(!attestation.final_recheck());
        }
        for dimension in 0..17 {
            let claims =
                TrustedHostDiagnosticTestClaimsV1::from_seed(b"host-claim-turnover", [8; 32])
                    .unwrap();
            let mut connection = TrustedHostDiagnosticTestConnectionV1::authenticated(claims);
            let control = connection.control();
            let invocation = connection.begin_invocation([3; 32]).unwrap();
            let attestation = invocation.attest([1; 32], [2; 32]).unwrap();
            control.substitute_claim_dimension(dimension);
            assert!(!attestation.final_recheck(), "dimension {dimension}");
        }
    }

    #[test]
    fn dropped_or_failed_invocation_consumes_the_authentication_event() {
        let claims = TrustedHostDiagnosticTestClaimsV1::from_seed(b"drop", [9; 32]).unwrap();
        let mut dropped = TrustedHostDiagnosticTestConnectionV1::authenticated(claims);
        drop(dropped.begin_invocation([3; 32]).unwrap());
        assert!(dropped.begin_invocation([3; 32]).is_none());

        let claims = TrustedHostDiagnosticTestClaimsV1::from_seed(b"failure", [9; 32]).unwrap();
        let mut failed = TrustedHostDiagnosticTestConnectionV1::authenticated(claims);
        let invocation = failed.begin_invocation([3; 32]).unwrap();
        assert!(invocation.attest([0; 32], [2; 32]).is_none());
        assert!(failed.begin_invocation([3; 32]).is_none());
    }
}
