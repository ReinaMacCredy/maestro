mod support;

use std::fs;
use std::io::Write;
use std::os::unix::fs as unix_fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value;
use serde_yaml::{Mapping as YamlMapping, Value as YamlValue};
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
        vec!["task", "set", "task-001", "--check", "CSV export verified"],
        vec!["task", "explore", "task-001"],
        vec!["task", "accept", "task-001"],
        vec!["task", "claim", "task-001"],
    ] {
        let output = maestro(repo, &args);
        assert_success(&output, &args);
    }

    let mut task = task_yaml(repo, "task-001");
    task["state"] = YamlValue::String("needs_verification".to_string());
    task["updated_at"] = YamlValue::String("test-complete".to_string());
    let mut entry = YamlMapping::new();
    entry.insert(
        YamlValue::String("state".to_string()),
        YamlValue::String("needs_verification".to_string()),
    );
    entry.insert(
        YamlValue::String("at".to_string()),
        YamlValue::String("test-complete".to_string()),
    );
    entry.insert(
        YamlValue::String("by".to_string()),
        YamlValue::String("maestro".to_string()),
    );
    entry.insert(
        YamlValue::String("summary".to_string()),
        YamlValue::String("done".to_string()),
    );
    entry.insert(
        YamlValue::String("claims".to_string()),
        YamlValue::Sequence(vec![YamlValue::String(claim.to_string())]),
    );
    task["state_history"]
        .as_sequence_mut()
        .expect("invariant: state_history should be editable")
        .push(YamlValue::Mapping(entry));
    write_task_yaml(repo, "task-001", &task);
}

fn task_dir(repo: &Path, id: &str) -> PathBuf {
    let prefix = format!("{id}-");
    for root in task_roots(repo) {
        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };
        for entry in entries {
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
    }
    panic!("invariant: task directory should exist for {id}");
}

fn task_roots(repo: &Path) -> Vec<PathBuf> {
    let mut roots = vec![repo.join(".maestro/tasks")];
    let features_dir = repo.join(".maestro/features");
    if let Ok(features) = fs::read_dir(features_dir) {
        for feature in features {
            let feature = feature.expect("invariant: feature entry should be readable");
            roots.push(feature.path().join("tasks"));
        }
    }
    roots
}

fn task_yaml(repo: &Path, id: &str) -> YamlValue {
    let raw = fs::read_to_string(task_dir(repo, id).join("task.yaml"))
        .expect("invariant: task.yaml should be readable");
    serde_yaml::from_str(&raw).expect("invariant: task.yaml should parse")
}

fn write_task_yaml(repo: &Path, id: &str, task: &YamlValue) {
    fs::write(
        task_dir(repo, id).join("task.yaml"),
        serde_yaml::to_string(task).expect("invariant: task.yaml should serialize"),
    )
    .expect("invariant: task.yaml should be writable");
}

fn verification_json(repo: &Path, id: &str) -> Value {
    let task = task_yaml(repo, id);
    let binding = task["verification"].clone();
    let binding_json =
        serde_json::to_value(&binding).expect("invariant: embedded verification should convert");
    let mut object = serde_json::Map::new();
    object.insert(
        "schema_version".to_string(),
        Value::String("maestro.task.v2#verification".to_string()),
    );
    for key in [
        "status",
        "verified_at",
        "verified_commit",
        "verified_by_run",
        "contract_hash",
        "proof_sources",
        "failures",
    ] {
        if let Some(value) = binding_json.get(key) {
            object.insert(key.to_string(), value.clone());
        }
    }
    if let Some(value) = binding_json.get("claim_checks") {
        object.insert("claims".to_string(), value.clone());
    }
    Value::Object(object)
}

fn write_verification_json(repo: &Path, id: &str, verification: &Value) {
    let mut task = task_yaml(repo, id);
    let mut binding = YamlMapping::new();
    for key in [
        "status",
        "verified_at",
        "verified_commit",
        "verified_by_run",
        "contract_hash",
        "proof_sources",
        "failures",
    ] {
        if let Some(value) = verification.get(key) {
            binding.insert(
                YamlValue::String(key.to_string()),
                serde_yaml::to_value(value).expect("invariant: verification field converts"),
            );
        }
    }
    if let Some(value) = verification.get("claims") {
        binding.insert(
            YamlValue::String("claim_checks".to_string()),
            serde_yaml::to_value(value).expect("invariant: verification claims convert"),
        );
    }
    task["verification"] = YamlValue::Mapping(binding);
    write_task_yaml(repo, id, &task);
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

fn write_feature_baseline(repo: &Path, feature_id: &str) {
    fs::write(
        repo.join(".maestro/features")
            .join(feature_id)
            .join("qa.md"),
        "---\namend_log_position: 0\n---\n\nbaseline\n",
    )
    .expect("invariant: feature baseline should be writable");
}

fn write_harness_verify_command(repo: &Path, command: &str) {
    let harness_dir = repo.join(".maestro/harness");
    fs::create_dir_all(&harness_dir).expect("invariant: harness dir should be creatable");
    fs::write(
        harness_dir.join("harness.yml"),
        format!(
            "schema_version: maestro.harness.v1\nstack:\n  kind: generic\n  detected_by: []\n  verify:\n  - '{}'\n",
            command.replace('\'', "''")
        ),
    )
    .expect("invariant: harness.yml should be writable");
}

fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', "'\\''"))
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
fn task_verify_passes_with_event_proof_and_embeds_verification() {
    let temp = setup_repo();
    let repo = temp.path();
    create_completed_task(repo, "implemented CSV export");
    write_event(repo, "task-001", "implemented CSV export");
    let task_before_verify = task_yaml(repo, "task-001");

    let verify = maestro(repo, &["task", "verify", "task-001"]);
    assert_success(&verify, &["task", "verify", "task-001"]);
    assert!(stdout(&verify).contains("verification passed for task-001"));

    let task = task_yaml(repo, "task-001");
    assert_eq!(task["state"], YamlValue::String("verified".to_string()));
    assert!(task["verification"]["verified_at"].as_str().is_some());
    assert!(task["verification"]["contract_hash"].as_str().is_some());

    let verification = verification_json(repo, "task-001");
    assert_eq!(
        verification["schema_version"],
        "maestro.task.v2#verification"
    );
    assert_eq!(verification["status"], "passed");
    assert_ne!(task["updated_at"], task_before_verify["updated_at"]);
    assert_eq!(
        task["verification"]["status"],
        YamlValue::String("passed".to_string())
    );
    assert_eq!(verification["claims"][0]["matched"], true);
    assert!(
        !task_dir(repo, "task-001")
            .join("verification.json")
            .exists(),
        "verification sidecar should not be written"
    );
    assert!(
        !task_dir(repo, "task-001")
            .join("verification.attempts")
            .exists(),
        "verification attempts sidecar should not be written"
    );
    assert!(
        verification["proof_sources"][0]["path"]
            .as_str()
            .expect("invariant: proof source path should be present")
            .contains("events.jsonl")
    );
}

#[test]
fn task_verify_warns_when_after_dependency_cleanup_fails_after_apply() {
    let temp = setup_repo();
    let repo = temp.path();
    assert_success(
        &maestro(repo, &["feature", "new", "Cleanup Dependency"]),
        &["feature", "new", "Cleanup Dependency"],
    );
    assert_success(
        &maestro(
            repo,
            &[
                "feature",
                "set",
                "cleanup-dependency",
                "--acceptance",
                "first task done",
                "--area",
                "task",
            ],
        ),
        &["feature", "set", "cleanup-dependency"],
    );
    write_feature_baseline(repo, "cleanup-dependency");
    assert_success(
        &maestro(repo, &["feature", "accept", "cleanup-dependency"]),
        &["feature", "accept", "cleanup-dependency"],
    );
    let plan = repo.join("PLAN-cleanup-dependency.md");
    fs::write(
        &plan,
        concat!(
            "## Task T1: First dependency\n",
            "check: first task done\n",
            "\n",
            "## Task T2: Dependent task\n",
            "after: T1\n",
            "check: second task done\n",
        ),
    )
    .expect("invariant: prepare plan should be writable");
    let plan_arg = plan
        .to_str()
        .expect("invariant: prepare plan path should be UTF-8");
    assert_success(
        &maestro(
            repo,
            &[
                "feature",
                "prepare",
                "cleanup-dependency",
                "--from",
                plan_arg,
            ],
        ),
        &["feature", "prepare", "cleanup-dependency", "--from"],
    );
    assert_success(
        &maestro(repo, &["task", "claim", "task-001"]),
        &["task", "claim", "task-001"],
    );
    fs::write(
        task_dir(repo, "task-002").join("task.yaml.lock"),
        "locked\n",
    )
    .expect("invariant: dependent task lock should be writable");

    let verify = maestro(
        repo,
        &[
            "task",
            "complete",
            "task-001",
            "--summary",
            "first task done",
            "--claim",
            "first task done",
            "--proof",
            "first task done",
        ],
    );

    assert_success(&verify, &["task", "complete", "task-001"]);
    assert!(stdout(&verify).contains("verification passed for task-001"));
    let err = stderr(&verify);
    assert!(
        err.contains("warning: after-dependency cleanup incomplete for task-001"),
        "{err}"
    );
    assert!(
        err.contains("follow-up: run maestro task list --blocked"),
        "{err}"
    );
    assert_eq!(
        task_yaml(repo, "task-001")["state"],
        YamlValue::String("verified".to_string())
    );
    assert_eq!(
        task_yaml(repo, "task-002")["blockers"][0]["resolved_at"],
        YamlValue::Null
    );
}

#[test]
fn task_verify_ignores_legacy_verification_json_directory() {
    let temp = setup_repo();
    let repo = temp.path();
    create_completed_task(repo, "implemented transaction report write");
    write_event(repo, "task-001", "implemented transaction report write");
    fs::create_dir(task_dir(repo, "task-001").join("verification.json"))
        .expect("invariant: blocking verification path should be creatable");

    let verify = maestro(repo, &["task", "verify", "task-001"]);

    assert_success(&verify, &["task", "verify", "task-001"]);
    assert!(stdout(&verify).contains("verification passed for task-001"));
    assert_eq!(
        task_yaml(repo, "task-001")["state"],
        YamlValue::String("verified".to_string())
    );
}

#[test]
fn task_verify_passed_apply_failure_leaves_report_unapplied() {
    let temp = setup_repo();
    let repo = temp.path();
    create_completed_task(repo, "implemented transaction apply");
    write_event(repo, "task-001", "implemented transaction apply");
    fs::write(
        task_dir(repo, "task-001").join("task.yaml.lock"),
        "locked\n",
    )
    .expect("invariant: task lock should be writable");

    let verify = maestro(repo, &["task", "verify", "task-001"]);

    assert_failure(&verify, &["task", "verify", "task-001"]);
    let err = stderr(&verify);
    assert!(err.contains("verification outcome was not embedded"));
    assert!(err.contains("task is locked"));
    assert!(
        !task_dir(repo, "task-001")
            .join("verification.json")
            .exists()
    );
    let task = task_yaml(repo, "task-001");
    assert_eq!(
        task["state"],
        YamlValue::String("needs_verification".to_string())
    );
    assert!(task["verification"]["verified_at"].as_str().is_none());

    let proof = maestro(repo, &["query", "proof", "task-001"]);
    assert_success(&proof, &["query", "proof", "task-001"]);
    let proof_out = stdout(&proof);
    assert!(proof_out.contains("proof task-001: missing"));
    assert!(proof_out.contains("missing task.yaml verification block"));
}

#[test]
fn task_verify_failed_apply_failure_leaves_report_unapplied() {
    let temp = setup_repo();
    let repo = temp.path();
    create_completed_task(repo, "implemented failed transaction apply");
    let before = task_yaml(repo, "task-001");
    fs::write(
        task_dir(repo, "task-001").join("task.yaml.lock"),
        "locked\n",
    )
    .expect("invariant: task lock should be writable");

    let verify = maestro(repo, &["task", "verify", "task-001"]);

    assert_failure(&verify, &["task", "verify", "task-001"]);
    let err = stderr(&verify);
    assert!(err.contains("verification failure: missing proof"));
    assert!(err.contains("verification outcome was not embedded"));
    assert!(err.contains("task is locked"));
    assert!(
        !task_dir(repo, "task-001")
            .join("verification.json")
            .exists()
    );
    assert_eq!(task_yaml(repo, "task-001"), before);

    let proof = maestro(repo, &["query", "proof", "task-001"]);
    assert_success(&proof, &["query", "proof", "task-001"]);
    let proof_out = stdout(&proof);
    assert!(proof_out.contains("proof task-001: missing"));
    assert!(proof_out.contains("missing task.yaml verification block"));
}

#[test]
fn task_verify_stale_snapshot_writes_unapplied_report_without_marking_verified() {
    let temp = setup_repo();
    let repo = temp.path();
    create_completed_task(repo, "implemented stale snapshot handling");
    write_event(repo, "task-001", "implemented stale snapshot handling");
    write_harness_verify_command(
        repo,
        &format!(
            "{} task update task-001 --summary concurrent-change",
            env!("CARGO_BIN_EXE_maestro")
        ),
    );

    let verify = maestro(repo, &["task", "verify", "task-001"]);

    assert_failure(&verify, &["task", "verify", "task-001"]);
    let err = stderr(&verify);
    assert!(err.contains("verification outcome was not embedded"));
    assert!(err.contains("task was modified"));
    assert!(
        !task_dir(repo, "task-001")
            .join("verification.json")
            .exists()
    );
    let task = task_yaml(repo, "task-001");
    assert_eq!(
        task["state"],
        YamlValue::String("needs_verification".to_string())
    );
    assert!(task["verification"]["verified_at"].as_str().is_none());

    let proof = maestro(repo, &["query", "proof", "task-001"]);
    assert_success(&proof, &["query", "proof", "task-001"]);
    assert!(stdout(&proof).contains("proof task-001: missing"));
}

#[test]
fn concurrent_verify_does_not_overwrite_applied_canonical_report_with_stale_attempt() {
    let temp = setup_repo();
    let repo = temp.path();
    create_completed_task(repo, "implemented concurrent verification");
    write_event(repo, "task-001", "implemented concurrent verification");
    let harness_path = repo.join(".maestro/harness/harness.yml");
    write_harness_verify_command(
        repo,
        &format!(
            "rm -f {} && {} task verify task-001",
            shell_quote(&harness_path),
            shell_quote(Path::new(env!("CARGO_BIN_EXE_maestro")))
        ),
    );

    let verify = maestro(repo, &["task", "verify", "task-001"]);

    assert_failure(&verify, &["task", "verify", "task-001"]);
    assert!(stderr(&verify).contains("task was modified"));
    let task = task_yaml(repo, "task-001");
    assert_eq!(task["state"], YamlValue::String("verified".to_string()));
    let verification = verification_json(repo, "task-001");
    assert_eq!(verification["status"], "passed");
    assert_eq!(
        task["verification"]["verified_at"].as_str(),
        verification["verified_at"].as_str()
    );

    let proof = maestro(repo, &["query", "proof", "task-001"]);
    assert_success(&proof, &["query", "proof", "task-001"]);
    let proof_out = stdout(&proof);
    assert!(proof_out.contains("proof task-001: accepted"));
    assert!(!proof_out.contains("unapplied"));
}

#[test]
fn failed_verification_demotes_previously_verified_task_through_task_verify() {
    let temp = setup_repo();
    let repo = temp.path();
    create_completed_task(repo, "implemented verified regression");
    write_event(repo, "task-001", "implemented verified regression");
    let first = maestro(repo, &["task", "verify", "task-001"]);
    assert_success(&first, &["task", "verify", "task-001"]);
    assert_eq!(
        task_yaml(repo, "task-001")["state"],
        YamlValue::String("verified".to_string())
    );
    let update = maestro(
        repo,
        &[
            "task",
            "update",
            "task-001",
            "--claim",
            "new unproved regression claim",
        ],
    );
    assert_success(&update, &["task", "update", "task-001", "--claim"]);

    let second = maestro(repo, &["task", "verify", "task-001"]);

    assert_failure(&second, &["task", "verify", "task-001"]);
    assert!(stderr(&second).contains("claim not backed by events/proof"));
    assert_eq!(
        task_yaml(repo, "task-001")["state"],
        YamlValue::String("needs_verification".to_string())
    );
    let task_after_failed_verify = task_yaml(repo, "task-001");
    assert_eq!(
        task_after_failed_verify["verification"]["status"],
        YamlValue::String("failed".to_string())
    );
    assert!(
        task_after_failed_verify["verification"]["verified_at"]
            .as_str()
            .is_some()
    );
    assert!(
        task_after_failed_verify["verification"]["verified_commit"]
            .as_str()
            .is_none()
    );
    assert_eq!(verification_json(repo, "task-001")["status"], "failed");
    let mut task = task_after_failed_verify;
    let history = task["state_history"]
        .as_sequence_mut()
        .expect("invariant: state_history should be editable");
    let latest = history
        .last_mut()
        .expect("invariant: failed verification should append history");
    latest["summary"] = YamlValue::String("unrelated history summary".to_string());
    latest["open_items"] = YamlValue::Sequence(Vec::new());
    write_task_yaml(repo, "task-001", &task);

    let proof = maestro(repo, &["query", "proof", "task-001"]);
    assert_success(&proof, &["query", "proof", "task-001"]);
    assert!(stdout(&proof).contains("proof task-001: failed"));
}

#[test]
fn task_show_flags_a_claim_added_after_verification_as_unverified() {
    let temp = setup_repo();
    let repo = temp.path();
    create_completed_task(repo, "implemented CSV export");
    write_event(repo, "task-001", "implemented CSV export");
    assert_success(
        &maestro(repo, &["task", "verify", "task-001"]),
        &["task", "verify", "task-001"],
    );

    // Before any post-verification update, the proven claim renders plain.
    let before = maestro(repo, &["task", "show", "task-001"]);
    assert_success(&before, &["task", "show", "task-001"]);
    let before_out = stdout(&before);
    assert!(before_out.contains("- implemented CSV export"));
    assert!(!before_out.contains("(unverified)"));

    // Recording a new claim on a verified task is still allowed (it is the
    // re-verification path), but `task show` must not let it masquerade as
    // proven while the task still reads `verified`.
    assert_success(
        &maestro(
            repo,
            &[
                "task",
                "update",
                "task-001",
                "--claim",
                "unproven follow-up",
            ],
        ),
        &[
            "task",
            "update",
            "task-001",
            "--claim",
            "unproven follow-up",
        ],
    );
    let after = maestro(repo, &["task", "show", "task-001"]);
    assert_success(&after, &["task", "show", "task-001"]);
    let after_out = stdout(&after);
    assert!(after_out.contains("state: verified"));
    assert!(after_out.contains("- unproven follow-up (unverified)"));
    // The verified claim must not vanish when a later claim is added: a reader
    // still needs to see what verification actually proved.
    assert!(after_out.contains("- implemented CSV export"));
    assert!(!after_out.contains("- implemented CSV export (unverified)"));
}

#[test]
fn legacy_failed_verification_without_receipt_still_reports_failed() {
    let temp = setup_repo();
    let repo = temp.path();
    create_completed_task(repo, "implemented legacy failed report");

    let verify = maestro(repo, &["task", "verify", "task-001"]);

    assert_failure(&verify, &["task", "verify", "task-001"]);
    let mut verification = verification_json(repo, "task-001");
    verification
        .as_object_mut()
        .expect("invariant: verification should be an object")
        .remove("task_snapshot");
    write_verification_json(repo, "task-001", &verification);
    let mut task = task_yaml(repo, "task-001");
    task["verification"]
        .as_mapping_mut()
        .expect("invariant: verification binding should be a map")
        .remove(YamlValue::String("applied_report".to_string()));
    write_task_yaml(repo, "task-001", &task);

    let proof = maestro(repo, &["query", "proof", "task-001"]);
    assert_success(&proof, &["query", "proof", "task-001"]);
    assert!(stdout(&proof).contains("proof task-001: failed"));
}

#[test]
fn legacy_passed_verification_without_receipt_still_reports_accepted() {
    let temp = setup_repo();
    let repo = temp.path();
    create_completed_task(repo, "implemented legacy passed report");
    write_event(repo, "task-001", "implemented legacy passed report");

    let verify = maestro(repo, &["task", "verify", "task-001"]);
    assert_success(&verify, &["task", "verify", "task-001"]);
    let mut verification = verification_json(repo, "task-001");
    verification
        .as_object_mut()
        .expect("invariant: verification should be an object")
        .remove("task_snapshot");
    write_verification_json(repo, "task-001", &verification);
    let mut task = task_yaml(repo, "task-001");
    task["verification"]
        .as_mapping_mut()
        .expect("invariant: verification binding should be a map")
        .remove(YamlValue::String("applied_report".to_string()));
    write_task_yaml(repo, "task-001", &task);

    let proof = maestro(repo, &["query", "proof", "task-001"]);
    assert_success(&proof, &["query", "proof", "task-001"]);
    assert!(stdout(&proof).contains("proof task-001: accepted"));
}

#[test]
fn task_verify_hashes_inline_acceptance_contract() {
    let temp = setup_repo();
    let repo = temp.path();
    create_completed_task(repo, "implemented acceptance resampling");
    write_event(repo, "task-001", "implemented acceptance resampling");

    let verify = maestro(repo, &["task", "verify", "task-001"]);
    assert_success(&verify, &["task", "verify", "task-001"]);

    let task = task_yaml(repo, "task-001");
    let verification = verification_json(repo, "task-001");
    assert_eq!(verification["status"], "passed");
    assert_eq!(
        task["verification"]["contract_hash"].as_str(),
        verification["contract_hash"].as_str()
    );
    let proof = maestro(repo, &["query", "proof", "task-001"]);
    assert_success(&proof, &["query", "proof", "task-001"]);
    assert!(stdout(&proof).contains("proof task-001: accepted"));
}

#[test]
fn task_verify_ignores_legacy_acceptance_symlink_created_by_harness_verify_command() {
    let temp = setup_repo();
    let repo = temp.path();
    create_completed_task(repo, "implemented acceptance symlink protection");
    write_event(
        repo,
        "task-001",
        "implemented acceptance symlink protection",
    );
    let acceptance_path = task_dir(repo, "task-001").join("acceptance.yaml");
    let external_acceptance = repo.join("external-acceptance.yaml");
    fs::write(
        &external_acceptance,
        "schema_version: maestro.acceptance.v1\ntask: task-001\nchecks: [external]\n",
    )
    .expect("invariant: external acceptance should be writable");
    write_harness_verify_command(
        repo,
        &format!(
            "rm -f {} && ln -s {} {}",
            shell_quote(&acceptance_path),
            shell_quote(&external_acceptance),
            shell_quote(&acceptance_path)
        ),
    );

    let verify = maestro(repo, &["task", "verify", "task-001"]);

    assert_success(&verify, &["task", "verify", "task-001"]);
    assert!(stdout(&verify).contains("verification passed for task-001"));
}

#[test]
fn task_verify_ignores_legacy_symlinked_verification_attempts_dir() {
    let temp = setup_repo();
    let repo = temp.path();
    create_completed_task(repo, "implemented attempts symlink protection");
    write_event(repo, "task-001", "implemented attempts symlink protection");
    let external_attempts = repo.join("external-attempts");
    fs::create_dir(&external_attempts).expect("invariant: external attempts dir should exist");
    unix_fs::symlink(
        &external_attempts,
        task_dir(repo, "task-001").join("verification.attempts"),
    )
    .expect("invariant: attempts symlink should be creatable on unix test host");

    let verify = maestro(repo, &["task", "verify", "task-001"]);

    assert_success(&verify, &["task", "verify", "task-001"]);
    assert_eq!(
        fs::read_dir(external_attempts)
            .expect("invariant: external attempts dir should be readable")
            .count(),
        0
    );
}

#[test]
fn query_proof_ignores_legacy_symlinked_verification_attempts_dir() {
    let temp = setup_repo();
    let repo = temp.path();
    create_completed_task(repo, "implemented attempts read protection");
    let external_attempts = repo.join("external-attempts");
    fs::create_dir(&external_attempts).expect("invariant: external attempts dir should exist");
    unix_fs::symlink(
        &external_attempts,
        task_dir(repo, "task-001").join("verification.attempts"),
    )
    .expect("invariant: attempts symlink should be creatable on unix test host");

    let proof = maestro(repo, &["query", "proof", "task-001"]);

    assert_success(&proof, &["query", "proof", "task-001"]);
    assert!(stdout(&proof).contains("proof task-001: missing"));
}

#[test]
fn task_verify_ignores_legacy_symlinked_canonical_verification_report() {
    let temp = setup_repo();
    let repo = temp.path();
    create_completed_task(repo, "implemented canonical report symlink protection");
    write_event(
        repo,
        "task-001",
        "implemented canonical report symlink protection",
    );
    let external_report = repo.join("external-verification.json");
    fs::write(&external_report, "{}\n").expect("invariant: external report should be writable");
    unix_fs::symlink(
        &external_report,
        task_dir(repo, "task-001").join("verification.json"),
    )
    .expect("invariant: canonical report symlink should be creatable on unix test host");

    let verify = maestro(repo, &["task", "verify", "task-001"]);

    assert_success(&verify, &["task", "verify", "task-001"]);
    assert_eq!(
        fs::read_to_string(external_report).expect("invariant: external report should be readable"),
        "{}\n"
    );
}

#[test]
fn query_proof_ignores_legacy_symlinked_canonical_verification_report() {
    let temp = setup_repo();
    let repo = temp.path();
    create_completed_task(repo, "implemented canonical report read protection");
    let external_report = repo.join("external-verification.json");
    fs::write(&external_report, "{}\n").expect("invariant: external report should be writable");
    unix_fs::symlink(
        &external_report,
        task_dir(repo, "task-001").join("verification.json"),
    )
    .expect("invariant: canonical report symlink should be creatable on unix test host");

    let proof = maestro(repo, &["query", "proof", "task-001"]);

    assert_success(&proof, &["query", "proof", "task-001"]);
    assert!(stdout(&proof).contains("proof task-001: missing"));
}

#[test]
fn query_proof_reports_failed_when_acceptance_disappears_after_failed_verify() {
    let temp = setup_repo();
    let repo = temp.path();
    create_completed_task(repo, "implemented failed proof status stability");

    let verify = maestro(repo, &["task", "verify", "task-001"]);
    assert_failure(&verify, &["task", "verify", "task-001"]);
    assert!(stderr(&verify).contains("missing proof"));
    let mut task = task_yaml(repo, "task-001");
    task.as_mapping_mut()
        .expect("invariant: task yaml should be a mapping")
        .remove(YamlValue::String("acceptance".to_string()));
    write_task_yaml(repo, "task-001", &task);

    let proof = maestro(repo, &["query", "proof", "task-001"]);

    assert_success(&proof, &["query", "proof", "task-001"]);
    assert!(stdout(&proof).contains("proof task-001: failed"));
}

#[test]
fn proof_status_reports_stale_when_inline_acceptance_changes_after_pass() {
    let temp = setup_repo();
    let repo = temp.path();
    create_completed_task(repo, "implemented legacy passed freshness error");
    write_event(
        repo,
        "task-001",
        "implemented legacy passed freshness error",
    );
    let verify = maestro(repo, &["task", "verify", "task-001"]);
    assert_success(&verify, &["task", "verify", "task-001"]);
    let mut task = task_yaml(repo, "task-001");
    task.as_mapping_mut()
        .expect("invariant: task yaml should be a mapping")
        .remove(YamlValue::String("acceptance".to_string()));
    write_task_yaml(repo, "task-001", &task);
    let paths = maestro::foundation::core::paths::MaestroPaths::new(repo.to_path_buf());

    let status = maestro::domain::proof::proof_status(&paths, "task-001");

    let status = status.expect("inline acceptance defaults should still produce proof status");
    assert_eq!(status.kind, maestro::domain::proof::ProofStatusKind::Stale);
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
fn event_create_rejects_an_unknown_task() {
    let temp = setup_repo();
    let repo = temp.path();
    create_completed_task(repo, "implemented event create proof");

    // An explicit, non-existent task id must fail loudly rather than log a
    // dangling proof event with exit 0 (T2).
    let create = maestro(
        repo,
        &[
            "event",
            "create",
            "--task-id",
            "task-999",
            "--message",
            "orphan",
            "--run",
            "manual-test",
        ],
    );
    assert_failure(&create, &["event", "create", "--task-id", "task-999"]);
    assert!(
        stderr(&create).contains("task not found"),
        "{}",
        stderr(&create)
    );
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
    assert!(query_out.contains("task.yaml#verification"));
    assert!(query_out.contains("claims: 1/1"));

    let mut task = task_yaml(repo, "task-001");
    task["acceptance"]["checks"] =
        YamlValue::Sequence(vec![YamlValue::String("new check".to_string())]);
    write_task_yaml(repo, "task-001", &task);

    let stale = maestro(repo, &["query", "proof", "task-001"]);
    assert_success(&stale, &["query", "proof", "task-001"]);
    let stale_out = stdout(&stale);
    assert!(stale_out.contains("proof task-001: stale"));
    assert!(stale_out.contains("contract_hash"));
}

#[test]
fn query_proof_reports_stale_hashes_for_failed_verification() {
    let temp = setup_repo();
    let repo = temp.path();
    create_completed_task(repo, "implemented CSV export");
    let verify = maestro(repo, &["task", "verify", "task-001"]);
    assert_failure(&verify, &["task", "verify", "task-001"]);

    let mut task = task_yaml(repo, "task-001");
    task["acceptance"]["checks"] =
        YamlValue::Sequence(vec![YamlValue::String("new check".to_string())]);
    write_task_yaml(repo, "task-001", &task);

    let stale = maestro(repo, &["query", "proof", "task-001"]);
    assert_success(&stale, &["query", "proof", "task-001"]);
    let stale_out = stdout(&stale);
    assert!(stale_out.contains("proof task-001: failed"));
    assert!(stale_out.contains("contract_hash"));
    assert!(stale_out.contains("missing proof"));
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
