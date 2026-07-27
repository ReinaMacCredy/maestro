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
    assert!(connectors.contains("acquire_trusted_host_diagnostic_connection"));
    assert!(connectors.contains("live_connection.profile_id() != profile_id"));
    assert!(mcp.contains("acquire_trusted_host_diagnostic_connection("));
    assert!(mcp.contains("ok_or(Stage10AdapterError::TrustedHostAuthorityRejected)"));
}

#[test]
fn interface_gap_records_the_real_stage9_provider_binding_without_activation_claims() {
    let gap: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(
            workspace().join("tools/vnext_contracts/stage10/interface-gap.v2.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(gap["status"], "stage9-currentness-provider-bound");
    assert_eq!(gap["satisfied_by_upstream"].as_array().unwrap().len(), 3);
    assert_eq!(
        gap["required_before_runtime_activation"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(gap["runtime_activation"], false);
}
