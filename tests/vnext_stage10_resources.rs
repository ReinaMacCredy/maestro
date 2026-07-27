use std::fs;
use std::path::Path;

use serde_json::Value;

fn workspace() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
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
fn host_descriptors_are_global_only_and_require_v2_before_activation() {
    for path in [
        "embedded/vnext/hosts/agents-compatible-cli.v1.json",
        "embedded/vnext/hosts/claude-code.v1.json",
    ] {
        let descriptor: Value =
            serde_json::from_str(&fs::read_to_string(workspace().join(path)).unwrap()).unwrap();
        assert_eq!(
            descriptor["installation_scope"],
            "global-user-agent-installation"
        );
        assert_eq!(descriptor["project_registration"], false);
        assert_eq!(
            descriptor["trusted_host_diagnostic"]["trusted_host_port"],
            "TrustedHostDiagnosticConnectionPortV1"
        );
        assert_eq!(
            descriptor["trusted_host_diagnostic"]["runtime_activation"],
            false
        );
    }
}
