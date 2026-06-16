//! T4 read surface for the monorepo `--project` scope: `list`/`ready` `--project`
//! filter, the `[project]` badge on human rows, group-by-project in `list` once
//! two or more distinct projects appear, and the flat `project` field in
//! `list`/`ready`/`status` `--json`. Drives the real binary so the contract is
//! exercised the way an agent consumer would.

pub mod card_support;
mod support;

use std::path::Path;
use std::process::{Command, Output};

use card_support::cards_repo;
use serde_json::Value;

fn maestro(cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .current_dir(cwd)
        .env("MAESTRO_AGENT", "codex")
        .env("MAESTRO_SESSION", "s1")
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

/// A card per project, one task with no project, for the multi-project cases.
fn seed_multi_project(repo: &Path) {
    run(
        repo,
        &["create", "-t", "task", "pay one", "--project", "svc-pay"],
    );
    run(
        repo,
        &["create", "-t", "task", "pay two", "--project", "svc-pay"],
    );
    run(
        repo,
        &["create", "-t", "task", "auth one", "--project", "svc-auth"],
    );
    run(repo, &["create", "-t", "task", "no project task"]);
}

#[test]
fn project_filter_returns_only_matching_cards_and_unknown_is_empty() {
    let temp = cards_repo("t4-filter");
    let repo = temp.path();
    seed_multi_project(repo);

    let pay = run(repo, &["list", "--project", "svc-pay"]);
    assert!(
        pay.contains("pay one") && pay.contains("pay two"),
        "svc-pay list keeps its cards:\n{pay}"
    );
    assert!(
        !pay.contains("auth one") && !pay.contains("no project task"),
        "svc-pay list drops other projects and the no-project card:\n{pay}"
    );

    // The namespaced `card list` spelling shares the handler, so the filter holds.
    let pay_ns = run(repo, &["card", "list", "--project", "svc-pay"]);
    assert!(
        pay_ns.contains("pay one") && !pay_ns.contains("auth one"),
        "card list --project filters too:\n{pay_ns}"
    );

    let ready_pay = run(repo, &["ready", "--project", "svc-pay"]);
    assert!(
        ready_pay.contains("pay one") && !ready_pay.contains("auth one"),
        "ready --project filters too:\n{ready_pay}"
    );

    // An unknown project is an empty result, not an error (exit 0).
    let unknown = maestro(repo, &["list", "--project", "svc-nope"]);
    assert!(
        unknown.status.success(),
        "unknown project exits 0:\nstderr:\n{}",
        String::from_utf8_lossy(&unknown.stderr)
    );
    let unknown_out = String::from_utf8_lossy(&unknown.stdout);
    assert!(
        !unknown_out.contains("pay one") && !unknown_out.contains("auth one"),
        "unknown project shows no cards:\n{unknown_out}"
    );

    let unknown_ready = maestro(repo, &["ready", "--project", "svc-nope"]);
    assert!(
        unknown_ready.status.success(),
        "unknown project ready exits 0:\nstderr:\n{}",
        String::from_utf8_lossy(&unknown_ready.stderr)
    );
}

#[test]
fn badge_shows_for_project_cards_and_is_absent_otherwise() {
    let temp = cards_repo("t4-badge");
    let repo = temp.path();
    // One declared project => flat list, but the project card still gets a badge.
    run(
        repo,
        &[
            "create",
            "-t",
            "task",
            "scoped task",
            "--project",
            "svc-pay",
        ],
    );
    run(repo, &["create", "-t", "task", "loose task"]);

    let list = run(repo, &["list"]);
    let scoped_line = list
        .lines()
        .find(|l| l.contains("scoped task"))
        .expect("scoped task row present");
    let loose_line = list
        .lines()
        .find(|l| l.contains("loose task"))
        .expect("loose task row present");
    assert!(
        scoped_line.contains("[svc-pay]"),
        "a card with a project shows the [project] badge:\n{scoped_line}"
    );
    assert!(
        !loose_line.contains('['),
        "a card without a project shows no badge:\n{loose_line}"
    );

    let ready = run(repo, &["ready"]);
    let ready_scoped = ready
        .lines()
        .find(|l| l.contains("scoped task"))
        .expect("scoped task ready row present");
    assert!(
        ready_scoped.contains("[svc-pay]"),
        "ready rows carry the badge too:\n{ready_scoped}"
    );
}

#[test]
fn grouping_headers_appear_only_at_two_distinct_projects() {
    // Two distinct projects among shown cards => grouped headers + an
    // `unassigned` group for the no-project card.
    let multi = cards_repo("t4-group-multi");
    let repo = multi.path();
    seed_multi_project(repo);
    let grouped = run(repo, &["list"]);
    assert!(
        grouped.contains("svc-auth:") && grouped.contains("svc-pay:"),
        "two distinct projects produce a header per project:\n{grouped}"
    );
    assert!(
        grouped.contains("unassigned:"),
        "the no-project card lands under an unassigned group when grouping:\n{grouped}"
    );
    // Headers are sorted (svc-auth before svc-pay), unassigned trails last.
    let auth_at = grouped.find("svc-auth:").unwrap();
    let pay_at = grouped.find("svc-pay:").unwrap();
    let unassigned_at = grouped.find("unassigned:").unwrap();
    assert!(
        auth_at < pay_at && pay_at < unassigned_at,
        "project headers sort with unassigned trailing:\n{grouped}"
    );

    // Exactly one distinct project => flat (no headers), badges still present.
    let single = cards_repo("t4-group-single");
    let repo = single.path();
    run(
        repo,
        &[
            "create",
            "-t",
            "task",
            "scoped task",
            "--project",
            "svc-pay",
        ],
    );
    run(repo, &["create", "-t", "task", "loose task"]);
    let flat_single = run(repo, &["list"]);
    assert!(
        !flat_single.contains("svc-pay:") && !flat_single.contains("unassigned:"),
        "one distinct project stays flat (no headers):\n{flat_single}"
    );
    assert!(
        flat_single.contains("[svc-pay]"),
        "one distinct project is flat but still badged:\n{flat_single}"
    );

    // Zero projects => flat AND byte-identical to today: no badges, no headers.
    let zero = cards_repo("t4-group-zero");
    let repo = zero.path();
    run(repo, &["create", "-t", "task", "alpha task"]);
    run(repo, &["create", "-t", "task", "beta task"]);
    let flat_zero = run(repo, &["list"]);
    assert!(
        !flat_zero.contains('['),
        "zero-project list carries no badge token:\n{flat_zero}"
    );
    assert!(
        !flat_zero.contains(':') || flat_zero.lines().filter(|l| l.ends_with(':')).count() == 1,
        "zero-project list has only the count header line, no project headers:\n{flat_zero}"
    );
    // Every numbered row ends in the title (today's shape: no trailing badge).
    for line in flat_zero
        .lines()
        .filter(|l| l.starts_with("  ") && l.contains(". "))
    {
        assert!(
            line.ends_with("task"),
            "zero-project rows end at the title, byte-identical to today:\n{line}"
        );
    }
}

#[test]
fn json_stays_flat_with_a_project_field_even_when_human_would_group() {
    let temp = cards_repo("t4-json");
    let repo = temp.path();
    seed_multi_project(repo);

    // list --json: a single dense envelope, each card a flat object with a
    // `project` field; never nested or grouped by project.
    let list = run(repo, &["list", "--json"]);
    let list_lines: Vec<&str> = list.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(
        list_lines.len(),
        1,
        "list --json is one dense line:\n{list}"
    );
    let list_json: Value = serde_json::from_str(list_lines[0]).expect("list --json parses");
    assert!(
        list_json.get("groups").is_none() && list_json.get("projects").is_none(),
        "list --json carries no grouping key:\n{list}"
    );
    let cards = list_json["cards"].as_array().expect("cards array");
    let pay = cards
        .iter()
        .find(|c| c["title"] == "pay one")
        .expect("pay one card");
    assert_eq!(
        pay["project"], "svc-pay",
        "the project rides as a flat field on the card object:\n{pay}"
    );
    let loose = cards
        .iter()
        .find(|c| c["title"] == "no project task")
        .expect("loose card");
    assert!(
        loose["project"].is_null(),
        "a no-project card carries project: null:\n{loose}"
    );

    // ready --json: same flat shape.
    let ready = run(repo, &["ready", "--json"]);
    let ready_json: Value = serde_json::from_str(ready.trim()).expect("ready --json parses");
    let ready_cards = ready_json["cards"].as_array().expect("ready cards array");
    let ready_pay = ready_cards
        .iter()
        .find(|c| c["title"] == "pay one")
        .expect("ready pay card");
    assert_eq!(
        ready_pay["project"], "svc-pay",
        "ready --json carries a flat project field:\n{ready_pay}"
    );

    // status --json: the per-row JSON carries a flat project field too.
    let status = run(repo, &["status", "--json"]);
    let status_json: Value = serde_json::from_str(status.trim()).expect("status --json parses");
    let task_rows = status_json["task_rows"]
        .as_array()
        .expect("task_rows array");
    let row = task_rows
        .iter()
        .find(|r| r["title"] == "pay one")
        .expect("pay one task row");
    assert_eq!(
        row["project"], "svc-pay",
        "status --json task rows carry a flat project field:\n{row}"
    );
}

#[test]
fn status_json_feature_rows_carry_a_flat_project_field() {
    let temp = cards_repo("t4-json-feature");
    let repo = temp.path();
    // A freshly created feature is `proposed` (non-terminal), so it surfaces in
    // the status JSON `active_features`; its project rides as a flat field.
    run(
        repo,
        &[
            "create",
            "-t",
            "feature",
            "billing rework",
            "--project",
            "svc-pay",
        ],
    );
    run(repo, &["create", "-t", "feature", "loose feature"]);

    let status = run(repo, &["status", "--json"]);
    let status_json: Value = serde_json::from_str(status.trim()).expect("status --json parses");
    let features = status_json["active_features"]
        .as_array()
        .expect("active_features array");
    let scoped = features
        .iter()
        .find(|f| f["title"] == "billing rework")
        .expect("billing rework feature row");
    assert_eq!(
        scoped["project"], "svc-pay",
        "status --json feature rows carry a flat project field:\n{scoped}"
    );
    let loose = features
        .iter()
        .find(|f| f["title"] == "loose feature")
        .expect("loose feature row");
    assert!(
        loose["project"].is_null(),
        "a no-project feature row carries project: null:\n{loose}"
    );
}

#[test]
fn status_has_no_project_flag() {
    let temp = cards_repo("t4-status-no-flag");
    let repo = temp.path();

    let out = maestro(repo, &["status", "--project", "svc-pay"]);
    assert!(
        !out.status.success(),
        "status must reject --project (the flag is intentionally absent):\nstdout:\n{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("unexpected argument") && stderr.contains("--project"),
        "status rejects the unknown --project arg:\n{stderr}"
    );
}
