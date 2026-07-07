mod common;
mod support;

use std::fs;
use std::path::Path;

use common::cli_harness::maestro as cli_maestro;
use maestro::domain::feature;
use maestro::foundation::core::paths::MaestroPaths;
use maestro::foundation::core::time::utc_now_timestamp;
use serde_json::Value;
use support::TestTempDir;

fn maestro(args: &[&str], cwd: &Path) -> std::process::Output {
    cli_maestro(cwd).args(args).output().into_raw()
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

fn init_repo(repo: &Path) {
    fs::create_dir(repo.join(".git")).expect("invariant: .git marker should be creatable");
    stdout(maestro(&["init", "--yes"], repo), &["init", "--yes"]);
}

fn create_feature(repo: &Path, title: &str) -> String {
    stdout(
        maestro(&["feature", "new", title, "--id-only"], repo),
        &["feature", "new", title, "--id-only"],
    )
    .trim()
    .to_string()
}

fn write_research(repo: &Path, id: &str, contents: &str) {
    let paths = MaestroPaths::new(repo);
    feature::write_sidecar_text(&paths, id, "research.md", contents)
        .expect("invariant: research.md should be writable");
}

fn today() -> String {
    utc_now_timestamp()[..10].to_string()
}

fn ready_receipt(project: &str) -> String {
    format!(
        r#"# Research Brief

## Research Status
skipped: false
skip_reason:
skipped_by:

## Hosting
project: {project}
rationale: intended repo is confirmed

## Problem
Help sales operators handle leads.

## Users / Stakeholders
Sales operators.

## Current Context
The target repo and workflow are known.

## Constraints
None.

## Unknowns
### Blocking
None.
### Important but non-blocking
None.
### Safe to defer
None.

## Assumptions
None.

## Landscape
Dedicated assistant.

## Recommended First Design Fork
Where should Copilot live in the Sales workflow?

## Stakeholder Actions
None.

## Research Validity
as_of: {as_of}
invalidates_when:
- stakeholder changes primary workflow

## Gate
READY_FOR_DESIGN
"#,
        as_of = today()
    )
}

fn json_check(repo: &Path, id: &str, extra: &[&str]) -> Value {
    let mut args = vec!["research", "check", id];
    args.extend(extra);
    args.push("--json");
    let output = stdout(maestro(&args, repo), &args);
    serde_json::from_str(&output).expect("research check JSON should parse")
}

#[test]
fn research_check_reports_fresh_ready_json() {
    let temp = TestTempDir::new("maestro-research-ready");
    let repo = temp.path();
    init_repo(repo);
    let id = create_feature(repo, "Sales Copilot");
    write_research(repo, &id, &ready_receipt("current-repo"));

    let json = json_check(repo, &id, &["--intended-project", "current-repo"]);

    assert_eq!(json["schema"], "maestro.research_check.v1");
    assert_eq!(json["card"], id);
    assert_eq!(json["status"], "ready");
    assert_eq!(json["gate"], "READY_FOR_DESIGN");
    assert_eq!(json["fresh"], true);
    assert_eq!(json["hosting"]["compatible"], true);
    assert_eq!(
        json["first_design_fork"],
        "Where should Copilot live in the Sales workflow?"
    );
}

#[test]
fn research_check_reports_missing_without_writing() {
    let temp = TestTempDir::new("maestro-research-missing");
    let repo = temp.path();
    init_repo(repo);
    let id = create_feature(repo, "Missing Research");

    let args = ["research", "check", id.as_str()];
    let human = stdout(maestro(&args, repo), &args);

    assert!(human.contains("research: missing"), "{human}");
    assert!(
        !repo
            .join(".maestro/cards")
            .join(&id)
            .join("research.md")
            .exists(),
        "check must not create research.md"
    );
}

#[test]
fn research_check_rejects_stale_ready_receipt() {
    let temp = TestTempDir::new("maestro-research-stale");
    let repo = temp.path();
    init_repo(repo);
    let id = create_feature(repo, "Stale Research");
    write_research(
        repo,
        &id,
        &ready_receipt("current-repo").replace(&today(), "2000-01-01"),
    );

    let json = json_check(repo, &id, &[]);

    assert_eq!(json["status"], "stale");
    assert!(
        json["reasons"]
            .as_array()
            .unwrap()
            .contains(&Value::from("stale"))
    );
}

#[test]
fn research_check_reports_hosting_mismatch() {
    let temp = TestTempDir::new("maestro-research-hosting");
    let repo = temp.path();
    init_repo(repo);
    let id = create_feature(repo, "Sandbox Research");
    write_research(repo, &id, &ready_receipt("sandbox-repo"));

    let json = json_check(repo, &id, &["--intended-project", "current-repo"]);

    assert_eq!(json["status"], "hosting_mismatch");
    assert_eq!(json["hosting"]["project"], "sandbox-repo");
    assert_eq!(json["hosting"]["compatible"], false);
}

#[test]
fn research_check_blocks_ready_without_first_design_fork() {
    let temp = TestTempDir::new("maestro-research-first-fork");
    let repo = temp.path();
    init_repo(repo);
    let id = create_feature(repo, "Missing First Fork");
    write_research(
        repo,
        &id,
        &ready_receipt("current-repo")
            .replace("Where should Copilot live in the Sales workflow?", "None."),
    );

    let json = json_check(repo, &id, &[]);

    assert_eq!(json["status"], "blocked");
    assert!(
        json["reasons"]
            .as_array()
            .unwrap()
            .contains(&Value::from("first_design_fork_missing"))
    );
}

#[test]
fn research_check_blocks_unknowns_and_open_stakeholders() {
    let temp = TestTempDir::new("maestro-research-blocked");
    let repo = temp.path();
    init_repo(repo);
    let id = create_feature(repo, "Blocked Research");
    let receipt = ready_receipt("current-repo")
        .replace("### Blocking\nNone.", "### Blocking\n- Which sales chat app is canonical?")
        .replace(
            "## Stakeholder Actions\nNone.",
            "## Stakeholder Actions\n- question: Which sales chat app is canonical?\n  ask: Sales Lead\n  status: open\n  blocks: integration architecture fork",
        );
    write_research(repo, &id, &receipt);

    let json = json_check(repo, &id, &[]);

    assert_eq!(json["status"], "blocked");
    let reasons = json["reasons"].as_array().unwrap();
    assert!(reasons.contains(&Value::from("blocked_unknowns")));
    assert!(reasons.contains(&Value::from("stakeholder_blocked")));
}

#[test]
fn research_check_distinguishes_valid_and_risky_skips() {
    let temp = TestTempDir::new("maestro-research-skips");
    let repo = temp.path();
    init_repo(repo);
    let valid_id = create_feature(repo, "Valid Skip");
    let risky_id = create_feature(repo, "Risky Skip");
    let valid = ready_receipt("current-repo")
        .replace("skipped: false", "skipped: true")
        .replace("skip_reason:", "skip_reason: settled spec pasted")
        .replace(
            "skipped_by:",
            "skipped_by: agent\nevidence: request.md has settled context",
        );
    let risky = valid
        .replace(
            "skip_reason: settled spec pasted",
            "skip_reason: user explicit",
        )
        .replace("skipped_by: agent", "skipped_by: user")
        .replace(
            "evidence: request.md has settled context",
            "unresolved_risks:\n- auth boundary is unknown",
        );
    write_research(repo, &valid_id, &valid);
    write_research(repo, &risky_id, &risky);

    let valid_json = json_check(repo, &valid_id, &[]);
    let risky_json = json_check(repo, &risky_id, &[]);

    assert_eq!(valid_json["status"], "skipped");
    assert!(
        valid_json["reasons"]
            .as_array()
            .unwrap()
            .contains(&Value::from("skip_valid"))
    );
    assert_eq!(risky_json["status"], "skipped");
    assert!(
        risky_json["reasons"]
            .as_array()
            .unwrap()
            .contains(&Value::from("skip_risky"))
    );
}

#[test]
fn sales_copilot_fixture_is_never_ready_on_wrong_repo() {
    let temp = TestTempDir::new("maestro-research-sales-copilot");
    let repo = temp.path();
    init_repo(repo);
    let id = create_feature(repo, "Sales Copilot");
    let receipt = ready_receipt("external")
        .replace("READY_FOR_DESIGN", "NEEDS_STAKEHOLDER")
        .replace("### Blocking\nNone.", "### Blocking\n- Which sales chat app is canonical?")
        .replace(
            "## Stakeholder Actions\nNone.",
            "## Stakeholder Actions\n- question: Which sales chat app is canonical?\n  ask: Sales Lead\n  status: open\n  blocks: integration architecture fork",
        );
    write_research(repo, &id, &receipt);

    let json = json_check(repo, &id, &["--intended-project", "current-repo"]);

    assert_ne!(json["status"], "ready");
    let reasons = json["reasons"].as_array().unwrap();
    assert!(
        reasons.contains(&Value::from("hosting_mismatch"))
            || reasons.contains(&Value::from("stakeholder_blocked")),
        "{json}"
    );
}
