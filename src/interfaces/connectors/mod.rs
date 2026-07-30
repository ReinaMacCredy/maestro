//! Stage-10 host and external-connector declarations.

use crate::domain::integration::{
    LiveAuthenticatedHostConnectionV1, Stage10OwnerLocalConnectionSeedV1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProtectedRuntimeActivationBindingV2 {
    Inactive {
        reason_code: &'static str,
    },
    #[allow(
        dead_code,
        reason = "production host descriptors remain inactive until a conformance-proven host is shipped"
    )]
    Active {
        provider_implementation_identity: &'static str,
        provider_revision: u64,
        host_owned_injection_entry: &'static str,
        production_conformance_proof_identity: &'static str,
        production_negative_proof_identity: &'static str,
        binary_identity: &'static str,
        release_id: &'static str,
    },
}

impl ProtectedRuntimeActivationBindingV2 {
    fn admits(self, connection: &dyn LiveAuthenticatedHostConnectionV1) -> bool {
        match self {
            Self::Inactive { .. } => false,
            Self::Active {
                provider_implementation_identity,
                provider_revision,
                host_owned_injection_entry,
                production_conformance_proof_identity,
                production_negative_proof_identity,
                binary_identity,
                release_id,
            } => {
                provider_revision > 0
                    && [
                        provider_implementation_identity,
                        host_owned_injection_entry,
                        production_conformance_proof_identity,
                        production_negative_proof_identity,
                        binary_identity,
                        release_id,
                    ]
                    .iter()
                    .all(|value| !value.is_empty())
                    && connection.provider_implementation_identity()
                        == provider_implementation_identity
                    && connection.provider_revision() == provider_revision
                    && connection.host_owned_injection_entry() == host_owned_injection_entry
                    && connection.production_conformance_proof_identity()
                        == production_conformance_proof_identity
                    && connection.production_negative_proof_identity()
                        == production_negative_proof_identity
                    && connection.binary_identity() == binary_identity
                    && connection.release_id() == release_id
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HostDescriptorV2 {
    pub profile_id: &'static str,
    pub installation_scope: &'static str,
    pub project_registration: bool,
    pub protected_runtime_activation: ProtectedRuntimeActivationBindingV2,
    pub descriptor_json: &'static str,
}

pub(crate) const HOST_DESCRIPTORS_V2: [HostDescriptorV2; 2] = [
    HostDescriptorV2 {
        profile_id: "agents-compatible-cli",
        installation_scope: "global-user-agent-installation",
        project_registration: false,
        protected_runtime_activation: ProtectedRuntimeActivationBindingV2::Inactive {
            reason_code: "supported_host_native_provider_unavailable",
        },
        descriptor_json: include_str!(
            "../../../embedded/vnext/hosts/agents-compatible-cli.v2.json"
        ),
    },
    HostDescriptorV2 {
        profile_id: "claude-code",
        installation_scope: "global-user-agent-installation",
        project_registration: false,
        protected_runtime_activation: ProtectedRuntimeActivationBindingV2::Inactive {
            reason_code: "supported_host_native_provider_unavailable",
        },
        descriptor_json: include_str!("../../../embedded/vnext/hosts/claude-code.v2.json"),
    },
];

pub(crate) fn host_descriptor(profile_id: &str) -> Option<&'static HostDescriptorV2> {
    HOST_DESCRIPTORS_V2
        .iter()
        .find(|descriptor| descriptor.profile_id == profile_id)
}

pub(crate) fn acquire_trusted_host_diagnostic_connection<'host>(
    profile_id: &str,
    live_connection: &'host mut dyn LiveAuthenticatedHostConnectionV1,
) -> Option<Stage10OwnerLocalConnectionSeedV1<'host>> {
    let descriptor = host_descriptor(profile_id)?;
    acquire_from_descriptor(descriptor, live_connection)
}

fn acquire_from_descriptor<'host>(
    descriptor: &HostDescriptorV2,
    live_connection: &'host mut dyn LiveAuthenticatedHostConnectionV1,
) -> Option<Stage10OwnerLocalConnectionSeedV1<'host>> {
    if live_connection.profile_id() != descriptor.profile_id
        || !descriptor
            .protected_runtime_activation
            .admits(live_connection)
    {
        return None;
    }
    Stage10OwnerLocalConnectionSeedV1::acquire_from_designated_connector(live_connection)
}

#[allow(
    dead_code,
    reason = "the Stage 10 bootstrap descriptor remains a shipped connector resource contract"
)]
pub const BOOTSTRAP_WIRING_JSON: &str =
    include_str!("../../../embedded/vnext/bootstrap/wiring.v1.json");

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::integration::AuthenticatedHostConnectionSnapshotV1;

    #[derive(Clone)]
    struct TestConnection {
        profile_id: &'static str,
        provider_implementation_identity: &'static str,
        provider_revision: u64,
        host_owned_injection_entry: &'static str,
        production_conformance_proof_identity: &'static str,
        production_negative_proof_identity: &'static str,
        binary_identity: &'static str,
        release_id: &'static str,
    }

    impl TestConnection {
        fn matching(profile_id: &'static str) -> Self {
            Self {
                profile_id,
                provider_implementation_identity: "candidate:provider:supported-host:v1",
                provider_revision: 1,
                host_owned_injection_entry: "crate::interfaces::connectors::acquire_trusted_host_diagnostic_connection",
                production_conformance_proof_identity: "sha256:conformance",
                production_negative_proof_identity: "sha256:negative",
                binary_identity: "sha256:binary",
                release_id: "candidate:release:supported-host:v1",
            }
        }
    }

    impl LiveAuthenticatedHostConnectionV1 for TestConnection {
        fn profile_id(&self) -> &str {
            self.profile_id
        }

        fn provider_implementation_identity(&self) -> &str {
            self.provider_implementation_identity
        }

        fn provider_revision(&self) -> u64 {
            self.provider_revision
        }

        fn host_owned_injection_entry(&self) -> &str {
            self.host_owned_injection_entry
        }

        fn production_conformance_proof_identity(&self) -> &str {
            self.production_conformance_proof_identity
        }

        fn production_negative_proof_identity(&self) -> &str {
            self.production_negative_proof_identity
        }

        fn binary_identity(&self) -> &str {
            self.binary_identity
        }

        fn release_id(&self) -> &str {
            self.release_id
        }

        fn claim_authenticated_invocation_no_io(
            &mut self,
            _challenge_commitment: [u8; 32],
            _invocation_nonce: [u8; 32],
            _inspect: &mut dyn FnMut(&dyn AuthenticatedHostConnectionSnapshotV1) -> bool,
        ) -> bool {
            false
        }

        fn recheck_authenticated_invocation_no_io(
            &mut self,
            _challenge_commitment: [u8; 32],
            _invocation_nonce: [u8; 32],
            _inspect: &mut dyn FnMut(&dyn AuthenticatedHostConnectionSnapshotV1) -> bool,
        ) -> bool {
            false
        }
    }

    const ACTIVE_DESCRIPTOR: HostDescriptorV2 = HostDescriptorV2 {
        profile_id: "supported-host",
        installation_scope: "global-user-agent-installation",
        project_registration: false,
        protected_runtime_activation: ProtectedRuntimeActivationBindingV2::Active {
            provider_implementation_identity: "candidate:provider:supported-host:v1",
            provider_revision: 1,
            host_owned_injection_entry: "crate::interfaces::connectors::acquire_trusted_host_diagnostic_connection",
            production_conformance_proof_identity: "sha256:conformance",
            production_negative_proof_identity: "sha256:negative",
            binary_identity: "sha256:binary",
            release_id: "candidate:release:supported-host:v1",
        },
        descriptor_json: "{}",
    };

    #[test]
    fn current_profiles_are_truthfully_inactive_and_cannot_inject() {
        for descriptor in HOST_DESCRIPTORS_V2 {
            assert!(matches!(
                descriptor.protected_runtime_activation,
                ProtectedRuntimeActivationBindingV2::Inactive {
                    reason_code: "supported_host_native_provider_unavailable"
                }
            ));
            let mut connection = TestConnection::matching(descriptor.profile_id);
            assert!(
                acquire_trusted_host_diagnostic_connection(descriptor.profile_id, &mut connection)
                    .is_none()
            );
        }
    }

    #[test]
    fn only_a_complete_active_descriptor_accepts_the_exclusive_host_borrow() {
        let mut matching = TestConnection::matching("supported-host");
        assert!(
            ACTIVE_DESCRIPTOR
                .protected_runtime_activation
                .admits(&matching)
        );
        assert!(acquire_from_descriptor(&ACTIVE_DESCRIPTOR, &mut matching).is_some());

        let mut wrong_profile = TestConnection::matching("other-host");
        assert!(acquire_from_descriptor(&ACTIVE_DESCRIPTOR, &mut wrong_profile).is_none());

        let incomplete = HostDescriptorV2 {
            protected_runtime_activation: ProtectedRuntimeActivationBindingV2::Active {
                provider_implementation_identity: "candidate:provider:supported-host:v1",
                provider_revision: 0,
                host_owned_injection_entry: "crate::interfaces::connectors::acquire_trusted_host_diagnostic_connection",
                production_conformance_proof_identity: "sha256:conformance",
                production_negative_proof_identity: "sha256:negative",
                binary_identity: "sha256:binary",
                release_id: "candidate:release:supported-host:v1",
            },
            ..ACTIVE_DESCRIPTOR
        };
        let mut matching = TestConnection::matching("supported-host");
        assert!(acquire_from_descriptor(&incomplete, &mut matching).is_none());

        let mut wrong_connections = Vec::new();
        let mut wrong = TestConnection::matching("supported-host");
        wrong.provider_implementation_identity = "candidate:provider:other:v1";
        wrong_connections.push(wrong);
        let mut wrong = TestConnection::matching("supported-host");
        wrong.provider_revision = 2;
        wrong_connections.push(wrong);
        let mut wrong = TestConnection::matching("supported-host");
        wrong.host_owned_injection_entry = "crate::interfaces::connectors::other";
        wrong_connections.push(wrong);
        let mut wrong = TestConnection::matching("supported-host");
        wrong.production_conformance_proof_identity = "sha256:other-conformance";
        wrong_connections.push(wrong);
        let mut wrong = TestConnection::matching("supported-host");
        wrong.production_negative_proof_identity = "sha256:other-negative";
        wrong_connections.push(wrong);
        let mut wrong = TestConnection::matching("supported-host");
        wrong.binary_identity = "sha256:other-binary";
        wrong_connections.push(wrong);
        let mut wrong = TestConnection::matching("supported-host");
        wrong.release_id = "candidate:release:other:v1";
        wrong_connections.push(wrong);
        for mut wrong in wrong_connections {
            assert!(acquire_from_descriptor(&ACTIVE_DESCRIPTOR, &mut wrong).is_none());
        }
    }
}
