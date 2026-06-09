//! End-to-end QA gate wiring (§4): accept reads `qa.md`, ship reads its fenced
//! QA slices block and the `feature.yaml` amends that `feature amend` writes.
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
    repo.join(".maestro/cards").join(id)
}

fn write_baseline(repo: &Path, id: &str, position: usize, scenario_ids: &[&str]) {
    let scenarios = scenario_ids
        .iter()
        .map(|id| format!("  - [{id}] scenario {id} (covers: ac-1)\n"))
        .collect::<String>();
    fs::write(
        feature_dir(repo, id).join("qa.md"),
        format!("---\namend_log_position: {position}\n---\n\n### QA Baseline Contract\n\n- Scenario Matrix:\n{scenarios}"),
    )
    .expect("invariant: qa.md should be writable");
}

fn write_qa_slices(repo: &Path, id: &str, covered: &[&str]) {
    let slices = covered
        .iter()
        .map(|id| format!("  - scenarios: [\"{id}\"]\n    evidence: [\"proof for {id}\"]\n"))
        .collect::<String>();
    write_qa_slices_yaml(repo, id, &format!("slices:\n{slices}"));
}

fn write_qa_slices_yaml(repo: &Path, id: &str, yaml: &str) {
    let path = feature_dir(repo, id).join("qa.md");
    let mut contents = fs::read_to_string(&path).unwrap_or_default();
    if let Some(start) = contents.find("\n```yaml\nslices:") {
        contents.truncate(start);
    }
    contents.push_str("\n```yaml\n");
    contents.push_str(yaml);
    if !yaml.ends_with('\n') {
        contents.push('\n');
    }
    contents.push_str("```\n");
    fs::write(path, contents).expect("invariant: qa.md should be writable");
}

fn verify_contract_from_qa(repo: &Path, id: &str) {
    let args = ["feature", "verify", id];
    let output = stdout(maestro(&args, repo), &args);
    assert!(
        output.contains("proof: qa.md counting slice OK"),
        "{output}"
    );
    assert!(
        output.contains("ok: every acceptance item has evidence"),
        "{output}"
    );
}

fn prove_contract(repo: &Path, id: &str) {
    let prove = [
        "feature",
        "verify",
        id,
        "--prove",
        "ac-1",
        "--evidence",
        "fixture evidence",
    ];
    stdout(maestro(&prove, repo), &prove);
    let sweep = ["feature", "verify", id];
    let output = stdout(maestro(&sweep, repo), &sweep);
    assert!(
        output.contains("ok: every acceptance item has evidence"),
        "{output}"
    );
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
        stderr.contains("target: .maestro/cards/report-builder/qa.md"),
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
        stderr.contains("target: .maestro/cards/report-builder/qa.md"),
        "{stderr}"
    );
    assert!(
        stderr.contains("retry: maestro feature ship report-builder --outcome \"<outcome>\""),
        "{stderr}"
    );

    // D count rule through the real YAML parse path: a slice that references the
    // scenario but omits `evidence` (serde default → empty) does not count.
    write_qa_slices_yaml(
        repo,
        "report-builder",
        "slices:\n  - scenarios: [\"bl-001\"]\n",
    );
    let stderr = assert_failure(maestro(&ship, repo), &ship);
    assert!(
        stderr.contains("bl-001"),
        "an evidence-less slice must not count: {stderr}"
    );

    write_qa_slices(repo, "report-builder", &["bl-001"]);
    verify_contract_from_qa(repo, "report-builder");
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
    let sweep = ["feature", "verify", "report-builder"];
    stdout(maestro(&sweep, repo), &sweep);
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
    verify_contract_from_qa(repo, "report-builder");
    let shipped = stdout(maestro(&ship, repo), &ship);
    assert!(shipped.contains("shipped report-builder"));
}

#[test]
fn accept_words_a_blank_baseline_as_empty_not_missing() {
    let temp = TestTempDir::new("maestro-qa-empty-baseline-test");
    let repo = temp.path();
    init_and_author(repo, "report-builder", "Report builder");

    // A present-but-whitespace qa.md: read_baseline collapses it to None like
    // an absent file, but the gate must distinguish the two in its remedy wording.
    fs::write(feature_dir(repo, "report-builder").join("qa.md"), "   \n\n")
        .expect("invariant: qa.md should be writable");

    let accept = ["feature", "accept", "report-builder"];
    let stderr = assert_failure(maestro(&accept, repo), &accept);
    assert!(
        stderr.contains("qa-baseline") && stderr.contains("empty"),
        "a blank baseline should read 'empty', not 'missing': {stderr}"
    );
    assert!(
        !stderr.contains("qa.md missing"),
        "a present-but-blank file must not be reported as missing: {stderr}"
    );
}

#[test]
fn qa_none_accept_skips_gates_until_a_behavioral_amend_requires_a_fresh_declaration() {
    let temp = TestTempDir::new("maestro-qa-none-test");
    let repo = temp.path();
    init_and_author(repo, "config-cleanup", "Config cleanup");

    let accept = [
        "feature",
        "accept",
        "config-cleanup",
        "--qa",
        "none",
        "--reason",
        "config-only, no behavior",
    ];
    let accepted = stdout(maestro(&accept, repo), &accept);
    assert!(accepted.contains("accepted config-cleanup"), "{accepted}");
    assert!(
        accepted.contains("qa: none (config-only, no behavior)"),
        "{accepted}"
    );
    let show = stdout(
        maestro(&["feature", "show", "config-cleanup"], repo),
        &["feature", "show", "config-cleanup"],
    );
    assert!(
        show.contains("qa: none (config-only, no behavior)"),
        "{show}"
    );

    stdout(
        maestro(&["feature", "start", "config-cleanup"], repo),
        &["feature", "start", "config-cleanup"],
    );
    let amend = [
        "feature",
        "amend",
        "config-cleanup",
        "--add-area",
        "runtime",
        "--reason",
        "scope grew",
    ];
    stdout(maestro(&amend, repo), &amend);

    let ship = ["feature", "ship", "config-cleanup"];
    let stale = assert_failure(maestro(&ship, repo), &ship);
    assert!(stale.contains("qa-baseline"), "{stale}");

    let redeclare = [
        "feature",
        "accept",
        "config-cleanup",
        "--qa",
        "none",
        "--reason",
        "still config-only after amend review",
    ];
    let redeclared = stdout(maestro(&redeclare, repo), &redeclare);
    assert!(
        redeclared.contains("recorded qa: none for config-cleanup"),
        "{redeclared}"
    );

    prove_contract(repo, "config-cleanup");
    let shipped = stdout(maestro(&ship, repo), &ship);
    assert!(shipped.contains("shipped config-cleanup"), "{shipped}");
    assert!(
        shipped.contains("qa: none (still config-only after amend review)"),
        "{shipped}"
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

    verify_contract_from_qa(repo, "report-builder");
    let ship = ["feature", "ship", "report-builder"];
    let shipped = stdout(maestro(&ship, repo), &ship);
    assert!(shipped.contains("shipped report-builder"));
}
