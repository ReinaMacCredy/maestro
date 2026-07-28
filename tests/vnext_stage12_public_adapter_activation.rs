use std::fs;

use serde_json::Value;

#[test]
fn public_adapter_descriptor_activates_exactly_two_global_read_only_tools() {
    let descriptor_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/embedded/vnext/adapter/mcp-tools.v1.json"
    );
    let descriptor: Value =
        serde_json::from_str(&fs::read_to_string(descriptor_path).expect("read descriptor"))
            .expect("parse descriptor");

    assert_eq!(descriptor["candidate_only"], false);
    assert_eq!(descriptor["runtime_activation"], true);
    assert_eq!(descriptor["runtime_registration"], true);
    assert_eq!(
        descriptor["scope"],
        Value::String("global-user-agent-installation".to_owned())
    );
    assert_eq!(descriptor["project_tools"], Value::Array(Vec::new()));

    let tools = descriptor["tools"].as_array().expect("tool rows");
    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0]["name"], "maestro_packet");
    assert_eq!(tools[1]["name"], "maestro_cli_search");
    for tool in tools {
        assert_eq!(tool["read_only"], true);
        assert_eq!(tool["writes"], false);
        assert_eq!(tool["network_io"], false);
    }
}

#[test]
fn packet_descriptor_preserves_three_modes_and_six_terminal_outcomes() {
    let descriptor_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/embedded/vnext/adapter/mcp-tools.v1.json"
    );
    let descriptor: Value =
        serde_json::from_str(&fs::read_to_string(descriptor_path).expect("read descriptor"))
            .expect("parse descriptor");
    let packet = &descriptor["tools"][0];

    assert_eq!(
        packet["request_modes"],
        serde_json::json!([
            "BootstrapNoRecipeV1",
            "DiscoverSelectionContextV1",
            "ProjectV1"
        ])
    );
    assert_eq!(
        packet["response_outcomes"],
        serde_json::json!([
            "Packet",
            "SelectionContext",
            "NoActiveStore",
            "Unavailable",
            "Stale",
            "Incompatible"
        ])
    );
}
