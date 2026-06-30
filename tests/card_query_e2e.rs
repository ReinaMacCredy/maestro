//! P4b/P4c/P4d/P4e/P4f end-to-end: the `maestro ready`, `maestro list`, `maestro dep`,
//! `maestro archive`, `maestro claim`, and `maestro note` verbs drive the card
//! query/edit/archive layers through the real binary. Coverage: a genuinely migrated repo (coarse
//! mapping DN3 over the real migrated status words); a hand-built blocker chain
//! (E8 gating clears); `--assignee` matching the agent portion of a claim;
//! `dep add` authoring a blocking edge that holds the dependent back (and
//! rejecting a self-block); `archive <feature>` moving the feature card and its
//! `parent=<feature>` children to the archive sibling (E4 query-driven cascade)
//! and refusing when a member is still open; `claim <id>` taking a free card under
//! `<agent>#<session>` (DN8), staying idempotent for the same session, refusing a
//! fresh foreign claim and reclaiming a stale one (E6/O2); `note <id>` appending
//! dated lines to a card's notes.md sidecar (D5); and the legacy store exiting 0
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
use serde_json::Value as JsonValue;
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

/// Run `maestro claim <id>` with a fixed `MAESTRO_SESSION` so the stamped
/// `<agent>#<session>` identity is deterministic (agent is `codex` per `maestro`).
fn maestro_claim(cwd: &Path, id: &str, session: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(["claim", id])
        .current_dir(cwd)
        .env("MAESTRO_AGENT", "codex")
        .env("MAESTRO_SESSION", session)
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

    // Author a feature-owned task through the real verbs. The task lands as a
    // workable card in its creation status ("draft" -> coarse OPEN), so it is
    // ready; the feature card is not workable and must never appear in `ready`.
    let feature_id =
        maestro::domain::feature::create(&paths, "Csv export", None).expect("create feature");
    assert_eq!(feature_id, "csv-export");
    task::create_task(
        &paths.tasks_dir(),
        "Add CSV export",
        CreateTaskOptions {
            feature: Some(feature_id.clone()),
            covers: Vec::new(),
            lane: None,
            risk: None,
            checks: vec!["exports a header row".to_string()],
            project: None,
            created_at: NOW.to_string(),
        },
    )
    .expect("create task");
    card_migrate::run(&paths, NOW).expect("migration succeeds");

    // The task is minted with a stable typed slug id (SPEC E2/O3); capture it
    // from the store and assert the CLI verbs surface it.
    let task1_id = task::load_task_records(&paths.tasks_dir())
        .expect("scan tasks")
        .into_iter()
        .next()
        .expect("one migrated task")
        .id;
    assert!(
        task1_id.starts_with("task-add-csv-export-"),
        "task keeps its typed slug id through migration: {task1_id}"
    );

    let repo = temp.path();

    // G3 + D3 + DN3 + D4: the workable, coarse-OPEN task surfaces; the feature
    // card does not.
    let ready = run(repo, &["ready"]);
    assert!(
        ready.contains(&task1_id),
        "ready should list the task:\n{ready}"
    );
    // The task's slug id contains "csv-export", so match the feature row
    // (id + type column) rather than the bare substring.
    assert!(
        !ready.contains("csv-export  feature"),
        "feature card is not workable and must stay out of ready:\n{ready}"
    );

    // `ready <feature>` scopes to a feature's children (handler-side retain,
    // a different path from `list --parent`'s ListFilter).
    let scoped = run(repo, &["ready", "csv-export"]);
    assert!(
        scoped.contains(&task1_id),
        "feature-scoped ready keeps its children:\n{scoped}"
    );
    let other = run(repo, &["ready", "no-such-feature"]);
    assert!(
        !other.contains(&task1_id),
        "scoping to another feature drops it:\n{other}"
    );

    // list --type splits the entity kinds.
    let features = run(repo, &["list", "--type", "feature"]);
    assert!(
        features.contains("csv-export"),
        "feature should list:\n{features}"
    );
    let tasks = run(repo, &["list", "--type", "task"]);
    assert!(tasks.contains(&task1_id), "task should list:\n{tasks}");

    let ready_json = run(repo, &["ready", "--json"]);
    let ready_value: serde_json::Value =
        serde_json::from_str(&ready_json).expect("ready --json emits valid JSON");
    assert_eq!(ready_value["version"], serde_json::json!(1));
    assert_eq!(ready_value["schema"], serde_json::json!("maestro.ready.v1"));
    let ready_cards = ready_value["cards"]
        .as_array()
        .expect("ready cards should be an array");
    let ready_task = ready_cards
        .iter()
        .find(|card| card["id"] == serde_json::json!(task1_id))
        .expect("ready JSON should include the migrated task");
    assert_eq!(ready_task["rank"], serde_json::json!(1));
    assert_eq!(ready_task["type"], serde_json::json!("task"));
    assert_eq!(ready_task["title"], serde_json::json!("Add CSV export"));
    assert_eq!(ready_task["status"], serde_json::json!("draft"));
    assert_eq!(ready_task["parent"], serde_json::json!("csv-export"));
    assert_eq!(ready_task["claimed_by"], serde_json::Value::Null);

    let list_json = run(repo, &["list", "--type", "task", "--json"]);
    let list_value: serde_json::Value =
        serde_json::from_str(&list_json).expect("list --json emits valid JSON");
    assert_eq!(list_value["version"], serde_json::json!(1));
    assert_eq!(list_value["schema"], serde_json::json!("maestro.list.v1"));
    let list_cards = list_value["cards"]
        .as_array()
        .expect("list cards should be an array");
    let listed_task = list_cards
        .iter()
        .find(|card| card["id"] == serde_json::json!(task1_id))
        .expect("list JSON should include the migrated task");
    assert_eq!(listed_task["type"], serde_json::json!("task"));
    assert_eq!(listed_task["title"], serde_json::json!("Add CSV export"));
    assert_eq!(listed_task["status"], serde_json::json!("draft"));
    assert_eq!(listed_task["parent"], serde_json::json!("csv-export"));
    assert_eq!(listed_task["claimed_by"], serde_json::Value::Null);
    assert_eq!(listed_task["claimed_at"], serde_json::Value::Null);
    assert_eq!(listed_task["archived"], serde_json::json!(false));

    // --parent is the children-of-a-feature query.
    let children = run(repo, &["list", "--parent", "csv-export"]);
    assert!(
        children.contains(&task1_id),
        "task is a child of the feature:\n{children}"
    );

    // --status reads the COARSE word: the draft task is open, not closed.
    let open = run(repo, &["list", "--status", "open"]);
    assert!(
        open.contains(&task1_id),
        "draft maps to coarse open:\n{open}"
    );
    let closed = run(repo, &["list", "--status", "closed"]);
    assert!(
        !closed.contains(&task1_id),
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
fn archive_moves_a_shipped_feature_and_its_children_through_the_binary() {
    let temp = TestTempDir::new("p4d-archive");
    let paths = MaestroPaths::new(temp.path());
    let repo = temp.path();

    // A shipped feature with two settled children, plus an unrelated feature that
    // must stay put. "shipped" is the feature terminal word -> coarse CLOSED.
    write_card(
        &paths,
        &Card::new(
            "csv-export",
            CardType::Feature,
            "CSV export",
            "shipped",
            NOW,
        ),
    );
    let mut child_a = Card::new("task-001", CardType::Task, "One", "verified", NOW);
    child_a.parent = Some("csv-export".to_string());
    let mut child_b = Card::new("task-002", CardType::Task, "Two", "closed", NOW);
    child_b.parent = Some("csv-export".to_string());
    write_card(&paths, &child_a);
    write_card(&paths, &child_b);
    write_card(
        &paths,
        &Card::new("other", CardType::Feature, "Other", "shipped", NOW),
    );

    let out = run(repo, &["archive", "csv-export"]);
    assert!(
        out.contains("archived feature csv-export")
            && out.contains("task-001")
            && out.contains("task-002"),
        "the verb reports the feature and its archived children:\n{out}"
    );

    // E4: the whole set moved to cards.sqlite and left the live store.
    assert!(repo.join(".maestro/archive/cards.sqlite").is_file());
    for id in ["csv-export", "task-001", "task-002"] {
        assert!(
            !repo.join(".maestro/archive/cards").join(id).exists(),
            "{id} should not leave a visible archive folder"
        );
        assert!(
            !repo.join(".maestro/cards").join(id).exists(),
            "{id} left the live store"
        );
    }
    // the unrelated feature is untouched
    assert!(repo.join(".maestro/cards/other").is_dir());

    // archived work no longer surfaces in the live board
    let ready = run(repo, &["ready"]);
    assert!(
        !ready.contains("task-001"),
        "an archived card is out of ready:\n{ready}"
    );
}

#[test]
fn legacy_shipped_feature_renders_as_closed_in_generic_card_views() {
    let temp = TestTempDir::new("p4d-legacy-display");
    let paths = MaestroPaths::new(temp.path());
    let repo = temp.path();

    // A pre-rename feature keeps `shipped` on disk (no migration, by design). The
    // type-agnostic card views must show the current spelling, never the legacy word.
    write_card(
        &paths,
        &Card::new(
            "csv-export",
            CardType::Feature,
            "CSV export",
            "shipped",
            NOW,
        ),
    );

    for args in [
        &["card", "show", "csv-export"][..],
        &["card", "list", "--all"][..],
    ] {
        let out = run(repo, args);
        assert!(
            out.contains("closed") && !out.contains("shipped"),
            "{args:?} renders the legacy terminal word as closed, not shipped:\n{out}"
        );
    }
}

#[test]
fn archived_task_still_renders_in_show_and_list_all() {
    let temp = TestTempDir::new("p4d-archive-read");
    let paths = MaestroPaths::new(temp.path());
    let repo = temp.path();

    write_card(
        &paths,
        &Card::new(
            "csv-export",
            CardType::Feature,
            "CSV export",
            "shipped",
            NOW,
        ),
    );
    let mut child = Card::new("task-001", CardType::Task, "One", "verified", NOW);
    child.parent = Some("csv-export".to_string());
    write_card(&paths, &child);
    run(repo, &["archive", "csv-export"]);

    // L6b: a historical reference to an archived task still renders through
    // `task show`, disclosed as archived so it cannot pass for live work.
    let show = run(repo, &["task", "show", "task-001"]);
    assert!(
        show.contains("One") && show.contains("archived: true"),
        "an archived task renders with the archive marker:\n{show}"
    );

    // The bare list stays live-tree only; `--all` reads the card archive.
    let live = run(repo, &["task", "list"]);
    assert!(
        !live.contains("task-001"),
        "an archived task stays out of the bare list:\n{live}"
    );
    let all = run(repo, &["task", "list", "--all"]);
    assert!(
        all.contains("One") && all.contains("REF") && !all.contains("task-001"),
        "`--all` includes the archived task:\n{all}"
    );
    let all_json: JsonValue =
        serde_json::from_str(&run(repo, &["task", "list", "--all", "--json"]))
            .expect("task list JSON parses");
    assert_eq!(
        all_json["tasks"][0]["id"],
        JsonValue::String("task-001".to_string())
    );
}

#[test]
fn archive_refuses_a_feature_with_open_work() {
    let temp = TestTempDir::new("p4d-archive-open");
    let paths = MaestroPaths::new(temp.path());
    let repo = temp.path();

    write_card(
        &paths,
        &Card::new(
            "csv-export",
            CardType::Feature,
            "CSV export",
            "shipped",
            NOW,
        ),
    );
    let mut open_child = Card::new("task-001", CardType::Task, "One", "in_progress", NOW);
    open_child.parent = Some("csv-export".to_string());
    write_card(&paths, &open_child);

    let out = maestro(repo, &["archive", "csv-export"]);
    assert!(
        !out.status.success(),
        "an open child blocks archive:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("task-001"),
        "the refusal names the open id"
    );
    assert!(
        repo.join(".maestro/cards/csv-export").is_dir(),
        "nothing moved when the gate fails"
    );
}

/// `archive` takes a feature id; a workable card is closed history, not an
/// archive root (the cascade is feature-scoped, SPEC E4).
#[test]
fn archive_refuses_a_non_feature_id() {
    let temp = TestTempDir::new("p4d-archive-nonfeature");
    let paths = MaestroPaths::new(temp.path());
    let repo = temp.path();

    write_card(
        &paths,
        &Card::new("task-001", CardType::Task, "One", "closed", NOW),
    );

    let out = maestro(repo, &["archive", "task-001"]);
    assert!(
        !out.status.success(),
        "a task id is refused:\nstdout:\n{}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("not a feature"),
        "the refusal names the type mismatch:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn claim_takes_a_free_card_is_idempotent_and_refuses_a_fresh_foreign_claim() {
    let temp = TestTempDir::new("p4e-claim");
    let paths = MaestroPaths::new(temp.path());
    let repo = temp.path();

    write_card(
        &paths,
        &Card::new("task-001", CardType::Task, "Work", "ready", NOW),
    );

    // a free card is taken under <agent>#<session> and moves to in_progress
    let out = maestro_claim(repo, "task-001", "s1");
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    assert!(
        out.status.success() && stdout.contains("claimed task-001 as codex#s1"),
        "fresh claim stamps the identity:\n{stdout}"
    );
    let listed = run(repo, &["list", "--assignee", "codex"]);
    assert!(
        listed.contains("task-001") && listed.contains("in_progress"),
        "the claimed card is in_progress and owned:\n{listed}"
    );

    // the same session re-claiming is a no-op
    let again = maestro_claim(repo, "task-001", "s1");
    assert!(
        String::from_utf8_lossy(&again.stdout).contains("already yours"),
        "an idempotent re-claim by the holder reports already-yours"
    );

    // a different live session is refused -- the claim is fresh, not stale
    let contend = maestro_claim(repo, "task-001", "s2");
    assert!(
        !contend.status.success() && String::from_utf8_lossy(&contend.stderr).contains("codex#s1"),
        "a fresh foreign claim is refused and names the holder:\nstderr:\n{}",
        String::from_utf8_lossy(&contend.stderr)
    );
}

#[test]
fn claim_reclaims_a_stale_foreign_claim_through_the_binary() {
    let temp = TestTempDir::new("p4e-claim-stale");
    let paths = MaestroPaths::new(temp.path());
    let repo = temp.path();

    // a card held by a long-dead session (claimed_at far in the past, well beyond
    // the 15-minute TTL) must be reclaimable so it never pins forever (E6/O2).
    let mut held = Card::new("task-001", CardType::Task, "Work", "in_progress", NOW);
    held.claimed_by = Some("claude#old".to_string());
    held.claimed_at = Some("2020-01-01T00:00:00.000Z".to_string());
    write_card(&paths, &held);

    let out = maestro_claim(repo, "task-001", "s9");
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    assert!(
        out.status.success()
            && stdout.contains("reclaimed task-001 from claude#old (stale) as codex#s9"),
        "a stale claim is taken over with a warning:\n{stdout}"
    );
}

#[test]
fn note_appends_dated_lines_to_a_card_sidecar_through_the_binary() {
    let temp = TestTempDir::new("p4f-note");
    let paths = MaestroPaths::new(temp.path());
    let repo = temp.path();

    write_card(
        &paths,
        &Card::new("task-001", CardType::Task, "Work", "ready", NOW),
    );

    let first = run(repo, &["note", "task-001", "chose option B"]);
    assert!(
        first.contains("noted task-001 (notes.md created)"),
        "the first note creates the sidecar:\n{first}"
    );
    let second = run(repo, &["note", "task-001", "B breaks on reparent"]);
    assert!(
        second.contains("noted task-001") && !second.contains("created"),
        "a later note appends without re-creating:\n{second}"
    );

    let notes = fs::read_to_string(
        card_path(&paths, "task-001")
            .parent()
            .unwrap()
            .join("notes.md"),
    )
    .expect("notes.md exists after the appends");
    assert!(
        notes.starts_with("# Work\n\n"),
        "the title header is seeded once:\n{notes}"
    );
    assert!(
        notes.contains("chose option B"),
        "first note line present:\n{notes}"
    );
    assert!(
        notes.contains("B breaks on reparent"),
        "second note line present:\n{notes}"
    );

    // noting a card that does not exist fails loud rather than creating an orphan.
    let missing = maestro(repo, &["note", "ghost", "anything"]);
    assert!(
        !missing.status.success(),
        "noting a missing card is an error:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&missing.stdout),
        String::from_utf8_lossy(&missing.stderr)
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
    let archive = run(repo, &["archive", "csv-export"]);
    assert!(
        archive.contains("no card store"),
        "archive on a legacy repo also guides rather than erroring:\n{archive}"
    );
    let claim = run(repo, &["claim", "task-001"]);
    assert!(
        claim.contains("no card store"),
        "claim on a legacy repo also guides rather than erroring:\n{claim}"
    );
    let note = run(repo, &["note", "task-001", "anything"]);
    assert!(
        note.contains("no card store"),
        "note on a legacy repo also guides rather than erroring:\n{note}"
    );
}

/// The agent contract holds even without a card store: `--json` stdout is a
/// valid zero-card envelope and the guiding notice moves to stderr (AGENTS.md
/// "Agent-facing read contracts are JSON, not human text").
#[test]
fn ready_and_list_json_stay_parseable_without_a_card_store() {
    let temp = TestTempDir::new("p4b-legacy-json");
    let repo = temp.path();
    fs::create_dir(repo.join(".git")).expect("invariant: .git marker should be creatable");

    for (verb, schema) in [("ready", "maestro.ready.v1"), ("list", "maestro.list.v1")] {
        let output = maestro(repo, &[verb, "--json"]);
        assert!(output.status.success(), "{verb} --json exits 0");
        let stdout = String::from_utf8(output.stdout).expect("stdout is UTF-8");
        let value: serde_json::Value = serde_json::from_str(&stdout)
            .unwrap_or_else(|e| panic!("{verb} --json must stay parseable JSON ({e}):\n{stdout}"));
        assert_eq!(value["version"], serde_json::json!(1));
        assert_eq!(value["schema"], serde_json::json!(schema));
        assert_eq!(
            value["cards"],
            serde_json::json!([]),
            "no card store means zero cards, not human text"
        );
        let stderr = String::from_utf8(output.stderr).expect("stderr is UTF-8");
        assert!(
            stderr.contains("no card store"),
            "the guiding notice moves to stderr in --json mode:\n{stderr}"
        );
    }
}

#[test]
fn watch_board_marks_a_card_model_blocked_task_blocked() {
    // Regression: the watch board built its blocked set from the legacy
    // `.maestro/tasks` scan plus the `blockers` field, so a card-model `blocks`
    // dep never lit the blocked glyph -- a held-back task read "ready" on the
    // board even though `maestro ready` correctly excluded it. A feature-parented
    // task blocked by an open sibling must read "blocked" on `watch snapshot`.
    let temp = TestTempDir::new("watch-blocked");
    let paths = MaestroPaths::new(temp.path());
    let repo = temp.path();

    let feature = Card::new("feat-x", CardType::Feature, "Feature X", "proposed", NOW);
    let mut blocker = Card::new("task-100", CardType::Task, "Blocker", "ready", NOW);
    blocker.parent = Some("feat-x".to_string());
    let mut blocked = Card::new("task-101", CardType::Task, "Dependent", "ready", NOW);
    blocked.parent = Some("feat-x".to_string());
    blocked.deps = vec![Dep {
        kind: DepKind::Blocks,
        target: "task-100".to_string(),
    }];
    write_card(&paths, &feature);
    write_card(&paths, &blocker);
    write_card(&paths, &blocked);

    let board = run(repo, &["watch", "snapshot"]);

    assert!(
        board.contains("blocked 1"),
        "the feature header counts the blocked task (was `blocked 0` before the fix):\n{board}"
    );
    let blocked_line = board
        .lines()
        .find(|line| line.contains("task-101"))
        .unwrap_or_else(|| panic!("the dependent task must render:\n{board}"));
    assert!(
        blocked_line.contains("blocked"),
        "the card-model blocked task reads blocked, not ready:\n{blocked_line}"
    );
    let blocker_line = board
        .lines()
        .find(|line| line.contains("task-100"))
        .unwrap_or_else(|| panic!("the blocker task must render:\n{board}"));
    assert!(
        blocker_line.contains("ready"),
        "the open blocker stays ready:\n{blocker_line}"
    );
}

/// The four open-bucket counts (active/ready/needs_verification/blocked) the
/// number after `key` on `line`, tolerating both the `active=1` (status) and
/// `active 1` (board) separators.
fn count_after(line: &str, key: &str) -> usize {
    let idx = line
        .find(key)
        .unwrap_or_else(|| panic!("missing `{key}` in: {line}"));
    line[idx + key.len()..]
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .unwrap_or_else(|_| panic!("no number after `{key}` in: {line}"))
}

#[test]
fn status_open_bucket_counts_agree_with_the_watch_board() {
    // Regression: `maestro status` counted from the legacy `TaskRecord`
    // projection (`active` = every live task, `ready` = `TaskState::Ready`,
    // `blocked` = the empty `blockers` field), so on a card-model repo it read
    // `active=4 ready=0 blocked=0` while the board read the correct partition.
    // Both surfaces now classify the card graph through `query::classify`, so
    // their four open buckets must agree. One card per board state.
    let temp = TestTempDir::new("status-board-agree");
    let paths = MaestroPaths::new(temp.path());
    let repo = temp.path();

    let feature = Card::new("feat-x", CardType::Feature, "Feature X", "proposed", NOW);
    let mut done = Card::new("task-done", CardType::Task, "Done", "verified", NOW);
    done.parent = Some("feat-x".to_string());
    let mut active = Card::new("task-active", CardType::Task, "Active", "in_progress", NOW);
    active.parent = Some("feat-x".to_string());
    active.claimed_by = Some("codex#s1".to_string());
    let mut ready = Card::new("task-ready", CardType::Task, "Ready", "ready", NOW);
    ready.parent = Some("feat-x".to_string());
    let mut nv = Card::new("task-nv", CardType::Task, "NV", "needs_verification", NOW);
    nv.parent = Some("feat-x".to_string());
    // blocked by the open `task-active`, so it stays out of `ready` and reads
    // blocked on both surfaces.
    let mut blocked = Card::new("task-blocked", CardType::Task, "Blocked", "ready", NOW);
    blocked.parent = Some("feat-x".to_string());
    blocked.deps = vec![Dep {
        kind: DepKind::Blocks,
        target: "task-active".to_string(),
    }];
    for card in [&feature, &done, &active, &ready, &nv, &blocked] {
        write_card(&paths, card);
    }

    let board = run(repo, &["watch", "snapshot"]);
    let board_header = board
        .lines()
        .find(|line| line.contains("| ready"))
        .unwrap_or_else(|| panic!("the board must render a feature header:\n{board}"));
    let status = run(repo, &["status"]);
    let status_line = status
        .lines()
        .find(|line| line.starts_with("tasks: active="))
        .unwrap_or_else(|| panic!("status must print the task summary line:\n{status}"));

    for key in ["active", "ready", "needs_verification", "blocked"] {
        assert_eq!(
            count_after(status_line, key),
            count_after(board_header, key),
            "`{key}` must agree between status and the board\nstatus: {status_line}\nboard:  {board_header}"
        );
    }
    // The partition is exercised, not just equal-to-each-other: one card per
    // open bucket plus the done card excluded from all four.
    assert_eq!(
        count_after(status_line, "active"),
        1,
        "status: {status_line}"
    );
    assert_eq!(
        count_after(status_line, "ready"),
        1,
        "status: {status_line}"
    );
    assert_eq!(
        count_after(status_line, "needs_verification"),
        1,
        "status: {status_line}"
    );
    assert_eq!(
        count_after(status_line, "blocked"),
        1,
        "status: {status_line}"
    );
}
