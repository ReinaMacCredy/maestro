pub mod card_support;
mod support;

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use card_support::{cards_repo, id_by_title};
use serde_json::Value;

fn maestro(cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .current_dir(cwd)
        .env("MAESTRO_AGENT", "codex")
        .env("MAESTRO_SESSION_ID", "test-driver")
        .env("MAESTRO_AUTO_UPDATE", "0")
        .env("CODEX_HOME", cwd.join(".codex-test-home"))
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

fn record(cwd: &Path, payload: &str) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(["hook", "record"])
        .current_dir(cwd)
        .env("MAESTRO_AUTO_UPDATE", "0")
        .env("CODEX_HOME", cwd.join(".codex-test-home"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("invariant: compiled maestro binary should run hook record");
    child
        .stdin
        .as_mut()
        .expect("invariant: stdin should be piped")
        .write_all(payload.as_bytes())
        .expect("invariant: payload should write");
    let output = child
        .wait_with_output()
        .expect("invariant: hook record should finish");
    assert!(
        output.status.success(),
        "hook record failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn session_show_renders_joined_text_and_json_readouts() {
    let temp = cards_repo("session-show-readout");
    let repo = temp.path();

    run(
        repo,
        &[
            "task",
            "create",
            "Inspect session story",
            "--check",
            "session show reads proof",
        ],
    );
    let task_id = id_by_title(repo, "Inspect session story");

    record(
        repo,
        &format!(r#"{{"session_id":"sess-a","event_type":"card_touch","card_id":"{task_id}"}}"#),
    );
    record(
        repo,
        &format!(
            r#"{{"session_id":"sess-a","event_type":"PostToolUse","tool_name":"Bash","task_id":"{task_id}","status":"ok","duration_ms":42,"tool_input":{{"command":"cargo test -- api_key=top-secret"}}}}"#
        ),
    );
    run(
        repo,
        &[
            "event",
            "create",
            "--task-id",
            &task_id,
            "--run",
            "sess-a",
            "--claim",
            "GREEN session show reads proof",
            "--message",
            "proof summary",
        ],
    );

    let text = run(repo, &["session", "show", "sess-a"]);
    assert!(text.contains("Session: sess-a"), "{text}");
    assert!(text.contains("Inspect session story"), "{text}");
    assert!(text.contains("commands: 1"), "{text}");
    assert!(text.contains("proof events: 1"), "{text}");
    assert!(text.contains("activity: ledger"), "{text}");
    assert!(text.contains("lifecycle: runs"), "{text}");
    assert!(text.contains("transcript: unavailable"), "{text}");
    assert!(
        !text.contains("top-secret") && !text.contains("api_key"),
        "session show must not leak raw tool input:\n{text}"
    );

    let json_out = run(repo, &["session", "show", "sess-a", "--json"]);
    let parsed: Value = serde_json::from_str(&json_out).expect("session JSON should parse");
    assert_eq!(parsed["session_id"], "sess-a");
    assert_eq!(parsed["activity"]["counts"]["command_finished"], 1);
    assert_eq!(parsed["activity"]["commands"], 1);
    assert_eq!(parsed["proof"]["events"], 1);
    assert_eq!(parsed["tasks"][0]["id"], task_id);
    assert_eq!(parsed["sources"]["activity"], "ledger");
    assert_eq!(parsed["sources"]["transcript"], "unavailable");
    let raw = serde_json::to_string(&parsed).expect("session JSON should serialize");
    assert!(!raw.contains("top-secret") && !raw.contains("api_key"));
}

fn seed_codex_transcript(repo: &Path, session_id: &str) {
    let dir = repo.join(".codex-test-home/sessions/2026/07/01");
    fs::create_dir_all(&dir).expect("invariant: transcript dir should be creatable");
    fs::write(
        dir.join(format!("rollout-2026-07-01T00-00-00-{session_id}.jsonl")),
        concat!(
            "{\"timestamp\":\"2026-07-01T00:00:00Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"user\",\"content\":[{\"type\":\"input_text\",\"text\":\"# AGENTS.md instructions\\n<INSTRUCTIONS>ignore</INSTRUCTIONS>\\n<environment_context>ignore</environment_context>\"}]}}\n",
            "{\"timestamp\":\"2026-07-01T00:00:01Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"user\",\"content\":[{\"type\":\"input_text\",\"text\":\"show the session transcript\"}]}}\n",
            "{\"timestamp\":\"2026-07-01T00:00:02Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"reading the transcript now\"}]}}\n",
            "{\"type\":\"response_item\",\"payload\":{\"type\":\"function_call\",\"name\":\"exec_command\",\"arguments\":\"secret=abc\"}}\n",
            "{\"type\":\"response_item\",\"payload\":{\"type\":\"function_call\",\"name\":\"apply_patch\",\"arguments\":\"token=def\"}}\n",
            "{\"type\":\"compacted\",\"payload\":{\"note\":\"private\"}}\n"
        ),
    )
    .expect("invariant: transcript fixture should write");
}

#[test]
fn session_show_uses_local_codex_transcript_as_labeled_backfill() {
    let temp = cards_repo("session-show-transcript-backfill");
    let repo = temp.path();
    seed_codex_transcript(repo, "legacy-sess");

    let text = run(repo, &["session", "show", "legacy-sess"]);
    assert!(text.contains("commands: 2"), "{text}");
    assert!(text.contains("compactions: 1"), "{text}");
    assert!(
        text.contains("activity: ledger + transcript backfill"),
        "{text}"
    );
    assert!(text.contains("transcript: backfill"), "{text}");
    assert!(!text.contains("transcript backfill unavailable"), "{text}");
    assert!(!text.contains("Transcript:"), "{text}");
    assert!(
        !text.contains("secret=abc") && !text.contains("token=def"),
        "session show must not leak raw transcript input:\n{text}"
    );

    let json_out = run(repo, &["session", "show", "legacy-sess", "--json"]);
    let parsed: Value = serde_json::from_str(&json_out).expect("session JSON should parse");
    assert_eq!(parsed["activity"]["commands"], 2);
    assert_eq!(parsed["activity"]["compactions"], 1);
    assert_eq!(
        parsed["activity"]["counts"]["transcript_command_observed"],
        2
    );
    assert_eq!(
        parsed["activity"]["counts"]["transcript_compaction_observed"],
        1
    );
    assert_eq!(
        parsed["sources"]["activity"],
        "ledger + transcript backfill"
    );
    assert_eq!(parsed["sources"]["transcript"], "backfill");
    assert!(
        parsed["gaps"].as_array().is_some_and(Vec::is_empty),
        "{parsed}"
    );
    let raw = serde_json::to_string(&parsed).expect("session JSON should serialize");
    assert!(!raw.contains("secret=abc") && !raw.contains("token=def"));
    assert!(parsed.get("transcript").is_none(), "{parsed}");

    let transcript_text = run(repo, &["session", "show", "legacy-sess", "--transcript"]);
    assert!(transcript_text.contains("Transcript:"), "{transcript_text}");
    assert!(
        transcript_text.contains("- user:\n  show the session transcript"),
        "{transcript_text}"
    );
    assert!(
        transcript_text.contains("- assistant:\n  reading the transcript now"),
        "{transcript_text}"
    );
    assert!(
        transcript_text.contains("- tool: exec_command"),
        "{transcript_text}"
    );
    assert!(
        transcript_text.contains("- tool: apply_patch"),
        "{transcript_text}"
    );
    assert!(
        transcript_text.contains("- compaction observed"),
        "{transcript_text}"
    );
    assert!(
        !transcript_text.contains("# AGENTS.md instructions")
            && !transcript_text.contains("secret=abc")
            && !transcript_text.contains("token=def"),
        "transcript output must omit bootstrap context and raw tool input:\n{transcript_text}"
    );

    let transcript_json = run(
        repo,
        &["session", "show", "legacy-sess", "--json", "--transcript"],
    );
    let parsed: Value = serde_json::from_str(&transcript_json).expect("session JSON should parse");
    let entries = parsed["transcript"]["entries"]
        .as_array()
        .expect("transcript entries should be present");
    assert!(entries.iter().any(|entry| entry["role"] == "user"));
    assert!(entries.iter().any(|entry| entry["role"] == "assistant"));
    assert!(entries.iter().any(|entry| entry["kind"] == "tool_call"));
    assert!(entries.iter().any(|entry| entry["kind"] == "compaction"));
    let raw = serde_json::to_string(&parsed).expect("session JSON should serialize");
    assert!(!raw.contains("secret=abc") && !raw.contains("token=def"));
}

#[test]
fn session_show_resolves_progress_task_ids_through_task_store() {
    let temp = cards_repo("session-show-progress-task");
    let repo = temp.path();

    let task_id = run(repo, &["task", "add", "Resolve progress task", "--id-only"])
        .trim()
        .to_string();
    record(
        repo,
        &format!(
            r#"{{"session_id":"progress-sess","event_type":"card_touch","card_id":"{task_id}"}}"#
        ),
    );

    let text = run(repo, &["session", "show", "progress-sess"]);
    assert!(text.contains(&task_id), "{text}");
    assert!(text.contains("Resolve progress task"), "{text}");
    assert!(text.contains("[ready]"), "{text}");
    assert!(!text.contains("(not in store)"), "{text}");
    assert!(!text.contains("[unknown]"), "{text}");

    let json_out = run(repo, &["session", "show", "progress-sess", "--json"]);
    let parsed: Value = serde_json::from_str(&json_out).expect("session JSON should parse");
    let task = &parsed["tasks"][0];
    assert_eq!(task["id"], task_id);
    assert_eq!(task["title"], "Resolve progress task");
    assert_eq!(task["status"], "ready");
    assert_eq!(task["type"], "task");
}
