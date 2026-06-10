//! P5D-S2 end-to-end: the DN9 flat verbs `maestro create`, `show`, `update`, and
//! `close` drive the card store through the real binary, alongside the beads-
//! structure `ready`/`list` output. Card-mode only; minted ids are recovered by
//! title (content-hash ids do not sort by creation order). The legacy guard for
//! the new verbs (exit 0 with a guiding line) is covered too.

mod card_support;
mod support;

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use card_support::{cards_repo, id_by_title};
use support::TestTempDir;

fn maestro(cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .current_dir(cwd)
        .env("MAESTRO_AGENT", "codex")
        .env("MAESTRO_SESSION", "s1")
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

fn run_err(cwd: &Path, args: &[&str]) -> String {
    let output = maestro(cwd, args);
    assert!(
        !output.status.success(),
        "maestro {args:?} unexpectedly succeeded\nstdout:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn create_mints_a_card_and_show_round_trips_it() {
    let temp = cards_repo("s2-create-show");
    let repo = temp.path();

    run(repo, &["create", "-t", "feature", "CSV export"]);
    let created = run(
        repo,
        &[
            "create",
            "-t",
            "task",
            "Add CSV export",
            "--parent",
            "csv-export",
            "--description",
            "exports a header row",
        ],
    );
    assert!(
        created.contains("created") && created.contains("(task)"),
        "create confirms the type:\n{created}"
    );

    let id = id_by_title(repo, "Add CSV export");
    assert!(
        id.starts_with("card-"),
        "a task is minted a content-hash id: {id}"
    );

    let shown = run(repo, &["show", &id]);
    assert!(shown.contains(&id), "show names the card:\n{shown}");
    assert!(
        shown.contains("task") && shown.contains("Add CSV export"),
        "show carries type and title:\n{shown}"
    );
    assert!(shown.contains("open"), "a new card is open:\n{shown}");
    assert!(
        shown.contains("parent: csv-export"),
        "show lists the parent:\n{shown}"
    );
    assert!(
        shown.contains("exports a header row"),
        "show prints the description body:\n{shown}"
    );
    assert!(
        shown.contains("(unclaimed)"),
        "a new card is unclaimed:\n{shown}"
    );
}

#[test]
fn show_renders_the_display_alias_for_parented_cards() {
    let temp = cards_repo("s2-display-alias");
    let repo = temp.path();

    run(repo, &["create", "-t", "feature", "CSV Export"]);
    let parent_args = ["--parent", "csv-export"];
    run(
        repo,
        &[&["create", "-t", "task", "First task"], &parent_args[..]].concat(),
    );
    run(
        repo,
        &[&["create", "-t", "task", "Second task"], &parent_args[..]].concat(),
    );

    // Hash ids do not sort by creation order, so derive the expected ordinals
    // from the id sort the alias is specified over (SPEC E2).
    let mut ordered = [
        id_by_title(repo, "First task"),
        id_by_title(repo, "Second task"),
    ];
    ordered.sort_unstable();
    for (index, id) in ordered.iter().enumerate() {
        let shown = run(repo, &["show", id]);
        assert!(
            shown.contains(&format!("alias: csv-export.{} (display only)", index + 1)),
            "show renders the dotted alias marked display-only:\n{shown}"
        );
    }

    let feature_shown = run(repo, &["show", "csv-export"]);
    assert!(
        !feature_shown.contains("alias:"),
        "a parentless card carries no alias line:\n{feature_shown}"
    );
    let ready = run(repo, &["ready"]);
    assert!(
        !ready.contains("csv-export."),
        "ready carries no alias column:\n{ready}"
    );
    let listed = run(repo, &["list"]);
    assert!(
        !listed.contains("csv-export."),
        "list carries no alias column:\n{listed}"
    );
}

#[test]
fn show_json_mirrors_the_card() {
    let temp = cards_repo("s2-show-json");
    let repo = temp.path();

    run(
        repo,
        &[
            "create",
            "-t",
            "task",
            "Json me",
            "--description",
            "body text",
        ],
    );
    let id = id_by_title(repo, "Json me");

    let json = run(repo, &["show", &id, "--json"]);
    let value: serde_json::Value =
        serde_json::from_str(&json).expect("show --json emits valid JSON");
    assert_eq!(value["id"], serde_json::json!(id), "json carries the id");
    assert_eq!(
        value["type"],
        serde_json::json!("task"),
        "json carries the renamed type field"
    );
    assert_eq!(
        value["title"],
        serde_json::json!("Json me"),
        "json carries the title"
    );
    assert_eq!(
        value["status"],
        serde_json::json!("open"),
        "json carries the status"
    );
    assert_eq!(
        value["description"],
        serde_json::json!("body text"),
        "json carries the description"
    );
}

#[test]
fn create_uses_a_slug_for_features_and_a_hash_for_other_types() {
    let temp = cards_repo("s2-create-ids");
    let repo = temp.path();

    let feature = run(repo, &["create", "-t", "feature", "CSV export"]);
    assert!(
        feature.contains("created csv-export (feature)"),
        "a feature keeps its creation slug as the id:\n{feature}"
    );
    assert!(
        repo.join(".maestro/cards/csv-export/card.yaml").is_file(),
        "the feature card lands at its slug directory"
    );

    let bug = run(repo, &["create", "-t", "bug", "Fix ordering race"]);
    assert!(
        bug.contains("created card-"),
        "a non-feature card is minted a content-hash id:\n{bug}"
    );
}

#[test]
fn update_writes_fields_claims_and_close_closes() {
    let temp = cards_repo("s2-update-close");
    let repo = temp.path();

    run(repo, &["create", "-t", "task", "Wire the verb"]);
    let id = id_by_title(repo, "Wire the verb");

    // Field mutation through the D1 CAS seam.
    let updated = run(
        repo,
        &[
            "update",
            &id,
            "--status",
            "needs_verification",
            "--description",
            "blocked on review",
        ],
    );
    assert!(
        updated.contains(&format!("updated {id}")),
        "update confirms the write:\n{updated}"
    );
    let shown = run(repo, &["show", &id]);
    assert!(
        shown.contains("needs_verification"),
        "the new status is persisted:\n{shown}"
    );
    assert!(
        shown.contains("blocked on review"),
        "the description is persisted:\n{shown}"
    );

    // `update --claim` delegates to the same seam as the standalone `claim` verb.
    let claimed = run(repo, &["update", &id, "--claim"]);
    assert!(
        claimed.contains(&format!("claimed {id} as codex#s1")),
        "update --claim stamps the identity:\n{claimed}"
    );
    let after_claim = run(repo, &["show", &id]);
    assert!(
        after_claim.contains("@codex#s1"),
        "the claim shows on the card:\n{after_claim}"
    );

    // `close` moves to the uniform terminal word; a second close guides.
    let closed = run(repo, &["close", &id]);
    assert!(
        closed.contains(&format!("closed {id}")),
        "close confirms:\n{closed}"
    );
    let after_close = run(repo, &["show", &id]);
    assert!(
        after_close.contains("closed"),
        "the card is closed:\n{after_close}"
    );
    let again = run(repo, &["close", &id]);
    assert!(
        again.contains("already closed"),
        "a second close guides rather than erroring:\n{again}"
    );
}

#[test]
fn ready_and_list_render_the_beads_structure() {
    let temp = cards_repo("s2-beads-output");
    let repo = temp.path();

    run(repo, &["create", "-t", "task", "First task"]);
    run(repo, &["create", "-t", "bug", "Second bug"]);

    let ready = run(repo, &["ready"]);
    assert!(
        ready.contains("Ready work (2 cards, no blockers):"),
        "beads ready header with a count:\n{ready}"
    );
    assert!(
        ready.contains("1. [P1]") && ready.contains("2. [P2]"),
        "numbered [P#] rank rows:\n{ready}"
    );
    assert!(
        ready.contains("(unclaimed)"),
        "unclaimed cards show the claim column:\n{ready}"
    );

    let list = run(repo, &["list"]);
    assert!(
        list.contains("2 cards:"),
        "beads list count header:\n{list}"
    );
    assert!(
        list.contains("1.") && list.contains("open"),
        "numbered rows carry the real status:\n{list}"
    );
}

/// SPEC E3: `close` and `update --status` are workable-card verbs. A decision
/// or feature keeps its per-type lifecycle (the generic write would bypass its
/// gates), and a typo'd status word is rejected loudly instead of silently
/// poisoning the coarse derivation.
#[test]
fn close_and_status_writes_are_guarded_per_type() {
    let temp = cards_repo("s2-status-guards");
    let repo = temp.path();

    run(repo, &["create", "-t", "decision", "Pick a queue"]);
    let decision = id_by_title(repo, "Pick a queue");
    let refused = run_err(repo, &["close", &decision]);
    assert!(
        refused.contains("maestro decision lock"),
        "close on a decision points at the per-type verb:\n{refused}"
    );

    run(repo, &["create", "-t", "feature", "CSV export"]);
    let refused = run_err(repo, &["update", "csv-export", "--status", "shipped"]);
    assert!(
        refused.contains("maestro feature ship"),
        "a generic status write on a feature points at the gated verb:\n{refused}"
    );

    run(repo, &["create", "-t", "task", "Wire the verb"]);
    let task = id_by_title(repo, "Wire the verb");
    let refused = run_err(repo, &["update", &task, "--status", "in-progress"]);
    assert!(
        refused.contains("unknown --status") && refused.contains("in_progress"),
        "a typo'd word is rejected naming the legal vocabulary:\n{refused}"
    );
}

/// SPEC G1/E1: `--parent` docks a card under a feature. A dangling or
/// non-feature parent is refused at create time with the fix named, and a
/// feature itself cannot take a parent.
#[test]
fn create_validates_the_parent_dock() {
    let temp = cards_repo("s2-parent-guards");
    let repo = temp.path();

    let dangling = run_err(repo, &["create", "-t", "task", "Orphan", "--parent", "ghost"]);
    assert!(
        dangling.contains("ghost not found") && dangling.contains("create the feature first"),
        "a dangling parent names the fix:\n{dangling}"
    );

    run(repo, &["create", "-t", "task", "Plain task"]);
    let task_id = id_by_title(repo, "Plain task");
    let non_feature = run_err(repo, &["create", "-t", "task", "Child", "--parent", &task_id]);
    assert!(
        non_feature.contains("not a feature"),
        "a non-feature parent is refused:\n{non_feature}"
    );

    run(repo, &["create", "-t", "feature", "CSV export"]);
    let nested = run_err(
        repo,
        &["create", "-t", "feature", "Sub", "--parent", "csv-export"],
    );
    assert!(
        nested.contains("cannot take --parent"),
        "a feature refuses a parent:\n{nested}"
    );

    run(repo, &["create", "-t", "task", "Docked", "--parent", "csv-export"]);
    let docked = run(repo, &["show", &id_by_title(repo, "Docked")]);
    assert!(
        docked.contains("parent: csv-export"),
        "a valid feature parent docks:\n{docked}"
    );
}

/// A card id is joined into `.maestro/cards/<id>/`, so a traversal-shaped id
/// must be refused at the verb boundary instead of addressing a path outside
/// the store.
#[test]
fn traversal_shaped_ids_are_refused() {
    let temp = cards_repo("s2-traversal");
    let repo = temp.path();

    for args in [
        vec!["show", "../../outside"],
        vec!["update", "../../outside", "--status", "ready"],
        vec!["close", "../../outside"],
        vec!["note", "../../outside", "text"],
        vec!["claim", "../../outside"],
    ] {
        let refused = run_err(repo, &args);
        assert!(
            refused.contains("invalid card id"),
            "{args:?} refuses the traversal id:\n{refused}"
        );
    }
}

#[test]
fn update_without_id_or_flags_guides_rather_than_erroring() {
    let temp = cards_repo("s2-update-guides");
    let repo = temp.path();
    run(repo, &["create", "-t", "task", "Lonely"]);
    let id = id_by_title(repo, "Lonely");

    // A bare `update` (no id) prints usage and exits 0 (no-dead-end-errors).
    let bare = run(repo, &["update"]);
    assert!(
        bare.contains("usage: maestro update"),
        "bare update prints usage:\n{bare}"
    );

    // `update <id>` with no mutation flag is guided, not an error.
    let no_flags = run(repo, &["update", &id]);
    assert!(
        no_flags.contains("nothing to update"),
        "a flagless update guides:\n{no_flags}"
    );
}

#[test]
fn the_new_verbs_on_a_legacy_repo_print_the_guiding_notice() {
    let temp = TestTempDir::new("s2-legacy");
    let repo = temp.path();
    fs::create_dir(repo.join(".git")).expect("invariant: .git marker should be creatable");

    let cases: [Vec<&str>; 4] = [
        vec!["create", "-t", "task", "X"],
        vec!["show", "anything"],
        vec!["update", "anything", "--status", "closed"],
        vec!["close", "anything"],
    ];
    for args in cases {
        let out = run(repo, &args);
        assert!(
            out.contains("no card store"),
            "legacy repo guides rather than erroring for {args:?}:\n{out}"
        );
    }
}
