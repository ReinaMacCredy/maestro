pub mod card_support;
mod support;

use std::fs;

use card_support::{card_doc, cards_repo};
use maestro::domain::decisions;
use maestro::domain::decisions::schema::DecisionStatus;
use maestro::foundation::core::fs::ensure_dir;
use maestro::foundation::core::paths::MaestroPaths;

#[test]
fn create_open_persists_first_global_decision() {
    let temp = cards_repo("maestro-decision-create");
    let paths = MaestroPaths::new(temp.path());

    let report = decisions::create_open(
        &paths,
        "Use single HARNESS.md",
        Some("too many files"),
        None,
        None,
    )
    .expect("invariant: create should succeed");

    assert!(
        report.record.id.starts_with("dec-"),
        "card-mode decision id: {}",
        report.record.id
    );
    assert_eq!(report.record.status, DecisionStatus::Open);
    assert_eq!(report.record.context.as_deref(), Some("too many files"));
    let card = card_doc(temp.path(), &report.record.id);
    assert_eq!(card["status"], "open");
    assert_eq!(card["description"], "too many files");
    assert!(
        card.get("extra").is_none(),
        "the global decision has no decision-specific payload yet"
    );
    assert!(
        !paths.decisions_file().is_file(),
        "card-mode creation must not write the legacy decisions.yaml store"
    );
}

#[test]
fn create_open_rejects_empty_slug_title() {
    let temp = cards_repo("maestro-decision-empty");
    let paths = MaestroPaths::new(temp.path());

    let err = decisions::create_open(&paths, "   ", None, None, None)
        .expect_err("empty-slug title must be rejected");
    assert!(
        err.to_string()
            .contains("at least one ASCII letter or digit"),
        "{err}"
    );
    assert!(
        fs::read_dir(paths.cards_dir())
            .expect("invariant: cards dir should be readable")
            .next()
            .is_none(),
        "a rejected title must not mint a card"
    );
}

#[test]
fn decision_exists_propagates_structured_store_errors() {
    let temp = cards_repo("maestro-decision-exists-error");
    let paths = MaestroPaths::new(temp.path());
    // A decision id normalizes straight to its card path, so a corrupt card.yaml
    // there is reached by the single-card load before the type check. The card
    // store rejects a wrong schema_version, so the lookup that gates supersede
    // validation and the frozen-legacy guard surfaces the error instead of
    // silently collapsing to false.
    ensure_dir(paths.cards_dir().join("decision-001"))
        .expect("invariant: card dir should be creatable");
    fs::write(
        paths.cards_dir().join("decision-001").join("card.yaml"),
        "schema_version: wrong.version\nid: decision-001\ntype: decision\ntitle: x\nstatus: open\ncreated_at: 1970-01-01T00:00:00Z\nupdated_at: 1970-01-01T00:00:00Z\n",
    )
    .expect("invariant: invalid decision card should be writable");

    let error = decisions::decision_exists(&paths, "decision-001")
        .expect_err("schema mismatch must not collapse to false");
    assert!(
        format!("{error:#}").contains("schema mismatch"),
        "{error:#}"
    );
}

#[test]
fn decision_set_plan_normalizes_children_and_hashes_input() {
    let raw = r#"
title: Lock all DecisionSet forks
feature: decisionset-anti-compression-workflow
project: maestro
source_approval:
  summary: user chose all rec
advisor_review:
  summary: advisor approved separate child decisions
children:
  - title: Storage shape
    order: 1
    decision: Use one DecisionSet plus child decisions.
    rejected:
      - Store one compressed summary decision.
    preview: decset -> dec child
  - key: cli-output
    title: CLI output
    order: 2
    context: show should be readable
    decision: Show compact receipt by default.
    rejected:
      - Always print full nested output.
"#;

    let plan = decisions::decision_set::plan_from_yaml(raw)
        .expect("valid DecisionSet YAML should produce a plan");

    assert!(
        plan.set_id
            .starts_with("decset-lock-all-decisionset-forks-")
    );
    assert_eq!(
        plan.feature.as_deref(),
        Some("decisionset-anti-compression-workflow")
    );
    assert_eq!(plan.project.as_deref(), Some("maestro"));
    assert_eq!(plan.schema_version, 1);
    assert!(plan.input_hash.starts_with("sha256:"));
    assert_eq!(plan.children.len(), 2);
    assert_eq!(plan.children[0].key, "storage-shape");
    assert_eq!(plan.children[0].order, 1);
    assert_eq!(plan.children[0].summary.child_decision_id, None);
    assert_eq!(plan.children[0].summary.title, "Storage shape");
    assert_eq!(plan.children[1].key, "cli-output");
    assert_eq!(plan.children[1].order, 2);
    assert_eq!(
        plan.warnings,
        Vec::<maestro::domain::decisions::decision_set::DecisionSetWarning>::new()
    );
}

#[test]
fn decision_set_plan_rejects_duplicate_titles_without_keys() {
    let raw = r#"
title: Duplicate titles
children:
  - title: Same
    decision: First
  - title: Same
    decision: Second
"#;

    let error = decisions::decision_set::plan_from_yaml(raw)
        .expect_err("duplicate child titles without explicit keys are ambiguous");
    let message = format!("{error:#}");
    assert!(message.contains("duplicate child title"), "{message}");
    assert!(message.contains("explicit unique key"), "{message}");
}

#[test]
fn decision_set_record_fields_round_trip_through_decision_card() {
    let temp = cards_repo("maestro-decision-set-card");
    let paths = MaestroPaths::new(temp.path());
    let raw = r#"
title: Lock all DecisionSet forks
feature: decisionset-anti-compression-workflow
children:
  - key: storage-shape
    title: Storage shape
    decision: Use one DecisionSet plus child decisions.
"#;
    let plan = decisions::decision_set::plan_from_yaml(raw).expect("plan");
    decisions::decision_set::write_plan_records(&paths, &plan, "2026-07-02T00:00:00Z")
        .expect("records should persist");

    let set = decisions::show(&paths, &plan.set_id).expect("set should load");
    let rendered = set.render();
    assert!(rendered.contains("kind: decision_set"), "{rendered}");
    assert!(rendered.contains("input_hash: sha256:"), "{rendered}");
    assert!(
        rendered.contains("child_decision_id: dec-storage-shape-"),
        "{rendered}"
    );

    let child_id = format!(
        "dec-storage-shape-{}",
        &plan.input_hash["sha256:".len()..][..4]
    );
    let child = decisions::show(&paths, &child_id).expect("child should load");
    let rendered_child = child.render();
    assert!(
        rendered_child.contains(&format!("decision_set_id: {}", plan.set_id)),
        "{rendered_child}"
    );
}

#[test]
fn decision_set_write_rolls_back_created_records_when_child_create_fails() {
    let temp = cards_repo("maestro-decision-set-rollback");
    let paths = MaestroPaths::new(temp.path());
    let raw = r#"
title: Lock all DecisionSet forks
children:
  - key: storage-shape
    title: Storage shape
    decision: Use one DecisionSet plus child decisions.
"#;
    let plan = decisions::decision_set::plan_from_yaml(raw).expect("plan");
    let child_id = format!(
        "dec-storage-shape-{}",
        &plan.input_hash["sha256:".len()..][..4]
    );
    let now = "2026-07-02T00:00:00Z";
    fs::write(
        paths.cards_dir().join("decisions.yaml"),
        format!(
            "- schema_version: maestro.card.v1\n  id: {child_id}\n  type: decision\n  title: Existing child\n  status: locked\n  created_at: {now}\n  updated_at: {now}\n"
        ),
    )
    .expect("invariant: colliding decision entry should be writable");

    let error = decisions::decision_set::write_plan_records(&paths, &plan, now)
        .expect_err("child collision should fail the batch");
    assert!(format!("{error:#}").contains("already exists"), "{error:#}");
    assert!(
        !decisions::decision_exists(&paths, &plan.set_id).expect("lookup should succeed"),
        "set record should be rolled back after child create failure"
    );
    assert!(
        decisions::decision_exists(&paths, &child_id).expect("lookup should succeed"),
        "pre-existing child collision must remain"
    );
}

#[test]
fn compressed_summary_detection_requires_multiple_signals() {
    let summary = r#"
Locked all 10 remaining recommendations as design decisions.
- dec-task-setup-uses-one-flag-per-task-first-5e9b
- dec-setup-blockers-use-local-aliases-b4f0
- dec-task-rows-store-blocker-lane-gate-fields-78f7
Verified each is locked with the intended preview.
"#;

    let detection = decisions::decision_set::detect_compressed_summary(summary)
        .expect("multi-decision summary should be detected");
    assert!(detection.blocking, "{detection:?}");
    assert!(
        detection
            .signals
            .iter()
            .any(|signal| signal == "lock_all_wording")
    );
    assert!(
        detection
            .signals
            .iter()
            .any(|signal| signal == "multiple_decision_ids")
    );
    assert!(
        decisions::decision_set::detect_compressed_summary("Choose option C: use YAML input.")
            .is_none(),
        "a normal single decision should not be flagged"
    );
}
