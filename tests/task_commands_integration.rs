mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_yaml::Value;
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
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_failure(output: &std::process::Output, args: &[&str]) {
    assert!(
        !output.status.success(),
        "maestro {:?} unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("invariant: stdout should be UTF-8")
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("invariant: stderr should be UTF-8")
}

fn setup_repo() -> TestTempDir {
    let temp = TestTempDir::new("maestro-task-cli");
    fs::create_dir_all(temp.path().join(".maestro"))
        .expect("invariant: .maestro directory should be creatable");
    temp
}

fn task_yaml_path(repo: &Path, id: &str) -> PathBuf {
    let tasks_dir = repo.join(".maestro/tasks");
    let prefix = format!("{id}-");
    let entries =
        fs::read_dir(&tasks_dir).expect("invariant: tasks directory should be readable in tests");
    for entry in entries {
        let entry = entry.expect("invariant: tasks entry should be readable");
        let name = entry
            .file_name()
            .to_str()
            .expect("invariant: tasks entry names should be UTF-8")
            .to_string();
        if name.starts_with(&prefix) {
            return entry.path().join("task.yaml");
        }
    }
    panic!("invariant: expected task directory for {id}");
}

fn task_yaml(repo: &Path, id: &str) -> Value {
    let path = task_yaml_path(repo, id);
    let raw = fs::read_to_string(path).expect("invariant: task.yaml should be readable");
    serde_yaml::from_str(&raw).expect("invariant: task.yaml should parse as YAML")
}

#[test]
fn create_explore_accept_claim_complete_flow_updates_task_record() {
    let temp = setup_repo();
    let repo = temp.path();

    let create = maestro(
        repo,
        &[
            "task",
            "create",
            "Add CSV export",
            "--feature",
            "billing-csv",
            "--lane",
            "normal",
            "--risk",
            "high",
        ],
    );
    assert_success(
        &create,
        &[
            "task",
            "create",
            "Add CSV export",
            "--feature",
            "billing-csv",
            "--lane",
            "normal",
            "--risk",
            "high",
        ],
    );
    assert!(stdout(&create).contains("created task-001"));

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
            "implemented CSV export",
        ],
    ] {
        let out = maestro(repo, &args);
        assert_success(&out, &args);
    }

    let doc = task_yaml(repo, "task-001");
    assert_eq!(
        doc["state"],
        Value::String("needs_verification".to_string())
    );
    assert_eq!(doc["claimed_by"], Value::String("maestro".to_string()));
    assert_eq!(doc["acceptance_locked"], Value::Bool(true));
    assert_eq!(doc["feature_id"], Value::String("billing-csv".to_string()));
    let history = doc["state_history"]
        .as_sequence()
        .expect("invariant: state_history should be an array");
    assert_eq!(history.len(), 5);
    assert!(!doc["updated_at"]
        .as_str()
        .expect("invariant: updated_at should be a string")
        .is_empty());
}

#[test]
fn blockers_terminal_transitions_and_claim_gate_behave_as_expected() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "Task A"]),
        &["task", "create", "Task A"],
    );
    assert_success(
        &maestro(repo, &["task", "explore", "task-001"]),
        &["task", "explore", "task-001"],
    );
    assert_success(
        &maestro(repo, &["task", "accept", "task-001"]),
        &["task", "accept", "task-001"],
    );
    assert_success(
        &maestro(
            repo,
            &[
                "task",
                "block",
                "task-001",
                "--reason",
                "waiting for dependency",
                "--by",
                "task-999",
            ],
        ),
        &[
            "task",
            "block",
            "task-001",
            "--reason",
            "waiting for dependency",
            "--by",
            "task-999",
        ],
    );
    let claim = maestro(repo, &["task", "claim", "task-001"]);
    assert_failure(&claim, &["task", "claim", "task-001"]);
    assert!(stderr(&claim).contains("unresolved blockers"));

    assert_success(
        &maestro(
            repo,
            &["task", "unblock", "task-001", "--blocker", "blk-001"],
        ),
        &["task", "unblock", "task-001", "--blocker", "blk-001"],
    );
    assert_success(
        &maestro(repo, &["task", "claim", "task-001"]),
        &["task", "claim", "task-001"],
    );

    assert_success(
        &maestro(repo, &["task", "create", "Task B"]),
        &["task", "create", "Task B"],
    );
    assert_success(
        &maestro(repo, &["task", "reject", "task-002", "--reason", "invalid"]),
        &["task", "reject", "task-002", "--reason", "invalid"],
    );
    assert_eq!(
        task_yaml(repo, "task-002")["state"],
        Value::String("rejected".to_string())
    );

    assert_success(
        &maestro(repo, &["task", "create", "Task C"]),
        &["task", "create", "Task C"],
    );
    assert_success(
        &maestro(
            repo,
            &["task", "abandon", "task-003", "--reason", "not needed"],
        ),
        &["task", "abandon", "task-003", "--reason", "not needed"],
    );
    assert_eq!(
        task_yaml(repo, "task-003")["state"],
        Value::String("abandoned".to_string())
    );

    assert_success(
        &maestro(repo, &["task", "create", "Task D"]),
        &["task", "create", "Task D"],
    );
    assert_success(
        &maestro(
            repo,
            &[
                "task",
                "supersede",
                "task-004",
                "--by",
                "task-005",
                "--reason",
                "replaced",
            ],
        ),
        &[
            "task",
            "supersede",
            "task-004",
            "--by",
            "task-005",
            "--reason",
            "replaced",
        ],
    );
    let superseded = task_yaml(repo, "task-004");
    assert_eq!(superseded["state"], Value::String("superseded".to_string()));
    let history = superseded["state_history"]
        .as_sequence()
        .expect("invariant: state_history should be present");
    let last = history
        .last()
        .expect("invariant: superseded task should have a terminal history entry");
    assert_eq!(last["to"], Value::String("task-005".to_string()));
}

#[test]
fn show_uses_maestro_current_task_when_no_id_is_provided() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "Task A"]),
        &["task", "create", "Task A"],
    );

    let show = maestro_with_env(
        repo,
        &["task", "show"],
        &[("MAESTRO_CURRENT_TASK", "task-001")],
    );
    assert_success(&show, &["task", "show"]);
    assert!(stdout(&show).contains("id: task-001"));

    let missing = maestro(repo, &["task", "show"]);
    assert_failure(&missing, &["task", "show"]);
    assert!(stderr(&missing).contains("MAESTRO_CURRENT_TASK"));
}

#[test]
fn list_supports_basic_output_and_requested_filters() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(
            repo,
            &["task", "create", "Task A", "--feature", "billing-csv"],
        ),
        &["task", "create", "Task A", "--feature", "billing-csv"],
    );
    assert_success(
        &maestro(
            repo,
            &["task", "create", "Task B", "--feature", "billing-csv"],
        ),
        &["task", "create", "Task B", "--feature", "billing-csv"],
    );
    assert_success(
        &maestro(repo, &["task", "create", "Task C", "--feature", "other"]),
        &["task", "create", "Task C", "--feature", "other"],
    );

    for args in [
        vec!["task", "explore", "task-001"],
        vec!["task", "accept", "task-001"],
        vec!["task", "explore", "task-002"],
        vec!["task", "accept", "task-002"],
        vec![
            "task",
            "block",
            "task-002",
            "--reason",
            "wait for task-001",
            "--by",
            "task-001",
        ],
    ] {
        let out = maestro(repo, &args);
        assert_success(&out, &args);
    }

    let all = maestro(repo, &["task", "list"]);
    assert_success(&all, &["task", "list"]);
    let all_out = stdout(&all);
    assert!(all_out.contains("ID\tSTATE\tTITLE"));
    assert!(all_out.contains("task-001"));
    assert!(all_out.contains("task-002"));
    assert!(all_out.contains("task-003"));

    let ready = maestro(repo, &["task", "list", "--ready"]);
    assert_success(&ready, &["task", "list", "--ready"]);
    let ready_out = stdout(&ready);
    assert!(ready_out.contains("task-001"));
    assert!(!ready_out.contains("task-002"));
    assert!(!ready_out.contains("task-003"));

    let blocked = maestro(repo, &["task", "list", "--blocked"]);
    assert_success(&blocked, &["task", "list", "--blocked"]);
    let blocked_out = stdout(&blocked);
    assert!(blocked_out.contains("task-002"));
    assert!(!blocked_out.contains("task-001"));

    let blocked_by = maestro(repo, &["task", "list", "--blocked-by", "task-001"]);
    assert_success(&blocked_by, &["task", "list", "--blocked-by", "task-001"]);
    assert!(stdout(&blocked_by).contains("task-002"));

    let blocks = maestro(repo, &["task", "list", "--blocks", "task-002"]);
    assert_success(&blocks, &["task", "list", "--blocks", "task-002"]);
    assert!(stdout(&blocks).contains("task-001"));

    let feature = maestro(repo, &["task", "list", "--feature", "billing-csv"]);
    assert_success(&feature, &["task", "list", "--feature", "billing-csv"]);
    let feature_out = stdout(&feature);
    assert!(feature_out.contains("task-001"));
    assert!(feature_out.contains("task-002"));
    assert!(!feature_out.contains("task-003"));
}
