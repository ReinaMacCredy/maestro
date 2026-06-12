//! P5D-S2 end-to-end: the DN9 flat verbs `maestro create`, `show`, `update`, and
//! `close` drive the card store through the real binary, alongside the beads-
//! structure `ready`/`list` output. Card-mode only; minted ids are recovered by
//! title (content-hash ids do not sort by creation order). The legacy guard for
//! the new verbs (exit 0 with a guiding line) is covered too.

pub mod card_support;
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
        id.starts_with("task-add-csv-export-"),
        "a task is minted a typed slug id: {id}"
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
fn create_uses_a_slug_for_features_and_a_typed_slug_for_other_types() {
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
        bug.contains("created bug-fix-ordering-race-"),
        "a non-feature card is minted a typed slug id:\n{bug}"
    );
}

/// SPEC-archive-memory A1: `list --grep` filters case-insensitively over
/// title, description, and sidecars, composing with the existing filters;
/// `--archived` extends the same query into `archive/cards/` with rows
/// marked `(archived: <parent>)`.
#[test]
fn list_grep_searches_live_cards_and_archived_extends_to_the_archive() {
    let temp = cards_repo("s2-grep-archived");
    let repo = temp.path();

    run(repo, &["feature", "new", "CSV export"]);
    run(
        repo,
        &[
            "create",
            "-t",
            "task",
            "Wire the exporter",
            "--parent",
            "csv-export",
            "--description",
            "emits a header row first",
        ],
    );
    run(repo, &["create", "-t", "task", "Unrelated work"]);
    let task_id = id_by_title(repo, "Wire the exporter");

    let by_title = run(repo, &["list", "--grep", "EXPORT"]);
    assert!(
        by_title.contains("csv-export") && by_title.contains(&task_id),
        "title grep is case-insensitive:\n{by_title}"
    );
    assert!(
        !by_title.contains("Unrelated work"),
        "non-matching cards stay out:\n{by_title}"
    );

    let by_body = run(repo, &["list", "--grep", "header row"]);
    assert!(
        by_body.contains(&task_id),
        "description grep matches:\n{by_body}"
    );

    let composed = run(repo, &["list", "--type", "task", "--grep", "export"]);
    assert!(
        composed.contains(&task_id) && !composed.contains("csv-export  feature"),
        "--grep composes with --type:\n{composed}"
    );

    let none = run(repo, &["list", "--grep", "no-such-term"]);
    assert!(none.contains("no cards match"), "{none}");

    // Archive the feature (terminal-gated: settle the child, cancel, archive).
    run(repo, &["close", &task_id]);
    run(
        repo,
        &["feature", "cancel", "csv-export", "--reason", "scope cut"],
    );
    run(repo, &["feature", "archive", "csv-export"]);

    let live_only = run(repo, &["list", "--grep", "export"]);
    assert!(
        !live_only.contains("csv-export"),
        "archived cards stay out without --archived:\n{live_only}"
    );

    let with_archive = run(repo, &["list", "--grep", "export", "--archived"]);
    assert!(
        with_archive.contains(&task_id) && with_archive.contains("(archived: csv-export)"),
        "an archived child row carries its parent marker:\n{with_archive}"
    );
    assert!(
        with_archive.contains("(archived)"),
        "the parentless feature row is marked archived:\n{with_archive}"
    );
}

/// SPEC-archive-memory-2 R2: `maestro archive --loose` sweeps terminal
/// parentless cards -- closed loose tasks/ideas and superseded decisions --
/// into the archive with one INDEX.md lid line per swept card. A locked loose
/// decision is standing law: reported as a kept rule, never moved. Open loose
/// cards are untouched and the sweep is idempotent.
#[test]
fn archive_loose_sweeps_terminal_parentless_cards_but_keeps_rules() {
    let temp = cards_repo("s2-archive-loose");
    let repo = temp.path();

    run(repo, &["create", "-t", "task", "Keep me open"]);
    run(repo, &["create", "-t", "task", "Sweep me"]);
    let swept_task = id_by_title(repo, "Sweep me");
    run(repo, &["update", &swept_task, "--status", "abandoned"]);

    run(repo, &["decision", "new", "Tabs or spaces"]);
    let old_rule = id_by_title(repo, "Tabs or spaces");
    run(
        repo,
        &[
            "decision",
            "lock",
            &old_rule,
            "--decision",
            "tabs",
            "--rejected",
            "spaces: drift",
        ],
    );
    run(repo, &["decision", "new", "Spaces after all"]);
    let new_rule = id_by_title(repo, "Spaces after all");
    run(
        repo,
        &[
            "decision",
            "lock",
            &new_rule,
            "--decision",
            "spaces",
            "--rejected",
            "tabs: rendering drift",
            "--supersedes",
            &old_rule,
        ],
    );

    run(
        repo,
        &[
            "harness",
            "propose",
            "--title",
            "Doctor empty dirs",
            "--evidence",
            "two empty card dirs",
        ],
    );
    let idea = id_by_title(repo, "Doctor empty dirs");
    run(repo, &["harness", "dismiss", &idea, "--reason", "noise"]);

    let receipt = run(repo, &["archive", "--loose"]);
    for boxed in [&swept_task, &old_rule, &idea] {
        assert!(
            receipt.contains(&format!("boxed: {boxed}")),
            "{boxed} should sweep:\n{receipt}"
        );
    }
    assert!(
        receipt.contains(&format!("kept:  {new_rule} (rule)")),
        "the locked rule stays live:\n{receipt}"
    );

    assert!(
        repo.join(".maestro/archive/cards/tasks")
            .join(&swept_task)
            .join("task.yaml")
            .is_file(),
        "a dir-backed loose task moves to the mirrored archive path"
    );
    let live_decisions = fs::read_to_string(repo.join(".maestro/cards/decisions.yaml"))
        .expect("invariant: live decisions.yaml should exist");
    // The kept rule's `supersedes:` field still references the old id, so
    // absence is asserted on the swept entry's title, not its id.
    assert!(
        live_decisions.contains(&new_rule) && !live_decisions.contains("Tabs or spaces"),
        "only the superseded entry leaves the live file:\n{live_decisions}"
    );
    let archived_decisions = fs::read_to_string(repo.join(".maestro/archive/cards/decisions.yaml"))
        .expect("invariant: archived decisions.yaml should exist");
    assert!(
        archived_decisions.contains(&old_rule),
        "the superseded entry lands in the archive container:\n{archived_decisions}"
    );

    let index = fs::read_to_string(repo.join(".maestro/archive/cards/INDEX.md"))
        .expect("invariant: INDEX.md should exist");
    assert!(
        index.contains(&format!("{swept_task}: abandoned -- Sweep me"))
            && index.contains(&format!("{old_rule}: superseded -- Tabs or spaces"))
            && index.contains(&format!("{idea}: dismissed -- Doctor empty dirs")),
        "each swept card gets a lid line:\n{index}"
    );

    let live = run(repo, &["list"]);
    assert!(
        live.contains("Keep me open") && live.contains(&new_rule) && !live.contains(&swept_task),
        "live list keeps current work + rules:\n{live}"
    );
    let recall = run(repo, &["list", "--grep", "Sweep me", "--archived"]);
    assert!(
        recall.contains(&swept_task),
        "swept cards stay recallable:\n{recall}"
    );
    let shown = run(repo, &["show", &old_rule]);
    assert!(
        shown.contains("Tabs or spaces"),
        "id-exact show falls through to the archive:\n{shown}"
    );

    let again = run(repo, &["archive", "--loose"]);
    assert!(
        !again.contains("boxed:") && again.contains(&format!("kept:  {new_rule} (rule)")),
        "a re-run sweeps nothing and still reports the rule:\n{again}"
    );
    let index_again = fs::read_to_string(repo.join(".maestro/archive/cards/INDEX.md"))
        .expect("invariant: INDEX.md should exist");
    assert_eq!(index, index_again, "a re-run appends no duplicate lid line");
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
fn update_composes_claim_with_field_edits_in_one_write() {
    let temp = cards_repo("s2-update-claim");
    let repo = temp.path();

    run(repo, &["create", "-t", "task", "Wire the verb"]);
    let id = id_by_title(repo, "Wire the verb");

    // A claim forces in_progress, so pairing it with --status is contradictory.
    let conflict = run_err(repo, &["update", &id, "--status", "ready", "--claim"]);
    assert!(
        conflict.contains("--status conflicts with --claim"),
        "the contradictory pair is refused up front:\n{conflict}"
    );

    // --title + --claim compose into one write; both effects land together.
    let combined = run(
        repo,
        &[
            "update",
            &id,
            "--title",
            "Wire the verb (claimed)",
            "--claim",
        ],
    );
    assert!(
        combined.contains(&format!("updated {id}")),
        "the field write is confirmed:\n{combined}"
    );
    assert!(
        combined.contains(&format!("claimed {id} as codex#s1")),
        "the claim is confirmed:\n{combined}"
    );
    let shown = run(repo, &["show", &id]);
    assert!(
        shown.contains("Wire the verb (claimed)"),
        "the title landed:\n{shown}"
    );
    assert!(shown.contains("@codex#s1"), "the claim landed:\n{shown}");
    assert!(
        shown.contains("in_progress"),
        "the claim moved the card in_progress:\n{shown}"
    );
}

#[test]
fn update_json_emits_compact_beads_style_array() {
    let temp = cards_repo("s2-update-json");
    let repo = temp.path();

    run(repo, &["create", "-t", "feature", "Auth"]);
    run(
        repo,
        &["create", "-t", "task", "Fix auth bug", "--parent", "auth"],
    );
    let id = id_by_title(repo, "Fix auth bug");

    let json = run(
        repo,
        &[
            "update",
            &id,
            "--title",
            "Fix auth bug now",
            "--claim",
            "--json",
        ],
    );
    let value: serde_json::Value =
        serde_json::from_str(&json).expect("update --json emits valid JSON");
    let cards = value
        .as_array()
        .expect("update --json emits a Beads-style array");
    assert_eq!(cards.len(), 1, "one updated card is returned");
    let card = &cards[0];
    assert_eq!(card["id"], serde_json::json!(id), "json carries the id");
    assert_eq!(
        card["title"],
        serde_json::json!("Fix auth bug now"),
        "json carries the updated title"
    );
    assert_eq!(
        card["status"],
        serde_json::json!("in_progress"),
        "claim moves the card in_progress"
    );
    assert_eq!(
        card["type"],
        serde_json::json!("task"),
        "json uses the public type field"
    );
    assert_eq!(
        card["parent"],
        serde_json::json!("auth"),
        "json carries the parent"
    );
    assert_eq!(
        card["claimed_by"],
        serde_json::json!("codex#s1"),
        "json uses Maestro's claimed_by field"
    );
    assert!(
        card["claimed_at"]
            .as_str()
            .is_some_and(|value| !value.is_empty()),
        "json carries the claim timestamp"
    );
    assert!(
        card.get("assignee").is_none(),
        "json does not introduce Beads-only assignee naming"
    );
    assert!(
        card.get("extra").is_none() && card.get("state_history").is_none(),
        "json stays compact and omits raw card carrier fields: {card:?}"
    );
}

#[test]
fn show_compact_json_keeps_raw_show_json_unchanged() {
    let temp = cards_repo("s2-show-compact-json");
    let repo = temp.path();

    run(repo, &["create", "-t", "feature", "Auth"]);
    run(
        repo,
        &["create", "-t", "task", "Fix auth bug", "--parent", "auth"],
    );
    let id = id_by_title(repo, "Fix auth bug");
    run(repo, &["update", &id, "--claim"]);

    let compact = run(repo, &["show", &id, "--compact-json"]);
    let compact: serde_json::Value =
        serde_json::from_str(&compact).expect("show --compact-json emits valid JSON");
    let card = compact
        .as_object()
        .expect("show --compact-json emits one compact card object");
    assert_eq!(card["id"], serde_json::json!(id), "compact json carries id");
    assert_eq!(
        card["title"],
        serde_json::json!("Fix auth bug"),
        "compact json carries title"
    );
    assert_eq!(
        card["status"],
        serde_json::json!("in_progress"),
        "compact json carries status after claim"
    );
    assert_eq!(
        card["type"],
        serde_json::json!("task"),
        "compact json carries public type"
    );
    assert_eq!(
        card["parent"],
        serde_json::json!("auth"),
        "compact json carries parent"
    );
    assert_eq!(
        card["claimed_by"],
        serde_json::json!("codex#s1"),
        "compact json carries claim owner"
    );
    assert!(
        card["claimed_at"]
            .as_str()
            .is_some_and(|value| !value.is_empty()),
        "compact json carries claim timestamp"
    );
    assert!(
        card.get("schema_version").is_none()
            && card.get("extra").is_none()
            && card.get("state_history").is_none(),
        "compact json omits raw carrier fields: {card:?}"
    );

    let raw = run(repo, &["show", &id, "--json"]);
    let raw: serde_json::Value =
        serde_json::from_str(&raw).expect("show --json keeps raw JSON valid");
    assert_eq!(
        raw["schema_version"],
        serde_json::json!("maestro.card.v1"),
        "raw show --json remains the raw card contract"
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

    let dangling = run_err(
        repo,
        &["create", "-t", "task", "Orphan", "--parent", "ghost"],
    );
    assert!(
        dangling.contains("ghost not found") && dangling.contains("create the feature first"),
        "a dangling parent names the fix:\n{dangling}"
    );

    run(repo, &["create", "-t", "task", "Plain task"]);
    let task_id = id_by_title(repo, "Plain task");
    let non_feature = run_err(
        repo,
        &["create", "-t", "task", "Child", "--parent", &task_id],
    );
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

    run(
        repo,
        &["create", "-t", "task", "Docked", "--parent", "csv-export"],
    );
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

/// SPEC-archive-memory-2 R6: the text index transparently accelerates
/// `list --grep [--archived]`. The first indexed read creates it, a write
/// staleness self-heals on the next read, a corrupt file falls back silently
/// (and heals), a sub-trigram term skips the index entirely, and
/// `index rebuild` is the explicit recovery verb -- results are identical on
/// every path.
#[test]
fn text_index_accelerates_grep_transparently_with_silent_fallback() {
    let temp = cards_repo("s2-text-index");
    let repo = temp.path();
    let index_file = repo.join(".maestro/index/text.json");

    run(repo, &["create", "-t", "task", "Streaming exporter"]);
    run(repo, &["create", "-t", "task", "Importer cleanup"]);
    let importer = id_by_title(repo, "Importer cleanup");
    run(repo, &["update", &importer, "--status", "abandoned"]);
    run(repo, &["archive", "--loose"]);

    // The first indexed read answers from a scan and writes the index.
    assert!(!index_file.exists(), "no index before the first grep");
    let live_hits = run(repo, &["list", "--grep", "exporter"]);
    assert!(live_hits.contains("Streaming exporter"), "{live_hits}");
    assert!(
        index_file.exists(),
        "list --grep creates the index transparently"
    );

    // The index covers the archive; the live-only view still scopes it out.
    let live_only = run(repo, &["list", "--grep", "importer"]);
    assert!(
        !live_only.contains(&importer),
        "an archived candidate stays out without --archived:\n{live_only}"
    );
    let with_archive = run(repo, &["list", "--grep", "importer", "--archived"]);
    assert!(
        with_archive.contains(&importer),
        "the archived card is recalled through the index:\n{with_archive}"
    );

    // A write between reads goes stale; the next read self-heals in-verb.
    run(repo, &["create", "-t", "task", "Quarterly exporter report"]);
    let after_write = run(repo, &["list", "--grep", "exporter"]);
    assert!(
        after_write.contains("Streaming exporter")
            && after_write.contains("Quarterly exporter report"),
        "a card written after indexing is found without a manual rebuild:\n{after_write}"
    );

    // A corrupt index is never an error: the read falls back, then heals it.
    fs::write(&index_file, "not json{{{").expect("invariant: index file is writable");
    let corrupt_read = run(repo, &["list", "--grep", "exporter"]);
    assert!(
        corrupt_read.contains("Streaming exporter")
            && corrupt_read.contains("Quarterly exporter report"),
        "a corrupt index must not change results:\n{corrupt_read}"
    );
    let healed = fs::read_to_string(&index_file).expect("invariant: index file readable");
    assert!(
        healed.starts_with('{'),
        "the read heals the corrupt index:\n{healed}"
    );

    // A term shorter than one trigram skips the index (plain scan, same answer).
    let short = run(repo, &["list", "--grep", "ex"]);
    assert!(
        short.contains("Streaming exporter") && short.contains("Quarterly exporter report"),
        "{short}"
    );

    // Explicit recovery verb with its receipt.
    let receipt = run(repo, &["index", "rebuild"]);
    assert!(receipt.contains("text index rebuilt"), "{receipt}");
    assert!(
        receipt.contains("(2 live, 1 archived)"),
        "the receipt counts both trees:\n{receipt}"
    );
    assert!(receipt.contains("next: maestro list --grep"), "{receipt}");
}
