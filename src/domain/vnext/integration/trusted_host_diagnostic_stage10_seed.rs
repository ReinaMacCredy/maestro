use crate::domain::vnext::authority::TrustedHostDiagnosticChallengeV1;
use sha2::{Digest, Sha256};

use super::trusted_host_diagnostic::{
    TrustedHostDiagnosticAttestationPortV1, TrustedHostDiagnosticConnectionPortV1,
    TrustedHostDiagnosticPresentationPortV1, sealed,
};

const ATTESTATION_DOMAIN_V1: &[u8] = b"maestro.vnext.trusted-host-diagnostic-attestation.v1";

pub(crate) trait AuthenticatedHostConnectionSnapshotV1 {
    fn provider_identity(&self) -> [u8; 32];
    fn profile_identity(&self) -> [u8; 32];
    fn profile_revision(&self) -> u64;
    fn process_incarnation(&self) -> [u8; 32];
    fn connection_incarnation(&self) -> [u8; 32];
    fn channel_incarnation(&self) -> [u8; 32];
    fn issuer_identity(&self) -> [u8; 32];
    fn realm_identity(&self) -> [u8; 32];
    fn audience_identity(&self) -> [u8; 32];
    fn authentication_event_identity(&self) -> [u8; 32];
    fn host_currentness_revision(&self) -> u64;
    fn revocation_revision(&self) -> u64;
    fn freshness_identity(&self) -> [u8; 32];
    fn carrier_commitment(&self) -> [u8; 32];
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
    fn incarnation_revision(&self) -> u64;
}

pub(crate) trait LiveAuthenticatedHostConnectionV1 {
    fn profile_id(&self) -> &str;

    fn claim_authenticated_invocation_no_io(
        &mut self,
        challenge_commitment: [u8; 32],
        invocation_nonce: [u8; 32],
        inspect: &mut dyn FnMut(&dyn AuthenticatedHostConnectionSnapshotV1) -> bool,
    ) -> bool;

    fn recheck_authenticated_invocation_no_io(
        &mut self,
        challenge_commitment: [u8; 32],
        invocation_nonce: [u8; 32],
        inspect: &mut dyn FnMut(&dyn AuthenticatedHostConnectionSnapshotV1) -> bool,
    ) -> bool;
}

pub(crate) struct Stage10OwnerLocalConnectionSeedV1<'host> {
    host: &'host mut dyn LiveAuthenticatedHostConnectionV1,
    invocation_attempted: bool,
}

impl<'host> Stage10OwnerLocalConnectionSeedV1<'host> {
    pub(crate) fn acquire_from_authenticated_host(
        host: &'host mut dyn LiveAuthenticatedHostConnectionV1,
    ) -> Option<Self> {
        (!host.profile_id().is_empty()).then_some(Self {
            host,
            invocation_attempted: false,
        })
    }
}

struct BoundAuthenticatedHostFactsV1 {
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
    host_currentness_revision: u64,
    revocation_revision: u64,
    freshness_identity: [u8; 32],
    carrier_commitment: [u8; 32],
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
    incarnation_revision: u64,
}

impl BoundAuthenticatedHostFactsV1 {
    fn capture(snapshot: &dyn AuthenticatedHostConnectionSnapshotV1) -> Option<Self> {
        let facts = Self {
            provider_identity: snapshot.provider_identity(),
            profile_identity: snapshot.profile_identity(),
            profile_revision: snapshot.profile_revision(),
            process_incarnation: snapshot.process_incarnation(),
            connection_incarnation: snapshot.connection_incarnation(),
            channel_incarnation: snapshot.channel_incarnation(),
            issuer_identity: snapshot.issuer_identity(),
            realm_identity: snapshot.realm_identity(),
            audience_identity: snapshot.audience_identity(),
            authentication_event_identity: snapshot.authentication_event_identity(),
            host_currentness_revision: snapshot.host_currentness_revision(),
            revocation_revision: snapshot.revocation_revision(),
            freshness_identity: snapshot.freshness_identity(),
            carrier_commitment: snapshot.carrier_commitment(),
            principal_identity: snapshot.principal_identity(),
            binding_identity: snapshot.binding_identity(),
            session_identity: snapshot.session_identity(),
            context_identity: snapshot.context_identity(),
            trust_root_revision: snapshot.trust_root_revision(),
            assurance_revision: snapshot.assurance_revision(),
            human_capable: snapshot.human_capable(),
            binding_not_before: snapshot.binding_not_before(),
            binding_expires_at: snapshot.binding_expires_at(),
            session_not_before: snapshot.session_not_before(),
            session_expires_at: snapshot.session_expires_at(),
            store_generation: snapshot.store_generation(),
            authority_epoch: snapshot.authority_epoch(),
            domain_identity: snapshot.domain_identity(),
            domain_role: snapshot.domain_role(),
            incarnation_revision: snapshot.incarnation_revision(),
        };
        facts.is_well_formed().then_some(facts)
    }

    fn is_well_formed(&self) -> bool {
        ![
            self.provider_identity,
            self.profile_identity,
            self.process_incarnation,
            self.connection_incarnation,
            self.channel_incarnation,
            self.issuer_identity,
            self.realm_identity,
            self.audience_identity,
            self.authentication_event_identity,
            self.freshness_identity,
            self.carrier_commitment,
            self.principal_identity,
            self.binding_identity,
            self.session_identity,
            self.context_identity,
            self.domain_identity,
        ]
        .contains(&[0; 32])
            && [
                self.profile_revision,
                self.host_currentness_revision,
                self.revocation_revision,
                self.trust_root_revision,
                self.assurance_revision,
                self.store_generation,
                self.authority_epoch,
                self.domain_role,
                self.incarnation_revision,
            ]
            .iter()
            .all(|value| *value > 0)
            && self.human_capable
            && self.binding_not_before < self.binding_expires_at
            && self.session_not_before < self.session_expires_at
    }

    fn matches(&self, snapshot: &dyn AuthenticatedHostConnectionSnapshotV1) -> bool {
        let Some(current) = Self::capture(snapshot) else {
            return false;
        };
        self.commitment_fields() == current.commitment_fields()
    }

    fn commitment_fields(&self) -> Vec<Vec<u8>> {
        vec![
            self.provider_identity.to_vec(),
            self.profile_identity.to_vec(),
            self.profile_revision.to_be_bytes().to_vec(),
            self.process_incarnation.to_vec(),
            self.connection_incarnation.to_vec(),
            self.channel_incarnation.to_vec(),
            self.issuer_identity.to_vec(),
            self.realm_identity.to_vec(),
            self.audience_identity.to_vec(),
            self.authentication_event_identity.to_vec(),
            self.host_currentness_revision.to_be_bytes().to_vec(),
            self.revocation_revision.to_be_bytes().to_vec(),
            self.freshness_identity.to_vec(),
            self.carrier_commitment.to_vec(),
            self.principal_identity.to_vec(),
            self.binding_identity.to_vec(),
            self.session_identity.to_vec(),
            self.context_identity.to_vec(),
            self.trust_root_revision.to_be_bytes().to_vec(),
            self.assurance_revision.to_be_bytes().to_vec(),
            vec![u8::from(self.human_capable)],
            self.binding_not_before.to_be_bytes().to_vec(),
            self.binding_expires_at.to_be_bytes().to_vec(),
            self.session_not_before.to_be_bytes().to_vec(),
            self.session_expires_at.to_be_bytes().to_vec(),
            self.store_generation.to_be_bytes().to_vec(),
            self.authority_epoch.to_be_bytes().to_vec(),
            self.domain_identity.to_vec(),
            self.domain_role.to_be_bytes().to_vec(),
            self.incarnation_revision.to_be_bytes().to_vec(),
        ]
    }
}

struct Stage10OwnerLocalAttestationSeedV1<'connection, 'host, 'anchor, 'view> {
    connection: &'connection mut Stage10OwnerLocalConnectionSeedV1<'host>,
    challenge: TrustedHostDiagnosticChallengeV1<'anchor, 'view>,
    facts: BoundAuthenticatedHostFactsV1,
    attestation_commitment: [u8; 32],
    presented: bool,
    consumed: bool,
}

struct Stage10OwnerLocalPresentationSeedV1<'presentation, 'connection, 'host, 'anchor, 'view> {
    attestation:
        &'presentation Stage10OwnerLocalAttestationSeedV1<'connection, 'host, 'anchor, 'view>,
}

impl sealed::Connection for Stage10OwnerLocalConnectionSeedV1<'_> {}
impl sealed::Attestation for Stage10OwnerLocalAttestationSeedV1<'_, '_, '_, '_> {}
impl sealed::Presentation for Stage10OwnerLocalPresentationSeedV1<'_, '_, '_, '_, '_> {}

impl TrustedHostDiagnosticConnectionPortV1 for Stage10OwnerLocalConnectionSeedV1<'_> {
    fn attest_in_current_view<'scope, 'view>(
        &'scope mut self,
        challenge: TrustedHostDiagnosticChallengeV1<'scope, 'view>,
    ) -> Option<Box<dyn TrustedHostDiagnosticAttestationPortV1 + 'scope>>
    where
        'view: 'scope,
    {
        if self.invocation_attempted {
            return None;
        }
        self.invocation_attempted = true;
        let challenge_commitment = challenge.commitment();
        let invocation_nonce = challenge.invocation_nonce();
        let mut visits = 0usize;
        let mut captured = None;
        let claimed = self.host.claim_authenticated_invocation_no_io(
            challenge_commitment,
            invocation_nonce,
            &mut |snapshot| {
                visits = visits.saturating_add(1);
                if visits != 1 {
                    return false;
                }
                captured = BoundAuthenticatedHostFactsV1::capture(snapshot);
                captured.is_some()
            },
        );
        if !claimed || visits != 1 {
            return None;
        }
        let facts = captured?;
        let attestation_commitment = attestation_commitment(&facts, challenge_commitment);
        Some(Box::new(Stage10OwnerLocalAttestationSeedV1 {
            connection: self,
            challenge,
            facts,
            attestation_commitment,
            presented: false,
            consumed: false,
        }))
    }
}

impl TrustedHostDiagnosticAttestationPortV1 for Stage10OwnerLocalAttestationSeedV1<'_, '_, '_, '_> {
    fn present_once(
        &mut self,
        inspect: &mut dyn FnMut(&dyn TrustedHostDiagnosticPresentationPortV1) -> bool,
    ) -> bool {
        if self.consumed || self.presented {
            return false;
        }
        self.presented = true;
        inspect(&Stage10OwnerLocalPresentationSeedV1 { attestation: self })
    }

    fn final_recheck(self: Box<Self>) -> Option<[u8; 32]> {
        let mut attestation = *self;
        if attestation.consumed || !attestation.presented {
            return None;
        }
        attestation.consumed = true;
        let challenge_commitment = attestation.challenge.commitment();
        let invocation_nonce = attestation.challenge.invocation_nonce();
        let mut visits = 0usize;
        let mut matched = false;
        let rechecked = attestation
            .connection
            .host
            .recheck_authenticated_invocation_no_io(
                challenge_commitment,
                invocation_nonce,
                &mut |snapshot| {
                    visits = visits.saturating_add(1);
                    if visits != 1 {
                        return false;
                    }
                    matched = attestation.facts.matches(snapshot);
                    matched
                },
            );
        (rechecked && visits == 1 && matched).then_some(attestation.attestation_commitment)
    }
}

impl Drop for Stage10OwnerLocalAttestationSeedV1<'_, '_, '_, '_> {
    fn drop(&mut self) {
        self.consumed = true;
    }
}

macro_rules! fact_getters {
    ($($name:ident: $type:ty),+ $(,)?) => {
        $(fn $name(&self) -> $type { self.attestation.facts.$name })+
    };
}

impl TrustedHostDiagnosticPresentationPortV1
    for Stage10OwnerLocalPresentationSeedV1<'_, '_, '_, '_, '_>
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

    fn attestation_commitment(&self) -> [u8; 32] {
        self.attestation.attestation_commitment
    }

    fact_getters!(
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
        host_currentness_revision: u64,
        revocation_revision: u64,
        freshness_identity: [u8; 32],
        carrier_commitment: [u8; 32],
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
        incarnation_revision: u64,
    );
}

fn attestation_commitment(
    facts: &BoundAuthenticatedHostFactsV1,
    challenge_commitment: [u8; 32],
) -> [u8; 32] {
    let fields = facts.commitment_fields();
    let mut digest = Sha256::new();
    digest.update((ATTESTATION_DOMAIN_V1.len() as u64).to_be_bytes());
    digest.update(ATTESTATION_DOMAIN_V1);
    digest.update((challenge_commitment.len() as u64).to_be_bytes());
    digest.update(challenge_commitment);
    for field in fields {
        digest.update((field.len() as u64).to_be_bytes());
        digest.update(field);
    }
    digest.finalize().into()
}

#[cfg(test)]
#[test]
fn stage10_owner_local_descendant_can_implement_the_sealed_host_ports() {
    fn require_connection<T: TrustedHostDiagnosticConnectionPortV1>() {}
    fn require_attestation<T: TrustedHostDiagnosticAttestationPortV1>() {}
    fn require_presentation<T: TrustedHostDiagnosticPresentationPortV1>() {}

    require_connection::<Stage10OwnerLocalConnectionSeedV1<'static>>();
    require_attestation::<Stage10OwnerLocalAttestationSeedV1<'static, 'static, 'static, 'static>>();
    require_presentation::<
        Stage10OwnerLocalPresentationSeedV1<'static, 'static, 'static, 'static, 'static>,
    >();
}

#[cfg(test)]
#[test]
fn production_attestation_carrier_is_move_only_and_has_no_bearer_surface() {
    let source = include_str!("trusted_host_diagnostic_stage10_seed.rs");
    let production = source.split("#[cfg(test)]").next().unwrap();
    for forbidden in [
        "#[derive(Clone",
        "#[derive(Copy",
        "impl Clone for",
        "impl Copy for",
        "Serialize",
        "Deserialize",
        "pub fn new",
        "pub(crate) fn new",
        "cache",
        "persist",
        "AtomicU64",
        "OnceLock",
    ] {
        assert!(
            !production.contains(forbidden),
            "forbidden carrier surface {forbidden}"
        );
    }
    for required in [
        "claim_authenticated_invocation_no_io",
        "recheck_authenticated_invocation_no_io",
        "invocation_attempted",
        "presented: bool",
        "consumed: bool",
        "carrier_commitment",
        "revocation_revision",
        "connection_incarnation",
    ] {
        assert!(
            production.contains(required),
            "missing production binding {required}"
        );
    }
}

#[cfg(test)]
mod production_factory_tests {
    use super::*;

    macro_rules! snapshot_getters {
        ($($name:ident: $type:ty),+ $(,)?) => {
            $(fn $name(&self) -> $type { self.$name })+
        };
    }

    impl AuthenticatedHostConnectionSnapshotV1 for BoundAuthenticatedHostFactsV1 {
        snapshot_getters!(
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
            host_currentness_revision: u64,
            revocation_revision: u64,
            freshness_identity: [u8; 32],
            carrier_commitment: [u8; 32],
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
            incarnation_revision: u64,
        );
    }

    struct AuthenticatedHost {
        profile_id: &'static str,
        facts: BoundAuthenticatedHostFactsV1,
    }

    impl LiveAuthenticatedHostConnectionV1 for AuthenticatedHost {
        fn profile_id(&self) -> &str {
            self.profile_id
        }

        fn claim_authenticated_invocation_no_io(
            &mut self,
            _challenge_commitment: [u8; 32],
            _invocation_nonce: [u8; 32],
            inspect: &mut dyn FnMut(&dyn AuthenticatedHostConnectionSnapshotV1) -> bool,
        ) -> bool {
            inspect(&self.facts)
        }

        fn recheck_authenticated_invocation_no_io(
            &mut self,
            _challenge_commitment: [u8; 32],
            _invocation_nonce: [u8; 32],
            inspect: &mut dyn FnMut(&dyn AuthenticatedHostConnectionSnapshotV1) -> bool,
        ) -> bool {
            inspect(&self.facts)
        }
    }

    fn facts() -> BoundAuthenticatedHostFactsV1 {
        BoundAuthenticatedHostFactsV1 {
            provider_identity: [1; 32],
            profile_identity: [2; 32],
            profile_revision: 1,
            process_incarnation: [3; 32],
            connection_incarnation: [4; 32],
            channel_incarnation: [5; 32],
            issuer_identity: [6; 32],
            realm_identity: [7; 32],
            audience_identity: [8; 32],
            authentication_event_identity: [9; 32],
            host_currentness_revision: 1,
            revocation_revision: 1,
            freshness_identity: [10; 32],
            carrier_commitment: [11; 32],
            principal_identity: [12; 32],
            binding_identity: [13; 32],
            session_identity: [14; 32],
            context_identity: [15; 32],
            trust_root_revision: 1,
            assurance_revision: 1,
            human_capable: true,
            binding_not_before: 10,
            binding_expires_at: 20,
            session_not_before: 10,
            session_expires_at: 20,
            store_generation: 1,
            authority_epoch: 1,
            domain_identity: [16; 32],
            domain_role: 1,
            incarnation_revision: 1,
        }
    }

    #[test]
    fn factory_accepts_only_a_named_authenticated_connection() {
        let mut host = AuthenticatedHost {
            profile_id: "agents-compatible-cli",
            facts: facts(),
        };
        assert!(
            Stage10OwnerLocalConnectionSeedV1::acquire_from_authenticated_host(&mut host).is_some()
        );

        let mut unnamed = AuthenticatedHost {
            profile_id: "",
            facts: facts(),
        };
        assert!(
            Stage10OwnerLocalConnectionSeedV1::acquire_from_authenticated_host(&mut unnamed)
                .is_none()
        );
    }

    #[test]
    fn final_snapshot_match_refuses_revocation_currentness_and_incarnation_mutants() {
        let expected = facts();
        assert!(expected.matches(&facts()));

        let mut revoked = facts();
        revoked.revocation_revision += 1;
        assert!(!expected.matches(&revoked));

        let mut advanced = facts();
        advanced.host_currentness_revision += 1;
        assert!(!expected.matches(&advanced));

        let mut replaced = facts();
        replaced.connection_incarnation[0] ^= 0xff;
        assert!(!expected.matches(&replaced));
    }
}
