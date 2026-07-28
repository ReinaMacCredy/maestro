use std::fs;
use std::path::Path;

use serde_json::Value;
use sha2::{Digest, Sha256};

fn workspace() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn resources_declare_the_exact_two_read_only_mcp_tools() {
    let connector: Value = serde_json::from_str(
        &fs::read_to_string(
            workspace().join("embedded/vnext/connectors/read-only-global-mcp.v1.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        connector["tools"],
        serde_json::json!(["maestro_packet", "maestro_cli_search"])
    );
    assert_eq!(connector["writes"], false);
    assert_eq!(connector["network_io"], false);
}

#[test]
fn v1_host_resources_remain_immutable_migration_evidence() {
    for (path, expected) in [
        (
            "embedded/vnext/hosts/agents-compatible-cli.v1.json",
            "ac9b0bbd4c632716c472199271e3c27540f76a252a63079bd3f5e44da9ce1740",
        ),
        (
            "embedded/vnext/hosts/claude-code.v1.json",
            "bb46c3279b7ffbc0991cb99d9a2842a98fad69b0f39092c46958bd9a312a4593",
        ),
        (
            "embedded/vnext/patterns/trusted-host-diagnostic.v1.json",
            "eb21da71907db3d6ae126e5cc12f525ece6d3c718869ca823596a46a791d7e0d",
        ),
        (
            "embedded/vnext/schemas/host-descriptor.v1.json",
            "83ede4b3f4e2f325d513a4f73d8deb1a7df418b7443d4ff76989f4882df37c60",
        ),
    ] {
        let bytes = fs::read(workspace().join(path)).unwrap();
        assert_eq!(sha256(&bytes), expected);
    }
}

#[test]
fn v2_host_descriptors_are_truthfully_inactive_without_provider_fields() {
    for path in [
        "embedded/vnext/hosts/agents-compatible-cli.v2.json",
        "embedded/vnext/hosts/claude-code.v2.json",
    ] {
        let descriptor: Value =
            serde_json::from_str(&fs::read_to_string(workspace().join(path)).unwrap()).unwrap();
        assert_eq!(descriptor["schema"], "maestro.vnext.host-descriptor.v2");
        assert_eq!(
            descriptor["installation_scope"],
            "global-user-agent-installation"
        );
        assert_eq!(descriptor["project_registration"], false);
        let activation = descriptor["protected_runtime_activation"]
            .as_object()
            .unwrap();
        assert_eq!(activation.len(), 2);
        assert_eq!(activation["variant"], "Inactive");
        assert_eq!(
            activation["reason_code"],
            "supported_host_native_provider_unavailable"
        );
    }
}

#[test]
fn v2_schema_closes_inactive_and_active_activation_shapes() {
    let schema: Value = serde_json::from_str(
        &fs::read_to_string(workspace().join("embedded/vnext/schemas/host-descriptor.v2.json"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(schema["additionalProperties"], false);
    let variants = schema["$defs"]["ProtectedRuntimeActivationBindingV2"]["oneOf"]
        .as_array()
        .unwrap();
    assert_eq!(variants.len(), 2);
    assert!(
        variants
            .iter()
            .all(|variant| variant["additionalProperties"] == false)
    );
    assert_eq!(
        variants[0]["required"],
        serde_json::json!(["variant", "reason_code"])
    );
    assert_eq!(variants[1]["required"].as_array().unwrap().len(), 8);
}
