pub mod card_support;
mod support;

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
