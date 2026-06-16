mod support;

use std::fs;

use maestro::domain::decisions::template::{decision_file_name, decision_markdown};
use maestro::domain::feature::query::{FeatureTaskCounts, count_tasks_for_feature};
use maestro::domain::feature::schema::FeatureRecord;
use maestro::foundation::core::fs::ensure_dir;
use maestro::foundation::core::paths::MaestroPaths;
use support::TestTempDir;

#[test]
fn created_feature_record_carries_v2_schema_version() {
    let temp_dir = TestTempDir::new("maestro-feature-schema");
    let paths = MaestroPaths::new(temp_dir.path());

    maestro::domain::feature::create(&paths, "Billing CSV export", None)
        .expect("invariant: create should succeed");

    // A feature is now a `feature`-typed card at `.maestro/cards/<slug>/card.yaml`;
    // the verbatim v2 feature record rides under `extra`, not a per-feature
    // `feature.yaml` directory.
    let yaml = fs::read_to_string(
        paths
            .cards_dir()
            .join("billing-csv-export")
            .join("card.yaml"),
    )
    .expect("invariant: feature card.yaml should be readable");
    assert!(yaml.contains("schema_version: maestro.feature.v2"));
    assert!(yaml.contains("status: proposed"));
}

#[test]
fn feature_records_do_not_store_task_counts() {
    let record = FeatureRecord::proposed(
        "billing-csv-export",
        "Billing CSV export",
        "2026-05-25T08:00:00Z",
    );

    let yaml = serde_yaml::to_string(&record).expect("invariant: record should serialize");

    assert!(!yaml.contains("task_count"));
    assert!(!yaml.contains("tasks:"));
}

#[test]
fn feature_task_counts_are_computed_from_task_cards() {
    let temp_dir = TestTempDir::new("maestro-feature-test");
    // `count_tasks_for_feature` takes the `.maestro/tasks` anchor and resolves the
    // repo root two parents up; the scan it drives now reads `Task`-typed cards
    // from `.maestro/cards/`, grouping by `card.parent`.
    let tasks_dir = temp_dir.path().join(".maestro/tasks");
    write_feature_task(
        temp_dir.path(),
        "task-001",
        "billing-csv-export",
        "verified",
    );
    write_feature_task(temp_dir.path(), "task-002", "billing-csv-export", "ready");
    write_feature_task(temp_dir.path(), "task-003", "other", "verified");

    let counts = count_tasks_for_feature(&tasks_dir, "billing-csv-export")
        .expect("invariant: feature task counts should compute");

    assert_eq!(
        counts,
        FeatureTaskCounts {
            total: 2,
            verified: 1
        }
    );
}

#[test]
fn decision_file_name_uses_padded_number_and_slug() {
    assert_eq!(
        decision_file_name(7, "Use single HARNESS.md instead of three adapter files"),
        "decision-007-use-single-harness-md-instead-of-three-adapter-files.md"
    );
}

#[test]
fn decision_markdown_matches_section_7_4_template() {
    let markdown = decision_markdown(1, "Use single HARNESS.md instead of three adapter files");

    assert!(
        markdown
            .starts_with("# decision-001: Use single HARNESS.md instead of three adapter files")
    );
    assert!(markdown.contains("## Status\nAccepted"));
    assert!(markdown.contains("## Context\nWhy this decision exists."));
    assert!(markdown.contains("## Decision\nWhat we decided."));
    assert!(markdown.contains("## Alternatives considered"));
    assert!(markdown.contains("## Consequences"));
    assert!(markdown.contains("## Linked tasks"));
}

/// Write a `Task`-typed card at `.maestro/cards/<id>/card.yaml` owned by
/// `feature_id` via `card.parent` -- the flat layout the count scan reads after
/// the card cutover. The verbatim task record rides under `extra`, mirroring the
/// shape a real `task create` mints; only `id`/`title`/`state` vary per call.
fn write_feature_task(repo: &std::path::Path, id: &str, feature_id: &str, state: &str) {
    let dir = repo.join(".maestro/cards").join(id);
    ensure_dir(&dir).expect("invariant: card directory should be creatable");
    fs::write(
        dir.join("card.yaml"),
        format!(
            "schema_version: maestro.card.v1\nid: {id}\ntype: task\ntitle: {id}\nstatus: {state}\nparent: {feature_id}\ncreated_at: \"2026-06-06T00:00:00.000Z\"\nupdated_at: \"2026-06-06T00:00:00.000Z\"\nextra:\n  schema_version: maestro.task.v2\n  id: {id}\n  title: {id}\n  state: {state}\n  acceptance_locked: false\n  acceptance: {{}}\n  verification: {{}}\n  created_at: \"2026-06-06T00:00:00.000Z\"\n  updated_at: \"2026-06-06T00:00:00.000Z\"\n"
        ),
    )
    .expect("invariant: card.yaml should be writable");
}
