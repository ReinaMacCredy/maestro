//! Implicit close: a `feature verify --prove` that completes close-readiness folds
//! the full close gate (evidence + suite + terminal close) into the same call.
//! Covers the locked decisions: fully-automatic trigger, `--no-close` suppressor,
//! the one-AC-left nudge, the write-once outcome default, gate-fail safety, and
//! the trigger confinement (a non-`--prove` verify must not auto-close).

mod support;
mod witness_support;

use std::fs;
use std::path::Path;
use std::process::Command;

use maestro::domain::feature;
use maestro::foundation::core::paths::MaestroPaths;
use support::TestTempDir;
use witness_support::write_valid_witness;

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
/// between the feature and close is proving its acceptance contract.
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
    let paths = MaestroPaths::new(repo);
    feature::write_sidecar_text(
        &paths,
        id,
        "qa.md",
        "---\namend_log_position: 0\n---\n\n### QA Baseline Contract\n\n- Scenario Matrix:\n  - [bl-001] baseline scenario\n",
    )
    .expect("invariant: qa.md should be writable");
    stdout(
        maestro(&["feature", "reconcile", id], repo),
        &["feature", "reconcile"],
    );
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
    let mut qa = feature::read_sidecar_text(&paths, id, "qa.md")
        .expect("invariant: qa.md readable")
        .expect("invariant: qa.md should exist");
    qa.push_str("\n```yaml\nslices:\n  - scenarios: [\"bl-001\"]\n    evidence: [\"proof for bl-001\"]\n```\n");
    feature::write_sidecar_text(&paths, id, "qa.md", &qa)
        .expect("invariant: qa.md should be writable");
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

fn write_t0_skip_witness(repo: &Path, id: &str) {
    let witness = "# Witness Brief\n\
skipped: true\n\
tier: T0\n\
skipped_by: user\n\
user_authorization_ref: test:user-authorization\n\
skip_reason: fixture has no independent review surface\n\
changed_surface: none\n";
    feature::write_sidecar_text(&MaestroPaths::new(repo), id, "witness.md", witness)
        .expect("invariant: witness.md should be writable");
}

/// bl-001: proving the last acceptance item keeps the proof, but witness/advisor
/// receipts now block the terminal close until the independent review exists.
#[test]
fn last_prove_reports_missing_witness_instead_of_auto_closing() {
    let temp = TestTempDir::new("maestro-autoclose-last-prove");
    let repo = temp.path();
    started_feature_two_acceptances(repo, "report-builder");
    write_stack_verify(repo, "true");

    // First proof: not yet closable.
    let first = prove(repo, "report-builder", "ac-1", &[]);
    assert!(first.status.success());

    // Second (last) proof completes proof readiness, but close now needs witness.
    let last = prove(repo, "report-builder", "ac-2", &[]);
    let out = stdout(last, &["feature", "verify", "--prove", "ac-2"]);
    assert!(
        out.contains("not yet closable") && out.contains("witness.md missing"),
        "last proof should preserve proof and name the witness gate: {out}"
    );

    let show = stdout(
        maestro(&["feature", "show", "report-builder"], repo),
        &["feature", "show"],
    );
    assert!(show.contains("in_progress"), "feature stays open: {show}");
}

/// bl-005: when an explicit T0 user-authorized skip receipt lets auto-close run,
/// no `--outcome` records a generated AC-proof summary.
#[test]
fn auto_close_records_default_outcome_with_t0_skip() {
    let temp = TestTempDir::new("maestro-autoclose-default-outcome");
    let repo = temp.path();
    started_feature_two_acceptances(repo, "report-builder");
    write_stack_verify(repo, "true");
    write_t0_skip_witness(repo, "report-builder");

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
fn auto_close_uses_explicit_outcome_override_with_t0_skip() {
    let temp = TestTempDir::new("maestro-autoclose-outcome-override");
    let repo = temp.path();
    started_feature_two_acceptances(repo, "report-builder");
    write_stack_verify(repo, "true");
    write_t0_skip_witness(repo, "report-builder");

    stdout(
        prove(repo, "report-builder", "ac-1", &[]),
        &["prove", "ac-1"],
    );
    stdout(
        prove(
            repo,
            "report-builder",
            "ac-2",
            &["--outcome", "closed the report builder"],
        ),
        &["prove", "ac-2", "--outcome"],
    );

    let show = stdout(
        maestro(&["feature", "show", "report-builder"], repo),
        &["feature", "show"],
    );
    assert!(
        show.contains("closed the report builder"),
        "explicit --outcome recorded verbatim: {show}"
    );
}

/// bl-003: `--no-close` records the proof but defers the auto-fire; the feature
/// stays in_progress and closes later via explicit `feature close`.
#[test]
fn no_close_defers_the_auto_fire() {
    let temp = TestTempDir::new("maestro-autoclose-no-close");
    let repo = temp.path();
    started_feature_two_acceptances(repo, "report-builder");
    write_stack_verify(repo, "true");

    stdout(
        prove(repo, "report-builder", "ac-1", &[]),
        &["prove", "ac-1"],
    );
    let deferred = stdout(
        prove(repo, "report-builder", "ac-2", &["--no-close"]),
        &["prove", "ac-2", "--no-close"],
    );
    assert!(
        deferred.contains("auto-close deferred"),
        "--no-close must defer: {deferred}"
    );

    let show = stdout(
        maestro(&["feature", "show", "report-builder"], repo),
        &["feature", "show"],
    );
    assert!(
        show.contains("in_progress"),
        "deferred feature stays in_progress: {show}"
    );

    // Explicit close still closes it once the post-proof witness exists.
    write_valid_witness(&MaestroPaths::new(repo), "report-builder");
    let closed = stdout(
        maestro(
            &["feature", "close", "report-builder", "--outcome", "done"],
            repo,
        ),
        &["feature", "close"],
    );
    assert!(closed.contains("closed report-builder"), "{closed}");
}

/// bl-004: when exactly one acceptance item is left, an advisory STDERR nudge
/// warns the next `--prove` will auto-close; the command itself does not block.
#[test]
fn one_acceptance_left_nudges_on_stderr() {
    let temp = TestTempDir::new("maestro-autoclose-nudge");
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
        stderr.contains("1 acceptance item left") && stderr.contains("auto-close"),
        "STDERR nudge expected: {stderr}"
    );
    assert!(
        stderr.contains("--no-close"),
        "nudge points at the suppressor: {stderr}"
    );

    let show = stdout(
        maestro(&["feature", "show", "report-builder"], repo),
        &["feature", "show"],
    );
    assert!(show.contains("in_progress"), "still in_progress: {show}");
}

/// bl-006: when auto-close is authorized by a T0 skip but the suite fails, the
/// proof is kept, the feature stays in_progress, and the command exits non-zero.
#[test]
fn auto_close_suite_failure_keeps_proof_and_stays_in_progress_with_t0_skip() {
    let temp = TestTempDir::new("maestro-autoclose-suite-fail");
    let repo = temp.path();
    started_feature_two_acceptances(repo, "report-builder");
    write_stack_verify(repo, "false");
    write_t0_skip_witness(repo, "report-builder");

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
        "a failed auto-close must not flip the feature: {show}"
    );
    // The proof was kept: a bare verify shows both acceptance items resolved.
    let sweep = stdout(
        maestro(&["feature", "verify", "report-builder"], repo),
        &["feature", "verify"],
    );
    assert!(
        sweep.contains("every acceptance item has evidence"),
        "the recorded proof survives the failed auto-close: {sweep}"
    );
}

/// A `--waive` that completes close-readiness also auto-closes when a T0 skip
/// receipt has explicit user authorization.
#[test]
fn waive_completing_readiness_auto_closes_with_t0_skip() {
    let temp = TestTempDir::new("maestro-autoclose-waive");
    let repo = temp.path();
    started_feature_two_acceptances(repo, "report-builder");
    write_stack_verify(repo, "true");
    write_t0_skip_witness(repo, "report-builder");

    stdout(
        prove(repo, "report-builder", "ac-1", &[]),
        &["prove", "ac-1"],
    );
    // Waiving the last unresolved acceptance item completes readiness -> auto-close.
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
        out.contains("auto-closing"),
        "a completing waive auto-closes: {out}"
    );

    let show = stdout(
        maestro(&["feature", "show", "report-builder"], repo),
        &["feature", "show"],
    );
    assert!(show.contains("closed"), "feature must be closed: {show}");
}

/// bl-002 confinement: a non-`--prove` `feature verify` (the contract sweep) that
/// completes close-readiness must NOT auto-close; only `--prove` fires the gate.
#[test]
fn bare_sweep_verify_does_not_auto_close() {
    let temp = TestTempDir::new("maestro-autoclose-confinement");
    let repo = temp.path();
    started_feature_two_acceptances(repo, "report-builder");
    write_stack_verify(repo, "true");

    // Prove both acceptance items with --no-close so readiness is reached without
    // ever auto-closing, leaving the gate clear for a bare sweep to evaluate.
    stdout(
        prove(repo, "report-builder", "ac-1", &["--no-close"]),
        &["prove", "ac-1", "--no-close"],
    );
    stdout(
        prove(repo, "report-builder", "ac-2", &["--no-close"]),
        &["prove", "ac-2", "--no-close"],
    );

    // A bare `feature verify` (sweep, no --prove) hits the ready gate but must not close.
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
        "a bare sweep verify must not auto-close: {show}"
    );
}
