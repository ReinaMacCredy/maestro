mod support;

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

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

fn init_repo() -> TestTempDir {
    let temp_dir = TestTempDir::new("maestro-hook-record-test");
    fs::create_dir(temp_dir.path().join(".git"))
        .expect("invariant: .git marker should be creatable");
    temp_dir
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
    assert!(events[0]["tool_input_hash"]
        .as_str()
        .expect("invariant: tool_input_hash should be a string")
        .starts_with("sha256:"));
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
