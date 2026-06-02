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
    assert!(
        !doc["updated_at"]
            .as_str()
            .expect("invariant: updated_at should be a string")
            .is_empty()
    );
}

#[test]
fn claim_from_draft_advances_to_in_progress() {
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
    assert_success(&claim, &["task", "claim", "task-001"]);

    let task = task_yaml(repo, "task-001");
    assert_eq!(task["state"], Value::String("in_progress".to_string()));
    assert_eq!(task["acceptance_locked"], Value::Bool(true));
    let history = task["state_history"]
        .as_sequence()
        .expect("invariant: state_history should be present");
    assert_eq!(history.len(), 4);
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
    assert!(stderr(&referenced).contains("task-001"));

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
    assert!(repo.join(".maestro/tasks/task-002-done").exists());

    // The real archive moves it to the sibling tree.
    let archived = stdout(&maestro(repo, &["task", "archive", "task-002"]));
    assert!(archived.contains("archived task-002"));
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
