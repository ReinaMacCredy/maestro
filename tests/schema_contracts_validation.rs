//! WS5 kernel gate: the embedded schema packs must validate against the Rust
//! constants, and the family classification seam must answer the three cases
//! the locked D6.1 posture defines (supported, routed legacy, unknown).

use maestro::domain::schema_contracts::{VersionClass, pack, packs};

#[test]
fn embedded_schema_packs_validate_cleanly() {
    let violations = maestro::domain::schema_contracts::validate::violations();
    assert!(
        violations.is_empty(),
        "embedded schema packs drifted from the Rust constants:\n{}",
        violations.join("\n")
    );
}

#[test]
fn catalog_serves_every_artifact_matrix_family() {
    let families: Vec<&str> = packs().iter().map(|pack| pack.family).collect();
    assert_eq!(
        families,
        [
            "backlog",
            "card",
            "decision",
            "feature",
            "harness",
            "install",
            "progress",
            "proof",
            "run-event",
            "run-evidence",
            "task",
        ],
        "the catalog must cover exactly the 11 artifact-matrix families"
    );
}

#[test]
fn task_versions_classify_per_the_supported_matrix() {
    let task = pack("task").expect("task pack ships");
    assert_eq!(task.classify("maestro.task.v2"), VersionClass::Supported);
    match task.classify("maestro.task.v1") {
        VersionClass::Legacy { route } => assert!(
            route.contains("migrate-v2"),
            "the legacy route must point at the explicit migrate verb, got {route}"
        ),
        other => panic!("maestro.task.v1 must classify as routed legacy, got {other:?}"),
    }
    assert_eq!(task.classify("maestro.task.v9"), VersionClass::Unknown);
    assert_eq!(task.classify("maestro.galaxy.v1"), VersionClass::Unknown);
}

#[test]
fn retired_run_version_is_not_readable_anywhere() {
    for pack in packs() {
        assert_eq!(
            pack.classify("maestro.run.v1"),
            VersionClass::Unknown,
            "{} must not read or route the reserved maestro.run.v1",
            pack.family
        );
    }
}
