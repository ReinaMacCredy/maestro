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
