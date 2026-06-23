pub mod card_support;
mod support;

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::os::unix::fs as unix_fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use card_support::{card_dir, card_doc, card_record_path, id_by_title, sole_idea_id, task_record};
use serde_json::Value as JsonValue;
use serde_yaml::{Mapping as YamlMapping, Value as YamlValue};
use support::TestTempDir;

const BASE_HARNESS_YAML: &str = concat!(
    "schema_version: maestro.harness.v1\n",
    "stack:\n",
    "  kind: generic\n",
    "  detected_by: []\n",
    "  verify: []\n"
);
const MCP_PROCESS_TIMEOUT: Duration = Duration::from_secs(10);

fn maestro(cwd: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("invariant: compiled maestro binary should run in integration tests")
}

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
    write_harness_yaml(repo, "");
}

fn write_harness_yaml(repo: &Path, extra: &str) {
    fs::write(
        repo.join(".maestro/harness/harness.yml"),
        format!("{BASE_HARNESS_YAML}{extra}"),
    )
    .expect("invariant: harness should be writable");
}

fn write_claims_only_harness(repo: &Path) {
    write_harness_yaml(repo, "claims_only_verification: true\n");
}

fn write_enabled_harness(repo: &Path) {
    write_harness_yaml(
        repo,
        concat!(
            "escalation:\n",
            "  enabled: true\n",
            "  warn_after: 2\n",
            "  act_after: 3\n"
        ),
    );
}

fn write_audit_harness(repo: &Path, every_sessions: usize) {
    write_harness_yaml(
        repo,
        &format!("audit:\n  every_sessions: {every_sessions}\n"),
    );
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

#[test]
fn explicit_intervention_events_cluster_into_harness_backlog() {
    let temp = setup_repo("maestro-explicit-intervention");
    let repo = temp.path();

    for session in ["sess-a", "sess-b"] {
        let args = [
            "event",
            "intervention",
            "--note",
            "used grep; standard is rg",
            "--topic",
            "use-rg",
            "--run",
            session,
        ];
        assert_success(&maestro(repo, &args), &args);
    }

    let list = run_success(repo, &["harness", "list"]);
    let id = ids_by_type(&list)
        .get("explicit_intervention")
        .cloned()
        .expect("explicit intervention item should be listed");
    let show = run_success(repo, &["harness", "show", &id]);
    assert!(show.contains("provenance: explicit-intervention"), "{show}");
    assert!(show.contains("topic: use-rg"), "{show}");
    assert!(show.contains("sessions_hit: sess-a, sess-b"), "{show}");
    assert!(show.contains("used grep; standard is rg"), "{show}");

    let apply = run_success(repo, &["harness", "apply", &id]);
    assert!(
        apply.contains("guidance for use-rg is recorded in repo instructions"),
        "{apply}"
    );
    assert!(
        apply.contains("no new intervention events on that topic"),
        "{apply}"
    );
}

#[test]
fn agent_audit_proposals_merge_and_surface_overdue_hint() {
    let temp = setup_repo("maestro-agent-audit");
    let repo = temp.path();
    write_audit_harness(repo, 1);
    write_prompt_session(repo, "audit-needed", &["start work"]);

    let status = run_success(repo, &["status"]);
    assert!(status.contains("repo audit overdue"), "{status}");
    assert!(status.contains("skill: maestro-audit"), "{status}");

    let args = [
        "harness",
        "propose",
        "--title",
        "Document build gate",
        "--evidence",
        "README.md:1 lacks build gate",
        "--evidence",
        "TESTING.md lacks build gate",
        "--topic",
        "build-gate",
    ];
    assert_success(
        &maestro_with_env(repo, &args, &[("MAESTRO_SESSION_ID", "audit-a")]),
        &args,
    );
    assert_success(
        &maestro_with_env(repo, &args, &[("MAESTRO_SESSION_ID", "audit-b")]),
        &args,
    );

    let list = run_success(repo, &["harness", "list"]);
    let id = ids_by_type(&list)
        .get("agent_audit")
        .cloned()
        .expect("agent audit item should be listed");
    let show = run_success(repo, &["harness", "show", &id]);
    assert!(show.contains("provenance: agent-audit"), "{show}");
    assert!(show.contains("topic: build-gate"), "{show}");
    assert!(show.contains("seen: 2x/2s"), "{show}");
    assert!(
        show.contains("audit-a: README.md:1 lacks build gate"),
        "{show}"
    );
    assert!(
        show.contains("audit-a: TESTING.md lacks build gate"),
        "{show}"
    );
    assert!(
        show.contains("audit-b: README.md:1 lacks build gate"),
        "{show}"
    );
    assert!(
        show.contains("audit-b: TESTING.md lacks build gate"),
        "{show}"
    );

    let apply = run_success(repo, &["harness", "apply", &id]);
    let task_id = spawned_task_id(&apply);
    mark_verified(repo, &task_id, "audit", "0", "100");
    let measure = run_success(repo, &["harness", "measure", &id]);
    assert!(
        measure.contains(&format!("{id} is now measured")),
        "{measure}"
    );

    let runtime_args = [
        "harness",
        "propose",
        "--title",
        "Document runtime gate",
        "--evidence",
        "README.md:2 lacks runtime gate",
        "--topic",
        "runtime-gate",
    ];
    assert_success(
        &maestro_with_env(repo, &runtime_args, &[("MAESTRO_SESSION_ID", "audit-c")]),
        &runtime_args,
    );
    let list = run_success(repo, &["harness", "list"]);
    let runtime_id = ids_by_type(&list)
        .get("agent_audit")
        .cloned()
        .expect("runtime audit item should be listed");
    let apply = run_success(repo, &["harness", "apply", &runtime_id]);
    let task_id = spawned_task_id(&apply);
    mark_verified(repo, &task_id, "audit", "0", "100");
    assert_success(
        &maestro_with_env(repo, &runtime_args, &[("MAESTRO_SESSION_ID", "audit-d")]),
        &runtime_args,
    );
    let reverted = run_success(repo, &["harness", "measure", &runtime_id]);
    assert!(
        reverted.contains(&format!("{runtime_id} reverted to proposed")),
        "{reverted}"
    );
    assert!(reverted.contains("friction still detected"), "{reverted}");
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
        let fields = untabify(line)
            .split('\t')
            .map(str::to_string)
            .collect::<Vec<_>>();
        // The aligned renderer drops the empty `!` cell under untabify, so an
        // over-threshold row has TYPE at index 3 and a quiet row at index 2.
        let type_index = if fields.get(1).map(String::as_str) == Some("!") {
            3
        } else {
            2
        };
        if let Some(item_type) = fields.get(type_index) {
            ids.insert(item_type.clone(), fields[0].clone());
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
    let mut remaining = bytes;
    let mut frames = Vec::new();
    while !remaining.is_empty() {
        let header_end = remaining
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("invariant: MCP frame should include header terminator");
        let header = std::str::from_utf8(&remaining[..header_end])
            .expect("invariant: MCP frame header should be UTF-8");
        let length = header
            .strip_prefix("Content-Length: ")
            .expect("invariant: MCP frame should include content length")
            .parse::<usize>()
            .expect("invariant: MCP content length should parse");
        let body_start = header_end + 4;
        let body_end = body_start + length;
        assert!(
            remaining.len() >= body_end,
            "MCP frame body shorter than declared Content-Length"
        );
        frames.push(
            serde_json::from_slice(&remaining[body_start..body_end])
                .expect("invariant: MCP response JSON"),
        );
        remaining = &remaining[body_end..];
    }
    frames
}

fn run_mcp_requests(repo: &Path, requests: &[&str]) -> Vec<JsonValue> {
    let output = run_mcp_bytes(repo, &mcp_frames(requests));
    parse_mcp_frames(&output.stdout)
}

fn run_mcp_bytes(repo: &Path, input: &[u8]) -> std::process::Output {
    let output = run_mcp_bytes_raw(repo, input);
    assert_success(&output, &["mcp", "serve"]);
    output
}

fn run_mcp_bytes_raw(repo: &Path, input: &[u8]) -> std::process::Output {
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
        .write_all(input)
        .expect("invariant: MCP requests should be writable");
    drop(child.stdin.take());
    let deadline = Instant::now() + MCP_PROCESS_TIMEOUT;
    loop {
        if child
            .try_wait()
            .expect("invariant: mcp serve wait should be inspectable")
            .is_some()
        {
            return child
                .wait_with_output()
                .expect("invariant: mcp serve output should be collectible");
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let output = child
                .wait_with_output()
                .expect("invariant: timed out mcp serve output should be collectible");
            panic!(
                "maestro mcp serve timed out after {:?}\nstdout:\n{}\nstderr:\n{}",
                MCP_PROCESS_TIMEOUT,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        thread::sleep(Duration::from_millis(10));
    }
}

/// The card's directory, located through the store probe (a pooled task's
/// `tasks/<id>/` dir, or a flat `.maestro/cards/<id>` fixture); verification
/// sidecars land beside the record.
fn task_dir(repo: &Path, id: &str) -> PathBuf {
    card_dir(repo, id)
}

/// Read/write the card record. The task fields the old `task.yaml` carried now
/// live verbatim under the card's `extra` mapping; the top-level card header
/// (`status`/timestamps) sits above it.
fn read_card(repo: &Path, id: &str) -> YamlValue {
    let path = card_record_path(repo, id);
    let raw = fs::read_to_string(&path).expect("invariant: card record should be readable");
    serde_yaml::from_str(&raw).expect("invariant: card record should parse")
}

fn write_card(repo: &Path, id: &str, card: &YamlValue) {
    let path = card_record_path(repo, id);
    fs::write(
        &path,
        serde_yaml::to_string(card).expect("invariant: card should serialize"),
    )
    .expect("invariant: card record should be writable");
}

fn create_task(repo: &Path, title: &str) {
    assert_success(
        &maestro(repo, &["task", "create", title]),
        &["task", "create", title],
    );
}

/// Create a task and recover its minted `card-<hash>` id by the unique title;
/// the single-task verification tests then drive `task_dir`/proof/embedded
/// helpers against that opaque id rather than the retired `task-001`.
fn create_one_task(repo: &Path, title: &str) -> String {
    create_task(repo, title);
    id_by_title(repo, title)
}

/// The minted id of the only `idea` card in the repo -- the persisted form of a
/// detected backlog proposal under D7 (the backlog has no file of its own).
fn sole_backlog_id(repo: &Path) -> String {
    sole_idea_id(repo)
}

/// One idea card's persisted record serialized on its own: the card's entry
/// plucked from `ideas.yaml`, or a flat fixture dir's whole `card.yaml`.
/// Content assertions stay precise -- sibling entries in the container file
/// can neither satisfy nor trip them.
fn idea_record(repo: &Path, id: &str) -> String {
    serde_yaml::to_string(&card_doc(repo, id)).expect("invariant: idea card should serialize")
}

/// Seed a backlog item directly as its persisted form: an `idea` card at
/// `.maestro/cards/<id>/card.yaml`. The old `backlog.yaml` item mapping is copied
/// under `extra` as a legacy fat payload, with the envelope's `title`/`status`
/// mirroring it so `harness list` reads cleanly.
/// `item_fields` is the indented body that used to live under `items: - ...`,
/// minus the `id` line (the id is the directory name). Keeping the id `hb-001`
/// lets the `harness show/apply/... hb-001` literals in these tests stand.
fn seed_idea_card(repo: &Path, id: &str, title: &str, status: &str, item_fields: &str) {
    let dir = repo.join(".maestro/cards").join(id);
    fs::create_dir_all(&dir).expect("invariant: card dir should be creatable");
    let mut extra = format!("  id: {id}\n");
    for line in item_fields.lines() {
        extra.push_str("  ");
        extra.push_str(line);
        extra.push('\n');
    }
    let card = format!(
        concat!(
            "schema_version: maestro.card.v1\n",
            "id: {id}\n",
            "type: idea\n",
            "title: {title}\n",
            "status: {status}\n",
            "created_at: 2026-06-09T00:00:00Z\n",
            "updated_at: 2026-06-09T00:00:00Z\n",
            "extra:\n{extra}"
        ),
        id = id,
        title = title,
        status = status,
        extra = extra,
    );
    fs::write(dir.join("card.yaml"), card).expect("invariant: idea card should be writable");
}

fn mark_verified(repo: &Path, id: &str, domain: &str, created_at: &str, verified_at: &str) {
    let mut card = read_card(repo, id);
    // Production derives the card status from the folded record's `state`, so a
    // direct mutation of `extra.state` keeps the top-level `status` in step.
    card["status"] = YamlValue::String("verified".to_string());
    card["extra"]["state"] = YamlValue::String("verified".to_string());
    card["extra"]["created_at"] = YamlValue::String(created_at.to_string());
    card["extra"]["lane"] = YamlValue::String(domain.to_string());
    card["extra"]["verification"]["verified_at"] = YamlValue::String(verified_at.to_string());
    write_card(repo, id, &card);
}

fn task_checks(repo: &Path, id: &str) -> Vec<String> {
    let card = read_card(repo, id);
    card["extra"]["acceptance"]["checks"]
        .as_sequence()
        .expect("invariant: checks should be a sequence")
        .iter()
        .map(|value| {
            value
                .as_str()
                .expect("invariant: check should be a string")
                .to_string()
        })
        .collect()
}

fn task_state(repo: &Path, id: &str) -> String {
    task_record(repo, id)["state"]
        .as_str()
        .expect("invariant: task state should be a string")
        .to_string()
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
    let mut card = read_card(repo, id);
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
    card["extra"]["verification"] = YamlValue::Mapping(verification);
    write_card(repo, id, &card);
}

#[test]
fn harness_detects_all_rule_based_backlog_proposals_and_applies_one() {
    let temp = setup_repo("maestro-improve-rules");
    let repo = temp.path();

    // Card ids are minted opaque (`card-<hash>`), so recover each task's id by
    // the unique title it was created with rather than assuming `task-00N`.
    let mut tasks = Vec::new();
    for index in 1..=9 {
        let title = format!("Task {index}");
        create_task(repo, &title);
        tasks.push(id_by_title(repo, &title));
    }
    mark_verified(repo, &tasks[0], "billing", "0", "10000");
    mark_verified(repo, &tasks[1], "billing", "10", "10010");
    for id in &tasks[2..7] {
        mark_verified(repo, id, "general", "0", "100");
    }
    // tasks[7]/tasks[8] (Task 8/Task 9) stay live to share a blocker reason and
    // trip recurring_blocker. They are separate from the verified tasks above
    // because a done task cannot take a blocker, and keeping them unverified
    // leaves the verification-duration medians that drive missing_skill untouched.

    write_embedded_failed_verification(repo, &tasks[2], "100", "api_key='top secret' true");
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
    for id in &tasks[7..9] {
        let args = [
            "task",
            "block",
            id,
            "--reason",
            "waiting for staging credentials",
        ];
        assert_success(&maestro(repo, &args), &args);
    }
    fs::write(
        task_dir(repo, &tasks[5]).join("task.md"),
        "Decision: use replay queue for hooks\n",
    )
    .expect("invariant: task markdown should be writable");
    fs::write(
        task_dir(repo, &tasks[6]).join("task.md"),
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
    // The backlog item is now an `idea` card; its evidence lives under `extra`.
    let missing_verification_id = ids
        .get("missing_verification")
        .expect("invariant: missing_verification id should be listed");
    let backlog = idea_record(repo, missing_verification_id);
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
        let spawned = spawned_task_id(&apply);
        assert!(apply.contains(&format!("spawned {spawned}")), "{apply}");
        assert!(apply.contains("check preset:"), "{apply}");
        assert!(apply.contains("next: maestro task claim"), "{apply}");
        let claim = run_success(repo, &["task", "claim", &spawned]);
        assert!(
            claim.contains(&format!("updated {spawned} -> in_progress")),
            "{claim}"
        );
        let applied = run_success(repo, &["harness", "show", id]);
        assert!(applied.contains("status: accepted"));
        assert!(applied.contains(&format!("spawned_task: {spawned}")));
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
    assert!(
        untabify(&list).contains("ID\t!\tSTATUS\tTYPE\tSEEN\tTITLE"),
        "{list}"
    );
    assert!(
        untabify(&list).contains("!\tproposed\trecurring_intervention\t9x/3s"),
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
    // D7 collapsed the backlog into idea cards: the proposal persists verbatim
    // under the card's `extra`, and the single recurring_intervention means a sole
    // idea card.
    let card = idea_record(repo, id);
    assert!(card.contains("fingerprint: recurring_intervention:global"));
    assert_eq!(card.matches("type: recurring_intervention").count(), 1);

    let dismiss = run_success(
        repo,
        &["harness", "dismiss", id, "--reason", "already handled"],
    );
    assert!(dismiss.contains(&format!("dismissed {id}")), "{dismiss}");
    let active = run_success(repo, &["harness", "list"]);
    assert!(!active.contains("recurring_intervention"), "{active}");
    let all = run_success(repo, &["harness", "list", "--all"]);
    assert!(
        untabify(&all).contains("dismissed\trecurring_intervention"),
        "{all}"
    );
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
    let id = create_one_task(repo, "Verify proof reader contract");
    mark_verified(repo, &id, "proof", "0", "100");

    let external = TestTempDir::new("maestro-external-verification-report");
    fs::write(
        external.path().join("verification.json"),
        r#"{"commands":["cargo test"]}"#,
    )
    .expect("invariant: external report fixture should be writable");
    let task_verification = task_dir(repo, &id).join("verification.json");
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
    let id = create_one_task(repo, "Verify legacy proof commands");

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
    write_embedded_failed_verification(repo, &id, "100", "cargo test");

    let proof = maestro(repo, &["query", "proof", &id]);
    assert_success(&proof, &["query", "proof", &id]);
    assert!(stdout(&proof).contains(&format!("proof {id}: failed")));

    let out = run_success(repo, &["harness", "list"]);
    assert!(out.contains("missing_verification"));
    assert!(out.contains("Add reusable verification for Verify legacy proof commands"));
}

#[test]
fn harness_accepts_multiple_embedded_command_receipts() {
    let temp = setup_repo("maestro-improve-legacy-command-objects");
    let repo = temp.path();
    let id = create_one_task(repo, "Verify legacy command objects");
    write_empty_harness(repo);
    write_embedded_failed_verification_commands(repo, &id, "125", &["cargo test", "cargo clippy"]);

    let proof = maestro(repo, &["query", "proof", &id]);
    assert_success(&proof, &["query", "proof", &id]);

    let out = run_success(repo, &["harness", "list"]);
    assert!(out.contains("missing_verification"));
    assert!(out.contains("Add reusable verification for Verify legacy command objects"));
    let backlog = idea_record(repo, &sole_backlog_id(repo));
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
    let id = create_one_task(repo, "Verify exact sensitive command matching");

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
    write_embedded_failed_verification(repo, &id, "150", command);

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
    let task = create_one_task(repo, "Refresh stale backlog evidence");
    write_empty_harness(repo);

    write_embedded_failed_verification(repo, &task, "175", "api_key='top secret' cargo test");
    // The seeded note's fingerprint matches the live task so re-detection merges
    // into it (rather than spawning a sibling), exercising the evidence refresh.
    seed_idea_card(
        repo,
        "hb-001",
        "Add reusable verification for Refresh stale backlog evidence",
        "proposed",
        &format!(
            concat!(
                "fingerprint: missing_verification:{task}\n",
                "source: {task}\n",
                "type: missing_verification\n",
                "title: Add reusable verification for Refresh stale backlog evidence\n",
                "priority: medium\n",
                "status: proposed\n",
                "evidence:\n",
                "  - \"manual note: keep this context\"\n",
                "  - verification.json used `api_key='top secret' cargo test` outside harness.yml\n"
            ),
            task = task,
        ),
    );

    let show = run_success(repo, &["harness", "show", "hb-001"]);
    assert!(show.contains("manual note: keep this context"));
    assert!(
        show.contains("task.yaml#verification used verification command 1 outside harness.yml")
    );
    assert!(!show.contains("top secret"));
    assert!(!show.contains("api_key"));
    let backlog = idea_record(repo, "hb-001");
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
    seed_idea_card(
        repo,
        "hb-001",
        "Add stale verification",
        "accepted",
        concat!(
            "source: task-001\n",
            "type: missing_verification\n",
            "title: Add stale verification\n",
            "priority: medium\n",
            "status: accepted\n",
            "evidence:\n",
            "  - \"manual note: keep this context\"\n",
            "  - verification.attempts/api_key=top_secret.json used `api_key='top secret' cargo test` outside harness.yml\n"
        ),
    );

    let show = run_success(repo, &["harness", "show", "hb-001"]);
    assert!(show.contains("manual note: keep this context"));
    assert!(show.contains(
        "verification.attempts/archived attempt used verification command 1 outside harness.yml"
    ));
    assert!(!show.contains("top secret"));
    assert!(!show.contains("api_key"));
    let backlog = idea_record(repo, "hb-001");
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
    let id = create_one_task(repo, "Preserve proof restore journal");
    let task_dir = task_dir(repo, &id);

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
    write_embedded_failed_verification(repo, &id, "200", "cargo test");

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
    let id = create_one_task(repo, "Verify latest attempt reader");
    let card_dir = task_dir(repo, &id);

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
    write_embedded_failed_verification(repo, &id, "300", "cargo test");

    let out = run_success(repo, &["harness", "list"]);
    assert!(out.contains("missing_verification"));
    assert!(out.contains("Add reusable verification for Verify latest attempt reader"));
    let backlog = idea_record(repo, &sole_backlog_id(repo));
    assert!(
        backlog.contains("task.yaml#verification used verification command 1 outside harness.yml")
    );
    assert!(!backlog.contains("verification.json used"));
    assert!(!card_dir.join("verification.json").exists());
}

#[test]
fn harness_uses_latest_attempt_when_canonical_report_is_malformed() {
    let temp = setup_repo("maestro-improve-proof-canonical-malformed-attempt-valid");
    let repo = temp.path();
    let id = create_one_task(repo, "Canonical malformed attempt valid");
    write_empty_harness(repo);

    let card_dir = task_dir(repo, &id);
    fs::write(card_dir.join("verification.json"), "{not-json")
        .expect("invariant: malformed canonical report should be writable");
    write_embedded_failed_verification(repo, &id, "325", "cargo test");

    let out = run_success(repo, &["harness", "list"]);
    assert!(out.contains("missing_verification"));
    assert!(out.contains("Add reusable verification for Canonical malformed attempt valid"));
    let backlog = idea_record(repo, &sole_backlog_id(repo));
    assert!(
        backlog.contains("task.yaml#verification used verification command 1 outside harness.yml")
    );
}

#[test]
fn harness_does_not_use_stale_canonical_commands_when_attempts_are_malformed() {
    let temp = setup_repo("maestro-improve-proof-stale-canonical-malformed-attempt");
    let repo = temp.path();
    let id = create_one_task(repo, "Stale canonical malformed attempt");
    write_empty_harness(repo);

    let card_dir = task_dir(repo, &id);
    fs::write(
        card_dir.join("verification.json"),
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
    let attempts_dir = card_dir.join("verification.attempts");
    fs::create_dir_all(&attempts_dir).expect("invariant: attempts dir should be writable");
    fs::write(attempts_dir.join("latest.json"), "{not-json")
        .expect("invariant: malformed attempt should be writable");

    let out = run_success(repo, &["harness", "list"]);
    assert!(
        out.contains("no improvement proposals found"),
        "expected legacy proof sidecars to be ignored in card mode, got:\n{out}"
    );
    assert!(!out.contains("Stale canonical malformed attempt"));
}

#[test]
fn harness_ignores_archived_attempt_when_latest_marker_is_malformed() {
    let temp = setup_repo("maestro-improve-proof-latest-malformed-archived-valid");
    let repo = temp.path();
    let id = create_one_task(repo, "Latest malformed archived valid");
    write_empty_harness(repo);

    let attempts_dir = task_dir(repo, &id).join("verification.attempts");
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
    let id = create_one_task(repo, "Newer archive malformed older valid");
    write_empty_harness(repo);

    let attempts_dir = task_dir(repo, &id).join("verification.attempts");
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
    let id = create_one_task(repo, "Stale marker newer archive");

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

    let attempts_dir = task_dir(repo, &id).join("verification.attempts");
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
    let id = create_one_task(repo, "Legacy failed canonical newer attempt");

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

    let card_dir = task_dir(repo, &id);
    fs::write(
        card_dir.join("verification.json"),
        failed_verification_report_with_command(
            "task-001",
            "900",
            "maestro.verification.v1",
            "cargo test",
        ),
    )
    .expect("invariant: legacy failed canonical report should be writable");
    let attempts_dir = card_dir.join("verification.attempts");
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
    write_embedded_failed_verification(repo, &id, "2026-06-06T00:00:00.000Z", "cargo clippy");

    let proof = run_success(repo, &["query", "proof", &id]);
    assert!(proof.contains("task.yaml#verification"));
    assert!(proof.contains("verified_at: 2026-06-06T00:00:00.000Z"));

    let out = run_success(repo, &["harness", "list"]);
    assert!(out.contains("missing_verification"));
    assert!(out.contains("Add reusable verification for Legacy failed canonical newer attempt"));
    let backlog = idea_record(repo, &sole_backlog_id(repo));
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
    let id = create_one_task(repo, "Ignore temp attempt sibling");

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

    let attempts_dir = task_dir(repo, &id).join("verification.attempts");
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
    assert!(
        out.contains("no improvement proposals found"),
        "expected legacy proof sidecars to be ignored in card mode, got:\n{out}"
    );
    assert!(!out.contains("Ignore temp attempt sibling"));
}

#[test]
fn harness_hides_secret_like_embedded_verification_commands() {
    let temp = setup_repo("maestro-improve-proof-secret-archive-name");
    let repo = temp.path();
    let id = create_one_task(repo, "Secret archive name");
    write_empty_harness(repo);

    let attempts_dir = task_dir(repo, &id).join("verification.attempts");
    fs::create_dir_all(&attempts_dir).expect("invariant: attempts dir should be writable");
    fs::write(attempts_dir.join("latest.json"), "{not-json")
        .expect("invariant: malformed latest marker should be writable");
    fs::write(
        attempts_dir.join("api_key=top_secret.json"),
        failed_verification_report("task-001", "250", "maestro.verification.v1"),
    )
    .expect("invariant: archived attempt report should be writable");
    write_embedded_failed_verification(repo, &id, "250", "api_key='top secret' cargo test");

    let out = run_success(repo, &["harness", "list"]);
    assert!(out.contains("missing_verification"));
    let backlog = idea_record(repo, &sole_backlog_id(repo));
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
    let id = create_one_task(repo, "Symlink archived attempt");
    write_empty_harness(repo);

    let attempts_dir = task_dir(repo, &id).join("verification.attempts");
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
    let id = create_one_task(repo, "Directory archived attempt");
    write_empty_harness(repo);

    let attempts_dir = task_dir(repo, &id).join("verification.attempts");
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
    let id = create_one_task(repo, "Multiple command labels");
    write_empty_harness(repo);

    write_embedded_failed_verification_commands(
        repo,
        &id,
        "850",
        &[
            "api_key='top secret' cargo test",
            "TOKEN='other secret' cargo clippy",
        ],
    );

    let out = run_success(repo, &["harness", "list"]);
    assert!(out.contains("missing_verification"));
    let backlog = idea_record(repo, &sole_backlog_id(repo));
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
    let malformed = create_one_task(repo, "Malformed proof report");
    let healthy = create_one_task(repo, "Healthy proof report");

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
        task_dir(repo, &malformed).join("verification.json"),
        "{not-json",
    )
    .expect("invariant: malformed report should be writable");
    write_embedded_failed_verification(repo, &healthy, "400", "cargo test");

    let out = run_success(repo, &["harness", "list"]);
    assert!(out.contains("missing_verification"));
    assert!(out.contains("Add reusable verification for Healthy proof report"));
    assert!(!out.contains("Malformed proof report"));
}

#[test]
fn harness_skips_malformed_latest_attempt_reports_and_continues_scanning() {
    let temp = setup_repo("maestro-improve-proof-malformed-attempt");
    let repo = temp.path();
    let malformed = create_one_task(repo, "Malformed attempt report");
    let healthy = create_one_task(repo, "Healthy attempt report");

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

    let malformed_attempts = task_dir(repo, &malformed).join("verification.attempts");
    fs::create_dir_all(&malformed_attempts).expect("invariant: attempts dir should be writable");
    fs::write(malformed_attempts.join("latest.json"), "{not-json")
        .expect("invariant: malformed attempt should be writable");

    let healthy_attempts = task_dir(repo, &healthy).join("verification.attempts");
    fs::create_dir_all(&healthy_attempts).expect("invariant: attempts dir should be writable");
    fs::write(healthy_attempts.join("latest.json"), "{not-json")
        .expect("invariant: legacy attempt should be writable");
    write_embedded_failed_verification(repo, &healthy, "500", "cargo test");

    let out = run_success(repo, &["harness", "list"]);
    assert!(out.contains("missing_verification"));
    assert!(out.contains("Add reusable verification for Healthy attempt report"));
    assert!(!out.contains("Malformed attempt report"));
}

#[test]
fn harness_skips_schema_mismatched_proof_reports_and_continues_scanning() {
    let temp = setup_repo("maestro-improve-proof-schema-mismatch");
    let repo = temp.path();
    let mismatched = create_one_task(repo, "Schema mismatched proof report");
    let healthy = create_one_task(repo, "Healthy proof report");
    write_empty_harness(repo);

    fs::write(
        task_dir(repo, &mismatched).join("verification.json"),
        failed_verification_report("task-001", "600", "maestro.verification.v0"),
    )
    .expect("invariant: schema mismatched report should be writable");
    write_embedded_failed_verification(repo, &healthy, "700", "cargo test");

    let out = run_success(repo, &["harness", "list"]);
    assert!(out.contains("missing_verification"));
    assert!(out.contains("Add reusable verification for Healthy proof report"));
    assert!(!out.contains("Schema mismatched proof report"));
}

#[test]
fn harness_ignores_legacy_canonical_proof_report_path_directory() {
    let temp = setup_repo("maestro-improve-proof-report-directory");
    let repo = temp.path();
    let id = create_one_task(repo, "Directory proof report");
    write_empty_harness(repo);

    fs::create_dir(task_dir(repo, &id).join("verification.json"))
        .expect("invariant: proof report directory should be creatable");

    let out = run_success(repo, &["harness", "list"]);
    assert!(out.contains("no improvement proposals found"));
    assert!(!out.contains("Directory proof report"));
}

#[test]
fn harness_ignores_legacy_verification_attempts_path_file() {
    let temp = setup_repo("maestro-improve-proof-attempts-file");
    let repo = temp.path();
    let id = create_one_task(repo, "Attempts file proof report");
    write_empty_harness(repo);

    fs::write(
        task_dir(repo, &id).join("verification.attempts"),
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

    let lines = run_mcp_requests(
        repo,
        &[
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#,
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"maestro_status","arguments":{}}}"#,
        ],
    );
    let tools = lines[1]["result"]["tools"]
        .as_array()
        .expect("invariant: tools/list should return an array");
    assert_eq!(tools.len(), 34);
    assert!(tools.iter().any(|tool| tool["name"] == "maestro_task_next"));
    assert!(
        tools
            .iter()
            .any(|tool| tool["name"] == "maestro_task_create")
    );
    assert!(
        tools
            .iter()
            .any(|tool| tool["name"] == "maestro_task_update")
    );
    assert!(
        tools
            .iter()
            .any(|tool| tool["name"] == "maestro_feature_start")
    );
    assert!(
        tools
            .iter()
            .any(|tool| tool["name"] == "maestro_feature_close")
    );
    assert!(
        tools
            .iter()
            .any(|tool| tool["name"] == "maestro_task_claim")
    );
    assert!(tools.iter().any(|tool| tool["name"] == "maestro_status"));
    assert!(
        tools
            .iter()
            .any(|tool| tool["name"] == "maestro_card_ready")
    );
    assert!(
        tools
            .iter()
            .any(|tool| tool["name"] == "maestro_card_graph")
    );
    let task_complete_tool = tools
        .iter()
        .find(|tool| tool["name"] == "maestro_task_complete")
        .expect("invariant: maestro_task_complete should be listed");
    assert!(
        !task_complete_tool["inputSchema"]["anyOf"].is_null(),
        "maestro_task_complete schema requires claim or claims:\n{task_complete_tool}"
    );
    let card_create_tool = tools
        .iter()
        .find(|tool| tool["name"] == "maestro_card_create")
        .expect("invariant: maestro_card_create should be listed");
    let card_create_intents = card_create_tool["inputSchema"]["properties"]["intent"]["enum"]
        .as_array()
        .expect("invariant: card_create intent enum should be listed");
    assert!(
        card_create_intents.iter().any(|intent| intent == "idea")
            && !card_create_intents
                .iter()
                .any(|intent| intent == "followup"),
        "maestro_card_create schema advertises actual card types:\n{card_create_tool}"
    );
    assert!(tools.iter().any(|tool| tool["name"] == "maestro_sync"));
    assert!(
        lines[2]["result"]["content"][0]["text"]
            .as_str()
            .expect("invariant: tool response should contain text")
            .contains("Tasks: 1")
    );
    assert!(
        lines[2]["result"]["content"][0]["text"]
            .as_str()
            .expect("invariant: tool response should contain text")
            .contains("MCP workflow guidance")
    );
}

#[test]
fn mcp_lifecycle_tools_expose_schemas_and_blocked_envelope() {
    let temp = setup_repo("maestro-mcp-lifecycle-envelope");
    let repo = temp.path();
    run_success(
        repo,
        &[
            "feature",
            "new",
            "MCP envelope feature",
            "--description",
            "feature for MCP envelope test",
        ],
    );

    let frames = run_mcp_requests(
        repo,
        &[
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"maestro_feature_prepare","arguments":{"feature_id":"mcp-envelope-feature","tasks":[{"title":"work","checks":["done"],"covers":["ac-1"]}]}}}"#,
        ],
    );
    let tools = frames[0]["result"]["tools"]
        .as_array()
        .expect("invariant: tools/list should return tools");
    for name in [
        "maestro_qa_baseline",
        "maestro_feature_accept",
        "maestro_feature_prepare",
        "maestro_feature_verify",
        "maestro_qa_slice",
        "maestro_feature_close",
    ] {
        assert!(
            tools.iter().any(|tool| tool["name"] == name),
            "{name} should be listed"
        );
    }

    let accept_schema = tools
        .iter()
        .find(|tool| tool["name"] == "maestro_feature_accept")
        .expect("invariant: accept tool should be listed")["inputSchema"]
        .clone();
    assert_eq!(
        accept_schema["properties"]["qa"]["oneOf"][0]["properties"]["mode"]["const"],
        "recorded_baseline"
    );
    assert_eq!(
        accept_schema["properties"]["qa"]["oneOf"][1]["properties"]["mode"]["const"],
        "none"
    );
    assert!(
        accept_schema["properties"]["qa"]["oneOf"][1]["required"]
            .as_array()
            .expect("invariant: required should be an array")
            .iter()
            .any(|item| item == "reason")
    );

    let text = frames[1]["result"]["content"][0]["text"]
        .as_str()
        .expect("invariant: blocked lifecycle result should be text");
    let envelope: JsonValue =
        serde_json::from_str(text).expect("blocked lifecycle result should be JSON");
    assert_eq!(envelope["ok"], false);
    assert_eq!(envelope["changed"], false);
    assert_eq!(envelope["tool"], "maestro_feature_prepare");
    assert_eq!(envelope["target"]["id"], "mcp-envelope-feature");
    assert_eq!(envelope["state_before"], "proposed");
    assert_eq!(envelope["state_after"], "proposed");
    assert_eq!(envelope["blocked"], true);
    assert!(envelope["reason_code"].as_str().is_some());
    assert!(envelope["message"].as_str().is_some());
    assert!(envelope["prerequisites"].is_array());
    assert!(envelope["valid_next"].is_array());
    assert!(envelope["raw"].as_str().is_some());
}

#[test]
fn mcp_intake_lifecycle_tools_accept_and_prepare_feature() {
    let temp = setup_repo("maestro-mcp-intake-lifecycle");
    let repo = temp.path();
    run_success(
        repo,
        &[
            "feature",
            "new",
            "MCP intake feature",
            "--description",
            "feature for MCP intake test",
        ],
    );
    run_success(
        repo,
        &[
            "feature",
            "set",
            "mcp-intake-feature",
            "--acceptance",
            "MCP intake AC is covered",
            "--area",
            "src/interfaces/mcp/tools.rs",
        ],
    );

    let requests = [
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"maestro_qa_baseline","arguments":{"feature_id":"mcp-intake-feature","observed":"[bl-001] current intake behavior is raw CLI only"}}}"#.to_string(),
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"maestro_feature_accept","arguments":{"feature_id":"mcp-intake-feature","qa":{"mode":"recorded_baseline"}}}}"#.to_string(),
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"maestro_feature_prepare","arguments":{"feature_id":"mcp-intake-feature","tasks":[{"title":"MCP intake task","checks":["MCP intake AC is covered"],"covers":["ac-1"]}]}}}"#.to_string(),
    ];
    let request_refs = requests.iter().map(String::as_str).collect::<Vec<_>>();
    let frames = run_mcp_requests(repo, &request_refs);

    for frame in &frames {
        let text = frame["result"]["content"][0]["text"]
            .as_str()
            .expect("invariant: lifecycle tool should return text");
        let envelope: JsonValue =
            serde_json::from_str(text).expect("lifecycle tool should return JSON");
        assert_eq!(envelope["ok"], true, "{text}");
        assert_eq!(envelope["blocked"], false, "{text}");
        assert_eq!(envelope["changed"], true, "{text}");
    }

    let feature = read_card(repo, "mcp-intake-feature");
    assert_eq!(
        feature["status"],
        YamlValue::String("in_progress".to_string())
    );
    let task_id = id_by_title(repo, "MCP intake task");
    let task = task_record(repo, &task_id);
    assert_eq!(task["state"], YamlValue::String("ready".to_string()));
    assert_eq!(
        task["covers"],
        YamlValue::Sequence(vec![YamlValue::String("ac-1".to_string())])
    );
}

#[test]
fn mcp_feature_accept_rejects_invalid_qa_shapes_before_cli() {
    let temp = setup_repo("maestro-mcp-invalid-qa");
    let repo = temp.path();
    run_success(repo, &["feature", "new", "MCP invalid QA"]);

    let frames = run_mcp_requests(
        repo,
        &[
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"maestro_feature_accept","arguments":{"feature_id":"mcp-invalid-qa","qa":"free text"}}}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"maestro_feature_accept","arguments":{"feature_id":"mcp-invalid-qa","qa":{"mode":"recorded_baseline","reason":"not allowed"}}}}"#,
        ],
    );

    assert!(
        frames[0]["error"]["message"]
            .as_str()
            .expect("invariant: invalid QA shape should be an MCP error")
            .contains("qa.mode")
    );
    assert!(
        frames[1]["error"]["message"]
            .as_str()
            .expect("invariant: invalid QA reason should be an MCP error")
            .contains("qa.reason is only valid when qa.mode is none")
    );
    let feature = read_card(repo, "mcp-invalid-qa");
    assert_eq!(feature["status"], YamlValue::String("proposed".to_string()));
}

#[test]
fn mcp_qa_lifecycle_tools_require_observed_evidence() {
    let temp = setup_repo("maestro-mcp-qa-evidence");
    let repo = temp.path();
    run_success(repo, &["feature", "new", "MCP QA evidence"]);

    let frames = run_mcp_requests(
        repo,
        &[
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"maestro_qa_baseline","arguments":{"feature_id":"mcp-qa-evidence","observed":""}}}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"maestro_qa_slice","arguments":{"feature_id":"mcp-qa-evidence","scenarios":["bl-001"]}}}"#,
        ],
    );

    assert!(
        frames[0]["error"]["message"]
            .as_str()
            .expect("invariant: empty observed should be an MCP error")
            .contains("observed must not be empty")
    );
    assert!(
        frames[1]["error"]["message"]
            .as_str()
            .expect("invariant: missing observed should be an MCP error")
            .contains("missing required argument: observed")
    );
}

#[test]
fn mcp_feature_gate_tools_verify_slice_and_close_without_autoclose() {
    let temp = setup_repo("maestro-mcp-feature-gate");
    let repo = temp.path();
    write_claims_only_harness(repo);
    run_success(
        repo,
        &[
            "feature",
            "new",
            "MCP feature gate",
            "--description",
            "feature for MCP feature gate test",
        ],
    );
    run_success(
        repo,
        &[
            "feature",
            "set",
            "mcp-feature-gate",
            "--acceptance",
            "MCP feature gate AC is covered",
            "--area",
            "src/interfaces/mcp/tools.rs",
        ],
    );
    let intake_requests = [
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"maestro_qa_baseline","arguments":{"feature_id":"mcp-feature-gate","observed":"[bl-001] feature gate currently closes only through CLI"}}}"#.to_string(),
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"maestro_feature_accept","arguments":{"feature_id":"mcp-feature-gate","qa":{"mode":"recorded_baseline"}}}}"#.to_string(),
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"maestro_feature_prepare","arguments":{"feature_id":"mcp-feature-gate","tasks":[{"title":"MCP feature gate task","checks":["MCP feature gate AC is covered"],"covers":["ac-1"]}]}}}"#.to_string(),
    ];
    let intake_refs = intake_requests
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    run_mcp_requests(repo, &intake_refs);

    let task_id = id_by_title(repo, "MCP feature gate task");
    run_success(repo, &["task", "claim", &task_id]);
    run_success(
        repo,
        &[
            "task",
            "complete",
            &task_id,
            "--summary",
            "done",
            "--claim",
            "MCP feature gate AC is covered",
            "--proof",
            "MCP feature gate AC is covered",
        ],
    );

    let close_requests = [
        r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"maestro_feature_verify","arguments":{"feature_id":"mcp-feature-gate","prove":["ac-1"],"evidence":["MCP feature gate AC is covered"]}}}"#.to_string(),
        r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"maestro_qa_slice","arguments":{"feature_id":"mcp-feature-gate","scenarios":["bl-001"],"observed":"MCP feature gate scenario still passes"}}}"#.to_string(),
        r#"{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"maestro_feature_close","arguments":{"feature_id":"mcp-feature-gate","dry_run":true}}}"#.to_string(),
        r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"maestro_feature_close","arguments":{"feature_id":"mcp-feature-gate","outcome":"MCP feature gate closed through typed MCP tools"}}}"#.to_string(),
    ];
    let close_refs = close_requests
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let frames = run_mcp_requests(repo, &close_refs);

    let verify_text = frames[0]["result"]["content"][0]["text"]
        .as_str()
        .expect("invariant: verify should return text");
    let verify_envelope: JsonValue =
        serde_json::from_str(verify_text).expect("verify should return envelope JSON");
    assert_eq!(verify_envelope["ok"], true, "{verify_text}");
    assert_eq!(
        verify_envelope["state_after"], "in_progress",
        "{verify_text}"
    );
    assert!(
        verify_envelope["valid_next"]
            .as_array()
            .expect("invariant: valid_next should be an array")
            .iter()
            .any(|entry| entry["tool"] == "maestro_feature_close"),
        "{verify_text}"
    );

    let slice_text = frames[1]["result"]["content"][0]["text"]
        .as_str()
        .expect("invariant: qa slice should return text");
    let slice_envelope: JsonValue =
        serde_json::from_str(slice_text).expect("qa slice should return envelope JSON");
    assert_eq!(slice_envelope["ok"], true, "{slice_text}");

    let dry_run_text = frames[2]["result"]["content"][0]["text"]
        .as_str()
        .expect("invariant: dry-run close should return text");
    let dry_run_envelope: JsonValue =
        serde_json::from_str(dry_run_text).expect("dry-run close should return envelope JSON");
    assert_eq!(dry_run_envelope["ok"], true, "{dry_run_text}");
    assert_eq!(dry_run_envelope["changed"], false, "{dry_run_text}");
    assert_eq!(
        dry_run_envelope["state_after"], "in_progress",
        "{dry_run_text}"
    );

    let close_text = frames[3]["result"]["content"][0]["text"]
        .as_str()
        .expect("invariant: close should return text");
    let close_envelope: JsonValue =
        serde_json::from_str(close_text).expect("close should return envelope JSON");
    assert_eq!(close_envelope["ok"], true, "{close_text}");
    assert_eq!(close_envelope["state_after"], "closed", "{close_text}");
    let feature = read_card(repo, "mcp-feature-gate");
    assert_eq!(feature["status"], YamlValue::String("closed".to_string()));
}

#[test]
fn mcp_task_tools_drive_normal_lifecycle_over_stdio() {
    let temp = setup_repo("maestro-mcp-task-lifecycle");
    let repo = temp.path();
    write_claims_only_harness(repo);

    let first_frames = run_mcp_requests(
        repo,
        &[
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"maestro_task_create","arguments":{"title":"MCP normal task","lane":"normal","risk":"low","checks":["first claim passed","second claim passed"]}}}"#,
        ],
    );
    let tools = first_frames[0]["result"]["tools"]
        .as_array()
        .expect("invariant: tools/list should return tools");
    for name in [
        "maestro_task_next",
        "maestro_task_create",
        "maestro_task_explore",
        "maestro_task_accept",
        "maestro_task_update",
    ] {
        assert!(
            tools.iter().any(|tool| tool["name"] == name),
            "{name} should be listed"
        );
    }
    assert!(
        first_frames[1]["result"]["content"][0]["text"]
            .as_str()
            .expect("invariant: create returns text")
            .contains("created")
    );

    let id = id_by_title(repo, "MCP normal task");
    let requests = [
        format!(
            r#"{{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{{"name":"maestro_task_explore","arguments":{{"id":"{id}"}}}}}}"#
        ),
        format!(
            r#"{{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{{"name":"maestro_task_accept","arguments":{{"id":"{id}"}}}}}}"#
        ),
        r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"maestro_task_next","arguments":{}}}"#.to_string(),
        format!(
            r#"{{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{{"name":"maestro_task_claim","arguments":{{"id":"{id}"}}}}}}"#
        ),
        format!(
            r#"{{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{{"name":"maestro_task_update","arguments":{{"id":"{id}","summary":"progress checkpoint","claims":["first claim passed","second claim passed"]}}}}}}"#
        ),
        format!(
            r#"{{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{{"name":"maestro_task_complete","arguments":{{"id":"{id}","summary":"done through MCP","claims":["first claim passed","second claim passed"],"proof":["first claim passed","second claim passed"]}}}}}}"#
        ),
        format!(
            r#"{{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{{"name":"maestro_verify","arguments":{{"id":"{id}"}}}}}}"#
        ),
    ];
    let request_refs = requests.iter().map(String::as_str).collect::<Vec<_>>();
    let frames = run_mcp_requests(repo, &request_refs);

    let next_text = frames[2]["result"]["content"][0]["text"]
        .as_str()
        .expect("invariant: task_next returns text");
    let next_json: JsonValue =
        serde_json::from_str(next_text).expect("task_next returns JSON text");
    assert!(
        !next_json["structured"].is_null(),
        "task_next includes structured output:\n{next_text}"
    );
    assert!(
        next_json["raw"].as_str().is_some(),
        "task_next includes raw output:\n{next_text}"
    );
    assert!(
        frames[5]["result"]["content"][0]["text"]
            .as_str()
            .expect("invariant: complete returns text")
            .contains("verification passed")
    );

    let doc = task_record(repo, &id);
    assert_eq!(doc["state"], YamlValue::String("verified".to_string()));
}

#[test]
fn mcp_card_tools_drive_lifecycle_ready_and_graph_over_stdio() {
    let temp = setup_repo("maestro-mcp-card-lifecycle");
    let repo = temp.path();

    let first_requests = [
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#.to_string(),
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"maestro_card_create","arguments":{"intent":"feature","title":"MCP Parent","problem":"parent feature"}}}"#.to_string(),
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"maestro_card_create","arguments":{"intent":"task","title":"MCP Card Task","parent":"mcp-parent","problem":"work item","acceptance":"ship it"}}}"#.to_string(),
    ];
    let first_refs = first_requests
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let first_frames = run_mcp_requests(repo, &first_refs);
    let tools = first_frames[0]["result"]["tools"]
        .as_array()
        .expect("invariant: tools/list should return tools");
    for name in [
        "maestro_card_create",
        "maestro_card_list",
        "maestro_card_show",
        "maestro_card_ready",
        "maestro_card_claim",
        "maestro_card_update",
        "maestro_card_close",
        "maestro_card_graph",
    ] {
        assert!(
            tools.iter().any(|tool| tool["name"] == name),
            "{name} should be listed"
        );
    }

    let id = id_by_title(repo, "MCP Card Task");
    let requests = [
        r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"maestro_card_ready","arguments":{"feature":"mcp-parent"}}}"#.to_string(),
        format!(
            r#"{{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{{"name":"maestro_card_show","arguments":{{"id":"{id}","json":true}}}}}}"#
        ),
        format!(
            r#"{{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{{"name":"maestro_card_graph","arguments":{{"id":"{id}"}}}}}}"#
        ),
        format!(
            r#"{{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{{"name":"maestro_card_claim","arguments":{{"id":"{id}"}}}}}}"#
        ),
        format!(
            r#"{{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{{"name":"maestro_card_update","arguments":{{"id":"{id}","progress":"half done","claim":true}}}}}}"#
        ),
        r#"{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"maestro_card_list","arguments":{"type":"task","json":true}}}"#.to_string(),
        format!(
            r#"{{"jsonrpc":"2.0","id":10,"method":"tools/call","params":{{"name":"maestro_card_close","arguments":{{"id":"{id}"}}}}}}"#
        ),
    ];
    let request_refs = requests.iter().map(String::as_str).collect::<Vec<_>>();
    let frames = run_mcp_requests(repo, &request_refs);
    assert!(
        frames[0]["result"]["content"][0]["text"]
            .as_str()
            .expect("invariant: ready returns text")
            .contains(&id)
    );
    assert!(
        frames[2]["result"]["content"][0]["text"]
            .as_str()
            .expect("invariant: graph returns text")
            .contains("mcp-parent")
    );
    assert!(
        frames[5]["result"]["content"][0]["text"]
            .as_str()
            .expect("invariant: list returns text")
            .contains(&id)
    );
    assert_eq!(
        card_doc(repo, &id)["status"],
        YamlValue::String("closed".to_string())
    );
}

#[test]
fn mcp_decision_list_windows_by_default_and_all_reaches_full() {
    let temp = setup_repo("maestro-mcp-decision-all");
    let repo = temp.path();
    for i in 1..=21 {
        run_success(repo, &["decision", "new", &format!("MCP decision {i:02}")]);
    }

    let lines = run_mcp_requests(
        repo,
        &[
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#,
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"maestro_decision_list","arguments":{}}}"#,
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"maestro_decision_list","arguments":{"all":true}}}"#,
        ],
    );

    // ac-5: the tool advertises the `all` boolean param.
    let tools = lines[1]["result"]["tools"]
        .as_array()
        .expect("invariant: tools/list should return an array");
    let decision_tool = tools
        .iter()
        .find(|tool| tool["name"] == "maestro_decision_list")
        .expect("invariant: maestro_decision_list should be listed");
    assert!(
        !decision_tool["inputSchema"]["properties"]["all"].is_null(),
        "maestro_decision_list advertises the all param:\n{decision_tool}"
    );

    let default_text = lines[2]["result"]["content"][0]["text"]
        .as_str()
        .expect("invariant: default call returns text");
    assert!(
        default_text.contains("20 of 21 recent"),
        "default MCP decision_list inherits the recent-20 window:\n{default_text}"
    );

    let all_text = lines[3]["result"]["content"][0]["text"]
        .as_str()
        .expect("invariant: all call returns text");
    assert!(
        !all_text.contains("recent") && all_text.matches("open").count() == 21,
        "all=true reaches the full decision history:\n{all_text}"
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

    let frames = run_mcp_requests(
        repo,
        &[concat!(
            "[",
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\",\"params\":{}},",
            "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\",\"params\":{}},",
            "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"maestro_status\",\"arguments\":{}}}",
            "]"
        )],
    );
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

    let output = run_mcp_bytes(
        repo,
        &mcp_frames(&[r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#]),
    );
    assert!(
        String::from_utf8(output.stdout.clone())
            .expect("invariant: MCP output should be UTF-8")
            .starts_with("Content-Length: ")
    );
    let frames = parse_mcp_frames(&output.stdout);
    assert_eq!(frames[0]["id"], 1);
}

#[test]
fn mcp_frame_parser_uses_content_length_as_bytes() {
    let frames = parse_mcp_frames(&mcp_frames(&[r#"{"message":"déjà vu"}"#]));

    assert_eq!(frames[0]["message"], "déjà vu");
}

#[test]
fn mcp_serve_rejects_oversized_content_length_before_body_allocation() {
    let temp = setup_repo("maestro-mcp-oversized-frame");
    let repo = temp.path();
    let oversized = format!("Content-Length: {}\r\n\r\n", 1024 * 1024 + 1);

    let output = run_mcp_bytes_raw(repo, oversized.as_bytes());

    assert!(!output.status.success(), "oversized MCP frame should fail");
    assert!(
        stderr(&output).contains("MCP frame exceeds maximum size"),
        "stderr should explain the frame limit:\n{}",
        stderr(&output)
    );
}

#[test]
fn mcp_serve_accepts_newline_delimited_json_rpc() {
    let temp = setup_repo("maestro-mcp-line-json");
    let repo = temp.path();

    let output = run_mcp_bytes(
        repo,
        b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\",\"params\":{}}\n",
    );
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

    let output = run_mcp_bytes(
        repo,
        b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"list\",\"params\":{}}\n",
    );
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

    let lines = run_mcp_requests(
        repo,
        &[
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{}}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"missing_tool","arguments":{}}}"#,
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"maestro_task_complete","arguments":{"id":"task-001","summary":"done","claims":["one",2]}}}"#,
            r#"{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}"#,
        ],
    );
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
            .contains("claims[1] must be a string")
    );
}

#[test]
fn harness_show_preserves_legacy_minimal_backlog_items() {
    let temp = setup_repo("maestro-improve-legacy");
    let repo = temp.path();
    seed_idea_card(
        repo,
        "hb-legacy",
        "Add legacy coverage",
        "proposed",
        "title: Add legacy coverage\n",
    );

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
    // The rejected path must not leak any harness artifact into the symlink target:
    // no backlog file (gone under D7) and no idea card written through the symlink.
    assert!(!external.path().join("backlog.yaml").exists());
    assert_eq!(
        fs::read_dir(external.path())
            .expect("invariant: external dir should be readable")
            .count(),
        0,
        "the rejected symlinked harness path must not leak any artifact"
    );
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

/// Set up a repo whose only proposal is a `missing_verification` note. The note
/// fires because the task's embedded verification records `cargo clippy`, absent
/// from the empty harness verify list; adding `cargo clippy` to verify later
/// silences it. Detection mints the note an opaque `card-<hash>` id (D7 retired
/// the sequential `hb-NNN` mint), so the id is returned for the caller to drive
/// `harness apply/unapply/measure <id>` against.
fn setup_missing_verification_note(prefix: &str) -> (TestTempDir, String) {
    let temp = setup_repo(prefix);
    let repo = temp.path();
    let task = create_one_task(repo, "Reusable verification");
    write_harness_verify(repo, &[]);
    write_embedded_failed_verification(repo, &task, "900", "cargo clippy");
    let list = run_success(repo, &["harness", "list"]);
    assert!(list.contains("missing_verification"));
    let note = sole_idea_id(repo);
    (temp, note)
}

#[test]
fn harness_apply_spawns_task_and_rejects_reaccept() {
    let (temp, note) = setup_missing_verification_note("maestro-harness-apply-spawn");
    let repo = temp.path();

    let apply = run_success(repo, &["harness", "apply", &note]);
    assert!(apply.contains(&format!("accepted {note}")));
    let spawned = spawned_task_id(&apply);

    // The spawned task is a real draft task.
    let show_task = run_success(repo, &["task", "show", &spawned]);
    assert!(show_task.contains("Reusable verification"));

    // Re-accepting is rejected; the task is already linked.
    let reapply = maestro(repo, &["harness", "apply", &note]);
    assert!(!reapply.status.success());
    assert!(stderr(&reapply).contains("already accepted"));
}

#[test]
fn harness_apply_refuses_dismissed_items_without_mutation() {
    let (temp, note) = setup_missing_verification_note("maestro-harness-apply-dismissed");
    let repo = temp.path();

    let dismiss = run_success(repo, &["harness", "dismiss", &note, "--reason", "noise"]);
    assert!(dismiss.contains(&format!("dismissed {note}")), "{dismiss}");

    let before = idea_record(repo, &note);
    let apply = maestro(repo, &["harness", "apply", &note]);
    assert!(!apply.status.success());
    let err = stderr(&apply);
    assert!(err.contains("already dismissed"), "{err}");
    assert_eq!(idea_record(repo, &note), before);
}

#[test]
fn harness_dismiss_refuses_accepted_and_measured_items_without_mutation() {
    let (accepted_temp, accepted_note) =
        setup_missing_verification_note("maestro-harness-dismiss-accepted");
    let accepted_repo = accepted_temp.path();

    let apply = run_success(accepted_repo, &["harness", "apply", &accepted_note]);
    assert!(
        apply.contains(&format!("accepted {accepted_note}")),
        "{apply}"
    );
    let before_accepted = idea_record(accepted_repo, &accepted_note);
    let dismiss_accepted = maestro(
        accepted_repo,
        &["harness", "dismiss", &accepted_note, "--reason", "skip it"],
    );
    assert!(!dismiss_accepted.status.success());
    let err = stderr(&dismiss_accepted);
    assert!(err.contains("is accepted"), "{err}");
    assert!(err.contains("harness unapply"), "{err}");
    assert_eq!(idea_record(accepted_repo, &accepted_note), before_accepted);

    let (measured_temp, measured_note) =
        setup_missing_verification_note("maestro-harness-dismiss-measured");
    let measured_repo = measured_temp.path();
    let apply = run_success(measured_repo, &["harness", "apply", &measured_note]);
    let spawned = spawned_task_id(&apply);
    mark_verified(measured_repo, &spawned, "general", "0", "100");
    write_harness_verify(measured_repo, &["cargo clippy"]);
    let measure = run_success(measured_repo, &["harness", "measure", &measured_note]);
    assert!(
        measure.contains(&format!("{measured_note} is now measured")),
        "{measure}"
    );

    let before_measured = idea_record(measured_repo, &measured_note);
    let dismiss_measured = maestro(
        measured_repo,
        &["harness", "dismiss", &measured_note, "--reason", "skip it"],
    );
    assert!(!dismiss_measured.status.success());
    let err = stderr(&dismiss_measured);
    assert!(err.contains("already measured"), "{err}");
    assert_eq!(idea_record(measured_repo, &measured_note), before_measured);
}

#[test]
fn harness_apply_check_flags_replace_the_preset_on_spawned_task() {
    let (temp, note) = setup_missing_verification_note("maestro-harness-apply-custom-checks");
    let repo = temp.path();

    let apply = run_success(
        repo,
        &[
            "harness",
            "apply",
            &note,
            "--check",
            "custom evidence renders",
            "--check",
            "dashboard remains usable",
        ],
    );
    assert!(apply.contains(&format!("accepted {note}")), "{apply}");
    let spawned = spawned_task_id(&apply);
    assert!(
        apply.contains("checks: 2 authored (preset replaced)"),
        "{apply}"
    );
    assert!(!apply.contains("check preset:"), "{apply}");
    assert_eq!(
        task_checks(repo, &spawned),
        vec!["custom evidence renders", "dashboard remains usable"]
    );
}

#[test]
fn harness_apply_rolls_back_spawned_task_when_the_store_save_loses_a_race() {
    let (temp, note) = setup_missing_verification_note("maestro-harness-apply-contended");
    let repo = temp.path();

    // Simulate another Maestro process holding the backlog's write marker: a
    // fresh (non-stale) reservation dir beside `ideas.yaml` makes the guarded
    // save in `apply` fail *after* the task is spawned, exercising the
    // rollback-on-save-failure path. The container layout collapsed the backlog
    // into `ideas.yaml` entries, so the contended artifact is that one file.
    let lock_dir = repo.join(".maestro/cards/.ideas.yaml.write-lock");
    fs::create_dir(&lock_dir).expect("invariant: write-lock marker should be creatable");

    let apply = maestro(repo, &["harness", "apply", &note]);
    assert!(
        !apply.status.success(),
        "apply should fail while the store is contended"
    );
    assert!(
        stderr(&apply).contains("is being written by another Maestro process; re-run the command"),
        "the actionable concurrency error must surface, got:\n{}",
        stderr(&apply)
    );

    // Clearing the contention and retrying spawns the task cleanly: proof the
    // first attempt rolled the task back and left the proposal proposed (else
    // re-accept would be rejected).
    fs::remove_dir_all(&lock_dir).expect("invariant: write-lock marker should be removable");
    let retry = run_success(repo, &["harness", "apply", &note]);
    assert!(
        retry.contains("spawned "),
        "retry after clearing contention should spawn a task cleanly: {retry}"
    );
}

#[test]
fn harness_unapply_reverts_accepted_item_and_abandons_spawned_task() {
    let (temp, note) = setup_missing_verification_note("maestro-harness-unapply");
    let repo = temp.path();

    let apply = run_success(repo, &["harness", "apply", &note]);
    let spawned = spawned_task_id(&apply);

    let unapply = run_success(
        repo,
        &[
            "harness",
            "unapply",
            &note,
            "--reason",
            "applied before ruling",
        ],
    );
    assert!(
        unapply.contains(&format!("{note} -> proposed (seen unchanged: 1x/1s)")),
        "{unapply}"
    );
    assert!(
        unapply.contains(&format!("{spawned} -> abandoned (link cleared)")),
        "{unapply}"
    );
    assert!(
        unapply.contains("history: accepted -> unapplied"),
        "{unapply}"
    );
    assert_eq!(task_state(repo, &spawned), "abandoned");

    let show = run_success(repo, &["harness", "show", &note]);
    assert!(show.contains("status: proposed"), "{show}");
    assert!(show.contains("seen: 1x/1s"), "{show}");
    assert!(!show.contains("spawned_task:"), "{show}");
    assert!(show.contains(&format!("- accepted ({spawned})")), "{show}");
    assert!(show.contains(&format!("- unapplied ({spawned})")), "{show}");
    assert!(show.contains("\"applied before ruling\""), "{show}");
}

#[test]
fn harness_unapply_clears_a_vanished_spawned_task_link() {
    // A spawned task that vanished from the live card store (its directory removed)
    // unapplies cleanly: the link is cleared and the note returns to proposed.
    //
    // The companion "archived" arm of this test is gone by design under the card
    // model: E4 retired per-task archive, and a harness-spawned task is unparented
    // (`feature: None`), so it can never ride a feature-cascade archive into
    // `archive/cards/`. The `UnappliedTask::Archived` classification is therefore
    // unreachable here -- a vanished link is always `Missing`. (The harness archive
    // fallback in `propose.rs` still probes the dead legacy `archive/tasks` path, a
    // latent cutover bug, but it is moot because no verb can archive an unparented
    // task in the first place.)
    let (missing, missing_note) =
        setup_missing_verification_note("maestro-harness-unapply-missing");
    let missing_repo = missing.path();
    let apply = run_success(missing_repo, &["harness", "apply", &missing_note]);
    let missing_task = spawned_task_id(&apply);
    fs::remove_dir_all(task_dir(missing_repo, &missing_task))
        .expect("invariant: spawned task dir should be removable");
    let unapply = run_success(missing_repo, &["harness", "unapply", &missing_note]);
    assert!(
        unapply.contains(&format!("{missing_task} is missing (link cleared)")),
        "{unapply}"
    );
    let show = run_success(missing_repo, &["harness", "show", &missing_note]);
    assert!(show.contains("status: proposed"), "{show}");
    assert!(!show.contains("spawned_task:"), "{show}");
    assert!(
        show.contains(&format!("linked task {missing_task} is missing")),
        "{show}"
    );
}

#[test]
fn harness_unapply_surfaces_an_unreadable_spawned_task() {
    // An unreadable live record is a read failure, not absence: unapply must
    // surface it and keep the link, never clear it as "missing" over a task
    // that still exists.
    let (temp, note) = setup_missing_verification_note("maestro-harness-unapply-corrupt");
    let repo = temp.path();
    let apply = run_success(repo, &["harness", "apply", &note]);
    let spawned = spawned_task_id(&apply);

    let record_path = card_record_path(repo, &spawned);
    let original = fs::read_to_string(&record_path)
        .expect("invariant: spawned task record should be readable");
    fs::write(&record_path, "state: [unclosed")
        .expect("invariant: spawned task record should be writable");

    let unapply = maestro(repo, &["harness", "unapply", &note]);
    assert!(
        !unapply.status.success(),
        "unapply must not treat an unreadable task as missing"
    );
    assert!(
        stderr(&unapply).contains("failed to parse"),
        "got:\n{}",
        stderr(&unapply)
    );

    // Repairing the record makes the same unapply converge with the link intact.
    fs::write(&record_path, original).expect("invariant: record should be repairable");
    let unapply = run_success(repo, &["harness", "unapply", &note]);
    assert!(
        unapply.contains(&format!("{spawned} -> abandoned (link cleared)")),
        "{unapply}"
    );
}

#[test]
fn harness_unapply_requires_an_accepted_item() {
    let (temp, note) = setup_missing_verification_note("maestro-harness-unapply-not-accepted");
    let repo = temp.path();

    // The note starts proposed; unapplying before applying must be a clean error,
    // not a no-op that touches anything.
    let unapply = maestro(repo, &["harness", "unapply", &note]);
    assert!(
        !unapply.status.success(),
        "unapply of a non-accepted item should fail"
    );
    assert!(
        stderr(&unapply).contains(&format!(
            "is not accepted; run `maestro harness apply {note}` before unapplying"
        )),
        "got:\n{}",
        stderr(&unapply)
    );
}

#[test]
fn harness_unapply_refuses_a_non_live_linked_task() {
    let (temp, note) = setup_missing_verification_note("maestro-harness-unapply-inprogress");
    let repo = temp.path();

    let apply = run_success(repo, &["harness", "apply", &note]);
    let spawned = spawned_task_id(&apply);
    // Drive the spawned task past the live states: unapply must refuse rather than
    // silently abandon in-flight work.
    run_success(repo, &["task", "claim", &spawned]);
    assert_eq!(task_state(repo, &spawned), "in_progress");

    let unapply = maestro(repo, &["harness", "unapply", &note]);
    assert!(
        !unapply.status.success(),
        "unapply must refuse a non-live linked task"
    );
    assert!(
        stderr(&unapply).contains(&format!("linked task {spawned} is in_progress"))
            && stderr(&unapply).contains("use `maestro harness measure` or close the task"),
        "got:\n{}",
        stderr(&unapply)
    );
    // The item stays accepted and the task untouched -- no partial change.
    assert_eq!(task_state(repo, &spawned), "in_progress");
    let show = run_success(repo, &["harness", "show", &note]);
    assert!(show.contains("status: accepted"), "{show}");
}

#[test]
fn harness_unapply_leaves_the_task_recoverable_when_the_store_save_loses_a_race() {
    let (temp, note) = setup_missing_verification_note("maestro-harness-unapply-contended");
    let repo = temp.path();

    let apply = run_success(repo, &["harness", "apply", &note]);
    let spawned = spawned_task_id(&apply);
    assert_eq!(task_state(repo, &spawned), "ready");

    // Hold the backlog's write marker so the guarded save in unapply fails.
    // Because unapply abandons the task only *after* the save commits, the failed
    // save must leave the spawned task live -- otherwise the item would be wedged:
    // accepted on disk but pointing at an abandoned task that re-running unapply
    // can't clear. The container layout made `ideas.yaml` the contended artifact.
    let lock_dir = repo.join(".maestro/cards/.ideas.yaml.write-lock");
    fs::create_dir(&lock_dir).expect("invariant: write-lock marker should be creatable");
    let unapply = maestro(repo, &["harness", "unapply", &note]);
    assert!(
        !unapply.status.success(),
        "unapply should fail while the store is contended"
    );
    assert!(
        stderr(&unapply)
            .contains("is being written by another Maestro process; re-run the command"),
        "got:\n{}",
        stderr(&unapply)
    );
    assert_eq!(
        task_state(repo, &spawned),
        "ready",
        "a lost save race must leave the linked task live, not stranded as abandoned"
    );

    // Clearing the contention and retrying completes cleanly: proof the item was
    // never wedged.
    fs::remove_dir_all(&lock_dir).expect("invariant: write-lock marker should be removable");
    let retry = run_success(repo, &["harness", "unapply", &note]);
    assert!(
        retry.contains(&format!("{spawned} -> abandoned (link cleared)")),
        "retry after clearing contention should abandon the task cleanly: {retry}"
    );
    assert_eq!(task_state(repo, &spawned), "abandoned");
}

#[test]
fn harness_measure_closes_silent_state_note() {
    let (temp, note) = setup_missing_verification_note("maestro-harness-measure-silent");
    let repo = temp.path();

    let apply = run_success(repo, &["harness", "apply", &note]);
    let spawned = spawned_task_id(&apply);
    // apply now presets a check and accepts the standalone task, so it is claimable.
    assert!(apply.contains("check preset:"), "{apply}");
    assert!(
        apply.contains(&format!("next: maestro task claim {spawned}")),
        "{apply}"
    );

    // The linked task is verified and the friction is gone (command now in verify).
    mark_verified(repo, &spawned, "general", "0", "100");
    write_harness_verify(repo, &["cargo clippy"]);

    // D7: an accepted state note whose detector is silent is flagged ready to measure.
    let ready = run_success(repo, &["harness", "list"]);
    assert!(ready.contains("ready to measure"));

    let measure = run_success(repo, &["harness", "measure", &note]);
    assert!(
        measure.contains(&format!("{note} is now measured")),
        "{measure}"
    );
    // A clean close (detector silent) carries no friction warning.
    assert!(!measure.contains("friction is still detected"), "{measure}");

    let show = run_success(repo, &["harness", "show", &note]);
    assert!(show.contains("status: measured"));
    assert!(show.contains("history:"));
    assert!(show.contains("- measured"));

    // A measured note is hidden by default and only shown under --all (D4); the
    // default view says how many it hid so they don't seem to have vanished (UX-3).
    let list = run_success(repo, &["harness", "list"]);
    assert!(!list.contains(&note));
    assert!(list.contains("terminal proposal(s) hidden"), "{list}");
    let all = run_success(repo, &["harness", "list", "--all"]);
    assert!(all.contains(&note));
}

#[test]
fn harness_measure_resolves_a_verified_linked_task_left_as_closed_history() {
    // S2-7 in card mode: E4 retired per-task archive (a finished task stays as
    // closed history in the card store, never moved to an archive sibling), so a
    // verified spawned task is no longer archived as terminal cleanup -- it simply
    // stays `verified` in the live tree. The measure gate and the "ready to
    // measure" hint must resolve it there, exactly as `task show` / `query proof`
    // do, with no `--force` escape hatch.
    let (temp, note) = setup_missing_verification_note("maestro-harness-measure-closed-history");
    let repo = temp.path();

    let apply = run_success(repo, &["harness", "apply", &note]);
    let spawned = spawned_task_id(&apply);

    mark_verified(repo, &spawned, "general", "0", "100");
    write_harness_verify(repo, &["cargo clippy"]);

    // The hint flags it ready to measure: hint and gate agree on the verified task.
    let ready = run_success(repo, &["harness", "list"]);
    assert!(ready.contains("ready to measure"), "{ready}");

    // measure succeeds against the verified linked task, not the --force hatch.
    let measure = run_success(repo, &["harness", "measure", &note]);
    assert!(
        measure.contains(&format!("{note} is now measured")),
        "{measure}"
    );

    let show = run_success(repo, &["harness", "show", &note]);
    assert!(show.contains("status: measured"), "{show}");
}

#[test]
fn harness_regression_reopens_measured_state_note_and_clears_link() {
    let (temp, note) = setup_missing_verification_note("maestro-harness-regress");
    let repo = temp.path();

    // Accept, verify the task, silence the friction, and measure to `measured`.
    let apply = run_success(repo, &["harness", "apply", &note]);
    let spawned = spawned_task_id(&apply);
    mark_verified(repo, &spawned, "general", "0", "100");
    write_harness_verify(repo, &["cargo clippy"]);
    let measure = run_success(repo, &["harness", "measure", &note]);
    assert!(measure.contains(&format!("{note} is now measured")));

    // Friction returns (command dropped from verify again): re-deriving reopens the
    // measured state note (D6) and pulls it back into the active set.
    write_harness_verify(repo, &[]);
    let list = run_success(repo, &["harness", "list"]);
    assert!(list.contains(&note));

    // The note is `proposed` again with a `regressed` record, and the old link is
    // cleared so the next accept spawns a fresh task (impl-default (c)).
    let show = run_success(repo, &["harness", "show", &note]);
    assert!(show.contains("status: proposed"));
    assert!(show.contains("- regressed"));
    assert!(!show.contains("spawned_task:"));
}

#[test]
fn harness_measure_reverts_ineffective_state_note_and_relinks_on_reapply() {
    let (temp, note) = setup_missing_verification_note("maestro-harness-measure-ineffective");
    let repo = temp.path();

    let apply = run_success(repo, &["harness", "apply", &note]);
    let spawned = spawned_task_id(&apply);
    mark_verified(repo, &spawned, "general", "0", "100");

    // Friction persists (cargo clippy still absent from verify): the note reverts.
    let measure = run_success(repo, &["harness", "measure", &note]);
    assert!(
        measure.contains(&format!("{note} reverted to proposed")),
        "{measure}"
    );
    assert!(measure.contains("ineffective"), "{measure}");

    let show = run_success(repo, &["harness", "show", &note]);
    assert!(show.contains("status: proposed"));
    assert!(show.contains("- ineffective"));
    assert!(!show.contains("spawned_task:"));

    // The cleared link means a re-accept spawns a fresh task, never the closed one.
    let reapply = run_success(repo, &["harness", "apply", &note]);
    let respawned = spawned_task_id(&reapply);
    assert_ne!(
        respawned, spawned,
        "re-accept must spawn a fresh task, not the closed one"
    );
}

#[test]
fn harness_measure_requires_verified_task_unless_forced() {
    let (temp, note) = setup_missing_verification_note("maestro-harness-measure-gate");
    let repo = temp.path();

    let apply = run_success(repo, &["harness", "apply", &note]);
    assert!(apply.contains("spawned "), "{apply}");

    // The linked task is still a draft: measure is gated.
    let gated = maestro(repo, &["harness", "measure", &note]);
    assert!(!gated.status.success());
    assert!(stderr(&gated).contains("not verified"));

    // --force bypasses the gate, but not the verdict: the friction persists
    // (cargo clippy still absent from verify), so the note reverts to proposed.
    let forced = run_success(repo, &["harness", "measure", &note, "--force"]);
    assert!(
        forced.contains(&format!("{note} reverted to proposed")),
        "{forced}"
    );
}

#[test]
fn harness_measure_closes_behavioral_note_without_silence() {
    let temp = setup_repo("maestro-harness-measure-behavioral");
    let repo = temp.path();
    let first = create_one_task(repo, "First blocked task");
    let second = create_one_task(repo, "Second blocked task");
    write_harness_verify(repo, &[]);
    for id in [&first, &second] {
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

    // The aggregate blocker is the sole detected proposal; D7 mints it an opaque id.
    let note = sole_backlog_id(repo);
    let apply = run_success(repo, &["harness", "apply", &note]);
    assert!(apply.contains(&format!("accepted {note}")));
    let spawned = spawned_task_id(&apply);
    mark_verified(repo, &spawned, "general", "0", "100");

    // The blocker still emits, but behavioral notes close on the deliberate,
    // verified-task measure with no silence check (D1). The close is honest about
    // the still-live friction (T9).
    let measure = run_success(repo, &["harness", "measure", &note]);
    assert!(
        measure.contains(&format!("{note} is now measured")),
        "{measure}"
    );
    assert!(measure.contains("friction is still detected"), "{measure}");
}

#[test]
fn harness_apply_on_a_measured_behavioral_item_does_not_point_at_the_dead_end_rederive() {
    let temp = setup_repo("maestro-harness-apply-measured-behavioral");
    let repo = temp.path();
    // A measured behavioral note (recurring_blocker is not a state detector, so
    // re-detection never reopens it). A fresh repo re-derives no recurring
    // blocker, so this item survives detect_and_merge unchanged. D7 collapsed the
    // backlog into idea cards, so the item is seeded as its persisted card form.
    seed_idea_card(
        repo,
        "hb-001",
        "Recurring blocker waiting-on-api across tasks",
        "measured",
        concat!(
            "fingerprint: recurring_blocker:waiting-on-api\n",
            "source: aggregate\n",
            "type: recurring_blocker\n",
            "title: Recurring blocker waiting-on-api across tasks\n",
            "priority: medium\n",
            "status: measured\n",
        ),
    );

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
    // so this arm is only reachable when the friction is already gone.) D7 collapsed
    // the backlog into idea cards, so the item is seeded as its persisted card form.
    seed_idea_card(
        repo,
        "hb-001",
        "Missing verification for cargo clippy",
        "measured",
        concat!(
            "fingerprint: missing_verification:cargo clippy\n",
            "source: reports\n",
            "type: missing_verification\n",
            "title: Missing verification for cargo clippy\n",
            "priority: medium\n",
            "status: measured\n",
        ),
    );

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
    let (temp, note) = setup_missing_verification_note("maestro-harness-ready-gate");
    let repo = temp.path();

    let apply = run_success(repo, &["harness", "apply", &note]);
    let spawned = spawned_task_id(&apply);

    // Silence the detector so the state-note is otherwise ready to measure, but
    // leave the linked task an unverified draft.
    write_harness_verify(repo, &["cargo clippy"]);

    // The no-force measure gate refuses an unverified task, so the hint must not
    // promise it (R12): a silent detector alone is not "ready to measure".
    let not_ready = run_success(repo, &["harness", "list"]);
    assert!(not_ready.contains(&note), "{not_ready}");
    assert!(!not_ready.contains("ready to measure"), "{not_ready}");

    // Once the linked task is verified, the gate would pass and the hint appears.
    mark_verified(repo, &spawned, "general", "0", "100");
    let ready = run_success(repo, &["harness", "list"]);
    assert!(ready.contains("ready to measure"), "{ready}");
}

#[test]
fn harness_measure_names_force_when_the_linked_task_vanished() {
    let (temp, note) = setup_missing_verification_note("maestro-harness-measure-vanished");
    let repo = temp.path();

    let apply = run_success(repo, &["harness", "apply", &note]);
    let spawned = spawned_task_id(&apply);

    // The linked task is deleted out from under the note (archived or removed).
    fs::remove_dir_all(task_dir(repo, &spawned))
        .expect("invariant: spawned task dir should be removable");

    // The no-force measure can no longer load the task; instead of leaking a bare
    // "not found", it names the --force escape hatch (R23).
    let gated = maestro(repo, &["harness", "measure", &note]);
    assert!(!gated.status.success());
    let err = stderr(&gated);
    assert!(err.contains("could not be loaded"), "{err}");
    assert!(err.contains("use --force to measure anyway"), "{err}");
}

/// Collapse aligned-table padding (runs of 2+ spaces) back to tabs so cell
/// assertions stay width-independent.
fn untabify(output: &str) -> String {
    output
        .lines()
        .map(|line| {
            line.split("  ")
                .map(str::trim)
                .filter(|cell| !cell.is_empty())
                .collect::<Vec<_>>()
                .join("\t")
        })
        .collect::<Vec<_>>()
        .join("\n")
}
