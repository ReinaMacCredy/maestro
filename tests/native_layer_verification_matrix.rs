use std::fs;
use std::path::Path;

#[test]
fn testing_guide_records_native_layer_verification_matrix() {
    let testing = normalize_markdown(&read_source_file(Path::new("TESTING.md")));
    for phrase in [
        "Native repository-harness layer verification matrix",
        "parser/classifier downgrade behavior",
        "tests/intake_integration.rs",
        "capability status, permission boundary, redaction, and scoped provider lookup",
        "tests/capability_integration.rs",
        "maturity/context/friction readout stability",
        "tests/maturity_integration.rs",
        "installer, sync, and shim safety",
        "tests/install_dry_run_integration.rs",
        "versioned additive JSON contracts",
        "resource guards and shipped guidance drift",
        "tests/resources_version_guard.rs",
        "tests/cli_reference_freshness.rs",
        "edge-case guardrails",
        "tests/native_layer_authority.rs",
        "absence of forbidden lifecycle side effects",
        "tests/architecture_write_safety.rs",
    ] {
        assert!(
            testing.contains(phrase),
            "TESTING.md must record native-layer verification phrase {phrase:?}"
        );
    }
}

fn read_source_file(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

fn normalize_markdown(body: &str) -> String {
    body.split_whitespace().collect::<Vec<_>>().join(" ")
}
