mod support;

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::os::unix::fs as unix_fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value as JsonValue;
use serde_yaml::{Mapping as YamlMapping, Value as YamlValue};
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

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("invariant: stderr should be UTF-8")
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

fn write_empty_harness(repo: &Path) {
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
}

fn write_enabled_harness(repo: &Path) {
    fs::write(
        repo.join(".maestro/harness/harness.yml"),
        concat!(
            "schema_version: maestro.harness.v1\n",
            "stack:\n",
            "  kind: generic\n",
            "  detected_by: []\n",
            "  verify: []\n",
            "escalation:\n",
            "  enabled: true\n",
            "  warn_after: 2\n",
            "  act_after: 3\n"
        ),
    )
    .expect("invariant: harness should be writable");
}

fn write_prompt_session(repo: &Path, session: &str, prompts: &[&str]) {
    let run_dir = repo.join(".maestro/runs").join(session);
    fs::create_dir_all(&run_dir).expect("invariant: run dir should be creatable");
    let events = prompts
        .iter()
        .map(|prompt| {
            format!(
                "{{\"event_type\":\"UserPromptSubmit\",\"prompt\":{}}}\n",
                serde_json::to_string(prompt).expect("invariant: prompt should serialize")
            )
        })
        .collect::<String>();
    fs::write(run_dir.join("events.jsonl"), events)
        .expect("invariant: events fixture should be writable");
}

fn write_correction_session(repo: &Path, session: &str) {
    write_prompt_session(
        repo,
        session,
        &[
            "no, use rg",
            "wait that's wrong",
            "actually keep scope tight",
        ],
    );
}

fn ids_by_type(list: &str) -> BTreeMap<String, String> {
    let mut ids = BTreeMap::new();
    for line in list.lines().skip(1) {
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() >= 4 {
            ids.insert(fields[3].to_string(), fields[0].to_string());
        }
    }
    ids
}

fn spawned_task_id(output: &str) -> String {
    let start = output
        .find("(spawned ")
        .expect("invariant: apply output should include spawned task")
        + "(spawned ".len();
    let end = output[start..]
        .find(')')
        .expect("invariant: spawned task should close")
        + start;
    output[start..end].to_string()
}

fn failed_verification_report(task_id: &str, verified_at: &str, schema_version: &str) -> String {
    failed_verification_report_with_command(task_id, verified_at, schema_version, "cargo test")
}

fn failed_verification_report_with_command(
    task_id: &str,
    verified_at: &str,
    schema_version: &str,
    command: &str,
) -> String {
    format!(
        concat!(
            "{{",
            r#""schema_version":"{}","#,
            r#""task_id":"{}","#,
            r#""status":"failed","#,
            r#""verified_at":"{}","#,
            r#""task_contract_hash":"task-hash","#,
            r#""acceptance_hash":"acceptance-hash","#,
            r#""checks_hash":"checks-hash","#,
            r#""claims":[],"#,
            r#""commands":[{{"cmd":{},"exit_code":1,"duration_ms":10}}],"#,
            r#""proof_sources":[],"#,
            r#""failures":["report"]"#,
            "}}\n"
        ),
        schema_version,
        task_id,
        verified_at,
        serde_json::to_string(command).expect("invariant: command should serialize")
    )
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
    task["lane"] = YamlValue::String(domain.to_string());
    task["verification"]["verified_at"] = YamlValue::String(verified_at.to_string());
    fs::write(
        &path,
        serde_yaml::to_string(&task).expect("invariant: task should serialize"),
    )
    .expect("invariant: task.yaml should be writable");
}

fn write_embedded_failed_verification(repo: &Path, id: &str, verified_at: &str, command: &str) {
    write_embedded_failed_verification_commands(repo, id, verified_at, &[command]);
}

fn write_embedded_failed_verification_commands(
    repo: &Path,
    id: &str,
    verified_at: &str,
    commands: &[&str],
) {
    let path = task_dir(repo, id).join("task.yaml");
    let raw = fs::read_to_string(&path).expect("invariant: task.yaml should be readable");
    let mut task: YamlValue =
        serde_yaml::from_str(&raw).expect("invariant: task.yaml should parse");
    let command_receipts = commands
        .iter()
        .map(|command| {
            let mut command_receipt = YamlMapping::new();
            command_receipt.insert(
                YamlValue::String("cmd".to_string()),
                YamlValue::String((*command).to_string()),
            );
            command_receipt.insert(
                YamlValue::String("exit_code".to_string()),
                YamlValue::Number(1.into()),
            );
            command_receipt.insert(
                YamlValue::String("duration_ms".to_string()),
                YamlValue::Number(10.into()),
            );
            YamlValue::Mapping(command_receipt)
        })
        .collect::<Vec<_>>();
    let mut verification = YamlMapping::new();
    verification.insert(
        YamlValue::String("status".to_string()),
        YamlValue::String("failed".to_string()),
    );
    verification.insert(
        YamlValue::String("verified_at".to_string()),
        YamlValue::String(verified_at.to_string()),
    );
    verification.insert(
        YamlValue::String("contract_hash".to_string()),
        YamlValue::String("task-hash".to_string()),
    );
    verification.insert(
        YamlValue::String("commands".to_string()),
        YamlValue::Sequence(command_receipts),
    );
    verification.insert(
        YamlValue::String("failures".to_string()),
        YamlValue::Sequence(vec![YamlValue::String("report".to_string())]),
    );
    task["verification"] = YamlValue::Mapping(verification);
    fs::write(
        &path,
        serde_yaml::to_string(&task).expect("invariant: task should serialize"),
    )
    .expect("invariant: task.yaml should be writable");
}

#[test]
fn harness_detects_all_rule_based_backlog_proposals_and_applies_one() {
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
    // Two live tasks share a blocker reason to trip recurring_blocker. They are
    // separate from the verified tasks above because a done task cannot take a
    // blocker, and keeping them unverified leaves the verification-duration
    // medians that drive missing_skill untouched.
    create_task(repo, "Task 8");
    create_task(repo, "Task 9");

    fs::write(
        repo.join(".maestro/harness/harness.yml"),
        concat!(
            "schema_version: maestro.harness.v1\n",
            "stack:\n",
            "  kind: generic\n",
            "  detected_by: []\n",
            "  verify:\n",
            "    - api_key='top secret' true\n"
        ),
    )
    .expect("invariant: harness should be writable");
    let verify = maestro(repo, &["task", "verify", "task-003"]);
    assert!(
        !verify.status.success(),
        "task verify should fail for missing proof but still embed verification outcome"
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
                "task-008",
                "--reason",
                "waiting for staging credentials",
            ],
        ),
        &[
            "task",
            "block",
            "task-008",
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
                "task-009",
                "--reason",
                "waiting for staging credentials",
            ],
        ),
        &[
            "task",
            "block",
            "task-009",
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

    let out = run_success(repo, &["harness", "list"]);
    assert!(out.contains("recurring_intervention"));
    assert!(out.contains("missing_verification"));
    assert!(out.contains("recurring_blocker"));
    assert!(out.contains("missing_skill"));
    assert!(out.contains("rediscovered_decision"));

    let ids = ids_by_type(&out);
    let show_id = ids
        .get("missing_skill")
        .expect("invariant: missing_skill id should be listed");
    let show = run_success(repo, &["harness", "show", show_id]);
    assert!(show.contains("status: proposed"));
    assert!(show.contains("evidence:"));
    let backlog = fs::read_to_string(repo.join(".maestro/harness/backlog.yaml"))
        .expect("invariant: backlog should be readable");
    assert!(
        backlog.contains("task.yaml#verification used verification command 1 outside harness.yml")
    );
    assert!(!backlog.contains("top secret"));
    assert!(!backlog.contains("api_key"));

    for item_type in [
        "recurring_intervention",
        "missing_verification",
        "recurring_blocker",
        "missing_skill",
        "rediscovered_decision",
    ] {
        let id = ids
            .get(item_type)
            .unwrap_or_else(|| panic!("invariant: {item_type} id should be listed"));
        let apply = run_success(repo, &["harness", "apply", id]);
        assert!(apply.contains(&format!("accepted {id}")), "{apply}");
        assert!(apply.contains("spawned task-"), "{apply}");
        assert!(apply.contains("check preset:"), "{apply}");
        assert!(apply.contains("next: maestro task claim"), "{apply}");
        let spawned = spawned_task_id(&apply);
        let claim = run_success(repo, &["task", "claim", &spawned]);
        assert!(
            claim.contains(&format!("updated {spawned} -> in_progress")),
            "{claim}"
        );
        let applied = run_success(repo, &["harness", "show", id]);
        assert!(applied.contains("status: accepted"));
        assert!(applied.contains("spawned_task: task-"));
    }
}

#[test]
fn harness_escalation_tracks_recurring_intervention_globally_and_dismisses() {
    let temp = setup_repo("maestro-harness-escalation-global");
    let repo = temp.path();
    write_enabled_harness(repo);
    write_correction_session(repo, "session-a");
    write_correction_session(repo, "session-b");
    write_correction_session(repo, "session-c");

    let list = run_success(repo, &["harness", "list"]);
    assert!(list.contains("ID\t!\tSTATUS\tTYPE\tSEEN\tTITLE"), "{list}");
    assert!(
        list.contains("!\tproposed\trecurring_intervention\t9x/3s"),
        "{list}"
    );
    let ids = ids_by_type(&list);
    let id = ids
        .get("recurring_intervention")
        .expect("invariant: recurring intervention should be listed");

    let show = run_success(repo, &["harness", "show", id]);
    assert!(show.contains("priority: high"), "{show}");
    assert!(show.contains("seen: 9x/3s"), "{show}");
    assert!(
        show.contains("sessions_hit: session-a, session-b, session-c"),
        "{show}"
    );
    let backlog = fs::read_to_string(repo.join(".maestro/harness/backlog.yaml"))
        .expect("invariant: backlog should be readable");
    assert!(backlog.contains("fingerprint: recurring_intervention:global"));
    assert_eq!(backlog.matches("type: recurring_intervention").count(), 1);

    let dismiss = run_success(
        repo,
        &["harness", "dismiss", id, "--reason", "already handled"],
    );
    assert!(dismiss.contains(&format!("dismissed {id}")), "{dismiss}");
    let active = run_success(repo, &["harness", "list"]);
    assert!(!active.contains("recurring_intervention"), "{active}");
    let all = run_success(repo, &["harness", "list", "--all"]);
    assert!(all.contains("dismissed\trecurring_intervention"), "{all}");
    assert_eq!(all.matches("recurring_intervention").count(), 1);
}

#[test]
fn correction_heuristic_is_gated_by_escalation_enabled() {
    let enabled = setup_repo("maestro-harness-correction-enabled");
    write_enabled_harness(enabled.path());
    write_prompt_session(enabled.path(), "noise", &["ok", "continue", "looks good"]);

    let enabled_list = run_success(enabled.path(), &["harness", "list"]);
    assert!(
        !enabled_list.contains("recurring_intervention"),
        "{enabled_list}"
    );
    write_prompt_session(
        enabled.path(),
        "corrections",
        &["no, use rg", "wait that's wrong", "actually verify it"],
    );
    let enabled_list = run_success(enabled.path(), &["harness", "list"]);
    assert!(
        enabled_list.contains("recurring_intervention"),
        "{enabled_list}"
    );

    let disabled = setup_repo("maestro-harness-correction-disabled");
    write_empty_harness(disabled.path());
    write_prompt_session(disabled.path(), "noise", &["ok", "continue", "looks good"]);
    let disabled_list = run_success(disabled.path(), &["harness", "list"]);
    assert!(
        disabled_list.contains("recurring_intervention"),
        "{disabled_list}"
    );
}

#[test]
fn harness_ignores_legacy_symlinked_verification_report() {
    let temp = setup_repo("maestro-improve-proof-reader");
    let repo = temp.path();
    create_task(repo, "Verify proof reader contract");
    mark_verified(repo, "task-001", "proof", "0", "100");

    let external = TestTempDir::new("maestro-external-verification-report");
    fs::write(
        external.path().join("verification.json"),
        r#"{"commands":["cargo test"]}"#,
    )
    .expect("invariant: external report fixture should be writable");
    let task_verification = task_dir(repo, "task-001").join("verification.json");
    unix_fs::symlink(
        external.path().join("verification.json"),
        &task_verification,
    )
    .expect("invariant: verification report symlink should be creatable");

    let out = run_success(repo, &["harness", "list"]);
    assert!(out.contains("no improvement proposals found"));
}

#[test]
fn harness_reads_embedded_verification_command_receipts() {
    let temp = setup_repo("maestro-improve-legacy-proof-commands");
    let repo = temp.path();
    create_task(repo, "Verify legacy proof commands");

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
    write_embedded_failed_verification(repo, "task-001", "100", "cargo test");

    let proof = maestro(repo, &["query", "proof", "task-001"]);
    assert_success(&proof, &["query", "proof", "task-001"]);
    assert!(stdout(&proof).contains("proof task-001: failed"));

    let out = run_success(repo, &["harness", "list"]);
    assert!(out.contains("missing_verification"));
    assert!(out.contains("Add reusable verification for Verify legacy proof commands"));
}

#[test]
fn harness_accepts_multiple_embedded_command_receipts() {
    let temp = setup_repo("maestro-improve-legacy-command-objects");
    let repo = temp.path();
    create_task(repo, "Verify legacy command objects");
    write_empty_harness(repo);
    write_embedded_failed_verification_commands(
        repo,
        "task-001",
        "125",
        &["cargo test", "cargo clippy"],
    );

    let proof = maestro(repo, &["query", "proof", "task-001"]);
    assert_success(&proof, &["query", "proof", "task-001"]);

    let out = run_success(repo, &["harness", "list"]);
    assert!(out.contains("missing_verification"));
    assert!(out.contains("Add reusable verification for Verify legacy command objects"));
    let backlog = fs::read_to_string(repo.join(".maestro/harness/backlog.yaml"))
        .expect("invariant: backlog should be readable");
    assert!(
        backlog.contains("task.yaml#verification used verification command 1 outside harness.yml")
    );
    assert!(
        backlog.contains("task.yaml#verification used verification command 2 outside harness.yml")
    );
}

#[test]
fn harness_exactly_matches_sensitive_harness_commands_without_displaying_them() {
    let temp = setup_repo("maestro-improve-exact-sensitive-command-match");
    let repo = temp.path();
    create_task(repo, "Verify exact sensitive command matching");

    let command = "api_key='top secret' cargo test";
    fs::write(
        repo.join(".maestro/harness/harness.yml"),
        format!(
            concat!(
                "schema_version: maestro.harness.v1\n",
                "stack:\n",
                "  kind: generic\n",
                "  detected_by: []\n",
                "  verify:\n",
                "    - \"{}\"\n"
            ),
            command
        ),
    )
    .expect("invariant: harness should be writable");
    write_embedded_failed_verification(repo, "task-001", "150", command);

    let out = run_success(repo, &["harness", "list"]);
    assert!(!out.contains("missing_verification"));
    assert!(!out.contains("Add reusable verification for Verify exact sensitive command matching"));
    assert!(!out.contains("top secret"));
    assert!(!out.contains("api_key"));
}

#[test]
fn harness_refreshes_existing_backlog_evidence_to_safe_labels() {
    let temp = setup_repo("maestro-improve-refresh-safe-evidence");
    let repo = temp.path();
    create_task(repo, "Refresh stale backlog evidence");
    write_empty_harness(repo);

    write_embedded_failed_verification(repo, "task-001", "175", "api_key='top secret' cargo test");
    fs::write(
        repo.join(".maestro/harness/backlog.yaml"),
        concat!(
            "schema_version: maestro.backlog.v1\n",
            "items:\n",
            "  - id: hb-001\n",
            "    fingerprint: missing_verification:task-001\n",
            "    source: task-001\n",
            "    type: missing_verification\n",
            "    title: Add reusable verification for Refresh stale backlog evidence\n",
            "    priority: medium\n",
            "    status: proposed\n",
            "    evidence:\n",
            "      - \"manual note: keep this context\"\n",
            "      - verification.json used `api_key='top secret' cargo test` outside harness.yml\n"
        ),
    )
    .expect("invariant: backlog should be writable");

    let show = run_success(repo, &["harness", "show", "hb-001"]);
    assert!(show.contains("manual note: keep this context"));
    assert!(
        show.contains("task.yaml#verification used verification command 1 outside harness.yml")
    );
    assert!(!show.contains("top secret"));
    assert!(!show.contains("api_key"));
    let backlog = fs::read_to_string(repo.join(".maestro/harness/backlog.yaml"))
        .expect("invariant: backlog should be readable");
    assert!(backlog.contains("manual note: keep this context"));
    assert!(
        backlog.contains("task.yaml#verification used verification command 1 outside harness.yml")
    );
    assert!(!backlog.contains("top secret"));
    assert!(!backlog.contains("api_key"));
}

#[test]
fn harness_scrubs_orphaned_legacy_missing_verification_evidence() {
    let temp = setup_repo("maestro-improve-scrub-orphan-evidence");
    let repo = temp.path();
    fs::write(
        repo.join(".maestro/harness/backlog.yaml"),
        concat!(
            "schema_version: maestro.backlog.v1\n",
            "items:\n",
            "  - id: hb-001\n",
            "    source: task-001\n",
            "    type: missing_verification\n",
            "    title: Add stale verification\n",
            "    priority: medium\n",
            "    status: accepted\n",
            "    evidence:\n",
            "      - \"manual note: keep this context\"\n",
            "      - verification.attempts/api_key=top_secret.json used `api_key='top secret' cargo test` outside harness.yml\n"
        ),
    )
    .expect("invariant: backlog should be writable");

    let show = run_success(repo, &["harness", "show", "hb-001"]);
    assert!(show.contains("manual note: keep this context"));
    assert!(show.contains(
        "verification.attempts/archived attempt used verification command 1 outside harness.yml"
    ));
    assert!(!show.contains("top secret"));
    assert!(!show.contains("api_key"));
    let backlog = fs::read_to_string(repo.join(".maestro/harness/backlog.yaml"))
        .expect("invariant: backlog should be readable");
    assert!(backlog.contains("manual note: keep this context"));
    assert!(backlog.contains(
        "verification.attempts/archived attempt used verification command 1 outside harness.yml"
    ));
    assert!(!backlog.contains("top_secret"));
    assert!(!backlog.contains("api_key"));
}

#[test]
fn harness_does_not_recover_canonical_proof_reports_while_detecting() {
    let temp = setup_repo("maestro-improve-proof-read-only");
    let repo = temp.path();
    create_task(repo, "Preserve proof restore journal");
    let task_dir = task_dir(repo, "task-001");

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
    let canonical = concat!(
        "{",
        r#""schema_version":"maestro.verification.v1","#,
        r#""task_id":"task-001","#,
        r#""status":"failed","#,
        r#""verified_at":"200","#,
        r#""task_contract_hash":"current-task","#,
        r#""acceptance_hash":"current-acceptance","#,
        r#""checks_hash":"current-checks","#,
        r#""claims":[],"#,
        r#""commands":[{"cmd":"cargo test","exit_code":1,"duration_ms":10}],"#,
        r#""proof_sources":[],"#,
        r#""failures":["current report"]"#,
        "}\n"
    );
    let journal = concat!(
        "{",
        r#""schema_version":"maestro.verification.restore.v1","#,
        r#""previous":"old canonical report\n""#,
        "}\n"
    );
    let canonical_path = task_dir.join("verification.json");
    let journal_path = task_dir.join("verification.json.restore");
    fs::write(&canonical_path, canonical).expect("invariant: canonical report should be writable");
    fs::write(&journal_path, journal).expect("invariant: restore journal should be writable");
    write_embedded_failed_verification(repo, "task-001", "200", "cargo test");

    let out = run_success(repo, &["harness", "list"]);
    assert!(out.contains("missing_verification"));
    assert_eq!(
        fs::read_to_string(&canonical_path).expect("invariant: canonical report should read"),
        canonical
    );
    assert_eq!(
        fs::read_to_string(&journal_path).expect("invariant: restore journal should read"),
        journal
    );
}

#[test]
fn harness_reads_latest_attempt_report_commands_without_canonical_report() {
    let temp = setup_repo("maestro-improve-proof-latest-attempt");
    let repo = temp.path();
    create_task(repo, "Verify latest attempt reader");
    let task_dir = task_dir(repo, "task-001");

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
    write_embedded_failed_verification(repo, "task-001", "300", "cargo test");

    let out = run_success(repo, &["harness", "list"]);
    assert!(out.contains("missing_verification"));
    assert!(out.contains("Add reusable verification for Verify latest attempt reader"));
    let backlog = fs::read_to_string(repo.join(".maestro/harness/backlog.yaml"))
        .expect("invariant: backlog should be readable");
    assert!(
        backlog.contains("task.yaml#verification used verification command 1 outside harness.yml")
    );
    assert!(!backlog.contains("verification.json used"));
    assert!(!task_dir.join("verification.json").exists());
}

#[test]
fn harness_uses_latest_attempt_when_canonical_report_is_malformed() {
    let temp = setup_repo("maestro-improve-proof-canonical-malformed-attempt-valid");
    let repo = temp.path();
    create_task(repo, "Canonical malformed attempt valid");
    write_empty_harness(repo);

    let task_dir = task_dir(repo, "task-001");
    fs::write(task_dir.join("verification.json"), "{not-json")
        .expect("invariant: malformed canonical report should be writable");
    write_embedded_failed_verification(repo, "task-001", "325", "cargo test");

    let out = run_success(repo, &["harness", "list"]);
    assert!(out.contains("missing_verification"));
    assert!(out.contains("Add reusable verification for Canonical malformed attempt valid"));
    let backlog = fs::read_to_string(repo.join(".maestro/harness/backlog.yaml"))
        .expect("invariant: backlog should be readable");
    assert!(
        backlog.contains("task.yaml#verification used verification command 1 outside harness.yml")
    );
}

#[test]
fn harness_does_not_use_stale_canonical_commands_when_attempts_are_malformed() {
    let temp = setup_repo("maestro-improve-proof-stale-canonical-malformed-attempt");
    let repo = temp.path();
    create_task(repo, "Stale canonical malformed attempt");
    write_empty_harness(repo);

    let task_dir = task_dir(repo, "task-001");
    fs::write(
        task_dir.join("verification.json"),
        concat!(
            "{",
            r#""schema_version":"maestro.verification.v1","#,
            r#""task_id":"task-001","#,
            r#""status":"passed","#,
            r#""verified_at":"325","#,
            r#""task_contract_hash":"stale-task","#,
            r#""acceptance_hash":"stale-acceptance","#,
            r#""checks_hash":"stale-checks","#,
            r#""claims":[],"#,
            r#""commands":[{"cmd":"cargo test","exit_code":0,"duration_ms":10}],"#,
            r#""proof_sources":[],"#,
            r#""failures":[]"#,
            "}\n"
        ),
    )
    .expect("invariant: stale canonical report should be writable");
    let attempts_dir = task_dir.join("verification.attempts");
    fs::create_dir_all(&attempts_dir).expect("invariant: attempts dir should be writable");
    fs::write(attempts_dir.join("latest.json"), "{not-json")
        .expect("invariant: malformed attempt should be writable");

    let out = run_success(repo, &["harness", "list"]);
    assert!(
        out.contains("no improvement proposals found"),
        "expected malformed attempts to suppress stale canonical evidence, got:\n{out}"
    );
    assert!(!out.contains("Stale canonical malformed attempt"));
}

#[test]
fn harness_ignores_archived_attempt_when_latest_marker_is_malformed() {
    let temp = setup_repo("maestro-improve-proof-latest-malformed-archived-valid");
    let repo = temp.path();
    create_task(repo, "Latest malformed archived valid");
    write_empty_harness(repo);

    let attempts_dir = task_dir(repo, "task-001").join("verification.attempts");
    fs::create_dir_all(&attempts_dir).expect("invariant: attempts dir should be writable");
    fs::write(attempts_dir.join("latest.json"), "{not-json")
        .expect("invariant: malformed latest marker should be writable");
    fs::write(
        attempts_dir.join("zz-valid-attempt.json"),
        failed_verification_report("task-001", "350", "maestro.verification.v1"),
    )
    .expect("invariant: archived attempt report should be writable");

    let out = run_success(repo, &["harness", "list"]);
    assert!(out.contains("no improvement proposals found"));
    assert!(!out.contains("Latest malformed archived valid"));
}

#[test]
fn harness_ignores_older_archived_attempt_when_newer_archive_is_malformed() {
    let temp = setup_repo("maestro-improve-proof-newer-archive-malformed");
    let repo = temp.path();
    create_task(repo, "Newer archive malformed older valid");
    write_empty_harness(repo);

    let attempts_dir = task_dir(repo, "task-001").join("verification.attempts");
    fs::create_dir_all(&attempts_dir).expect("invariant: attempts dir should be writable");
    fs::write(attempts_dir.join("latest.json"), "{not-json")
        .expect("invariant: malformed latest marker should be writable");
    fs::write(
        attempts_dir.join("aa-valid-attempt.json"),
        failed_verification_report("task-001", "350", "maestro.verification.v1"),
    )
    .expect("invariant: older valid attempt report should be writable");
    fs::write(attempts_dir.join("zz-malformed-attempt.json"), "{not-json")
        .expect("invariant: newer malformed attempt report should be writable");

    let out = run_success(repo, &["harness", "list"]);
    assert!(out.contains("no improvement proposals found"));
    assert!(!out.contains("Newer archive malformed older valid"));
}

#[test]
fn harness_ignores_newer_archived_attempt_when_latest_marker_is_stale() {
    let temp = setup_repo("maestro-improve-proof-stale-marker-newer-archive");
    let repo = temp.path();
    create_task(repo, "Stale marker newer archive");

    fs::write(
        repo.join(".maestro/harness/harness.yml"),
        concat!(
            "schema_version: maestro.harness.v1\n",
            "stack:\n",
            "  kind: generic\n",
            "  detected_by: []\n",
            "  verify:\n",
            "    - cargo test\n"
        ),
    )
    .expect("invariant: harness should be writable");

    let attempts_dir = task_dir(repo, "task-001").join("verification.attempts");
    fs::create_dir_all(&attempts_dir).expect("invariant: attempts dir should be writable");
    fs::write(
        attempts_dir.join("latest.json"),
        failed_verification_report_with_command(
            "task-001",
            "900",
            "maestro.verification.v1",
            "cargo test",
        ),
    )
    .expect("invariant: stale latest marker should be writable");
    fs::write(
        attempts_dir.join("zz-newer-attempt.json"),
        failed_verification_report_with_command(
            "task-001",
            "1000",
            "maestro.verification.v1",
            "cargo clippy",
        ),
    )
    .expect("invariant: newer archived attempt report should be writable");

    let out = run_success(repo, &["harness", "list"]);
    assert!(out.contains("no improvement proposals found"));
    assert!(!out.contains("Stale marker newer archive"));
}

#[test]
fn harness_and_query_use_embedded_verification_over_legacy_sidecars() {
    let temp = setup_repo("maestro-improve-proof-legacy-failed-canonical-newer-attempt");
    let repo = temp.path();
    create_task(repo, "Legacy failed canonical newer attempt");

    fs::write(
        repo.join(".maestro/harness/harness.yml"),
        concat!(
            "schema_version: maestro.harness.v1\n",
            "stack:\n",
            "  kind: generic\n",
            "  detected_by: []\n",
            "  verify:\n",
            "    - cargo test\n"
        ),
    )
    .expect("invariant: harness should be writable");

    let task_dir = task_dir(repo, "task-001");
    fs::write(
        task_dir.join("verification.json"),
        failed_verification_report_with_command(
            "task-001",
            "900",
            "maestro.verification.v1",
            "cargo test",
        ),
    )
    .expect("invariant: legacy failed canonical report should be writable");
    let attempts_dir = task_dir.join("verification.attempts");
    fs::create_dir_all(&attempts_dir).expect("invariant: attempts dir should be writable");
    fs::write(
        attempts_dir.join("latest.json"),
        failed_verification_report_with_command(
            "task-001",
            "1000",
            "maestro.verification.v1",
            "cargo clippy",
        ),
    )
    .expect("invariant: newer latest attempt report should be writable");
    write_embedded_failed_verification(
        repo,
        "task-001",
        "2026-06-06T00:00:00.000Z",
        "cargo clippy",
    );

    let proof = run_success(repo, &["query", "proof", "task-001"]);
    assert!(proof.contains("task.yaml#verification"));
    assert!(proof.contains("verified_at: 2026-06-06T00:00:00.000Z"));

    let out = run_success(repo, &["harness", "list"]);
    assert!(out.contains("missing_verification"));
    assert!(out.contains("Add reusable verification for Legacy failed canonical newer attempt"));
    let backlog = fs::read_to_string(repo.join(".maestro/harness/backlog.yaml"))
        .expect("invariant: backlog should be readable");
    assert!(
        backlog.contains("task.yaml#verification used verification command 1 outside harness.yml")
    );
    assert!(!backlog.contains("verification.json used"));
    assert!(!backlog.contains("verification.attempts/latest.json used"));
}

#[test]
fn harness_ignores_atomic_temp_attempt_siblings() {
    let temp = setup_repo("maestro-improve-proof-temp-attempt-sibling");
    let repo = temp.path();
    create_task(repo, "Ignore temp attempt sibling");

    fs::write(
        repo.join(".maestro/harness/harness.yml"),
        concat!(
            "schema_version: maestro.harness.v1\n",
            "stack:\n",
            "  kind: generic\n",
            "  detected_by: []\n",
            "  verify:\n",
            "    - cargo test\n"
        ),
    )
    .expect("invariant: harness should be writable");

    let attempts_dir = task_dir(repo, "task-001").join("verification.attempts");
    fs::create_dir_all(&attempts_dir).expect("invariant: attempts dir should be writable");
    fs::write(
        attempts_dir.join("latest.json"),
        failed_verification_report_with_command(
            "task-001",
            "900",
            "maestro.verification.v1",
            "cargo test",
        ),
    )
    .expect("invariant: latest marker should be writable");
    fs::write(
        attempts_dir.join(".latest.json.tmp.fake"),
        failed_verification_report_with_command(
            "task-001",
            "1000",
            "maestro.verification.v1",
            "cargo clippy",
        ),
    )
    .expect("invariant: temp attempt sibling should be writable");

    let out = run_success(repo, &["harness", "list"]);
    let backlog = fs::read_to_string(repo.join(".maestro/harness/backlog.yaml"))
        .expect("invariant: backlog should be readable");
    assert!(
        out.contains("no improvement proposals found"),
        "expected no proposal from temp sibling, got:\n{out}\nbacklog:\n{backlog}"
    );
    assert!(!out.contains("Ignore temp attempt sibling"));
}

#[test]
fn harness_hides_secret_like_embedded_verification_commands() {
    let temp = setup_repo("maestro-improve-proof-secret-archive-name");
    let repo = temp.path();
    create_task(repo, "Secret archive name");
    write_empty_harness(repo);

    let attempts_dir = task_dir(repo, "task-001").join("verification.attempts");
    fs::create_dir_all(&attempts_dir).expect("invariant: attempts dir should be writable");
    fs::write(attempts_dir.join("latest.json"), "{not-json")
        .expect("invariant: malformed latest marker should be writable");
    fs::write(
        attempts_dir.join("api_key=top_secret.json"),
        failed_verification_report("task-001", "250", "maestro.verification.v1"),
    )
    .expect("invariant: archived attempt report should be writable");
    write_embedded_failed_verification(repo, "task-001", "250", "api_key='top secret' cargo test");

    let out = run_success(repo, &["harness", "list"]);
    assert!(out.contains("missing_verification"));
    let backlog = fs::read_to_string(repo.join(".maestro/harness/backlog.yaml"))
        .expect("invariant: backlog should be readable");
    assert!(
        backlog.contains("task.yaml#verification used verification command 1 outside harness.yml")
    );
    assert!(!backlog.contains("api_key"));
    assert!(!backlog.contains("top_secret"));
    assert!(!backlog.contains("top secret"));
}

#[cfg(unix)]
#[test]
fn harness_ignores_legacy_archived_attempt_candidate_symlink() {
    let temp = setup_repo("maestro-improve-proof-archived-symlink");
    let repo = temp.path();
    create_task(repo, "Symlink archived attempt");
    write_empty_harness(repo);

    let attempts_dir = task_dir(repo, "task-001").join("verification.attempts");
    fs::create_dir_all(&attempts_dir).expect("invariant: attempts dir should be writable");
    let external = TestTempDir::new("maestro-external-proof-attempt");
    fs::write(
        external.path().join("attempt.json"),
        failed_verification_report("task-001", "300", "maestro.verification.v1"),
    )
    .expect("invariant: external attempt should be writable");
    unix_fs::symlink(
        external.path().join("attempt.json"),
        attempts_dir.join("zz-symlink.json"),
    )
    .expect("invariant: attempt symlink should be creatable");

    let out = run_success(repo, &["harness", "list"]);
    assert!(out.contains("no improvement proposals found"));
    assert!(!out.contains("Symlink archived attempt"));
}

#[test]
fn harness_ignores_legacy_archived_attempt_candidate_directory() {
    let temp = setup_repo("maestro-improve-proof-archived-directory");
    let repo = temp.path();
    create_task(repo, "Directory archived attempt");
    write_empty_harness(repo);

    let attempts_dir = task_dir(repo, "task-001").join("verification.attempts");
    fs::create_dir_all(attempts_dir.join("zz-directory.json"))
        .expect("invariant: attempt directory should be creatable");

    let out = run_success(repo, &["harness", "list"]);
    assert!(out.contains("no improvement proposals found"));
    assert!(!out.contains("Directory archived attempt"));
}

#[test]
fn harness_distinguishes_multiple_missing_verification_commands_safely() {
    let temp = setup_repo("maestro-improve-proof-multiple-safe-labels");
    let repo = temp.path();
    create_task(repo, "Multiple command labels");
    write_empty_harness(repo);

    write_embedded_failed_verification_commands(
        repo,
        "task-001",
        "850",
        &[
            "api_key='top secret' cargo test",
            "TOKEN='other secret' cargo clippy",
        ],
    );

    let out = run_success(repo, &["harness", "list"]);
    assert!(out.contains("missing_verification"));
    let backlog = fs::read_to_string(repo.join(".maestro/harness/backlog.yaml"))
        .expect("invariant: backlog should be readable");
    assert!(
        backlog.contains("task.yaml#verification used verification command 1 outside harness.yml")
    );
    assert!(
        backlog.contains("task.yaml#verification used verification command 2 outside harness.yml")
    );
    assert!(!backlog.contains("top secret"));
    assert!(!backlog.contains("other secret"));
    assert!(!backlog.contains("api_key"));
    assert!(!backlog.contains("TOKEN"));
}

#[test]
fn harness_skips_malformed_proof_reports_and_continues_scanning() {
    let temp = setup_repo("maestro-improve-proof-malformed");
    let repo = temp.path();
    create_task(repo, "Malformed proof report");
    create_task(repo, "Healthy proof report");

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
    fs::write(
        task_dir(repo, "task-001").join("verification.json"),
        "{not-json",
    )
    .expect("invariant: malformed report should be writable");
    write_embedded_failed_verification(repo, "task-002", "400", "cargo test");

    let out = run_success(repo, &["harness", "list"]);
    assert!(out.contains("missing_verification"));
    assert!(out.contains("Add reusable verification for Healthy proof report"));
    assert!(!out.contains("Malformed proof report"));
}

#[test]
fn harness_skips_malformed_latest_attempt_reports_and_continues_scanning() {
    let temp = setup_repo("maestro-improve-proof-malformed-attempt");
    let repo = temp.path();
    create_task(repo, "Malformed attempt report");
    create_task(repo, "Healthy attempt report");

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

    let malformed_attempts = task_dir(repo, "task-001").join("verification.attempts");
    fs::create_dir_all(&malformed_attempts).expect("invariant: attempts dir should be writable");
    fs::write(malformed_attempts.join("latest.json"), "{not-json")
        .expect("invariant: malformed attempt should be writable");

    let healthy_attempts = task_dir(repo, "task-002").join("verification.attempts");
    fs::create_dir_all(&healthy_attempts).expect("invariant: attempts dir should be writable");
    fs::write(healthy_attempts.join("latest.json"), "{not-json")
        .expect("invariant: legacy attempt should be writable");
    write_embedded_failed_verification(repo, "task-002", "500", "cargo test");

    let out = run_success(repo, &["harness", "list"]);
    assert!(out.contains("missing_verification"));
    assert!(out.contains("Add reusable verification for Healthy attempt report"));
    assert!(!out.contains("Malformed attempt report"));
}

#[test]
fn harness_skips_schema_mismatched_proof_reports_and_continues_scanning() {
    let temp = setup_repo("maestro-improve-proof-schema-mismatch");
    let repo = temp.path();
    create_task(repo, "Schema mismatched proof report");
    create_task(repo, "Healthy proof report");
    write_empty_harness(repo);

    fs::write(
        task_dir(repo, "task-001").join("verification.json"),
        failed_verification_report("task-001", "600", "maestro.verification.v0"),
    )
    .expect("invariant: schema mismatched report should be writable");
    write_embedded_failed_verification(repo, "task-002", "700", "cargo test");

    let out = run_success(repo, &["harness", "list"]);
    assert!(out.contains("missing_verification"));
    assert!(out.contains("Add reusable verification for Healthy proof report"));
    assert!(!out.contains("Schema mismatched proof report"));
}

#[test]
fn harness_ignores_legacy_canonical_proof_report_path_directory() {
    let temp = setup_repo("maestro-improve-proof-report-directory");
    let repo = temp.path();
    create_task(repo, "Directory proof report");
    write_empty_harness(repo);

    fs::create_dir(task_dir(repo, "task-001").join("verification.json"))
        .expect("invariant: proof report directory should be creatable");

    let out = run_success(repo, &["harness", "list"]);
    assert!(out.contains("no improvement proposals found"));
    assert!(!out.contains("Directory proof report"));
}

#[test]
fn harness_ignores_legacy_verification_attempts_path_file() {
    let temp = setup_repo("maestro-improve-proof-attempts-file");
    let repo = temp.path();
    create_task(repo, "Attempts file proof report");
    write_empty_harness(repo);

    fs::write(
        task_dir(repo, "task-001").join("verification.attempts"),
        "not a directory",
    )
    .expect("invariant: attempts file should be writable");

    let out = run_success(repo, &["harness", "list"]);
    assert!(out.contains("no improvement proposals found"));
    assert!(!out.contains("Attempts file proof report"));
}

#[test]
fn mcp_serve_lists_tools_and_calls_status_over_stdio() {
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
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"maestro_status","arguments":{}}}"#,
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
    assert_eq!(tools.len(), 16);
    assert!(
        tools
            .iter()
            .any(|tool| tool["name"] == "maestro_feature_start")
    );
    assert!(
        tools
            .iter()
            .any(|tool| tool["name"] == "maestro_feature_ship")
    );
    assert!(
        tools
            .iter()
            .any(|tool| tool["name"] == "maestro_task_claim")
    );
    assert!(tools.iter().any(|tool| tool["name"] == "maestro_status"));
    assert!(tools.iter().any(|tool| tool["name"] == "maestro_sync"));
    assert!(
        lines[2]["result"]["content"][0]["text"]
            .as_str()
            .expect("invariant: tool response should contain text")
            .contains("Tasks: 1")
    );
}

#[test]
fn mcp_tool_aliases_list_available_tools() {
    let temp = setup_repo("maestro-mcp-aliases");
    let repo = temp.path();

    for args in [["mcp", "tools"], ["mcp", "list"]] {
        let output = run_success(repo, &args);
        assert!(output.contains("maestro_status"));
        assert!(output.contains("maestro_task_list"));
    }
}

#[test]
fn mcp_stdio_alias_runs_server() {
    let temp = setup_repo("maestro-mcp-stdio-alias");
    let repo = temp.path();
    run_success(repo, &["mcp", "stdin"]);
    run_success(repo, &["mcp", "stdio"]);
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
            "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"maestro_status\",\"arguments\":{}}}",
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
    assert!(
        String::from_utf8(output.stdout.clone())
            .expect("invariant: MCP output should be UTF-8")
            .starts_with("Content-Length: ")
    );
    let frames = parse_mcp_frames(&output.stdout);
    assert_eq!(frames[0]["id"], 1);
}

#[test]
fn mcp_serve_accepts_newline_delimited_json_rpc() {
    let temp = setup_repo("maestro-mcp-line-json");
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
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\",\"params\":{}}\n")
        .expect("invariant: MCP request should be writable");
    drop(child.stdin.take());

    let output = child
        .wait_with_output()
        .expect("invariant: mcp serve should return after stdin closes");
    assert_success(&output, &["mcp", "serve"]);
    let frames = parse_mcp_frames(&output.stdout);
    assert!(
        frames[0]["result"]["tools"]
            .as_array()
            .expect("invariant: tools should be an array")
            .iter()
            .any(|tool| tool["name"] == "maestro_status")
    );
}

#[test]
fn mcp_serve_accepts_list_method_alias() {
    let temp = setup_repo("maestro-mcp-list-method");
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
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"list\",\"params\":{}}\n")
        .expect("invariant: MCP request should be writable");
    drop(child.stdin.take());

    let output = child
        .wait_with_output()
        .expect("invariant: mcp serve should return after stdin closes");
    assert_success(&output, &["mcp", "serve"]);
    let frames = parse_mcp_frames(&output.stdout);
    assert!(
        frames[0]["result"]["tools"]
            .as_array()
            .expect("invariant: tools should be an array")
            .iter()
            .any(|tool| tool["name"] == "maestro_status")
    );
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
    assert!(
        lines[0]["error"]["message"]
            .as_str()
            .expect("invariant: error message should be a string")
            .contains("missing tool name")
    );
    assert!(
        lines[1]["error"]["message"]
            .as_str()
            .expect("invariant: error message should be a string")
            .contains("unknown MCP tool")
    );
    assert!(
        lines[2]["error"]["message"]
            .as_str()
            .expect("invariant: error message should be a string")
            .contains("claims must contain exactly one claim")
    );
}

#[test]
fn harness_show_preserves_legacy_minimal_backlog_items() {
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

    let list = run_success(repo, &["harness", "list"]);
    assert!(list.contains("hb-legacy"));
    assert!(list.contains("proposed"));
    assert!(list.contains("unknown"));

    let show = run_success(repo, &["harness", "show", "hb-legacy"]);
    assert!(show.contains("status: proposed"));
    assert!(show.contains("type: unknown"));
    assert!(show.contains("priority: medium"));
}

#[test]
fn harness_refuses_symlinked_harness_backlog_paths() {
    let temp = setup_repo("maestro-improve-symlink");
    let repo = temp.path();
    let external = TestTempDir::new("maestro-improve-external");
    fs::remove_dir_all(repo.join(".maestro/harness"))
        .expect("invariant: harness dir should be removable");
    unix_fs::symlink(external.path(), repo.join(".maestro/harness"))
        .expect("invariant: symlink should be creatable");

    let output = maestro(repo, &["harness", "list"]);
    assert!(
        !output.status.success(),
        "improve list should reject symlinked harness path\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("symlink"));
    assert!(!external.path().join("backlog.yaml").exists());
}

fn write_harness_verify(repo: &Path, commands: &[&str]) {
    let mut yaml = String::from(
        "schema_version: maestro.harness.v1\nstack:\n  kind: generic\n  detected_by: []\n  verify:",
    );
    if commands.is_empty() {
        yaml.push_str(" []\n");
    } else {
        yaml.push('\n');
        for command in commands {
            yaml.push_str(&format!("    - {command}\n"));
        }
    }
    fs::write(repo.join(".maestro/harness/harness.yml"), yaml)
        .expect("invariant: harness should be writable");
}

/// Set up a repo whose only proposal is a `missing_verification` note (hb-001).
/// The note fires because task-001's report records `cargo clippy`, absent from the
/// empty harness verify list. Adding `cargo clippy` to verify later silences it.
fn setup_missing_verification_note(prefix: &str) -> TestTempDir {
    let temp = setup_repo(prefix);
    let repo = temp.path();
    create_task(repo, "Reusable verification");
    write_harness_verify(repo, &[]);
    write_embedded_failed_verification(repo, "task-001", "900", "cargo clippy");
    let list = run_success(repo, &["harness", "list"]);
    assert!(list.contains("missing_verification"));
    temp
}

#[test]
fn harness_apply_spawns_task_and_rejects_reaccept() {
    let temp = setup_missing_verification_note("maestro-harness-apply-spawn");
    let repo = temp.path();

    let apply = run_success(repo, &["harness", "apply", "hb-001"]);
    assert!(apply.contains("accepted hb-001"));
    assert!(apply.contains("spawned task-002"));

    // The spawned task is a real draft task.
    let show_task = run_success(repo, &["task", "show", "task-002"]);
    assert!(show_task.contains("Reusable verification"));

    // Re-accepting is rejected; the task is already linked.
    let reapply = maestro(repo, &["harness", "apply", "hb-001"]);
    assert!(!reapply.status.success());
    assert!(stderr(&reapply).contains("already accepted"));
}

#[test]
fn harness_measure_closes_silent_state_note() {
    let temp = setup_missing_verification_note("maestro-harness-measure-silent");
    let repo = temp.path();

    let apply = run_success(repo, &["harness", "apply", "hb-001"]);
    assert!(apply.contains("spawned task-002"));
    // apply now presets a check and accepts the standalone task, so it is claimable.
    assert!(apply.contains("check preset:"), "{apply}");
    assert!(
        apply.contains("next: maestro task claim task-002"),
        "{apply}"
    );

    // The linked task is verified and the friction is gone (command now in verify).
    mark_verified(repo, "task-002", "general", "0", "100");
    write_harness_verify(repo, &["cargo clippy"]);

    // D7: an accepted state note whose detector is silent is flagged ready to measure.
    let ready = run_success(repo, &["harness", "list"]);
    assert!(ready.contains("ready to measure"));

    let measure = run_success(repo, &["harness", "measure", "hb-001"]);
    assert!(measure.contains("hb-001 is now measured"), "{measure}");
    // A clean close (detector silent) carries no friction warning.
    assert!(!measure.contains("friction is still detected"), "{measure}");

    let show = run_success(repo, &["harness", "show", "hb-001"]);
    assert!(show.contains("status: measured"));
    assert!(show.contains("history:"));
    assert!(show.contains("- measured"));

    // A measured note is hidden by default and only shown under --all (D4); the
    // default view says how many it hid so they don't seem to have vanished (UX-3).
    let list = run_success(repo, &["harness", "list"]);
    assert!(!list.contains("hb-001"));
    assert!(list.contains("terminal proposal(s) hidden"), "{list}");
    let all = run_success(repo, &["harness", "list", "--all"]);
    assert!(all.contains("hb-001"));
}

#[test]
fn harness_measure_resolves_a_linked_task_through_the_archive() {
    // S2-7: archiving a verified spawned task is normal terminal cleanup. The
    // measure gate and the "ready to measure" hint read only the live tree, so
    // archiving used to flip a measurable proposal to "could not be loaded; use
    // --force". Both must resolve the linked task through the archive, like
    // query proof / task show.
    let temp = setup_missing_verification_note("maestro-harness-measure-archived");
    let repo = temp.path();

    let apply = run_success(repo, &["harness", "apply", "hb-001"]);
    assert!(apply.contains("spawned task-002"));

    mark_verified(repo, "task-002", "general", "0", "100");
    write_harness_verify(repo, &["cargo clippy"]);

    // Archive the verified spawned task (normal terminal cleanup).
    run_success(repo, &["task", "archive", "task-002"]);

    // The hint still flags it ready to measure: hint and gate agree across the
    // archive boundary.
    let ready = run_success(repo, &["harness", "list"]);
    assert!(ready.contains("ready to measure"), "{ready}");

    // measure succeeds via the archive fallback, not the --force escape hatch.
    let measure = run_success(repo, &["harness", "measure", "hb-001"]);
    assert!(measure.contains("hb-001 is now measured"), "{measure}");

    let show = run_success(repo, &["harness", "show", "hb-001"]);
    assert!(show.contains("status: measured"), "{show}");
}

#[test]
fn harness_regression_reopens_measured_state_note_and_clears_link() {
    let temp = setup_missing_verification_note("maestro-harness-regress");
    let repo = temp.path();

    // Accept, verify the task, silence the friction, and measure to `measured`.
    let apply = run_success(repo, &["harness", "apply", "hb-001"]);
    assert!(apply.contains("spawned task-002"));
    mark_verified(repo, "task-002", "general", "0", "100");
    write_harness_verify(repo, &["cargo clippy"]);
    let measure = run_success(repo, &["harness", "measure", "hb-001"]);
    assert!(measure.contains("hb-001 is now measured"));

    // Friction returns (command dropped from verify again): re-deriving reopens the
    // measured state note (D6) and pulls it back into the active set.
    write_harness_verify(repo, &[]);
    let list = run_success(repo, &["harness", "list"]);
    assert!(list.contains("hb-001"));

    // The note is `proposed` again with a `regressed` record, and the old link is
    // cleared so the next accept spawns a fresh task (impl-default (c)).
    let show = run_success(repo, &["harness", "show", "hb-001"]);
    assert!(show.contains("status: proposed"));
    assert!(show.contains("- regressed"));
    assert!(!show.contains("spawned_task:"));
}

#[test]
fn harness_measure_reverts_ineffective_state_note_and_relinks_on_reapply() {
    let temp = setup_missing_verification_note("maestro-harness-measure-ineffective");
    let repo = temp.path();

    let apply = run_success(repo, &["harness", "apply", "hb-001"]);
    assert!(apply.contains("spawned task-002"));
    mark_verified(repo, "task-002", "general", "0", "100");

    // Friction persists (cargo clippy still absent from verify): the note reverts.
    let measure = run_success(repo, &["harness", "measure", "hb-001"]);
    assert!(measure.contains("hb-001 reverted to proposed"), "{measure}");
    assert!(measure.contains("ineffective"), "{measure}");

    let show = run_success(repo, &["harness", "show", "hb-001"]);
    assert!(show.contains("status: proposed"));
    assert!(show.contains("- ineffective"));
    assert!(!show.contains("spawned_task:"));

    // The cleared link means a re-accept spawns a fresh task, never the closed one.
    let reapply = run_success(repo, &["harness", "apply", "hb-001"]);
    assert!(reapply.contains("spawned task-003"));
}

#[test]
fn harness_measure_requires_verified_task_unless_forced() {
    let temp = setup_missing_verification_note("maestro-harness-measure-gate");
    let repo = temp.path();

    let apply = run_success(repo, &["harness", "apply", "hb-001"]);
    assert!(apply.contains("spawned task-002"));

    // The linked task is still a draft: measure is gated.
    let gated = maestro(repo, &["harness", "measure", "hb-001"]);
    assert!(!gated.status.success());
    assert!(stderr(&gated).contains("not verified"));

    // --force bypasses the gate, but not the verdict: the friction persists
    // (cargo clippy still absent from verify), so the note reverts to proposed.
    let forced = run_success(repo, &["harness", "measure", "hb-001", "--force"]);
    assert!(forced.contains("hb-001 reverted to proposed"), "{forced}");
}

#[test]
fn harness_measure_closes_behavioral_note_without_silence() {
    let temp = setup_repo("maestro-harness-measure-behavioral");
    let repo = temp.path();
    create_task(repo, "First blocked task");
    create_task(repo, "Second blocked task");
    write_harness_verify(repo, &[]);
    for id in ["task-001", "task-002"] {
        assert_success(
            &maestro(
                repo,
                &[
                    "task",
                    "block",
                    id,
                    "--reason",
                    "waiting for staging credentials",
                ],
            ),
            &[
                "task",
                "block",
                id,
                "--reason",
                "waiting for staging credentials",
            ],
        );
    }

    let list = run_success(repo, &["harness", "list"]);
    assert!(list.contains("recurring_blocker"));

    let apply = run_success(repo, &["harness", "apply", "hb-001"]);
    assert!(apply.contains("accepted hb-001"));
    assert!(apply.contains("spawned task-003"));
    mark_verified(repo, "task-003", "general", "0", "100");

    // The blocker still emits, but behavioral notes close on the deliberate,
    // verified-task measure with no silence check (D1). The close is honest about
    // the still-live friction (T9).
    let measure = run_success(repo, &["harness", "measure", "hb-001"]);
    assert!(measure.contains("hb-001 is now measured"), "{measure}");
    assert!(measure.contains("friction is still detected"), "{measure}");
}

#[test]
fn harness_apply_on_a_measured_behavioral_item_does_not_point_at_the_dead_end_rederive() {
    let temp = setup_repo("maestro-harness-apply-measured-behavioral");
    let repo = temp.path();
    // A measured behavioral note (recurring_blocker is not a state detector, so
    // re-detection never reopens it). A fresh repo re-derives no recurring
    // blocker, so this item survives detect_and_merge unchanged.
    fs::write(
        repo.join(".maestro/harness/backlog.yaml"),
        concat!(
            "schema_version: maestro.backlog.v1\n",
            "items:\n",
            "  - id: hb-001\n",
            "    fingerprint: recurring_blocker:waiting-on-api\n",
            "    source: aggregate\n",
            "    type: recurring_blocker\n",
            "    title: Recurring blocker waiting-on-api across tasks\n",
            "    priority: medium\n",
            "    status: measured\n",
        ),
    )
    .expect("invariant: backlog should be writable");

    let apply = maestro(repo, &["harness", "apply", "hb-001"]);
    assert!(!apply.status.success());
    let err = stderr(&apply);
    assert!(err.contains("already measured"), "{err}");
    assert!(err.contains("re-detection will not reopen it"), "{err}");
    // The re-derive remedy is a dead end for behavioral items; it must be gone.
    assert!(!err.contains("harness list"), "{err}");
}

#[test]
fn harness_apply_on_a_measured_state_detector_explains_auto_reopen_not_a_dead_end() {
    let temp = setup_repo("maestro-harness-apply-measured-state");
    let repo = temp.path();
    // A measured state-detector note whose friction is gone: a fresh repo
    // re-derives no missing_verification, so detect_and_merge leaves it measured.
    // (A live-friction state detector would have been reopened to proposed first,
    // so this arm is only reachable when the friction is already gone.)
    fs::write(
        repo.join(".maestro/harness/backlog.yaml"),
        concat!(
            "schema_version: maestro.backlog.v1\n",
            "items:\n",
            "  - id: hb-001\n",
            "    fingerprint: missing_verification:cargo clippy\n",
            "    source: reports\n",
            "    type: missing_verification\n",
            "    title: Missing verification for cargo clippy\n",
            "    priority: medium\n",
            "    status: measured\n",
        ),
    )
    .expect("invariant: backlog should be writable");

    let apply = maestro(repo, &["harness", "apply", "hb-001"]);
    assert!(!apply.status.success());
    let err = stderr(&apply);
    assert!(err.contains("already measured"), "{err}");
    assert!(err.contains("reopens automatically if it recurs"), "{err}");
    assert!(err.contains("nothing to apply now"), "{err}");
    // A state detector reopens on recurrence, so it must NOT claim it never will;
    // and re-deriving now is a dead end, so the harness-list remedy must be gone.
    assert!(!err.contains("re-detection will not reopen it"), "{err}");
    assert!(!err.contains("harness list"), "{err}");
}

#[test]
fn harness_list_withholds_ready_to_measure_until_linked_task_verified() {
    let temp = setup_missing_verification_note("maestro-harness-ready-gate");
    let repo = temp.path();

    let apply = run_success(repo, &["harness", "apply", "hb-001"]);
    assert!(apply.contains("spawned task-002"));

    // Silence the detector so the state-note is otherwise ready to measure, but
    // leave the linked task an unverified draft.
    write_harness_verify(repo, &["cargo clippy"]);

    // The no-force measure gate refuses an unverified task, so the hint must not
    // promise it (R12): a silent detector alone is not "ready to measure".
    let not_ready = run_success(repo, &["harness", "list"]);
    assert!(not_ready.contains("hb-001"), "{not_ready}");
    assert!(!not_ready.contains("ready to measure"), "{not_ready}");

    // Once the linked task is verified, the gate would pass and the hint appears.
    mark_verified(repo, "task-002", "general", "0", "100");
    let ready = run_success(repo, &["harness", "list"]);
    assert!(ready.contains("ready to measure"), "{ready}");
}

#[test]
fn harness_measure_names_force_when_the_linked_task_vanished() {
    let temp = setup_missing_verification_note("maestro-harness-measure-vanished");
    let repo = temp.path();

    let apply = run_success(repo, &["harness", "apply", "hb-001"]);
    assert!(apply.contains("spawned task-002"));

    // The linked task is deleted out from under the note (archived or removed).
    fs::remove_dir_all(task_dir(repo, "task-002"))
        .expect("invariant: spawned task dir should be removable");

    // The no-force measure can no longer load the task; instead of leaking a bare
    // "not found", it names the --force escape hatch (R23).
    let gated = maestro(repo, &["harness", "measure", "hb-001"]);
    assert!(!gated.status.success());
    let err = stderr(&gated);
    assert!(err.contains("could not be loaded"), "{err}");
    assert!(err.contains("use --force to measure anyway"), "{err}");
}
