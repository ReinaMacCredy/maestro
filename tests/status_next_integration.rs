mod support;

use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::Value as JsonValue;
use serde_yaml::Value as YamlValue;
use support::TestTempDir;

fn maestro(cwd: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("invariant: compiled maestro binary should run in integration tests")
}

fn maestro_with_env(cwd: &Path, args: &[&str], envs: &[(&str, &str)]) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_maestro"));
    command.args(args).current_dir(cwd);
    for (key, value) in envs {
        command.env(key, value);
    }
    command
        .output()
        .expect("invariant: compiled maestro binary should run in integration tests")
}

fn assert_success(output: &std::process::Output, args: &[&str]) {
    assert!(
        output.status.success(),
        "maestro {:?} failed\nstdout:\n{}\nstderr:\n{}",
        args,
        stdout(output),
        stderr(output)
    );
}

fn assert_failure(output: &std::process::Output, args: &[&str]) {
    assert!(
        !output.status.success(),
        "maestro {:?} unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        args,
        stdout(output),
        stderr(output)
    );
}

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("invariant: stdout should be UTF-8")
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("invariant: stderr should be UTF-8")
}

fn setup_repo(prefix: &str) -> TestTempDir {
    let temp = TestTempDir::new(prefix);
    fs::create_dir(temp.path().join(".git")).expect("invariant: .git marker should be creatable");
    let init = maestro(temp.path(), &["init", "--yes"]);
    assert_success(&init, &["init", "--yes"]);
    temp
}

fn run(repo: &Path, args: &[&str]) -> String {
    let output = maestro(repo, args);
    assert_success(&output, args);
    stdout(&output)
}

fn task_yaml(repo: &Path, id: &str) -> YamlValue {
    let prefix = format!("{id}-");
    let tasks_dir = repo.join(".maestro/tasks");
    for entry in fs::read_dir(tasks_dir).expect("invariant: tasks dir should be readable") {
        let entry = entry.expect("invariant: task entry should be readable");
        let name = entry
            .file_name()
            .to_str()
            .expect("invariant: task dir should be UTF-8")
            .to_string();
        if name.starts_with(&prefix) {
            let raw = fs::read_to_string(entry.path().join("task.yaml"))
                .expect("invariant: task.yaml should be readable");
            return serde_yaml::from_str(&raw).expect("invariant: task.yaml should parse");
        }
    }
    panic!("invariant: task directory should exist for {id}");
}

#[test]
fn status_before_init_is_friendly_and_read_only() {
    let temp = TestTempDir::new("maestro-status-preinit");
    fs::create_dir(temp.path().join(".git")).expect("invariant: .git marker should be creatable");

    let status = maestro(temp.path(), &["status"]);

    assert_success(&status, &["status"]);
    let out = stdout(&status);
    assert!(out.contains("maestro status: not initialized"));
    assert!(out.contains("next: maestro init --yes"));
    assert!(!temp.path().join(".maestro").exists());
}

#[test]
fn task_next_no_action_prints_summary_and_exits_nonzero() {
    let temp = setup_repo("maestro-task-next-empty");
    let repo = temp.path();

    let next = maestro(repo, &["task", "next"]);

    assert_failure(&next, &["task", "next"]);
    assert!(stdout(&next).contains("no actionable tasks"));
    assert!(stderr(&next).contains("no actionable tasks"));
}

#[test]
fn status_and_task_next_choose_current_task_before_ready_queue() {
    let temp = setup_repo("maestro-status-current");
    let repo = temp.path();
    run(
        repo,
        &["task", "create", "Ready task", "--check", "ready check"],
    );
    run(repo, &["task", "explore", "task-001"]);
    run(repo, &["task", "accept", "task-001"]);
    run(repo, &["task", "create", "Draft task"]);

    let next = maestro_with_env(
        repo,
        &["task", "next"],
        &[("MAESTRO_CURRENT_TASK", "task-002")],
    );

    assert_success(&next, &["task", "next"]);
    let out = stdout(&next);
    assert!(out.contains("next: maestro task set task-002 --check"));
    assert!(out.contains("task: task-002"));

    let status = maestro_with_env(
        repo,
        &["status", "--json"],
        &[("MAESTRO_CURRENT_TASK", "task-002")],
    );
    assert_success(&status, &["status", "--json"]);
    let json: JsonValue =
        serde_json::from_str(&stdout(&status)).expect("invariant: status JSON should parse");
    assert_eq!(json["schema"], "maestro.status.v1");
    assert_eq!(json["current_task"], "task-002");
    assert_eq!(json["next_action"]["kind"], "add_task_check");
    assert_eq!(json["next_action"]["requires_input"], true);
}

#[test]
fn task_create_check_handoff_and_list_columns_are_actionable() {
    let temp = setup_repo("maestro-create-check-next");
    let repo = temp.path();

    let create = run(
        repo,
        &[
            "task",
            "create",
            "Add export",
            "--check",
            "cargo test passes",
        ],
    );

    assert!(create.contains("created task-001 (draft)"));
    assert!(create.contains("verify+ locked:"));
    assert!(create.contains("next: maestro task explore task-001"));

    let list = run(repo, &["task", "list"]);
    assert!(list.contains("NEXT"));
    assert!(list.contains("INSPECT"));
    assert!(list.contains("maestro task show task-001"));
}

#[test]
fn complete_with_proof_records_proof_and_auto_verifies() {
    let temp = setup_repo("maestro-complete-proof-auto");
    let repo = temp.path();

    run(
        repo,
        &[
            "task",
            "create",
            "Add export",
            "--check",
            "cargo test passes",
        ],
    );
    run(repo, &["task", "explore", "task-001"]);
    run(repo, &["task", "accept", "task-001"]);
    run(repo, &["task", "claim", "task-001"]);
    let complete = run(
        repo,
        &[
            "task",
            "complete",
            "task-001",
            "--summary",
            "done",
            "--claim",
            "cargo test passes",
            "--proof",
            "cargo test passes",
        ],
    );

    assert!(complete.contains("auto: recorded task_proof event"));
    assert!(complete.contains("auto: maestro task verify task-001"));
    assert!(complete.contains("verification passed for task-001"));
    assert_eq!(
        task_yaml(repo, "task-001")["state"],
        YamlValue::String("verified".to_string())
    );
}
