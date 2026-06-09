//! P4b/P4c end-to-end: the `maestro ready`, `maestro list`, and `maestro dep`
//! verbs drive the card query and edit layers through the real binary. Coverage:
//! a genuinely migrated repo (coarse mapping DN3 over the real migrated status
//! words); a hand-built blocker chain (E8 gating clears); `--assignee` matching
//! the agent portion of a claim; `dep add` authoring a blocking edge that holds
//! the dependent back (and rejecting a self-block); and the legacy store exiting
//! 0 with a guiding notice rather than a dead-end error.

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
fn list_assignee_matches_the_agent_portion_of_a_claim() {
    let temp = TestTempDir::new("p4b-assignee");
    let paths = MaestroPaths::new(temp.path());
    let repo = temp.path();

    // Claims are `<agent>#<session>` (DN8); `--assignee claude` must find the
    // session-suffixed claim, not require the full token.
    let mut claimed = Card::new("task-001", CardType::Task, "Claimed", "in_progress", NOW);
    claimed.claimed_by = Some("claude#s1".to_string());
    let free = Card::new("task-002", CardType::Task, "Free", "ready", NOW);
    write_card(&paths, &claimed);
    write_card(&paths, &free);

    let by_agent = run(repo, &["list", "--assignee", "claude"]);
    assert!(
        by_agent.contains("task-001"),
        "--assignee claude finds the claude#s1 claim:\n{by_agent}"
    );
    assert!(
        !by_agent.contains("task-002"),
        "the unclaimed card must not answer an assignee filter:\n{by_agent}"
    );
    let other_agent = run(repo, &["list", "--assignee", "codex"]);
    assert!(
        !other_agent.contains("task-001"),
        "a different agent does not match the claude claim:\n{other_agent}"
    );
}

#[test]
fn dep_add_blocks_the_dependent_through_the_binary() {
    let temp = TestTempDir::new("p4c-dep-add");
    let paths = MaestroPaths::new(temp.path());
    let repo = temp.path();

    // two free, ready tasks
    write_card(
        &paths,
        &Card::new("task-001", CardType::Task, "Blocker", "ready", NOW),
    );
    write_card(
        &paths,
        &Card::new("task-002", CardType::Task, "Dependent", "ready", NOW),
    );

    let before = run(repo, &["ready"]);
    assert!(
        before.contains("task-001") && before.contains("task-002"),
        "both free cards are ready before any edge:\n{before}"
    );

    // `dep add <child> <parent>`: the edge is stored on the dependent (child).
    let added = run(repo, &["dep", "add", "task-002", "task-001"]);
    assert!(
        added.contains("task-002 is now blocked by task-001"),
        "the verb confirms the new edge:\n{added}"
    );

    let after = run(repo, &["ready"]);
    assert!(
        after.contains("task-001"),
        "the open blocker stays ready:\n{after}"
    );
    assert!(
        !after.contains("task-002"),
        "the dependent is held back while its blocker is open (E8):\n{after}"
    );

    // idempotent: a second identical add is a no-op.
    let again = run(repo, &["dep", "add", "task-002", "task-001"]);
    assert!(
        again.contains("already blocked by"),
        "a duplicate edge reports already-present:\n{again}"
    );

    // a card cannot block itself -- the verb fails rather than writing a
    // permanently-unready self-edge.
    let self_block = maestro(repo, &["dep", "add", "task-001", "task-001"]);
    assert!(
        !self_block.status.success(),
        "a self-block is rejected:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&self_block.stdout),
        String::from_utf8_lossy(&self_block.stderr)
    );
}

#[test]
fn ready_list_and_dep_on_a_legacy_repo_print_the_guiding_notice() {
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
    let dep = run(repo, &["dep", "add", "task-002", "task-001"]);
    assert!(
        dep.contains("no card store"),
        "dep add on a legacy repo also guides rather than erroring:\n{dep}"
    );
}
