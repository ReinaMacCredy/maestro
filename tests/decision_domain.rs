mod support;

use std::fs;

use maestro::decisions::schema::{DecisionStatus, DecisionStore};
use maestro::decisions::template::decision_markdown;
use maestro::domain::decisions;
use maestro::foundation::core::fs::ensure_dir;
use maestro::foundation::core::paths::MaestroPaths;
use support::TestTempDir;

fn read_global_store(paths: &MaestroPaths) -> DecisionStore {
    let yaml = fs::read_to_string(paths.decisions_file())
        .expect("invariant: decisions.yaml should be readable");
    serde_yaml::from_str(&yaml).expect("invariant: decisions.yaml should parse")
}

#[test]
fn create_open_persists_first_global_decision() {
    let temp = TestTempDir::new("maestro-decision-create");
    let paths = MaestroPaths::new(temp.path());

    let report = decisions::create_open(
        &paths,
        "Use single HARNESS.md",
        Some("too many files"),
        None,
    )
    .expect("invariant: create should succeed");

    assert_eq!(report.record.id, "decision-001");
    assert_eq!(report.record.status, DecisionStatus::Open);
    assert_eq!(report.record.context.as_deref(), Some("too many files"));
    assert!(paths.decisions_file().is_file());
    assert!(
        !paths
            .decisions_dir()
            .join("decision-001-use-single-harness-md.md")
            .is_file(),
        "structured creation must not write frozen legacy markdown"
    );
    let store = read_global_store(&paths);
    assert_eq!(store.decisions, vec![report.record]);
}

#[test]
fn create_open_auto_increments_from_structured_store() {
    let temp = TestTempDir::new("maestro-decision-increment");
    let paths = MaestroPaths::new(temp.path());

    let first = decisions::create_open(&paths, "First decision", None, None)
        .expect("invariant: first create should succeed");
    let second = decisions::create_open(&paths, "Second decision", None, None)
        .expect("invariant: second create should succeed");

    assert_eq!(first.record.id, "decision-001");
    assert_eq!(second.record.id, "decision-002");
    let store = read_global_store(&paths);
    assert_eq!(store.decisions.len(), 2);
}

#[test]
fn create_open_skips_leaked_allocation_marker() {
    let temp = TestTempDir::new("maestro-decision-alloc-marker");
    let paths = MaestroPaths::new(temp.path());
    ensure_dir(paths.decisions_dir().join(".alloc-decision-001"))
        .expect("invariant: leaked allocation marker should be creatable");

    let report = decisions::create_open(&paths, "After leaked marker", None, None)
        .expect("invariant: create should succeed");

    assert_eq!(report.record.id, "decision-002");
    let store = read_global_store(&paths);
    assert_eq!(store.decisions[0].id, "decision-002");
}

#[test]
fn create_open_increments_past_a_seeded_legacy_gap() {
    let temp = TestTempDir::new("maestro-decision-gap");
    let paths = MaestroPaths::new(temp.path());

    decisions::create_open(&paths, "First decision", None, None)
        .expect("invariant: create should succeed");
    ensure_dir(paths.decisions_dir()).expect("invariant: legacy dir should be creatable");
    fs::write(
        paths.decisions_dir().join("decision-007-existing.md"),
        decision_markdown(7, "Existing"),
    )
    .expect("invariant: seed decision should be writable");

    let next = decisions::create_open(&paths, "After the gap", None, None)
        .expect("invariant: create should succeed");
    assert_eq!(
        next.record.id, "decision-008",
        "next number must be max existing + 1"
    );
}

#[test]
fn create_open_rejects_empty_slug_title() {
    let temp = TestTempDir::new("maestro-decision-empty");
    let paths = MaestroPaths::new(temp.path());

    let err = decisions::create_open(&paths, "   ", None, None)
        .expect_err("empty-slug title must be rejected");
    assert!(
        err.to_string()
            .contains("at least one ASCII letter or digit"),
        "{err}"
    );
    assert!(
        !paths.decisions_file().is_file(),
        "no structured store must be written"
    );
}

#[test]
fn decision_exists_propagates_structured_store_errors() {
    let temp = TestTempDir::new("maestro-decision-exists-error");
    let paths = MaestroPaths::new(temp.path());
    ensure_dir(
        paths
            .decisions_file()
            .parent()
            .expect("invariant: decisions file should have parent"),
    )
    .expect("invariant: decisions parent should be creatable");
    fs::write(
        paths.decisions_file(),
        "schema_version: wrong.version\ndecisions: []\n",
    )
    .expect("invariant: invalid decisions store should be writable");

    let error = decisions::decision_exists(&paths, "decision-001")
        .expect_err("schema mismatch must not collapse to false");
    assert!(
        format!("{error:#}").contains("schema mismatch"),
        "{error:#}"
    );
}
