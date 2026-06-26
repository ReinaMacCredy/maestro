//! decision-002 pairing: the full repo-global `stack.verify` suite runs at
//! `feature close` (real close only), backstopping the per-task narrow falsifier.
//! Proves the operations close coordinator runs the suite, blocks on failure, and
//! leaves read-only paths (`--dry-run`) free of suite execution.

mod support;

use std::fs;
use std::path::Path;
use std::process::Command;

use support::TestTempDir;

fn maestro(args: &[&str], cwd: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("invariant: compiled maestro binary should be runnable in integration tests")
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

fn write_stack_verify(repo: &Path, command: &str) {
    fs::write(
        repo.join(".maestro/harness/harness.yml"),
        format!(
            "schema_version: maestro.harness.v1\nstack:\n  kind: generic\n  detected_by: []\n  verify:\n  - '{}'\n",
            command.replace('\'', "''")
        ),
    )
    .expect("invariant: harness.yml should be writable");
}

/// Drive a feature to a state where every evidence gate (live tasks / QA /
/// acceptance sweep) is clear, so only the full-suite backstop is left to decide.
fn closable_feature(repo: &Path, id: &str) {
    fs::create_dir(repo.join(".git")).expect("invariant: .git marker should be creatable");
    stdout(maestro(&["init", "--yes"], repo), &["init"]);
    stdout(
        maestro(&["feature", "new", "Report builder"], repo),
        &["feature", "new"],
    );
    let set = [
        "feature",
        "set",
        id,
        "--acceptance",
        "behaves",
        "--area",
        "reports",
    ];
    stdout(maestro(&set, repo), &set);
    let feature_dir = repo.join(".maestro/cards").join(id);
    fs::write(
        feature_dir.join("qa.md"),
        "---\namend_log_position: 0\n---\n\n### QA Baseline Contract\n\n- Scenario Matrix:\n  - [bl-001] scenario bl-001 (covers: ac-1)\n",
    )
    .expect("invariant: qa.md should be writable");
    stdout(
        maestro(&["feature", "finalize", id], repo),
        &["feature", "finalize"],
    );
    stdout(
        maestro(&["feature", "accept", id], repo),
        &["feature", "accept"],
    );
    stdout(
        maestro(&["feature", "start", id], repo),
        &["feature", "start"],
    );
    // Cover the baseline scenario with a counting slice.
    let mut qa = fs::read_to_string(feature_dir.join("qa.md")).expect("invariant: qa.md readable");
    qa.push_str("\n```yaml\nslices:\n  - scenarios: [\"bl-001\"]\n    evidence: [\"proof for bl-001\"]\n```\n");
    fs::write(feature_dir.join("qa.md"), qa).expect("invariant: qa.md should be writable");
    // Resolve the acceptance contract sweep.
    stdout(
        maestro(&["feature", "verify", id], repo),
        &["feature", "verify"],
    );
}

#[test]
fn feature_close_blocks_when_the_full_suite_fails() {
    let temp = TestTempDir::new("maestro-close-suite-fail");
    let repo = temp.path();
    closable_feature(repo, "report-builder");
    write_stack_verify(repo, "false");

    let close = ["feature", "close", "report-builder", "--outcome", "done"];
    let stderr = assert_failure(maestro(&close, repo), &close);
    assert!(
        stderr.contains("full verify suite failed"),
        "close must block on a failing suite: {stderr}"
    );
    assert!(
        stderr.contains("false (exit"),
        "the failing command is named: {stderr}"
    );

    // The feature did NOT transition; it stays in_progress.
    let show = stdout(
        maestro(&["feature", "show", "report-builder"], repo),
        &["feature", "show"],
    );
    assert!(
        show.contains("in_progress"),
        "a blocked close must not flip the feature: {show}"
    );
}

#[test]
fn feature_close_succeeds_when_the_full_suite_passes() {
    let temp = TestTempDir::new("maestro-close-suite-pass");
    let repo = temp.path();
    closable_feature(repo, "report-builder");
    write_stack_verify(repo, "true");

    let close = ["feature", "close", "report-builder", "--outcome", "done"];
    let closed = stdout(maestro(&close, repo), &close);
    assert!(closed.contains("closed report-builder"), "{closed}");
    assert!(closed.contains("full verify suite passed"), "{closed}");
}

#[test]
fn feature_close_dry_run_does_not_execute_the_suite() {
    let temp = TestTempDir::new("maestro-close-suite-dryrun");
    let repo = temp.path();
    closable_feature(repo, "report-builder");
    // A suite that would FAIL if run; dry-run must still preview cleanly.
    write_stack_verify(repo, "false");

    let dry = ["feature", "close", "report-builder", "--dry-run"];
    let preview = stdout(maestro(&dry, repo), &dry);
    assert!(
        preview.contains("would close"),
        "dry-run must preview without running the suite: {preview}"
    );
    assert!(
        preview.contains("full verify suite would run"),
        "dry-run should state the suite would run on a real close: {preview}"
    );
    assert!(
        !preview.contains("full verify suite failed"),
        "dry-run must not execute the suite: {preview}"
    );

    // Still in_progress: a preview writes nothing.
    let show = stdout(
        maestro(&["feature", "show", "report-builder"], repo),
        &["feature", "show"],
    );
    assert!(show.contains("in_progress"), "{show}");
}
