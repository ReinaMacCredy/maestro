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

#[derive(Clone, Copy)]
enum HeadingStyle {
    Markdown,
    Label,
}

fn ready_receipt(project: &str) -> String {
    ready_receipt_with_style(project, HeadingStyle::Markdown)
}

fn colon_label_receipt(project: &str) -> String {
    ready_receipt_with_style(project, HeadingStyle::Label)
}

fn ready_receipt_with_style(project: &str, style: HeadingStyle) -> String {
    let heading = |level: usize, title: &str| match style {
        HeadingStyle::Markdown => format!("{} {title}", "#".repeat(level)),
        HeadingStyle::Label => format!("{title}:"),
    };
    format!(
        r#"{title}

{research_status}
skipped: false
skip_reason:
skipped_by:

{hosting}
project: {project}
rationale: intended repo is confirmed

{problem}
Help sales operators handle leads.

{users}
Sales operators.

{current_context}
The target repo and workflow are known.

{constraints}
None.

{unknowns}
{blocking}
None.
{important}
None.
{safe_to_defer}
None.

{assumptions}
None.

{landscape}
Dedicated assistant.

{first_fork}
Where should Copilot live in the Sales workflow?

{stakeholder_actions}
None.

{validity}
as_of: {as_of}
invalidates_when:
- stakeholder changes primary workflow

{gate}
READY_FOR_DESIGN
"#,
        title = heading(1, "Research Brief"),
        research_status = heading(2, "Research Status"),
        hosting = heading(2, "Hosting"),
        problem = heading(2, "Problem"),
        users = heading(2, "Users / Stakeholders"),
        current_context = heading(2, "Current Context"),
        constraints = heading(2, "Constraints"),
        unknowns = heading(2, "Unknowns"),
        blocking = heading(3, "Blocking"),
        important = heading(3, "Important but non-blocking"),
        safe_to_defer = heading(3, "Safe to defer"),
        assumptions = heading(2, "Assumptions"),
        landscape = heading(2, "Landscape"),
        first_fork = heading(2, "Recommended First Design Fork"),
        stakeholder_actions = heading(2, "Stakeholder Actions"),
        validity = heading(2, "Research Validity"),
        gate = heading(2, "Gate"),
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
    assert_eq!(json["version"], 1);
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
fn research_check_accepts_documented_colon_label_receipt() {
    let temp = TestTempDir::new("maestro-research-colon-labels");
    let repo = temp.path();
    init_repo(repo);
    let id = create_feature(repo, "Colon Research");
    write_research(repo, &id, &colon_label_receipt("current-repo"));

    let json = json_check(repo, &id, &["--intended-project", "current-repo"]);

    assert_eq!(json["status"], "ready");
    assert_eq!(json["gate"], "READY_FOR_DESIGN");
    assert_eq!(json["fresh"], true);
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
    assert_eq!(
        valid_json["next"],
        "maestro-design may start from the valid skip receipt"
    );
    assert!(
        valid_json["reasons"]
            .as_array()
            .unwrap()
            .contains(&Value::from("skip_valid"))
    );
    assert_eq!(risky_json["status"], "risky_skipped");
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
