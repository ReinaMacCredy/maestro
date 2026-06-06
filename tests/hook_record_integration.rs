mod support;

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;

use git2::{Repository, Signature};
use maestro::hooks::event::run_dir_name;
use serde_json::Value;
use support::TestTempDir;

fn maestro_record(cwd: &Path, payload: &str) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(["hook", "record"])
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("invariant: compiled maestro binary should be runnable in hook tests");
    child
        .stdin
        .as_mut()
        .expect("invariant: piped stdin should be available")
        .write_all(payload.as_bytes())
        .expect("invariant: hook payload should be writable to stdin");
    child
        .wait_with_output()
        .expect("invariant: maestro hook record should return process output")
}

fn maestro(cwd: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("invariant: compiled maestro binary should be runnable in hook tests")
}

fn maestro_with_env(cwd: &Path, args: &[&str], envs: &[(&str, &str)]) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_maestro"));
    command.args(args).current_dir(cwd);
    for (key, value) in envs {
        command.env(key, value);
    }
    command
        .output()
        .expect("invariant: compiled maestro binary should be runnable in hook tests")
}

fn maestro_without_session_env(cwd: &Path, args: &[&str]) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_maestro"));
    command.args(args).current_dir(cwd);
    for key in [
        "MAESTRO_SESSION_ID",
        "MAESTRO_RUN_ID",
        "CODEX_SESSION_ID",
        "CLAUDE_SESSION_ID",
        "CLAUDECODE_SESSION_ID",
    ] {
        command.env_remove(key);
    }
    command
        .output()
        .expect("invariant: compiled maestro binary should be runnable in hook tests")
}

fn init_repo() -> TestTempDir {
    let temp_dir = TestTempDir::new("maestro-hook-record-test");
    fs::create_dir(temp_dir.path().join(".git"))
        .expect("invariant: .git marker should be creatable");
    temp_dir
}

fn init_repo_with_head() -> (TestTempDir, String) {
    let temp_dir = TestTempDir::new("maestro-hook-record-git-test");
    let repository =
        Repository::init(temp_dir.path()).expect("invariant: git repository should initialize");
    fs::write(temp_dir.path().join("README.md"), "fixture\n")
        .expect("invariant: fixture file should be writable");

    let mut index = repository
        .index()
        .expect("invariant: git index should be readable");
    index
        .add_path(Path::new("README.md"))
        .expect("invariant: fixture file should be addable");
    index.write().expect("invariant: git index should write");
    let tree_id = index
        .write_tree()
        .expect("invariant: git tree should write");
    let tree = repository
        .find_tree(tree_id)
        .expect("invariant: git tree should be readable");
    let signature = Signature::now("Maestro Test", "maestro@example.invalid")
        .expect("invariant: git signature should be constructible");
    let commit = repository
        .commit(
            Some("HEAD"),
            &signature,
            &signature,
            "initial fixture",
            &tree,
            &[],
        )
        .expect("invariant: git commit should write")
        .to_string();

    (temp_dir, commit)
}

fn commit_change(repo: &Path) -> String {
    let repository = Repository::open(repo).expect("invariant: git repository should open");
    fs::write(repo.join("CHANGELOG.md"), "second fixture\n")
        .expect("invariant: second fixture file should be writable");

    let mut index = repository
        .index()
        .expect("invariant: git index should be readable");
    index
        .add_path(Path::new("CHANGELOG.md"))
        .expect("invariant: second fixture file should be addable");
    index.write().expect("invariant: git index should write");
    let tree_id = index
        .write_tree()
        .expect("invariant: git tree should write");
    let tree = repository
        .find_tree(tree_id)
        .expect("invariant: git tree should be readable");
    let parent = repository
        .head()
        .and_then(|head| head.peel_to_commit())
        .expect("invariant: git HEAD commit should be readable");
    let signature = Signature::now("Maestro Test", "maestro@example.invalid")
        .expect("invariant: git signature should be constructible");

    repository
        .commit(
            Some("HEAD"),
            &signature,
            &signature,
            "second fixture",
            &tree,
            &[&parent],
        )
        .expect("invariant: second git commit should write")
        .to_string()
}

fn read_events(repo: &Path, session: &str) -> Vec<Value> {
    let path = repo
        .join(".maestro")
        .join("runs")
        .join(session)
        .join("events.jsonl");
    let raw = fs::read_to_string(path).expect("invariant: events.jsonl should be readable");
    raw.lines()
        .map(|line| serde_json::from_str(line).expect("invariant: event line should be valid JSON"))
        .collect()
}

#[test]
fn start_and_stop_events_capture_current_commit() {
    let (repo, start_commit) = init_repo_with_head();
    let start_output = maestro_record(
        repo.path(),
        r#"{"session_id":"session-git","event_type":"SessionStart"}"#,
    );
    assert!(
        start_output.status.success(),
        "hook record failed for SessionStart\nstderr:\n{}",
        String::from_utf8_lossy(&start_output.stderr)
    );

    let stop_commit = commit_change(repo.path());
    assert_ne!(start_commit, stop_commit);
    let stop_output = maestro_record(
        repo.path(),
        r#"{"session_id":"session-git","event_type":"Stop"}"#,
    );
    assert!(
        stop_output.status.success(),
        "hook record failed for Stop\nstderr:\n{}",
        String::from_utf8_lossy(&stop_output.stderr)
    );

    let events = read_events(repo.path(), "session-git");
    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["event_type"], "SessionStart");
    assert_eq!(events[0]["commit"], start_commit);
    assert_eq!(events[1]["event_type"], "Stop");
    assert_eq!(events[1]["commit"], stop_commit);
}

#[test]
fn valid_event_writes_schema_and_event_type_for_session() {
    let repo = init_repo();
    let output = maestro_record(
        repo.path(),
        r#"{"session_id":"session-123","hook_event_name":"SessionStart","agent":"codex"}"#,
    );

    assert!(output.status.success());
    let events = read_events(repo.path(), "session-123");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["schema_version"], "maestro.event.v1");
    assert_eq!(events[0]["event_type"], "SessionStart");
    assert_eq!(events[0]["session_id"], "session-123");
    let timestamp = events[0]["ts"]
        .as_str()
        .expect("invariant: normalized hook event should include a timestamp");
    assert!(timestamp.contains('T'));
    assert!(timestamp.ends_with('Z'));
}

#[test]
fn unrecognized_event_type_is_reported_and_not_recorded() {
    let repo = init_repo();
    let output = maestro_record(
        repo.path(),
        r#"{"session_id":"session-unknown","hook_event_name":"NotARealEvent"}"#,
    );

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("ignored unrecognized event type `NotARealEvent`"),
        "hook record should explain the drop instead of staying silent: {stderr}"
    );
    let events_path = repo
        .path()
        .join(".maestro/runs")
        .join(run_dir_name("session-unknown"))
        .join("events.jsonl");
    assert!(
        !events_path.exists(),
        "an unrecognized event must not be appended to the run log"
    );
}

#[test]
fn missing_session_id_writes_unattributed_run() {
    let repo = init_repo();
    let output = maestro_record(repo.path(), r#"{"event_type":"UserPromptSubmit"}"#);

    assert!(output.status.success());
    let events = read_events(repo.path(), "unattributed");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["event_type"], "UserPromptSubmit");
    assert!(events[0].get("session_id").is_none());
}

#[test]
fn unsafe_session_ids_do_not_collide_after_sanitization() {
    let repo = init_repo();
    let first = run_dir_name("alpha/beta");
    let second = run_dir_name("alpha?beta");
    let literal_encoded = run_dir_name("alpha%2Fbeta");

    assert_ne!(first, second);
    assert_ne!(first, literal_encoded);
    assert!(
        maestro_record(
            repo.path(),
            r#"{"session_id":"alpha/beta","event_type":"UserPromptSubmit"}"#,
        )
        .status
        .success()
    );
    assert!(
        maestro_record(
            repo.path(),
            r#"{"session_id":"alpha?beta","event_type":"UserPromptSubmit"}"#,
        )
        .status
        .success()
    );
    assert!(
        maestro_record(
            repo.path(),
            r#"{"session_id":"alpha%2Fbeta","event_type":"UserPromptSubmit"}"#,
        )
        .status
        .success()
    );

    assert_eq!(read_events(repo.path(), &first).len(), 1);
    assert_eq!(read_events(repo.path(), &second).len(), 1);
    assert_eq!(read_events(repo.path(), &literal_encoded).len(), 1);
}

#[test]
fn literal_unattributed_session_does_not_share_missing_session_bucket() {
    let repo = init_repo();
    let literal = run_dir_name("unattributed");

    assert_ne!(literal, "unattributed");
    assert!(
        maestro_record(repo.path(), r#"{"event_type":"UserPromptSubmit"}"#)
            .status
            .success()
    );
    assert!(
        maestro_record(
            repo.path(),
            r#"{"session_id":"unattributed","event_type":"UserPromptSubmit"}"#,
        )
        .status
        .success()
    );

    assert_eq!(read_events(repo.path(), "unattributed").len(), 1);
    assert_eq!(read_events(repo.path(), &literal).len(), 1);
}

#[test]
fn malformed_json_exits_successfully_without_writing_events() {
    let repo = init_repo();
    let output = maestro_record(repo.path(), "{not json");

    assert!(output.status.success());
    assert!(!repo.path().join(".maestro/runs").exists());
}

#[test]
fn pre_tool_use_hashes_tool_input_without_persisting_raw_content() {
    let repo = init_repo();
    let output = maestro_record(
        repo.path(),
        r#"{
            "session_id":"session-privacy",
            "event_type":"PreToolUse",
            "tool_name":"Bash",
            "tool_input":{"command":"echo raw-secret-value"}
        }"#,
    );

    assert!(output.status.success());
    let path = repo
        .path()
        .join(".maestro/runs/session-privacy/events.jsonl");
    let raw = fs::read_to_string(path).expect("invariant: events.jsonl should be readable");
    assert!(!raw.contains("raw-secret-value"));

    let events = read_events(repo.path(), "session-privacy");
    assert_eq!(events[0]["event_type"], "PreToolUse");
    assert!(events[0].get("tool_input").is_none());
    assert!(
        events[0]["tool_input_hash"]
            .as_str()
            .expect("invariant: tool_input_hash should be a string")
            .starts_with("sha256:")
    );
}

#[test]
fn shared_hook_events_are_accepted() {
    let repo = init_repo();
    for event_type in [
        "SessionStart",
        "UserPromptSubmit",
        "PreToolUse",
        "PermissionRequest",
        "PostToolUse",
        "Stop",
    ] {
        let output = maestro_record(
            repo.path(),
            &format!(r#"{{"session_id":"session-all","event_type":"{event_type}"}}"#),
        );
        assert!(output.status.success());
    }

    let events = read_events(repo.path(), "session-all");
    let event_types = events
        .iter()
        .map(|event| {
            event["event_type"]
                .as_str()
                .expect("invariant: event_type should be a string")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        event_types,
        [
            "SessionStart",
            "UserPromptSubmit",
            "PreToolUse",
            "PermissionRequest",
            "PostToolUse",
            "Stop"
        ]
    );
}

#[test]
fn skill_activation_event_is_normalized() {
    let repo = init_repo();
    let output = maestro_record(
        repo.path(),
        r#"{"session_id":"session-skill","event_type":"SkillActivation","skill_name":"maestro-task","activation_mode":"agent_selected"}"#,
    );

    assert!(output.status.success());
    let events = read_events(repo.path(), "session-skill");
    assert_eq!(events[0]["event_type"], "skill_activation");
    assert_eq!(events[0]["skill_name"], "maestro-task");
    assert_eq!(events[0]["activation_mode"], "agent_selected");
}

#[test]
fn hook_record_flags_print_ack_and_use_session_or_cli_run_dirs() {
    let repo = init_repo();
    let explicit = maestro(
        repo.path(),
        &[
            "hook",
            "record",
            "--event",
            "skill_activation",
            "--skill",
            "qa-baseline",
            "--session",
            "session-flags",
        ],
    );
    assert!(
        explicit.status.success(),
        "hook record flags failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&explicit.stdout),
        String::from_utf8_lossy(&explicit.stderr)
    );
    let explicit_out = String::from_utf8_lossy(&explicit.stdout);
    assert!(
        explicit_out.contains("recorded skill_activation (qa-baseline) -> runs/session-flags"),
        "{explicit_out}"
    );
    let explicit_events = read_events(repo.path(), "session-flags");
    assert_eq!(explicit_events[0]["event_type"], "skill_activation");
    assert_eq!(explicit_events[0]["skill_name"], "qa-baseline");

    let env_attributed = maestro_with_env(
        repo.path(),
        &["hook", "record", "--event", "UserPromptSubmit"],
        &[("MAESTRO_SESSION_ID", "session-env")],
    );
    assert!(env_attributed.status.success());
    assert!(
        String::from_utf8_lossy(&env_attributed.stdout)
            .contains("recorded UserPromptSubmit -> runs/session-env")
    );
    let env_events = read_events(repo.path(), "session-env");
    assert_eq!(env_events[0]["event_type"], "UserPromptSubmit");

    let cli_attributed = maestro_without_session_env(
        repo.path(),
        &["hook", "record", "--event", "UserPromptSubmit"],
    );
    assert!(cli_attributed.status.success());
    let cli_out = String::from_utf8_lossy(&cli_attributed.stdout);
    let run_dir = cli_out
        .split("runs/")
        .nth(1)
        .expect("invariant: ack should include run dir")
        .trim();
    assert!(run_dir.starts_with("cli-"), "{cli_out}");
    let cli_events = read_events(repo.path(), run_dir);
    assert_eq!(cli_events[0]["event_type"], "UserPromptSubmit");
    assert!(!repo.path().join(".maestro/runs/unattributed").exists());
}

#[test]
fn append_after_partial_trailing_line_preserves_next_jsonl_event() {
    let repo = init_repo();
    let events_path = repo
        .path()
        .join(".maestro/runs/session-partial/events.jsonl");
    fs::create_dir_all(
        events_path
            .parent()
            .expect("invariant: event path has parent"),
    )
    .expect("invariant: event parent should be creatable");
    fs::write(&events_path, r#"{"event_type":"partial""#)
        .expect("invariant: partial fixture should be writable");

    let output = maestro_record(
        repo.path(),
        r#"{"session_id":"session-partial","event_type":"UserPromptSubmit"}"#,
    );

    assert!(output.status.success());
    let raw = fs::read_to_string(events_path).expect("invariant: events.jsonl should be readable");
    let valid_events = raw
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect::<Vec<_>>();
    assert_eq!(valid_events.len(), 1);
    assert_eq!(valid_events[0]["event_type"], "UserPromptSubmit");
}

#[test]
fn concurrent_same_session_records_append_complete_json_lines() {
    let repo = init_repo();
    let repo_path = repo.path().to_path_buf();
    let handles = (0..12)
        .map(|index| {
            let repo_path = repo_path.clone();
            thread::spawn(move || {
                maestro_record(
                    &repo_path,
                    &format!(
                        r#"{{"session_id":"session-concurrent","event_type":"PostToolUse","tool_name":"Tool{index}"}}"#
                    ),
                )
            })
        })
        .collect::<Vec<_>>();

    for handle in handles {
        let output = handle
            .join()
            .expect("invariant: hook record worker should not panic");
        assert!(
            output.status.success(),
            "hook record failed\nstderr:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let events_path = repo
        .path()
        .join(".maestro/runs/session-concurrent/events.jsonl");
    let raw = fs::read_to_string(events_path).expect("invariant: events.jsonl should be readable");
    let events = raw
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("event line should be valid JSON"))
        .collect::<Vec<_>>();
    assert_eq!(events.len(), 12);
    assert!(
        events
            .iter()
            .all(|event| event["event_type"] == "PostToolUse")
    );
    let actual_tools = events
        .iter()
        .map(|event| {
            event["tool_name"]
                .as_str()
                .expect("event should include tool_name")
        })
        .fold(BTreeMap::<String, usize>::new(), |mut counts, tool| {
            *counts.entry(tool.to_string()).or_default() += 1;
            counts
        });
    let expected_tools = (0..12)
        .map(|index| (format!("Tool{index}"), 1_usize))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(actual_tools, expected_tools);
}

#[test]
fn stop_evidence_failure_warns_without_failing_hook() {
    let repo = init_repo();
    let evidence_path = repo
        .path()
        .join(".maestro/runs/session-warning/run_evidence.yaml");
    fs::create_dir_all(&evidence_path)
        .expect("invariant: blocking evidence directory should be creatable");

    let output = maestro_record(
        repo.path(),
        r#"{"session_id":"session-warning","event_type":"Stop"}"#,
    );

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("failed to write run evidence"));
    let events = read_events(repo.path(), "session-warning");
    assert_eq!(events[0]["event_type"], "Stop");
}

#[cfg(unix)]
#[test]
fn hook_record_refuses_symlinked_run_artifacts_without_failing_adapter() {
    let repo = init_repo();
    let external = TestTempDir::new("maestro-hook-external");
    fs::create_dir_all(repo.path().join(".maestro"))
        .expect("invariant: maestro dir should be creatable");
    std::os::unix::fs::symlink(external.path(), repo.path().join(".maestro/runs"))
        .expect("invariant: symlinked runs dir should be creatable");

    let output = maestro_record(
        repo.path(),
        r#"{"session_id":"session-symlink","event_type":"SessionStart"}"#,
    );

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("symlink"));
    assert!(
        !external
            .path()
            .join("session-symlink/events.jsonl")
            .exists()
    );
}
