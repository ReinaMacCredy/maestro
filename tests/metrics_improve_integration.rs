mod support;

use std::fs;
use std::io::Write;
use std::os::unix::fs as unix_fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value as JsonValue;
use serde_yaml::Value as YamlValue;
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

fn setup_repo(prefix: &str) -> TestTempDir {
    let temp = TestTempDir::new(prefix);
    fs::create_dir(temp.path().join(".git")).expect("invariant: .git marker should be creatable");
    assert_success(
        &maestro(temp.path(), &["init", "--yes"]),
        &["init", "--yes"],
    );
    temp
}

fn run_success(repo: &Path, args: &[&str]) -> String {
    let output = maestro(repo, args);
    assert_success(&output, args);
    stdout(&output)
}

fn mcp_frames(values: &[&str]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for value in values {
        bytes.extend_from_slice(format!("Content-Length: {}\r\n\r\n", value.len()).as_bytes());
        bytes.extend_from_slice(value.as_bytes());
    }
    bytes
}

fn parse_mcp_frames(bytes: &[u8]) -> Vec<JsonValue> {
    let raw = String::from_utf8(bytes.to_vec()).expect("invariant: MCP output should be UTF-8");
    let mut remaining = raw.as_str();
    let mut frames = Vec::new();
    while !remaining.is_empty() {
        let (header, rest) = remaining
            .split_once("\r\n\r\n")
            .expect("invariant: MCP frame should include header terminator");
        let length = header
            .strip_prefix("Content-Length: ")
            .expect("invariant: MCP frame should include content length")
            .parse::<usize>()
            .expect("invariant: MCP content length should parse");
        let (body, next) = rest.split_at(length);
        frames.push(serde_json::from_str(body).expect("invariant: MCP response JSON"));
        remaining = next;
    }
    frames
}

fn task_dir(repo: &Path, id: &str) -> PathBuf {
    let prefix = format!("{id}-");
    for entry in
        fs::read_dir(repo.join(".maestro/tasks")).expect("invariant: tasks dir should be readable")
    {
        let entry = entry.expect("invariant: task entry should be readable");
        let name = entry
            .file_name()
            .to_str()
            .expect("invariant: task dir name should be UTF-8")
            .to_string();
        if name.starts_with(&prefix) {
            return entry.path();
        }
    }
    panic!("invariant: task directory should exist for {id}");
}

fn create_task(repo: &Path, title: &str) {
    assert_success(
        &maestro(repo, &["task", "create", title]),
        &["task", "create", title],
    );
}

fn mark_verified(repo: &Path, id: &str, domain: &str, created_at: &str, verified_at: &str) {
    let path = task_dir(repo, id).join("task.yaml");
    let raw = fs::read_to_string(&path).expect("invariant: task.yaml should be readable");
    let mut task: YamlValue =
        serde_yaml::from_str(&raw).expect("invariant: task.yaml should parse");
    task["state"] = YamlValue::String("verified".to_string());
    task["created_at"] = YamlValue::String(created_at.to_string());
    task["verification"]["verified_at"] = YamlValue::String(verified_at.to_string());
    task["affected_areas"] = YamlValue::Sequence(vec![YamlValue::String(domain.to_string())]);
    fs::write(
        &path,
        serde_yaml::to_string(&task).expect("invariant: task should serialize"),
    )
    .expect("invariant: task.yaml should be writable");
}

#[test]
fn metrics_summary_reads_tasks_and_run_evidence_without_cache() {
    let temp = setup_repo("maestro-metrics-summary");
    let repo = temp.path();

    create_task(repo, "Verified export task");
    create_task(repo, "In progress parser task");
    mark_verified(repo, "task-001", "billing", "100", "2380");
    assert_success(
        &maestro(repo, &["task", "explore", "task-002"]),
        &["task", "explore", "task-002"],
    );
    assert_success(
        &maestro(repo, &["task", "accept", "task-002"]),
        &["task", "accept", "task-002"],
    );
    assert_success(
        &maestro(repo, &["task", "claim", "task-002"]),
        &["task", "claim", "task-002"],
    );

    let run_dir = repo.join(".maestro/runs/session-metrics");
    fs::create_dir_all(&run_dir).expect("invariant: run dir should be creatable");
    fs::write(
        run_dir.join("run_evidence.yaml"),
        concat!(
            "schema_version: maestro.run_evidence.v1\n",
            "session_id: session-metrics\n",
            "agent: codex_cli\n",
            "task_id: task-001\n",
            "duration_seconds: 2460\n",
            "human_interventions: 2\n"
        ),
    )
    .expect("invariant: run evidence should be writable");
    let bad_run_dir = repo.join(".maestro/runs/session-bad");
    fs::create_dir_all(&bad_run_dir).expect("invariant: bad run dir should be creatable");
    fs::write(bad_run_dir.join("run_evidence.yaml"), "not: [valid")
        .expect("invariant: bad run evidence should be writable");

    let out = run_success(repo, &["metrics", "summary"]);
    assert!(out.contains("Tasks: 2 (1 verified, 0 needs_verification, 1 in_progress)"));
    assert!(out.contains("Avg time-to-verify: 38 min"));
    assert!(out.contains("codex_cli: 1 tasks, 41 min avg"));
    assert!(out.contains("Interventions: 1.0 per task"));
    assert!(out.contains("Skipped run evidence: 1"));
    assert!(!repo.join(".maestro/cache").exists());
}

#[test]
fn improve_detects_all_rule_based_backlog_proposals_and_applies_one() {
    let temp = setup_repo("maestro-improve-rules");
    let repo = temp.path();

    for index in 1..=7 {
        create_task(repo, &format!("Task {index}"));
    }
    mark_verified(repo, "task-001", "billing", "0", "10000");
    mark_verified(repo, "task-002", "billing", "10", "10010");
    for index in 3..=7 {
        mark_verified(repo, &format!("task-{index:03}"), "general", "0", "100");
    }

    fs::write(
        repo.join(".maestro/harness/harness.yml"),
        concat!(
            "schema_version: maestro.harness.v1\n",
            "stack:\n",
            "  kind: generic\n",
            "  detected_by: []\n",
            "  verify:\n",
            "    - API_KEY=sk_live_xxx true\n"
        ),
    )
    .expect("invariant: harness should be writable");
    let verify = maestro(repo, &["task", "verify", "task-003"]);
    assert!(
        !verify.status.success(),
        "task verify should fail for missing proof but still write verification.json"
    );
    fs::write(
        repo.join(".maestro/harness/harness.yml"),
        concat!(
            "schema_version: maestro.harness.v1\n",
            "stack:\n",
            "  kind: generic\n",
            "  detected_by: []\n",
            "  verify: []\n"
        ),
    )
    .expect("invariant: harness should be writable");
    assert_success(
        &maestro(
            repo,
            &[
                "task",
                "block",
                "task-004",
                "--reason",
                "waiting for staging credentials",
            ],
        ),
        &[
            "task",
            "block",
            "task-004",
            "--reason",
            "waiting for staging credentials",
        ],
    );
    assert_success(
        &maestro(
            repo,
            &[
                "task",
                "block",
                "task-005",
                "--reason",
                "waiting for staging credentials",
            ],
        ),
        &[
            "task",
            "block",
            "task-005",
            "--reason",
            "waiting for staging credentials",
        ],
    );
    fs::write(
        task_dir(repo, "task-006").join("task.md"),
        "Decision: use replay queue for hooks\n",
    )
    .expect("invariant: task markdown should be writable");
    fs::write(
        task_dir(repo, "task-007").join("task.md"),
        "Decision: use replay queue for hooks\n",
    )
    .expect("invariant: task markdown should be writable");

    let run_dir = repo.join(".maestro/runs/session-corrections");
    fs::create_dir_all(&run_dir).expect("invariant: run dir should be creatable");
    fs::write(
        run_dir.join("events.jsonl"),
        concat!(
            "{\"event_type\":\"UserPromptSubmit\",\"prompt\":\"actually use the real task\"}\n",
            "{\"event_type\":\"UserPromptSubmit\",\"prompt\":\"wait, verify this first\"}\n",
            "{\"event_type\":\"UserPromptSubmit\",\"prompt\":\"no keep the scope tight\"}\n",
            "{\"event_type\":\"UserPromptSubmit\",\"prompt\":\"actually this is a long narrative prompt that discusses implementation context without being a compact correction or interruption and should not count as a correction event\"}\n"
        ),
    )
    .expect("invariant: events fixture should be writable");

    let friction = run_success(repo, &["query", "friction"]);
    assert!(friction.contains("user_prompts: 4"));
    assert!(friction.contains("corrections: 3"));

    let out = run_success(repo, &["improve", "list"]);
    assert!(out.contains("recurring_intervention"));
    assert!(out.contains("missing_verification"));
    assert!(out.contains("recurring_blocker"));
    assert!(out.contains("missing_skill"));
    assert!(out.contains("rediscovered_decision"));

    let show = run_success(repo, &["improve", "show", "hb-001"]);
    assert!(show.contains("status: proposed"));
    assert!(show.contains("evidence:"));
    let backlog = fs::read_to_string(repo.join(".maestro/harness/backlog.yaml"))
        .expect("invariant: backlog should be readable");
    assert!(backlog.contains("API_KEY=<redacted>"));
    assert!(!backlog.contains("sk_live_xxx"));

    let apply = run_success(repo, &["improve", "apply", "hb-001"]);
    assert!(apply.contains("applied hb-001"));
    let applied = run_success(repo, &["improve", "show", "hb-001"]);
    assert!(applied.contains("status: applied"));
}

#[test]
fn mcp_serve_lists_tools_and_calls_metrics_summary_over_stdio() {
    let temp = setup_repo("maestro-mcp-serve");
    let repo = temp.path();
    create_task(repo, "MCP visible task");

    let mut child = Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(["mcp", "serve"])
        .current_dir(repo)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("invariant: compiled maestro binary should run mcp serve");
    let stdin = child
        .stdin
        .as_mut()
        .expect("invariant: mcp stdin should be piped");
    stdin
        .write_all(&mcp_frames(&[
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#,
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"maestro_metrics_summary","arguments":{}}}"#,
        ]))
        .expect("invariant: MCP requests should be writable");
    drop(child.stdin.take());

    let output = child
        .wait_with_output()
        .expect("invariant: mcp serve should return after stdin closes");
    assert_success(&output, &["mcp", "serve"]);

    let lines = parse_mcp_frames(&output.stdout);
    let tools = lines[1]["result"]["tools"]
        .as_array()
        .expect("invariant: tools/list should return an array");
    assert_eq!(tools.len(), 14);
    assert!(tools
        .iter()
        .any(|tool| tool["name"] == "maestro_task_claim"));
    assert!(tools
        .iter()
        .any(|tool| tool["name"] == "maestro_metrics_summary"));
    assert!(lines[2]["result"]["content"][0]["text"]
        .as_str()
        .expect("invariant: tool response should contain text")
        .contains("Tasks: 1"));
}

#[test]
fn mcp_serve_handles_json_rpc_batches_over_stdio_frames() {
    let temp = setup_repo("maestro-mcp-batch");
    let repo = temp.path();
    create_task(repo, "MCP batch task");

    let mut child = Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(["mcp", "serve"])
        .current_dir(repo)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("invariant: compiled maestro binary should run mcp serve");
    child
        .stdin
        .as_mut()
        .expect("invariant: mcp stdin should be piped")
        .write_all(&mcp_frames(&[concat!(
            "[",
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\",\"params\":{}},",
            "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\",\"params\":{}},",
            "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"maestro_metrics_summary\",\"arguments\":{}}}",
            "]"
        )]))
        .expect("invariant: MCP requests should be writable");
    drop(child.stdin.take());

    let output = child
        .wait_with_output()
        .expect("invariant: mcp serve should return after stdin closes");
    assert_success(&output, &["mcp", "serve"]);
    let frames = parse_mcp_frames(&output.stdout);
    assert_eq!(frames.len(), 1);
    let batch = frames[0]
        .as_array()
        .expect("invariant: batch response should be an array");
    assert_eq!(batch.len(), 2);
    assert_eq!(batch[0]["id"], 1);
    assert_eq!(batch[1]["id"], 2);
}

#[test]
fn mcp_serve_uses_content_length_framing() {
    let temp = setup_repo("maestro-mcp-serve");
    let repo = temp.path();
    create_task(repo, "MCP visible task");

    let mut child = Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(["mcp", "serve"])
        .current_dir(repo)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("invariant: compiled maestro binary should run mcp serve");
    let stdin = child
        .stdin
        .as_mut()
        .expect("invariant: mcp stdin should be piped");
    stdin
        .write_all(&mcp_frames(&[
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        ]))
        .expect("invariant: MCP requests should be writable");
    drop(child.stdin.take());

    let output = child
        .wait_with_output()
        .expect("invariant: mcp serve should return after stdin closes");
    assert_success(&output, &["mcp", "serve"]);
    assert!(String::from_utf8(output.stdout.clone())
        .expect("invariant: MCP output should be UTF-8")
        .starts_with("Content-Length: "));
    let frames = parse_mcp_frames(&output.stdout);
    assert_eq!(frames[0]["id"], 1);
}

#[test]
fn mcp_serve_reports_invalid_requests_without_running_tools() {
    let temp = setup_repo("maestro-mcp-invalid");
    let repo = temp.path();

    let mut child = Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(["mcp", "serve"])
        .current_dir(repo)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("invariant: compiled maestro binary should run mcp serve");
    child
        .stdin
        .as_mut()
        .expect("invariant: mcp stdin should be piped")
        .write_all(&mcp_frames(&[
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{}}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"missing_tool","arguments":{}}}"#,
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"maestro_task_complete","arguments":{"id":"task-001","summary":"done","claims":["one","two"]}}}"#,
            r#"{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}"#,
        ]))
        .expect("invariant: MCP requests should be writable");
    drop(child.stdin.take());

    let output = child
        .wait_with_output()
        .expect("invariant: mcp serve should return after stdin closes");
    assert_success(&output, &["mcp", "serve"]);

    let lines = parse_mcp_frames(&output.stdout);
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0]["error"]["code"], -32602);
    assert!(lines[0]["error"]["message"]
        .as_str()
        .expect("invariant: error message should be a string")
        .contains("missing tool name"));
    assert!(lines[1]["error"]["message"]
        .as_str()
        .expect("invariant: error message should be a string")
        .contains("unknown MCP tool"));
    assert!(lines[2]["error"]["message"]
        .as_str()
        .expect("invariant: error message should be a string")
        .contains("claims must contain exactly one claim"));
}

#[test]
fn improve_show_preserves_legacy_minimal_backlog_items() {
    let temp = setup_repo("maestro-improve-legacy");
    let repo = temp.path();
    fs::write(
        repo.join(".maestro/harness/backlog.yaml"),
        concat!(
            "schema_version: maestro.backlog.v1\n",
            "items:\n",
            "  - id: hb-legacy\n",
            "    title: Add legacy coverage\n"
        ),
    )
    .expect("invariant: legacy backlog should be writable");

    let list = run_success(repo, &["improve", "list"]);
    assert!(list.contains("hb-legacy"));
    assert!(list.contains("proposed"));
    assert!(list.contains("unknown"));

    let show = run_success(repo, &["improve", "show", "hb-legacy"]);
    assert!(show.contains("status: proposed"));
    assert!(show.contains("type: unknown"));
    assert!(show.contains("priority: medium"));
}

#[test]
fn improve_refuses_symlinked_harness_backlog_paths() {
    let temp = setup_repo("maestro-improve-symlink");
    let repo = temp.path();
    let external = TestTempDir::new("maestro-improve-external");
    fs::remove_dir_all(repo.join(".maestro/harness"))
        .expect("invariant: harness dir should be removable");
    unix_fs::symlink(external.path(), repo.join(".maestro/harness"))
        .expect("invariant: symlink should be creatable");

    let output = maestro(repo, &["improve", "list"]);
    assert!(
        !output.status.success(),
        "improve list should reject symlinked harness path\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("symlink"));
    assert!(!external.path().join("backlog.yaml").exists());
}
