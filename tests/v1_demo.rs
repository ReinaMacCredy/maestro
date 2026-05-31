mod support;

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::Value;
use support::TestTempDir;

fn maestro(cwd: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .current_dir(cwd)
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

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("invariant: stdout should be UTF-8")
}

fn run(repo: &Path, args: &[&str]) -> String {
    let output = maestro(repo, args);
    assert_success(&output, args);
    stdout(&output)
}

fn mcp_frame(value: &str) -> Vec<u8> {
    let mut bytes = format!("Content-Length: {}\r\n\r\n", value.len()).into_bytes();
    bytes.extend_from_slice(value.as_bytes());
    bytes
}

fn parse_mcp_frame(bytes: &[u8]) -> Value {
    let raw = String::from_utf8(bytes.to_vec()).expect("invariant: MCP output should be UTF-8");
    let (_, body) = raw
        .split_once("\r\n\r\n")
        .expect("invariant: MCP frame should include header terminator");
    serde_json::from_str(body).expect("invariant: MCP response should be JSON")
}

#[test]
fn v1_demo_runs_core_flow_watch_metrics_query_and_mcp() {
    let temp = TestTempDir::new("maestro-v1-demo");
    let repo = temp.path();
    fs::create_dir(repo.join(".git")).expect("invariant: git marker should be creatable");

    run(repo, &["init", "--yes"]);
    run(repo, &["install", "--agent", "claude"]);
    run(repo, &["install", "--agent", "codex"]);
    run(repo, &["task", "create", "Demo task"]);
    run(repo, &["task", "set", "task-001", "--check", "demo task verified"]);
    run(repo, &["task", "explore", "task-001"]);
    run(repo, &["task", "accept", "task-001"]);
    run(repo, &["task", "claim", "task-001"]);
    run(
        repo,
        &[
            "task",
            "complete",
            "task-001",
            "--summary",
            "done",
            "--claim",
            "implemented demo task",
        ],
    );

    let run_dir = repo.join(".maestro/runs/demo-session");
    fs::create_dir_all(&run_dir).expect("invariant: run dir should be creatable");
    fs::write(
        run_dir.join("events.jsonl"),
        "{\"task_id\":\"task-001\",\"kind\":\"proof\",\"message\":\"implemented demo task\"}\n",
    )
    .expect("invariant: proof event should be writable");
    run(repo, &["task", "verify", "task-001"]);

    let watch = run(repo, &["task", "list", "--watch", "--interval", "1"]);
    assert!(watch.contains("scheduler:"));
    assert!(watch.contains("Demo task"));
    assert!(watch.contains("verified"));

    let metrics = run(repo, &["metrics", "summary"]);
    assert!(metrics.contains("Tasks: 1"));
    assert!(metrics.contains("verified"));

    let proof = run(repo, &["query", "proof", "task-001"]);
    assert!(proof.contains("proof task-001: accepted"));

    let mut child = Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(["mcp", "serve"])
        .current_dir(repo)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("invariant: mcp serve should spawn");
    child
        .stdin
        .as_mut()
        .expect("invariant: stdin should be piped")
        .write_all(&mcp_frame(
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#,
        ))
        .expect("invariant: MCP request should write");
    drop(child.stdin.take());
    let output = child
        .wait_with_output()
        .expect("invariant: mcp serve should finish after stdin closes");
    assert_success(&output, &["mcp", "serve"]);
    let response = parse_mcp_frame(&output.stdout);
    assert!(response["result"]["tools"]
        .as_array()
        .expect("invariant: tools should be an array")
        .iter()
        .any(|tool| tool["name"] == "maestro_metrics_summary"));

    let help = run(repo, &["--help"]);
    for dropped in ["mission", "verdict", "handoff", "policy", "workflow"] {
        assert!(!help.contains(dropped), "help should not expose {dropped}");
    }
}
