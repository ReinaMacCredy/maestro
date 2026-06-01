mod support;

use std::fs;

use maestro::decisions::template::{decision_file_name, decision_markdown};
use maestro::feature::query::{FeatureTaskCounts, count_tasks_for_feature};
use maestro::feature::schema::FeatureRecord;
use maestro::foundation::core::fs::ensure_dir;
use maestro::foundation::core::paths::MaestroPaths;
use support::TestTempDir;

#[test]
fn created_feature_record_carries_v1_schema_version() {
    let temp_dir = TestTempDir::new("maestro-feature-schema");
    let paths = MaestroPaths::new(temp_dir.path());

    maestro::feature::create(&paths, "Billing CSV export")
        .expect("invariant: create should succeed");

    let yaml = fs::read_to_string(
        paths
            .features_dir()
            .join("billing-csv-export")
            .join("feature.yaml"),
    )
    .expect("invariant: per-feature feature.yaml should be readable");
    assert!(yaml.contains("schema_version: maestro.feature.v1"));
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
fn feature_task_counts_are_computed_from_task_yaml_files() {
    let temp_dir = TestTempDir::new("maestro-feature-test");
    let tasks_dir = temp_dir.path().join(".maestro/tasks");
    write_task(&tasks_dir, "task-001", "billing-csv-export", "verified");
    write_task(&tasks_dir, "task-002", "billing-csv-export", "ready");
    write_task(&tasks_dir, "task-003", "other", "verified");

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

fn write_task(tasks_dir: &std::path::Path, id: &str, feature_id: &str, state: &str) {
    let dir = tasks_dir.join(id);
    ensure_dir(&dir).expect("invariant: task directory should be creatable");
    fs::write(
        dir.join("task.yaml"),
        format!("id: {id}\nfeature_id: {feature_id}\nstate: {state}\n"),
    )
    .expect("invariant: task.yaml should be writable");
}
