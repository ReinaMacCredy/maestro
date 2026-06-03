//! End-to-end QA gate wiring (§4): accept reads `baseline.md`, ship reads
//! `qa-slices.yaml` and the real `amend-log.yaml` that `feature amend` writes.
//! The pure gate predicates are unit-tested in `domain::feature::qa`; this file
//! proves the CLI actually consults the on-disk artifacts.

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

fn init_and_author(repo: &Path, id: &str, title: &str) {
    fs::create_dir(repo.join(".git")).expect("invariant: .git marker should be creatable");
    stdout(maestro(&["init", "--yes"], repo), &["init"]);
    stdout(
        maestro(&["feature", "new", title], repo),
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
}

fn feature_dir(repo: &Path, id: &str) -> std::path::PathBuf {
    repo.join(".maestro/features").join(id)
}

fn write_baseline(repo: &Path, id: &str, position: usize, scenario_ids: &[&str]) {
    let scenarios = scenario_ids
        .iter()
        .map(|id| format!("  - [{id}] scenario {id}\n"))
        .collect::<String>();
    fs::write(
        feature_dir(repo, id).join("baseline.md"),
        format!("---\namend_log_position: {position}\n---\n\n### QA Baseline Contract\n\n- Scenario Matrix:\n{scenarios}"),
    )
    .expect("invariant: baseline.md should be writable");
}

fn write_qa_slices(repo: &Path, id: &str, covered: &[&str]) {
    let slices = covered
        .iter()
        .map(|id| format!("  - scenarios: [\"{id}\"]\n    evidence: [\"proof for {id}\"]\n"))
        .collect::<String>();
    fs::write(
        feature_dir(repo, id).join("qa-slices.yaml"),
        format!("slices:\n{slices}"),
    )
    .expect("invariant: qa-slices.yaml should be writable");
}

#[test]
fn feature_qa_gates_via_cli() {
    let temp = TestTempDir::new("maestro-qa-gate-test");
    let repo = temp.path();
    init_and_author(repo, "report-builder", "Report builder");

    // F — accept blocks until a baseline is captured (before edits).
    let accept = ["feature", "accept", "report-builder"];
    let stderr = assert_failure(maestro(&accept, repo), &accept);
    assert!(
        stderr.contains("qa-baseline"),
        "accept should name the missing baseline: {stderr}"
    );
    assert!(stderr.contains("skill: qa-baseline"), "{stderr}");
    assert!(
        stderr.contains("target: .maestro/features/report-builder/baseline.md"),
        "{stderr}"
    );
    assert!(
        stderr.contains("retry: maestro feature accept report-builder"),
        "{stderr}"
    );

    write_baseline(repo, "report-builder", 0, &["bl-001"]);
    let accepted = stdout(maestro(&accept, repo), &accept);
    assert!(accepted.contains("accepted report-builder"));
    stdout(
        maestro(&["feature", "start", "report-builder"], repo),
        &["feature", "start"],
    );

    // Coverage — ship blocks while [bl-001] has no counting slice.
    let ship = ["feature", "ship", "report-builder"];
    let stderr = assert_failure(maestro(&ship, repo), &ship);
    assert!(
        stderr.contains("bl-001"),
        "ship should name the uncovered scenario: {stderr}"
    );
    assert!(stderr.contains("coverage incomplete"));
    assert!(stderr.contains("skill: qa-slice"), "{stderr}");
    assert!(
        stderr.contains("target: .maestro/features/report-builder/qa-slices.yaml"),
        "{stderr}"
    );
    assert!(
        stderr.contains("retry: maestro feature ship report-builder --outcome \"<outcome>\""),
        "{stderr}"
    );

    // D count rule through the real YAML parse path: a slice that references the
    // scenario but omits `evidence` (serde default → empty) does not count.
    fs::write(
        feature_dir(repo, "report-builder").join("qa-slices.yaml"),
        "slices:\n  - scenarios: [\"bl-001\"]\n",
    )
    .expect("invariant: qa-slices.yaml should be writable");
    let stderr = assert_failure(maestro(&ship, repo), &ship);
    assert!(
        stderr.contains("bl-001"),
        "an evidence-less slice must not count: {stderr}"
    );

    write_qa_slices(repo, "report-builder", &["bl-001"]);
    let dry = ["feature", "ship", "report-builder", "--dry-run"];
    let preview = stdout(maestro(&dry, repo), &dry);
    assert!(
        preview.contains("would ship"),
        "dry-run should pass once covered: {preview}"
    );

    // E freshness — a behavioral amend (new area) staleness-blocks ship; the gate
    // reads the amend-log.yaml that `feature amend` actually wrote.
    let amend = [
        "feature",
        "amend",
        "report-builder",
        "--add-area",
        "exports",
        "--reason",
        "scope grew",
    ];
    stdout(maestro(&amend, repo), &amend);
    let stderr = assert_failure(maestro(&ship, repo), &ship);
    assert!(
        stderr.contains("stale"),
        "behavioral amend should stale the baseline: {stderr}"
    );
    assert!(stderr.contains("skill: qa-baseline"), "{stderr}");

    // Refresh the baseline past the amend and add the new scenario; coverage now
    // demands a slice for [bl-002].
    write_baseline(repo, "report-builder", 1, &["bl-001", "bl-002"]);
    let stderr = assert_failure(maestro(&ship, repo), &ship);
    assert!(
        stderr.contains("bl-002"),
        "re-extended baseline needs a slice for the new scenario: {stderr}"
    );
    assert!(
        !stderr.contains("stale"),
        "freshness should clear once position is bumped: {stderr}"
    );

    write_qa_slices(repo, "report-builder", &["bl-001", "bl-002"]);
    let shipped = stdout(maestro(&ship, repo), &ship);
    assert!(shipped.contains("shipped report-builder"));
}

#[test]
fn accept_words_a_blank_baseline_as_empty_not_missing() {
    let temp = TestTempDir::new("maestro-qa-empty-baseline-test");
    let repo = temp.path();
    init_and_author(repo, "report-builder", "Report builder");

    // A present-but-whitespace baseline.md: read_baseline collapses it to None like
    // an absent file, but the gate must distinguish the two in its remedy wording.
    fs::write(
        feature_dir(repo, "report-builder").join("baseline.md"),
        "   \n\n",
    )
    .expect("invariant: baseline.md should be writable");

    let accept = ["feature", "accept", "report-builder"];
    let stderr = assert_failure(maestro(&accept, repo), &accept);
    assert!(
        stderr.contains("qa-baseline") && stderr.contains("empty"),
        "a blank baseline should read 'empty', not 'missing': {stderr}"
    );
    assert!(
        !stderr.contains("baseline.md missing"),
        "a present-but-blank file must not be reported as missing: {stderr}"
    );
}

#[test]
fn non_goal_amend_does_not_block_ship_via_cli() {
    let temp = TestTempDir::new("maestro-qa-nongoal-test");
    let repo = temp.path();
    init_and_author(repo, "report-builder", "Report builder");

    write_baseline(repo, "report-builder", 0, &["bl-001"]);
    stdout(
        maestro(&["feature", "accept", "report-builder"], repo),
        &["feature", "accept"],
    );
    stdout(
        maestro(&["feature", "start", "report-builder"], repo),
        &["feature", "start"],
    );
    write_qa_slices(repo, "report-builder", &["bl-001"]);

    // A non-goal amend is not behavioral, so it must not stale the baseline.
    let amend = [
        "feature",
        "amend",
        "report-builder",
        "--add-non-goal",
        "no pdf export",
        "--reason",
        "clarify scope",
    ];
    stdout(maestro(&amend, repo), &amend);

    let ship = ["feature", "ship", "report-builder"];
    let shipped = stdout(maestro(&ship, repo), &ship);
    assert!(shipped.contains("shipped report-builder"));
}
