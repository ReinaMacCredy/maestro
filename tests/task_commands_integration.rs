mod support;

use std::fs;
use std::os::unix::fs as unix_fs;
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

    // The task links to a real feature; `create --feature` now rejects a dangling ref.
    assert_success(
        &maestro(repo, &["feature", "new", "Billing CSV"]),
        &["feature", "new", "Billing CSV"],
    );

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
            "--proof",
            "implemented CSV export",
        ],
    ] {
        let out = maestro(repo, &args);
        assert_success(&out, &args);
    }

    let doc = task_yaml(repo, "task-001");
    assert_eq!(doc["state"], Value::String("verified".to_string()));
    assert_eq!(doc["claimed_by"], Value::String("maestro".to_string()));
    assert_eq!(doc["acceptance_locked"], Value::Bool(true));
    assert_eq!(doc["feature_id"], Value::String("billing-csv".to_string()));
    let history = doc["state_history"]
        .as_sequence()
        .expect("invariant: state_history should be an array");
    assert_eq!(history.len(), 6);
    assert!(
        !doc["updated_at"]
            .as_str()
            .expect("invariant: updated_at should be a string")
            .is_empty()
    );
}

#[test]
fn claim_from_draft_is_blocked_with_the_explicit_ready_path() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "Direct claim task"]),
        &["task", "create", "Direct claim task"],
    );
    assert_success(
        &maestro(
            repo,
            &["task", "set", "task-001", "--check", "direct claim check"],
        ),
        &["task", "set", "task-001", "--check", "direct claim check"],
    );
    let claim = maestro(repo, &["task", "claim", "task-001"]);
    assert_failure(&claim, &["task", "claim", "task-001"]);
    let message = stderr(&claim);
    assert!(message.contains("blocked: task task-001 is not ready to claim"));
    assert!(message.contains("next: maestro task explore task-001"));

    let task = task_yaml(repo, "task-001");
    assert_eq!(task["state"], Value::String("draft".to_string()));
    assert_eq!(task["acceptance_locked"], Value::Bool(false));
}

#[test]
fn supersede_rejects_a_nonexistent_target_and_leaves_the_task_untouched() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "Original task"]),
        &["task", "create", "Original task"],
    );

    let args = &[
        "task",
        "supersede",
        "task-001",
        "--by",
        "task-999",
        "--reason",
        "replaced",
    ];
    let supersede = maestro(repo, args);
    assert_failure(&supersede, args);
    assert!(
        stderr(&supersede).contains("supersede target"),
        "supersede should reject a dangling target: {}",
        stderr(&supersede)
    );
    let task = task_yaml(repo, "task-001");
    assert_eq!(task["state"], Value::String("draft".to_string()));
}

#[test]
fn supersede_records_an_existing_target() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "Old"]),
        &["task", "create", "Old"],
    );
    assert_success(
        &maestro(repo, &["task", "create", "New"]),
        &["task", "create", "New"],
    );

    let args = &[
        "task",
        "supersede",
        "task-001",
        "--by",
        "task-002",
        "--reason",
        "replaced by task-002",
    ];
    assert_success(&maestro(repo, args), args);
    let task = task_yaml(repo, "task-001");
    assert_eq!(task["state"], Value::String("superseded".to_string()));
}

#[test]
fn claim_from_exploring_fails_with_an_actionable_message() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "Exploring task"]),
        &["task", "create", "Exploring task"],
    );
    assert_success(
        &maestro(repo, &["task", "explore", "task-001"]),
        &["task", "explore", "task-001"],
    );

    let claim = maestro(repo, &["task", "claim", "task-001"]);
    assert_failure(&claim, &["task", "claim", "task-001"]);
    let message = stderr(&claim);
    assert!(
        message.contains("exploring") && message.contains("task accept"),
        "claiming an exploring task should name the state and point at accept: {message}"
    );
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
        &maestro(
            repo,
            &["task", "set", "task-001", "--check", "task a check"],
        ),
        &["task", "set", "task-001", "--check", "task a check"],
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
        &maestro(repo, &["task", "create", "Task E"]),
        &["task", "create", "Task E"],
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
fn show_treats_empty_current_task_env_as_unset() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "Task A"]),
        &["task", "create", "Task A"],
    );

    // An empty MAESTRO_CURRENT_TASK must give the "id required" remedy, not fall
    // through to a confusing "invalid task id" / "task not found".
    let show = maestro_with_env(repo, &["task", "show"], &[("MAESTRO_CURRENT_TASK", "")]);
    assert_failure(&show, &["task", "show"]);
    let err = stderr(&show);
    assert!(err.contains("task id is required"), "got: {err}");
    assert!(!err.contains("invalid task id"), "got: {err}");
}

#[test]
fn task_id_prefix_lookup_rejects_ambiguous_matches() {
    let temp = setup_repo();
    let repo = temp.path();
    assert_success(
        &maestro(repo, &["task", "create", "First task"]),
        &["task", "create", "First task"],
    );
    let original = task_yaml_path(repo, "task-001");
    let duplicate_dir = repo.join(".maestro/tasks/task-001-duplicate");
    fs::create_dir(&duplicate_dir).expect("invariant: duplicate task dir should be creatable");
    fs::copy(original, duplicate_dir.join("task.yaml"))
        .expect("invariant: duplicate task yaml should be writable");

    let show = maestro(repo, &["task", "show", "task-001"]);
    assert!(
        !show.status.success(),
        "ambiguous task lookup unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&show.stdout),
        String::from_utf8_lossy(&show.stderr)
    );
    assert!(String::from_utf8_lossy(&show.stderr).contains("ambiguous"));
}

#[test]
fn task_lookup_rejects_path_traversal_ids() {
    let temp = setup_repo();
    let repo = temp.path();
    assert_success(
        &maestro(repo, &["task", "create", "First task"]),
        &["task", "create", "First task"],
    );

    let show = maestro(repo, &["task", "show", "../task-001"]);
    assert_failure(&show, &["task", "show", "../task-001"]);
    assert!(stderr(&show).contains("invalid task id"));

    let nested_show = maestro(repo, &["task", "show", "task-001/sub"]);
    assert_failure(&nested_show, &["task", "show", "task-001/sub"]);
    assert!(stderr(&nested_show).contains("invalid task id"));
}

#[test]
fn task_lookup_rejects_symlinked_task_dirs() {
    let temp = setup_repo();
    let repo = temp.path();
    assert_success(
        &maestro(repo, &["task", "create", "First task"]),
        &["task", "create", "First task"],
    );
    let task_path = task_yaml_path(repo, "task-001");
    let original_dir = task_path
        .parent()
        .expect("invariant: task yaml should have parent")
        .to_path_buf();
    let external_dir = repo.join("external-task");
    fs::rename(&original_dir, &external_dir).expect("invariant: task dir should be movable");
    unix_fs::symlink(&external_dir, &original_dir)
        .expect("invariant: symlinked task dir should be creatable");

    let show = maestro(repo, &["task", "show", "task-001"]);
    assert_failure(&show, &["task", "show", "task-001"]);
    assert!(stderr(&show).contains("task not found"));
}

#[test]
fn list_supports_basic_output_and_requested_filters() {
    let temp = setup_repo();
    let repo = temp.path();

    // The tasks link to real features; `create --feature` now rejects a dangling ref.
    assert_success(
        &maestro(repo, &["feature", "new", "Billing CSV"]),
        &["feature", "new", "Billing CSV"],
    );
    assert_success(
        &maestro(repo, &["feature", "new", "Other"]),
        &["feature", "new", "Other"],
    );

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
    assert!(all_out.contains("ID\tSTATE\tNEXT\tINSPECT\tTITLE"));
    assert!(all_out.contains("maestro task show task-001"));
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

    assert_success(
        &maestro(repo, &["task", "claim", "task-001"]),
        &["task", "claim", "task-001"],
    );
    assert_success(
        &maestro(
            repo,
            &[
                "task",
                "update",
                "task-001",
                "--summary",
                "progress noted",
                "--claim",
                "partial implementation",
            ],
        ),
        &[
            "task",
            "update",
            "task-001",
            "--summary",
            "progress noted",
            "--claim",
            "partial implementation",
        ],
    );
    let watch = maestro(repo, &["task", "list", "--watch", "--interval", "0"]);
    assert_success(&watch, &["task", "list", "--watch", "--interval", "0"]);
    let watch_out = stdout(&watch);
    assert!(watch_out.contains("scheduler: 1 agents active"));
    // The watch groups by the feature's human title (resolved from the registry),
    // falling back to the raw id only for dangling refs — now that the feature exists.
    assert!(watch_out.contains("Billing CSV"));
    assert!(watch_out.contains("~ Task A"));
    assert!(watch_out.contains("in-progress (maestro)"));
    assert!(watch_out.contains("! Task B"));
    assert!(watch_out.contains("blocked by task-001"));

    let task_watch = maestro(repo, &["task", "watch", "task-001", "--interval", "0"]);
    assert_success(
        &task_watch,
        &["task", "watch", "task-001", "--interval", "0"],
    );
    let task_watch_out = stdout(&task_watch);
    assert!(task_watch_out.contains("~ Task A"));
    assert!(!task_watch_out.contains("Task B"));

    let watch_feature = maestro(
        repo,
        &[
            "task",
            "list",
            "--watch",
            "--feature",
            "billing-csv",
            "--interval",
            "0",
        ],
    );
    assert_success(
        &watch_feature,
        &[
            "task",
            "list",
            "--watch",
            "--feature",
            "billing-csv",
            "--interval",
            "0",
        ],
    );
    let watch_feature_out = stdout(&watch_feature);
    assert!(watch_feature_out.contains("~ Task A"));
    assert!(watch_feature_out.contains("! Task B"));
    assert!(!watch_feature_out.contains("Task C"));

    let snapshot = maestro(repo, &["watch", "snapshot"]);
    assert_success(&snapshot, &["watch", "snapshot"]);
    let snapshot_out = stdout(&snapshot);
    assert!(snapshot_out.contains("scheduler: 1 agents active"));
    assert!(snapshot_out.contains("~ Task A"));
    assert!(snapshot_out.contains("! Task B"));
}

#[test]
fn task_create_never_reissues_an_archived_id() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "Live task"]),
        &["task", "create", "Live task"],
    );

    // An archived task-005 still owns its id; the next create must skip past it
    // (L6a union scan), not collide by reissuing 005 or 002.
    fs::create_dir_all(repo.join(".maestro/archive/tasks/task-005"))
        .expect("invariant: archive tasks dir should be creatable");

    let created = stdout(&maestro(repo, &["task", "create", "Next task"]));
    assert!(
        created.contains("task-006"),
        "expected task-006, got: {created}"
    );
    assert!(!repo.join(".maestro/tasks/task-005").exists());
}

#[test]
fn archive_moves_terminal_tasks_and_enforces_guards() {
    let temp = setup_repo();
    let repo = temp.path();

    // task-001 stays live; archiving a live task is refused (only done tasks archive).
    assert_success(
        &maestro(repo, &["task", "create", "Keeper"]),
        &["task", "create", "Keeper"],
    );
    let live = maestro(repo, &["task", "archive", "task-001"]);
    assert_failure(&live, &["task", "archive", "task-001"]);
    assert!(stderr(&live).contains("not done"));
    assert!(stderr(&live).contains("blocked: task is not done"));
    assert!(stderr(&live).contains("finish first: maestro task complete task-001"));

    // task-002 is abandoned (terminal) and thus archive-eligible.
    assert_success(
        &maestro(repo, &["task", "create", "Done"]),
        &["task", "create", "Done"],
    );
    assert_success(
        &maestro(repo, &["task", "abandon", "task-002", "--reason", "nope"]),
        &["task", "abandon", "task-002", "--reason", "nope"],
    );

    // L6c: a live task (task-001) blocked by task-002 makes the archive refuse,
    // naming the referrer.
    assert_success(
        &maestro(
            repo,
            &[
                "task", "block", "task-001", "--reason", "needs 2", "--by", "task-002",
            ],
        ),
        &[
            "task", "block", "task-001", "--reason", "needs 2", "--by", "task-002",
        ],
    );
    let referenced = maestro(repo, &["task", "archive", "task-002"]);
    assert_failure(&referenced, &["task", "archive", "task-002"]);
    let referenced_err = stderr(&referenced);
    assert!(referenced_err.contains("blocked: live task still references this task"));
    assert!(referenced_err.contains("task-001"));
    // The remedy is the working one (unblock the referrer); the dead "archive the
    // referrer first" detour is gone -- a live referrer can never be archived.
    assert!(referenced_err.contains("maestro task unblock task-001"));
    assert!(!referenced_err.contains("archive task-001 first"));

    // Clearing the blocker unblocks the archive.
    assert_success(
        &maestro(
            repo,
            &["task", "unblock", "task-001", "--blocker", "blk-001"],
        ),
        &["task", "unblock", "task-001", "--blocker", "blk-001"],
    );

    // --dry-run previews without moving.
    let preview = stdout(&maestro(
        repo,
        &["task", "archive", "task-002", "--dry-run"],
    ));
    assert!(preview.contains("would archive task-002"));
    assert!(preview.contains("writes: none"));
    assert!(repo.join(".maestro/tasks/task-002-done").exists());

    // The real archive moves it to the sibling tree.
    let archived = stdout(&maestro(repo, &["task", "archive", "task-002"]));
    assert!(archived.contains("archived task-002"));
    assert!(archived.contains("restore: maestro task unarchive task-002"));
    assert!(archived.contains("next: maestro status"));
    assert!(!repo.join(".maestro/tasks/task-002-done").exists());
    assert!(repo.join(".maestro/archive/tasks/task-002-done").exists());

    // Default list hides it; `--all` reads the archive; `show` falls through (L6b).
    assert!(!stdout(&maestro(repo, &["task", "list"])).contains("task-002"));
    assert!(stdout(&maestro(repo, &["task", "list", "--all"])).contains("task-002"));
    assert!(stdout(&maestro(repo, &["task", "show", "task-002"])).contains("task-002"));

    // Idempotent: re-archiving an archived task is a no-op at exit 0.
    let again = maestro(repo, &["task", "archive", "task-002"]);
    assert_success(&again, &["task", "archive", "task-002"]);
    assert!(stdout(&again).contains("already archived"));

    // unarchive restores it to the live tree.
    let restored = stdout(&maestro(repo, &["task", "unarchive", "task-002"]));
    assert!(restored.contains("unarchived task-002"));
    assert!(restored.contains("archive again: maestro task archive task-002"));
    assert!(restored.contains("next: maestro status"));
    assert!(repo.join(".maestro/tasks/task-002-done").exists());
    assert!(!repo.join(".maestro/archive/tasks/task-002-done").exists());
}

#[test]
fn list_hides_terminal_tasks_until_all_is_passed() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "Live task"]),
        &["task", "create", "Live task"],
    );
    assert_success(
        &maestro(repo, &["task", "create", "Done task"]),
        &["task", "create", "Done task"],
    );
    assert_success(
        &maestro(
            repo,
            &["task", "abandon", "task-002", "--reason", "not needed"],
        ),
        &["task", "abandon", "task-002", "--reason", "not needed"],
    );

    // Default list keeps task-002 (abandoned, terminal) off the active set and
    // reports the count behind a parser-skippable hint.
    let default = maestro(repo, &["task", "list"]);
    assert_success(&default, &["task", "list"]);
    let default_out = stdout(&default);
    assert!(default_out.contains("task-001"));
    assert!(!default_out.contains("task-002"));
    assert!(default_out.contains("# 1 terminal task(s) hidden; use --all to include"));

    // `--all` includes the terminal task and drops the hint.
    let all = maestro(repo, &["task", "list", "--all"]);
    assert_success(&all, &["task", "list", "--all"]);
    let all_out = stdout(&all);
    assert!(all_out.contains("task-001"));
    assert!(all_out.contains("task-002"));
    assert!(!all_out.contains("terminal task(s) hidden"));
}

#[test]
fn list_all_marks_archived_rows_distinct_from_live_terminal() {
    let temp = setup_repo();
    let repo = temp.path();

    // task-001: abandoned but left live (a live-terminal row).
    // task-002: abandoned then archived (an archived row, same terminal state).
    for title in ["Live terminal", "To archive"] {
        assert_success(
            &maestro(repo, &["task", "create", title]),
            &["task", "create", title],
        );
    }
    for id in ["task-001", "task-002"] {
        assert_success(
            &maestro(repo, &["task", "abandon", id, "--reason", "done"]),
            &["task", "abandon", id, "--reason", "done"],
        );
    }
    assert_success(
        &maestro(repo, &["task", "archive", "task-002"]),
        &["task", "archive", "task-002"],
    );

    // Under --all both terminal rows appear, but only the archived one is marked
    // so an archived task is distinguishable from a live-terminal one (#3).
    let all_out = stdout(&maestro(repo, &["task", "list", "--all"]));
    let row = |id: &str| {
        all_out
            .lines()
            .find(|l| l.starts_with(id))
            .unwrap_or_else(|| panic!("{id} row present in --all output:\n{all_out}"))
            .to_string()
    };
    assert!(
        !row("task-001").contains("(archived)"),
        "{}",
        row("task-001")
    );
    assert!(
        row("task-002").contains("(archived)"),
        "{}",
        row("task-002")
    );
}

#[test]
fn set_on_a_settled_task_refuses_the_link_change_before_writing_checks() {
    let temp = setup_repo();
    let repo = temp.path();

    // task-001 is created (draft, no checks) then abandoned: settled, but never
    // accepted so its acceptance stays unlocked — the state where set_checks
    // would otherwise write before set_feature's settled guard fires.
    assert_success(
        &maestro(repo, &["task", "create", "Dead end"]),
        &["task", "create", "Dead end"],
    );
    assert_success(
        &maestro(
            repo,
            &["task", "abandon", "task-001", "--reason", "scrapped"],
        ),
        &["task", "abandon", "task-001", "--reason", "scrapped"],
    );

    // A combined `--check --feature` set must fail fast on the settled task.
    let args = &[
        "task",
        "set",
        "task-001",
        "--check",
        "must not persist",
        "--feature",
        "billing",
    ];
    let set = maestro(repo, args);
    assert_failure(&set, args);
    assert!(stderr(&set).contains("settled history"));

    // The refused set wrote no check: acceptance carries nothing from it.
    let acceptance = task_yaml_path(repo, "task-001")
        .parent()
        .expect("invariant: task path should have a directory")
        .join("acceptance.yaml");
    if acceptance.exists() {
        let raw =
            fs::read_to_string(&acceptance).expect("invariant: acceptance.yaml should be readable");
        assert!(
            !raw.contains("must not persist"),
            "a refused set must not persist its checks: {raw}"
        );
    }
}

#[test]
fn set_check_rejects_an_empty_value_so_it_cannot_satisfy_the_acceptance_gate() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "Empty-check probe"]),
        &["task", "create", "Empty-check probe"],
    );

    // A `--check ''` whose value is empty must be refused: stored verbatim it
    // would have list length 1 and so satisfy the standalone >=1-check
    // acceptance gate while carrying no contract.
    let args = &["task", "set", "task-001", "--check", ""];
    let set = maestro(repo, args);
    assert_failure(&set, args);
    assert!(stderr(&set).contains("check cannot be empty"));

    // The refused set wrote nothing, so the standalone-checks gate still
    // refuses accept — the empty check never satisfies it.
    assert_success(
        &maestro(repo, &["task", "explore", "task-001"]),
        &["task", "explore", "task-001"],
    );
    let accept = maestro(repo, &["task", "accept", "task-001"]);
    assert_failure(&accept, &["task", "accept", "task-001"]);
    assert!(stderr(&accept).contains("has no checks"));
}

#[test]
fn accept_on_a_terminal_task_reports_the_terminal_state_not_a_dead_end_add_check_remedy() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "Doomed standalone"]),
        &["task", "create", "Doomed standalone"],
    );
    assert_success(
        &maestro(
            repo,
            &["task", "reject", "task-001", "--reason", "out of scope"],
        ),
        &["task", "reject", "task-001", "--reason", "out of scope"],
    );

    // The task is terminal (rejected) and has no checks. accept must surface the
    // real, actionable blocker -- a terminal task cannot transition -- not the
    // add-check remedy, which is a dead end: adding a check still cannot move a
    // terminal task to ready, so the state gate must be evaluated before the
    // content gate.
    let accept = maestro(repo, &["task", "accept", "task-001"]);
    assert_failure(&accept, &["task", "accept", "task-001"]);
    let message = stderr(&accept);
    assert!(
        message.contains("terminal state"),
        "expected the terminal-state error, got: {message}"
    );
    assert!(
        !message.contains("has no checks"),
        "accept on a terminal task must not hand the dead-end add-check remedy: {message}"
    );
}

#[test]
fn set_check_rejects_a_terminal_task_whose_checks_are_settled_history() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "Doomed"]),
        &["task", "create", "Doomed"],
    );
    assert_success(
        &maestro(
            repo,
            &["task", "reject", "task-001", "--reason", "out of scope"],
        ),
        &["task", "reject", "task-001", "--reason", "out of scope"],
    );

    // A rejected task is terminal but never accepted (acceptance_locked is false),
    // so it slips past the lock guard. Editing its checks must still be refused --
    // they are settled history.
    let args = &["task", "set", "task-001", "--check", "too late"];
    let set = maestro(repo, args);
    assert_failure(&set, args);
    let message = stderr(&set);
    assert!(
        message.contains("settled history"),
        "expected the terminal settled-history guard, got: {message}"
    );
}

#[test]
fn set_check_on_a_previously_accepted_terminal_task_reports_settled_history_not_the_lock() {
    let temp = setup_repo();
    let repo = temp.path();

    // Drive the task to accepted (acceptance_locked = true), then reject it: it is
    // now terminal AND acceptance-locked. Editing its checks must report the
    // terminal settled-history reason, not "acceptance is locked ... after accept",
    // which would falsely imply the block is tied to a still-active accepted
    // contract. The terminal guard must be evaluated before the lock guard.
    for args in [
        vec!["task", "create", "Was accepted"],
        vec!["task", "explore", "task-001"],
        vec!["task", "set", "task-001", "--check", "build passes"],
        vec!["task", "accept", "task-001"],
        vec!["task", "reject", "task-001", "--reason", "out of scope"],
    ] {
        assert_success(&maestro(repo, &args), &args);
    }

    let args = &["task", "set", "task-001", "--check", "too late"];
    let set = maestro(repo, args);
    assert_failure(&set, args);
    let message = stderr(&set);
    assert!(
        message.contains("settled history"),
        "expected the terminal settled-history guard, got: {message}"
    );
    assert!(
        !message.contains("acceptance is locked"),
        "a terminal task must not report the acceptance lock (the terminal reason is the accurate one): {message}"
    );
}

#[test]
fn set_check_honors_an_on_disk_acceptance_lock_even_when_the_task_snapshot_is_stale() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "Race probe"]),
        &["task", "create", "Race probe"],
    );

    // Simulate the accept/set_checks race: a concurrent `accept` freezes the
    // contract on disk (writes locked_by into acceptance.yaml) AFTER a racing
    // `set_checks` has already loaded an unlocked task.yaml snapshot. The task
    // stays draft (acceptance_locked = false), so the snapshot guard does not
    // fire; only re-reading the acceptance file's own lock marker catches it.
    let acceptance = task_yaml_path(repo, "task-001")
        .parent()
        .expect("invariant: task path should have a directory")
        .join("acceptance.yaml");
    fs::write(
        &acceptance,
        "schema_version: maestro.acceptance.v1\ntask: task-001\nchecks: []\nlocked_by: maestro\nlocked_at: now\n",
    )
    .expect("invariant: acceptance.yaml should be writable");

    let args = &[
        "task",
        "set",
        "task-001",
        "--check",
        "must not clobber the frozen contract",
    ];
    let set = maestro(repo, args);
    assert_failure(&set, args);
    assert!(
        stderr(&set).contains("acceptance is locked"),
        "set_checks must refuse to overwrite a contract already frozen on disk: {}",
        stderr(&set)
    );

    // The refused set left the frozen contract intact (no clobber).
    let raw =
        fs::read_to_string(&acceptance).expect("invariant: acceptance.yaml should be readable");
    assert!(
        raw.contains("locked_by: maestro") && !raw.contains("must not clobber"),
        "the frozen contract must survive the refused set: {raw}"
    );
}

#[test]
fn complete_on_a_pre_claim_task_points_at_claim_not_a_dead_end() {
    let temp = setup_repo();
    let repo = temp.path();

    for args in [
        vec!["task", "create", "Ship it"],
        vec!["task", "explore", "task-001"],
        vec!["task", "set", "task-001", "--check", "build passes"],
        vec!["task", "accept", "task-001"],
    ] {
        assert_success(&maestro(repo, &args), &args);
    }

    // task-001 is ready but never claimed. Completing it must point at `claim` (the
    // get-to-in_progress verb), not the generic "cannot transition" dead end.
    let complete_args = &[
        "task",
        "complete",
        "task-001",
        "--summary",
        "did it",
        "--claim",
        "build passes",
    ];
    let complete = maestro(repo, complete_args);
    assert_failure(&complete, complete_args);
    let message = stderr(&complete);
    assert!(
        message.contains("maestro task claim task-001"),
        "expected the claim remedy, got: {message}"
    );
    assert!(
        !message.contains("cannot transition"),
        "expected the actionable claim remedy, not the generic catch-all: {message}"
    );
}

#[test]
fn task_create_rejects_an_empty_or_whitespace_title() {
    let temp = setup_repo();
    let repo = temp.path();

    // Sibling create verbs (feature new / decision new) reject a blank title;
    // task create must too, instead of writing a task with a meaningless label.
    for title in ["", "   "] {
        let create = maestro(repo, &["task", "create", title]);
        assert_failure(&create, &["task", "create", title]);
        assert!(
            stderr(&create).contains("title must not be empty"),
            "unexpected error for {title:?}: {}",
            stderr(&create)
        );
    }
}

#[test]
fn task_block_rejects_an_empty_or_whitespace_reason() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "blocked"]),
        &["task", "create", "blocked"],
    );
    // The sibling claim/check/complete verbs all reject a blank value; block must
    // too, rather than persist a dangling-colon blank-reason blocker.
    for reason in ["", "   "] {
        let block = maestro(
            repo,
            &[
                "task", "block", "task-001", "--reason", reason, "--by", "task-002",
            ],
        );
        assert_failure(&block, &["task", "block", "--reason", reason]);
        assert!(
            stderr(&block).contains("`--reason` must not be empty"),
            "unexpected error for {reason:?}: {}",
            stderr(&block)
        );
    }
}

#[test]
fn task_reject_abandon_supersede_reject_an_empty_or_whitespace_reason() {
    let temp = setup_repo();
    let repo = temp.path();

    // `block --reason` already guards blank; reject/abandon/supersede are its
    // missed peers -- terminal, audited transitions where a blank reason would
    // leave a permanent, un-amendable record with no explanation. The guard fires
    // before any state change, so the draft tasks survive both iterations.
    for args in [
        vec!["task", "create", "reject target"],
        vec!["task", "create", "abandon target"],
        vec!["task", "create", "supersede target"],
        vec!["task", "create", "supersede by"],
    ] {
        assert_success(&maestro(repo, &args), &args);
    }

    for reason in ["", "   "] {
        let reject = maestro(repo, &["task", "reject", "task-001", "--reason", reason]);
        assert_failure(&reject, &["task", "reject", "--reason", reason]);
        assert!(
            stderr(&reject).contains("needs an audited reason")
                && stderr(&reject).contains("reason: --reason is empty"),
            "reject {reason:?}: {}",
            stderr(&reject)
        );

        let abandon = maestro(repo, &["task", "abandon", "task-002", "--reason", reason]);
        assert_failure(&abandon, &["task", "abandon", "--reason", reason]);
        assert!(
            stderr(&abandon).contains("needs an audited reason")
                && stderr(&abandon).contains("reason: --reason is empty"),
            "abandon {reason:?}: {}",
            stderr(&abandon)
        );

        let supersede = maestro(
            repo,
            &[
                "task",
                "supersede",
                "task-003",
                "--by",
                "task-004",
                "--reason",
                reason,
            ],
        );
        assert_failure(&supersede, &["task", "supersede", "--reason", reason]);
        assert!(
            stderr(&supersede).contains("needs an audited reason")
                && stderr(&supersede).contains("reason: --reason is empty"),
            "supersede {reason:?}: {}",
            stderr(&supersede)
        );
    }
}

#[test]
fn task_update_with_no_fields_shows_worked_examples_like_task_set() {
    let temp = setup_repo();
    let repo = temp.path();
    assert_success(
        &maestro(repo, &["task", "create", "needs an update"]),
        &["task", "create", "needs an update"],
    );

    // `task set` teaches the exact invocation on its no-args error; `task update`,
    // its sibling, must too rather than dead-end with a bare one-liner.
    let update = maestro(repo, &["task", "update", "task-001"]);
    assert_failure(&update, &["task", "update", "task-001"]);
    let message = stderr(&update);
    assert!(
        message.contains("maestro task update task-001 --summary"),
        "expected a worked --summary example: {message}"
    );
    assert!(
        message.contains("maestro task update task-001 --claim"),
        "expected a worked --claim example: {message}"
    );
}

#[test]
fn event_create_rejects_an_empty_or_whitespace_claim() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "proofed"]),
        &["task", "create", "proofed"],
    );
    // `task complete --claim ""`/`task update --claim ""` are both refused; the
    // event verb that records the same proof artifact must not accept a blank one.
    for claim in ["", "   "] {
        let event = maestro(
            repo,
            &["event", "create", "--task-id", "task-001", "--claim", claim],
        );
        assert_failure(&event, &["event", "create", "--claim", claim]);
        assert!(
            stderr(&event).contains("`--claim` must not be empty"),
            "unexpected error for {claim:?}: {}",
            stderr(&event)
        );
    }
}

#[test]
fn task_update_rejects_an_empty_claim_so_no_blank_proof_is_recorded() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "Empty-claim probe"]),
        &["task", "create", "Empty-claim probe"],
    );
    assert_success(
        &maestro(repo, &["task", "set", "task-001", "--check", "builds"]),
        &["task", "set", "task-001", "--check", "builds"],
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
        &maestro(repo, &["task", "claim", "task-001"]),
        &["task", "claim", "task-001"],
    );

    let history_len = |repo: &Path| {
        task_yaml(repo, "task-001")["state_history"]
            .as_sequence()
            .expect("invariant: state_history should be an array")
            .len()
    };
    let before = history_len(repo);

    // A `--claim ''` is meaningless: a claim is the proof a later `task verify`
    // checks against, so a blank one must be refused and nothing recorded.
    let args = &["task", "update", "task-001", "--claim", ""];
    let update = maestro(repo, args);
    assert_failure(&update, args);
    assert!(stderr(&update).contains("`--claim` must not be empty"));

    // The refused update appended no history entry.
    assert_eq!(history_len(repo), before);
}

#[test]
fn task_block_is_refused_on_a_done_task_so_no_open_blocker_is_baked_in() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "Abandoned probe"]),
        &["task", "create", "Abandoned probe"],
    );
    assert_success(
        &maestro(
            repo,
            &["task", "abandon", "task-001", "--reason", "scrapped"],
        ),
        &["task", "abandon", "task-001", "--reason", "scrapped"],
    );

    // Block alone must not bypass the terminal guard the 5 sibling verbs honor:
    // a finished task cannot take an open blocker (e.g. "abandoned / blocked").
    let args = &[
        "task",
        "block",
        "task-001",
        "--reason",
        "needs dep",
        "--by",
        "task-002",
    ];
    let block = maestro(repo, args);
    assert_failure(&block, args);
    assert!(stderr(&block).contains("cannot block task-001 — done"));

    // No blocker was written onto the done task.
    let doc = task_yaml(repo, "task-001");
    let blockers = doc["blockers"].as_sequence();
    assert!(
        blockers.map(|b| b.is_empty()).unwrap_or(true),
        "a refused block must not persist a blocker: {doc:?}"
    );
}

#[test]
fn task_supersede_by_itself_is_refused_so_no_self_reference_is_recorded() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "Self-supersede probe"]),
        &["task", "create", "Self-supersede probe"],
    );

    // `--by` naming the task itself would record a corrupt superseded_by: self.
    let args = &[
        "task",
        "supersede",
        "task-001",
        "--by",
        "task-001",
        "--reason",
        "oops",
    ];
    let supersede = maestro(repo, args);
    assert_failure(&supersede, args);
    assert!(stderr(&supersede).contains("cannot supersede task-001 by itself"));

    // The task stays in its prior state with no superseded_by ref.
    let doc = task_yaml(repo, "task-001");
    assert_eq!(doc["state"], Value::String("draft".to_string()));
    assert!(doc.get("superseded_by").is_none() || doc["superseded_by"].is_null());
}

#[test]
fn task_unblock_is_refused_on_an_already_resolved_blocker() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "Double-unblock probe"]),
        &["task", "create", "Double-unblock probe"],
    );
    assert_success(
        &maestro(
            repo,
            &[
                "task", "block", "task-001", "--reason", "waiting", "--by", "task-999",
            ],
        ),
        &[
            "task", "block", "task-001", "--reason", "waiting", "--by", "task-999",
        ],
    );
    assert_success(
        &maestro(
            repo,
            &["task", "unblock", "task-001", "--blocker", "blk-001"],
        ),
        &["task", "unblock", "task-001", "--blocker", "blk-001"],
    );

    // Capture the resolved state after the first (legitimate) unblock.
    let after_first = task_yaml(repo, "task-001");
    let resolved_at = after_first["blockers"][0]["resolved_at"]
        .as_str()
        .expect("invariant: first unblock should set resolved_at")
        .to_string();
    let history_len = after_first["state_history"]
        .as_sequence()
        .expect("invariant: state_history should be an array")
        .len();

    // A second unblock of the same blocker must be refused, not silently
    // overwrite the original resolved_at or append a duplicate history entry.
    let args = &["task", "unblock", "task-001", "--blocker", "blk-001"];
    let second = maestro(repo, args);
    assert_failure(&second, args);
    assert!(stderr(&second).contains("blocker blk-001 is already resolved"));

    let after_second = task_yaml(repo, "task-001");
    assert_eq!(
        after_second["blockers"][0]["resolved_at"].as_str(),
        Some(resolved_at.as_str()),
        "the original resolved_at must be preserved"
    );
    assert_eq!(
        after_second["state_history"]
            .as_sequence()
            .expect("invariant: state_history should be an array")
            .len(),
        history_len,
        "a refused unblock must not append history"
    );
}

#[test]
fn read_verbs_do_not_scaffold_the_tasks_dir_but_create_still_does() {
    // R30: a pure inspect (`task list`/`task doctor`) must leave disk untouched,
    // matching feature/decision/query; only a mutator (`create`) may scaffold.
    let temp = setup_repo();
    let repo = temp.path();
    let tasks_dir = repo.join(".maestro/tasks");
    assert!(!tasks_dir.exists(), "setup must start without a tasks dir");

    let list = maestro(repo, &["task", "list"]);
    assert_success(&list, &["task", "list"]);
    assert!(stdout(&list).contains("no tasks found"));
    assert!(
        !tasks_dir.exists(),
        "`task list` must not scaffold .maestro/tasks"
    );

    let doctor = maestro(repo, &["task", "doctor"]);
    assert_success(&doctor, &["task", "doctor"]);
    assert!(
        !tasks_dir.exists(),
        "`task doctor` must not scaffold .maestro/tasks"
    );

    let create = maestro(repo, &["task", "create", "first task"]);
    assert_success(&create, &["task", "create"]);
    assert!(
        tasks_dir.exists(),
        "`task create` must still create .maestro/tasks on first write"
    );
}

#[test]
fn task_show_marks_an_archived_task_and_leaves_a_live_one_unmarked() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "Doomed"]),
        &["task", "create", "Doomed"],
    );
    assert_success(
        &maestro(
            repo,
            &["task", "reject", "task-001", "--reason", "out of scope"],
        ),
        &["task", "reject", "task-001"],
    );
    assert_success(
        &maestro(repo, &["task", "archive", "task-001"]),
        &["task", "archive", "task-001"],
    );

    // task show reads through the archive so a historical id still renders; it must
    // disclose the archived state (like feature show) so it is not mistaken for live.
    let archived = maestro(repo, &["task", "show", "task-001"]);
    assert_success(&archived, &["task", "show", "task-001"]);
    assert!(
        stdout(&archived).contains("archived: true"),
        "an archived task show must mark it: {}",
        stdout(&archived)
    );

    // A live task must NOT carry the marker, so it really distinguishes the trees.
    assert_success(
        &maestro(repo, &["task", "create", "Live one"]),
        &["task", "create", "Live one"],
    );
    let live = maestro(repo, &["task", "show", "task-002"]);
    assert_success(&live, &["task", "show", "task-002"]);
    assert!(
        !stdout(&live).contains("archived: true"),
        "a live task show must not be marked archived: {}",
        stdout(&live)
    );
}

#[test]
fn forward_verbs_on_a_verified_task_point_at_a_follow_up_not_a_bare_dead_end() {
    let temp = setup_repo();
    let repo = temp.path();

    for args in [
        vec!["task", "create", "Done deal"],
        vec!["task", "set", "task-001", "--check", "build passes"],
        vec!["task", "explore", "task-001"],
        vec!["task", "accept", "task-001"],
        vec!["task", "claim", "task-001"],
        vec![
            "task",
            "complete",
            "task-001",
            "--summary",
            "did it",
            "--claim",
            "build passes",
            "--proof",
            "build passes",
        ],
    ] {
        assert_success(&maestro(repo, &args), &args);
    }

    // Verified is a settled success terminus; a forward verb (claim/complete) means
    // new work, so the error must point at a follow-up task, not the bare
    // "cannot transition" catch-all dead end.
    for verb in [
        vec!["task", "claim", "task-001"],
        vec![
            "task",
            "complete",
            "task-001",
            "--summary",
            "more",
            "--claim",
            "x",
        ],
    ] {
        let out = maestro(repo, &verb);
        assert_failure(&out, &verb);
        let message = stderr(&out);
        assert!(
            message.contains("maestro task create"),
            "expected the follow-up remedy for {verb:?}: {message}"
        );
        assert!(
            !message.contains("cannot transition"),
            "must not be the bare catch-all for {verb:?}: {message}"
        );
    }
}
