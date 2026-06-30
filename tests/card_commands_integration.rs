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

use card_support::{card_doc, card_record_path, cards_repo, id_by_title};
use maestro::domain::channel;
use maestro::foundation::core::paths::MaestroPaths;
use serde_json::Value;
use support::TestTempDir;

fn maestro(cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .current_dir(cwd)
        .env("MAESTRO_AGENT", "codex")
        .env("MAESTRO_SESSION", "s1")
        .env("MAESTRO_AUTO_UPDATE", "0")
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

fn git(cwd: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("invariant: git should be runnable in integration tests");
    assert!(
        output.status.success(),
        "git {args:?} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn write_claims_only_harness(repo: &Path) {
    let harness = repo.join(".maestro/harness");
    fs::create_dir_all(&harness).expect("invariant: harness dir should be creatable");
    fs::write(
        harness.join("harness.yml"),
        concat!(
            "schema_version: maestro.harness.v1\n",
            "stack:\n",
            "  kind: generic\n",
            "  detected_by: []\n",
            "  verify: []\n",
            "claims_only_verification: true\n",
        ),
    )
    .expect("invariant: harness should be writable");
}

fn seed_legacy_sidecar_archive(repo: &Path) {
    let legacy = repo.join(".maestro/archive/cards/legacy-sidecar-feature");
    fs::create_dir_all(&legacy).expect("invariant: legacy archive dir should be creatable");
    fs::write(
        legacy.join("card.yaml"),
        r#"schema_version: maestro.card.v1
id: legacy-sidecar-feature
type: feature
title: Legacy Sidecar Feature
status: shipped
created_at: "1"
updated_at: "1"
description: cardtokenx lives in the archived card record
"#,
    )
    .expect("invariant: legacy archived card should be writable");
    fs::write(
        legacy.join("notes.md"),
        "# Legacy Sidecar Feature\n\nnotestokenx from archived notes\n",
    )
    .expect("invariant: legacy archived notes should be writable");
    fs::write(
        legacy.join("spec.md"),
        "# Legacy Sidecar Feature\n\n## Problem\n\nspectokenx from archived spec\n",
    )
    .expect("invariant: legacy archived spec should be writable");
    fs::write(
        legacy.join("qa.md"),
        "### QA Baseline Contract\n\nqatokenx from archived qa\n",
    )
    .expect("invariant: legacy archived qa should be writable");
    fs::write(
        legacy.join("decisions.yaml"),
        r#"- schema_version: maestro.card.v1
  id: dec-sidecar-token
  type: decision
  title: Archived sidecar decision
  status: locked
  parent: legacy-sidecar-feature
  created_at: "1"
  updated_at: "1"
  description: decisiontokenx from archived decisions
"#,
    )
    .expect("invariant: legacy archived decisions should be writable");
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
fn legacy_workable_cards_remain_readable_after_progress_card_addition() {
    let temp = cards_repo("legacy-workable-readable");
    let repo = temp.path();

    run(repo, &["create", "-t", "task", "Legacy task card"]);
    run(repo, &["create", "-t", "bug", "Legacy bug card"]);
    run(repo, &["create", "-t", "chore", "Legacy chore card"]);
    let task_id = id_by_title(repo, "Legacy task card");
    let bug_id = id_by_title(repo, "Legacy bug card");
    let chore_id = id_by_title(repo, "Legacy chore card");

    let tasks = run(repo, &["list", "--type", "task"]);
    assert!(
        tasks.contains(&task_id),
        "task card remains listable:\n{tasks}"
    );
    assert!(
        !tasks.contains(&bug_id) && !tasks.contains(&chore_id),
        "--type task still filters to task cards only:\n{tasks}"
    );

    for (id, kind) in [
        (task_id.as_str(), "task"),
        (bug_id.as_str(), "bug"),
        (chore_id.as_str(), "chore"),
    ] {
        let shown = run(repo, &["show", id]);
        assert!(
            shown.contains(kind) && shown.contains(id),
            "{kind} card remains readable:\n{shown}"
        );
    }

    let ready = run(repo, &["ready"]);
    assert!(
        ready.contains(&task_id) && ready.contains(&bug_id) && ready.contains(&chore_id),
        "legacy workable cards remain on the ready board:\n{ready}"
    );
}

#[test]
fn progress_card_queries_show_progress_card_and_task_list_shows_low_tasks() {
    let temp = cards_repo("progress-card-query");
    let repo = temp.path();

    let task_id = run(repo, &["task", "add", "Fix typo", "--id-only"])
        .trim()
        .to_string();
    let progress_id = id_by_title(repo, "Progress for maestro");

    let progress_cards = run(repo, &["list", "--type", "progress"]);
    assert!(
        progress_cards.contains(&progress_id) && progress_cards.contains("Progress for maestro"),
        "progress cards are queryable by type:\n{progress_cards}"
    );
    assert!(
        !progress_cards.contains(&task_id),
        "low-level tasks stay inside progress.yml, not card list rows:\n{progress_cards}"
    );

    let shown = run(repo, &["show", &progress_id]);
    assert!(
        shown.contains("progress") && shown.contains(&progress_id),
        "show renders the progress card:\n{shown}"
    );

    let tasks = run(repo, &["task", "list"]);
    assert!(
        tasks.contains("REF")
            && tasks.contains("STATE")
            && tasks.contains("NEXT")
            && tasks.contains("TITLE")
            && tasks.contains("Fix typo"),
        "task list renders low-level progress tasks:\n{tasks}"
    );
    assert!(
        !tasks.contains(&task_id),
        "routine task list hides low-level task ids; use --json for stable ids:\n{tasks}"
    );

    let ready = run(repo, &["ready"]);
    assert!(
        !ready.contains(&progress_id) && !ready.contains(&task_id),
        "progress cards and their low tasks do not enter the legacy card ready board:\n{ready}"
    );
}

#[test]
fn custom_card_requires_kind_prepares_owned_tasks_and_closes_after_verification() {
    let temp = cards_repo("custom-card-container-flow");
    let repo = temp.path();
    write_claims_only_harness(repo);

    let missing_kind = run_err(repo, &["card", "create", "-t", "custom", "Inbox polish"]);
    assert!(
        missing_kind.contains("custom cards require --kind"),
        "{missing_kind}"
    );

    let created = run(
        repo,
        &[
            "card",
            "create",
            "-t",
            "custom",
            "--kind",
            "ui",
            "Inbox polish",
        ],
    );
    assert!(created.contains("(custom)"), "{created}");
    let custom_id = id_by_title(repo, "Inbox polish");
    assert!(
        custom_id.starts_with("custom-inbox-polish-"),
        "custom card id carries custom prefix: {custom_id}"
    );
    let custom = card_doc(repo, &custom_id);
    assert_eq!(custom["type"].as_str(), Some("custom"));
    assert_eq!(custom["status"].as_str(), Some("proposed"));
    assert_eq!(custom["extra"]["kind"].as_str(), Some("ui"));

    let prepared = run(
        repo,
        &[
            "card",
            "prepare",
            &custom_id,
            "--task",
            "T1: Build inbox UI",
            "--check",
            "inbox UI renders",
        ],
    );
    assert!(prepared.contains("prepared 1 task(s)"), "{prepared}");
    let task_id = id_by_title(repo, "Build inbox UI");
    assert_eq!(
        card_doc(repo, &task_id)["parent"].as_str(),
        Some(custom_id.as_str())
    );

    let close_before_verify = run_err(repo, &["card", "close", &custom_id]);
    assert!(
        close_before_verify.contains("owned task(s) are not verified"),
        "{close_before_verify}"
    );

    run(repo, &["task", "claim", &task_id]);
    run(
        repo,
        &[
            "task",
            "complete",
            &task_id,
            "--summary",
            "built inbox UI",
            "--claim",
            "inbox UI renders",
            "--proof",
            "inbox UI renders",
        ],
    );

    let closed = run(repo, &["card", "close", &custom_id]);
    assert!(closed.contains("closed"), "{closed}");
    let custom = card_doc(repo, &custom_id);
    assert_eq!(custom["status"].as_str(), Some("closed"));
}

#[test]
fn bug_card_prepares_owned_tasks_and_closes_after_verification() {
    let temp = cards_repo("bug-card-container-flow");
    let repo = temp.path();
    write_claims_only_harness(repo);

    run(repo, &["card", "create", "-t", "bug", "Fix parser panic"]);
    let bug_id = id_by_title(repo, "Fix parser panic");

    let prepared = run(
        repo,
        &[
            "card",
            "prepare",
            &bug_id,
            "--task",
            "T1: Patch parser panic",
            "--check",
            "parser no longer panics",
        ],
    );
    assert!(prepared.contains("prepared 1 task(s)"), "{prepared}");
    assert!(
        !repo
            .join(".maestro/cards")
            .join(&bug_id)
            .join("prepare-inline.md")
            .exists(),
        "inline card prepare file should be cleaned up after a successful prepare"
    );
    let task_id = id_by_title(repo, "Patch parser panic");
    assert_eq!(
        card_doc(repo, &task_id)["parent"].as_str(),
        Some(bug_id.as_str())
    );

    let close_before_verify = run_err(repo, &["card", "close", &bug_id]);
    assert!(
        close_before_verify.contains("owned task(s) are not verified"),
        "{close_before_verify}"
    );

    run(repo, &["task", "claim", &task_id]);
    run(
        repo,
        &[
            "task",
            "complete",
            &task_id,
            "--summary",
            "patched parser panic",
            "--claim",
            "parser no longer panics",
            "--proof",
            "parser no longer panics",
        ],
    );

    let closed = run(repo, &["card", "close", &bug_id]);
    assert!(closed.contains("closed"), "{closed}");
    let bug = card_doc(repo, &bug_id);
    assert_eq!(bug["status"].as_str(), Some("closed"));
}

#[test]
fn chore_card_with_owned_simple_tasks_closes_after_tasks_are_done() {
    let temp = cards_repo("chore-owned-task-close");
    let repo = temp.path();

    run(repo, &["card", "create", "-t", "chore", "Clean docs"]);
    let chore_id = id_by_title(repo, "Clean docs");
    run(
        repo,
        &["task", "add", "--card", &chore_id, "Fix README heading"],
    );
    let task_id = id_by_title(repo, "Fix README heading");

    let close_before_done = run_err(repo, &["card", "close", &chore_id]);
    assert!(
        close_before_done.contains("owned task(s) are not verified"),
        "{close_before_done}"
    );

    run(repo, &["task", "start", &task_id]);
    run(
        repo,
        &["task", "done", &task_id, "--proof", "Fix README heading"],
    );

    let closed = run(repo, &["card", "close", &chore_id]);
    assert!(closed.contains("closed"), "{closed}");
    let chore = card_doc(repo, &chore_id);
    assert_eq!(chore["status"].as_str(), Some("closed"));
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

/// Plant a `.maestro/harness/harness.yml` declaring `projects:` so the
/// folder->project auto-infer (T3) activates. The body is the minimal valid
/// `HarnessConfig` plus the declared scopes.
fn write_projects_declaration(repo: &Path, patterns: &[&str]) {
    let dir = repo.join(".maestro/harness");
    fs::create_dir_all(&dir).expect("invariant: harness dir should be creatable");
    let mut yaml = String::from(
        "schema_version: maestro.harness.v1\n\
         stack:\n  kind: generic\n  detected_by: []\n  verify: []\n\
         projects:\n",
    );
    for pattern in patterns {
        yaml.push_str(&format!("  - \"{pattern}\"\n"));
    }
    fs::write(dir.join("harness.yml"), yaml).expect("invariant: harness.yml should be writable");
}

/// T3 ac-2/ac-3: with `projects: ["*"]` declared, a card created from a
/// subfolder infers `project` from the first path segment with no `--project`
/// flag, driven through the real binary's `current_dir`.
#[test]
fn create_infers_project_from_subfolder_under_top_level_wildcard() {
    let temp = cards_repo("s2-infer-wildcard");
    let repo = temp.path();
    write_projects_declaration(repo, &["*"]);
    let subfolder = repo.join("svc-pay/src");
    fs::create_dir_all(&subfolder).expect("invariant: subfolder should be creatable");

    run(&subfolder, &["create", "-t", "bug", "Race in checkout"]);
    let bug = id_by_title(repo, "Race in checkout");
    assert_eq!(
        card_doc(repo, &bug)["project"],
        "svc-pay",
        "a card created under svc-pay/ infers project=svc-pay with no flag"
    );
}

/// T3: `--project` always overrides inference -- the explicit name wins even
/// when the cwd would infer a different segment.
#[test]
fn explicit_project_flag_overrides_inference() {
    let temp = cards_repo("s2-infer-override");
    let repo = temp.path();
    write_projects_declaration(repo, &["*"]);
    let subfolder = repo.join("svc-pay/src");
    fs::create_dir_all(&subfolder).expect("invariant: subfolder should be creatable");

    run(
        &subfolder,
        &[
            "create",
            "-t",
            "bug",
            "Shared concern",
            "--project",
            "shared",
        ],
    );
    let bug = id_by_title(repo, "Shared concern");
    assert_eq!(
        card_doc(repo, &bug)["project"],
        "shared",
        "--project overrides the svc-pay the cwd would infer"
    );
}

/// T3 activation gate: with NO `projects:` declared, the same subfolder create
/// infers nothing -- inference is off until a repo opts in.
#[test]
fn create_without_projects_declaration_infers_nothing() {
    let temp = cards_repo("s2-infer-no-declaration");
    let repo = temp.path();
    let subfolder = repo.join("svc-pay/src");
    fs::create_dir_all(&subfolder).expect("invariant: subfolder should be creatable");

    run(&subfolder, &["create", "-t", "bug", "No scope here"]);
    let bug = id_by_title(repo, "No scope here");
    assert!(
        card_doc(repo, &bug).get("project").is_none(),
        "no projects: declaration means no inference"
    );
}

#[test]
fn create_reports_malformed_projects_declaration_instead_of_dropping_scope() {
    let temp = cards_repo("s2-infer-bad-config");
    let repo = temp.path();
    let harness_dir = repo.join(".maestro/harness");
    fs::create_dir_all(&harness_dir).expect("invariant: harness dir should be creatable");
    fs::write(
        harness_dir.join("harness.yml"),
        "schema_version: maestro.harness.v1\nprojects: [\n",
    )
    .expect("invariant: harness.yml should be writable");
    let subfolder = repo.join("svc-pay/src");
    fs::create_dir_all(&subfolder).expect("invariant: subfolder should be creatable");

    let stderr = run_err(&subfolder, &["create", "-t", "bug", "Bad scope"]);

    assert!(
        stderr.contains("failed to parse") && stderr.contains("harness.yml"),
        "malformed project config should be reported, not treated as no inference:\n{stderr}"
    );
}

/// T3 ac-11 guard: a card created from any subfolder still lands in the single
/// root `.maestro/cards/`; the subfolder cwd does not split the store.
#[test]
fn subfolder_create_lands_in_the_single_root_card_store() {
    let temp = cards_repo("s2-infer-single-store");
    let repo = temp.path();
    write_projects_declaration(repo, &["*"]);
    let subfolder = repo.join("svc-pay/src");
    fs::create_dir_all(&subfolder).expect("invariant: subfolder should be creatable");

    run(&subfolder, &["create", "-t", "bug", "Lands at root"]);
    let bug = id_by_title(repo, "Lands at root");
    let record = card_support::card_record_path(repo, &bug);
    assert!(
        record.starts_with(repo.join(".maestro/cards")),
        "the card created from svc-pay/src lives under the single root .maestro/cards/, \
         got {}",
        record.display()
    );
    assert!(
        !subfolder.join(".maestro").exists(),
        "no nested .maestro/ store is created under the subfolder"
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

/// `card list --grep` may use the unified memory shard as an accelerator, but
/// must keep the legacy exact card predicate, row order, human rows, and JSON
/// envelope. Rich alias recall belongs to `maestro grep`, not card-list grep.
#[test]
fn list_grep_uses_unified_candidates_without_alias_ranking_or_source_leakage() {
    let temp = cards_repo("s2-grep-card-list-compat");
    let repo = temp.path();
    let memory_shard = repo.join(".maestro/index/search/memory.shard");
    fs::create_dir_all(repo.join("src")).expect("invariant: src dir should be creatable");
    fs::write(
        repo.join("src/runtime_source.rs"),
        "fn sourceonly_runtime_symbol() {}\n",
    )
    .expect("invariant: source fixture should be writable");

    run(
        repo,
        &[
            "create",
            "-t",
            "task",
            "Alpha runtime literal",
            "--description",
            "contains the exact runtime token",
        ],
    );
    run(
        repo,
        &[
            "create",
            "-t",
            "task",
            "Session alias only",
            "--description",
            "contains session without the queried word",
        ],
    );
    run(
        repo,
        &[
            "create",
            "-t",
            "task",
            "Zulu runtime literal",
            "--description",
            "another exact runtime token",
        ],
    );
    let alpha_id = id_by_title(repo, "Alpha runtime literal");
    let alias_id = id_by_title(repo, "Session alias only");
    let zulu_id = id_by_title(repo, "Zulu runtime literal");

    let unfiltered = run(repo, &["list", "--type", "task"]);
    assert!(
        !memory_shard.exists(),
        "no memory shard before card-list grep"
    );

    let filtered = run(repo, &["list", "--grep", "runtime"]);
    assert!(
        memory_shard.exists(),
        "card-list grep should consult the unified memory shard as an accelerator"
    );
    assert!(filtered.contains(&alpha_id), "{filtered}");
    assert!(filtered.contains(&zulu_id), "{filtered}");
    assert!(
        !filtered.contains(&alias_id),
        "card-list grep is exact substring, not alias recall:\n{filtered}"
    );
    assert!(
        !filtered.contains("runtime_source.rs"),
        "card-list grep never renders source hits:\n{filtered}"
    );
    assert_eq!(
        unfiltered.find(&alpha_id).unwrap() < unfiltered.find(&zulu_id).unwrap(),
        filtered.find(&alpha_id).unwrap() < filtered.find(&zulu_id).unwrap(),
        "card-list grep preserves scan/list order instead of ranked grep order"
    );

    let source_only = run(repo, &["list", "--grep", "sourceonly"]);
    assert!(source_only.contains("no cards match"), "{source_only}");
    assert!(
        !source_only.contains("runtime_source.rs"),
        "source-only terms do not become card-list rows:\n{source_only}"
    );

    let json_out = run(repo, &["list", "--grep", "runtime", "--json"]);
    let json: Value = serde_json::from_str(&json_out).expect("list output should be JSON");
    assert_eq!(json["schema"], "maestro.list.v1");
    let cards = json["cards"].as_array().expect("cards should be an array");
    assert_eq!(cards.len(), 2, "{json:#}");
    assert_eq!(cards[0]["id"], alpha_id);
    assert_eq!(cards[1]["id"], zulu_id);
    assert!(
        cards
            .iter()
            .all(|card| card.get("corpus").is_none() && card.get("score_reasons").is_none()),
        "list JSON envelope must not grow grep-ranking/source fields:\n{json:#}"
    );

    let rich = run(repo, &["grep", "--json", "runtime corpus:memory"]);
    let rich_json: Value = serde_json::from_str(&rich).expect("grep output should be JSON");
    let alias_hit = rich_json["hits"]
        .as_array()
        .expect("grep hits should be an array")
        .iter()
        .find(|hit| hit["id"] == alias_id)
        .unwrap_or_else(|| panic!("maestro grep should keep alias recall:\n{rich_json:#}"));
    assert!(
        alias_hit["score_reasons"]
            .as_array()
            .expect("score reasons should be an array")
            .iter()
            .any(|reason| reason["factor"] == "domain_alias"),
        "rich grep, not card-list grep, owns alias recall:\n{rich_json:#}"
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
        repo.join(".maestro/archive/cards.sqlite").is_file(),
        "loose archive writes DB-backed snapshots"
    );
    assert!(
        !repo
            .join(".maestro/archive/cards/tasks")
            .join(&swept_task)
            .exists()
    );
    let live_decisions = fs::read_to_string(repo.join(".maestro/cards/decisions.yaml"))
        .expect("invariant: live decisions.yaml should exist");
    // The kept rule's `supersedes:` field still references the old id, so
    // absence is asserted on the swept entry's title, not its id.
    assert!(
        live_decisions.contains(&new_rule) && !live_decisions.contains("Tabs or spaces"),
        "only the superseded entry leaves the live file:\n{live_decisions}"
    );
    let archived_decisions = run(repo, &["show", &old_rule]);
    assert!(
        archived_decisions.contains(&old_rule),
        "the superseded entry reads back from the archive DB:\n{archived_decisions}"
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
fn archive_migrate_db_imports_legacy_folders_and_cleanup_removes_quarantine() {
    let temp = cards_repo("s2-archive-db-migration");
    let repo = temp.path();

    let legacy = repo.join(".maestro/archive/cards/legacy-feature");
    fs::create_dir_all(&legacy).expect("invariant: legacy archive dir should be creatable");
    fs::write(
        legacy.join("card.yaml"),
        r#"schema_version: maestro.card.v1
id: legacy-feature
type: feature
title: Legacy Feature
status: shipped
created_at: "1"
updated_at: "1"
"#,
    )
    .expect("invariant: legacy archived card should be writable");
    fs::write(legacy.join("notes.md"), "legacy note\n")
        .expect("invariant: legacy archived sidecar should be writable");
    let legacy_task = repo.join(".maestro/archive/cards/tasks/task-legacy-0001");
    fs::create_dir_all(&legacy_task).expect("invariant: legacy task archive dir should exist");
    fs::write(
        legacy_task.join("task.yaml"),
        r#"schema_version: maestro.card.v1
id: task-legacy-0001
type: task
title: Legacy Task
status: verified
created_at: "1"
updated_at: "1"
"#,
    )
    .expect("invariant: legacy archived task should be writable");

    let dry_run = run(repo, &["archive", "migrate-db", "--dry-run"]);
    assert!(
        dry_run.contains("folder-backed archived cards: 2")
            && dry_run.contains("would import snapshots: 2"),
        "dry-run reports the importable legacy folders:\n{dry_run}"
    );
    assert!(legacy.exists(), "dry-run leaves the legacy folder in place");
    assert!(
        legacy_task.exists(),
        "dry-run leaves the nested legacy task folder in place"
    );
    assert!(
        !repo.join(".maestro/archive/cards.sqlite").exists(),
        "dry-run does not create the archive DB"
    );

    let applied = run(repo, &["archive", "migrate-db", "--apply"]);
    assert!(
        applied.contains("imported snapshots: 2") && applied.contains("quarantined folders: 2"),
        "apply imports and quarantines the legacy folders:\n{applied}"
    );
    assert!(
        repo.join(".maestro/archive/cards.sqlite").is_file(),
        "apply writes the DB-backed archive"
    );
    assert!(
        !legacy.exists(),
        "apply removes the visible per-card archive dir"
    );
    assert!(
        !legacy_task.exists(),
        "apply removes the nested visible task archive dir"
    );

    let quarantine_root = repo.join(".maestro/archive");
    let quarantines: Vec<_> = fs::read_dir(&quarantine_root)
        .expect("invariant: archive dir should be readable")
        .map(|entry| {
            entry
                .expect("invariant: archive entry should be readable")
                .path()
        })
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("legacy-cards-"))
        })
        .collect();
    assert_eq!(quarantines.len(), 1, "one quarantine is created");
    assert!(
        quarantines[0].join("legacy-feature/card.yaml").is_file(),
        "quarantine keeps the old feature folder until explicit cleanup"
    );
    assert!(
        quarantines[0]
            .join("tasks/task-legacy-0001/task.yaml")
            .is_file(),
        "quarantine preserves the old nested task pool path"
    );

    let shown = run(repo, &["show", "legacy-feature"]);
    assert!(
        shown.contains("Legacy Feature") && shown.contains("archived: read-only"),
        "show falls through to the DB-backed archive:\n{shown}"
    );
    let shown_task = run(repo, &["show", "task-legacy-0001"]);
    assert!(
        shown_task.contains("Legacy Task") && shown_task.contains("archived: read-only"),
        "show falls through to the nested task imported into the DB-backed archive:\n{shown_task}"
    );

    let doctor = run(repo, &["archive", "doctor"]);
    assert!(
        doctor.contains("archive: ok")
            && doctor.contains("snapshots: 2")
            && doctor.contains("archived cards: 2")
            && doctor.contains("legacy quarantines: 1"),
        "doctor verifies the imported DB snapshot and quarantine count:\n{doctor}"
    );

    let cleanup_dry_run = run(repo, &["archive", "cleanup", "--dry-run"]);
    assert!(
        cleanup_dry_run.contains("legacy quarantines: 1") && cleanup_dry_run.contains("doctor: ok"),
        "cleanup dry-run reports the quarantine without deleting it:\n{cleanup_dry_run}"
    );
    assert!(
        quarantines[0].exists(),
        "cleanup dry-run keeps the quarantine folder"
    );

    let cleanup = run(repo, &["archive", "cleanup", "--apply"]);
    assert!(
        cleanup.contains("removed legacy quarantines: 1"),
        "cleanup apply removes the quarantine:\n{cleanup}"
    );
    assert!(
        !quarantines[0].exists(),
        "cleanup apply deletes the quarantine folder"
    );

    let doctor_after_cleanup = run(repo, &["archive", "doctor"]);
    assert!(
        doctor_after_cleanup.contains("legacy quarantines: 0"),
        "doctor reflects cleanup:\n{doctor_after_cleanup}"
    );
}

#[test]
fn archive_migrate_db_refuses_tracked_archive_when_db_ignored() {
    let temp = cards_repo("s2-archive-db-tracked-source");
    let repo = temp.path();
    git(repo, &["init", "-q"]);
    fs::write(repo.join(".gitignore"), ".maestro/\n")
        .expect("invariant: root gitignore should be writable");

    let legacy = repo.join(".maestro/archive/cards/legacy-feature");
    fs::create_dir_all(&legacy).expect("invariant: legacy archive dir should be creatable");
    fs::write(
        legacy.join("card.yaml"),
        r#"schema_version: maestro.card.v1
id: legacy-feature
type: feature
title: Legacy Feature
status: shipped
created_at: "1"
updated_at: "1"
"#,
    )
    .expect("invariant: legacy archived card should be writable");
    git(repo, &["add", ".gitignore"]);
    git(
        repo,
        &[
            "add",
            "-f",
            ".maestro/archive/cards/legacy-feature/card.yaml",
        ],
    );
    git(
        repo,
        &[
            "-c",
            "user.name=Maestro Test",
            "-c",
            "user.email=maestro@example.invalid",
            "commit",
            "-q",
            "-m",
            "track legacy archive",
        ],
    );

    let err = run_err(repo, &["archive", "migrate-db", "--apply"]);

    assert!(
        err.contains("archive DB durability guard refused archive migrate-db")
            && err.contains(".maestro/archive/cards.sqlite")
            && err.contains(".maestro/archive/cards/legacy-feature/card.yaml"),
        "migrate-db should name the unsafe tracked-to-untracked replacement:\n{err}"
    );
    assert!(
        legacy.exists(),
        "the legacy archive folder must remain when migration is refused"
    );
    assert!(
        !repo.join(".maestro/archive/cards.sqlite").exists(),
        "the guard must fail before creating an ignored DB"
    );
}

#[test]
fn archive_cleanup_refuses_when_quarantine_only_durable_copy() {
    let temp = cards_repo("s2-archive-db-cleanup-durable-copy");
    let repo = temp.path();
    git(repo, &["init", "-q"]);
    fs::write(repo.join(".gitignore"), ".maestro/\n")
        .expect("invariant: root gitignore should be writable");

    let legacy = repo.join(".maestro/archive/cards/legacy-feature");
    fs::create_dir_all(&legacy).expect("invariant: legacy archive dir should be creatable");
    fs::write(
        legacy.join("card.yaml"),
        r#"schema_version: maestro.card.v1
id: legacy-feature
type: feature
title: Legacy Feature
status: shipped
created_at: "1"
updated_at: "1"
"#,
    )
    .expect("invariant: legacy archived card should be writable");
    git(repo, &["add", ".gitignore"]);
    git(
        repo,
        &[
            "add",
            "-f",
            ".maestro/archive/cards/legacy-feature/card.yaml",
        ],
    );
    git(
        repo,
        &[
            "-c",
            "user.name=Maestro Test",
            "-c",
            "user.email=maestro@example.invalid",
            "commit",
            "-q",
            "-m",
            "track legacy archive",
        ],
    );

    let quarantine = repo.join(".maestro/archive/legacy-cards-2026-06-30");
    fs::create_dir_all(&quarantine).expect("invariant: quarantine dir should be creatable");
    fs::rename(&legacy, quarantine.join("legacy-feature"))
        .expect("invariant: legacy archive should move to quarantine");

    let err = run_err(repo, &["archive", "cleanup", "--apply"]);

    assert!(
        err.contains("archive DB durability guard refused archive cleanup")
            && err.contains(".maestro/archive/cards.sqlite")
            && err.contains(".maestro/archive/cards/legacy-feature/card.yaml"),
        "cleanup should name the tracked source whose only local copy is quarantined:\n{err}"
    );
    assert!(
        quarantine.join("legacy-feature/card.yaml").exists(),
        "cleanup must leave the quarantine intact when the DB is not durable"
    );
}

#[test]
fn archive_db_sidecars_are_searchable_after_migration() {
    let temp = cards_repo("s2-archive-db-sidecar-memory");
    let repo = temp.path();
    seed_legacy_sidecar_archive(repo);

    run(repo, &["archive", "migrate-db", "--apply"]);
    assert!(
        !repo
            .join(".maestro/archive/cards/legacy-sidecar-feature")
            .exists(),
        "migration should remove the filesystem sidecar source"
    );

    for term in [
        "notestokenx",
        "spectokenx",
        "qatokenx",
        "decisiontokenx",
        "cardtokenx",
    ] {
        let out = run(repo, &["grep", "--json", &format!("{term} corpus:memory")]);
        let json: Value = serde_json::from_str(&out).expect("grep output should be JSON");
        assert_eq!(json["ok"], true, "{json:#}");
        assert!(
            json["hits"]
                .as_array()
                .expect("hits should be an array")
                .iter()
                .any(|hit| hit["id"] == "legacy-sidecar-feature" && hit["archived"] == true),
            "memory grep should find archived DB sidecar term {term}:\n{json:#}"
        );
    }
}

#[test]
fn card_list_grep_finds_db_archived_sidecar_text() {
    let temp = cards_repo("s2-archive-db-sidecar-list");
    let repo = temp.path();
    seed_legacy_sidecar_archive(repo);

    run(repo, &["archive", "migrate-db", "--apply"]);
    let list = run(repo, &["list", "--grep", "spectokenx", "--archived"]);

    assert!(
        list.contains("legacy-sidecar-feature") && list.contains("Legacy Sidecar Feature"),
        "card list grep should recall archived DB sidecar text:\n{list}"
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
            "in_progress",
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
        shown.contains("in_progress"),
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
fn generic_update_refuses_task_verification_lifecycle_states() {
    let temp = cards_repo("s2-update-gated-status");
    let repo = temp.path();

    run(repo, &["create", "-t", "task", "Wire verification"]);
    let id = id_by_title(repo, "Wire verification");

    let needs_verification = run_err(repo, &["update", &id, "--status", "needs_verification"]);
    assert!(
        needs_verification.contains("cannot set")
            && needs_verification.contains("needs_verification")
            && needs_verification.contains("maestro task complete"),
        "needs_verification refusal should point at task complete:\n{needs_verification}"
    );

    let verified = run_err(repo, &["update", &id, "--status", "verified"]);
    assert!(
        verified.contains("cannot set")
            && verified.contains("verified")
            && verified.contains("maestro task verify"),
        "verified refusal should point at task verify:\n{verified}"
    );

    let shown = run(repo, &["show", &id]);
    assert!(
        shown.contains("open"),
        "refused lifecycle writes must leave status unchanged:\n{shown}"
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

#[test]
fn linked_inbox_is_advisory_and_explicit_task_blocker_is_the_execution_gate() {
    let temp = cards_repo("s2-msg-advisory-task-gate");
    let repo = temp.path();

    maestro_in_session(repo, "setup", &["create", "-t", "chore", "Api card"]);
    let api = id_by_title(repo, "Api card");
    maestro_in_session(repo, "setup", &["create", "-t", "chore", "Ui card"]);
    let ui = id_by_title(repo, "Ui card");
    maestro_in_session(repo, "setup", &["link", "add", &api, &ui]);

    maestro_in_session(repo, "api", &["note", &api, "bind"]);
    maestro_in_session(
        repo,
        "api",
        &["msg", "send", &ui, "API endpoint is done; UI can wire data"],
    );

    maestro_in_session(repo, "ui", &["note", &ui, "bind"]);
    let read = maestro_in_session(repo, "ui", &["msg", "read"]);
    assert!(
        read.status.success(),
        "msg read should succeed\nstderr:\n{}",
        String::from_utf8_lossy(&read.stderr)
    );
    let read_out = String::from_utf8_lossy(&read.stdout);
    assert!(
        read_out.contains("inbox is advisory")
            && read_out.contains("maestro task block <id> --by <task-id>"),
        "msg read should explain that messages are advisory and task blockers enforce order:\n{read_out}"
    );
    let list = maestro_in_session(repo, "ui", &["msg", "list", &api]);
    assert!(
        list.status.success(),
        "msg list should succeed\nstderr:\n{}",
        String::from_utf8_lossy(&list.stderr)
    );
    let list_out = String::from_utf8_lossy(&list.stdout);
    assert!(
        list_out.contains("inbox is advisory")
            && list_out.contains("maestro task block <id> --by <task-id>"),
        "msg list should carry the same advisory boundary:\n{list_out}"
    );

    maestro_in_session(repo, "api", &["msg", "send", &ui, "second advisory"]);
    let add = maestro_in_session(repo, "ui", &["task", "add", "Wire UI", "--id-only"]);
    assert!(
        add.status.success(),
        "unread inbox must not block task add\nstderr:\n{}",
        String::from_utf8_lossy(&add.stderr)
    );
    assert!(
        String::from_utf8_lossy(&add.stderr).contains("[inbox]"),
        "task add should still surface the unread inbox banner without blocking:\n{}",
        String::from_utf8_lossy(&add.stderr)
    );
    let wire_ui = String::from_utf8(add.stdout)
        .expect("task id should be UTF-8")
        .trim()
        .to_string();

    let next = maestro_in_session(repo, "ui", &["task", "next"]);
    assert!(
        next.status.success(),
        "unread inbox must not block task next\nstderr:\n{}",
        String::from_utf8_lossy(&next.stderr)
    );
    assert!(
        String::from_utf8_lossy(&next.stdout).contains(&wire_ui),
        "task next should still point at the ready task while inbox is unread:\n{}",
        String::from_utf8_lossy(&next.stdout)
    );

    let start = maestro_in_session(repo, "ui", &["task", "start", &wire_ui]);
    assert!(
        start.status.success(),
        "unread inbox must not block task start\nstderr:\n{}",
        String::from_utf8_lossy(&start.stderr)
    );
    let done = maestro_in_session(
        repo,
        "ui",
        &[
            "task",
            "done",
            &wire_ui,
            "--summary",
            "wired UI",
            "--proof",
            "wired UI",
        ],
    );
    assert!(
        done.status.success(),
        "unread inbox must not block low-ceremony task verification\nstderr:\n{}",
        String::from_utf8_lossy(&done.stderr)
    );

    let endpoint = String::from_utf8(
        maestro_in_session(
            repo,
            "ui",
            &["task", "add", "Implement endpoint", "--id-only"],
        )
        .stdout,
    )
    .expect("task id should be UTF-8")
    .trim()
    .to_string();
    let blocked_ui = String::from_utf8(
        maestro_in_session(repo, "ui", &["task", "add", "Blocked UI", "--id-only"]).stdout,
    )
    .expect("task id should be UTF-8")
    .trim()
    .to_string();
    let block = maestro_in_session(
        repo,
        "ui",
        &[
            "task",
            "block",
            &blocked_ui,
            "--reason",
            "waiting on endpoint",
            "--by",
            &endpoint,
        ],
    );
    assert!(
        block.status.success(),
        "explicit task blocker should be recordable\nstderr:\n{}",
        String::from_utf8_lossy(&block.stderr)
    );

    let blocked_start = maestro_in_session(repo, "ui", &["task", "start", &blocked_ui]);
    assert!(
        !blocked_start.status.success(),
        "explicit task blocker must block task start"
    );
    assert!(
        String::from_utf8_lossy(&blocked_start.stderr).contains("unresolved blockers"),
        "explicit blocker failure should name unresolved blockers:\n{}",
        String::from_utf8_lossy(&blocked_start.stderr)
    );
}

#[test]
fn msg_send_to_task_rejects_with_parent_link_and_blocker_guidance() {
    let temp = cards_repo("s2-msg-task-endpoint-reject");
    let repo = temp.path();

    maestro_in_session(repo, "setup", &["create", "-t", "chore", "Sender card"]);
    let sender = id_by_title(repo, "Sender card");
    maestro_in_session(repo, "setup", &["create", "-t", "chore", "Workflow card"]);
    let workflow = id_by_title(repo, "Workflow card");
    maestro_in_session(
        repo,
        "setup",
        &["create", "-t", "task", "Low task", "--parent", &workflow],
    );
    let task = id_by_title(repo, "Low task");
    maestro_in_session(repo, "setup", &["link", "add", &sender, &task]);

    maestro_in_session(repo, "sender", &["note", &sender, "bind"]);
    let rejected = maestro_in_session(repo, "sender", &["msg", "send", &task, "wrong layer"]);
    assert!(
        !rejected.status.success(),
        "direct Task-addressed message should fail"
    );
    let err = String::from_utf8_lossy(&rejected.stderr);
    assert!(
        err.contains(&format!("Task {task} is not a message inbox endpoint")),
        "error names the rejected Task endpoint:\n{err}"
    );
    assert!(
        err.contains(&format!("Owning parent Card: {workflow}")),
        "error names the owning parent Card:\n{err}"
    );
    assert!(
        err.contains(&format!("maestro link add {sender} {workflow}")),
        "unlinked parent guidance starts with the Card link:\n{err}"
    );
    assert!(
        err.contains(&format!("maestro msg send {workflow} <text>")),
        "error directs advisory coordination to the parent Card:\n{err}"
    );
    assert!(
        err.contains(&format!(
            "maestro task block {task} --by <dependency-task-id>"
        )),
        "error directs ordering to explicit Task blockers:\n{err}"
    );
}

#[test]
fn parent_card_surfaces_legacy_task_addressed_channels_read_only() {
    let temp = cards_repo("s2-msg-legacy-task-channel");
    let repo = temp.path();

    maestro_in_session(repo, "setup", &["create", "-t", "chore", "Sender card"]);
    let sender = id_by_title(repo, "Sender card");
    maestro_in_session(repo, "setup", &["create", "-t", "chore", "Workflow card"]);
    let workflow = id_by_title(repo, "Workflow card");
    maestro_in_session(
        repo,
        "setup",
        &["create", "-t", "task", "Low task", "--parent", &workflow],
    );
    let task = id_by_title(repo, "Low task");

    let paths = MaestroPaths::new(repo);
    channel::send(
        &paths,
        &sender,
        &task,
        "legacy-session",
        "old task-addressed note",
    )
    .expect("legacy fixture channel should be writable");

    maestro_in_session(repo, "workflow", &["note", &workflow, "bind"]);
    let listed = maestro_in_session(repo, "workflow", &["msg", "list"]);
    assert!(
        listed.status.success(),
        "parent msg list should succeed\nstderr:\n{}",
        String::from_utf8_lossy(&listed.stderr)
    );
    let list_out = String::from_utf8_lossy(&listed.stdout);
    assert!(
        list_out.contains("legacy task-addressed channel")
            && list_out.contains(&task)
            && list_out.contains(&sender)
            && list_out.contains("read-only")
            && list_out.contains(&format!("reply via parent Card {workflow}")),
        "parent list should show legacy task-addressed history as read-only:\n{list_out}"
    );

    let read = maestro_in_session(repo, "workflow", &["msg", "read"]);
    assert!(
        read.status.success(),
        "parent msg read should succeed\nstderr:\n{}",
        String::from_utf8_lossy(&read.stderr)
    );
    let read_out = String::from_utf8_lossy(&read.stdout);
    assert!(
        read_out.contains("legacy task-addressed channel")
            && read_out.contains("old task-addressed note")
            && read_out.contains("inbox is advisory"),
        "parent read should surface the legacy message and keep advisory ordering guidance:\n{read_out}"
    );

    let scoped = maestro_in_session(repo, "workflow", &["msg", "list", &sender]);
    assert!(
        scoped.status.success(),
        "scoped parent msg list should succeed\nstderr:\n{}",
        String::from_utf8_lossy(&scoped.stderr)
    );
    let scoped_out = String::from_utf8_lossy(&scoped.stdout);
    assert!(
        scoped_out.contains("legacy task-addressed channel")
            && scoped_out.contains("old task-addressed note"),
        "scoped list should recover the same legacy channel by partner:\n{scoped_out}"
    );
}

#[test]
fn msg_send_from_asserts_current_card_without_impersonating() {
    let temp = cards_repo("s2-msg-from-assertion");
    let repo = temp.path();

    maestro_in_session(repo, "setup", &["create", "-t", "chore", "Alpha"]);
    let alpha = id_by_title(repo, "Alpha");
    maestro_in_session(repo, "setup", &["create", "-t", "chore", "Bravo"]);
    let bravo = id_by_title(repo, "Bravo");
    maestro_in_session(repo, "setup", &["link", "add", &alpha, &bravo]);

    maestro_in_session(repo, "asess", &["note", &alpha, "bind"]);
    let sent = maestro_in_session(
        repo,
        "asess",
        &["msg", "send", "--from", &alpha, &bravo, "your move"],
    );
    assert!(
        sent.status.success(),
        "matching --from should send\nstderr:\n{}",
        String::from_utf8_lossy(&sent.stderr)
    );

    let wrong = maestro_in_session(
        repo,
        "asess",
        &["msg", "send", "--from", &bravo, &alpha, "impersonate"],
    );
    assert!(!wrong.status.success(), "mismatched --from should fail");
    let err = String::from_utf8_lossy(&wrong.stderr);
    assert!(err.contains("--from does not match current card"), "{err}");
    assert!(err.contains(&alpha) && err.contains(&bravo), "{err}");
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
    let refused = run_err(repo, &["update", "csv-export", "--status", "closed"]);
    assert!(
        refused.contains("maestro feature close"),
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
        non_feature.contains("not a card container"),
        "a non-container parent is refused:\n{non_feature}"
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
        bare.contains("usage: maestro card update"),
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
    assert!(
        receipt.contains("next: maestro card list --grep"),
        "{receipt}"
    );
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
        .env("MAESTRO_AUTO_UPDATE", "0")
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
            .all(|event| event["agent_runtime"] == "codex"),
        "every auto card_touch carries the resolved runtime identity: {touches:#?}"
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

/// `assign` sets an advisory routing hint; it is not a claim and must not change
/// what the assigner is working on. Regression for the post-ship finding that
/// `assign` emitted a `card_touch`, silently re-binding the assigner's `active`
/// and `msg` current card to the card it merely routed.
#[test]
fn assign_is_advisory_and_does_not_bind_the_assigners_session() {
    let temp = cards_repo("assign-no-bind");
    let repo = temp.path();

    // Create both cards under a SETUP session so their create-touches land in a
    // different bucket and never pollute the assigner's binding.
    maestro_in_session(repo, "setup", &["create", "-t", "chore", "My work"]);
    let mine = id_by_title(repo, "My work");
    maestro_in_session(repo, "setup", &["create", "-t", "chore", "Someone else's"]);
    let other = id_by_title(repo, "Someone else's");

    let session = "assigner";
    // Claiming binds the assigner's session to `mine` -- their real work.
    let claimed = maestro_in_session(repo, session, &["claim", &mine]);
    assert!(
        claimed.status.success(),
        "claim exits 0\nstderr:\n{}",
        String::from_utf8_lossy(&claimed.stderr)
    );
    // Routing an advisory hint to `other` must leave the current card untouched.
    let assigned = maestro_in_session(repo, session, &["assign", &other, "dana"]);
    assert!(
        assigned.status.success(),
        "assign exits 0\nstderr:\n{}",
        String::from_utf8_lossy(&assigned.stderr)
    );

    let touches = card_touch_events(repo, session);
    assert_eq!(
        touches.len(),
        1,
        "only the claim binds the assigner; assign emits no card_touch: {touches:#?}"
    );
    assert_eq!(
        touches.last().expect("the claim touch")["card_id"],
        Value::String(mine.clone()),
        "the assigner's current card stays the claimed card, not the routed one: {touches:#?}"
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

// ---------------------------------------------------------------------------
// cards-as-lightweight-progress-tracking-without-the-full-pipeline
// ---------------------------------------------------------------------------

/// Run a card verb under an explicit `<agent>#<session>` identity, returning
/// `(success, stdout, stderr)`. The per-session focus nudge keys on the claim
/// identity, so a nudge test must vary the session the same way two real agents
/// would (the default helper pins agent=codex session=s1).
fn run_as(cwd: &Path, agent: &str, session: &str, args: &[&str]) -> (bool, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .current_dir(cwd)
        .env("MAESTRO_AGENT", agent)
        .env("MAESTRO_SESSION", session)
        .env("MAESTRO_AUTO_UPDATE", "0")
        .output()
        .expect("invariant: compiled maestro binary should run in integration tests");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn run_lease_as_json(cwd: &Path, agent: &str, session: &str, args: &[&str]) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .current_dir(cwd)
        .env("MAESTRO_AGENT", agent)
        .env("MAESTRO_SESSION", session)
        .env("MAESTRO_RUN_ID", session)
        .env("MAESTRO_AUTO_UPDATE", "0")
        .output()
        .expect("invariant: compiled maestro binary should run in integration tests");
    assert!(
        output.status.success(),
        "maestro {args:?} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "work lease stdout should parse as JSON ({error})\nstdout:\n{}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

fn set_card_claim(repo: &Path, id: &str, claimed_by: &str, claimed_at: &str) {
    let path = card_record_path(repo, id);
    let raw = fs::read_to_string(&path).expect("card record should be readable");
    let mut doc: serde_yaml::Value =
        serde_yaml::from_str(&raw).expect("card record should parse as yaml");
    let map = doc
        .as_mapping_mut()
        .expect("dir-backed card should be a yaml mapping");
    map.insert(
        serde_yaml::Value::String("claimed_by".to_string()),
        serde_yaml::Value::String(claimed_by.to_string()),
    );
    map.insert(
        serde_yaml::Value::String("claimed_at".to_string()),
        serde_yaml::Value::String(claimed_at.to_string()),
    );
    fs::write(
        &path,
        serde_yaml::to_string(&doc).expect("updated card record should serialize"),
    )
    .expect("updated card record should be writable");
}

#[test]
fn loop_work_lease_claims_one_ready_card_and_returns_worker_contract() {
    let temp = cards_repo("loop-work-lease-happy");
    let repo = temp.path();

    let first = run(repo, &["create", "-t", "task", "Lease first", "--id-only"]);
    let first = first.trim().to_string();
    let second = run(repo, &["create", "-t", "task", "Lease second", "--id-only"]);
    let second = second.trim().to_string();

    let lease = run_lease_as_json(
        repo,
        "codex",
        "lease-happy",
        &["loop", "work-lease", "--json"],
    );

    assert_eq!(lease["schema"], "maestro.work_lease.v1");
    assert_eq!(lease["version"], 1);
    assert_eq!(lease["helper"]["role"], "internal_choose_phase_helper");
    assert_eq!(lease["helper"]["phase"], "choose");
    assert_eq!(lease["helper"]["parent_recipe"], "unattended");
    assert_eq!(lease["helper"]["selection_limit"], "exactly_one_ready_unit");
    assert!(
        lease["helper"]["hard_boundary"]
            .as_str()
            .unwrap()
            .contains("not a top-level lifecycle"),
        "lease should demote Work Lease to an internal helper: {lease:#}"
    );
    assert_eq!(lease["status"], "leased");
    let leased_id = lease["selected_card"]["id"]
        .as_str()
        .expect("leased card id should be present");
    assert!(
        leased_id == first || leased_id == second,
        "lease should pick one ready card, got {leased_id}"
    );
    assert_eq!(card_doc(repo, leased_id)["status"], "in_progress");
    assert_eq!(card_doc(repo, leased_id)["claimed_by"], "codex#lease-happy");
    assert_eq!(lease["claim"]["claimed_by"], "codex#lease-happy");
    assert_eq!(lease["lease"]["stale_after_seconds"], 900);
    assert_eq!(lease["selected_action"]["kind"], "work_card");
    assert_eq!(
        lease["handles"]["inspect"]["selected_card"],
        format!("maestro card show {leased_id} --json")
    );
    assert_eq!(
        lease["handles"]["status"]["claim"],
        format!("maestro card show {leased_id} --json")
    );
    assert_eq!(
        lease["handles"]["reconcile"]["run_events_jsonl"],
        ".maestro/runs/lease-happy/events.jsonl"
    );
    assert_eq!(
        lease["handles"]["reconcile"]["run_report"],
        "maestro query run --json"
    );
    assert!(
        lease["handles"]["restart_policy"]
            .as_str()
            .unwrap()
            .contains("no daemon, queue, scheduler, executor, or hidden store"),
        "lease should describe restart-stable handles without side state: {lease:#}"
    );
    assert_eq!(lease["ship_authority"]["status"], "absent");
    assert_eq!(lease["ship_authority"]["external_ship_allowed"], false);
    assert!(lease["recurrence_guard"]["required"].as_bool().unwrap());
    assert!(
        lease["allowed_follow_up_verbs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|verb| verb == "maestro query run --json"),
        "lease should expose reconcile/read handles: {lease:#}"
    );
    assert!(
        lease["worker_prompt"]
            .as_str()
            .unwrap()
            .contains("Do not push"),
        "absent ship authority should fail closed in worker prompt"
    );

    let run_log = fs::read_to_string(repo.join(".maestro/runs/lease-happy/events.jsonl"))
        .expect("work lease should leave durable run evidence");
    assert!(
        run_log.contains("\"event_type\":\"ownership_acquire\"")
            && run_log.contains("\"action\":\"work_lease_acquire\""),
        "lease acquisition should be inspectable in run events:\n{run_log}"
    );

    let restarted_card = run(repo, &["card", "show", leased_id, "--json"]);
    let restarted_card: Value =
        serde_json::from_str(&restarted_card).expect("restart card status json should parse");
    assert_eq!(restarted_card["status"], "in_progress");
    assert_eq!(restarted_card["claimed_by"], "codex#lease-happy");

    let restarted_report = run(repo, &["query", "run", "--json"]);
    let restarted_report: Value =
        serde_json::from_str(&restarted_report).expect("restart run report json should parse");
    assert_eq!(
        restarted_report["autonomy"]["ledger_paths"][0],
        ".maestro/runs/lease-happy/events.jsonl"
    );
    assert!(
        restarted_report["autonomy"]["actions"]
            .as_array()
            .expect("restart run report should include actions")
            .iter()
            .any(|action| {
                action["action"] == "work_lease_acquire" && action["target_id"] == leased_id
            }),
        "restart run report should reconcile the lease action: {restarted_report:#}"
    );
}

#[test]
fn loop_work_lease_no_ready_work_returns_dry_json_without_card_mutation() {
    let temp = cards_repo("loop-work-lease-dry");
    let repo = temp.path();
    let before = fs::read_dir(repo.join(".maestro/cards"))
        .expect("cards dir should be readable")
        .count();

    let lease = run_lease_as_json(
        repo,
        "codex",
        "lease-dry",
        &["loop", "work-lease", "--json"],
    );

    assert_eq!(lease["status"], "dry");
    assert_eq!(lease["helper"]["role"], "internal_choose_phase_helper");
    assert_eq!(lease["helper"]["phase"], "choose");
    assert_eq!(lease["reason"], "no ready cards matched this lease scope");
    assert!(lease.get("selected_card").is_none());
    let after = fs::read_dir(repo.join(".maestro/cards"))
        .expect("cards dir should be readable")
        .count();
    assert_eq!(before, after, "dry lease should not mutate cards");
}

#[test]
fn loop_work_lease_does_not_steal_live_claims() {
    let temp = cards_repo("loop-work-lease-live-claim");
    let repo = temp.path();
    let held = run(repo, &["create", "-t", "task", "Held live", "--id-only"]);
    let held = held.trim().to_string();
    set_card_claim(repo, &held, "claude#live", "2999-01-01T00:00:00.000Z");

    let lease = run_lease_as_json(
        repo,
        "codex",
        "lease-contend",
        &["loop", "work-lease", "--json"],
    );

    assert_eq!(lease["status"], "blocked");
    assert_eq!(lease["blocked_cards"][0]["id"], held);
    assert_eq!(card_doc(repo, &held)["claimed_by"], "claude#live");
    assert_eq!(card_doc(repo, &held)["status"], "open");
}

#[test]
fn loop_work_lease_reclaims_stale_claims_with_existing_claim_policy() {
    let temp = cards_repo("loop-work-lease-stale-claim");
    let repo = temp.path();
    let held = run(repo, &["create", "-t", "task", "Held stale", "--id-only"]);
    let held = held.trim().to_string();
    set_card_claim(repo, &held, "claude#old", "2020-01-01T00:00:00.000Z");

    let lease = run_lease_as_json(
        repo,
        "codex",
        "lease-stale",
        &["loop", "work-lease", "--json"],
    );

    assert_eq!(lease["status"], "leased");
    assert_eq!(lease["selected_card"]["id"], held);
    assert_eq!(lease["claim"]["outcome"], "reclaimed_stale");
    assert_eq!(card_doc(repo, &held)["claimed_by"], "codex#lease-stale");
    assert_eq!(card_doc(repo, &held)["status"], "in_progress");
}

#[test]
fn loop_work_lease_partial_ship_authority_fails_closed() {
    let temp = cards_repo("loop-work-lease-authority");
    let repo = temp.path();
    let card = run(
        repo,
        &["create", "-t", "task", "Partial authority", "--id-only"],
    );
    let card = card.trim().to_string();

    let lease = run_lease_as_json(
        repo,
        "codex",
        "lease-authority",
        &[
            "loop",
            "work-lease",
            "--json",
            "--authority-ref",
            "prompt:night",
            "--allow-external-action",
            "push",
        ],
    );

    assert_eq!(lease["status"], "leased");
    assert_eq!(lease["selected_card"]["id"], card);
    assert_eq!(lease["ship_authority"]["status"], "ambiguous");
    assert_eq!(lease["ship_authority"]["external_ship_allowed"], false);
    assert_eq!(
        lease["ship_authority"]["reason"],
        "partial ship authority is not enough; provide ref, summary, scope, target, allowed external actions, and required evidence"
    );
    assert!(
        lease["worker_prompt"]
            .as_str()
            .unwrap()
            .contains("Do not push"),
        "partial authority must fail closed in the leased worker contract"
    );
}

#[test]
fn loop_work_lease_overbroad_ship_authority_fails_closed() {
    let temp = cards_repo("loop-work-lease-overbroad-authority");
    let repo = temp.path();
    let card = run(
        repo,
        &["create", "-t", "task", "Overbroad authority", "--id-only"],
    );
    let card = card.trim().to_string();

    let lease = run_lease_as_json(
        repo,
        "codex",
        "lease-overbroad-authority",
        &[
            "loop",
            "work-lease",
            "--json",
            "--authority-ref",
            "prompt:night",
            "--authority-summary",
            "ship everything",
            "--authority-scope",
            "repo",
            "--authority-target",
            "main",
            "--allow-external-action",
            "everything",
            "--required-evidence",
            "green tests",
        ],
    );

    assert_eq!(lease["status"], "leased");
    assert_eq!(lease["selected_card"]["id"], card);
    assert_eq!(lease["ship_authority"]["status"], "overbroad");
    assert_eq!(lease["ship_authority"]["external_ship_allowed"], false);
    assert_eq!(
        lease["ship_authority"]["reason"],
        "ship authority must name concrete external actions, not all/everything/*"
    );
    assert!(
        lease["worker_prompt"]
            .as_str()
            .unwrap()
            .contains("Do not push"),
        "overbroad authority must fail closed in the leased worker contract"
    );
}

#[test]
fn create_batch_mints_one_open_card_per_title() {
    let temp = cards_repo("s2-create-batch");
    let repo = temp.path();

    // ac-1: three titles in one invocation mint three open cards.
    let created = run(repo, &["create", "-t", "task", "alpha", "beta", "gamma"]);
    assert_eq!(
        created
            .lines()
            .filter(|l| l.starts_with("created "))
            .count(),
        3,
        "three titles mint three cards:\n{created}"
    );
    let open = run(repo, &["list", "--type", "task", "--status", "open"]);
    for title in ["alpha", "beta", "gamma"] {
        assert!(open.contains(title), "{title} is open:\n{open}");
    }

    // ac-1: --id-only prints exactly the new ids, one per line, nothing else.
    let ids = run(
        repo,
        &["create", "-t", "task", "delta", "epsilon", "--id-only"],
    );
    let id_lines: Vec<&str> = ids.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(id_lines.len(), 2, "two titles print two bare ids:\n{ids}");
    for line in &id_lines {
        assert!(
            line.starts_with("task-") && !line.contains(' '),
            "an --id-only line is a bare id:\n{line}"
        );
    }
}

#[test]
fn create_single_title_is_backward_compatible() {
    let temp = cards_repo("s2-create-single");
    let repo = temp.path();

    // ac-2: a single positional title still mints exactly one card with the
    // unchanged `created <id> (<type>): <title>` shape.
    let created = run(repo, &["create", "-t", "task", "only one"]);
    assert_eq!(
        created
            .lines()
            .filter(|l| l.starts_with("created "))
            .count(),
        1,
        "one title mints one card:\n{created}"
    );
    assert!(
        created.contains("(task): only one"),
        "single create keeps its confirmation shape:\n{created}"
    );
}

#[test]
fn create_batch_refuses_per_card_text_and_mints_nothing() {
    let temp = cards_repo("s2-create-batch-guard");
    let repo = temp.path();

    // ac-3: --description is refused in batch mode, pointing at `card update`,
    // and no card is created by the rejected call (the guard runs before mint).
    let err = run_err(
        repo,
        &["create", "-t", "task", "a", "b", "--description", "d"],
    );
    assert!(
        err.contains("--description") && err.contains("card update"),
        "the batch --description refusal points at card update:\n{err}"
    );
    // --active-form is the same per-card text hazard; refused identically.
    let af_err = run_err(
        repo,
        &["create", "-t", "task", "a", "b", "--active-form", "doing"],
    );
    assert!(
        af_err.contains("--active-form") && af_err.contains("card update"),
        "the batch --active-form refusal points at card update:\n{af_err}"
    );
    let listed = run(repo, &["list", "--type", "task", "--all"]);
    assert!(
        listed.contains("no cards match"),
        "a refused batch create mints nothing:\n{listed}"
    );

    // ac-3: --parent still applies to every card in a batch.
    run(repo, &["create", "-t", "feature", "Auth"]);
    run(
        repo,
        &["create", "-t", "task", "p1", "p2", "--parent", "auth"],
    );
    let parented = run(repo, &["list", "--parent", "auth"]);
    assert!(
        parented.contains("p1") && parented.contains("p2") && parented.matches("auth").count() >= 2,
        "a batch --parent docks both cards:\n{parented}"
    );
}

#[test]
fn task_create_still_mints_a_draft() {
    let temp = cards_repo("s2-task-create-draft");
    let repo = temp.path();

    // ac-4: the gated `task create` path is unchanged -- it mints at draft, not
    // the lightweight `open`.
    run(repo, &["task", "create", "Gated task"]);
    let id = id_by_title(repo, "Gated task");
    let doc = card_doc(repo, &id);
    assert_eq!(
        doc["status"], "draft",
        "task create still mints a draft card:\n{doc:?}"
    );
}

#[test]
fn active_form_persists_and_does_not_change_status() {
    let temp = cards_repo("s2-active-form");
    let repo = temp.path();

    // ac-8: --active-form is stored at create and is display-only (status open).
    let id = run(
        repo,
        &[
            "create",
            "-t",
            "task",
            "Export rows",
            "--active-form",
            "Wiring export",
            "--id-only",
        ],
    );
    let id = id.trim();
    let doc = card_doc(repo, id);
    assert_eq!(
        doc["active_form"], "Wiring export",
        "create stores active_form"
    );
    assert_eq!(doc["status"], "open", "active_form does not change status");

    // ac-8: update can set it; an unset card serializes no key.
    let plain = run(repo, &["create", "-t", "task", "Plain", "--id-only"]);
    let plain = plain.trim();
    assert!(
        card_doc(repo, plain).get("active_form").is_none(),
        "an unset active_form serializes no key"
    );
    run(repo, &["update", plain, "--active-form", "Doing plain"]);
    assert_eq!(
        card_doc(repo, plain)["active_form"],
        "Doing plain",
        "update sets active_form"
    );
}

#[test]
fn claim_nudges_when_session_already_holds_another_in_progress() {
    let temp = cards_repo("s2-claim-nudge");
    let repo = temp.path();

    let c1 = run(repo, &["create", "-t", "task", "one", "--id-only"]);
    let c1 = c1.trim();
    let c2 = run(repo, &["create", "-t", "task", "two", "--id-only"]);
    let c2 = c2.trim();
    let c3 = run(repo, &["create", "-t", "task", "three", "--id-only"]);
    let c3 = c3.trim();

    // ac-7: agent-A's first claim is silent.
    let (ok1, _, err1) = run_as(repo, "claude", "A", &["claim", c1]);
    assert!(ok1, "first claim succeeds");
    assert!(err1.trim().is_empty(), "first claim is silent:\n{err1}");

    // ac-7: agent-A's second claim emits an advisory naming the first, exit 0.
    let (ok2, _, err2) = run_as(repo, "claude", "A", &["claim", c2]);
    assert!(ok2, "second claim still succeeds (never blocks)");
    assert!(
        err2.contains(c1) && err2.contains("in_progress"),
        "the nudge names the already-active card:\n{err2}"
    );

    // ac-7: the first card is NOT auto-released; both stay in_progress for A.
    assert_eq!(card_doc(repo, c1)["status"], "in_progress");
    assert_eq!(card_doc(repo, c1)["claimed_by"], "claude#A");
    assert_eq!(card_doc(repo, c2)["status"], "in_progress");

    // ac-7: a different session holding only one card is silent (per-session).
    let (ok3, _, err3) = run_as(repo, "claude", "B", &["claim", c3]);
    assert!(ok3, "agent-B claim succeeds");
    assert!(
        err3.trim().is_empty(),
        "a different session holding one card is silent:\n{err3}"
    );

    // ac-7: the nudge fires identically through `update --claim`.
    let u1 = run(repo, &["create", "-t", "task", "u-one", "--id-only"]);
    let u1 = u1.trim();
    let u2 = run(repo, &["create", "-t", "task", "u-two", "--id-only"]);
    let u2 = u2.trim();
    let (_, _, ue1) = run_as(repo, "codex", "X", &["update", u1, "--claim"]);
    assert!(
        ue1.trim().is_empty(),
        "first update --claim is silent:\n{ue1}"
    );
    let (_, _, ue2) = run_as(repo, "codex", "X", &["update", u2, "--claim"]);
    assert!(
        ue2.contains(u1) && ue2.contains("in_progress"),
        "update --claim shares the nudge seam:\n{ue2}"
    );
}
