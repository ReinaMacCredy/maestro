mod support;

use std::fs;
use std::path::Path;

use maestro::domain::feature::{self, ContractAdditions, ContractEdits};
use maestro::foundation::core::fs::ensure_dir;
use maestro::foundation::core::paths::MaestroPaths;
use support::TestTempDir;

/// Write a feature record directly into its per-feature directory.
fn write_feature(paths: &MaestroPaths, id: &str, contents: &str) {
    let dir = paths.features_dir().join(id);
    ensure_dir(&dir).expect("invariant: feature dir should be creatable");
    fs::write(dir.join("feature.yaml"), contents)
        .expect("invariant: feature.yaml should be writable");
}

/// Write a minimal task.yaml carrying the fields the feature projection reads.
fn write_task(tasks_dir: &Path, id: &str, feature_id: &str, state: &str) {
    let dir = tasks_dir
        .parent()
        .expect("invariant: tasks dir should have parent")
        .join("features")
        .join(feature_id)
        .join("tasks")
        .join(format!("{id}-{id}"));
    ensure_dir(&dir).expect("invariant: task directory should be creatable");
    // A complete TaskRecord: the cancel cascade loads and transitions the child,
    // so a projection-only stub (id/feature_id/state) fails to deserialize.
    fs::write(
        dir.join("task.yaml"),
        format!(
            "schema_version: maestro.task.v2\nid: {id}\ntitle: {id}\nstate: {state}\nacceptance_locked: false\nverification: {{}}\ncreated_at: \"2026-06-06T00:00:00.000Z\"\nupdated_at: \"2026-06-06T00:00:00.000Z\"\n"
        ),
    )
    .expect("invariant: task.yaml should be writable");
}

const BAD_RECORD: &str = "schema_version: maestro.galaxy.v9\nid: billing-csv\ntitle: Billing CSV\nstatus: proposed\ncreated_at: \"1\"\nupdated_at: \"1\"\n";

/// Author a complete contract on a freshly-created Proposed feature, plus a
/// present baseline with no `[bl-NNN]` scenarios. The empty Scenario Matrix
/// declares no behavioral surface (QA C), so it satisfies the accept precondition
/// (F) and the ship coverage skip without entangling these transition-machinery
/// tests with slice authoring (the behavioral path lives in feature_qa_gate_integration).
fn author_contract(paths: &MaestroPaths, id: &str) {
    feature::set(
        paths,
        id,
        ContractEdits {
            acceptance: Some(vec!["exports a valid csv".to_string()]),
            affected_areas: Some(vec!["billing".to_string()]),
            ..Default::default()
        },
    )
    .expect("invariant: set should succeed on a proposed feature");

    let dir = paths.features_dir().join(id);
    ensure_dir(&dir).expect("invariant: feature dir should be creatable");
    fs::write(
        dir.join("qa.md"),
        "---\namend_log_position: 0\n---\n\n### QA Baseline Contract\n\n- Baseline gaps:\n  - none (no behavioral surface)\n",
    )
    .expect("invariant: qa.md should be writable");
}

fn verify_contract(paths: &MaestroPaths, id: &str) {
    feature::verify_feature(
        paths,
        id,
        Some(feature::FeatureProofUpdate::Explicit {
            ac_id: "ac-1".to_string(),
            evidence: "fixture evidence".to_string(),
        }),
    )
    .expect("invariant: proof should record");
    feature::verify_feature(paths, id, None).expect("invariant: sweep should succeed");
}

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
fn set_replaces_per_field_and_is_proposed_only() {
    let temp = TestTempDir::new("maestro-feature-set");
    let paths = MaestroPaths::new(temp.path());

    feature::create(&paths, "Billing CSV").expect("invariant: create should succeed");
    let view = feature::set(
        &paths,
        "billing-csv",
        ContractEdits {
            acceptance: Some(vec!["a".to_string(), "b".to_string()]),
            affected_areas: Some(vec!["billing".to_string()]),
            description: Some("export billing rows".to_string()),
            ..Default::default()
        },
    )
    .expect("invariant: set should succeed");
    assert_eq!(view.acceptance.len(), 2);
    assert_eq!(view.affected_areas, vec!["billing".to_string()]);

    // A second set replaces the whole field rather than appending.
    let view = feature::set(
        &paths,
        "billing-csv",
        ContractEdits {
            acceptance: Some(vec!["only".to_string()]),
            ..Default::default()
        },
    )
    .expect("invariant: set should replace");
    assert_eq!(view.acceptance, vec!["only".to_string()]);
    assert_eq!(view.affected_areas, vec!["billing".to_string()]);

    // Once accepted the contract is frozen; set is rejected.
    author_contract(&paths, "billing-csv");
    feature::accept(&paths, "billing-csv", false).expect("invariant: accept should succeed");
    let error = feature::set(
        &paths,
        "billing-csv",
        ContractEdits {
            acceptance: Some(vec!["late".to_string()]),
            ..Default::default()
        },
    )
    .expect_err("invariant: set must be rejected once frozen");
    assert!(error.to_string().contains("frozen"));
}

#[test]
fn set_rejects_a_blank_contract_value_so_it_cannot_satisfy_the_accept_gate() {
    let temp = TestTempDir::new("maestro-feature-set-blank");
    let paths = MaestroPaths::new(temp.path());

    feature::create(&paths, "Billing CSV").expect("invariant: create should succeed");
    // A `--acceptance ""` (or whitespace) would store a `[""]` that satisfies the
    // length-only accept gate while carrying no contract; it must be refused, like
    // the sibling `task set --check ""`.
    for value in ["", "   "] {
        let error = feature::set(
            &paths,
            "billing-csv",
            ContractEdits {
                acceptance: Some(vec![value.to_string()]),
                ..Default::default()
            },
        )
        .expect_err("invariant: a blank acceptance value must be rejected");
        assert!(
            error
                .to_string()
                .contains("must not be empty or whitespace"),
            "unexpected error: {error}"
        );
    }
}

#[test]
fn amend_rejects_a_blank_addition_value() {
    let temp = TestTempDir::new("maestro-feature-amend-blank");
    let paths = MaestroPaths::new(temp.path());

    feature::create(&paths, "Billing CSV").expect("invariant: create should succeed");
    author_contract(&paths, "billing-csv");
    feature::accept(&paths, "billing-csv", false).expect("invariant: accept should succeed");

    let error = feature::amend(
        &paths,
        "billing-csv",
        ContractAdditions {
            acceptance: vec!["   ".to_string()],
            ..Default::default()
        },
        "real reason",
    )
    .expect_err("invariant: a blank amend value must be rejected");
    assert!(
        error
            .to_string()
            .contains("must not be empty or whitespace"),
        "unexpected error: {error}"
    );
}

#[test]
fn verify_rejects_unknown_acceptance_evidence_kind() {
    let temp = TestTempDir::new("maestro-feature-evidence-kind");
    let paths = MaestroPaths::new(temp.path());
    feature::create(&paths, "Billing CSV").expect("invariant: create should succeed");
    feature::set(
        &paths,
        "billing-csv",
        ContractEdits {
            acceptance: Some(vec!["exports rows".to_string()]),
            affected_areas: Some(vec!["billing".to_string()]),
            ..Default::default()
        },
    )
    .expect("invariant: set should succeed");

    let path = paths
        .features_dir()
        .join("billing-csv")
        .join("feature.yaml");
    let mut raw: serde_yaml::Value = serde_yaml::from_str(
        &fs::read_to_string(&path).expect("invariant: feature should be readable"),
    )
    .expect("invariant: feature should parse");
    raw["acceptance_evidence"] = serde_yaml::Value::Sequence(vec![serde_yaml::Value::Mapping(
        [
            (
                serde_yaml::Value::String("ac_id".to_string()),
                serde_yaml::Value::String("ac-1".to_string()),
            ),
            (
                serde_yaml::Value::String("kind".to_string()),
                serde_yaml::Value::String("typo".to_string()),
            ),
            (
                serde_yaml::Value::String("text".to_string()),
                serde_yaml::Value::String("must not count".to_string()),
            ),
            (
                serde_yaml::Value::String("at".to_string()),
                serde_yaml::Value::String("2026-06-06T00:00:00.000Z".to_string()),
            ),
        ]
        .into_iter()
        .collect(),
    )]);
    fs::write(
        &path,
        serde_yaml::to_string(&raw).expect("invariant: feature should serialize"),
    )
    .expect("invariant: feature should be writable");

    let error = feature::verify_feature(&paths, "billing-csv", None)
        .expect_err("unknown evidence kind must not become explicit proof");
    assert!(format!("{error:#}").contains("kind"), "{error:#}");
}

#[test]
fn accept_gate_requires_acceptance_and_areas() {
    let temp = TestTempDir::new("maestro-feature-accept-gate");
    let paths = MaestroPaths::new(temp.path());

    feature::create(&paths, "Billing CSV").expect("invariant: create should succeed");
    let error = feature::accept(&paths, "billing-csv", false)
        .expect_err("invariant: accept must block on an incomplete contract");
    let message = error.to_string();
    assert!(message.contains("acceptance"));
    assert!(message.contains("affected_areas"));

    author_contract(&paths, "billing-csv");
    let report = feature::accept(&paths, "billing-csv", false).expect("invariant: accept succeeds");
    assert!(report.changed);
    assert_eq!(report.status, feature::FeatureStatus::Ready);
}

#[test]
fn accept_dry_run_previews_without_transitioning() {
    let temp = TestTempDir::new("maestro-feature-accept-dry");
    let paths = MaestroPaths::new(temp.path());

    feature::create(&paths, "Billing CSV").expect("invariant: create should succeed");
    let report =
        feature::accept(&paths, "billing-csv", true).expect("invariant: dry-run is exit 0");
    assert!(!report.changed);
    assert!(report.note.contains("would block"));

    let view = feature::show(&paths, "billing-csv").expect("invariant: show should succeed");
    assert_eq!(view.status, feature::FeatureStatus::Proposed);
}

#[test]
fn full_lifecycle_new_set_accept_start_ship() {
    let temp = TestTempDir::new("maestro-feature-lifecycle");
    let paths = MaestroPaths::new(temp.path());

    feature::create(&paths, "Billing CSV").expect("invariant: create should succeed");
    author_contract(&paths, "billing-csv");
    feature::accept(&paths, "billing-csv", false).expect("invariant: accept should succeed");
    let started = feature::start(&paths, "billing-csv").expect("invariant: start should succeed");
    assert_eq!(started.status, feature::FeatureStatus::InProgress);
    verify_contract(&paths, "billing-csv");
    let shipped =
        feature::ship(&paths, "billing-csv", None, false).expect("invariant: ship should succeed");
    assert_eq!(shipped.status, feature::FeatureStatus::Shipped);
}

#[test]
fn illegal_transitions_name_the_gap() {
    let temp = TestTempDir::new("maestro-feature-illegal");
    let paths = MaestroPaths::new(temp.path());

    feature::create(&paths, "Billing CSV").expect("invariant: create should succeed");
    // start before accept
    let error = feature::start(&paths, "billing-csv").expect_err("invariant: start must block");
    assert!(error.to_string().contains("not accepted"));

    author_contract(&paths, "billing-csv");
    feature::accept(&paths, "billing-csv", false).expect("invariant: accept should succeed");
    // ship before start
    let error =
        feature::ship(&paths, "billing-csv", None, false).expect_err("invariant: ship must block");
    assert!(error.to_string().contains("not started"));
}

#[test]
fn completed_transitions_are_idempotent_no_ops() {
    let temp = TestTempDir::new("maestro-feature-noop");
    let paths = MaestroPaths::new(temp.path());

    feature::create(&paths, "Billing CSV").expect("invariant: create should succeed");
    author_contract(&paths, "billing-csv");
    feature::accept(&paths, "billing-csv", false).expect("invariant: accept should succeed");

    // accept again: no-op at exit 0
    let report =
        feature::accept(&paths, "billing-csv", false).expect("invariant: re-accept is a no-op");
    assert!(!report.changed);
    assert!(report.note.contains("already ready"));
}

#[test]
fn ship_blocks_on_live_child_task() {
    let temp = TestTempDir::new("maestro-feature-ship-block");
    let paths = MaestroPaths::new(temp.path());

    feature::create(&paths, "Billing CSV").expect("invariant: create should succeed");
    author_contract(&paths, "billing-csv");
    feature::accept(&paths, "billing-csv", false).expect("invariant: accept should succeed");
    feature::start(&paths, "billing-csv").expect("invariant: start should succeed");
    verify_contract(&paths, "billing-csv");

    write_task(&paths.tasks_dir(), "task-001", "billing-csv", "in_progress");
    let error =
        feature::ship(&paths, "billing-csv", None, false).expect_err("invariant: ship must block");
    assert!(error.to_string().contains("task-001"));

    // A verified child does not block ship.
    write_task(&paths.tasks_dir(), "task-001", "billing-csv", "verified");
    let shipped =
        feature::ship(&paths, "billing-csv", None, false).expect("invariant: ship succeeds");
    assert_eq!(shipped.status, feature::FeatureStatus::Shipped);
}

#[test]
fn cancel_cascades_to_live_child_tasks() {
    let temp = TestTempDir::new("maestro-feature-cancel");
    let paths = MaestroPaths::new(temp.path());

    feature::create(&paths, "Billing CSV").expect("invariant: create should succeed");
    author_contract(&paths, "billing-csv");
    feature::accept(&paths, "billing-csv", false).expect("invariant: accept should succeed");
    feature::start(&paths, "billing-csv").expect("invariant: start should succeed");
    write_task(&paths.tasks_dir(), "task-001", "billing-csv", "in_progress");

    // Dry-run previews the cascade without mutating the feature or its children.
    let preview = feature::cancel(&paths, "billing-csv", "scope dropped", true)
        .expect("invariant: dry-run cancel succeeds");
    assert!(!preview.changed);
    assert_eq!(preview.abandoned, vec!["task-001".to_string()]);
    assert!(preview.note.contains("would cancel"));
    let still_live = feature::show(&paths, "billing-csv").expect("invariant: show should succeed");
    assert_eq!(still_live.status, feature::FeatureStatus::InProgress);

    let report = feature::cancel(&paths, "billing-csv", "scope dropped", false)
        .expect("invariant: cancel succeeds");
    assert!(report.changed);
    assert_eq!(report.abandoned, vec!["task-001".to_string()]);

    let view = feature::show(&paths, "billing-csv").expect("invariant: show should succeed");
    assert_eq!(view.status, feature::FeatureStatus::Cancelled);
    // The audited reason is persisted on the feature record.
    assert_eq!(view.cancel_reason.as_deref(), Some("scope dropped"));
    // The child task is now abandoned.
    let task_raw = fs::read_to_string(
        paths
            .features_dir()
            .join("billing-csv/tasks/task-001-task-001/task.yaml"),
    )
    .expect("invariant: child task should be readable");
    assert!(task_raw.contains("abandoned"));
}

#[test]
fn cannot_cancel_a_shipped_feature() {
    let temp = TestTempDir::new("maestro-feature-cancel-shipped");
    let paths = MaestroPaths::new(temp.path());

    feature::create(&paths, "Billing CSV").expect("invariant: create should succeed");
    author_contract(&paths, "billing-csv");
    feature::accept(&paths, "billing-csv", false).expect("invariant: accept should succeed");
    feature::start(&paths, "billing-csv").expect("invariant: start should succeed");
    verify_contract(&paths, "billing-csv");
    feature::ship(&paths, "billing-csv", None, false).expect("invariant: ship should succeed");

    let error = feature::cancel(&paths, "billing-csv", "too late", false)
        .expect_err("invariant: shipped features cannot be cancelled");
    assert!(error.to_string().contains("terminal"));
}

#[test]
fn amend_is_append_only_with_value_dedup() {
    let temp = TestTempDir::new("maestro-feature-amend");
    let paths = MaestroPaths::new(temp.path());

    feature::create(&paths, "Billing CSV").expect("invariant: create should succeed");
    author_contract(&paths, "billing-csv");
    feature::accept(&paths, "billing-csv", false).expect("invariant: accept should succeed");

    // amend grows the frozen contract
    let report = feature::amend(
        &paths,
        "billing-csv",
        ContractAdditions {
            acceptance: vec!["handles empty rows".to_string()],
            ..Default::default()
        },
        "widen scope",
    )
    .expect("invariant: amend should succeed");
    assert!(report.changed);
    assert_eq!(
        report.added.acceptance,
        vec!["handles empty rows".to_string()]
    );

    // re-adding a present value is a no-op (safe retries)
    let report = feature::amend(
        &paths,
        "billing-csv",
        ContractAdditions {
            acceptance: vec!["handles empty rows".to_string()],
            ..Default::default()
        },
        "retry",
    )
    .expect("invariant: amend retry should succeed");
    assert!(!report.changed);

    // the feature record embeds the one genuine amend
    let feature_raw = fs::read_to_string(
        paths
            .features_dir()
            .join("billing-csv")
            .join("feature.yaml"),
    )
    .expect("invariant: feature.yaml should be readable");
    assert!(feature_raw.contains("widen scope"));
    assert!(!feature_raw.contains("retry"));
}

#[test]
fn amend_rejects_a_proposed_feature() {
    let temp = TestTempDir::new("maestro-feature-amend-proposed");
    let paths = MaestroPaths::new(temp.path());

    feature::create(&paths, "Billing CSV").expect("invariant: create should succeed");
    let error = feature::amend(
        &paths,
        "billing-csv",
        ContractAdditions {
            acceptance: vec!["x".to_string()],
            ..Default::default()
        },
        "too early",
    )
    .expect_err("invariant: amend must be rejected before accept");
    assert!(error.to_string().contains("not accepted"));
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
fn strict_list_errors_on_incompatible_record() {
    let temp = TestTempDir::new("maestro-feature-strict");
    let paths = MaestroPaths::new(temp.path());
    write_feature(&paths, "billing-csv", BAD_RECORD);

    let error = feature::list(&paths).expect_err("invariant: strict read must error on bad schema");
    assert!(error.to_string().contains("schema"));
}

#[test]
fn strict_show_errors_on_incompatible_record() {
    let temp = TestTempDir::new("maestro-feature-strict-show");
    let paths = MaestroPaths::new(temp.path());
    write_feature(&paths, "billing-csv", BAD_RECORD);

    let error = feature::show(&paths, "billing-csv")
        .expect_err("invariant: strict read must error on bad schema");
    assert!(error.to_string().contains("schema"));
}

#[test]
fn tolerant_titles_degrade_to_empty_on_incompatible_record() {
    let temp = TestTempDir::new("maestro-feature-tolerant");
    let paths = MaestroPaths::new(temp.path());
    write_feature(&paths, "billing-csv", BAD_RECORD);

    let titles = feature::titles(&paths);
    assert!(titles.is_empty(), "tolerant titles must skip a bad record");
}

#[test]
fn tolerant_titles_degrade_to_empty_on_unparseable_record() {
    let temp = TestTempDir::new("maestro-feature-tolerant-parse");
    let paths = MaestroPaths::new(temp.path());
    write_feature(&paths, "billing-csv", "this: is: not: valid: yaml: [");

    let titles = feature::titles(&paths);
    assert!(titles.is_empty(), "tolerant titles must skip a parse error");
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
fn diagnose_reports_count_on_compatible_store() {
    let temp = TestTempDir::new("maestro-feature-diag-ok");
    let paths = MaestroPaths::new(temp.path());

    feature::create(&paths, "Billing CSV").expect("invariant: create should succeed");

    assert_eq!(feature::diagnose(&paths).found, Ok(1));
}

#[test]
fn diagnose_reports_absent_features_dir_as_error_data() {
    let temp = TestTempDir::new("maestro-feature-diag-absent");
    let paths = MaestroPaths::new(temp.path());

    assert!(
        feature::diagnose(&paths).found.is_err(),
        "an absent features dir must report as error data so doctor flags it"
    );
}

#[test]
fn diagnose_reports_incompatible_record_as_error_data() {
    let temp = TestTempDir::new("maestro-feature-diag-bad");
    let paths = MaestroPaths::new(temp.path());
    write_feature(&paths, "billing-csv", BAD_RECORD);

    assert!(
        feature::diagnose(&paths).found.is_err(),
        "an incompatible record must report as error data"
    );
}

#[test]
fn diagnose_reports_parse_error_as_error_data() {
    let temp = TestTempDir::new("maestro-feature-diag-parse");
    let paths = MaestroPaths::new(temp.path());
    write_feature(&paths, "billing-csv", "this: is: not: valid: yaml: [");

    assert!(
        feature::diagnose(&paths).found.is_err(),
        "a parse failure must report as error data"
    );
}

#[test]
fn status_label_renders_snake_case() {
    assert_eq!(
        feature::status_label(&feature::FeatureStatus::Proposed),
        "proposed"
    );
    assert_eq!(
        feature::status_label(&feature::FeatureStatus::Ready),
        "ready"
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
