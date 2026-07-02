pub mod card_support;
mod support;

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use card_support::{cards_repo, id_by_title};
use maestro::domain::search::transcript::{
    TranscriptConsentRecord, TranscriptConsentScope, TranscriptProvider, TranscriptSegmentInput,
    TranscriptStore,
};
use serde_json::Value;

fn maestro(cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .current_dir(cwd)
        .env("MAESTRO_AGENT", "codex")
        .env("MAESTRO_SESSION_ID", "test-driver")
        .env("MAESTRO_AUTO_UPDATE", "0")
        .env("CODEX_HOME", cwd.join(".codex-test-home"))
        .env("MAESTRO_TRANSCRIPT_HOME", cwd.join(".transcript-test-home"))
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
        .env("MAESTRO_TRANSCRIPT_HOME", cwd.join(".transcript-test-home"))
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

#[test]
fn session_show_does_not_run_archive_readout_by_default() {
    let temp = cards_repo("session-show-archive-readout");
    let repo = temp.path();
    let feature_dir = repo.join(".maestro/cards/archivable-feature");
    fs::create_dir_all(&feature_dir).expect("invariant: feature dir should be creatable");
    fs::write(
        feature_dir.join("card.yaml"),
        "schema_version: maestro.card.v1\nid: archivable-feature\ntype: feature\ntitle: Archivable Feature\nstatus: closed\ncreated_at: \"1\"\nupdated_at: \"1\"\n",
    )
    .expect("invariant: feature card should be writable");
    record(
        repo,
        r#"{"session_id":"sess-archive","event_type":"card_touch","card_id":"archivable-feature"}"#,
    );

    let text = run(repo, &["session", "show", "sess-archive"]);
    assert!(
        !text.contains("[archive]"),
        "session show should leave archive inspection to explicit archive commands:\n{text}"
    );
    assert!(
        repo.join(".maestro/cards/archivable-feature/card.yaml")
            .is_file(),
        "session show must not archive the feature"
    );
}

fn seed_redacted_transcript_store(repo: &Path, session_id: &str) {
    let store = TranscriptStore::new(repo.join(".transcript-test-home"));
    let workspace = repo.display().to_string();
    store
        .grant_consent(TranscriptConsentRecord {
            provider: TranscriptProvider::Codex,
            workspace: workspace.clone(),
            scope: TranscriptConsentScope::Project,
            granted: true,
            reason: Some("test fixture".to_string()),
        })
        .expect("invariant: consent should write");
    for input in [
        TranscriptSegmentInput {
            provider: TranscriptProvider::Codex,
            session_id: session_id.to_string(),
            segment_id: "user-1".to_string(),
            source_kind: "codex_user_message".to_string(),
            workspace: workspace.clone(),
            text: "show the session transcript".to_string(),
            raw_tool_arguments: None,
            raw_tool_output: None,
            raw_reasoning: None,
            raw_environment: None,
        },
        TranscriptSegmentInput {
            provider: TranscriptProvider::Codex,
            session_id: session_id.to_string(),
            segment_id: "assistant-1".to_string(),
            source_kind: "codex_assistant_message".to_string(),
            workspace: workspace.clone(),
            text: "reading the transcript now".to_string(),
            raw_tool_arguments: None,
            raw_tool_output: None,
            raw_reasoning: None,
            raw_environment: None,
        },
        TranscriptSegmentInput {
            provider: TranscriptProvider::Codex,
            session_id: session_id.to_string(),
            segment_id: "tool-1".to_string(),
            source_kind: "codex_tool_call".to_string(),
            workspace: workspace.clone(),
            text: "tool call: exec_command".to_string(),
            raw_tool_arguments: Some("secret=abc".to_string()),
            raw_tool_output: None,
            raw_reasoning: None,
            raw_environment: None,
        },
        TranscriptSegmentInput {
            provider: TranscriptProvider::Codex,
            session_id: session_id.to_string(),
            segment_id: "tool-2".to_string(),
            source_kind: "codex_tool_call".to_string(),
            workspace: workspace.clone(),
            text: "tool call: apply_patch".to_string(),
            raw_tool_arguments: Some("token=def".to_string()),
            raw_tool_output: None,
            raw_reasoning: None,
            raw_environment: None,
        },
    ] {
        store
            .append_redacted_segment(input)
            .expect("invariant: segment should append");
    }
}

#[test]
fn session_show_uses_redacted_transcript_store_as_labeled_backfill() {
    let temp = cards_repo("session-show-transcript-backfill");
    let repo = temp.path();
    seed_redacted_transcript_store(repo, "legacy-sess");

    let text = run(repo, &["session", "show", "legacy-sess"]);
    assert!(text.contains("commands: 2"), "{text}");
    assert!(text.contains("compactions: 0"), "{text}");
    assert!(
        text.contains("activity: ledger + transcript store"),
        "{text}"
    );
    assert!(text.contains("transcript: transcript store"), "{text}");
    assert!(
        !text.contains("redacted transcript store unavailable"),
        "{text}"
    );
    assert!(!text.contains("Transcript:"), "{text}");
    assert!(
        !text.contains("secret=abc") && !text.contains("token=def"),
        "session show must not leak raw transcript input:\n{text}"
    );

    let json_out = run(repo, &["session", "show", "legacy-sess", "--json"]);
    let parsed: Value = serde_json::from_str(&json_out).expect("session JSON should parse");
    assert_eq!(parsed["activity"]["commands"], 2);
    assert_eq!(parsed["activity"]["compactions"], 0);
    assert_eq!(
        parsed["activity"]["counts"]["transcript_command_observed"],
        2
    );
    assert_eq!(parsed["sources"]["activity"], "ledger + transcript store");
    assert_eq!(parsed["sources"]["transcript"], "transcript store");
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
