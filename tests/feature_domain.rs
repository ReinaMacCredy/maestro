mod support;

use std::fs;
use std::path::Path;

use maestro::core::fs::ensure_dir;
use maestro::core::paths::MaestroPaths;
use maestro::domain::feature;
use support::TestTempDir;

fn write_registry(paths: &MaestroPaths, contents: &str) {
    let dir = paths.features_dir();
    ensure_dir(&dir).expect("invariant: features dir should be creatable");
    fs::write(dir.join("features.yaml"), contents).expect("invariant: registry should be writable");
}

fn write_task(tasks_dir: &Path, id: &str, feature_id: &str, state: &str) {
    let dir = tasks_dir.join(id);
    ensure_dir(&dir).expect("invariant: task directory should be creatable");
    fs::write(
        dir.join("task.yaml"),
        format!("feature_id: {feature_id}\nstate: {state}\n"),
    )
    .expect("invariant: task.yaml should be writable");
}

const BAD_REGISTRY: &str = "schema_version: maestro.galaxy.v9\nfeatures: []\n";

#[test]
fn create_generates_slug_id_and_persists() {
    let temp = TestTempDir::new("maestro-feature-create");
    let paths = MaestroPaths::new(temp.path());

    feature::create(&paths, "Billing CSV Export").expect("invariant: create should succeed");

    let views = feature::list(&paths).expect("invariant: list should succeed");
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].id, "billing-csv-export");
    assert_eq!(views[0].title, "Billing CSV Export");
    assert_eq!(views[0].status, feature::FeatureStatus::Proposed);
}

#[test]
fn create_rejects_empty_title() {
    let temp = TestTempDir::new("maestro-feature-empty");
    let paths = MaestroPaths::new(temp.path());

    let error = feature::create(&paths, "   ").expect_err("invariant: empty title must error");
    assert!(error.to_string().contains("ASCII"));
}

#[test]
fn create_rejects_duplicate_id() {
    let temp = TestTempDir::new("maestro-feature-dup");
    let paths = MaestroPaths::new(temp.path());

    feature::create(&paths, "Billing CSV").expect("invariant: first create should succeed");
    let error =
        feature::create(&paths, "Billing CSV").expect_err("invariant: duplicate id must error");
    assert!(error.to_string().contains("already exists"));
}

#[test]
fn set_status_mutates_and_bumps_updated_at() {
    let temp = TestTempDir::new("maestro-feature-status");
    let paths = MaestroPaths::new(temp.path());

    feature::create(&paths, "Billing CSV").expect("invariant: create should succeed");
    let before = feature::show(&paths, "billing-csv").expect("invariant: show should succeed");

    feature::set_status(&paths, "billing-csv", feature::FeatureStatus::Shipped)
        .expect("invariant: set_status should succeed");

    let after = feature::show(&paths, "billing-csv").expect("invariant: show should succeed");
    assert_eq!(after.status, feature::FeatureStatus::Shipped);
    assert!(
        after.updated_at >= before.updated_at,
        "updated_at should not regress"
    );
}

#[test]
fn set_status_errors_on_missing_feature() {
    let temp = TestTempDir::new("maestro-feature-missing");
    let paths = MaestroPaths::new(temp.path());

    let error = feature::set_status(&paths, "nope", feature::FeatureStatus::Shipped)
        .expect_err("invariant: missing feature must error");
    assert!(error.to_string().contains("not found"));
}

#[test]
fn list_joins_task_counts() {
    let temp = TestTempDir::new("maestro-feature-counts");
    let paths = MaestroPaths::new(temp.path());

    feature::create(&paths, "Billing CSV").expect("invariant: create should succeed");
    let tasks_dir = paths.tasks_dir();
    write_task(&tasks_dir, "task-001", "billing-csv", "verified");
    write_task(&tasks_dir, "task-002", "billing-csv", "ready");

    let views = feature::list(&paths).expect("invariant: list should succeed");
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].counts.total, 2);
    assert_eq!(views[0].counts.verified, 1);
}

#[test]
fn strict_list_errors_on_incompatible_registry() {
    let temp = TestTempDir::new("maestro-feature-strict");
    let paths = MaestroPaths::new(temp.path());
    write_registry(&paths, BAD_REGISTRY);

    let error = feature::list(&paths).expect_err("invariant: strict read must error on bad schema");
    assert!(error.to_string().contains("schema"));
}

#[test]
fn strict_show_errors_on_incompatible_registry() {
    let temp = TestTempDir::new("maestro-feature-strict-show");
    let paths = MaestroPaths::new(temp.path());
    write_registry(&paths, BAD_REGISTRY);

    let error = feature::show(&paths, "anything")
        .expect_err("invariant: strict read must error on bad schema");
    assert!(error.to_string().contains("schema"));
}

#[test]
fn tolerant_titles_degrade_to_empty_on_incompatible_registry() {
    let temp = TestTempDir::new("maestro-feature-tolerant");
    let paths = MaestroPaths::new(temp.path());
    write_registry(&paths, BAD_REGISTRY);

    let titles = feature::titles(&paths);
    assert!(
        titles.is_empty(),
        "tolerant titles must degrade to empty for a bad registry"
    );
}

#[test]
fn tolerant_titles_degrade_to_empty_on_unparseable_registry() {
    let temp = TestTempDir::new("maestro-feature-tolerant-parse");
    let paths = MaestroPaths::new(temp.path());
    write_registry(&paths, "this: is: not: valid: yaml: [");

    let titles = feature::titles(&paths);
    assert!(
        titles.is_empty(),
        "tolerant titles must degrade on parse error"
    );
}

#[test]
fn tolerant_titles_return_id_to_title_map() {
    let temp = TestTempDir::new("maestro-feature-titles");
    let paths = MaestroPaths::new(temp.path());

    feature::create(&paths, "Billing CSV").expect("invariant: create should succeed");

    let titles = feature::titles(&paths);
    assert_eq!(
        titles.get("billing-csv").map(String::as_str),
        Some("Billing CSV")
    );
}

#[test]
fn diagnose_reports_count_on_compatible_registry() {
    let temp = TestTempDir::new("maestro-feature-diag-ok");
    let paths = MaestroPaths::new(temp.path());

    feature::create(&paths, "Billing CSV").expect("invariant: create should succeed");

    let diagnostic = feature::diagnose(&paths);
    assert_eq!(diagnostic.found, Ok(("maestro.feature.v1".to_string(), 1)));
    assert_eq!(
        diagnostic.compatibility,
        Some(maestro::core::schema::Compat::Exact)
    );
}

#[test]
fn diagnose_reports_absent_registry_as_error_data() {
    let temp = TestTempDir::new("maestro-feature-diag-absent");
    let paths = MaestroPaths::new(temp.path());

    let diagnostic = feature::diagnose(&paths);
    assert!(
        diagnostic.found.is_err(),
        "an absent registry must report as error data so doctor flags it"
    );
    assert_eq!(
        diagnostic.compatibility, None,
        "an absent registry has no compatibility verdict"
    );
}

#[test]
fn diagnose_reports_incompatible_version_as_data() {
    let temp = TestTempDir::new("maestro-feature-diag-bad");
    let paths = MaestroPaths::new(temp.path());
    write_registry(&paths, BAD_REGISTRY);

    let diagnostic = feature::diagnose(&paths);
    assert_eq!(diagnostic.found, Ok(("maestro.galaxy.v9".to_string(), 0)));
    assert_eq!(
        diagnostic.compatibility,
        Some(maestro::core::schema::Compat::Incompatible)
    );
}

#[test]
fn diagnose_reports_parse_error_as_data() {
    let temp = TestTempDir::new("maestro-feature-diag-parse");
    let paths = MaestroPaths::new(temp.path());
    write_registry(&paths, "this: is: not: valid: yaml: [");

    let diagnostic = feature::diagnose(&paths);
    assert!(
        diagnostic.found.is_err(),
        "diagnose should report parse error as data"
    );
    assert_eq!(
        diagnostic.compatibility, None,
        "a parse failure has no compatibility verdict"
    );
}

#[test]
fn status_label_renders_snake_case() {
    assert_eq!(
        feature::status_label(&feature::FeatureStatus::Proposed),
        "proposed"
    );
    assert_eq!(
        feature::status_label(&feature::FeatureStatus::InProgress),
        "in_progress"
    );
    assert_eq!(
        feature::status_label(&feature::FeatureStatus::Shipped),
        "shipped"
    );
    assert_eq!(
        feature::status_label(&feature::FeatureStatus::Cancelled),
        "cancelled"
    );
}
