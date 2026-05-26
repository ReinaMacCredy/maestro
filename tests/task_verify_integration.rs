mod support;

use std::fs;
use std::io::Write;
use std::os::unix::fs as unix_fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value;
use serde_yaml::Value as YamlValue;
use sha2::{Digest, Sha256};
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

fn assert_failure(output: &std::process::Output, args: &[&str]) {
    assert!(
        !output.status.success(),
        "maestro {:?} unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("invariant: stdout should be UTF-8")
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("invariant: stderr should be UTF-8")
}

fn setup_repo() -> TestTempDir {
    let temp = TestTempDir::new("maestro-task-verify-cli");
    fs::create_dir_all(temp.path().join(".maestro"))
        .expect("invariant: .maestro directory should be creatable");
    temp
}

fn sha256_prefixed_json(raw: &str) -> String {
    let value: Value = serde_json::from_str(raw).expect("invariant: test JSON should parse");
    let bytes = serde_json::to_vec(&value).expect("invariant: test JSON should serialize");
    let digest = Sha256::digest(&bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }
    format!("sha256:{hex}")
}

fn create_completed_task(repo: &Path, claim: &str) {
    for args in [
        vec!["task", "create", "Add CSV export"],
        vec!["task", "explore", "task-001"],
        vec!["task", "accept", "task-001"],
        vec!["task", "claim", "task-001"],
        vec![
            "task",
            "complete",
            "task-001",
            "--summary",
            "done",
            "--claim",
            claim,
        ],
    ] {
        let output = maestro(repo, &args);
        assert_success(&output, &args);
    }
}

fn task_dir(repo: &Path, id: &str) -> PathBuf {
    let prefix = format!("{id}-");
    let tasks_dir = repo.join(".maestro/tasks");
    for entry in fs::read_dir(tasks_dir).expect("invariant: tasks dir should be readable") {
        let entry = entry.expect("invariant: task entry should be readable");
        let name = entry
            .file_name()
            .to_str()
            .expect("invariant: task entry should be UTF-8")
            .to_string();
        if name.starts_with(&prefix) {
            return entry.path();
        }
    }
    panic!("invariant: task directory should exist for {id}");
}

fn task_yaml(repo: &Path, id: &str) -> YamlValue {
    let raw = fs::read_to_string(task_dir(repo, id).join("task.yaml"))
        .expect("invariant: task.yaml should be readable");
    serde_yaml::from_str(&raw).expect("invariant: task.yaml should parse")
}

fn verification_json(repo: &Path, id: &str) -> Value {
    let raw = fs::read_to_string(task_dir(repo, id).join("verification.json"))
        .expect("invariant: verification.json should be readable");
    serde_json::from_str(&raw).expect("invariant: verification.json should parse")
}

fn write_event(repo: &Path, task_id: &str, message: &str) {
    let run_dir = repo.join(".maestro/runs/run-001");
    fs::create_dir_all(&run_dir).expect("invariant: run dir should be creatable");
    fs::write(
        run_dir.join("events.jsonl"),
        format!("{{\"task_id\":\"{task_id}\",\"kind\":\"proof\",\"message\":\"{message}\"}}\n"),
    )
    .expect("invariant: events.jsonl should be writable");
}

fn record_hook_event(repo: &Path, payload: &str) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(["hook", "record"])
        .current_dir(repo)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("invariant: compiled maestro binary should run hook record");
    child
        .stdin
        .as_mut()
        .expect("invariant: hook stdin should be piped")
        .write_all(payload.as_bytes())
        .expect("invariant: hook payload should be writable");
    let output = child
        .wait_with_output()
        .expect("invariant: hook record should return output");
    assert_success(&output, &["hook", "record"]);
}

#[test]
fn task_verify_passes_with_event_proof_and_persists_verification_json() {
    let temp = setup_repo();
    let repo = temp.path();
    create_completed_task(repo, "implemented CSV export");
    write_event(repo, "task-001", "implemented CSV export");

    let verify = maestro(repo, &["task", "verify", "task-001"]);
    assert_success(&verify, &["task", "verify", "task-001"]);
    assert!(stdout(&verify).contains("verification passed for task-001"));

    let task = task_yaml(repo, "task-001");
    assert_eq!(task["state"], YamlValue::String("verified".to_string()));
    assert!(task["verification"]["verified_at"].as_str().is_some());
    assert!(task["verification"]["acceptance_hash"].as_str().is_some());

    let verification = verification_json(repo, "task-001");
    assert_eq!(verification["schema_version"], "maestro.verification.v1");
    assert_eq!(verification["status"], "passed");
    assert_eq!(verification["claims"][0]["matched"], true);
    assert!(verification["proof_sources"][0]["path"]
        .as_str()
        .expect("invariant: proof source path should be present")
        .contains("events.jsonl"));
}

#[test]
fn top_level_verify_alias_verifies_task() {
    let temp = setup_repo();
    let repo = temp.path();
    create_completed_task(repo, "implemented alias verification");
    write_event(repo, "task-001", "implemented alias verification");

    let verify = maestro(repo, &["verify", "task-001"]);
    assert_success(&verify, &["verify", "task-001"]);
    assert_eq!(verification_json(repo, "task-001")["status"], "passed");
}

#[test]
fn query_proof_accepts_task_id_flag() {
    let temp = setup_repo();
    let repo = temp.path();
    create_completed_task(repo, "implemented query flag proof");
    write_event(repo, "task-001", "implemented query flag proof");
    let verify = maestro(repo, &["verify", "task-001"]);
    assert_success(&verify, &["verify", "task-001"]);

    let proof = maestro(repo, &["query", "proof", "--task-id", "task-001"]);
    assert_success(&proof, &["query", "proof", "--task-id", "task-001"]);
    assert!(stdout(&proof).contains("proof task-001: accepted"));
}

#[test]
fn event_create_writes_task_proof_for_verification() {
    let temp = setup_repo();
    let repo = temp.path();
    create_completed_task(repo, "implemented event create proof");

    let create = maestro(
        repo,
        &[
            "event",
            "create",
            "--task-id",
            "task-001",
            "--message",
            "implemented event create proof",
            "--run",
            "manual-test",
        ],
    );
    assert_success(&create, &["event", "create"]);
    let verify = maestro(repo, &["verify", "task-001"]);
    assert_success(&verify, &["verify", "task-001"]);
    assert_eq!(verification_json(repo, "task-001")["status"], "passed");
}

#[test]
fn event_create_payload_can_back_current_task_claims() {
    let temp = setup_repo();
    let repo = temp.path();
    create_completed_task(repo, "implemented payload proof");
    let update = maestro(
        repo,
        &[
            "task",
            "update",
            "task-001",
            "--claim",
            "recorded post-completion evidence",
        ],
    );
    assert_success(
        &update,
        &["task", "update", "task-001", "--claim", "<claim>"],
    );

    let create = maestro(
        repo,
        &[
            "event",
            "create",
            "--task-id",
            "task-001",
            "--payload",
            "{\"proof\":\"ok\"}",
            "--run",
            "manual-test",
        ],
    );
    assert_success(&create, &["event", "create", "--payload"]);
    let verify = maestro(repo, &["verify", "task-001"]);
    assert_success(&verify, &["verify", "task-001"]);
    assert_eq!(verification_json(repo, "task-001")["status"], "passed");
}

#[test]
fn event_create_and_verify_infer_single_current_task() {
    let temp = setup_repo();
    let repo = temp.path();
    create_completed_task(repo, "implemented inferred task proof");

    let create = maestro(
        repo,
        &["event", "create", "--payload", "{\"proof\":\"ok\"}"],
    );
    assert_success(&create, &["event", "create", "--payload"]);
    let verify = maestro(repo, &["verify"]);
    assert_success(&verify, &["verify"]);
    assert_eq!(verification_json(repo, "task-001")["status"], "passed");
}

#[test]
fn task_verify_accepts_task_proof_event_alias() {
    let temp = setup_repo();
    let repo = temp.path();
    create_completed_task(repo, "implemented brownfield loop proof");
    let run_dir = repo.join(".maestro/runs/run-001");
    fs::create_dir_all(&run_dir).expect("invariant: run dir should be creatable");
    fs::write(
        run_dir.join("events.jsonl"),
        concat!(
            "{\"event\":\"task_proof\",",
            "\"task_id\":\"task-001\",",
            "\"message\":\"implemented brownfield loop proof\"}\n"
        ),
    )
    .expect("invariant: events.jsonl should be writable");

    let verify = maestro(repo, &["task", "verify", "task-001"]);
    assert_success(&verify, &["task", "verify", "task-001"]);
    assert_eq!(verification_json(repo, "task-001")["status"], "passed");
}

#[test]
fn task_verify_accepts_phase4_post_tool_use_hook_event() {
    let temp = setup_repo();
    let repo = temp.path();
    let tool_input_hash = sha256_prefixed_json(r#"{"command":"cargo test"}"#);
    let claim = format!("Bash {tool_input_hash}");
    create_completed_task(repo, &claim);
    record_hook_event(
        repo,
        r#"{"session_id":"run-001","event_type":"PostToolUse","task_id":"task-001","tool_name":"Bash","status":"ok","tool_input":{"command":"cargo test"}}"#,
    );

    let verify = maestro(repo, &["task", "verify", "task-001"]);
    assert_success(&verify, &["task", "verify", "task-001"]);
    assert_eq!(verification_json(repo, "task-001")["status"], "passed");
}

#[test]
fn task_verify_does_not_accept_generic_phase4_tool_success_claim() {
    let temp = setup_repo();
    let repo = temp.path();
    create_completed_task(repo, "Bash ok");
    record_hook_event(
        repo,
        r#"{"session_id":"run-001","event_type":"PostToolUse","task_id":"task-001","tool_name":"Bash","status":"ok","tool_input":{"command":"cargo test"}}"#,
    );

    let verify = maestro(repo, &["task", "verify", "task-001"]);
    assert_failure(&verify, &["task", "verify", "task-001"]);
    assert!(stderr(&verify).contains("claim not backed by events/proof"));
}

#[test]
fn task_verify_does_not_accept_hash_only_phase4_tool_claim() {
    let temp = setup_repo();
    let repo = temp.path();
    let tool_input_hash = sha256_prefixed_json(r#"{"command":"cargo test"}"#);
    create_completed_task(repo, &tool_input_hash);
    record_hook_event(
        repo,
        r#"{"session_id":"run-001","event_type":"PostToolUse","task_id":"task-001","tool_name":"Bash","status":"ok","tool_input":{"command":"cargo test"}}"#,
    );

    let verify = maestro(repo, &["task", "verify", "task-001"]);
    assert_failure(&verify, &["task", "verify", "task-001"]);
    assert!(stderr(&verify).contains("claim not backed by events/proof"));
}

#[test]
fn task_verify_does_not_accept_failed_phase4_tool_event() {
    let temp = setup_repo();
    let repo = temp.path();
    let tool_input_hash = sha256_prefixed_json(r#"{"command":"cargo test"}"#);
    let claim = format!("Bash {tool_input_hash}");
    create_completed_task(repo, &claim);
    record_hook_event(
        repo,
        r#"{"session_id":"run-001","event_type":"PostToolUse","task_id":"task-001","tool_name":"Bash","status":"error","tool_input":{"command":"cargo test"}}"#,
    );

    let verify = maestro(repo, &["task", "verify", "task-001"]);
    assert_failure(&verify, &["task", "verify", "task-001"]);
    assert!(stderr(&verify).contains("missing proof"));
}

#[test]
fn task_verify_does_not_infer_tests_pass_from_any_successful_bash_event() {
    let temp = setup_repo();
    let repo = temp.path();
    create_completed_task(repo, "tests pass");
    record_hook_event(
        repo,
        r#"{"session_id":"run-001","event_type":"PostToolUse","task_id":"task-001","tool_name":"Bash","status":"ok","tool_input":{"command":"echo hi"}}"#,
    );

    let verify = maestro(repo, &["task", "verify", "task-001"]);
    assert_failure(&verify, &["task", "verify", "task-001"]);
    assert!(stderr(&verify).contains("claim not backed by events/proof"));
}

#[test]
fn task_verify_fails_clearly_when_proof_is_missing_or_claims_do_not_match_events() {
    let temp = setup_repo();
    let repo = temp.path();
    create_completed_task(repo, "implemented CSV export");

    let missing = maestro(repo, &["task", "verify", "task-001"]);
    assert_failure(&missing, &["task", "verify", "task-001"]);
    assert!(stderr(&missing).contains("missing proof"));
    assert_eq!(verification_json(repo, "task-001")["status"], "failed");

    write_event(repo, "task-001", "ran unrelated smoke test");
    let mismatch = maestro(repo, &["task", "verify", "task-001"]);
    assert_failure(&mismatch, &["task", "verify", "task-001"]);
    assert!(stderr(&mismatch).contains("claim not backed by events/proof"));
}

#[test]
fn task_verify_requires_exact_event_task_id_match() {
    let temp = setup_repo();
    let repo = temp.path();
    create_completed_task(repo, "implemented CSV export");
    write_event(repo, "task-0010", "implemented CSV export");

    let verify = maestro(repo, &["task", "verify", "task-001"]);
    assert_failure(&verify, &["task", "verify", "task-001"]);
    assert!(stderr(&verify).contains("claim not backed by events/proof"));
}

#[test]
fn task_verify_requires_exact_claim_match() {
    let temp = setup_repo();
    let repo = temp.path();
    create_completed_task(repo, "implemented CSV export");
    write_event(repo, "task-001", "not implemented CSV export");

    let verify = maestro(repo, &["task", "verify", "task-001"]);
    assert_failure(&verify, &["task", "verify", "task-001"]);
    assert!(stderr(&verify).contains("claim not backed by events/proof"));
}

#[test]
fn task_verify_ignores_non_proof_events_for_claim_matching() {
    let temp = setup_repo();
    let repo = temp.path();
    create_completed_task(repo, "implemented CSV export");
    let run_dir = repo.join(".maestro/runs/run-001");
    fs::create_dir_all(&run_dir).expect("invariant: run dir should be creatable");
    fs::write(
        run_dir.join("events.jsonl"),
        "{\"task_id\":\"task-001\",\"kind\":\"UserPromptSubmit\",\"message\":\"implemented CSV export\"}\n",
    )
    .expect("invariant: events.jsonl should be writable");

    let verify = maestro(repo, &["task", "verify", "task-001"]);
    assert_failure(&verify, &["task", "verify", "task-001"]);
    assert!(stderr(&verify).contains("missing proof"));
}

#[test]
fn task_verify_ignores_bad_json_and_symlinked_run_dirs() {
    let temp = setup_repo();
    let repo = temp.path();
    create_completed_task(repo, "implemented CSV export");
    let runs_dir = repo.join(".maestro/runs");
    let run_dir = runs_dir.join("run-001");
    fs::create_dir_all(&run_dir).expect("invariant: run dir should be creatable");
    fs::write(
        run_dir.join("events.jsonl"),
        concat!(
            "not json\n",
            "{\"task_id\":\"task-001\",\"kind\":\"proof\",\"message\":\"implemented CSV export\"}\n"
        ),
    )
    .expect("invariant: events.jsonl should be writable");
    let bad_run_dir = runs_dir.join("run-002");
    fs::create_dir_all(&bad_run_dir).expect("invariant: bad run dir should be creatable");
    fs::write(bad_run_dir.join("events.jsonl"), [0xff, b'\n'])
        .expect("invariant: bad events should be writable");
    unix_fs::symlink(&runs_dir, runs_dir.join("loop"))
        .expect("invariant: symlink should be creatable on unix test host");

    let verify = maestro(repo, &["task", "verify", "task-001"]);
    assert_success(&verify, &["task", "verify", "task-001"]);
}

#[cfg(unix)]
#[test]
fn task_verify_ignores_symlinked_runs_root() {
    let temp = setup_repo();
    let repo = temp.path();
    create_completed_task(repo, "implemented CSV export");
    let external = TestTempDir::new("maestro-task-verify-external-runs");
    let external_run = external.path().join("run-001");
    fs::create_dir_all(&external_run).expect("invariant: external run dir should be creatable");
    fs::write(
        external_run.join("events.jsonl"),
        "{\"task_id\":\"task-001\",\"kind\":\"proof\",\"message\":\"implemented CSV export\"}\n",
    )
    .expect("invariant: external events should be writable");
    let runs_dir = repo.join(".maestro/runs");
    unix_fs::symlink(external.path(), &runs_dir)
        .expect("invariant: symlinked runs root should be creatable");

    let verify = maestro(repo, &["task", "verify", "task-001"]);

    assert_failure(&verify, &["task", "verify", "task-001"]);
    assert!(stderr(&verify).contains("missing proof"));
}

#[cfg(unix)]
#[test]
fn task_verify_ignores_events_when_maestro_root_is_symlinked() {
    let temp = TestTempDir::new("maestro-task-verify-symlinked-root");
    let repo = temp.path();
    fs::create_dir(repo.join(".git")).expect("invariant: .git marker should be creatable");
    let external = TestTempDir::new("maestro-task-verify-external-root");
    unix_fs::symlink(external.path(), repo.join(".maestro"))
        .expect("invariant: symlinked .maestro root should be creatable");
    create_completed_task(repo, "implemented CSV export");
    let external_run = external.path().join("runs/run-001");
    fs::create_dir_all(&external_run).expect("invariant: external run dir should be creatable");
    fs::write(
        external_run.join("events.jsonl"),
        "{\"task_id\":\"task-001\",\"kind\":\"proof\",\"message\":\"implemented CSV export\"}\n",
    )
    .expect("invariant: external events should be writable");

    let verify = maestro(repo, &["task", "verify", "task-001"]);

    assert_failure(&verify, &["task", "verify", "task-001"]);
    assert!(stderr(&verify).contains("missing proof"));
}

#[test]
fn task_verify_ignores_binary_proof_artifacts_when_text_proof_exists() {
    let temp = setup_repo();
    let repo = temp.path();
    create_completed_task(repo, "documented operator handoff");
    let proof_dir = task_dir(repo, "task-001").join("proof");
    fs::create_dir_all(&proof_dir).expect("invariant: proof dir should be creatable");
    fs::write(proof_dir.join("screenshot.png"), [0xff, 0xd8, 0xff])
        .expect("invariant: binary proof should be writable");
    fs::write(
        proof_dir.join("handoff.txt"),
        "claim: documented operator handoff\n",
    )
    .expect("invariant: text proof should be writable");

    let verify = maestro(repo, &["task", "verify", "task-001"]);
    assert_success(&verify, &["task", "verify", "task-001"]);
}

#[test]
fn task_verify_ignores_post_tool_use_messages_for_claim_matching() {
    let temp = setup_repo();
    let repo = temp.path();
    create_completed_task(repo, "implemented CSV export");
    let run_dir = repo.join(".maestro/runs/run-001");
    fs::create_dir_all(&run_dir).expect("invariant: run dir should be creatable");
    fs::write(
        run_dir.join("events.jsonl"),
        "{\"task_id\":\"task-001\",\"kind\":\"PostToolUse\",\"message\":\"implemented CSV export\"}\n",
    )
    .expect("invariant: events.jsonl should be writable");

    let verify = maestro(repo, &["task", "verify", "task-001"]);
    assert_failure(&verify, &["task", "verify", "task-001"]);
    assert!(stderr(&verify).contains("missing proof"));
}

#[test]
fn query_proof_uses_persisted_verification_and_reports_stale_hashes() {
    let temp = setup_repo();
    let repo = temp.path();
    create_completed_task(repo, "implemented CSV export");
    write_event(repo, "task-001", "implemented CSV export");
    assert_success(
        &maestro(repo, &["task", "verify", "task-001"]),
        &["task", "verify", "task-001"],
    );

    let query = maestro(repo, &["query", "proof", "task-001"]);
    assert_success(&query, &["query", "proof", "task-001"]);
    let query_out = stdout(&query);
    assert!(query_out.contains("proof task-001: accepted"));
    assert!(query_out.contains("verification.json"));
    assert!(query_out.contains("claims: 1/1"));

    fs::write(
        task_dir(repo, "task-001").join("acceptance.yaml"),
        "schema_version: maestro.acceptance.v1\ntask: task-001\nchecks:\n- new check\nlocked_by: maestro\nlocked_at: now\n",
    )
    .expect("invariant: acceptance.yaml should be writable");

    let stale = maestro(repo, &["query", "proof", "task-001"]);
    assert_success(&stale, &["query", "proof", "task-001"]);
    let stale_out = stdout(&stale);
    assert!(stale_out.contains("proof task-001: stale"));
    assert!(stale_out.contains("acceptance_hash"));
    assert!(stale_out.contains("checks_hash"));
}

#[test]
fn task_local_proof_artifacts_can_satisfy_completion_claims() {
    let temp = setup_repo();
    let repo = temp.path();
    create_completed_task(repo, "documented operator handoff");
    let proof_dir = task_dir(repo, "task-001").join("proof");
    fs::create_dir_all(&proof_dir).expect("invariant: proof dir should be creatable");
    fs::write(
        proof_dir.join("handoff.txt"),
        "claim: documented operator handoff\n",
    )
    .expect("invariant: proof file should be writable");

    let verify = maestro(repo, &["task", "verify", "task-001"]);
    assert_success(&verify, &["task", "verify", "task-001"]);
    assert_eq!(verification_json(repo, "task-001")["status"], "passed");
}
