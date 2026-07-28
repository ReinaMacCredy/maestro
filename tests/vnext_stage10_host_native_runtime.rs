use std::fs;
use std::path::Path;

use serde_json::Value;

fn workspace() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn current_v2_host_profiles_are_inactive_and_bind_no_provider_fields() {
    for path in [
        "embedded/vnext/hosts/agents-compatible-cli.v2.json",
        "embedded/vnext/hosts/claude-code.v2.json",
    ] {
        let descriptor: Value =
            serde_json::from_str(&fs::read_to_string(workspace().join(path)).unwrap()).unwrap();
        assert_eq!(descriptor["schema"], "maestro.vnext.host-descriptor.v2");
        assert_eq!(
            descriptor["protected_runtime_activation"],
            serde_json::json!({
                "variant": "Inactive",
                "reason_code": "supported_host_native_provider_unavailable"
            })
        );
    }
}

#[test]
fn host_native_seam_accepts_only_an_exclusive_same_process_borrow() {
    let source = fs::read_to_string(workspace().join("src/interfaces/connectors/mod.rs")).unwrap();
    for required in [
        "ProtectedRuntimeActivationBindingV2::Active",
        "&'host mut dyn LiveAuthenticatedHostConnectionV1",
        "Stage10OwnerLocalConnectionSeedV1<'host>",
        "Stage10OwnerLocalConnectionSeedV1::acquire_from_authenticated_host",
        "live_connection.profile_id() != descriptor.profile_id",
        "connection.provider_implementation_identity()",
        "connection.production_conformance_proof_identity()",
        "connection.production_negative_proof_identity()",
        "connection.binary_identity()",
        "connection.release_id()",
    ] {
        assert!(
            source.contains(required),
            "missing host-native seam {required}"
        );
    }
    for forbidden in [
        "std::env::var",
        "UnixStream",
        "TcpStream",
        "credential_parser",
        "global_registry",
    ] {
        assert!(
            !source.contains(forbidden),
            "host-native seam contains ambient fallback {forbidden}"
        );
    }
}

#[test]
fn packet_and_search_adapters_do_not_discover_an_ambient_repository() {
    let packet = fs::read_to_string(workspace().join("src/interfaces/cli/packet.rs")).unwrap();
    let packet_production = packet.split("#[cfg(test)]").next().unwrap();
    assert!(packet_production.contains("request.repository_locator"));
    assert!(packet_production.contains("is_absolute()"));
    assert!(packet_production.contains("canonicalize()"));
    assert!(!packet_production.contains("discover_repo_root"));
    assert!(!packet_production.contains("current_dir()"));

    let search =
        fs::read_to_string(workspace().join("src/operations/adapters/live_projection.rs")).unwrap();
    assert!(search.contains("std::env::current_exe()"));
    assert!(!search.contains("discover_repo_root"));
    assert!(search.contains("_ => false"));
}
