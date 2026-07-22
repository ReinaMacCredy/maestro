use crate::domain::vnext::authority::TrustedHostDiagnosticChallengeV1;

use super::trusted_host_diagnostic::{
    TrustedHostDiagnosticAttestationPortV1, TrustedHostDiagnosticConnectionPortV1,
    TrustedHostDiagnosticPresentationPortV1, sealed,
};

struct Stage10OwnerLocalConnectionSeedV1 {
    live_connection_incarnation: [u8; 32],
}

struct Stage10OwnerLocalAttestationSeedV1<'connection, 'anchor, 'view> {
    connection: &'connection mut Stage10OwnerLocalConnectionSeedV1,
    challenge: TrustedHostDiagnosticChallengeV1<'anchor, 'view>,
    presented: bool,
}

struct Stage10OwnerLocalPresentationSeedV1<'presentation, 'connection, 'anchor, 'view> {
    attestation: &'presentation Stage10OwnerLocalAttestationSeedV1<'connection, 'anchor, 'view>,
}

impl sealed::Connection for Stage10OwnerLocalConnectionSeedV1 {}
impl sealed::Attestation for Stage10OwnerLocalAttestationSeedV1<'_, '_, '_> {}
impl sealed::Presentation for Stage10OwnerLocalPresentationSeedV1<'_, '_, '_, '_> {}

impl TrustedHostDiagnosticConnectionPortV1 for Stage10OwnerLocalConnectionSeedV1 {
    fn attest_in_current_view<'scope, 'view>(
        &'scope mut self,
        challenge: TrustedHostDiagnosticChallengeV1<'scope, 'view>,
    ) -> Option<Box<dyn TrustedHostDiagnosticAttestationPortV1 + 'scope>>
    where
        'view: 'scope,
    {
        if self.live_connection_incarnation == [0; 32] {
            return None;
        }
        Some(Box::new(Stage10OwnerLocalAttestationSeedV1 {
            connection: self,
            challenge,
            presented: false,
        }))
    }
}

impl TrustedHostDiagnosticAttestationPortV1 for Stage10OwnerLocalAttestationSeedV1<'_, '_, '_> {
    fn present_once(
        &mut self,
        inspect: &mut dyn FnMut(&dyn TrustedHostDiagnosticPresentationPortV1) -> bool,
    ) -> bool {
        if self.presented {
            return false;
        }
        self.presented = true;
        inspect(&Stage10OwnerLocalPresentationSeedV1 { attestation: self })
    }

    fn final_recheck(self: Box<Self>) -> Option<[u8; 32]> {
        (self.connection.live_connection_incarnation != [0; 32]).then_some([1; 32])
    }
}

macro_rules! fixed_digest_getters {
    ($($name:ident),+ $(,)?) => {
        $(fn $name(&self) -> [u8; 32] { [1; 32] })+
    };
}

macro_rules! fixed_revision_getters {
    ($($name:ident),+ $(,)?) => {
        $(fn $name(&self) -> u64 { 1 })+
    };
}

impl TrustedHostDiagnosticPresentationPortV1
    for Stage10OwnerLocalPresentationSeedV1<'_, '_, '_, '_>
{
    fn anchor_commitment(&self) -> [u8; 32] {
        self.attestation.challenge.anchor_commitment()
    }

    fn authority_commitment(&self) -> [u8; 32] {
        self.attestation.challenge.authority_commitment()
    }

    fn protected_subject_commitment(&self) -> [u8; 32] {
        self.attestation.challenge.protected_subject_commitment()
    }

    fn invocation_nonce(&self) -> [u8; 32] {
        self.attestation.challenge.invocation_nonce()
    }

    fn challenge_commitment(&self) -> [u8; 32] {
        self.attestation.challenge.commitment()
    }

    fixed_digest_getters!(
        attestation_commitment,
        provider_identity,
        profile_identity,
        process_incarnation,
        connection_incarnation,
        channel_incarnation,
        issuer_identity,
        realm_identity,
        audience_identity,
        authentication_event_identity,
        freshness_identity,
        carrier_commitment,
        principal_identity,
        binding_identity,
        session_identity,
        context_identity,
        domain_identity,
    );

    fixed_revision_getters!(
        profile_revision,
        host_currentness_revision,
        revocation_revision,
        trust_root_revision,
        assurance_revision,
        binding_not_before,
        binding_expires_at,
        session_not_before,
        session_expires_at,
        store_generation,
        authority_epoch,
        domain_role,
        incarnation_revision,
    );

    fn human_capable(&self) -> bool {
        true
    }
}

#[test]
fn stage10_owner_local_descendant_can_implement_the_sealed_host_ports() {
    fn require_connection<T: TrustedHostDiagnosticConnectionPortV1>() {}
    fn require_attestation<T: TrustedHostDiagnosticAttestationPortV1>() {}
    fn require_presentation<T: TrustedHostDiagnosticPresentationPortV1>() {}

    require_connection::<Stage10OwnerLocalConnectionSeedV1>();
    require_attestation::<Stage10OwnerLocalAttestationSeedV1<'static, 'static, 'static>>();
    require_presentation::<Stage10OwnerLocalPresentationSeedV1<'static, 'static, 'static, 'static>>(
    );
}
