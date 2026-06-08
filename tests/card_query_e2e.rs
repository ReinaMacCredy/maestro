//! P4b end-to-end: the `maestro ready` and `maestro list` verbs drive the
//! card query layer (scan -> workable/coarse/ready -> render) through the real
//! binary. The first test runs against a genuinely migrated repo so the coarse
//! mapping (DN3) is exercised on the actual status words migration emits, not
//! synthetic ones; the second hand-builds a blocker chain to prove the `blocks`
//! gating (E8) clears end-to-end; the third confirms the legacy store exits 0
//! with a guiding notice rather than a dead-end error.

mod support;

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use maestro::domain::card::schema::{Card, CardType, Dep, DepKind};
use maestro::domain::card::store::{card_path, load_with_snapshot, save_with_snapshot};
use maestro::domain::task::{self, CreateTaskOptions};
use maestro::foundation::core::paths::MaestroPaths;
use maestro::operations::card_migrate;
use support::TestTempDir;

const NOW: &str = "2026-06-08T12:00:00Z";

fn maestro(cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .current_dir(cwd)
        .env("MAESTRO_AGENT", "codex")
        .output()
        .expect("invariant: compiled maestro binary should run in integration tests")
}

fn run(cwd: &Path, args: &[&str]) -> String {
    let output = maestro(cwd, args);
    assert!(
        output.status.success(),
        "maestro {args:?} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("invariant: stdout should be UTF-8")
}

fn write_card(paths: &MaestroPaths, card: &Card) {
    let path = card_path(paths, &card.id);
    let snapshot = load_with_snapshot(&path).expect("invariant: snapshot read should succeed");
    save_with_snapshot(&path, card, &snapshot).expect("invariant: card write should succeed");
}

#[test]
fn ready_and_list_reflect_a_migrated_card_store() {
    let temp = TestTempDir::new("p4b-ready-migrated");
    let paths = MaestroPaths::new(temp.path());

    // Author a feature-owned task through the real verbs while the repo is still
    // legacy, then migrate. The migrated task lands as a workable card in its
    // creation status ("draft" -> coarse OPEN), so it is ready; the feature card
    // is not workable and must never appear in `ready`.
    let feature_id =
        maestro::domain::feature::create(&paths, "Csv export").expect("create feature");
    assert_eq!(feature_id, "csv-export");
    let task = task::create_task(
        &paths.tasks_dir(),
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
    card_migrate::run(&paths, NOW).expect("migration succeeds");

    let repo = temp.path();

    // G3 + D3 + DN3 + D4: the workable, coarse-OPEN task surfaces; the feature
    // card does not.
    let ready = run(repo, &["ready"]);
    assert!(
        ready.contains("task-001"),
        "ready should list the task:\n{ready}"
    );
    assert!(
        !ready.contains("csv-export"),
        "feature card is not workable and must stay out of ready:\n{ready}"
    );

    // `ready <feature>` scopes to a feature's children (handler-side retain,
    // a different path from `list --parent`'s ListFilter).
    let scoped = run(repo, &["ready", "csv-export"]);
    assert!(
        scoped.contains("task-001"),
        "feature-scoped ready keeps its children:\n{scoped}"
    );
    let other = run(repo, &["ready", "no-such-feature"]);
    assert!(
        !other.contains("task-001"),
        "scoping to another feature drops it:\n{other}"
    );

    // list --type splits the entity kinds.
    let features = run(repo, &["list", "--type", "feature"]);
    assert!(
        features.contains("csv-export"),
        "feature should list:\n{features}"
    );
    let tasks = run(repo, &["list", "--type", "task"]);
    assert!(tasks.contains("task-001"), "task should list:\n{tasks}");

    // --parent is the children-of-a-feature query.
    let children = run(repo, &["list", "--parent", "csv-export"]);
    assert!(
        children.contains("task-001"),
        "task is a child of the feature:\n{children}"
    );

    // --status reads the COARSE word: the draft task is open, not closed.
    let open = run(repo, &["list", "--status", "open"]);
    assert!(
        open.contains("task-001"),
        "draft maps to coarse open:\n{open}"
    );
    let closed = run(repo, &["list", "--status", "closed"]);
    assert!(
        !closed.contains("task-001"),
        "an open task must not match --status closed:\n{closed}"
    );
}

#[test]
fn ready_hides_a_blocked_card_until_its_blocker_closes() {
    let temp = TestTempDir::new("p4b-blocker");
    let paths = MaestroPaths::new(temp.path());
    let repo = temp.path();

    // task-100 is free; task-101 blocks on it. Writing into cards/ makes the
    // repo card-mode (the .maestro/cards/ dir is both the store and the marker).
    let blocker = Card::new("task-100", CardType::Task, "Blocker", "ready", NOW);
    let mut blocked = Card::new("task-101", CardType::Task, "Blocked", "ready", NOW);
    blocked.deps = vec![Dep {
        kind: DepKind::Blocks,
        target: "task-100".to_string(),
    }];
    write_card(&paths, &blocker);
    write_card(&paths, &blocked);

    // E8: the blocker is coarse-OPEN, so the blocked card is held back.
    let before = run(repo, &["ready"]);
    assert!(
        before.contains("task-100"),
        "the unblocked card is ready:\n{before}"
    );
    assert!(
        !before.contains("task-101"),
        "the card blocked by an open card is held back:\n{before}"
    );

    // Close the blocker; now the dependent clears, and the closed blocker itself
    // leaves ready (coarse CLOSED is not OPEN).
    let path = card_path(&paths, "task-100");
    let snapshot = load_with_snapshot(&path).expect("snapshot");
    let mut closed = snapshot.card.clone().expect("blocker exists");
    closed.status = "closed".to_string();
    save_with_snapshot(&path, &closed, &snapshot).expect("rewrite blocker");

    let after = run(repo, &["ready"]);
    assert!(
        after.contains("task-101"),
        "the blocker is closed, so the dependent is ready:\n{after}"
    );
    assert!(
        !after.contains("task-100"),
        "a closed card is not open and must leave ready:\n{after}"
    );
}

#[test]
fn ready_and_list_on_a_legacy_repo_print_the_guiding_notice() {
    let temp = TestTempDir::new("p4b-legacy");
    let repo = temp.path();
    fs::create_dir(repo.join(".git")).expect("invariant: .git marker should be creatable");

    let ready = run(repo, &["ready"]);
    assert!(
        ready.contains("no card store"),
        "legacy repo gets a guiding notice, not an error:\n{ready}"
    );
    let list = run(repo, &["list"]);
    assert!(
        list.contains("no card store"),
        "legacy repo gets a guiding notice, not an error:\n{list}"
    );
}
