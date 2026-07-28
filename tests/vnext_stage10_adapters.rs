use std::fs;
use std::path::Path;

fn workspace() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn production_adapter_requires_the_sealed_stage9_currentness_port() {
    let source = fs::read_to_string(workspace().join("src/operations/adapters/mod.rs")).unwrap();
    assert!(source.contains("TrustedHostDiagnosticConnectionPortV1"));
    assert!(source.contains("protected_continuity_diagnostic_with_ports"));
    assert!(source.contains("&mut dyn ProtectedDiagnosticCurrentViewProviderV1"));
    assert!(!source.contains("Option<&mut dyn ProtectedDiagnosticCurrentViewProviderV1>"));
    assert!(!source.contains("Stage9CurrentnessUnavailable"));
    assert!(!source.contains("trusted_host_diagnostic_stage10_seed"));
}

#[test]
fn operations_adapter_facade_keeps_live_projection_private_and_narrow() {
    let adapters = fs::read_to_string(workspace().join("src/operations/adapters/mod.rs")).unwrap();
    assert!(adapters.lines().any(|line| line == "mod live_projection;"));
    assert!(!adapters.contains("pub(crate) mod live_projection;"));
    assert!(adapters.contains("pub(crate) use live_projection::{"));
    for exported in [
        "LiveProjectionReadProviderV1",
        "RunningBinaryIdentityV1",
        "cli_search",
        "decode_cli_search_request",
        "encode_cli_search_envelope",
    ] {
        assert!(
            adapters.contains(exported),
            "missing narrow adapter facade export {exported}"
        );
    }
    for moved in [
        "fn cli_search(",
        "fn decode_cli_search_request(",
        "fn encode_cli_search_envelope(",
        "fn cli_search_is_literal_and_does_not_compute_a_next_action(",
        "fn cli_search_transport_is_exact_and_canonical(",
        "fn cli_search_transport_rejects_duplicate_and_unknown_fields(",
    ] {
        assert!(
            !adapters.contains(moved),
            "adapter facade still owns live projection implementation {moved}"
        );
    }

    let projection =
        fs::read_to_string(workspace().join("src/operations/adapters/live_projection.rs")).unwrap();
    for moved in [
        "fn cli_search(",
        "fn decode_cli_search_request(",
        "fn encode_cli_search_envelope(",
        "fn cli_search_is_literal_and_does_not_compute_a_next_action(",
        "fn cli_search_transport_is_exact_and_canonical(",
        "fn cli_search_transport_rejects_duplicate_and_unknown_fields(",
    ] {
        assert!(
            projection.contains(moved),
            "private live projection leaf is missing {moved}"
        );
    }

    let packet = fs::read_to_string(workspace().join("src/interfaces/cli/packet.rs")).unwrap();
    assert!(packet.contains("use crate::operations::adapters::LiveProjectionReadProviderV1;"));
    assert!(!packet.contains("adapters::live_projection"));
}

#[test]
fn protected_diagnostic_enters_through_the_authenticated_host_factory() {
    let integration =
        fs::read_to_string(workspace().join("src/domain/integration/mod.rs")).unwrap();
    let seed = fs::read_to_string(
        workspace().join("src/domain/integration/trusted_host_diagnostic_stage10_seed.rs"),
    )
    .unwrap();
    let connectors =
        fs::read_to_string(workspace().join("src/interfaces/connectors/mod.rs")).unwrap();
    let mcp = fs::read_to_string(workspace().join("src/interfaces/mcp/mod.rs")).unwrap();

    assert!(integration.contains("mod trusted_host_diagnostic_stage10_seed;"));
    assert!(!integration.contains("#[cfg(test)]\nmod trusted_host_diagnostic_stage10_seed;"));
    for required in [
        "claim_authenticated_invocation_no_io",
        "recheck_authenticated_invocation_no_io",
        "invocation_attempted",
        "attestation_commitment",
        "carrier_commitment",
        "revocation_revision",
        "connection_incarnation",
    ] {
        assert!(
            seed.contains(required),
            "missing trusted-host binding {required}"
        );
    }
    assert!(!seed.contains("fixed_digest_getters"));
    assert!(!seed.contains("fixed_revision_getters"));
    assert!(connectors.contains("HostDescriptorV2"));
    assert!(connectors.contains("ProtectedRuntimeActivationBindingV2"));
    assert!(connectors.contains("supported_host_native_provider_unavailable"));
    assert!(connectors.contains("matches!("));
    assert!(connectors.contains("acquire_trusted_host_diagnostic_connection"));
    assert!(connectors.contains("live_connection.profile_id() != descriptor.profile_id"));
    assert!(connectors.contains("agents-compatible-cli.v2.json"));
    assert!(connectors.contains("claude-code.v2.json"));
    assert!(!connectors.contains("agents-compatible-cli.v1.json"));
    assert!(!connectors.contains("claude-code.v1.json"));
    assert!(mcp.contains("acquire_trusted_host_diagnostic_connection("));
    assert!(mcp.contains("ok_or(Stage10AdapterError::TrustedHostAuthorityRejected)"));
}

#[test]
fn interface_gap_records_host_native_injection_and_truthful_inactive_profiles() {
    let gap: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(
            workspace().join("tools/vnext_contracts/stage10/interface-gap.v2.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        gap["status"],
        "host-native-injection-seam-bound-with-inactive-profiles"
    );
    assert_eq!(gap["satisfied_by_upstream"].as_array().unwrap().len(), 5);
    assert_eq!(
        gap["required_before_runtime_activation"]
            .as_array()
            .unwrap()
            .len(),
        6
    );
    assert_eq!(gap["protected_runtime_activation"], false);
    assert_eq!(
        gap["inactive_reason_code"],
        "supported_host_native_provider_unavailable"
    );
}
