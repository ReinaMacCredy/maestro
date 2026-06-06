use std::error::Error;
use std::path::PathBuf;

use maestro::foundation::core::error::MaestroError;
use maestro::foundation::core::schema::{
    ACCEPTANCE_SCHEMA_VERSION, ALL_SCHEMA_VERSIONS, BACKLOG_SCHEMA_VERSION, Compat,
    EVENT_SCHEMA_VERSION, FEATURE_SCHEMA_VERSION, GLOBAL_SKILLS_LOCK_SCHEMA_VERSION,
    HARNESS_SCHEMA_VERSION, INSTALL_LOCK_SCHEMA_VERSION, RUN_EVIDENCE_SCHEMA_VERSION,
    RUN_SCHEMA_VERSION, TASK_SCHEMA_VERSION, VERIFICATION_RESTORE_SCHEMA_VERSION,
    VERIFICATION_SCHEMA_VERSION, classify,
};

#[test]
fn all_active_schema_versions_are_declared_once() {
    assert_eq!(
        ALL_SCHEMA_VERSIONS,
        &[
            HARNESS_SCHEMA_VERSION,
            FEATURE_SCHEMA_VERSION,
            TASK_SCHEMA_VERSION,
            RUN_SCHEMA_VERSION,
            EVENT_SCHEMA_VERSION,
            RUN_EVIDENCE_SCHEMA_VERSION,
            INSTALL_LOCK_SCHEMA_VERSION,
            GLOBAL_SKILLS_LOCK_SCHEMA_VERSION,
            BACKLOG_SCHEMA_VERSION,
        ]
    );
    assert_eq!(ALL_SCHEMA_VERSIONS.len(), 9);
}

#[test]
fn schema_constants_match_current_artifact_contract() {
    assert_eq!(HARNESS_SCHEMA_VERSION, "maestro.harness.v1");
    assert_eq!(FEATURE_SCHEMA_VERSION, "maestro.feature.v2");
    assert_eq!(TASK_SCHEMA_VERSION, "maestro.task.v2");
    assert_eq!(RUN_SCHEMA_VERSION, "maestro.run.v1");
    assert_eq!(EVENT_SCHEMA_VERSION, "maestro.event.v1");
    assert_eq!(RUN_EVIDENCE_SCHEMA_VERSION, "maestro.run_evidence.v1");
    assert_eq!(VERIFICATION_SCHEMA_VERSION, "maestro.verification.v1");
    assert_eq!(ACCEPTANCE_SCHEMA_VERSION, "maestro.acceptance.v1");
    assert_eq!(INSTALL_LOCK_SCHEMA_VERSION, "maestro.install_lock.v1");
    assert_eq!(
        GLOBAL_SKILLS_LOCK_SCHEMA_VERSION,
        "maestro.global_skills_lock.v1"
    );
    assert_eq!(BACKLOG_SCHEMA_VERSION, "maestro.backlog.v1");
    assert_eq!(
        VERIFICATION_RESTORE_SCHEMA_VERSION,
        "maestro.verification.restore.v1"
    );
}

#[test]
fn classify_maps_exact_match_and_treats_everything_else_as_incompatible() {
    // Exact: found equals expected for the same artifact.
    assert_eq!(
        classify(FEATURE_SCHEMA_VERSION, FEATURE_SCHEMA_VERSION),
        Compat::Exact
    );

    // This is a clean-rewrite binary with no migration path: every non-exact
    // version is Incompatible, regardless of family, generation, or shape.
    // An older generation of a known family stops hard (no migration exists).
    assert_eq!(
        classify("maestro.feature.v0", FEATURE_SCHEMA_VERSION),
        Compat::Incompatible
    );
    assert_eq!(
        classify("maestro.task.v0", TASK_SCHEMA_VERSION),
        Compat::Incompatible
    );
    // A missing or empty schema_version is incompatible.
    assert_eq!(
        classify("<missing>", HARNESS_SCHEMA_VERSION),
        Compat::Incompatible
    );
    assert_eq!(
        classify("", ACCEPTANCE_SCHEMA_VERSION),
        Compat::Incompatible
    );
    // An unknown family or unparseable tag is incompatible.
    assert_eq!(
        classify("maestro.galaxy.v9", FEATURE_SCHEMA_VERSION),
        Compat::Incompatible
    );
    assert_eq!(
        classify("totally-bogus", FEATURE_SCHEMA_VERSION),
        Compat::Incompatible
    );
    // A newer generation than this binary understands is incompatible.
    assert_eq!(
        classify("maestro.feature.v3", FEATURE_SCHEMA_VERSION),
        Compat::Incompatible
    );
    // The install-lock gate stays hard.
    assert_eq!(
        classify("maestro.install_lock.v0", INSTALL_LOCK_SCHEMA_VERSION),
        Compat::Incompatible
    );
}

#[test]
fn typed_errors_implement_display_debug_and_error() {
    let error = MaestroError::SchemaMismatch {
        artifact: "task.yaml".to_string(),
        expected: TASK_SCHEMA_VERSION,
        found: "maestro.task.v0".to_string(),
    };

    assert_eq!(
        error.to_string(),
        "schema mismatch for task.yaml: expected maestro.task.v2, found maestro.task.v0"
    );
    assert!(
        format!("{error:?}").contains("SchemaMismatch"),
        "Debug output should include the variant name"
    );

    let as_error: &dyn Error = &error;
    assert_eq!(
        as_error.to_string(),
        "schema mismatch for task.yaml: expected maestro.task.v2, found maestro.task.v0"
    );
}

#[test]
fn path_errors_include_the_relevant_path() {
    let path = PathBuf::from("/tmp/outside");
    let error = MaestroError::OutsideRepository { path };

    assert_eq!(
        error.to_string(),
        "operation would write outside repository root: /tmp/outside"
    );
}
