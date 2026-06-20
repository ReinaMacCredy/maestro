//! Implicit ship: a `feature verify --prove` that completes ship-readiness folds
//! the full ship gate (evidence + suite + terminal close) into the same call.
//! Covers the locked decisions: fully-automatic trigger, `--no-ship` suppressor,
//! the one-AC-left nudge, the write-once outcome default, gate-fail safety, and
//! the trigger confinement (a non-`--prove` verify must not auto-ship).

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

/// A started feature with two explicit acceptance items (proven via `--prove`) and
/// a QA baseline whose lone scenario is slice-covered, so the only thing standing
/// between the feature and ship is proving its acceptance contract.
fn started_feature_two_acceptances(repo: &Path, id: &str) {
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
        "first behavior",
        "--acceptance",
        "second behavior",
        "--area",
        "reports",
    ];
    stdout(maestro(&set, repo), &set);
    let feature_dir = repo.join(".maestro/cards").join(id);
    fs::write(
        feature_dir.join("qa.md"),
        "---\namend_log_position: 0\n---\n\n### QA Baseline Contract\n\n- Scenario Matrix:\n  - [bl-001] baseline scenario\n",
    )
    .expect("invariant: qa.md should be writable");
    stdout(
        maestro(&["feature", "accept", id], repo),
        &["feature", "accept"],
    );
    stdout(
        maestro(&["feature", "start", id], repo),
        &["feature", "start"],
    );
    let mut qa = fs::read_to_string(feature_dir.join("qa.md")).expect("invariant: qa.md readable");
    qa.push_str("\n```yaml\nslices:\n  - scenarios: [\"bl-001\"]\n    evidence: [\"proof for bl-001\"]\n```\n");
    fs::write(feature_dir.join("qa.md"), qa).expect("invariant: qa.md should be writable");
}

fn prove(repo: &Path, id: &str, ac: &str, extra: &[&str]) -> std::process::Output {
    let mut args = vec![
        "feature",
        "verify",
        id,
        "--prove",
        ac,
        "--evidence",
        "observed",
    ];
    args.extend_from_slice(extra);
    maestro(&args, repo)
}

/// bl-001: proving the last acceptance item auto-runs the full gate + terminal
/// close in the same call, with no separate `feature ship`.
#[test]
fn last_prove_auto_ships_in_the_same_call() {
    let temp = TestTempDir::new("maestro-autoship-last-prove");
    let repo = temp.path();
    started_feature_two_acceptances(repo, "report-builder");
    write_stack_verify(repo, "true");

    // First proof: not yet shippable.
    let first = prove(repo, "report-builder", "ac-1", &[]);
    assert!(first.status.success());

    // Second (last) proof: completes readiness -> auto-ship.
    let last = prove(repo, "report-builder", "ac-2", &[]);
    let out = stdout(last, &["feature", "verify", "--prove", "ac-2"]);
    assert!(
        out.contains("auto-shipping"),
        "should announce auto-ship: {out}"
    );
    assert!(
        out.contains("full verify suite passed"),
        "the full suite ran: {out}"
    );
    assert!(out.contains("ship receipt"), "ship receipt printed: {out}");

    let show = stdout(
        maestro(&["feature", "show", "report-builder"], repo),
        &["feature", "show"],
    );
    assert!(show.contains("shipped"), "feature must be shipped: {show}");
}

/// bl-005: an auto-ship with no `--outcome` records a generated AC-proof summary;
/// `--outcome` on the triggering verify is recorded verbatim.
#[test]
fn auto_ship_records_default_outcome() {
    let temp = TestTempDir::new("maestro-autoship-default-outcome");
    let repo = temp.path();
    started_feature_two_acceptances(repo, "report-builder");
    write_stack_verify(repo, "true");

    stdout(
        prove(repo, "report-builder", "ac-1", &[]),
        &["prove", "ac-1"],
    );
    stdout(
        prove(repo, "report-builder", "ac-2", &[]),
        &["prove", "ac-2"],
    );

    let show = stdout(
        maestro(&["feature", "show", "report-builder"], repo),
        &["feature", "show"],
    );
    assert!(
        show.contains("acceptance proven"),
        "default outcome is a generated AC-proof summary: {show}"
    );
}

#[test]
fn auto_ship_uses_explicit_outcome_override() {
    let temp = TestTempDir::new("maestro-autoship-outcome-override");
    let repo = temp.path();
    started_feature_two_acceptances(repo, "report-builder");
    write_stack_verify(repo, "true");

    stdout(
        prove(repo, "report-builder", "ac-1", &[]),
        &["prove", "ac-1"],
    );
    stdout(
        prove(
            repo,
            "report-builder",
            "ac-2",
            &["--outcome", "shipped the report builder"],
        ),
        &["prove", "ac-2", "--outcome"],
    );

    let show = stdout(
        maestro(&["feature", "show", "report-builder"], repo),
        &["feature", "show"],
    );
    assert!(
        show.contains("shipped the report builder"),
        "explicit --outcome recorded verbatim: {show}"
    );
}

/// bl-003: `--no-ship` records the proof but defers the auto-fire; the feature
/// stays in_progress and ships later via explicit `feature ship`.
#[test]
fn no_ship_defers_the_auto_fire() {
    let temp = TestTempDir::new("maestro-autoship-no-ship");
    let repo = temp.path();
    started_feature_two_acceptances(repo, "report-builder");
    write_stack_verify(repo, "true");

    stdout(
        prove(repo, "report-builder", "ac-1", &[]),
        &["prove", "ac-1"],
    );
    let deferred = stdout(
        prove(repo, "report-builder", "ac-2", &["--no-ship"]),
        &["prove", "ac-2", "--no-ship"],
    );
    assert!(
        deferred.contains("auto-ship deferred"),
        "--no-ship must defer: {deferred}"
    );

    let show = stdout(
        maestro(&["feature", "show", "report-builder"], repo),
        &["feature", "show"],
    );
    assert!(
        show.contains("in_progress"),
        "deferred feature stays in_progress: {show}"
    );

    // Explicit ship still closes it.
    let shipped = stdout(
        maestro(
            &["feature", "ship", "report-builder", "--outcome", "done"],
            repo,
        ),
        &["feature", "ship"],
    );
    assert!(shipped.contains("shipped report-builder"), "{shipped}");
}

/// bl-004: when exactly one acceptance item is left, an advisory STDERR nudge
/// warns the next `--prove` will auto-ship; the command itself does not block.
#[test]
fn one_acceptance_left_nudges_on_stderr() {
    let temp = TestTempDir::new("maestro-autoship-nudge");
    let repo = temp.path();
    started_feature_two_acceptances(repo, "report-builder");
    write_stack_verify(repo, "true");

    let first = prove(repo, "report-builder", "ac-1", &[]);
    assert!(
        first.status.success(),
        "the nudge must not block the command"
    );
    let stderr = String::from_utf8(first.stderr).expect("stderr utf8");
    assert!(
        stderr.contains("1 acceptance item left") && stderr.contains("auto-ship"),
        "STDERR nudge expected: {stderr}"
    );
    assert!(
        stderr.contains("--no-ship"),
        "nudge points at the suppressor: {stderr}"
    );

    let show = stdout(
        maestro(&["feature", "show", "report-builder"], repo),
        &["feature", "show"],
    );
    assert!(show.contains("in_progress"), "still in_progress: {show}");
}

/// bl-006: when the auto-fired suite fails, the proof is kept, the feature stays
/// in_progress, the suite output is surfaced, and the command exits non-zero.
#[test]
fn auto_ship_suite_failure_keeps_proof_and_stays_in_progress() {
    let temp = TestTempDir::new("maestro-autoship-suite-fail");
    let repo = temp.path();
    started_feature_two_acceptances(repo, "report-builder");
    write_stack_verify(repo, "false");

    stdout(
        prove(repo, "report-builder", "ac-1", &[]),
        &["prove", "ac-1"],
    );
    let last = prove(repo, "report-builder", "ac-2", &[]);
    assert!(
        !last.status.success(),
        "a failing auto-fired suite must exit non-zero"
    );
    let stderr = String::from_utf8(last.stderr).expect("stderr utf8");
    assert!(
        stderr.contains("full verify suite failed"),
        "suite failure surfaced: {stderr}"
    );

    let show = stdout(
        maestro(&["feature", "show", "report-builder"], repo),
        &["feature", "show"],
    );
    assert!(
        show.contains("in_progress"),
        "a failed auto-ship must not flip the feature: {show}"
    );
    // The proof was kept: a bare verify shows both acceptance items resolved.
    let sweep = stdout(
        maestro(&["feature", "verify", "report-builder"], repo),
        &["feature", "verify"],
    );
    assert!(
        sweep.contains("every acceptance item has evidence"),
        "the recorded proof survives the failed auto-ship: {sweep}"
    );
}

/// A `--waive` that completes ship-readiness also auto-ships: the trigger is the
/// recorded-update branch (prove OR waive), consistent with the fully-automatic
/// decision. D4 confines the trigger to `feature verify` (never task verify); it
/// does not single out `--prove` over `--waive` within a feature verify.
#[test]
fn waive_completing_readiness_also_auto_ships() {
    let temp = TestTempDir::new("maestro-autoship-waive");
    let repo = temp.path();
    started_feature_two_acceptances(repo, "report-builder");
    write_stack_verify(repo, "true");

    stdout(
        prove(repo, "report-builder", "ac-1", &[]),
        &["prove", "ac-1"],
    );
    // Waiving the last unresolved acceptance item completes readiness -> auto-ship.
    let last = maestro(
        &[
            "feature",
            "verify",
            "report-builder",
            "--waive",
            "ac-2",
            "--reason",
            "not applicable for this slice",
        ],
        repo,
    );
    let out = stdout(last, &["feature", "verify", "--waive", "ac-2"]);
    assert!(
        out.contains("auto-shipping"),
        "a completing waive auto-ships: {out}"
    );

    let show = stdout(
        maestro(&["feature", "show", "report-builder"], repo),
        &["feature", "show"],
    );
    assert!(show.contains("shipped"), "feature must be shipped: {show}");
}

/// bl-002 confinement: a non-`--prove` `feature verify` (the contract sweep) that
/// completes ship-readiness must NOT auto-ship; only `--prove` fires the gate.
#[test]
fn bare_sweep_verify_does_not_auto_ship() {
    let temp = TestTempDir::new("maestro-autoship-confinement");
    let repo = temp.path();
    started_feature_two_acceptances(repo, "report-builder");
    write_stack_verify(repo, "true");

    // Prove both acceptance items with --no-ship so readiness is reached without
    // ever auto-shipping, leaving the gate clear for a bare sweep to evaluate.
    stdout(
        prove(repo, "report-builder", "ac-1", &["--no-ship"]),
        &["prove", "ac-1", "--no-ship"],
    );
    stdout(
        prove(repo, "report-builder", "ac-2", &["--no-ship"]),
        &["prove", "ac-2", "--no-ship"],
    );

    // A bare `feature verify` (sweep, no --prove) hits the ready gate but must not ship.
    let sweep = stdout(
        maestro(&["feature", "verify", "report-builder"], repo),
        &["feature", "verify"],
    );
    assert!(
        sweep.contains("every acceptance item has evidence"),
        "the sweep sees a clear contract: {sweep}"
    );

    let show = stdout(
        maestro(&["feature", "show", "report-builder"], repo),
        &["feature", "show"],
    );
    assert!(
        show.contains("in_progress"),
        "a bare sweep verify must not auto-ship: {show}"
    );
}
