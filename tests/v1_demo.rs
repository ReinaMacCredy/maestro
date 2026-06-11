pub mod card_support;
mod support;

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use card_support::id_by_title;
use serde_json::Value;
use support::TestTempDir;

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

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("invariant: stdout should be UTF-8")
}

fn run_with_env(repo: &Path, args: &[&str], envs: &[(&str, &str)]) -> String {
    let output = maestro_with_env(repo, args, envs);
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
fn v1_demo_runs_core_flow_watch_query_and_mcp() {
    let temp = TestTempDir::new("maestro-v1-demo");
    let repo = temp.path();
    let home = TestTempDir::new("maestro-v1-demo-home");
    let home_var = home.path().to_string_lossy().into_owned();
    let envs = [("HOME", home_var.as_str())];
    fs::create_dir(repo.join(".git")).expect("invariant: git marker should be creatable");

    run_with_env(repo, &["init", "--yes"], &envs);
    run_with_env(repo, &["harness", "set", "--claims-only"], &envs);
    run_with_env(repo, &["install", "--agent", "claude"], &envs);
    run_with_env(repo, &["install", "--agent", "codex"], &envs);
    run_with_env(repo, &["task", "create", "Demo task"], &envs);
    let id = id_by_title(repo, "Demo task");
    run_with_env(
        repo,
        &["task", "set", &id, "--check", "demo task verified"],
        &envs,
    );
    run_with_env(repo, &["task", "explore", &id], &envs);
    run_with_env(repo, &["task", "accept", &id], &envs);
    run_with_env(repo, &["task", "claim", &id], &envs);
    run_with_env(
        repo,
        &[
            "task",
            "complete",
            &id,
            "--summary",
            "done",
            "--claim",
            "implemented demo task",
            "--proof",
            "implemented demo task",
        ],
        &envs,
    );

    let watch = run_with_env(repo, &["task", "list", "--watch", "--interval", "1"], &envs);
    assert!(watch.contains("scheduler:"));
    assert!(watch.contains("Demo task"));
    assert!(watch.contains("verified"));

    let proof = run_with_env(repo, &["query", "proof", &id], &envs);
    assert!(proof.contains(&format!("proof {id}: accepted")));

    let mut command = Command::new(env!("CARGO_BIN_EXE_maestro"));
    command
        .args(["mcp", "serve"])
        .current_dir(repo)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in envs {
        command.env(key, value);
    }
    let mut child = command.spawn().expect("invariant: mcp serve should spawn");
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
    assert!(
        response["result"]["tools"]
            .as_array()
            .expect("invariant: tools should be an array")
            .iter()
            .any(|tool| tool["name"] == "maestro_status")
    );

    let help = run_with_env(repo, &["--help"], &envs);
    for dropped in ["mission", "verdict", "policy", "workflow"] {
        assert!(!help.contains(dropped), "help should not expose {dropped}");
    }
}
