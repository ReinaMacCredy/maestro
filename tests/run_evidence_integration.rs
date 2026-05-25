mod support;

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_yaml::Value;
use support::TestTempDir;

fn maestro_record(cwd: &Path, payload: &str) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(["hook", "record"])
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("invariant: compiled maestro binary should be runnable in evidence tests");
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
    let temp_dir = TestTempDir::new("maestro-run-evidence-test");
    fs::create_dir(temp_dir.path().join(".git"))
        .expect("invariant: .git marker should be creatable");
    temp_dir
}

fn record_event(repo: &Path, payload: &str) {
    let output = maestro_record(repo, payload);
    assert!(
        output.status.success(),
        "hook record should exit 0, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn run_evidence(repo: &Path, session: &str) -> Value {
    let raw = fs::read_to_string(
        repo.join(".maestro")
            .join("runs")
            .join(session)
            .join("run_evidence.yaml"),
    )
    .expect("invariant: run_evidence.yaml should be readable");
    serde_yaml::from_str(&raw).expect("invariant: run_evidence.yaml should parse")
}

#[test]
fn stop_record_writes_run_evidence_with_tool_counts_and_duration() {
    let repo = init_repo();
    record_event(
        repo.path(),
        r#"{"session_id":"session-123","event_type":"SessionStart","agent":"codex","task_id":"task-002"}"#,
    );
    record_event(
        repo.path(),
        r#"{"session_id":"session-123","event_type":"UserPromptSubmit"}"#,
    );
    record_event(
        repo.path(),
        r#"{"session_id":"session-123","event_type":"PreToolUse","tool_name":"Bash"}"#,
    );
    record_event(
        repo.path(),
        r#"{"session_id":"session-123","event_type":"PostToolUse","tool_name":"Bash"}"#,
    );
    record_event(
        repo.path(),
        r#"{"session_id":"session-123","event_type":"Stop"}"#,
    );

    let evidence = run_evidence(repo.path(), "session-123");
    assert_eq!(evidence["schema_version"], "maestro.run_evidence.v1");
    assert_eq!(evidence["session_id"], "session-123");
    assert_eq!(evidence["agent"], "codex");
    assert_eq!(evidence["task_id"], "task-002");
    assert_eq!(evidence["tools_used"]["Bash"], 1);
    assert!(evidence["start_at"].as_str().is_some());
    assert!(evidence["end_at"].as_str().is_some());
    assert!(evidence["duration_seconds"].as_u64().is_some());
}

#[test]
fn follow_up_prompts_count_as_human_interventions() {
    let repo = init_repo();
    for payload in [
        r#"{"session_id":"session-prompts","event_type":"SessionStart"}"#,
        r#"{"session_id":"session-prompts","event_type":"UserPromptSubmit"}"#,
        r#"{"session_id":"session-prompts","event_type":"UserPromptSubmit"}"#,
        r#"{"session_id":"session-prompts","event_type":"UserPromptSubmit"}"#,
        r#"{"session_id":"session-prompts","event_type":"Stop"}"#,
    ] {
        record_event(repo.path(), payload);
    }

    let evidence = run_evidence(repo.path(), "session-prompts");
    assert_eq!(evidence["human_interventions"], 2);
}

#[test]
fn evidence_generation_tolerates_invalid_lines_and_missing_fields() {
    let repo = init_repo();
    record_event(
        repo.path(),
        r#"{"session_id":"session-tolerant","event_type":"SessionStart"}"#,
    );
    let events_path = repo
        .path()
        .join(".maestro/runs/session-tolerant/events.jsonl");
    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(&events_path)
        .expect("invariant: events.jsonl should be appendable");
    writeln!(file, "not json").expect("invariant: malformed fixture line should be writable");
    writeln!(
        file,
        r#"{{"event_type":"PostToolUse","tool_name":"Read","ts":"not-a-timestamp"}}"#
    )
    .expect("invariant: incomplete fixture line should be writable");

    record_event(
        repo.path(),
        r#"{"session_id":"session-tolerant","event_type":"Stop"}"#,
    );

    let evidence = run_evidence(repo.path(), "session-tolerant");
    assert_eq!(evidence["schema_version"], "maestro.run_evidence.v1");
    assert_eq!(evidence["session_id"], "session-tolerant");
    assert_eq!(evidence["tools_used"]["Read"], 1);
    assert!(evidence["duration_seconds"].as_u64().is_some());
}
