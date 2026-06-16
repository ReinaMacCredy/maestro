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

use card_support::{card_doc, cards_repo, id_by_title};
use serde_json::Value;
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

#[test]
fn generic_create_persists_project_for_bug_and_chore() {
    let temp = cards_repo("s2-create-project-generic");
    let repo = temp.path();

    run(
        repo,
        &[
            "create",
            "-t",
            "bug",
            "Fix ordering race",
            "--project",
            "svc-pay",
        ],
    );
    let bug = id_by_title(repo, "Fix ordering race");
    assert_eq!(
        card_doc(repo, &bug)["project"],
        "svc-pay",
        "a bug minted with --project persists project on the card"
    );

    run(
        repo,
        &["create", "-t", "chore", "Tidy logs", "--project", "svc-pay"],
    );
    let chore = id_by_title(repo, "Tidy logs");
    assert_eq!(
        card_doc(repo, &chore)["project"],
        "svc-pay",
        "a chore minted with --project persists project on the card"
    );

    run(
        repo,
        &["create", "-t", "idea", "Spark", "--project", "svc-pay"],
    );
    let idea = id_by_title(repo, "Spark");
    assert_eq!(
        card_doc(repo, &idea)["project"],
        "svc-pay",
        "an idea minted with --project persists project on the card"
    );

    run(repo, &["create", "-t", "bug", "No project here"]);
    let bare = id_by_title(repo, "No project here");
    assert!(
        card_doc(repo, &bare).get("project").is_none(),
        "a card minted without --project has no project key"
    );
}

#[test]
fn typed_creates_persist_project_at_creation() {
    let temp = cards_repo("s2-create-project-typed");
    let repo = temp.path();

    run(
        repo,
        &["feature", "new", "Billing CSV", "--project", "svc-pay"],
    );
    assert_eq!(
        card_doc(repo, "billing-csv")["project"],
        "svc-pay",
        "feature new --project persists project on the folded card"
    );

    run(
        repo,
        &["task", "create", "Wire the export", "--project", "svc-pay"],
    );
    let task = id_by_title(repo, "Wire the export");
    assert_eq!(
        card_doc(repo, &task)["project"],
        "svc-pay",
        "task create --project persists project on the folded card"
    );

    // Plain open decision (no --lock): isolates create-time persistence from
    // the re-fold a lock would trigger (carry is exercised separately).
    run(
        repo,
        &["decision", "new", "Adopt X", "--project", "svc-pay"],
    );
    let decision = id_by_title(repo, "Adopt X");
    assert_eq!(
        card_doc(repo, &decision)["project"],
        "svc-pay",
        "decision new --project persists project on the folded card"
    );
}

#[test]
fn project_survives_a_typed_update_via_the_fold_carry() {
    let temp = cards_repo("s2-project-carry");
    let repo = temp.path();

    run(
        repo,
        &["task", "create", "Wire the export", "--project", "svc-pay"],
    );
    let task = id_by_title(repo, "Wire the export");
    // A typed update re-folds the card from its record; without the carry the
    // base-only `project` field would be wiped here.
    run(repo, &["task", "explore", &task]);
    assert_eq!(
        card_doc(repo, &task)["project"],
        "svc-pay",
        "project survives a task re-fold"
    );

    run(
        repo,
        &["decision", "new", "Adopt X", "--project", "svc-pay"],
    );
    let decision = id_by_title(repo, "Adopt X");
    run(
        repo,
        &[
            "decision",
            "lock",
            &decision,
            "--decision",
            "X",
            "--rejected",
            "Y: slower",
        ],
    );
    assert_eq!(
        card_doc(repo, &decision)["project"],
        "svc-pay",
        "project survives a decision re-fold"
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
        live.contains("Keep me open") && !live.contains(&swept_task) && !live.contains(&new_rule),
        "bare list keeps current work, hides the swept task and the coarse-closed rule:\n{live}"
    );
    let all = run(repo, &["list", "--all"]);
    assert!(
        all.contains("Keep me open") && all.contains(&new_rule) && !all.contains(&swept_task),
        "--all restores the live locked rule while archived sweeps stay gone:\n{all}"
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

#[test]
fn bare_list_bounds_to_the_live_slice_and_all_restores_closed() {
    let temp = cards_repo("s2-list-live-slice");
    let repo = temp.path();

    run(repo, &["create", "-t", "task", "Live one"]);
    run(repo, &["create", "-t", "task", "Live two"]);
    run(repo, &["create", "-t", "task", "Done one"]);
    let done = id_by_title(repo, "Done one");
    run(repo, &["update", &done, "--status", "closed"]);

    // ac-1: bare list hides the coarse-closed card and the header carries the delta.
    let bare = run(repo, &["list"]);
    assert!(
        bare.contains("2 live (1 closed hidden; --all)"),
        "bare list header shows the live count + hidden delta:\n{bare}"
    );
    assert!(
        bare.contains("Live one") && bare.contains("Live two") && !bare.contains(&done),
        "bare list shows open work, hides the closed card:\n{bare}"
    );

    // ac-2: --all restores the closed card and drops the slice header.
    let all = run(repo, &["list", "--all"]);
    assert!(
        all.contains("3 cards:") && all.contains(&done),
        "--all restores the closed card under the plain header:\n{all}"
    );

    // ac-2: an explicit --status filter governs without needing --all.
    let closed_only = run(repo, &["list", "--status", "closed"]);
    assert!(
        closed_only.contains(&done) && !closed_only.contains("Live one"),
        "an explicit --status filter governs the result on its own:\n{closed_only}"
    );

    // ac-3: bare --json emits the SAME sliced element set as bare list; --all restores.
    let bare_json: Value =
        serde_json::from_str(&run(repo, &["list", "--json"])).expect("bare list json");
    let bare_ids: Vec<&str> = bare_json["cards"]
        .as_array()
        .expect("cards array")
        .iter()
        .map(|c| c["id"].as_str().expect("card id"))
        .collect();
    assert!(
        bare_ids.len() == 2 && !bare_ids.contains(&done.as_str()),
        "bare list --json carries the live slice, not the closed card:\n{bare_ids:?}"
    );
    let all_json: Value =
        serde_json::from_str(&run(repo, &["list", "--all", "--json"])).expect("all list json");
    assert_eq!(
        all_json["cards"].as_array().expect("cards array").len(),
        3,
        "--all --json restores the full non-archived set"
    );
}

#[test]
fn link_add_show_graph_and_reverse_remove_round_trip() {
    let temp = cards_repo("s2-card-links-round-trip");
    let repo = temp.path();

    run(repo, &["create", "-t", "task", "Draft the loop"]);
    run(repo, &["create", "-t", "task", "Route the skill"]);
    let first = id_by_title(repo, "Draft the loop");
    let second = id_by_title(repo, "Route the skill");
    let ready_before: serde_json::Value =
        serde_json::from_str(&run(repo, &["ready", "--json"])).expect("ready json before");
    let list_before: serde_json::Value =
        serde_json::from_str(&run(repo, &["list", "--json"])).expect("list json before");

    let added = run(repo, &["link", "add", &first, &second]);
    assert!(
        added.contains(&format!(
            "{first} and {second} are now linked (messaging works both ways)"
        )),
        "link add confirms the bidirectional relation:\n{added}"
    );
    let duplicate = run(repo, &["link", "add", &first, &second]);
    assert!(
        duplicate.contains("already related"),
        "forward duplicate add is idempotent:\n{duplicate}"
    );
    let reverse_duplicate = run(repo, &["link", "add", &second, &first]);
    assert!(
        reverse_duplicate.contains("already related"),
        "reverse duplicate add is idempotent:\n{reverse_duplicate}"
    );

    let first_doc = card_doc(repo, &first);
    let deps = first_doc["deps"]
        .as_sequence()
        .expect("deps should be a YAML sequence");
    assert_eq!(deps.len(), 1, "only one related edge is stored");
    assert_eq!(deps[0]["kind"].as_str(), Some("related"));
    assert_eq!(deps[0]["target"].as_str(), Some(second.as_str()));
    let second_doc = card_doc(repo, &second);
    assert!(
        second_doc["deps"].as_sequence().is_none_or(Vec::is_empty),
        "reverse add does not write a reciprocal edge: {second_doc:?}"
    );

    let ready_after: serde_json::Value =
        serde_json::from_str(&run(repo, &["ready", "--json"])).expect("ready json after");
    let list_after: serde_json::Value =
        serde_json::from_str(&run(repo, &["list", "--json"])).expect("list json after");
    assert_eq!(
        ready_before, ready_after,
        "related links do not change the ready JSON contract"
    );
    assert_eq!(
        list_before, list_after,
        "related links do not change the list JSON contract"
    );

    let first_show = run(repo, &["show", &first]);
    assert!(
        first_show.contains(&format!("related: {second}")),
        "show renders the local related edge:\n{first_show}"
    );
    let second_show = run(repo, &["show", &second]);
    assert!(
        second_show.contains(&format!("related by: {first}")),
        "show renders the reverse related edge:\n{second_show}"
    );
    let graph = run(repo, &["query", "graph", &first]);
    assert!(
        graph.contains(&format!("- related: {second}")),
        "query graph still sees the relation:\n{graph}"
    );

    let removed = run(repo, &["link", "remove", &second, &first]);
    assert!(
        removed.contains(&format!(
            "removed related link between {second} and {first}"
        )),
        "reverse remove confirms the deleted relation:\n{removed}"
    );
    let first_doc = card_doc(repo, &first);
    assert!(
        first_doc["deps"].as_sequence().is_none_or(Vec::is_empty),
        "reverse remove deletes the one stored edge: {first_doc:?}"
    );
}

#[test]
fn dep_remove_unblocks_the_child_and_is_directional() {
    let temp = cards_repo("s2-card-dep-remove");
    let repo = temp.path();

    run(repo, &["create", "-t", "task", "Blocked work"]);
    run(repo, &["create", "-t", "task", "The blocker"]);
    let child = id_by_title(repo, "Blocked work");
    let parent = id_by_title(repo, "The blocker");

    run(repo, &["dep", "add", &child, &parent]);
    let blocked_ready = run(repo, &["ready", "--json"]);
    assert!(
        !blocked_ready.contains(&child),
        "the dependent is not ready while blocked:\n{blocked_ready}"
    );
    let child_show = run(repo, &["show", &child]);
    assert!(
        child_show.contains(&format!("blocked by: {parent}")),
        "show renders the blocking edge:\n{child_show}"
    );

    // The edge lives on the child, not the parent, so reverse-order remove is a no-op.
    let reverse = run(repo, &["dep", "remove", &parent, &child]);
    assert!(
        reverse.contains(&format!("{parent} is not blocked by {child}")),
        "reverse-order remove is a no-op:\n{reverse}"
    );
    let still_show = run(repo, &["show", &child]);
    assert!(
        still_show.contains(&format!("blocked by: {parent}")),
        "the real edge survives a reverse-order remove:\n{still_show}"
    );

    let removed = run(repo, &["dep", "remove", &child, &parent]);
    assert!(
        removed.contains(&format!("{child} is no longer blocked by {parent}")),
        "the correctly-ordered remove confirms:\n{removed}"
    );
    let child_doc = card_doc(repo, &child);
    assert!(
        child_doc["deps"].as_sequence().is_none_or(Vec::is_empty),
        "the blocks edge is deleted: {child_doc:?}"
    );
    let unblocked_ready = run(repo, &["ready", "--json"]);
    assert!(
        unblocked_ready.contains(&child),
        "the dependent is ready once unblocked:\n{unblocked_ready}"
    );

    let again = run(repo, &["dep", "remove", &child, &parent]);
    assert!(
        again.contains(&format!("{child} is not blocked by {parent}")),
        "removing an absent edge is an idempotent no-op:\n{again}"
    );
}

#[test]
fn link_rejects_self_missing_archived_and_traversal_ids() {
    let temp = cards_repo("s2-card-links-guards");
    let repo = temp.path();

    run(repo, &["create", "-t", "task", "Primary"]);
    run(repo, &["create", "-t", "task", "Archive me"]);
    let primary = id_by_title(repo, "Primary");
    let archived = id_by_title(repo, "Archive me");

    let self_link = run_err(repo, &["link", "add", &primary, &primary]);
    assert!(
        self_link.contains("cannot link to itself"),
        "self-link is rejected:\n{self_link}"
    );
    let missing = run_err(repo, &["link", "add", &primary, "missing-card"]);
    assert!(
        missing.contains("no live card missing-card"),
        "missing live id is rejected:\n{missing}"
    );

    run(repo, &["update", &archived, "--status", "abandoned"]);
    run(repo, &["archive", "--loose"]);
    let archived_link = run_err(repo, &["link", "add", &primary, &archived]);
    assert!(
        archived_link.contains(&format!("no live card {archived}")),
        "archived-only id is rejected:\n{archived_link}"
    );

    for args in [
        vec!["link", "add", "../../outside", &primary],
        vec!["link", "remove", &primary, "../../outside"],
    ] {
        let refused = run_err(repo, &args);
        assert!(
            refused.contains("invalid card id"),
            "{args:?} refuses traversal-shaped ids:\n{refused}"
        );
    }
}

/// dec-terminal-card-link-msg-keep-the-live-5878: a terminal partner refuses a
/// NEW link/channel with an honest dead-end (never the circular "run link add"),
/// but a partner you were ALREADY linked to before it finished stays messageable.
#[test]
fn terminal_partner_refuses_new_link_and_channel_but_keeps_existing() {
    let temp = cards_repo("s2-terminal-link-msg");
    let repo = temp.path();

    maestro_in_session(repo, "setup", &["create", "-t", "chore", "Sender"]);
    let sender = id_by_title(repo, "Sender");
    maestro_in_session(repo, "setup", &["create", "-t", "chore", "Finished"]);
    let finished = id_by_title(repo, "Finished");
    maestro_in_session(repo, "setup", &["close", &finished]);
    maestro_in_session(repo, "setup", &["create", "-t", "chore", "Ally"]);
    let ally = id_by_title(repo, "Ally");
    maestro_in_session(repo, "setup", &["link", "add", &sender, &ally]);
    maestro_in_session(repo, "setup", &["close", &ally]);

    // link add to a terminal card: honest, names the finished card.
    let link_err = run_err(repo, &["link", "add", &sender, &finished]);
    assert!(
        link_err.contains(&format!("{finished} is finished")),
        "link add to terminal names the finished card:\n{link_err}"
    );
    assert!(
        link_err.contains("you can't open a new conversation"),
        "link add terminal wording is honest:\n{link_err}"
    );

    // Bind the running session to the live sender, then exercise msg send.
    maestro_in_session(repo, "msgsess", &["note", &sender, "bind"]);

    // msg send to a terminal UNLINKED partner: finished / no channel, NEVER link add.
    let send_terminal =
        maestro_in_session(repo, "msgsess", &["msg", "send", &finished, "closing?"]);
    assert!(
        !send_terminal.status.success(),
        "send to a terminal unlinked partner fails"
    );
    let send_terminal_err = String::from_utf8_lossy(&send_terminal.stderr);
    assert!(
        send_terminal_err.contains(&format!("{finished} is finished")),
        "send terminal names the finished partner:\n{send_terminal_err}"
    );
    assert!(
        send_terminal_err.contains("no channel can be opened"),
        "send terminal states no channel:\n{send_terminal_err}"
    );
    assert!(
        !send_terminal_err.contains("link add"),
        "send terminal must NOT point back at link add:\n{send_terminal_err}"
    );

    // Messaging an already-linked terminal partner still works.
    let send_ally = maestro_in_session(repo, "msgsess", &["msg", "send", &ally, "thanks"]);
    assert!(
        send_ally.status.success(),
        "send to an already-linked terminal partner still works\nstderr:\n{}",
        String::from_utf8_lossy(&send_ally.stderr)
    );
    assert!(
        String::from_utf8_lossy(&send_ally.stdout).contains(&format!("sent to {ally}")),
        "send to an already-linked terminal partner confirms\nstdout:\n{}",
        String::from_utf8_lossy(&send_ally.stdout)
    );
}

/// ac-4: `msg list` labels the viewer count `your unread: N` and shows a partner
/// read-through indicator derived from the partner's cursor -- absent until the
/// partner actually reads, present (a ts) afterwards.
#[test]
fn msg_list_relabels_unread_and_shows_partner_read_through_after_they_read() {
    let temp = cards_repo("s2-msg-readthrough");
    let repo = temp.path();

    maestro_in_session(repo, "setup", &["create", "-t", "chore", "Sender"]);
    let sender = id_by_title(repo, "Sender");
    maestro_in_session(repo, "setup", &["create", "-t", "chore", "Peer"]);
    let peer = id_by_title(repo, "Peer");
    maestro_in_session(repo, "setup", &["link", "add", &sender, &peer]);

    // Bind the running session to the sender and send a message to the peer.
    maestro_in_session(repo, "asess", &["note", &sender, "bind"]);
    let sent = maestro_in_session(repo, "asess", &["msg", "send", &peer, "deal?"]);
    assert!(sent.status.success(), "send should succeed");

    // Before the peer reads: viewer count relabeled, NO read-through indicator.
    let before = maestro_in_session(repo, "asess", &["msg", "list"]);
    let before_out = String::from_utf8_lossy(&before.stdout);
    assert!(
        before_out.contains("your unread: 0"),
        "the viewer count is relabeled 'your unread: N':\n{before_out}"
    );
    assert!(
        !before_out.contains("peer read through"),
        "no read-through until the peer reads:\n{before_out}"
    );

    // Peer binds and reads, advancing its cursor.
    maestro_in_session(repo, "bsess", &["note", &peer, "bind"]);
    maestro_in_session(repo, "bsess", &["msg", "read"]);

    // After the peer reads: the read-through indicator appears, adjacent to the
    // relabeled count, derived from the peer's cursor.
    let after = maestro_in_session(repo, "asess", &["msg", "list"]);
    let after_out = String::from_utf8_lossy(&after.stdout);
    assert!(
        after_out.contains("your unread: 0") && after_out.contains("peer read through "),
        "after the peer reads, msg list shows the read-through ts:\n{after_out}"
    );
}

/// ac-5: `msg send` echoes the acting/sender card, and the `msg list` overview
/// shows the last message's direction (whose turn it is to reply).
#[test]
fn msg_send_echoes_sender_and_list_shows_direction() {
    let temp = cards_repo("s2-msg-direction");
    let repo = temp.path();

    maestro_in_session(repo, "setup", &["create", "-t", "chore", "Alpha"]);
    let alpha = id_by_title(repo, "Alpha");
    maestro_in_session(repo, "setup", &["create", "-t", "chore", "Bravo"]);
    let bravo = id_by_title(repo, "Bravo");
    maestro_in_session(repo, "setup", &["link", "add", &alpha, &bravo]);

    // Alpha sends: the confirmation names the acting/sender card.
    maestro_in_session(repo, "asess", &["note", &alpha, "bind"]);
    let sent = maestro_in_session(repo, "asess", &["msg", "send", &bravo, "your move"]);
    let sent_out = String::from_utf8_lossy(&sent.stdout);
    assert!(
        sent_out.contains(&format!("sent to {bravo} (from {alpha})")),
        "send echoes the sender card:\n{sent_out}"
    );

    // Alpha spoke last -> Alpha's overview reads 'from you' (Bravo's turn).
    let alpha_view = maestro_in_session(repo, "asess", &["msg", "list"]);
    let alpha_out = String::from_utf8_lossy(&alpha_view.stdout);
    assert!(
        alpha_out.contains("(from you)"),
        "the sender's overview shows the last message was from them:\n{alpha_out}"
    );

    // Bravo replies; now Bravo spoke last -> Alpha's overview reads 'from them'.
    maestro_in_session(repo, "bsess", &["note", &bravo, "bind"]);
    maestro_in_session(repo, "bsess", &["msg", "send", &alpha, "on it"]);
    let alpha_after = maestro_in_session(repo, "asess", &["msg", "list"]);
    let alpha_after_out = String::from_utf8_lossy(&alpha_after.stdout);
    assert!(
        alpha_after_out.contains("(from them)"),
        "after the partner replies the overview shows the last was from them:\n{alpha_after_out}"
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
        vec!["link", "add", "../../outside", "task-001"],
        vec!["link", "remove", "task-001", "../../outside"],
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

    let cases: [Vec<&str>; 5] = [
        vec!["create", "-t", "task", "X"],
        vec!["show", "anything"],
        vec!["update", "anything", "--status", "closed"],
        vec!["close", "anything"],
        vec!["link", "add", "anything", "other"],
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

/// Drive a verb under a fixed session id. `MAESTRO_SESSION_ID` is the first key
/// `cli_run_id()` reads, so it wins over any ambient agent-runtime var and the
/// `card_touch` run event lands in a deterministic `runs/<session>/` bucket.
fn maestro_in_session(cwd: &Path, session: &str, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .current_dir(cwd)
        .env("MAESTRO_AGENT", "codex")
        .env("MAESTRO_SESSION_ID", session)
        .output()
        .expect("invariant: compiled maestro binary should run in integration tests")
}

/// The parsed `card_touch` run events recorded in a session bucket, in append
/// order. Missing bucket -> no events.
fn card_touch_events(repo: &Path, session: &str) -> Vec<Value> {
    let path = repo
        .join(".maestro/runs")
        .join(session)
        .join("events.jsonl");
    let raw = fs::read_to_string(path).unwrap_or_default();
    raw.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Value>(line).expect("event line should be valid JSON"))
        .filter(|event| event["event_type"] == "card_touch")
        .collect()
}

#[test]
fn card_mutating_verbs_auto_emit_card_touch_tagged_session_and_card() {
    let temp = cards_repo("t3-card-touch");
    let repo = temp.path();
    let session = "t3sess";

    maestro_in_session(repo, session, &["create", "-t", "chore", "First chore"]);
    let first = id_by_title(repo, "First chore");
    maestro_in_session(repo, session, &["create", "-t", "chore", "Second chore"]);
    let second = id_by_title(repo, "Second chore");
    // A non-create mutation (update) also binds: it is the most recent touch.
    let updated = maestro_in_session(
        repo,
        session,
        &["update", &first, "--description", "touched"],
    );
    assert!(
        updated.status.success(),
        "update exits 0\nstderr:\n{}",
        String::from_utf8_lossy(&updated.stderr)
    );

    let touches = card_touch_events(repo, session);
    assert_eq!(
        touches.len(),
        3,
        "each of create/create/update emits one card_touch: {touches:#?}"
    );
    assert!(
        touches.iter().all(|event| event["session_id"] == session),
        "every card_touch carries the resolved session id: {touches:#?}"
    );
    assert!(
        touches
            .iter()
            .any(|event| event["card_id"] == Value::String(second.clone())),
        "the second card's id is bound: {touches:#?}"
    );
    // latest-touch-wins (D3): the running session's current card is the most
    // recent card_touch, here the `update` of the first card.
    assert_eq!(
        touches.last().expect("at least one touch")["card_id"],
        Value::String(first.clone()),
        "current card resolves to the most recent touch: {touches:#?}"
    );
}

#[test]
fn card_touch_emit_is_non_fatal_when_the_run_log_cannot_be_written() {
    let temp = cards_repo("t3-nonfatal");
    let repo = temp.path();
    // A regular file where the runs tree belongs makes the event append fail; the
    // mutating verb must still succeed (emit is best-effort, mirrors hook record).
    fs::write(repo.join(".maestro/runs"), b"not a directory")
        .expect("invariant: runs sentinel file should be writable");

    let created = maestro_in_session(repo, "t3x", &["create", "-t", "chore", "Still works"]);
    assert!(
        created.status.success(),
        "create exits 0 even when the card_touch append fails\nstderr:\n{}",
        String::from_utf8_lossy(&created.stderr)
    );
    let id = id_by_title(repo, "Still works");
    assert!(
        id.starts_with("chore-"),
        "the card was still created despite the failed emit: {id}"
    );
}

/// Dense JSON encoding: multi-item read verbs emit a single compact line while
/// single-item verbs stay pretty-printed. The envelope and fields are unchanged,
/// so every `from_str`-into-one-document consumer keeps working.
#[test]
fn multi_item_json_verbs_emit_single_compact_line() {
    let temp = cards_repo("s2-dense-json-encoding");
    let repo = temp.path();

    run(repo, &["create", "-t", "task", "Alpha card"]);
    run(repo, &["create", "-t", "task", "Beta card"]);

    for verb in ["list", "ready", "status"] {
        let out = run(repo, &[verb, "--json"]);
        let trimmed = out.trim_end_matches('\n');
        assert!(
            !trimmed.contains('\n'),
            "{verb} --json must be a single compact line, got:\n{out}"
        );
        let value: Value = serde_json::from_str(trimmed).unwrap_or_else(|e| {
            panic!("{verb} --json must parse as one JSON document: {e}\n{out}")
        });
        assert!(value.is_object(), "{verb} --json stays an envelope object");
    }

    let list_out = run(repo, &["list", "--json"]);
    let list_value: Value =
        serde_json::from_str(list_out.trim_end()).expect("list --json parses as one document");
    assert_eq!(list_value["schema"], Value::from("maestro.list.v1"));
    assert!(
        list_value["cards"]
            .as_array()
            .is_some_and(|cards| cards.len() >= 2),
        "list --json keeps the {{version,schema,cards}} envelope:\n{list_out}"
    );

    let first = id_by_title(repo, "Alpha card");
    let show = run(repo, &["show", &first, "--json"]);
    assert!(
        show.trim_end().contains('\n'),
        "single-item show --json stays pretty-printed (multi-line):\n{show}"
    );
}
