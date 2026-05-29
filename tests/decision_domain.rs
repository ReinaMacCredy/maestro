mod support;

use std::fs;

use maestro::core::paths::MaestroPaths;
use maestro::decisions::template::decision_markdown;
use maestro::domain::decisions;
use support::TestTempDir;

#[test]
fn create_persists_first_decision_with_template() {
    let temp = TestTempDir::new("maestro-decision-create");
    let paths = MaestroPaths::new(temp.path());

    let number = decisions::create(&paths, "Use single HARNESS.md")
        .expect("invariant: create should succeed");
    assert_eq!(number, 1);

    let file = paths
        .decisions_dir()
        .join("decision-001-use-single-harness-md.md");
    assert!(file.is_file(), "create must persist the decision file");
    assert_eq!(
        fs::read_to_string(&file).expect("invariant: decision file should be readable"),
        decision_markdown(1, "Use single HARNESS.md")
    );
}

#[test]
fn create_auto_increments_from_highest_existing() {
    let temp = TestTempDir::new("maestro-decision-increment");
    let paths = MaestroPaths::new(temp.path());

    let first = decisions::create(&paths, "First decision")
        .expect("invariant: first create should succeed");
    let second = decisions::create(&paths, "Second decision")
        .expect("invariant: second create should succeed");

    assert_eq!(first, 1);
    assert_eq!(second, 2);
    assert!(paths
        .decisions_dir()
        .join("decision-002-second-decision.md")
        .is_file());
}

#[test]
fn create_increments_past_a_seeded_gap() {
    let temp = TestTempDir::new("maestro-decision-gap");
    let paths = MaestroPaths::new(temp.path());

    decisions::create(&paths, "First decision").expect("invariant: create should succeed");
    fs::write(
        paths.decisions_dir().join("decision-007-existing.md"),
        decision_markdown(7, "Existing"),
    )
    .expect("invariant: seed decision should be writable");

    let next =
        decisions::create(&paths, "After the gap").expect("invariant: create should succeed");
    assert_eq!(next, 8, "next number must be max existing + 1");
}

#[test]
fn create_does_not_validate_empty_title() {
    let temp = TestTempDir::new("maestro-decision-empty");
    let paths = MaestroPaths::new(temp.path());

    let number =
        decisions::create(&paths, "   ").expect("invariant: empty title must silently slugify");
    assert_eq!(number, 1);
    assert!(
        paths.decisions_dir().join("decision-001-.md").is_file(),
        "empty title slugifies to an empty slug, matching the un-validated baseline"
    );
}
