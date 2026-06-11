mod support;

use std::fs;

use maestro::domain::harness::schema::{HarnessConfig, StackKind, detect_stack};
use maestro::domain::harness::templates::{HARNESS_MD, harness_yml};
use maestro::foundation::core::paths::MaestroPaths;
use support::TestTempDir;

#[test]
fn harness_markdown_matches_spec_section_14_protocol() {
    assert_eq!(HARNESS_MD, include_str!("../embedded/harness/HARNESS.md"));
    assert!(HARNESS_MD.contains("# Maestro Harness Protocol"));
    assert!(HARNESS_MD.contains("Start with `maestro status`"));
    assert!(HARNESS_MD.contains("maestro task next"));
    assert!(HARNESS_MD.contains("--proof \"<observed evidence>\""));
    assert!(HARNESS_MD.contains("maestro query proof <id>"));
    assert!(!HARNESS_MD.contains("## If you are Claude Code"));
    assert!(!HARNESS_MD.contains("## If you are Codex CLI"));
}

#[test]
fn harness_config_detects_rust_stack_defaults() {
    let temp_dir = TestTempDir::new("maestro-harness-test");
    fs::write(
        temp_dir.path().join("Cargo.toml"),
        "[package]\nname = \"demo\"\n",
    )
    .expect("invariant: Cargo.toml should be writable");

    let config = HarnessConfig::detect(temp_dir.path());

    assert_eq!(config.schema_version, "maestro.harness.v1");
    assert_eq!(config.stack.kind, StackKind::Rust);
    assert_eq!(config.stack.detected_by, vec!["Cargo.toml"]);
    assert_eq!(
        config.stack.verify,
        vec!["cargo build", "cargo test", "cargo clippy -- -D warnings"]
    );
}

#[test]
fn harness_config_detects_python_stdlib_test_defaults() {
    let temp_dir = TestTempDir::new("maestro-harness-test");
    fs::write(
        temp_dir.path().join("pyproject.toml"),
        "[project]\nname = \"demo\"\n",
    )
    .expect("invariant: pyproject.toml should be writable");
    fs::create_dir(temp_dir.path().join("tests"))
        .expect("invariant: tests dir should be creatable");

    let config = HarnessConfig::detect(temp_dir.path());

    assert_eq!(config.stack.kind, StackKind::Python);
    assert_eq!(config.stack.detected_by, vec!["pyproject.toml"]);
    assert_eq!(
        config.stack.verify,
        vec!["python -m unittest discover -s tests"]
    );
}

#[test]
fn stack_detection_uses_generic_unknown_stack_fallback() {
    let temp_dir = TestTempDir::new("maestro-harness-test");

    let stack = detect_stack(temp_dir.path());

    assert_eq!(stack.kind, StackKind::Generic);
    assert!(stack.detected_by.is_empty());
    assert!(stack.verify.is_empty());
}

#[test]
fn harness_yaml_is_valid_yaml() {
    let temp_dir = TestTempDir::new("maestro-harness-test");
    let config = HarnessConfig::detect(temp_dir.path());
    let harness = harness_yml(&config).expect("invariant: harness config should serialize");

    assert!(harness.contains("schema_version: maestro.harness.v1"));
    assert!(harness.contains("kind: generic"));
    assert!(harness.contains("escalation:"));
    assert!(harness.contains("enabled: true"));
    assert!(harness.contains("warn_after: 2"));
    assert!(harness.contains("act_after: 3"));
}

#[test]
fn maestro_paths_include_phase_2_artifact_locations() {
    let temp_dir = TestTempDir::new("maestro-harness-test");
    let paths = MaestroPaths::new(temp_dir.path().to_path_buf());

    assert_eq!(paths.tasks_dir(), temp_dir.path().join(".maestro/tasks"));
    assert_eq!(paths.runs_dir(), temp_dir.path().join(".maestro/runs"));
    assert_eq!(
        paths.install_lock_file(),
        temp_dir.path().join(".maestro/install-lock.yaml")
    );
}
