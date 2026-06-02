mod support;

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use maestro::domain::run;
use maestro::foundation::core::paths::MaestroPaths;
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

#[test]
fn evidence_generation_fails_for_missing_source_log_without_writing_empty_evidence() {
    let repo = init_repo();
    let run_dir = repo.path().join(".maestro/runs/session-missing-source");
    fs::create_dir_all(&run_dir).expect("invariant: run dir should be creatable");
    let paths = MaestroPaths::new(repo.path());

    let error = run::write_evidence_for_session(&paths, "session-missing-source")
        .expect_err("evidence generation should fail when events.jsonl is missing");

    assert!(format!("{error:#}").contains("events.jsonl"));
    assert!(!run_dir.join("run_evidence.yaml").exists());
}

#[test]
fn evidence_regeneration_is_idempotent_for_same_event_log() {
    let repo = init_repo();
    for payload in [
        r#"{"session_id":"session-idempotent","event_type":"SessionStart","agent":"codex"}"#,
        r#"{"session_id":"session-idempotent","event_type":"PostToolUse","tool_name":"Bash"}"#,
        r#"{"session_id":"session-idempotent","event_type":"Stop"}"#,
    ] {
        record_event(repo.path(), payload);
    }
    let evidence_path = repo
        .path()
        .join(".maestro/runs/session-idempotent/run_evidence.yaml");
    let first = fs::read_to_string(&evidence_path).expect("invariant: evidence should be readable");
    let paths = MaestroPaths::new(repo.path());

    run::write_evidence_for_session(&paths, "session-idempotent")
        .expect("invariant: evidence regeneration should succeed");
    let second =
        fs::read_to_string(&evidence_path).expect("invariant: evidence should be readable");
    run::write_evidence_for_session(&paths, "session-idempotent")
        .expect("invariant: evidence regeneration should succeed");
    let third = fs::read_to_string(&evidence_path).expect("invariant: evidence should be readable");

    assert_eq!(first, second);
    assert_eq!(second, third);
}

#[test]
fn evidence_can_be_regenerated_after_stop_when_late_event_is_recorded() {
    let repo = init_repo();
    for payload in [
        r#"{"session_id":"session-late-event","event_type":"SessionStart","agent":"codex"}"#,
        r#"{"session_id":"session-late-event","event_type":"Stop"}"#,
    ] {
        record_event(repo.path(), payload);
    }

    let stop_evidence = run_evidence(repo.path(), "session-late-event");
    assert!(stop_evidence["tools_used"]["Bash"].is_null());

    record_event(
        repo.path(),
        r#"{"session_id":"session-late-event","event_type":"PostToolUse","tool_name":"Bash"}"#,
    );
    let stale_stop_evidence = run_evidence(repo.path(), "session-late-event");
    assert!(stale_stop_evidence["tools_used"]["Bash"].is_null());

    let paths = MaestroPaths::new(repo.path());
    run::write_evidence_for_session(&paths, "session-late-event")
        .expect("invariant: evidence regeneration should succeed after late event");

    let evidence_path = repo
        .path()
        .join(".maestro/runs/session-late-event/run_evidence.yaml");
    let regenerated = run_evidence(repo.path(), "session-late-event");
    assert_eq!(regenerated["tools_used"]["Bash"], 1);
    let first_regeneration =
        fs::read(&evidence_path).expect("invariant: regenerated evidence should be readable");

    run::write_evidence_for_session(&paths, "session-late-event")
        .expect("invariant: no-op evidence regeneration should succeed");

    let second_regeneration =
        fs::read(&evidence_path).expect("invariant: regenerated evidence should be readable");
    assert_eq!(first_regeneration, second_regeneration);
}

#[test]
fn evidence_generation_ignores_partial_trailing_event_line() {
    let repo = init_repo();
    let run_dir = repo.path().join(".maestro/runs/session-partial-read");
    fs::create_dir_all(&run_dir).expect("invariant: run dir should be creatable");
    fs::write(
        run_dir.join("events.jsonl"),
        concat!(
            "{\"event_type\":\"SessionStart\",\"session_id\":\"session-partial-read\",\"agent\":\"codex\",\"ts\":\"2026-05-26T00:00:00Z\"}\n",
            "{\"event_type\":\"PostToolUse\",\"tool_name\":\"Bash\""
        ),
    )
    .expect("invariant: partial event fixture should be writable");
    let paths = MaestroPaths::new(repo.path());

    run::write_evidence_for_session(&paths, "session-partial-read")
        .expect("invariant: evidence generation should tolerate partial trailing lines");

    let evidence = run_evidence(repo.path(), "session-partial-read");
    assert_eq!(evidence["schema_version"], "maestro.run_evidence.v1");
    assert_eq!(evidence["agent"], "codex");
    assert!(evidence["tools_used"]["Bash"].is_null());
}

#[test]
fn stop_record_writes_run_evidence_when_session_id_is_absent() {
    // A hook payload with no session_id lands in the raw `unattributed` run
    // bucket (a real session literally named "unattributed" is disambiguated to
    // `%75nattributed`). The append path and the Stop evidence path must agree on
    // that directory, or the Stop record fails to find its own event log and
    // silently writes no evidence.
    let repo = init_repo();
    record_event(
        repo.path(),
        r#"{"event_type":"SessionStart","agent":"codex"}"#,
    );
    record_event(repo.path(), r#"{"event_type":"Stop"}"#);

    let evidence = run_evidence(repo.path(), "unattributed");
    assert_eq!(evidence["schema_version"], "maestro.run_evidence.v1");
    assert_eq!(evidence["session_id"], "");
    assert_eq!(evidence["agent"], "codex");
}

#[cfg(unix)]
#[test]
fn managed_run_discovery_ignores_symlinked_run_paths() {
    let repo = init_repo();
    let external = TestTempDir::new("maestro-run-evidence-external-runs");
    fs::create_dir_all(external.path().join("run-001"))
        .expect("invariant: external run dir should be creatable");
    fs::write(
        external.path().join("run-001/events.jsonl"),
        "{\"event_type\":\"SessionStart\"}\n",
    )
    .expect("invariant: external events should be writable");
    fs::create_dir_all(repo.path().join(".maestro"))
        .expect("invariant: maestro dir should be creatable");
    std::os::unix::fs::symlink(external.path(), repo.path().join(".maestro/runs"))
        .expect("invariant: symlinked runs dir should be creatable");
    let paths = MaestroPaths::new(repo.path());

    let files = run::managed_event_logs(&paths).expect("invariant: discovery should succeed");

    assert!(files.is_empty());
}
