//! Slice 3c end-to-end: after migration the task verbs read and write the card
//! store, the legacy `task.yaml` is frozen, the task->feature link survives as
//! `card.parent` (so the on-demand counts group by it), and a card-mode
//! `set_feature` retargets that field without moving a directory
//! (SPEC-beads-model P1 dual-read cutover).

mod support;

use std::fs;
use std::path::PathBuf;

use maestro::domain::card::schema::{Card, CardType};
use maestro::domain::card::store::card_path;
use maestro::domain::card::{StoreMode, store_mode};
use maestro::domain::task::{self, CreateTaskOptions, TaskState};
use maestro::feature::query::count_tasks_by_feature;
use maestro::foundation::core::paths::MaestroPaths;
use maestro::operations::card_migrate;
use support::TestTempDir;

const NOW: &str = "2026-06-08T12:00:00Z";

fn card(paths: &MaestroPaths, id: &str) -> Card {
    let contents = std::fs::read_to_string(card_path(paths, id)).expect("card.yaml readable");
    serde_yaml::from_str(&contents).expect("card.yaml parses")
}

/// The single legacy `task.yaml` under the feature's task root, read straight off
/// disk to prove migration left it frozen.
fn legacy_task_yaml(paths: &MaestroPaths, feature: &str) -> PathBuf {
    let tasks = paths.features_dir().join(feature).join("tasks");
    let dir = fs::read_dir(&tasks)
        .expect("feature tasks dir readable")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.is_dir())
        .expect("one legacy task dir");
    dir.join("task.yaml")
}

fn legacy_state(path: &PathBuf) -> String {
    let contents = fs::read_to_string(path).expect("legacy task.yaml readable");
    let value: serde_yaml::Value = serde_yaml::from_str(&contents).expect("task.yaml parses");
    value["state"]
        .as_str()
        .expect("state is a string")
        .to_string()
}

#[test]
fn task_verbs_read_and_write_the_card_after_migration() {
    let temp = TestTempDir::new("task-card-cutover");
    let paths = MaestroPaths::new(temp.path());
    let tasks_dir = paths.tasks_dir();

    // Author a feature-owned task through the real verbs while the repo is still
    // legacy, so the migrated card carries a faithful record. The feature exists
    // first so the task lands under `features/<id>/tasks/` -- the only carrier of
    // the feature link, which the migration must recover into `card.parent`.
    let feature_id =
        maestro::domain::feature::create(&paths, "Csv export").expect("create feature");
    assert_eq!(feature_id, "csv-export");
    let task = task::create_task(
        &tasks_dir,
        "Add CSV export",
        CreateTaskOptions {
            feature: Some(feature_id.clone()),
            covers: Vec::new(),
            lane: None,
            risk: None,
            checks: vec!["exports a header row".to_string()],
            created_at: NOW.to_string(),
        },
    )
    .expect("create task");
    assert_eq!(task.id, "task-001");
    assert_eq!(
        store_mode(&paths),
        StoreMode::Legacy,
        "no cards/ yet -> legacy"
    );

    let legacy_yaml = legacy_task_yaml(&paths, &feature_id);

    // Fold the legacy trees into cards/. Non-destructive: the legacy task.yaml is
    // left in place, frozen at `draft`.
    card_migrate::run(&paths, NOW).expect("migration succeeds");
    assert_eq!(
        store_mode(&paths),
        StoreMode::Cards,
        "cards/ present -> card store"
    );

    let migrated = card(&paths, "task-001");
    assert_eq!(migrated.card_type, CardType::Task);
    assert_eq!(
        migrated.parent.as_deref(),
        Some("csv-export"),
        "the dir-derived feature link folds into card.parent"
    );
    assert_eq!(migrated.status, "draft");
    assert_eq!(legacy_state(&legacy_yaml), "draft");

    // Scan + count read the card: feature_id is recovered from card.parent, so
    // the on-demand counts group by it without a directory to read.
    let records = task::load_task_records(&tasks_dir).expect("scan tasks");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].id, "task-001");
    assert_eq!(records[0].feature_id.as_deref(), Some("csv-export"));
    assert_eq!(records[0].state, TaskState::Draft);
    let counts = count_tasks_by_feature(&tasks_dir).expect("count by feature");
    assert_eq!(counts.get("csv-export").map(|c| c.total), Some(1));

    // create_task in card mode mints the next id off the card store (no legacy
    // archive here -> next = max(cards=1, archive=0)+1) and writes a card, not a
    // legacy tree. This is the one CRUD verb with its own allocation path.
    let second = task::create_task(
        &tasks_dir,
        "Second task",
        CreateTaskOptions {
            feature: None,
            covers: Vec::new(),
            lane: None,
            risk: None,
            checks: Vec::new(),
            created_at: NOW.to_string(),
        },
    )
    .expect("create task in card mode");
    assert_eq!(second.id, "task-002");
    assert_eq!(card(&paths, "task-002").card_type, CardType::Task);
    assert_eq!(card(&paths, "task-002").parent, None);

    // Drive the lifecycle through the card. Each verb's load must read the card
    // the previous verb wrote (not a stale snapshot), proving the CAS round-trip.
    task::transition_task(
        &tasks_dir,
        "task-001",
        TaskState::Exploring,
        "maestro",
        NOW,
        task::TransitionDetails::default(),
    )
    .expect("explore");
    assert_eq!(card(&paths, "task-001").status, "exploring");

    let accepted = task::accept_task(&tasks_dir, "task-001", "maestro", NOW).expect("accept");
    assert_eq!(accepted.state, TaskState::Ready);
    assert_eq!(card(&paths, "task-001").status, "ready");

    let claimed = task::claim_task(&tasks_dir, "task-001", "claude#s1", NOW).expect("claim");
    assert_eq!(claimed.state, TaskState::InProgress);
    assert_eq!(card(&paths, "task-001").status, "in_progress");

    // Three card writes later, the legacy task.yaml is still frozen -> the card,
    // not the legacy file, is authoritative.
    assert_eq!(
        legacy_state(&legacy_yaml),
        "draft",
        "legacy task.yaml stays frozen across every card write"
    );

    // set_feature in card mode retargets card.parent with no directory move: the
    // count drops the feature, and no feature task tree is created or removed.
    task::set_feature(&tasks_dir, "task-001", None, "maestro", NOW).expect("detach feature");
    assert_eq!(
        card(&paths, "task-001").parent,
        None,
        "the parent field is cleared in place"
    );
    let counts = count_tasks_by_feature(&tasks_dir).expect("recount");
    assert_eq!(
        counts.get("csv-export"),
        None,
        "the detached task no longer counts toward the feature"
    );
    let records = task::load_task_records(&tasks_dir).expect("rescan");
    assert_eq!(records[0].feature_id, None);
}
