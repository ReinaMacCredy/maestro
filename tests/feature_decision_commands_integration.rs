mod support;

use std::fs;
use std::os::unix::fs as unix_fs;
use std::path::Path;
use std::process::Command;

use maestro::decisions::template::decision_markdown;
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
    for args in [
        vec!["task", "explore", "task-001"],
        vec!["task", "accept", "task-001"],
        vec!["task", "claim", "task-001"],
        vec![
            "task",
            "complete",
            "task-001",
            "--summary",
            "done",
            "--claim",
            "first behavior works",
            "--proof",
            "first behavior works",
        ],
        vec!["task", "verify", "task-001"],
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
    assert!(sweep.contains("proof: task-001 OK"), "{sweep}");
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
    assert!(sweep.contains("proof: task-001 OK"), "{sweep}");
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
    let show = stdout(
        maestro(&["feature", "show", "coverage-display"], temp_dir.path()),
        &["feature", "show", "coverage-display"],
    );
    assert!(show.contains("- [ac-1] first behavior works"), "{show}");
    assert!(show.contains("- [ac-2] second behavior works"), "{show}");
    assert!(show.contains("covers: task-001"), "{show}");
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

    verify_task_claim(temp_dir.path(), "task-001", "second behavior works");
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
    assert!(sweep.contains("proof: task-001 OK"), "{sweep}");
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
    verify_task_claim(temp_dir.path(), "task-002", "second behavior works");
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
        stale_preview.contains("task-002 settled at"),
        "{stale_preview}"
    );
    let refreshed = stdout(
        maestro(&["feature", "verify", "coverage-display"], temp_dir.path()),
        &["feature", "verify", "coverage-display"],
    );
    assert!(refreshed.contains("re-derived after: task-002 settled at"));
    assert!(refreshed.contains("proof: task-001, task-002 OK"));
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
    assert!(create_output.contains("spec: .maestro/features/billing-csv-export/spec.md"));
    assert!(
        temp_dir
            .path()
            .join(".maestro/features/billing-csv-export/spec.md")
            .is_file()
    );
    assert!(
        temp_dir
            .path()
            .join(".maestro/features/billing-csv-export/decisions.yaml")
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
    assert!(accept_stderr.contains("skill: qa-baseline"));
    assert!(accept_stderr.contains("target: .maestro/features/billing-csv-export/qa.md"));
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
    let features_dir = temp_dir.path().join(".maestro/features");
    write_baseline(&features_dir, "billing-csv-export");
    write_qa_slice(&features_dir, "billing-csv-export");

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

    let tasks_dir = temp_dir.path().join(".maestro/tasks");
    write_task(&tasks_dir, "task-001", "billing-csv-export", "verified");
    write_task(&tasks_dir, "task-002", "billing-csv-export", "verified");
    write_task(&tasks_dir, "task-003", "billing-csv-export", "in_progress");
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
    write_task(&tasks_dir, "task-003", "billing-csv-export", "verified");
    let ship_output = stdout(maestro(&ship_args, temp_dir.path()), &ship_args);
    assert!(ship_output.contains("shipped billing-csv-export"));
    assert!(ship_output.contains("ship receipt:"));
    assert!(ship_output.contains("next: maestro status"));

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
    assert!(list_all.contains("\t3\t3\t"));
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

    let before_unknown = feature_yaml(root, "edit-acceptance");
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
    assert_eq!(feature_yaml(root, "edit-acceptance"), before_unknown);

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
    assert_eq!(feature_yaml(root, "edit-acceptance"), before_unknown);

    let help = stdout(
        maestro(&["feature", "set", "--help"], root),
        &["feature", "set", "--help"],
    );
    assert!(help.contains("--edit-acceptance"), "{help}");
    assert!(help.contains("--text"), "{help}");
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

    let before_count_mismatch = feature_yaml(root, "batch-proof");
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
    assert_eq!(feature_yaml(root, "batch-proof"), before_count_mismatch);

    let before_bad_id = feature_yaml(root, "batch-proof");
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
    assert_eq!(feature_yaml(root, "batch-proof"), before_bad_id);

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
        blocked_sweep.contains("1 live child task(s): task-001"),
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
    write_baseline(
        &temp_dir.path().join(".maestro/features"),
        "billing-csv-export",
    );
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

    let tasks_dir = temp_dir.path().join(".maestro/tasks");
    write_task(&tasks_dir, "task-001", "billing-csv-export", "in_progress");

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

    let task_raw = fs::read_to_string(
        tasks_dir
            .parent()
            .expect("invariant: tasks dir should have parent")
            .join("features/billing-csv-export/tasks/task-001-task-001/task.yaml"),
    )
    .expect("invariant: cascaded child task should be readable");
    assert!(task_raw.contains("abandoned"));
}

#[test]
fn decision_new_list_show_auto_increment_and_preserve_template() {
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
    assert!(new_output.contains("opened decision-008 (status: open)"));

    let decisions_yaml = temp_dir.path().join(".maestro/decisions.yaml");
    let yaml =
        fs::read_to_string(&decisions_yaml).expect("invariant: decisions.yaml should be readable");
    assert!(yaml.contains("id: decision-008"), "{yaml}");
    assert!(
        !decisions_dir
            .join("decision-008-use-single-harness-md-instead-of-three-adapter-files.md")
            .is_file(),
        "decision new must not write frozen legacy markdown"
    );

    let list_output = stdout(
        maestro(&["decision", "list"], temp_dir.path()),
        &["decision", "list"],
    );
    assert!(list_output.contains("decision-007\tlegacy\tlegacy-md"));
    assert!(list_output.contains("decision-008\topen\tglobal"));

    let show_output = stdout(
        maestro(&["decision", "show", "decision-008"], temp_dir.path()),
        &["decision", "show", "decision-008"],
    );
    assert!(show_output.contains("store:"));
    assert!(show_output.contains("id: decision-008"));
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
    let lock_args = [
        "decision",
        "lock",
        "decision-001",
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
    let bad_dir = temp_dir.path().join(".maestro/features/bad-feature");
    fs::create_dir_all(&bad_dir).expect("invariant: bad feature dir should be creatable");
    fs::write(
        bad_dir.join("feature.yaml"),
        "schema_version: maestro.feature.v1\nid: bad-feature\ntitle: Bad Feature\nstatus: proposed\ncreated_at: \"1\"\nupdated_at: \"1\"\n",
    )
    .expect("invariant: bad feature record should be writable");

    let status = stdout(maestro(&["status"], temp_dir.path()), &["status"]);
    assert!(status.contains("healthy-feature"), "{status}");
    assert!(status.contains("bad-feature\tunreadable"), "{status}");
    assert!(status.contains("fix: run maestro migrate-v2"), "{status}");

    let list = stdout(
        maestro(&["feature", "list", "--all"], temp_dir.path()),
        &["feature", "list", "--all"],
    );
    assert!(list.contains("healthy-feature"), "{list}");
    assert!(list.contains("bad-feature\tunreadable"), "{list}");
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
    let bad_dir = temp_dir.path().join(".maestro/features/bad-feature");
    fs::create_dir_all(&bad_dir).expect("invariant: bad feature dir should be creatable");
    fs::write(
        bad_dir.join("feature.yaml"),
        "schema_version: maestro.feature.v1\nid: bad-feature\ntitle: Bad Feature\nstatus: proposed\ncreated_at: \"1\"\nupdated_at: \"1\"\n",
    )
    .expect("invariant: bad feature record should be writable");
    fs::write(
        bad_dir.join("decisions.yaml"),
        "schema_version: maestro.decisions.v0\ndecisions: []\n",
    )
    .expect("invariant: incompatible decision store should be writable");

    let spec = stdout(
        maestro(&["feature", "spec", "bad-feature"], temp_dir.path()),
        &["feature", "spec", "bad-feature"],
    );
    assert!(spec.contains("status: unreadable"), "{spec}");
    assert!(spec.contains("## Raw feature.yaml"), "{spec}");
    assert!(
        spec.contains("schema_version: maestro.feature.v1"),
        "{spec}"
    );

    let decisions = stdout(
        maestro(&["decision", "list"], temp_dir.path()),
        &["decision", "list"],
    );
    assert!(
        decisions.contains("decision-001\topen\tfeature:healthy-feature"),
        "{decisions}"
    );
    assert!(
        decisions.contains("decisions.yaml\tunreadable\tfeature:bad-feature"),
        "{decisions}"
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
    let bad_dir = temp_dir.path().join(".maestro/features/bad-feature");
    fs::create_dir_all(&bad_dir).expect("invariant: bad feature dir should be creatable");
    fs::write(
        bad_dir.join("feature.yaml"),
        "schema_version: maestro.feature.v1\nid: bad-feature\ntitle: Bad Feature\nstatus: proposed\ncreated_at: \"1\"\nupdated_at: \"1\"\n",
    )
    .expect("invariant: bad feature record should be writable");

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
    fs::remove_file(temp_dir.path().join(".maestro/decisions.yaml"))
        .expect("invariant: decisions.yaml should be removable");
    fs::remove_dir(temp_dir.path().join(".maestro/decisions"))
        .expect("invariant: decisions dir should be removable");
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
    assert!(out.contains("opened decision-001 (status: open)"), "{out}");
    assert!(out.contains("feature: agent-cli-ux"), "{out}");

    let lock_args = [
        "decision",
        "lock",
        "decision-001",
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
    assert!(out.contains("locked decision-001"), "{out}");
    assert!(out.contains("note:"), "{out}");

    let record = fs::read_to_string(
        temp_dir
            .path()
            .join(".maestro/features/agent-cli-ux/decisions.yaml"),
    )
    .expect("invariant: structured decision should be readable");
    assert!(record.contains("context: nanosecond epochs are hard to inspect"));
    assert!(record.contains("decision: render RFC3339 UTC with milliseconds"));
    assert!(record.contains("unix seconds: too lossy"));
    assert!(record.contains("raw nanos: unreadable"));
    assert!(record.contains("preview:"));
    assert!(record.contains("status: locked"));
    let notes = fs::read_to_string(
        temp_dir
            .path()
            .join(".maestro/features/agent-cli-ux/notes.md"),
    )
    .expect("invariant: feature notes should be readable");
    assert_dated_note_line(&notes, "decision-001 locked -- Timestamps use RFC3339");

    let spec = stdout(
        maestro(&["feature", "spec", "agent-cli-ux"], temp_dir.path()),
        &["feature", "spec", "agent-cli-ux"],
    );
    assert!(spec.contains("status: proposed"), "{spec}");
    assert!(spec.contains("## Decisions"), "{spec}");
    assert!(
        spec.contains("decision-001 [locked]: Timestamps use RFC3339"),
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
    let second_lock = [
        "decision",
        "lock",
        "decision-002",
        "--decision",
        "render human timestamps in every agent-facing artifact",
        "--rejected",
        "feature-only timestamps: leaves decisions hard to inspect",
        "--supersedes",
        "decision-001",
    ];
    stdout(maestro(&second_lock, temp_dir.path()), &second_lock);
    let record = fs::read_to_string(
        temp_dir
            .path()
            .join(".maestro/features/agent-cli-ux/decisions.yaml"),
    )
    .expect("invariant: structured decision should be readable");
    assert!(record.contains("status: superseded"), "{record}");
    assert!(record.contains("superseded_by: decision-002"), "{record}");
    let notes = fs::read_to_string(
        temp_dir
            .path()
            .join(".maestro/features/agent-cli-ux/notes.md"),
    )
    .expect("invariant: feature notes should be readable");
    assert_dated_note_line(
        &notes,
        "decision-002 locked -- Use human timestamps everywhere",
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
    let feature_notes = fs::read_to_string(
        temp_dir
            .path()
            .join(".maestro/features/billing-csv/notes.md"),
    )
    .expect("invariant: feature notes should be readable");
    assert!(feature_notes.starts_with("# Billing CSV\n\n"));
    assert_dated_note_line(&feature_notes, "locked: export columns");

    stdout(
        maestro(&["task", "create", "Add CSV export"], temp_dir.path()),
        &["task", "create", "Add CSV export"],
    );
    let task_note = stdout(
        maestro(
            &["task", "note", "task-001", "proved: csv opens"],
            temp_dir.path(),
        ),
        &["task", "note", "task-001", "proved: csv opens"],
    );
    assert!(
        task_note.contains("noted task-001 (notes.md created)"),
        "{task_note}"
    );
    let task_dir = fs::read_dir(temp_dir.path().join(".maestro/tasks"))
        .expect("invariant: tasks dir should be listable")
        .find_map(|entry| {
            let entry = entry.expect("invariant: task entry should be readable");
            entry
                .file_name()
                .to_str()
                .filter(|name| name.starts_with("task-001"))
                .map(|_| entry.path())
        })
        .expect("invariant: task-001 dir should exist");
    let task_notes = fs::read_to_string(task_dir.join("notes.md"))
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

    let decisions_dir = temp_dir.path().join(".maestro/decisions");
    let nested_dir = decisions_dir.join("nested");
    fs::create_dir(&nested_dir).expect("invariant: nested decisions dir should be creatable");
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

fn feature_yaml(root: &Path, slug: &str) -> String {
    fs::read_to_string(
        root.join(".maestro")
            .join("features")
            .join(slug)
            .join("feature.yaml"),
    )
    .expect("invariant: feature.yaml should be readable")
}

fn feature_acceptance(root: &Path, slug: &str) -> Vec<String> {
    let yaml: YamlValue =
        serde_yaml::from_str(&feature_yaml(root, slug)).expect("invariant: feature.yaml parses");
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
fn write_baseline(features_dir: &Path, id: &str) {
    let dir = features_dir.join(id);
    ensure_dir(&dir).expect("invariant: feature directory should be creatable");
    fs::write(
        dir.join("qa.md"),
        "---\namend_log_position: 0\n---\n\n### QA Baseline Contract\n\n- Scenario Matrix:\n  - [bl-001] csv export round-trips\n",
    )
    .expect("invariant: qa.md should be writable");
}

/// Write a counting QA slice (scenarios + evidence) covering `[bl-001]`.
fn write_qa_slice(features_dir: &Path, id: &str) {
    let dir = features_dir.join(id);
    ensure_dir(&dir).expect("invariant: feature directory should be creatable");
    let path = dir.join("qa.md");
    let mut contents = fs::read_to_string(&path).unwrap_or_default();
    contents.push_str("\n```yaml\nslices:\n  - scenarios: [\"bl-001\"]\n    evidence: [\"manual: exported csv opens in a spreadsheet\"]\n```\n");
    fs::write(path, contents).expect("invariant: qa.md should be writable");
}

fn write_task(tasks_dir: &Path, id: &str, feature_id: &str, state: &str) {
    let task_dir = tasks_dir
        .parent()
        .expect("invariant: tasks dir should have parent")
        .join("features")
        .join(feature_id)
        .join("tasks")
        .join(format!("{id}-{id}"));
    ensure_dir(&task_dir).expect("invariant: task directory should be creatable");
    // A complete TaskRecord: the cancel cascade loads and transitions the child,
    // so a projection-only stub (id/feature_id/state) fails to deserialize.
    fs::write(
        task_dir.join("task.yaml"),
        format!(
            "schema_version: maestro.task.v2\nid: {id}\ntitle: {id}\nstate: {state}\nacceptance_locked: false\nverification: {{}}\ncreated_at: \"2026-06-06T00:00:00.000Z\"\nupdated_at: \"2026-06-06T00:00:00.000Z\"\n"
        ),
    )
    .expect("invariant: task yaml should be writable");
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
    let features_dir = root.join(".maestro/features");
    write_baseline(&features_dir, slug);
    write_qa_slice(&features_dir, slug);
    stdout(
        maestro(&["feature", "accept", slug], root),
        &["feature", "accept", slug],
    );
    stdout(
        maestro(&["feature", "start", slug], root),
        &["feature", "start", slug],
    );
    write_task(&root.join(".maestro/tasks"), child, slug, "verified");
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
    let archived = temp_dir.path().join(".maestro/archive/features/csv-export");
    ensure_dir(&archived).expect("invariant: archive feature dir should be creatable");
    fs::write(
        archived.join("feature.yaml"),
        "schema_version: maestro.feature.v1\nid: csv-export\ntitle: CSV Export\nstatus: shipped\ncreated_at: \"1\"\nupdated_at: \"1\"\n",
    )
    .expect("invariant: archived feature yaml should be writable");

    let args = ["feature", "new", "CSV Export"];
    let stderr = assert_failure(maestro(&args, temp_dir.path()), &args);
    assert!(stderr.contains("csv-export"));
    assert!(stderr.contains("archive"));
}

/// Drive a feature to Shipped, then archive it: the feature dir + its terminal
/// child tasks leave the live scan, the QA artifacts travel inside the archived
/// dir, reads fall through (L6b), and unarchive round-trips (§5.9).
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

    let features_dir = root.join(".maestro/features");
    write_baseline(&features_dir, "billing-csv-export");
    write_qa_slice(&features_dir, "billing-csv-export");
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

    let tasks_dir = root.join(".maestro/tasks");
    write_task(&tasks_dir, "task-001", "billing-csv-export", "verified");
    write_task(&tasks_dir, "task-002", "billing-csv-export", "verified");
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

    // The feature dir + QA artifacts moved into the archive sibling tree.
    let archived_feature = root.join(".maestro/archive/features/billing-csv-export");
    assert!(archived_feature.join("feature.yaml").is_file());
    assert!(archived_feature.join("qa.md").is_file());
    assert!(!features_dir.join("billing-csv-export").exists());
    assert!(archived_feature.join("tasks/task-001-task-001").is_dir());
    assert!(archived_feature.join("tasks/task-002-task-002").is_dir());
    assert!(
        !features_dir
            .join("billing-csv-export")
            .join("tasks/task-001-task-001")
            .exists()
    );

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

    // Unarchive restores the feature dir + its archived children.
    let unarchive_args = ["feature", "unarchive", "billing-csv-export"];
    let restored = stdout(maestro(&unarchive_args, root), &unarchive_args);
    assert!(restored.contains("unarchived feature billing-csv-export"));
    assert!(restored.contains("task-001"));
    assert!(restored.contains("restore receipt:"));
    assert!(restored.contains("next: maestro status"));
    assert!(
        features_dir
            .join("billing-csv-export")
            .join("qa.md")
            .is_file()
    );
    assert!(
        features_dir
            .join("billing-csv-export")
            .join("tasks/task-001-task-001")
            .is_dir()
    );
    assert!(!archived_feature.exists());
}

/// The nested archive case: terminal child tasks move with the feature directory.
#[test]
fn feature_archive_moves_nested_child_tasks_with_feature_dir() {
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
    write_baseline(&root.join(".maestro/features"), "billing-csv-export");
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
    let tasks_dir = root.join(".maestro/tasks");
    write_task(&tasks_dir, "task-001", "billing-csv-export", "in_progress");
    write_task(&tasks_dir, "task-002", "billing-csv-export", "in_progress");
    write_task(&tasks_dir, "task-003", "billing-csv-export", "in_progress");
    let cancel_args = [
        "feature",
        "cancel",
        "billing-csv-export",
        "--reason",
        "scope dropped",
    ];
    let cancelled = stdout(maestro(&cancel_args, root), &cancel_args);
    assert!(cancelled.contains("cancel receipt:"));
    assert!(cancelled.contains("next: maestro status"));

    // A live task (task-004) blocked by the terminal child task-002 entangles it.
    stdout(
        maestro(&["task", "create", "Holder"], root),
        &["task", "create", "Holder"],
    );
    let block_args = [
        "task", "block", "task-004", "--reason", "needs 2", "--by", "task-002",
    ];
    stdout(maestro(&block_args, root), &block_args);

    // Archive moves the feature directory and every nested terminal child task.
    let archive_args = ["feature", "archive", "billing-csv-export"];
    let first = stdout(maestro(&archive_args, root), &archive_args);
    assert!(first.contains("archived feature billing-csv-export"));
    assert!(first.contains("task-001"));
    assert!(first.contains("task-002"));
    assert!(first.contains("task-003"));
    assert!(
        root.join(".maestro/archive/features/billing-csv-export/feature.yaml")
            .is_file()
    );
    assert!(
        root.join(".maestro/archive/features/billing-csv-export/tasks/task-001-task-001")
            .is_dir()
    );
    assert!(
        root.join(".maestro/archive/features/billing-csv-export/tasks/task-002-task-002")
            .is_dir()
    );
    assert!(
        root.join(".maestro/archive/features/billing-csv-export/tasks/task-003-task-003")
            .is_dir()
    );

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

/// `feature archive --shipped` archives every shipped feature (each cascading its
/// children) and leaves non-shipped features in the live tree.
#[test]
fn feature_archive_shipped_sweeps_only_shipped_features() {
    let temp_dir = TestTempDir::new("maestro-feature-archive-bulk");
    let root = temp_dir.path();
    init_git_marker(root);
    stdout(maestro(&["init", "--yes"], root), &["init", "--yes"]);

    // Two shipped features (each with a verified child) + one still in progress.
    ship_feature(root, "Alpha export", "alpha-export", "task-001");
    ship_feature(root, "Beta export", "beta-export", "task-002");

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
    write_baseline(&root.join(".maestro/features"), "gamma-export");
    stdout(
        maestro(&["feature", "accept", "gamma-export"], root),
        &["feature", "accept", "gamma-export"],
    );
    stdout(
        maestro(&["feature", "start", "gamma-export"], root),
        &["feature", "start", "gamma-export"],
    );

    // --shipped archives both shipped features and their children; gamma stays live.
    let bulk = ["feature", "archive", "--shipped"];
    let out = stdout(maestro(&bulk, root), &bulk);
    assert!(out.contains("archived shipped features"));
    assert!(out.contains("archive summary:"));
    assert!(out.contains("features: 2 archived"));
    assert!(out.contains("child tasks: 2 archived"));
    assert!(out.contains("next: maestro status"));

    let features_dir = root.join(".maestro/features");
    assert!(root.join(".maestro/archive/features/alpha-export").is_dir());
    assert!(root.join(".maestro/archive/features/beta-export").is_dir());
    assert!(
        root.join(".maestro/archive/features/alpha-export/tasks/task-001-task-001")
            .is_dir()
    );
    assert!(
        root.join(".maestro/archive/features/beta-export/tasks/task-002-task-002")
            .is_dir()
    );
    // The in-progress feature is untouched and stays in the live tree.
    assert!(
        features_dir
            .join("gamma-export")
            .join("feature.yaml")
            .is_file()
    );
    assert!(!root.join(".maestro/archive/features/gamma-export").exists());

    // Idempotent: no shipped features remain live.
    let again = stdout(maestro(&bulk, root), &bulk);
    assert!(again.contains("no shipped features to archive"));

    // A feature id and --shipped are mutually exclusive.
    let both = ["feature", "archive", "alpha-export", "--shipped"];
    let err = assert_failure(maestro(&both, root), &both);
    assert!(err.contains("not both"));

    // Neither an id nor --shipped: the remedy must not claim "not both".
    let neither = ["feature", "archive"];
    let err = assert_failure(maestro(&neither, root), &neither);
    assert!(err.contains("provide a feature id or --shipped"));
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
