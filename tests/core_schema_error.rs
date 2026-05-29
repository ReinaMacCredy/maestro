use std::error::Error;
use std::path::PathBuf;

use maestro::foundation::core::error::MaestroError;
use maestro::foundation::core::schema::{
    classify, Compat, ACCEPTANCE_SCHEMA_VERSION, ALL_SCHEMA_VERSIONS, BACKLOG_SCHEMA_VERSION,
    EVENT_SCHEMA_VERSION, FEATURE_SCHEMA_VERSION, HARNESS_SCHEMA_VERSION,
    INSTALL_LOCK_SCHEMA_VERSION, RUN_EVIDENCE_SCHEMA_VERSION, RUN_SCHEMA_VERSION,
    TASK_SCHEMA_VERSION, VERIFICATION_RESTORE_SCHEMA_VERSION, VERIFICATION_SCHEMA_VERSION,
};

#[test]
fn all_v1_schema_versions_are_declared_once() {
    assert_eq!(
        ALL_SCHEMA_VERSIONS,
        &[
            HARNESS_SCHEMA_VERSION,
            FEATURE_SCHEMA_VERSION,
            TASK_SCHEMA_VERSION,
            RUN_SCHEMA_VERSION,
            EVENT_SCHEMA_VERSION,
            RUN_EVIDENCE_SCHEMA_VERSION,
            VERIFICATION_SCHEMA_VERSION,
            ACCEPTANCE_SCHEMA_VERSION,
            INSTALL_LOCK_SCHEMA_VERSION,
            BACKLOG_SCHEMA_VERSION,
            VERIFICATION_RESTORE_SCHEMA_VERSION,
        ]
    );
    assert_eq!(ALL_SCHEMA_VERSIONS.len(), 11);
}

#[test]
fn schema_constants_match_spec_section_37() {
    assert_eq!(HARNESS_SCHEMA_VERSION, "maestro.harness.v1");
    assert_eq!(FEATURE_SCHEMA_VERSION, "maestro.feature.v1");
    assert_eq!(TASK_SCHEMA_VERSION, "maestro.task.v1");
    assert_eq!(RUN_SCHEMA_VERSION, "maestro.run.v1");
    assert_eq!(EVENT_SCHEMA_VERSION, "maestro.event.v1");
    assert_eq!(RUN_EVIDENCE_SCHEMA_VERSION, "maestro.run_evidence.v1");
    assert_eq!(VERIFICATION_SCHEMA_VERSION, "maestro.verification.v1");
    assert_eq!(ACCEPTANCE_SCHEMA_VERSION, "maestro.acceptance.v1");
    assert_eq!(INSTALL_LOCK_SCHEMA_VERSION, "maestro.install_lock.v1");
    assert_eq!(BACKLOG_SCHEMA_VERSION, "maestro.backlog.v1");
    assert_eq!(
        VERIFICATION_RESTORE_SCHEMA_VERSION,
        "maestro.verification.restore.v1"
    );
}

#[test]
fn classify_maps_exact_needs_migration_and_incompatible() {
    // Exact: found equals expected for the same artifact.
    assert_eq!(
        classify(FEATURE_SCHEMA_VERSION, FEATURE_SCHEMA_VERSION),
        Compat::Exact
    );

    // NeedsMigration: an older generation of a family the bundled
    // v0_106_to_v0_8 migration converts.
    assert_eq!(
        classify("maestro.feature.v0", FEATURE_SCHEMA_VERSION),
        Compat::NeedsMigration
    );
    assert_eq!(
        classify("maestro.task.v0", TASK_SCHEMA_VERSION),
        Compat::NeedsMigration
    );
    // v0.106.1 legacy artifacts carry no schema_version; the readers surface
    // that as the "<missing>" marker, which still routes to migrate.
    assert_eq!(
        classify("<missing>", HARNESS_SCHEMA_VERSION),
        Compat::NeedsMigration
    );
    assert_eq!(
        classify("", ACCEPTANCE_SCHEMA_VERSION),
        Compat::NeedsMigration
    );

    // Incompatible: an unknown / unparseable version stops, even when shaped
    // like a maestro tag but from an unknown family.
    assert_eq!(
        classify("maestro.galaxy.v9", FEATURE_SCHEMA_VERSION),
        Compat::Incompatible
    );
    assert_eq!(
        classify("totally-bogus", FEATURE_SCHEMA_VERSION),
        Compat::Incompatible
    );
    // A newer generation than this binary understands is not migratable.
    assert_eq!(
        classify("maestro.feature.v2", FEATURE_SCHEMA_VERSION),
        Compat::Incompatible
    );

    // Non-migratable family: even an older generation stops hard, because no
    // migration module produces it. This keeps the install-lock gate hard.
    assert_eq!(
        classify("maestro.install_lock.v0", INSTALL_LOCK_SCHEMA_VERSION),
        Compat::Incompatible
    );
    assert_eq!(
        classify("<missing>", INSTALL_LOCK_SCHEMA_VERSION),
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
        "schema mismatch for task.yaml: expected maestro.task.v1, found maestro.task.v0"
    );
    assert!(
        format!("{error:?}").contains("SchemaMismatch"),
        "Debug output should include the variant name"
    );

    let as_error: &dyn Error = &error;
    assert_eq!(
        as_error.to_string(),
        "schema mismatch for task.yaml: expected maestro.task.v1, found maestro.task.v0"
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
