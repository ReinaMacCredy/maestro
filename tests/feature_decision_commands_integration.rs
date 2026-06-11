pub mod card_support;
mod support;

use std::fs;
use std::os::unix::fs as unix_fs;
use std::path::Path;
use std::process::Command;

use card_support::{card_dir, card_doc, id_by_title};
use maestro::domain::decisions::template::decision_markdown;
use maestro::foundation::core::fs::ensure_dir;
use serde_yaml::Value as YamlValue;
use support::TestTempDir;

fn maestro(args: &[&str], cwd: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("invariant: compiled maestro binary should be runnable in integration tests")
}

fn init_git_marker(repo: &Path) {
    fs::create_dir(repo.join(".git")).expect("invariant: .git marker should be creatable");
}

fn stdout(output: std::process::Output, args: &[&str]) -> String {
    assert!(
        output.status.success(),
        "maestro {:?} failed\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout).expect("invariant: stdout should be UTF-8")
}

fn assert_failure(output: std::process::Output, args: &[&str]) -> String {
    assert!(
        !output.status.success(),
        "maestro {:?} unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stderr).expect("invariant: stderr should be UTF-8")
}

fn assert_dated_note_line(notes: &str, expected_text: &str) {
    let line = notes
        .lines()
        .find(|line| line.ends_with(expected_text))
        .expect("invariant: expected dated note line should exist");
    assert_eq!(line.len(), 12 + expected_text.len(), "{line}");
    assert_eq!(&line[4..5], "-", "{line}");
    assert_eq!(&line[7..8], "-", "{line}");
    assert_eq!(&line[10..12], "  ", "{line}");
}

#[test]
fn feature_verify_sweeps_acceptance_contract() {
    let temp_dir = TestTempDir::new("maestro-feature-verify-contract");
    init_git_marker(temp_dir.path());
    stdout(
        maestro(&["init", "--yes"], temp_dir.path()),
        &["init", "--yes"],
    );
    stdout(
        maestro(&["harness", "set", "--claims-only"], temp_dir.path()),
        &["harness", "set", "--claims-only"],
    );
    stdout(
        maestro(&["feature", "new", "Contract Sweep"], temp_dir.path()),
        &["feature", "new", "Contract Sweep"],
    );
    let set_args = [
        "feature",
        "set",
        "contract-sweep",
        "--acceptance",
        "first behavior works",
        "--acceptance",
        "second behavior works",
        "--area",
        "src",
    ];
    stdout(maestro(&set_args, temp_dir.path()), &set_args);
    stdout(
        maestro(
            &[
                "feature",
                "accept",
                "contract-sweep",
                "--qa",
                "none",
                "--reason",
                "contract-only test",
            ],
            temp_dir.path(),
        ),
        &[
            "feature",
            "accept",
            "contract-sweep",
            "--qa",
            "none",
            "--reason",
            "contract-only test",
        ],
    );
    let create_args = [
        "task",
        "create",
        "Implement first behavior",
        "--feature",
        "contract-sweep",
        "--covers",
        "ac-1",
        "--check",
        "first behavior works",
    ];
    stdout(maestro(&create_args, temp_dir.path()), &create_args);
    let task_id = id_by_title(temp_dir.path(), "Implement first behavior");
    for args in [
        vec!["task", "explore", &task_id],
        vec!["task", "accept", &task_id],
        vec!["task", "claim", &task_id],
        vec![
            "task",
            "complete",
            &task_id,
            "--summary",
            "done",
            "--claim",
            "first behavior works",
            "--proof",
            "first behavior works",
        ],
        vec!["task", "verify", &task_id],
    ] {
        stdout(maestro(&args, temp_dir.path()), &args);
    }
    stdout(
        maestro(&["feature", "start", "contract-sweep"], temp_dir.path()),
        &["feature", "start", "contract-sweep"],
    );

    let blocked = stdout(
        maestro(
            &[
                "feature",
                "ship",
                "contract-sweep",
                "--dry-run",
                "--outcome",
                "done",
            ],
            temp_dir.path(),
        ),
        &[
            "feature",
            "ship",
            "contract-sweep",
            "--dry-run",
            "--outcome",
            "done",
        ],
    );
    assert!(blocked.contains("contract sweep missing"), "{blocked}");

    let sweep = stdout(
        maestro(&["feature", "verify", "contract-sweep"], temp_dir.path()),
        &["feature", "verify", "contract-sweep"],
    );
    assert!(sweep.contains(&format!("proof: {task_id} OK")), "{sweep}");
    assert!(sweep.contains("NO FRESH EVIDENCE"), "{sweep}");

    let prove_args = [
        "feature",
        "verify",
        "contract-sweep",
        "--prove",
        "ac-2",
        "--evidence",
        "manual proof",
    ];
    stdout(maestro(&prove_args, temp_dir.path()), &prove_args);
    let sweep = stdout(
        maestro(&["feature", "verify", "contract-sweep"], temp_dir.path()),
        &["feature", "verify", "contract-sweep"],
    );
    assert!(sweep.contains(&format!("proof: {task_id} OK")), "{sweep}");
    assert!(sweep.contains("proof: manual proof OK"), "{sweep}");
    assert!(
        sweep.contains("ok: every acceptance item has evidence"),
        "{sweep}"
    );

    let ship_preview = stdout(
        maestro(
            &[
                "feature",
                "ship",
                "contract-sweep",
                "--dry-run",
                "--outcome",
                "done",
            ],
            temp_dir.path(),
        ),
        &[
            "feature",
            "ship",
            "contract-sweep",
            "--dry-run",
            "--outcome",
            "done",
        ],
    );
    assert!(
        ship_preview.contains("would ship contract-sweep"),
        "{ship_preview}"
    );
}

#[test]
fn feature_contract_display_warnings_waivers_and_stale_sweep() {
    let temp_dir = TestTempDir::new("maestro-feature-contract-display");
    init_git_marker(temp_dir.path());
    stdout(
        maestro(&["init", "--yes"], temp_dir.path()),
        &["init", "--yes"],
    );
    stdout(
        maestro(&["harness", "set", "--claims-only"], temp_dir.path()),
        &["harness", "set", "--claims-only"],
    );
    stdout(
        maestro(&["feature", "new", "Coverage Display"], temp_dir.path()),
        &["feature", "new", "Coverage Display"],
    );
    let set_args = [
        "feature",
        "set",
        "coverage-display",
        "--acceptance",
        "first behavior works",
        "--acceptance",
        "second behavior works",
        "--acceptance",
        "third behavior works",
        "--area",
        "src",
    ];
    stdout(maestro(&set_args, temp_dir.path()), &set_args);
    let accept_args = [
        "feature",
        "accept",
        "coverage-display",
        "--qa",
        "none",
        "--reason",
        "contract display test",
    ];
    stdout(maestro(&accept_args, temp_dir.path()), &accept_args);

    let create_args = [
        "task",
        "create",
        "Implement second behavior",
        "--feature",
        "coverage-display",
        "--covers",
        "ac-2",
        "--check",
        "second behavior works",
    ];
    stdout(maestro(&create_args, temp_dir.path()), &create_args);
    let impl_id = id_by_title(temp_dir.path(), "Implement second behavior");
    let show = stdout(
        maestro(&["feature", "show", "coverage-display"], temp_dir.path()),
        &["feature", "show", "coverage-display"],
    );
    assert!(show.contains("- [ac-1] first behavior works"), "{show}");
    assert!(show.contains("- [ac-2] second behavior works"), "{show}");
    assert!(show.contains(&format!("covers: {impl_id}")), "{show}");
    assert!(show.contains("- [ac-3] third behavior works"), "{show}");

    let draft = stdout(
        maestro(
            &["feature", "prepare", "coverage-display", "--draft"],
            temp_dir.path(),
        ),
        &["feature", "prepare", "coverage-display", "--draft"],
    );
    assert!(draft.contains("warning: 2 acceptance item(s)"), "{draft}");
    assert!(draft.contains("ac-1, ac-3"), "{draft}");
    assert!(
        draft.contains("maestro task set <task-id> --covers <ac-id>"),
        "{draft}"
    );

    let start = stdout(
        maestro(&["feature", "start", "coverage-display"], temp_dir.path()),
        &["feature", "start", "coverage-display"],
    );
    assert!(start.contains("warning: 2 acceptance item(s)"), "{start}");
    assert!(start.contains("ac-1, ac-3"), "{start}");

    verify_task_claim(temp_dir.path(), &impl_id, "second behavior works");
    let waive_args = [
        "feature",
        "verify",
        "coverage-display",
        "--waive",
        "ac-1",
        "--reason",
        "not applicable in fixture",
    ];
    stdout(maestro(&waive_args, temp_dir.path()), &waive_args);
    let prove_args = [
        "feature",
        "verify",
        "coverage-display",
        "--prove",
        "ac-3",
        "--evidence",
        "manual third proof",
    ];
    stdout(maestro(&prove_args, temp_dir.path()), &prove_args);
    let sweep = stdout(
        maestro(&["feature", "verify", "coverage-display"], temp_dir.path()),
        &["feature", "verify", "coverage-display"],
    );
    assert!(
        sweep.contains("WAIVED: not applicable in fixture"),
        "{sweep}"
    );
    assert!(sweep.contains(&format!("proof: {impl_id} OK")), "{sweep}");
    assert!(sweep.contains("proof: manual third proof OK"), "{sweep}");
    assert!(
        sweep.contains("ok: every acceptance item has evidence"),
        "{sweep}"
    );
    let ship_preview = stdout(
        maestro(
            &[
                "feature",
                "ship",
                "coverage-display",
                "--dry-run",
                "--outcome",
                "done",
            ],
            temp_dir.path(),
        ),
        &[
            "feature",
            "ship",
            "coverage-display",
            "--dry-run",
            "--outcome",
            "done",
        ],
    );
    assert!(ship_preview.contains("would ship coverage-display"));

    let create_hotfix_args = [
        "task",
        "create",
        "Hotfix second behavior",
        "--feature",
        "coverage-display",
        "--covers",
        "ac-2",
        "--check",
        "second behavior works",
    ];
    stdout(
        maestro(&create_hotfix_args, temp_dir.path()),
        &create_hotfix_args,
    );
    let hotfix_id = id_by_title(temp_dir.path(), "Hotfix second behavior");
    verify_task_claim(temp_dir.path(), &hotfix_id, "second behavior works");
    // The sweep lists the two ac-2 covering task ids in scan (id-sorted) order.
    let mut covering = [impl_id.clone(), hotfix_id.clone()];
    covering.sort();
    let covering_proof = format!("proof: {}, {} OK", covering[0], covering[1]);
    let stale_preview = stdout(
        maestro(
            &[
                "feature",
                "ship",
                "coverage-display",
                "--dry-run",
                "--outcome",
                "done",
            ],
            temp_dir.path(),
        ),
        &[
            "feature",
            "ship",
            "coverage-display",
            "--dry-run",
            "--outcome",
            "done",
        ],
    );
    assert!(
        stale_preview.contains("contract sweep stale"),
        "{stale_preview}"
    );
    assert!(
        stale_preview.contains(&format!("{hotfix_id} settled at")),
        "{stale_preview}"
    );
    let refreshed = stdout(
        maestro(&["feature", "verify", "coverage-display"], temp_dir.path()),
        &["feature", "verify", "coverage-display"],
    );
    assert!(refreshed.contains(&format!("re-derived after: {hotfix_id} settled at")));
    assert!(refreshed.contains(&covering_proof), "{refreshed}");
    assert!(refreshed.contains("ok: every acceptance item has evidence"));
}

fn verify_task_claim(root: &Path, id: &str, claim: &str) {
    for args in [
        vec!["task", "explore", id],
        vec!["task", "accept", id],
        vec!["task", "claim", id],
        vec![
            "task",
            "complete",
            id,
            "--summary",
            "done",
            "--claim",
            claim,
            "--proof",
            claim,
        ],
        vec!["task", "verify", id],
    ] {
        stdout(maestro(&args, root), &args);
    }
}

#[test]
fn feature_guarded_lifecycle_via_cli() {
    let temp_dir = TestTempDir::new("maestro-feature-command-test");
    init_git_marker(temp_dir.path());
    stdout(
        maestro(&["init", "--yes"], temp_dir.path()),
        &["init", "--yes"],
    );

    let create_args = [
        "feature",
        "new",
        "Billing CSV export",
        "--description",
        "export billing rows",
        "--question",
        "Which export filename?",
    ];
    let create_output = stdout(maestro(&create_args, temp_dir.path()), &create_args);
    assert!(create_output.contains("created feature billing-csv-export"));
    // The feature lands as a flat card with its spec scaffolded beside it; the
    // per-feature decisions.yaml is retired (decisions are cards now).
    assert!(
        temp_dir
            .path()
            .join(".maestro/cards/billing-csv-export/card.yaml")
            .is_file()
    );

    let show_output = stdout(
        maestro(&["feature", "show", "billing-csv-export"], temp_dir.path()),
        &["feature", "show", "billing-csv-export"],
    );
    assert!(show_output.contains("status: proposed"));
    assert!(show_output.contains("description: export billing rows"));
    assert!(show_output.contains("decisions: 0 (open: 0, locked: 0, superseded: 0)"));
    // `feature new` no longer scaffolds notes.md; notes are created on first write.
    assert!(!show_output.contains("notes:"));

    // accept blocks on an incomplete contract, naming the gaps.
    let accept_args = ["feature", "accept", "billing-csv-export"];
    let accept_stderr = assert_failure(maestro(&accept_args, temp_dir.path()), &accept_args);
    assert!(accept_stderr.contains("acceptance"));
    assert!(accept_stderr.contains("affected_areas"));
    assert!(accept_stderr.contains("skill: maestro-card (qa-baseline)"));
    assert!(accept_stderr.contains("target: .maestro/cards/billing-csv-export/qa.md"));
    assert!(accept_stderr.contains("retry: maestro feature accept billing-csv-export"));

    // author the contract, then accept freezes it.
    let set_args = [
        "feature",
        "set",
        "billing-csv-export",
        "--acceptance",
        "exports a valid csv",
        "--area",
        "billing",
    ];
    let set_output = stdout(maestro(&set_args, temp_dir.path()), &set_args);
    assert!(set_output.contains("acceptance=1"));
    assert!(set_output.contains("areas=1"));

    let question_args = [
        "feature",
        "set",
        "billing-csv-export",
        "--question",
        "Which export filename?",
    ];
    let question_output = stdout(maestro(&question_args, temp_dir.path()), &question_args);
    assert!(question_output.contains("questions=1"));

    let help = stdout(
        maestro(&["feature", "set", "--help"], temp_dir.path()),
        &["feature", "set", "--help"],
    );
    assert!(help.contains("REPLACES the full questions list"), "{help}");
    assert!(help.contains("--add-acceptance"), "{help}");

    let redundant_clear_args = [
        "feature",
        "set",
        "billing-csv-export",
        "--clear-questions",
        "--question",
        "Which export filename?",
    ];
    let redundant_clear = stdout(
        maestro(&redundant_clear_args, temp_dir.path()),
        &redundant_clear_args,
    );
    assert!(
        redundant_clear.contains("questions replaced (1)"),
        "{redundant_clear}"
    );

    let clear_questions_args = ["feature", "set", "billing-csv-export", "--clear-questions"];
    let clear_questions_output = stdout(
        maestro(&clear_questions_args, temp_dir.path()),
        &clear_questions_args,
    );
    assert!(clear_questions_output.contains("questions=0"));

    // accept also requires a captured baseline (F); ship requires it proven.
    let cards_dir = temp_dir.path().join(".maestro/cards");
    write_baseline(&cards_dir, "billing-csv-export");
    write_qa_slice(&cards_dir, "billing-csv-export");

    let dry_args = ["feature", "accept", "billing-csv-export", "--dry-run"];
    let dry_output = stdout(maestro(&dry_args, temp_dir.path()), &dry_args);
    assert!(dry_output.contains("would accept"));

    let accept_output = stdout(maestro(&accept_args, temp_dir.path()), &accept_args);
    assert!(accept_output.contains("accepted billing-csv-export"));

    // start, then ship blocks while a live child task exists.
    stdout(
        maestro(&["feature", "start", "billing-csv-export"], temp_dir.path()),
        &["feature", "start", "billing-csv-export"],
    );

    write_task(&cards_dir, "task-001", "billing-csv-export", "verified");
    write_task(&cards_dir, "task-002", "billing-csv-export", "verified");
    write_task(&cards_dir, "task-003", "billing-csv-export", "in_progress");
    verify_acceptance(temp_dir.path(), "billing-csv-export");

    let ship_args = [
        "feature",
        "ship",
        "billing-csv-export",
        "--outcome",
        "csv export shipped",
    ];
    let ship_stderr = assert_failure(maestro(&ship_args, temp_dir.path()), &ship_args);
    assert!(ship_stderr.contains("task-003"));

    // resolve the live child, then ship succeeds.
    write_task(&cards_dir, "task-003", "billing-csv-export", "verified");
    let ship_output = stdout(maestro(&ship_args, temp_dir.path()), &ship_args);
    assert!(ship_output.contains("shipped billing-csv-export"));
    assert!(ship_output.contains("ship receipt:"));
    // The closing moment points at the archive, not the status dead end (R4).
    assert!(ship_output.contains("next: maestro archive billing-csv-export"));
    assert!(!ship_output.contains("optional: maestro feature archive"));

    let show_after_ship = stdout(
        maestro(&["feature", "show", "billing-csv-export"], temp_dir.path()),
        &["feature", "show", "billing-csv-export"],
    );
    assert!(show_after_ship.contains("status: shipped"));
    // ship --outcome records the one-line result; show renders it.
    assert!(show_after_ship.contains("outcome: csv export shipped"));

    // A shipped feature is terminal, so the default list hides it behind a hint.
    let list_output = stdout(
        maestro(&["feature", "list"], temp_dir.path()),
        &["feature", "list"],
    );
    assert!(!list_output.contains("billing-csv-export"));
    assert!(list_output.contains("terminal feature(s) hidden"));

    // `--all` surfaces it with its frozen status and computed counts.
    let list_all = stdout(
        maestro(&["feature", "list", "--all"], temp_dir.path()),
        &["feature", "list", "--all"],
    );
    assert!(list_all.contains("billing-csv-export"));
    assert!(list_all.contains("shipped"));
    assert!(list_all.contains("NEXT"));
    assert!(list_all.contains("INSPECT"));
    assert!(list_all.contains("maestro feature show billing-csv-export"));
    assert!(untabify(&list_all).contains("\t3\t3\t"));
    // the outcome rides the title column in `list --all`.
    assert!(list_all.contains("csv export shipped"));
}

#[test]
fn feature_authoring_append_flags_are_proposed_only() {
    let temp_dir = TestTempDir::new("maestro-feature-authoring-ergonomics");
    init_git_marker(temp_dir.path());
    stdout(
        maestro(&["init", "--yes"], temp_dir.path()),
        &["init", "--yes"],
    );

    let new_args = [
        "feature",
        "new",
        "Authoring UX",
        "--description",
        "reduce wipe-prone feature authoring",
        "--question",
        "What is still a loose question?",
    ];
    stdout(maestro(&new_args, temp_dir.path()), &new_args);
    let set_args = [
        "feature",
        "set",
        "authoring-ux",
        "--acceptance",
        "first criterion",
        "--area",
        "src/interfaces/cli",
    ];
    stdout(maestro(&set_args, temp_dir.path()), &set_args);

    let append_args = [
        "feature",
        "set",
        "authoring-ux",
        "--add-acceptance",
        "second criterion",
        "--add-area",
        "tests",
        "--add-question",
        "Should this become a decision?",
    ];
    let append = stdout(maestro(&append_args, temp_dir.path()), &append_args);
    assert!(append.contains("+1 acceptance (2 total)"), "{append}");
    assert!(append.contains("+1 areas (2 total)"), "{append}");
    assert!(append.contains("fork hint:"), "{append}");

    let show = stdout(
        maestro(&["feature", "show", "authoring-ux"], temp_dir.path()),
        &["feature", "show", "authoring-ux"],
    );
    assert!(show.contains("first criterion"));
    assert!(show.contains("second criterion"));
    assert!(show.contains("Should this become a decision?"));

    let accept_args = [
        "feature",
        "accept",
        "authoring-ux",
        "--qa",
        "none",
        "--reason",
        "contract-only test",
    ];
    stdout(maestro(&accept_args, temp_dir.path()), &accept_args);
    let late_args = [
        "feature",
        "set",
        "authoring-ux",
        "--add-acceptance",
        "late criterion",
    ];
    let frozen = assert_failure(maestro(&late_args, temp_dir.path()), &late_args);
    assert!(frozen.contains("contract frozen"), "{frozen}");
    assert!(frozen.contains("feature amend"), "{frozen}");
}

#[test]
fn feature_set_edits_one_acceptance_item_by_id() {
    let temp_dir = TestTempDir::new("maestro-feature-edit-acceptance");
    let root = temp_dir.path();
    init_git_marker(root);
    stdout(maestro(&["init", "--yes"], root), &["init", "--yes"]);
    stdout(
        maestro(&["feature", "new", "Edit Acceptance"], root),
        &["feature", "new", "Edit Acceptance"],
    );
    let set_args = [
        "feature",
        "set",
        "edit-acceptance",
        "--acceptance",
        "first criterion",
        "--acceptance",
        "second criterion",
        "--acceptance",
        "third criterion",
        "--area",
        "feature authoring",
    ];
    stdout(maestro(&set_args, root), &set_args);

    let edit_args = [
        "feature",
        "set",
        "edit-acceptance",
        "--edit-acceptance",
        "ac-2",
        "--text",
        "corrected second criterion",
    ];
    let edit = stdout(maestro(&edit_args, root), &edit_args);
    assert!(edit.contains("acceptance edited (1)"), "{edit}");
    assert_eq!(
        feature_acceptance(root, "edit-acceptance"),
        vec![
            "first criterion",
            "corrected second criterion",
            "third criterion"
        ]
    );

    let duplicate_args = [
        "feature",
        "set",
        "edit-acceptance",
        "--edit-acceptance",
        "ac-2",
        "--text",
        "intermediate value",
        "--edit-acceptance",
        "ac-2",
        "--text",
        "last value wins",
    ];
    stdout(maestro(&duplicate_args, root), &duplicate_args);
    assert_eq!(
        feature_acceptance(root, "edit-acceptance"),
        vec!["first criterion", "last value wins", "third criterion"]
    );

    let before_unknown = feature_record(root, "edit-acceptance");
    let unknown_args = [
        "feature",
        "set",
        "edit-acceptance",
        "--edit-acceptance",
        "ac-9",
        "--text",
        "must not write",
    ];
    let unknown = assert_failure(maestro(&unknown_args, root), &unknown_args);
    assert!(unknown.contains("unknown acceptance id"), "{unknown}");
    assert!(unknown.contains("ac-9"), "{unknown}");
    assert_eq!(feature_record(root, "edit-acceptance"), before_unknown);

    let count_guard_args = [
        "feature",
        "set",
        "edit-acceptance",
        "--edit-acceptance",
        "ac-1",
    ];
    let count_guard = assert_failure(maestro(&count_guard_args, root), &count_guard_args);
    assert!(
        count_guard.contains("each --edit-acceptance needs its --text"),
        "{count_guard}"
    );
    assert_eq!(feature_record(root, "edit-acceptance"), before_unknown);

    let help = stdout(
        maestro(&["feature", "set", "--help"], root),
        &["feature", "set", "--help"],
    );
    assert!(help.contains("--edit-acceptance"), "{help}");
    assert!(help.contains("--text"), "{help}");
}

#[test]
fn feature_new_existing_slug_fails_without_clobbering_record() {
    let temp_dir = TestTempDir::new("maestro-feature-existing-slug-test");
    init_git_marker(temp_dir.path());
    stdout(
        maestro(&["init", "--yes"], temp_dir.path()),
        &["init", "--yes"],
    );
    stdout(
        maestro(&["feature", "new", "Billing CSV"], temp_dir.path()),
        &["feature", "new", "Billing CSV"],
    );
    let card_yaml = temp_dir.path().join(".maestro/cards/billing-csv/card.yaml");
    let original = fs::read_to_string(&card_yaml).expect("invariant: card.yaml should be readable");

    let stderr = assert_failure(
        maestro(&["feature", "new", "Billing CSV"], temp_dir.path()),
        &["feature", "new", "Billing CSV"],
    );
    assert!(
        stderr.contains("feature billing-csv already exists"),
        "{stderr}"
    );
    let after =
        fs::read_to_string(&card_yaml).expect("invariant: card.yaml should remain readable");
    assert_eq!(after, original);
}

#[test]
fn feature_verify_records_repeatable_paired_proofs_atomically() {
    let temp_dir = TestTempDir::new("maestro-feature-batch-prove");
    let root = temp_dir.path();
    init_git_marker(root);
    stdout(maestro(&["init", "--yes"], root), &["init", "--yes"]);

    stdout(
        maestro(&["feature", "new", "Batch Proof"], root),
        &["feature", "new", "Batch Proof"],
    );
    let set_args = [
        "feature",
        "set",
        "batch-proof",
        "--acceptance",
        "first behavior works",
        "--acceptance",
        "second behavior works",
        "--acceptance",
        "third behavior works",
        "--area",
        "feature workflow",
    ];
    stdout(maestro(&set_args, root), &set_args);
    let accept_args = [
        "feature",
        "accept",
        "batch-proof",
        "--qa",
        "none",
        "--reason",
        "covered by acceptance evidence",
    ];
    stdout(maestro(&accept_args, root), &accept_args);

    let batch_args = [
        "feature",
        "verify",
        "batch-proof",
        "--prove",
        "ac-1",
        "--evidence",
        "first proof",
        "--prove",
        "ac-3",
        "--evidence",
        "third proof",
    ];
    let batch = stdout(maestro(&batch_args, root), &batch_args);
    assert!(
        batch.contains("recorded explicit ac-1: first proof; explicit ac-3: third proof"),
        "{batch}"
    );

    let before_count_mismatch = feature_record(root, "batch-proof");
    let count_mismatch_args = [
        "feature",
        "verify",
        "batch-proof",
        "--prove",
        "ac-2",
        "--evidence",
        "second proof",
        "--prove",
        "ac-3",
    ];
    let stderr = assert_failure(maestro(&count_mismatch_args, root), &count_mismatch_args);
    assert!(
        stderr.contains("each --prove needs its --evidence"),
        "{stderr}"
    );
    assert_eq!(feature_record(root, "batch-proof"), before_count_mismatch);

    let before_bad_id = feature_record(root, "batch-proof");
    let bad_id_args = [
        "feature",
        "verify",
        "batch-proof",
        "--prove",
        "ac-2",
        "--evidence",
        "second proof",
        "--prove",
        "ac-9",
        "--evidence",
        "bad proof",
    ];
    let stderr = assert_failure(maestro(&bad_id_args, root), &bad_id_args);
    assert!(stderr.contains("unknown acceptance id"), "{stderr}");
    assert!(stderr.contains("ac-9"), "{stderr}");
    assert_eq!(feature_record(root, "batch-proof"), before_bad_id);

    let fixed_args = [
        "feature",
        "verify",
        "batch-proof",
        "--prove",
        "ac-2",
        "--evidence",
        "second proof",
        "--prove",
        "ac-3",
        "--evidence",
        "third proof again",
    ];
    stdout(maestro(&fixed_args, root), &fixed_args);
    let sweep = stdout(
        maestro(&["feature", "verify", "batch-proof"], root),
        &["feature", "verify", "batch-proof"],
    );
    assert!(sweep.contains("proof: first proof OK"), "{sweep}");
    assert!(sweep.contains("proof: second proof OK"), "{sweep}");
    assert!(sweep.contains("proof: third proof again OK"), "{sweep}");
}

#[test]
fn feature_verify_green_sweep_prints_state_appropriate_next_hint() {
    let temp_dir = TestTempDir::new("maestro-feature-green-sweep-next");
    let root = temp_dir.path();
    init_git_marker(root);
    stdout(maestro(&["init", "--yes"], root), &["init", "--yes"]);

    create_qa_none_feature(root, "Ready Hint", "ready-hint");
    record_feature_evidence(root, "ready-hint");
    let ready_sweep = stdout(
        maestro(&["feature", "verify", "ready-hint"], root),
        &["feature", "verify", "ready-hint"],
    );
    assert!(ready_sweep.contains("ok: every acceptance item has evidence"));
    assert!(
        ready_sweep.contains("next: maestro feature start ready-hint"),
        "{ready_sweep}"
    );

    create_qa_none_feature(root, "Ship Hint", "ship-hint");
    stdout(
        maestro(&["feature", "start", "ship-hint"], root),
        &["feature", "start", "ship-hint"],
    );
    record_feature_evidence(root, "ship-hint");
    let ship_sweep = stdout(
        maestro(&["feature", "verify", "ship-hint"], root),
        &["feature", "verify", "ship-hint"],
    );
    assert!(ship_sweep.contains("ok: every acceptance item has evidence"));
    assert!(
        ship_sweep.contains("next: maestro feature ship ship-hint --outcome \"<outcome>\""),
        "{ship_sweep}"
    );

    create_qa_none_feature(root, "Blocked Ship Hint", "blocked-ship-hint");
    stdout(
        maestro(&["feature", "start", "blocked-ship-hint"], root),
        &["feature", "start", "blocked-ship-hint"],
    );
    stdout(
        maestro(
            &[
                "task",
                "create",
                "Live child",
                "--feature",
                "blocked-ship-hint",
            ],
            root,
        ),
        &[
            "task",
            "create",
            "Live child",
            "--feature",
            "blocked-ship-hint",
        ],
    );
    let child_id = id_by_title(root, "Live child");
    record_feature_evidence(root, "blocked-ship-hint");
    let blocked_sweep = stdout(
        maestro(&["feature", "verify", "blocked-ship-hint"], root),
        &["feature", "verify", "blocked-ship-hint"],
    );
    assert!(blocked_sweep.contains("ok: every acceptance item has evidence"));
    assert!(
        blocked_sweep.contains("not yet shippable:"),
        "{blocked_sweep}"
    );
    assert!(
        blocked_sweep.contains(&format!("1 live child task(s): {child_id}")),
        "{blocked_sweep}"
    );
    assert!(
        blocked_sweep.contains("fix: verify or abandon them, then re-ship"),
        "{blocked_sweep}"
    );
    assert!(
        !blocked_sweep.contains("next: maestro feature ship"),
        "{blocked_sweep}"
    );
}

#[test]
fn feature_cancel_via_cli_cascades_to_live_tasks() {
    let temp_dir = TestTempDir::new("maestro-feature-cancel-test");
    init_git_marker(temp_dir.path());
    stdout(
        maestro(&["init", "--yes"], temp_dir.path()),
        &["init", "--yes"],
    );

    stdout(
        maestro(&["feature", "new", "Billing CSV export"], temp_dir.path()),
        &["feature", "new", "Billing CSV export"],
    );
    let set_args = [
        "feature",
        "set",
        "billing-csv-export",
        "--acceptance",
        "exports a valid csv",
        "--area",
        "billing",
    ];
    stdout(maestro(&set_args, temp_dir.path()), &set_args);
    let cards_dir = temp_dir.path().join(".maestro/cards");
    write_baseline(&cards_dir, "billing-csv-export");
    stdout(
        maestro(
            &["feature", "accept", "billing-csv-export"],
            temp_dir.path(),
        ),
        &["feature", "accept", "billing-csv-export"],
    );
    stdout(
        maestro(&["feature", "start", "billing-csv-export"], temp_dir.path()),
        &["feature", "start", "billing-csv-export"],
    );

    write_task(&cards_dir, "task-001", "billing-csv-export", "in_progress");

    let cancel_args = [
        "feature",
        "cancel",
        "billing-csv-export",
        "--reason",
        "scope dropped",
    ];
    let cancel_output = stdout(maestro(&cancel_args, temp_dir.path()), &cancel_args);
    assert!(cancel_output.contains("cancelled billing-csv-export"));
    assert!(cancel_output.contains("task-001"));

    let show_output = stdout(
        maestro(&["feature", "show", "billing-csv-export"], temp_dir.path()),
        &["feature", "show", "billing-csv-export"],
    );
    assert!(show_output.contains("status: cancelled"));

    // The cascade transitions the flat child card in place; the card carries the
    // abandoned status.
    let cascaded = card_doc(temp_dir.path(), "task-001");
    assert_eq!(
        cascaded["status"],
        YamlValue::String("abandoned".into()),
        "{cascaded:?}"
    );
}

#[test]
fn decision_new_list_show_mint_card_ids_and_preserve_template() {
    let temp_dir = TestTempDir::new("maestro-decision-command-test");
    init_git_marker(temp_dir.path());
    stdout(
        maestro(&["init", "--yes"], temp_dir.path()),
        &["init", "--yes"],
    );

    let decisions_dir = temp_dir.path().join(".maestro/decisions");
    ensure_dir(&decisions_dir).expect("invariant: decisions directory should be creatable");
    fs::write(
        decisions_dir.join("decision-007-existing.md"),
        decision_markdown(7, "Existing"),
    )
    .expect("invariant: seed decision should be writable");

    let title = "Use single HARNESS.md instead of three adapter files";
    let new_output = stdout(
        maestro(&["decision", "new", title], temp_dir.path()),
        &["decision", "new", title],
    );
    let decision_id = id_by_title(temp_dir.path(), title);
    assert!(
        new_output.contains(&format!("opened {decision_id} (status: open)")),
        "{new_output}"
    );

    let card = card_doc(temp_dir.path(), &decision_id);
    assert_eq!(
        card["type"],
        YamlValue::String("decision".into()),
        "{card:?}"
    );
    assert!(
        !decisions_dir
            .join(format!(
                "{decision_id}-use-single-harness-md-instead-of-three-adapter-files.md"
            ))
            .is_file(),
        "decision new must not write frozen legacy markdown"
    );

    let list_output = stdout(
        maestro(&["decision", "list"], temp_dir.path()),
        &["decision", "list"],
    );
    assert!(untabify(&list_output).contains("decision-007\tlegacy\tlegacy-md"));
    assert!(untabify(&list_output).contains(&format!("{decision_id}\topen\tglobal")));

    let show_output = stdout(
        maestro(&["decision", "show", &decision_id], temp_dir.path()),
        &["decision", "show", &decision_id],
    );
    assert!(show_output.contains("store:"));
    assert!(show_output.contains(&format!("id: {decision_id}")));
    assert!(show_output.contains(title));

    let doctor = stdout(maestro(&["doctor"], temp_dir.path()), &["doctor"]);
    assert!(
        doctor.contains("warning: decision-007-existing.md"),
        "{doctor}"
    );
    assert!(
        doctor.contains("still contains decision template placeholder text"),
        "{doctor}"
    );
}

/// S3d: `feature new` scaffolds `spec.md` beside the card and every spec
/// surface (receipt, `feature spec` read, subsequent edits) resolves through
/// `.maestro/cards/<id>/` -- with no legacy `features/` tree ever created.
#[test]
fn feature_new_scaffolds_spec_in_the_card_dir_and_feature_spec_reads_it() {
    let temp_dir = TestTempDir::new("maestro-feature-spec-scaffold");
    init_git_marker(temp_dir.path());
    stdout(
        maestro(&["init", "--yes"], temp_dir.path()),
        &["init", "--yes"],
    );

    let create_args = ["feature", "new", "Billing CSV export"];
    let receipt = stdout(maestro(&create_args, temp_dir.path()), &create_args);
    assert!(
        receipt.contains("spec: .maestro/cards/billing-csv-export/spec.md"),
        "{receipt}"
    );
    assert!(
        receipt.contains("decisions: maestro decision new"),
        "the retired per-feature decisions.yaml must not be advertised: {receipt}"
    );

    let spec_path = temp_dir
        .path()
        .join(".maestro/cards/billing-csv-export/spec.md");
    let scaffold = fs::read_to_string(&spec_path).expect("spec.md scaffolded beside the card");
    assert!(scaffold.starts_with("# Billing CSV export"), "{scaffold}");

    fs::write(
        &spec_path,
        "# Billing CSV export\n\n## Current state\n\nrows export by hand today\n",
    )
    .expect("invariant: spec.md should be writable");
    let spec_args = ["feature", "spec", "billing-csv-export"];
    let spec = stdout(maestro(&spec_args, temp_dir.path()), &spec_args);
    assert!(
        spec.contains("rows export by hand today"),
        "feature spec must read the card-dir spec.md: {spec}"
    );
    assert!(spec.contains("## Contract"), "{spec}");
    assert!(!spec.contains("(no spec.md found)"), "{spec}");

    assert!(
        !temp_dir.path().join(".maestro/features").exists(),
        "no legacy features/ tree may reappear"
    );
}

/// L6b: `feature spec` crosses the archive boundary like `feature show` -- an
/// archived feature renders its archived spec.md, not the unreadable-card
/// recovery view.
#[test]
fn feature_spec_falls_through_to_an_archived_feature() {
    let temp_dir = TestTempDir::new("maestro-feature-spec-archived");
    let root = temp_dir.path();
    init_git_marker(root);
    stdout(maestro(&["init", "--yes"], root), &["init", "--yes"]);

    let create_args = ["feature", "new", "Billing CSV export"];
    stdout(maestro(&create_args, root), &create_args);
    fs::write(
        root.join(".maestro/cards/billing-csv-export/spec.md"),
        "# Billing CSV export\n\n## Current state\n\nrows export by hand today\n",
    )
    .expect("invariant: spec.md should be writable");
    let cancel_args = [
        "feature",
        "cancel",
        "billing-csv-export",
        "--reason",
        "scope cut",
    ];
    stdout(maestro(&cancel_args, root), &cancel_args);
    let archive_args = ["feature", "archive", "billing-csv-export"];
    stdout(maestro(&archive_args, root), &archive_args);

    let spec_args = ["feature", "spec", "billing-csv-export"];
    let spec = stdout(maestro(&spec_args, root), &spec_args);
    assert!(spec.contains("archived: true"), "{spec}");
    assert!(
        spec.contains("rows export by hand today"),
        "the archived spec.md renders: {spec}"
    );
    assert!(!spec.contains("status: unreadable"), "{spec}");
}

/// SPEC-archive-memory A2: archiving a terminal feature appends ONE digest
/// line to `archive/cards/INDEX.md` (date, id, coarse `closed`, outcome,
/// child count); a dry-run writes nothing and a re-run duplicates nothing.
#[test]
fn feature_archive_appends_one_digest_line_to_the_archive_index() {
    let temp_dir = TestTempDir::new("maestro-archive-index");
    let root = temp_dir.path();
    init_git_marker(root);
    stdout(maestro(&["init", "--yes"], root), &["init", "--yes"]);

    let create_args = ["feature", "new", "Billing CSV export"];
    stdout(maestro(&create_args, root), &create_args);
    let cancel_args = [
        "feature",
        "cancel",
        "billing-csv-export",
        "--reason",
        "scope cut",
    ];
    stdout(maestro(&cancel_args, root), &cancel_args);

    let index_path = root.join(".maestro/archive/cards/INDEX.md");
    let dry_args = ["feature", "archive", "billing-csv-export", "--dry-run"];
    stdout(maestro(&dry_args, root), &dry_args);
    assert!(!index_path.exists(), "a dry-run must not write the index");

    let archive_args = ["feature", "archive", "billing-csv-export"];
    stdout(maestro(&archive_args, root), &archive_args);
    let index = fs::read_to_string(&index_path).expect("invariant: INDEX.md should exist");
    assert!(
        index.starts_with("# Archived cards\n"),
        "the first append writes the header:\n{index}"
    );
    assert!(
        index.contains("billing-csv-export: closed -- no outcome recorded; 0 child task(s)"),
        "an outcome-less feature falls back in its digest line:\n{index}"
    );

    stdout(maestro(&archive_args, root), &archive_args);
    let again = fs::read_to_string(&index_path).expect("invariant: INDEX.md should exist");
    assert_eq!(index, again, "a sweep re-run appends no duplicate line");
}

/// S8: `feature spec --section --append/--replace` fills the spec during
/// brainstorm/plan -- appends accumulate, replace overwrites, an unknown
/// section is created, and the bare verb renders what was written.
#[test]
fn feature_spec_section_writes_fill_the_spec_from_the_cli() {
    let temp_dir = TestTempDir::new("maestro-feature-spec-write");
    init_git_marker(temp_dir.path());
    stdout(
        maestro(&["init", "--yes"], temp_dir.path()),
        &["init", "--yes"],
    );

    let create_args = ["feature", "new", "Billing CSV export"];
    let receipt = stdout(maestro(&create_args, temp_dir.path()), &create_args);
    assert!(
        receipt.contains(
            "fill: maestro feature spec billing-csv-export --section \"Current state\" --append"
        ),
        "feature new must advertise the spec write verb: {receipt}"
    );

    let id = "billing-csv-export";
    let append_args = [
        "feature",
        "spec",
        id,
        "--section",
        "Current state",
        "--append",
        "rows export by hand today",
    ];
    let appended = stdout(maestro(&append_args, temp_dir.path()), &append_args);
    assert!(
        appended.contains("appended to section \"Current state\""),
        "{appended}"
    );
    stdout(
        maestro(
            &[
                "feature",
                "spec",
                id,
                "--section",
                "Problem",
                "--replace",
                "no streaming path",
            ],
            temp_dir.path(),
        ),
        &["feature", "spec", id, "--section", "Problem", "--replace"],
    );
    let created = stdout(
        maestro(
            &[
                "feature",
                "spec",
                id,
                "--section",
                "Fork walkthroughs",
                "--append",
                "F1: stream vs buffer",
            ],
            temp_dir.path(),
        ),
        &["feature", "spec", id, "--section", "Fork walkthroughs"],
    );
    assert!(created.contains("(new section)"), "{created}");

    let spec = fs::read_to_string(
        temp_dir
            .path()
            .join(".maestro/cards/billing-csv-export/spec.md"),
    )
    .expect("spec.md present");
    assert_eq!(
        spec,
        "# Billing CSV export\n\n## Current state\n\nrows export by hand today\n\n## Problem\n\nno streaming path\n\n## Fork walkthroughs\n\nF1: stream vs buffer\n"
    );

    let render_args = ["feature", "spec", id];
    let render = stdout(maestro(&render_args, temp_dir.path()), &render_args);
    assert!(render.contains("rows export by hand today"), "{render}");
    assert!(render.contains("F1: stream vs buffer"), "{render}");

    let orphan_args = ["feature", "spec", id, "--append", "text without a section"];
    let orphan = assert_failure(maestro(&orphan_args, temp_dir.path()), &orphan_args);
    assert!(
        orphan.contains("--append/--replace need --section"),
        "{orphan}"
    );
    let bare_args = ["feature", "spec", id, "--section", "Current state"];
    let bare = assert_failure(maestro(&bare_args, temp_dir.path()), &bare_args);
    assert!(bare.contains("--section needs the text to write"), "{bare}");
}

/// S8 receipt: written text containing markdown headings gets a note -- the
/// section body runs to the next heading, so embedded headings become section
/// boundaries a later `--section` edit silently stops at.
#[test]
fn feature_spec_write_notes_embedded_headings() {
    let temp_dir = TestTempDir::new("maestro-feature-spec-headings");
    init_git_marker(temp_dir.path());
    stdout(
        maestro(&["init", "--yes"], temp_dir.path()),
        &["init", "--yes"],
    );
    stdout(
        maestro(&["feature", "new", "Billing CSV export"], temp_dir.path()),
        &["feature", "new", "Billing CSV export"],
    );

    let plain_args = [
        "feature",
        "spec",
        "billing-csv-export",
        "--section",
        "Current state",
        "--append",
        "rows export by hand today",
    ];
    let plain = stdout(maestro(&plain_args, temp_dir.path()), &plain_args);
    assert!(!plain.contains("note:"), "{plain}");

    let heading_args = [
        "feature",
        "spec",
        "billing-csv-export",
        "--section",
        "Current state",
        "--append",
        "intro\n\n## Rollout\n\nlater",
    ];
    let noted = stdout(maestro(&heading_args, temp_dir.path()), &heading_args);
    assert!(
        noted.contains("note: the text contains markdown headings"),
        "{noted}"
    );
    assert!(
        noted.contains("a later --section \"Current state\" edit stops at the first one"),
        "{noted}"
    );
}

/// The unarchive pre-flight's child-level collision gets the same remediation
/// decoration as its feature-level and archive-side twins, instead of the
/// bare domain error.
#[test]
fn feature_unarchive_decorates_a_live_child_collision() {
    let temp_dir = TestTempDir::new("maestro-feature-unarchive-child-conflict");
    let root = temp_dir.path();
    init_git_marker(root);
    stdout(maestro(&["init", "--yes"], root), &["init", "--yes"]);

    stdout(
        maestro(&["feature", "new", "Billing CSV export"], root),
        &["feature", "new", "Billing CSV export"],
    );
    let cancel_args = [
        "feature",
        "cancel",
        "billing-csv-export",
        "--reason",
        "scope dropped",
    ];
    stdout(maestro(&cancel_args, root), &cancel_args);
    let archive_args = ["feature", "archive", "billing-csv-export"];
    stdout(maestro(&archive_args, root), &archive_args);

    // An archived child outside the container (the root pool of the archive
    // tree) restores by an individual move; plant a live copy at its target.
    let task_yaml = "schema_version: maestro.card.v1\nid: card-conflict1\ntype: task\ntitle: Conflicting child\nstatus: verified\nparent: billing-csv-export\ncreated_at: \"1\"\nupdated_at: \"1\"\n";
    for home in [
        root.join(".maestro/archive/cards/tasks/card-conflict1"),
        root.join(".maestro/cards/tasks/card-conflict1"),
    ] {
        fs::create_dir_all(&home).expect("invariant: task dir should be creatable");
        fs::write(home.join("task.yaml"), task_yaml)
            .expect("invariant: task record should be writable");
    }

    let unarchive_args = ["feature", "unarchive", "billing-csv-export"];
    let err = assert_failure(maestro(&unarchive_args, root), &unarchive_args);
    assert!(
        err.contains("cannot unarchive billing-csv-export:"),
        "{err}"
    );
    assert!(
        err.contains("a live copy of card-conflict1 already occupies"),
        "{err}"
    );
    assert!(
        err.contains(
            "resolve the live copy conflict, then retry: maestro feature unarchive billing-csv-export"
        ),
        "{err}"
    );
}

#[test]
fn feature_spec_renders_multiline_decision_preview() {
    let temp_dir = TestTempDir::new("maestro-feature-spec-preview");
    init_git_marker(temp_dir.path());
    stdout(
        maestro(&["init", "--yes"], temp_dir.path()),
        &["init", "--yes"],
    );
    stdout(
        maestro(&["feature", "new", "Preview Contract"], temp_dir.path()),
        &["feature", "new", "Preview Contract"],
    );
    stdout(
        maestro(
            &[
                "decision",
                "new",
                "Choose preview shape",
                "--feature",
                "preview-contract",
            ],
            temp_dir.path(),
        ),
        &[
            "decision",
            "new",
            "Choose preview shape",
            "--feature",
            "preview-contract",
        ],
    );
    let decision_id = id_by_title(temp_dir.path(), "Choose preview shape");
    let lock_args = [
        "decision",
        "lock",
        &decision_id,
        "--decision",
        "Use boxed ASCII",
        "--rejected",
        "plain text: less concrete",
        "--preview",
        "+-----+\n| yes |\n+-----+",
    ];
    stdout(maestro(&lock_args, temp_dir.path()), &lock_args);

    let spec = stdout(
        maestro(&["feature", "spec", "preview-contract"], temp_dir.path()),
        &["feature", "spec", "preview-contract"],
    );
    assert!(spec.contains("preview:"), "{spec}");
    assert!(spec.contains("+-----+"), "{spec}");
    assert!(spec.contains("| yes |"), "{spec}");
}

#[test]
fn status_and_feature_list_degrade_when_one_feature_record_is_incompatible() {
    let temp_dir = TestTempDir::new("maestro-feature-roster-degradation");
    init_git_marker(temp_dir.path());
    stdout(
        maestro(&["init", "--yes"], temp_dir.path()),
        &["init", "--yes"],
    );
    stdout(
        maestro(&["feature", "new", "Healthy Feature"], temp_dir.path()),
        &["feature", "new", "Healthy Feature"],
    );
    write_bad_feature_card(temp_dir.path(), "bad-feature");

    let status = stdout(maestro(&["status"], temp_dir.path()), &["status"]);
    assert!(status.contains("healthy-feature"), "{status}");
    assert!(
        untabify(&status).contains("bad-feature\tunreadable"),
        "{status}"
    );
    assert!(status.contains("fix: run maestro migrate-v2"), "{status}");

    let list = stdout(
        maestro(&["feature", "list", "--all"], temp_dir.path()),
        &["feature", "list", "--all"],
    );
    assert!(list.contains("healthy-feature"), "{list}");
    assert!(
        untabify(&list).contains("bad-feature\tunreadable"),
        "{list}"
    );
    assert!(list.contains("fix: run maestro migrate-v2"), "{list}");
}

#[test]
fn feature_spec_and_decision_list_degrade_per_record() {
    let temp_dir = TestTempDir::new("maestro-feature-decision-per-record-degradation");
    init_git_marker(temp_dir.path());
    stdout(
        maestro(&["init", "--yes"], temp_dir.path()),
        &["init", "--yes"],
    );
    stdout(
        maestro(&["feature", "new", "Healthy Feature"], temp_dir.path()),
        &["feature", "new", "Healthy Feature"],
    );
    stdout(
        maestro(
            &[
                "decision",
                "new",
                "Keep healthy decision visible",
                "--feature",
                "healthy-feature",
            ],
            temp_dir.path(),
        ),
        &[
            "decision",
            "new",
            "Keep healthy decision visible",
            "--feature",
            "healthy-feature",
        ],
    );
    let healthy_decision = id_by_title(temp_dir.path(), "Keep healthy decision visible");
    write_bad_feature_card(temp_dir.path(), "bad-feature");
    // A corrupt decision card: a valid card envelope (type decision) whose folded
    // `extra` is malformed, so the decision scan cannot fold it.
    let corrupt_dir = temp_dir.path().join(".maestro/cards/card-corrupt-decision");
    ensure_dir(&corrupt_dir).expect("invariant: corrupt decision dir should be creatable");
    fs::write(
        corrupt_dir.join("card.yaml"),
        "schema_version: maestro.card.v1\nid: card-corrupt-decision\ntype: decision\ntitle: Corrupt Decision\nstatus: open\ncreated_at: \"1\"\nupdated_at: \"1\"\nextra:\n  garbage: [unclosed\n",
    )
    .expect("invariant: corrupt decision card should be writable");

    let spec = stdout(
        maestro(&["feature", "spec", "bad-feature"], temp_dir.path()),
        &["feature", "spec", "bad-feature"],
    );
    assert!(spec.contains("status: unreadable"), "{spec}");
    // The raw dump prints the whole card.yaml under the card header; the inner v1
    // version is visible inside the folded `extra` block.
    assert!(spec.contains("## Raw card.yaml"), "{spec}");
    assert!(
        spec.contains("schema_version: maestro.feature.v1"),
        "{spec}"
    );

    // obsolete-premise: card mode has no per-feature `decisions.yaml`, so there is
    // no `decisions.yaml\tunreadable\tfeature:<id>` row to assert. Surfacing an
    // unreadable decision row is a deferred follow-up; today a corrupt decision card
    // is silently swallowed. The card-mode behavior: the healthy decision lists, the
    // corrupt one does not, and the command still exits 0.
    let decisions = stdout(
        maestro(&["decision", "list"], temp_dir.path()),
        &["decision", "list"],
    );
    assert!(
        untabify(&decisions).contains(&format!(
            "{healthy_decision}\topen\tfeature:healthy-feature"
        )),
        "{decisions}"
    );
    assert!(
        !decisions.contains("card-corrupt-decision"),
        "a corrupt decision card is silently swallowed:\n{decisions}"
    );
}

#[test]
fn status_on_only_v1_feature_records_fails_with_migrate_hint() {
    let temp_dir = TestTempDir::new("maestro-feature-migrate-hint");
    init_git_marker(temp_dir.path());
    stdout(
        maestro(&["init", "--yes"], temp_dir.path()),
        &["init", "--yes"],
    );
    write_bad_feature_card(temp_dir.path(), "bad-feature");

    let error = assert_failure(maestro(&["status"], temp_dir.path()), &["status"]);
    assert!(error.contains("schema mismatch"), "{error}");
    assert!(error.contains("fix: run maestro migrate-v2"), "{error}");
}

#[test]
fn unhinted_errors_do_not_print_fix_line() {
    let temp_dir = TestTempDir::new("maestro-unhinted-error-no-fix-line");
    init_git_marker(temp_dir.path());
    stdout(
        maestro(&["init", "--yes"], temp_dir.path()),
        &["init", "--yes"],
    );

    let error = assert_failure(
        maestro(&["decision", "new", ""], temp_dir.path()),
        &["decision", "new", ""],
    );
    assert!(
        error.contains("Error: decision title cannot be empty"),
        "{error}"
    );
    assert!(!error.contains("\nfix:"), "{error}");
}

#[test]
fn migrate_v2_recreates_decision_scaffold_so_doctor_is_clean() {
    let temp_dir = TestTempDir::new("maestro-migrate-doctor-clean");
    init_git_marker(temp_dir.path());
    stdout(
        maestro(&["init", "--yes"], temp_dir.path()),
        &["init", "--yes"],
    );
    // Card-mode init scaffolds no decision stores at all, so the scaffold is
    // absent by construction; migrate-v2 must create it from nothing.
    assert!(!temp_dir.path().join(".maestro/decisions.yaml").exists());
    assert!(!temp_dir.path().join(".maestro/decisions").exists());
    let feature_dir = temp_dir.path().join(".maestro/features/legacy-feature");
    fs::create_dir_all(&feature_dir).expect("invariant: feature dir should be creatable");
    fs::write(
        feature_dir.join("feature.yaml"),
        "schema_version: maestro.feature.v1\nid: legacy-feature\ntitle: Legacy Feature\nstatus: proposed\ncreated_at: \"1\"\nupdated_at: \"1\"\n",
    )
    .expect("invariant: v1 feature should be writable");

    stdout(maestro(&["migrate-v2"], temp_dir.path()), &["migrate-v2"]);
    let doctor = stdout(maestro(&["doctor"], temp_dir.path()), &["doctor"]);
    assert!(doctor.contains("doctor: ok"), "{doctor}");
    assert!(
        temp_dir.path().join(".maestro/decisions.yaml").is_file(),
        "migrate-v2 should recreate the structured decision store"
    );
    assert!(
        temp_dir.path().join(".maestro/decisions").is_dir(),
        "migrate-v2 should recreate the legacy decisions directory"
    );
}

#[test]
fn decision_new_and_lock_write_structured_feature_record() {
    let temp_dir = TestTempDir::new("maestro-decision-command-complete-test");
    init_git_marker(temp_dir.path());
    stdout(
        maestro(&["init", "--yes"], temp_dir.path()),
        &["init", "--yes"],
    );
    stdout(
        maestro(&["feature", "new", "Agent CLI UX"], temp_dir.path()),
        &["feature", "new", "Agent CLI UX"],
    );

    let open_args = [
        "decision",
        "new",
        "Timestamps use RFC3339",
        "--context",
        "nanosecond epochs are hard to inspect",
        "--feature",
        "agent-cli-ux",
    ];
    let out = stdout(maestro(&open_args, temp_dir.path()), &open_args);
    // Decisions mint typed slug ids now (no decision-001 auto-increment);
    // recover by the unique title.
    let first_id = id_by_title(temp_dir.path(), "Timestamps use RFC3339");
    assert!(first_id.starts_with("dec-"), "{first_id}");
    assert!(
        out.contains(&format!("opened {first_id} (status: open)")),
        "{out}"
    );
    assert!(out.contains("feature: agent-cli-ux"), "{out}");

    let lock_args = [
        "decision",
        "lock",
        &first_id,
        "--decision",
        "render RFC3339 UTC with milliseconds",
        "--rejected",
        "unix seconds: too lossy",
        "--rejected",
        "raw nanos: unreadable",
        "--preview",
        "updated_at: 2026-06-06T00:00:00.000Z",
    ];
    let out = stdout(maestro(&lock_args, temp_dir.path()), &lock_args);
    assert!(out.contains(&format!("locked {first_id}")), "{out}");
    assert!(out.contains("note:"), "{out}");

    // The structured decision record is now folded under the decision card's `extra`.
    let record = decision_record_yaml(temp_dir.path(), &first_id);
    assert!(record.contains("context: nanosecond epochs are hard to inspect"));
    assert!(record.contains("decision: render RFC3339 UTC with milliseconds"));
    assert!(record.contains("unix seconds: too lossy"));
    assert!(record.contains("raw nanos: unreadable"));
    assert!(record.contains("preview:"));
    assert!(record.contains("status: locked"));
    // Feature notes ride the feature card's directory.
    let notes = fs::read_to_string(temp_dir.path().join(".maestro/cards/agent-cli-ux/notes.md"))
        .expect("invariant: feature notes should be readable");
    assert_dated_note_line(
        &notes,
        &format!("{first_id} locked -- Timestamps use RFC3339"),
    );

    let spec = stdout(
        maestro(&["feature", "spec", "agent-cli-ux"], temp_dir.path()),
        &["feature", "spec", "agent-cli-ux"],
    );
    assert!(spec.contains("status: proposed"), "{spec}");
    assert!(spec.contains("## Decisions"), "{spec}");
    assert!(
        spec.contains(&format!("{first_id} [locked]: Timestamps use RFC3339")),
        "{spec}"
    );

    let second_open = [
        "decision",
        "new",
        "Use human timestamps everywhere",
        "--context",
        "the first lock was too narrow",
        "--feature",
        "agent-cli-ux",
    ];
    stdout(maestro(&second_open, temp_dir.path()), &second_open);
    let second_id = id_by_title(temp_dir.path(), "Use human timestamps everywhere");
    let second_lock = [
        "decision",
        "lock",
        &second_id,
        "--decision",
        "render human timestamps in every agent-facing artifact",
        "--rejected",
        "feature-only timestamps: leaves decisions hard to inspect",
        "--supersedes",
        &first_id,
    ];
    stdout(maestro(&second_lock, temp_dir.path()), &second_lock);
    let record = decision_record_yaml(temp_dir.path(), &first_id);
    assert!(record.contains("status: superseded"), "{record}");
    assert!(
        record.contains(&format!("superseded_by: {second_id}")),
        "{record}"
    );
    let notes = fs::read_to_string(temp_dir.path().join(".maestro/cards/agent-cli-ux/notes.md"))
        .expect("invariant: feature notes should be readable");
    assert_dated_note_line(
        &notes,
        &format!("{second_id} locked -- Use human timestamps everywhere"),
    );

    let help = stdout(
        maestro(&["decision", "--help"], temp_dir.path()),
        &["decision", "--help"],
    );
    assert!(help.contains("new"), "{help}");
    assert!(help.contains("show"), "{help}");
    assert!(help.contains("list"), "{help}");
    assert!(help.contains("lock"), "{help}");
}

#[test]
fn feature_and_task_note_create_dated_notes_on_first_write() {
    let temp_dir = TestTempDir::new("maestro-note-command-test");
    init_git_marker(temp_dir.path());
    stdout(
        maestro(&["init", "--yes"], temp_dir.path()),
        &["init", "--yes"],
    );
    stdout(
        maestro(&["feature", "new", "Billing CSV"], temp_dir.path()),
        &["feature", "new", "Billing CSV"],
    );

    let feature_note = stdout(
        maestro(
            &["feature", "note", "billing-csv", "locked: export columns"],
            temp_dir.path(),
        ),
        &["feature", "note", "billing-csv", "locked: export columns"],
    );
    assert!(
        feature_note.contains("noted billing-csv (notes.md created)"),
        "{feature_note}"
    );
    let feature_notes =
        fs::read_to_string(temp_dir.path().join(".maestro/cards/billing-csv/notes.md"))
            .expect("invariant: feature notes should be readable");
    assert!(feature_notes.starts_with("# Billing CSV\n\n"));
    assert_dated_note_line(&feature_notes, "locked: export columns");

    let first = Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(["feature", "note", "billing-csv", "parallel first"])
        .current_dir(temp_dir.path())
        .spawn()
        .expect("invariant: first feature note command should spawn");
    let second = Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(["feature", "note", "billing-csv", "parallel second"])
        .current_dir(temp_dir.path())
        .spawn()
        .expect("invariant: second feature note command should spawn");
    let first = first
        .wait_with_output()
        .expect("invariant: first feature note command should finish");
    let second = second
        .wait_with_output()
        .expect("invariant: second feature note command should finish");
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    let feature_notes =
        fs::read_to_string(temp_dir.path().join(".maestro/cards/billing-csv/notes.md"))
            .expect("invariant: feature notes should be readable after parallel appends");
    assert!(feature_notes.contains("parallel first"), "{feature_notes}");
    assert!(feature_notes.contains("parallel second"), "{feature_notes}");

    stdout(
        maestro(&["task", "create", "Add CSV export"], temp_dir.path()),
        &["task", "create", "Add CSV export"],
    );
    let task_id = id_by_title(temp_dir.path(), "Add CSV export");
    let task_note = stdout(
        maestro(
            &["task", "note", &task_id, "proved: csv opens"],
            temp_dir.path(),
        ),
        &["task", "note", &task_id, "proved: csv opens"],
    );
    assert!(
        task_note.contains(&format!("noted {task_id} (notes.md created)")),
        "{task_note}"
    );
    let task_notes = fs::read_to_string(card_dir(temp_dir.path(), &task_id).join("notes.md"))
        .expect("invariant: task notes should be readable");
    assert!(task_notes.starts_with("# Add CSV export\n\n"));
    assert_dated_note_line(&task_notes, "proved: csv opens");
}

#[test]
fn decision_show_rejects_path_traversal_ids() {
    let temp_dir = TestTempDir::new("maestro-decision-command-traversal-test");
    init_git_marker(temp_dir.path());
    stdout(
        maestro(&["init", "--yes"], temp_dir.path()),
        &["init", "--yes"],
    );

    let stderr = assert_failure(
        maestro(&["decision", "show", "../outside.md"], temp_dir.path()),
        &["decision", "show", "../outside.md"],
    );
    assert!(stderr.contains("invalid decision id"));

    // Card-mode init scaffolds no legacy decisions dir; plant it for the
    // traversal probes against the legacy-markdown read path.
    let decisions_dir = temp_dir.path().join(".maestro/decisions");
    let nested_dir = decisions_dir.join("nested");
    fs::create_dir_all(&nested_dir).expect("invariant: nested decisions dir should be creatable");
    fs::write(nested_dir.join("secret.md"), "secret\n")
        .expect("invariant: nested decision file should be writable");
    let nested_stderr = assert_failure(
        maestro(&["decision", "show", "nested/secret.md"], temp_dir.path()),
        &["decision", "show", "nested/secret.md"],
    );
    assert!(nested_stderr.contains("invalid decision id"));

    fs::write(temp_dir.path().join("outside.md"), "secret\n")
        .expect("invariant: outside file should be writable");
    unix_fs::symlink(
        temp_dir.path().join("outside.md"),
        decisions_dir.join("decision-001-leak.md"),
    )
    .expect("invariant: decision symlink should be creatable");
    let symlink_stderr = assert_failure(
        maestro(&["decision", "show", "decision-001-leak"], temp_dir.path()),
        &["decision", "show", "decision-001-leak"],
    );
    assert!(symlink_stderr.contains("not found"));
}

#[test]
fn decision_new_rejects_an_empty_title() {
    let temp_dir = TestTempDir::new("maestro-decision-command-empty-title");
    init_git_marker(temp_dir.path());
    stdout(
        maestro(&["init", "--yes"], temp_dir.path()),
        &["init", "--yes"],
    );

    let stderr = assert_failure(
        maestro(&["decision", "new", "   "], temp_dir.path()),
        &["decision", "new", "   "],
    );
    assert!(stderr.contains("title cannot be empty"), "{stderr}");

    // The boundary rejected it before any file was written; no husk left behind.
    let decisions_dir = temp_dir.path().join(".maestro/decisions");
    if decisions_dir.is_dir() {
        let husk = fs::read_dir(&decisions_dir)
            .expect("invariant: decisions dir should be listable")
            .count();
        assert_eq!(
            husk, 0,
            "no decision file should be written for an empty title"
        );
    }
}

#[test]
fn feature_verbs_reject_path_traversal_ids() {
    let temp_dir = TestTempDir::new("maestro-feature-command-traversal-test");
    init_git_marker(temp_dir.path());
    stdout(
        maestro(&["init", "--yes"], temp_dir.path()),
        &["init", "--yes"],
    );

    // Read path: `show` joins the id into `features/<id>/feature.yaml`. A `..`
    // id must be rejected before the join can escape the features dir (the
    // `load_record_at` chokepoint).
    let show_stderr = assert_failure(
        maestro(&["feature", "show", "../outside"], temp_dir.path()),
        &["feature", "show", "../outside"],
    );
    assert!(show_stderr.contains("invalid feature id"));

    // An absolute id replaces the whole path on join — a distinct failure mode
    // from `..`, both caught by the single-normal-component rule.
    let abs_stderr = assert_failure(
        maestro(&["feature", "show", "/etc/passwd"], temp_dir.path()),
        &["feature", "show", "/etc/passwd"],
    );
    assert!(abs_stderr.contains("invalid feature id"));

    // `unarchive` is the one verb that never goes through `load_record_at`; it
    // stats and renames directly, so it carries its own guard.
    let unarchive_stderr = assert_failure(
        maestro(&["feature", "unarchive", "../outside"], temp_dir.path()),
        &["feature", "unarchive", "../outside"],
    );
    assert!(unarchive_stderr.contains("invalid feature id"));
}

/// The folded decision record reconstructed from a decision card, serialized back
/// to YAML so an assertion written against the old per-feature `decisions.yaml`
/// text (`record.contains("status: locked")`, ...) reads against the card store.
fn decision_record_yaml(root: &Path, id: &str) -> String {
    let card = card_doc(root, id);
    let mut record = card["extra"].clone();
    if let Some(map) = record.as_mapping_mut() {
        seed_string(map, "id", &card["id"]);
        seed_string(map, "title", &card["title"]);
        seed_string(map, "status", &card["status"]);
        seed_optional_string(map, "feature", &card["parent"]);
        seed_optional_string(map, "context", &card["description"]);
        seed_string(map, "created_at", &card["created_at"]);
    }
    serde_yaml::to_string(&record).expect("invariant: decision record should serialize")
}

/// The folded FeatureRecord reconstructed from a feature card -- card-mode
/// replacement for reading the legacy `.maestro/features/<slug>/feature.yaml`.
/// Returned as a value so a "did this verb mutate the record?" check compares
/// the folded record before/after.
fn feature_record(root: &Path, slug: &str) -> YamlValue {
    let card = card_doc(root, slug);
    let mut record = card["extra"].clone();
    if let Some(map) = record.as_mapping_mut() {
        seed_string(map, "id", &card["id"]);
        seed_string(map, "title", &card["title"]);
        seed_string(map, "status", &card["status"]);
        seed_optional_string(map, "description", &card["description"]);
        seed_string(map, "created_at", &card["created_at"]);
        seed_string(map, "updated_at", &card["updated_at"]);
    }
    record
}

fn seed_string(map: &mut serde_yaml::Mapping, key: &str, value: &YamlValue) {
    let key = YamlValue::String(key.to_string());
    if !map.contains_key(&key) {
        map.insert(key, value.clone());
    }
}

fn seed_optional_string(map: &mut serde_yaml::Mapping, key: &str, value: &YamlValue) {
    if !value.is_null() {
        seed_string(map, key, value);
    }
}

fn feature_acceptance(root: &Path, slug: &str) -> Vec<String> {
    let yaml = feature_record(root, slug);
    yaml.get("acceptance")
        .and_then(YamlValue::as_sequence)
        .expect("invariant: acceptance is a sequence")
        .iter()
        .map(|value| {
            value
                .as_str()
                .expect("invariant: acceptance item is a string")
                .to_string()
        })
        .collect()
}

fn create_qa_none_feature(root: &Path, title: &str, slug: &str) {
    stdout(
        maestro(&["feature", "new", title], root),
        &["feature", "new", title],
    );
    let set_args = [
        "feature",
        "set",
        slug,
        "--acceptance",
        "observable behavior works",
        "--area",
        "feature workflow",
    ];
    stdout(maestro(&set_args, root), &set_args);
    let accept_args = [
        "feature",
        "accept",
        slug,
        "--qa",
        "none",
        "--reason",
        "covered by acceptance evidence",
    ];
    stdout(maestro(&accept_args, root), &accept_args);
}

fn record_feature_evidence(root: &Path, slug: &str) {
    let prove_args = [
        "feature",
        "verify",
        slug,
        "--prove",
        "ac-1",
        "--evidence",
        "observed in integration test",
    ];
    stdout(maestro(&prove_args, root), &prove_args);
}

/// Write a minimal QA baseline (one `[bl-001]` scenario) so the accept gate's
/// baseline precondition (F) and the ship gate's coverage check are satisfiable.
/// In card mode the QA artifact rides the feature card directory at
/// `.maestro/cards/<id>/qa.md`, so callers pass `.maestro/cards` here.
fn write_baseline(cards_dir: &Path, id: &str) {
    let dir = cards_dir.join(id);
    ensure_dir(&dir).expect("invariant: card directory should be creatable");
    fs::write(
        dir.join("qa.md"),
        "---\namend_log_position: 0\n---\n\n### QA Baseline Contract\n\n- Scenario Matrix:\n  - [bl-001] csv export round-trips\n",
    )
    .expect("invariant: qa.md should be writable");
}

/// Write a counting QA slice (scenarios + evidence) covering `[bl-001]`.
fn write_qa_slice(cards_dir: &Path, id: &str) {
    let dir = cards_dir.join(id);
    ensure_dir(&dir).expect("invariant: card directory should be creatable");
    let path = dir.join("qa.md");
    let mut contents = fs::read_to_string(&path).unwrap_or_default();
    contents.push_str("\n```yaml\nslices:\n  - scenarios: [\"bl-001\"]\n    evidence: [\"manual: exported csv opens in a spreadsheet\"]\n```\n");
    fs::write(path, contents).expect("invariant: qa.md should be writable");
}

/// Fabricate a child task as a flat card at `.maestro/cards/<id>/card.yaml` with
/// `parent: <feature_id>` (card-mode feature ownership is the flat `parent`, not a
/// directory). A literal id like `task-001` is non-opaque but the card scanner and
/// the cancel/archive cascades accept it. The card carries a full TaskRecord under
/// `extra` so the cancel cascade can load and transition the child.
fn write_task(cards_dir: &Path, id: &str, feature_id: &str, state: &str) {
    let card_dir = cards_dir.join(id);
    ensure_dir(&card_dir).expect("invariant: card directory should be creatable");
    fs::write(
        card_dir.join("card.yaml"),
        format!(
            "schema_version: maestro.card.v1\nid: {id}\ntype: task\ntitle: {id}\nstatus: {state}\nparent: {feature_id}\ncreated_at: \"2026-06-06T00:00:00.000Z\"\nupdated_at: \"2026-06-06T00:00:00.000Z\"\nextra:\n  schema_version: maestro.task.v2\n  id: {id}\n  title: {id}\n  state: {state}\n  acceptance_locked: false\n  verification: {{}}\n  created_at: \"2026-06-06T00:00:00.000Z\"\n  updated_at: \"2026-06-06T00:00:00.000Z\"\n"
        ),
    )
    .expect("invariant: card.yaml should be writable");
}

/// Fabricate a schema-incompatible feature card directly in the card store at
/// `.maestro/cards/<id>/card.yaml`: a valid `maestro.card.v1` envelope whose folded
/// `extra` carries the OLD `maestro.feature.v1` version. The card load chokes on the
/// inner version, so the roster surfaces it as `unreadable`; keeping the inner
/// version EXACTLY `maestro.feature.v1` is what makes the `migrate-v2` fix hint fire
/// (any other version yields the generic doctor hint).
fn write_bad_feature_card(root: &Path, id: &str) {
    let dir = root.join(".maestro/cards").join(id);
    ensure_dir(&dir).expect("invariant: bad feature card dir should be creatable");
    fs::write(
        dir.join("card.yaml"),
        format!(
            "schema_version: maestro.card.v1\nid: {id}\ntype: feature\ntitle: Bad Feature\nstatus: proposed\ncreated_at: \"1\"\nupdated_at: \"1\"\nextra:\n  schema_version: maestro.feature.v1\n  id: {id}\n  title: Bad Feature\n  status: proposed\n  created_at: \"1\"\n  updated_at: \"1\"\n"
        ),
    )
    .expect("invariant: bad feature card should be writable");
}

/// Drive a fresh feature all the way to Shipped: new -> set -> baseline+slice ->
/// accept -> start -> one verified child -> ship.
fn ship_feature(root: &Path, title: &str, slug: &str, child: &str) {
    stdout(
        maestro(&["feature", "new", title], root),
        &["feature", "new", title],
    );
    let set_args = [
        "feature",
        "set",
        slug,
        "--acceptance",
        "works",
        "--area",
        "core",
    ];
    stdout(maestro(&set_args, root), &set_args);
    let cards_dir = root.join(".maestro/cards");
    write_baseline(&cards_dir, slug);
    write_qa_slice(&cards_dir, slug);
    stdout(
        maestro(&["feature", "accept", slug], root),
        &["feature", "accept", slug],
    );
    stdout(
        maestro(&["feature", "start", slug], root),
        &["feature", "start", slug],
    );
    write_task(&cards_dir, child, slug, "verified");
    verify_acceptance(root, slug);
    stdout(
        maestro(&["feature", "ship", slug], root),
        &["feature", "ship", slug],
    );
}

fn verify_acceptance(root: &Path, slug: &str) {
    let prove_args = [
        "feature",
        "verify",
        slug,
        "--prove",
        "ac-1",
        "--evidence",
        "fixture evidence",
    ];
    stdout(maestro(&prove_args, root), &prove_args);
    stdout(
        maestro(&["feature", "verify", slug], root),
        &["feature", "verify", slug],
    );
}

#[test]
fn feature_create_refuses_a_slug_held_in_the_archive() {
    let temp_dir = TestTempDir::new("maestro-feature-archive-slug");
    init_git_marker(temp_dir.path());
    stdout(
        maestro(&["init", "--yes"], temp_dir.path()),
        &["init", "--yes"],
    );

    // An archived feature still owns its slug; `create` must not reissue it (L6a).
    let archived = temp_dir.path().join(".maestro/archive/cards/csv-export");
    ensure_dir(&archived).expect("invariant: archive card dir should be creatable");
    fs::write(
        archived.join("card.yaml"),
        "schema_version: maestro.card.v1\nid: csv-export\ntype: feature\ntitle: CSV Export\nstatus: shipped\ncreated_at: \"1\"\nupdated_at: \"1\"\n",
    )
    .expect("invariant: archived card yaml should be writable");

    let args = ["feature", "new", "CSV Export"];
    let stderr = assert_failure(maestro(&args, temp_dir.path()), &args);
    assert!(stderr.contains("csv-export"));
    assert!(stderr.contains("archive"));
}

/// Drive a feature to Shipped, then archive it: the feature card dir + its
/// terminal child card dirs leave the live scan, the QA sidecar travels inside
/// the archived feature dir, reads fall through (L6b), and unarchive
/// round-trips (§5.9).
#[test]
fn feature_archive_cascades_children_with_qa_and_round_trips() {
    let temp_dir = TestTempDir::new("maestro-feature-archive-shipped");
    let root = temp_dir.path();
    init_git_marker(root);
    stdout(maestro(&["init", "--yes"], root), &["init", "--yes"]);

    stdout(
        maestro(&["feature", "new", "Billing CSV export"], root),
        &["feature", "new", "Billing CSV export"],
    );
    let set_args = [
        "feature",
        "set",
        "billing-csv-export",
        "--acceptance",
        "exports a valid csv",
        "--area",
        "billing",
    ];
    stdout(maestro(&set_args, root), &set_args);

    let cards_dir = root.join(".maestro/cards");
    write_baseline(&cards_dir, "billing-csv-export");
    write_qa_slice(&cards_dir, "billing-csv-export");
    stdout(
        maestro(&["feature", "accept", "billing-csv-export"], root),
        &["feature", "accept", "billing-csv-export"],
    );

    // A non-terminal feature cannot be archived.
    let in_progress = ["feature", "archive", "billing-csv-export"];
    stdout(
        maestro(&["feature", "start", "billing-csv-export"], root),
        &["feature", "start", "billing-csv-export"],
    );
    let not_terminal = assert_failure(maestro(&in_progress, root), &in_progress);
    assert!(not_terminal.contains("not terminal"));

    write_task(&cards_dir, "task-001", "billing-csv-export", "verified");
    write_task(&cards_dir, "task-002", "billing-csv-export", "verified");
    verify_acceptance(root, "billing-csv-export");
    stdout(
        maestro(&["feature", "ship", "billing-csv-export"], root),
        &["feature", "ship", "billing-csv-export"],
    );

    // Archive cascades the terminal children alongside the feature.
    let archive_args = ["feature", "archive", "billing-csv-export"];
    let archived = stdout(maestro(&archive_args, root), &archive_args);
    assert!(archived.contains("archived feature billing-csv-export"));
    assert!(archived.contains("task-001"));
    assert!(archived.contains("task-002"));
    assert!(archived.contains("archive receipt:"));
    assert!(archived.contains("child tasks: 2 archived"));
    assert!(archived.contains("restore: maestro feature unarchive billing-csv-export"));

    // The feature card dir (QA sidecar inside) + each child card dir moved into
    // the flat archive sibling tree.
    let archive_cards = root.join(".maestro/archive/cards");
    let archived_feature = archive_cards.join("billing-csv-export");
    assert!(archived_feature.join("card.yaml").is_file());
    assert!(archived_feature.join("qa.md").is_file());
    assert!(!cards_dir.join("billing-csv-export").exists());
    assert!(archive_cards.join("task-001").join("card.yaml").is_file());
    assert!(archive_cards.join("task-002").join("card.yaml").is_file());
    assert!(!cards_dir.join("task-001").exists());
    assert!(!cards_dir.join("task-002").exists());

    // L6b: show falls through to the archive; list --all reads it.
    let show = stdout(
        maestro(&["feature", "show", "billing-csv-export"], root),
        &["feature", "show", "billing-csv-export"],
    );
    assert!(show.contains("status: shipped"));
    assert!(show.contains("tasks_total: 2"));
    // L6b: the read-fallthrough discloses it is an archive view (R26).
    assert!(show.contains("archived: true"));
    // A full cascade left nothing live, so the count is complete and unannotated.
    assert!(!show.contains("not archived"));
    let list_all = stdout(
        maestro(&["feature", "list", "--all"], root),
        &["feature", "list", "--all"],
    );
    assert!(list_all.contains("billing-csv-export"));

    let archived_accept_args = ["feature", "accept", "billing-csv-export"];
    let archived_accept =
        assert_failure(maestro(&archived_accept_args, root), &archived_accept_args);
    assert!(
        archived_accept.contains("feature billing-csv-export is archived (shipped)"),
        "{archived_accept}"
    );
    assert!(
        archived_accept.contains("inspect: maestro feature show billing-csv-export"),
        "{archived_accept}"
    );
    assert!(
        archived_accept.contains("restore: maestro feature unarchive billing-csv-export"),
        "{archived_accept}"
    );
    assert!(
        archived_accept.contains("then: retry the command"),
        "{archived_accept}"
    );
    let missing_accept_args = ["feature", "accept", "missing-feature"];
    let missing_accept = assert_failure(maestro(&missing_accept_args, root), &missing_accept_args);
    assert!(missing_accept.contains("feature not found: missing-feature"));

    // Idempotent: re-archiving is a no-op at exit 0.
    let again = maestro(&archive_args, root);
    let again_out = stdout(again, &archive_args);
    assert!(again_out.contains("already archived"));

    // Unarchive restores the feature card dir + each archived child card dir.
    let unarchive_args = ["feature", "unarchive", "billing-csv-export"];
    let restored = stdout(maestro(&unarchive_args, root), &unarchive_args);
    assert!(restored.contains("unarchived feature billing-csv-export"));
    assert!(restored.contains("task-001"));
    assert!(restored.contains("restore receipt:"));
    assert!(restored.contains("next: maestro status"));
    assert!(cards_dir.join("billing-csv-export").join("qa.md").is_file());
    assert!(cards_dir.join("task-001").join("card.yaml").is_file());
    assert!(!archived_feature.exists());
}

/// The cascade case: every terminal `parent=<feature>` card dir moves to the
/// flat archive tree alongside the feature card dir.
#[test]
fn feature_archive_moves_terminal_child_cards_with_feature() {
    let temp_dir = TestTempDir::new("maestro-feature-archive-straggler");
    let root = temp_dir.path();
    init_git_marker(root);
    stdout(maestro(&["init", "--yes"], root), &["init", "--yes"]);

    stdout(
        maestro(&["feature", "new", "Billing CSV export"], root),
        &["feature", "new", "Billing CSV export"],
    );
    let set_args = [
        "feature",
        "set",
        "billing-csv-export",
        "--acceptance",
        "exports a valid csv",
        "--area",
        "billing",
    ];
    stdout(maestro(&set_args, root), &set_args);
    let cards_dir = root.join(".maestro/cards");
    write_baseline(&cards_dir, "billing-csv-export");
    stdout(
        maestro(&["feature", "accept", "billing-csv-export"], root),
        &["feature", "accept", "billing-csv-export"],
    );
    stdout(
        maestro(&["feature", "start", "billing-csv-export"], root),
        &["feature", "start", "billing-csv-export"],
    );

    // Three live children; cancel abandons them so the feature is terminal with
    // terminal children (cheaper than the ship gate, exercises the same cascade).
    write_task(&cards_dir, "task-001", "billing-csv-export", "in_progress");
    write_task(&cards_dir, "task-002", "billing-csv-export", "in_progress");
    write_task(&cards_dir, "task-003", "billing-csv-export", "in_progress");
    let cancel_args = [
        "feature",
        "cancel",
        "billing-csv-export",
        "--reason",
        "scope dropped",
    ];
    let cancelled = stdout(maestro(&cancel_args, root), &cancel_args);
    assert!(cancelled.contains("cancel receipt:"));
    // The closing moment points at the archive, not the status dead end (R4).
    assert!(cancelled.contains("next: maestro archive billing-csv-export"));

    // A live standalone task blocked by the terminal child task-002 entangles it.
    stdout(
        maestro(&["task", "create", "Holder"], root),
        &["task", "create", "Holder"],
    );
    let holder_id = id_by_title(root, "Holder");
    let block_args = [
        "task", "block", &holder_id, "--reason", "needs 2", "--by", "task-002",
    ];
    stdout(maestro(&block_args, root), &block_args);

    // Archive moves the feature card dir and every terminal child card dir.
    let archive_args = ["feature", "archive", "billing-csv-export"];
    let first = stdout(maestro(&archive_args, root), &archive_args);
    assert!(first.contains("archived feature billing-csv-export"));
    assert!(first.contains("task-001"));
    assert!(first.contains("task-002"));
    assert!(first.contains("task-003"));
    let archive_cards = root.join(".maestro/archive/cards");
    assert!(archive_cards.join("billing-csv-export/card.yaml").is_file());
    assert!(archive_cards.join("task-001/card.yaml").is_file());
    assert!(archive_cards.join("task-002/card.yaml").is_file());
    assert!(archive_cards.join("task-003/card.yaml").is_file());
    // The entangled live blocker-holder stays in the live store.
    assert!(card_support::card_record_path(root, &holder_id).is_file());

    // R25/R26: an archived show discloses it is the archive view.
    let show = stdout(
        maestro(&["feature", "show", "billing-csv-export"], root),
        &["feature", "show", "billing-csv-export"],
    );
    assert!(show.contains("archived: true"));
    assert!(show.contains("tasks_total: 3"));

    let again = stdout(maestro(&archive_args, root), &archive_args);
    assert!(again.contains("already archived"));
}

/// A decision entry is a record, not a workable child: an open (never-locked)
/// fork must not block archive, and it rides the container move inside
/// `decisions.yaml` rather than counting as a cascaded child task.
#[test]
fn feature_archive_ignores_open_decisions_and_moves_them_with_the_container() {
    let temp_dir = TestTempDir::new("maestro-feature-archive-open-decision");
    let root = temp_dir.path();
    init_git_marker(root);
    stdout(maestro(&["init", "--yes"], root), &["init", "--yes"]);

    stdout(
        maestro(&["feature", "new", "Billing CSV export"], root),
        &["feature", "new", "Billing CSV export"],
    );
    let decision_args = [
        "decision",
        "new",
        "Pick the writer",
        "--feature",
        "billing-csv-export",
        "--context",
        "csv crate vs hand-rolled",
    ];
    stdout(maestro(&decision_args, root), &decision_args);
    let cancel_args = [
        "feature",
        "cancel",
        "billing-csv-export",
        "--reason",
        "scope dropped",
    ];
    stdout(maestro(&cancel_args, root), &cancel_args);

    let archive_args = ["feature", "archive", "billing-csv-export"];
    let archived = stdout(maestro(&archive_args, root), &archive_args);
    assert!(archived.contains("archived feature billing-csv-export"));
    assert!(archived.contains("child tasks: 0 archived"));

    // The whole container moved, the open fork still inside it.
    let archived_feature = root.join(".maestro/archive/cards/billing-csv-export");
    assert!(archived_feature.join("card.yaml").is_file());
    let decisions = fs::read_to_string(archived_feature.join("decisions.yaml"))
        .expect("invariant: archived container should keep its decisions.yaml");
    assert!(decisions.contains("Pick the writer"));
    assert!(!root.join(".maestro/cards/billing-csv-export").exists());

    // Round-trip: the decision rides back without counting as a child task.
    let unarchive_args = ["feature", "unarchive", "billing-csv-export"];
    let restored = stdout(maestro(&unarchive_args, root), &unarchive_args);
    assert!(restored.contains("child tasks: 0 restored"));
    let live_decisions =
        fs::read_to_string(root.join(".maestro/cards/billing-csv-export/decisions.yaml"))
            .expect("invariant: restored container should keep its decisions.yaml");
    assert!(live_decisions.contains("Pick the writer"));
}

/// `feature archive --closed` archives every terminal feature -- shipped AND
/// cancelled (each cascading its children) -- and leaves live features in the
/// live tree. Doctor surfaces the backlog before the sweep and goes green after.
#[test]
fn feature_archive_closed_sweeps_terminal_features() {
    let temp_dir = TestTempDir::new("maestro-feature-archive-bulk");
    let root = temp_dir.path();
    init_git_marker(root);
    stdout(maestro(&["init", "--yes"], root), &["init", "--yes"]);

    // Two shipped features (each with a verified child), one cancelled, one
    // still in progress.
    ship_feature(root, "Alpha export", "alpha-export", "task-001");
    ship_feature(root, "Beta export", "beta-export", "task-002");

    stdout(
        maestro(&["feature", "new", "Delta export"], root),
        &["feature", "new", "Delta export"],
    );
    let cancel_delta = [
        "feature",
        "cancel",
        "delta-export",
        "--reason",
        "scope dropped",
    ];
    stdout(maestro(&cancel_delta, root), &cancel_delta);

    stdout(
        maestro(&["feature", "new", "Gamma export"], root),
        &["feature", "new", "Gamma export"],
    );
    let set_gamma = [
        "feature",
        "set",
        "gamma-export",
        "--acceptance",
        "works",
        "--area",
        "core",
    ];
    stdout(maestro(&set_gamma, root), &set_gamma);
    write_baseline(&root.join(".maestro/cards"), "gamma-export");
    stdout(
        maestro(&["feature", "accept", "gamma-export"], root),
        &["feature", "accept", "gamma-export"],
    );
    stdout(
        maestro(&["feature", "start", "gamma-export"], root),
        &["feature", "start", "gamma-export"],
    );

    // Doctor reports the archive backlog as an advisory before the sweep (R4).
    let doctor = stdout(maestro(&["doctor"], root), &["doctor"]);
    assert!(doctor.contains(
        "warning: 3 closed feature(s) not archived; sweep with `maestro feature archive --closed`"
    ));

    // --closed archives the shipped and cancelled features and their children;
    // in-progress gamma stays live.
    let bulk = ["feature", "archive", "--closed"];
    let out = stdout(maestro(&bulk, root), &bulk);
    assert!(out.contains("archived closed features"));
    assert!(out.contains("archive summary:"));
    assert!(out.contains("features: 3 archived"));
    assert!(out.contains("child tasks: 2 archived"));
    assert!(out.contains("next: maestro status"));

    let cards_dir = root.join(".maestro/cards");
    let archive_cards = root.join(".maestro/archive/cards");
    assert!(archive_cards.join("alpha-export/card.yaml").is_file());
    assert!(archive_cards.join("beta-export/card.yaml").is_file());
    assert!(archive_cards.join("delta-export/card.yaml").is_file());
    assert!(archive_cards.join("task-001/card.yaml").is_file());
    assert!(archive_cards.join("task-002/card.yaml").is_file());
    // The in-progress feature is untouched and stays in the live store.
    assert!(cards_dir.join("gamma-export").join("card.yaml").is_file());
    assert!(!archive_cards.join("gamma-export").exists());

    // With the backlog swept, the doctor advisory becomes a green check.
    let doctor_after = stdout(maestro(&["doctor"], root), &["doctor"]);
    assert!(doctor_after.contains("check archive: ok (no closed features awaiting archive)"));
    assert!(!doctor_after.contains("closed feature(s) not archived"));

    // Idempotent: no closed features remain live.
    let again = stdout(maestro(&bulk, root), &bulk);
    assert!(again.contains("no closed features to archive"));

    // A feature id and --closed are mutually exclusive.
    let both = ["feature", "archive", "alpha-export", "--closed"];
    let err = assert_failure(maestro(&both, root), &both);
    assert!(err.contains("not both"));

    // Neither an id nor --closed: the remedy must not claim "not both".
    let neither = ["feature", "archive"];
    let err = assert_failure(maestro(&neither, root), &neither);
    assert!(err.contains("provide a feature id or --closed"));
    assert!(!err.contains("not both"));
}

#[test]
fn feature_set_on_a_terminal_feature_does_not_recommend_the_dead_end_amend() {
    let temp_dir = TestTempDir::new("maestro-feature-set-terminal-test");
    init_git_marker(temp_dir.path());
    stdout(
        maestro(&["init", "--yes"], temp_dir.path()),
        &["init", "--yes"],
    );
    stdout(
        maestro(&["feature", "new", "Billing CSV export"], temp_dir.path()),
        &["feature", "new", "Billing CSV export"],
    );
    let cancel_args = [
        "feature",
        "cancel",
        "billing-csv-export",
        "--reason",
        "scope dropped",
    ];
    stdout(maestro(&cancel_args, temp_dir.path()), &cancel_args);

    // On a terminal feature, `set` must not point at `amend`: amend dead-ends on
    // terminal too, so recommending it leaves no path forward.
    let set_args = [
        "feature",
        "set",
        "billing-csv-export",
        "--acceptance",
        "exports a valid csv",
        "--area",
        "billing",
    ];
    let err = assert_failure(maestro(&set_args, temp_dir.path()), &set_args);
    assert!(err.contains("terminal (status: cancelled)"));
    assert!(!err.contains("feature amend"));
}

/// Collapse aligned-table padding (runs of 2+ spaces) back to tabs so cell
/// assertions stay width-independent.
fn untabify(output: &str) -> String {
    output
        .lines()
        .map(|line| {
            line.split("  ")
                .map(str::trim)
                .filter(|cell| !cell.is_empty())
                .collect::<Vec<_>>()
                .join("\t")
        })
        .collect::<Vec<_>>()
        .join("\n")
}
