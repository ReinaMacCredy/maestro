mod support;

use std::fs;
use std::path::Path;
use std::process::Command;

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
        stdout(output),
        stderr(output)
    );
}

fn assert_failure(output: &std::process::Output, args: &[&str]) {
    assert!(
        !output.status.success(),
        "maestro {:?} unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        args,
        stdout(output),
        stderr(output)
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
    let init = maestro(temp.path(), &["init", "--yes"]);
    assert_success(&init, &["init", "--yes"]);
    temp
}

fn run(repo: &Path, args: &[&str]) -> String {
    let output = maestro(repo, args);
    assert_success(&output, args);
    stdout(&output)
}

fn task_yaml(repo: &Path, id: &str) -> YamlValue {
    let prefix = format!("{id}-");
    let tasks_dir = repo.join(".maestro/tasks");
    for entry in fs::read_dir(tasks_dir).expect("invariant: tasks dir should be readable") {
        let entry = entry.expect("invariant: task entry should be readable");
        let name = entry
            .file_name()
            .to_str()
            .expect("invariant: task dir should be UTF-8")
            .to_string();
        if name.starts_with(&prefix) {
            let raw = fs::read_to_string(entry.path().join("task.yaml"))
                .expect("invariant: task.yaml should be readable");
            return serde_yaml::from_str(&raw).expect("invariant: task.yaml should parse");
        }
    }
    panic!("invariant: task directory should exist for {id}");
}

fn write_baseline(repo: &Path, feature_id: &str) {
    let dir = repo.join(".maestro/features").join(feature_id);
    fs::write(
        dir.join("baseline.md"),
        "---\namend_log_position: 0\n---\n\n### QA Baseline Contract\n\n- Scenario Matrix:\n  - [bl-001] csv export round-trips\n",
    )
    .expect("invariant: baseline.md should be writable");
}

fn write_disabled_harness(repo: &Path) {
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

fn write_correction_session(repo: &Path, session: &str) {
    let run_dir = repo.join(".maestro/runs").join(session);
    fs::create_dir_all(&run_dir).expect("invariant: run dir should be creatable");
    fs::write(
        run_dir.join("events.jsonl"),
        concat!(
            "{\"event_type\":\"UserPromptSubmit\",\"prompt\":\"no, use rg\"}\n",
            "{\"event_type\":\"UserPromptSubmit\",\"prompt\":\"wait that's wrong\"}\n",
            "{\"event_type\":\"UserPromptSubmit\",\"prompt\":\"actually verify it\"}\n"
        ),
    )
    .expect("invariant: events fixture should be writable");
}

fn backlog_yaml(repo: &Path) -> YamlValue {
    let raw = fs::read_to_string(repo.join(".maestro/harness/backlog.yaml"))
        .expect("invariant: backlog should be readable");
    serde_yaml::from_str(&raw).expect("invariant: backlog should parse")
}

#[test]
fn status_before_init_is_friendly_and_read_only() {
    let temp = TestTempDir::new("maestro-status-preinit");
    fs::create_dir(temp.path().join(".git")).expect("invariant: .git marker should be creatable");

    let status = maestro(temp.path(), &["status"]);

    assert_success(&status, &["status"]);
    let out = stdout(&status);
    assert!(out.contains("maestro status: not initialized"));
    assert!(out.contains("- preview setup: maestro init --dry-run"));
    assert!(out.contains("- initialize: maestro init --yes"));
    assert!(!temp.path().join(".maestro").exists());
}

#[test]
fn task_next_no_action_prints_summary_and_exits_nonzero() {
    let temp = setup_repo("maestro-task-next-empty");
    let repo = temp.path();

    let next = maestro(repo, &["task", "next"]);

    assert_failure(&next, &["task", "next"]);
    assert!(stdout(&next).contains("no actionable tasks"));
    assert!(stderr(&next).contains("no actionable tasks"));
}

#[test]
fn status_and_task_next_choose_current_task_before_ready_queue() {
    let temp = setup_repo("maestro-status-current");
    let repo = temp.path();
    run(
        repo,
        &["task", "create", "Ready task", "--check", "ready check"],
    );
    run(repo, &["task", "explore", "task-001"]);
    run(repo, &["task", "accept", "task-001"]);
    run(repo, &["task", "create", "Draft task"]);

    let next = maestro_with_env(
        repo,
        &["task", "next"],
        &[("MAESTRO_CURRENT_TASK", "task-002")],
    );

    assert_success(&next, &["task", "next"]);
    let out = stdout(&next);
    assert!(out.contains("template: maestro task set task-002 --check"));
    assert!(out.contains("task: task-002"));

    let status = maestro_with_env(
        repo,
        &["status", "--json"],
        &[("MAESTRO_CURRENT_TASK", "task-002")],
    );
    assert_success(&status, &["status", "--json"]);
    let json: JsonValue =
        serde_json::from_str(&stdout(&status)).expect("invariant: status JSON should parse");
    assert_eq!(json["schema"], "maestro.status.v1");
    assert_eq!(json["current_task"], "task-002");
    assert_eq!(json["next_action"]["kind"], "add_task_check");
    assert_eq!(json["next_action"]["requires_input"], true);
    assert_eq!(
        json["next_action"]["command"]["display"],
        "maestro task set task-002 --check \"<observable result>\""
    );
    assert!(json["next_action"]["command"]["argv"].is_null());
    assert_eq!(
        json["next_action"]["command"]["argv_template"],
        serde_json::json!([
            "maestro",
            "task",
            "set",
            "task-002",
            "--check",
            "<observable result>"
        ])
    );
    assert_eq!(
        json["next_action"]["command"]["requires_input"][0]["name"],
        "observable_result"
    );
}

#[test]
fn disabled_escalation_keeps_status_and_task_next_output_unchanged() {
    let temp = setup_repo("maestro-escalation-disabled-output");
    let repo = temp.path();
    write_disabled_harness(repo);
    run(
        repo,
        &["task", "create", "Ready task", "--check", "ready check"],
    );
    run(repo, &["task", "explore", "task-001"]);
    run(repo, &["task", "accept", "task-001"]);

    let status_before = run(repo, &["status"]);
    let next_before = run(repo, &["task", "next"]);
    write_correction_session(repo, "session-a");
    write_correction_session(repo, "session-b");
    write_correction_session(repo, "session-c");

    assert_eq!(run(repo, &["status"]), status_before);
    assert_eq!(run(repo, &["task", "next"]), next_before);
}

#[test]
fn harness_friction_surfaces_in_status_task_next_list_and_complete() {
    let temp = setup_repo("maestro-harness-surfacing");
    let repo = temp.path();
    write_correction_session(repo, "session-a");
    write_correction_session(repo, "session-b");
    write_correction_session(repo, "session-c");
    run(
        repo,
        &["task", "create", "Ready task", "--check", "ready proof"],
    );
    run(repo, &["task", "explore", "task-001"]);
    run(repo, &["task", "accept", "task-001"]);

    let status = run(repo, &["status"]);
    assert!(status.contains("HARNESS FRICTION"), "{status}");
    assert!(
        status.contains("! friction hb-001 over threshold"),
        "{status}"
    );
    assert!(
        status.contains("apply: maestro harness apply hb-001"),
        "{status}"
    );

    let next = run(repo, &["task", "next"]);
    let friction_at = next
        .find("HARNESS FRICTION")
        .expect("invariant: task next should show friction");
    let normal_at = next
        .find("run: maestro task claim --next")
        .expect("invariant: task next should keep normal next action");
    assert!(friction_at < normal_at, "{next}");

    let list = run(repo, &["harness", "list"]);
    assert!(list.contains("ID\t!\tSTATUS\tTYPE\tSEEN\tTITLE"), "{list}");
    assert!(
        list.contains("hb-001\t!\tproposed\trecurring_intervention\t9x/3s"),
        "{list}"
    );

    run(repo, &["task", "claim", "task-001"]);
    let complete = run(
        repo,
        &[
            "task",
            "complete",
            "task-001",
            "--summary",
            "done",
            "--claim",
            "ready proof",
            "--proof",
            "ready proof",
        ],
    );
    assert!(
        complete.contains("verification passed for task-001"),
        "{complete}"
    );
    assert!(complete.contains("HARNESS FRICTION"), "{complete}");
}

#[test]
fn hot_verbs_skip_detect_until_evidence_stamp_changes() {
    let temp = setup_repo("maestro-harness-stamp-skip");
    let repo = temp.path();
    write_correction_session(repo, "session-a");
    write_correction_session(repo, "session-b");
    write_correction_session(repo, "session-c");

    let status = run(repo, &["status"]);
    assert!(status.contains("HARNESS FRICTION"), "{status}");
    for _ in 0..3 {
        run(repo, &["status"]);
    }
    let stamped = backlog_yaml(repo);
    assert_eq!(
        stamped["items"][0]["sessions_hit"]
            .as_sequence()
            .unwrap()
            .len(),
        3
    );

    let mut edited = stamped;
    edited["items"] = YamlValue::Sequence(Vec::new());
    fs::write(
        repo.join(".maestro/harness/backlog.yaml"),
        serde_yaml::to_string(&edited).expect("invariant: backlog should serialize"),
    )
    .expect("invariant: backlog should be writable");
    let skipped = run(repo, &["status"]);
    assert!(!skipped.contains("HARNESS FRICTION"), "{skipped}");

    run(repo, &["task", "create", "Task-dir stamp change"]);
    let refreshed = run(repo, &["status"]);
    assert!(refreshed.contains("HARNESS FRICTION"), "{refreshed}");
}

#[test]
fn current_task_infers_feature_context_without_feature_env() {
    let temp = setup_repo("maestro-status-current-feature");
    let repo = temp.path();
    run(repo, &["feature", "new", "CSV export"]);
    run(
        repo,
        &[
            "task",
            "create",
            "Implement CSV writer",
            "--feature",
            "csv-export",
        ],
    );

    let status = maestro_with_env(
        repo,
        &["status", "--json"],
        &[
            ("MAESTRO_CURRENT_TASK", "task-001"),
            ("MAESTRO_CURRENT_FEATURE", "wrong-feature"),
        ],
    );
    assert_success(&status, &["status", "--json"]);
    let json: JsonValue =
        serde_json::from_str(&stdout(&status)).expect("invariant: status JSON should parse");

    assert_eq!(json["current_task"], "task-001");
    assert_eq!(json["current_feature"], "csv-export");
    assert_eq!(json["next_action"]["feature_id"], "csv-export");
    assert_eq!(
        json["next_action"]["command"]["argv"],
        serde_json::json!(["maestro", "task", "explore", "task-001"])
    );

    let human = run(repo, &["status"]);
    assert!(human.contains("ACTIVE FEATURES"), "{human}");
    assert!(human.contains("csv-export"), "{human}");
    assert!(human.contains("maestro feature show csv-export"), "{human}");
}

#[test]
fn human_fallback_warnings_are_first_line_for_status_and_task_next() {
    let temp = setup_repo("maestro-status-warning-first");
    let repo = temp.path();
    run(
        repo,
        &["task", "create", "Ready task", "--check", "ready check"],
    );
    run(repo, &["task", "explore", "task-001"]);
    run(repo, &["task", "accept", "task-001"]);

    let status = maestro_with_env(repo, &["status"], &[("MAESTRO_CURRENT_TASK", "task-999")]);
    assert_success(&status, &["status"]);
    let status_out = stdout(&status);
    assert!(
        status_out.starts_with("warning: MAESTRO_CURRENT_TASK=task-999 was not found"),
        "{status_out}"
    );

    let next = maestro_with_env(
        repo,
        &["task", "next"],
        &[("MAESTRO_CURRENT_TASK", "task-999")],
    );
    assert_success(&next, &["task", "next"]);
    let next_out = stdout(&next);
    assert!(
        next_out.starts_with("warning: MAESTRO_CURRENT_TASK=task-999 was not found"),
        "{next_out}"
    );
    assert!(next_out.contains("run: maestro task claim --next"));
}

#[test]
fn ready_to_ship_status_json_and_task_next_broader_actions_are_structured() {
    let temp = setup_repo("maestro-ready-to-ship-json");
    let repo = temp.path();
    run(repo, &["feature", "new", "CSV export"]);
    run(
        repo,
        &[
            "feature",
            "set",
            "csv-export",
            "--acceptance",
            "CSV export round-trips",
            "--area",
            "export flow",
        ],
    );
    write_baseline(repo, "csv-export");
    run(repo, &["feature", "accept", "csv-export"]);
    run(repo, &["feature", "start", "csv-export"]);
    run(
        repo,
        &[
            "task",
            "create",
            "Implement CSV writer",
            "--feature",
            "csv-export",
        ],
    );
    run(repo, &["task", "explore", "task-001"]);
    run(repo, &["task", "accept", "task-001"]);
    run(repo, &["task", "claim", "task-001"]);
    let complete = run(
        repo,
        &[
            "task",
            "complete",
            "task-001",
            "--summary",
            "done",
            "--claim",
            "CSV export round-trips",
            "--proof",
            "CSV export round-trips",
        ],
    );
    assert!(
        complete.contains("template: maestro feature ship csv-export --outcome \"<outcome>\""),
        "{complete}"
    );

    let status = maestro(repo, &["status", "--json"]);
    assert_success(&status, &["status", "--json"]);
    let status_json: JsonValue =
        serde_json::from_str(&stdout(&status)).expect("invariant: status JSON should parse");
    let ready = &status_json["sections"]["ready_to_ship"][0];
    assert_eq!(ready["feature_id"], "csv-export");
    assert_eq!(ready["next_action"]["kind"], "feature_ship");
    assert!(ready["next_action"]["command"]["argv"].is_null());
    assert_eq!(
        ready["next_action"]["command"]["argv_template"],
        serde_json::json!([
            "maestro",
            "feature",
            "ship",
            "csv-export",
            "--outcome",
            "<outcome>"
        ])
    );
    assert_eq!(
        ready["next_action"]["command"]["requires_input"][0]["flag"],
        "--outcome"
    );

    let next = maestro(repo, &["task", "next", "--json"]);
    assert_failure(&next, &["task", "next", "--json"]);
    let next_json: JsonValue =
        serde_json::from_str(&stdout(&next)).expect("invariant: task next JSON should parse");
    assert!(next_json["next_action"].is_null());
    assert_eq!(
        next_json["broader_actions"][0]["kind"],
        "feature_ready_to_ship"
    );
    assert_eq!(next_json["broader_actions"][0]["feature_id"], "csv-export");
}

#[test]
fn manual_and_root_verify_pass_use_context_aware_handoff() {
    let temp = setup_repo("maestro-manual-verify-handoff");
    let repo = temp.path();
    run(
        repo,
        &[
            "task",
            "create",
            "Manual proof task",
            "--check",
            "manual proof passes",
        ],
    );
    run(repo, &["task", "explore", "task-001"]);
    run(repo, &["task", "accept", "task-001"]);
    run(repo, &["task", "claim", "task-001"]);
    let complete = maestro(
        repo,
        &[
            "task",
            "complete",
            "task-001",
            "--summary",
            "done",
            "--claim",
            "manual proof passes",
        ],
    );
    assert_failure(&complete, &["task", "complete", "task-001"]);
    run(
        repo,
        &[
            "event",
            "create",
            "--task-id",
            "task-001",
            "--claim",
            "manual proof passes",
        ],
    );

    let task_verify = run(repo, &["task", "verify", "task-001"]);
    assert!(task_verify.contains("verification passed for task-001"));
    assert!(task_verify.contains("task verified: task-001"));
    assert!(task_verify.contains("next: maestro status"));
    assert!(task_verify.contains("inspect: maestro task show task-001"));

    let root_verify = run(repo, &["verify", "task-001"]);
    assert!(root_verify.contains("verification passed for task-001"));
    assert!(root_verify.contains("task verified: task-001"));
    assert!(root_verify.contains("next: maestro status"));
}

#[test]
fn status_limits_large_task_rows_and_points_to_task_list() {
    let temp = setup_repo("maestro-status-row-limit");
    let repo = temp.path();

    for i in 0..6 {
        run(repo, &["task", "create", &format!("Draft task {i}")]);
    }

    let status = run(repo, &["status"]);

    assert!(status.contains("... 1 more active task(s); run maestro task list"));
}

#[test]
fn task_create_check_handoff_and_list_columns_are_actionable() {
    let temp = setup_repo("maestro-create-check-next");
    let repo = temp.path();

    let create = run(
        repo,
        &[
            "task",
            "create",
            "Add export",
            "--check",
            "cargo test passes",
        ],
    );

    assert!(create.contains("created task-001 (draft)"));
    assert!(create.contains("verify+ locked:"));
    assert!(create.contains("next: maestro task explore task-001"));

    let list = run(repo, &["task", "list"]);
    assert!(list.contains("NEXT"));
    assert!(list.contains("INSPECT"));
    assert!(list.contains("run: explore"));
    assert!(list.contains("maestro task show task-001"));
}

#[test]
fn task_list_next_column_uses_verify_contract_state_not_only_lifecycle_state() {
    let temp = setup_repo("maestro-list-missing-check");
    let repo = temp.path();

    run(repo, &["task", "create", "Update README"]);

    let list = run(repo, &["task", "list"]);
    assert!(list.contains("template: add_check"), "{list}");
    assert!(list.contains("maestro task show task-001"), "{list}");
    assert!(
        !list.contains("task-001\tdraft\trun: explore"),
        "standalone draft without checks must not point at explore first: {list}"
    );
}

#[test]
fn complete_with_proof_records_proof_and_auto_verifies() {
    let temp = setup_repo("maestro-complete-proof-auto");
    let repo = temp.path();

    run(
        repo,
        &[
            "task",
            "create",
            "Add export",
            "--check",
            "cargo test passes",
        ],
    );
    run(repo, &["task", "explore", "task-001"]);
    run(repo, &["task", "accept", "task-001"]);
    run(repo, &["task", "claim", "task-001"]);
    let complete = run(
        repo,
        &[
            "task",
            "complete",
            "task-001",
            "--summary",
            "done",
            "--claim",
            "cargo test passes",
            "--proof",
            "cargo test passes",
        ],
    );

    assert!(complete.contains("auto: recorded task_proof event"));
    assert!(complete.contains("auto: maestro task verify task-001"));
    assert!(complete.contains("verification passed for task-001"));
    assert_eq!(
        task_yaml(repo, "task-001")["state"],
        YamlValue::String("verified".to_string())
    );
}

#[test]
fn feature_linked_complete_handoff_uses_existing_feature_command() {
    let temp = setup_repo("maestro-feature-linked-complete-next");
    let repo = temp.path();

    run(repo, &["feature", "new", "CSV export"]);
    run(
        repo,
        &[
            "task",
            "create",
            "Implement CSV writer",
            "--feature",
            "csv-export",
        ],
    );
    run(repo, &["task", "explore", "task-001"]);
    run(repo, &["task", "accept", "task-001"]);
    run(repo, &["task", "claim", "task-001"]);
    let complete = run(
        repo,
        &[
            "task",
            "complete",
            "task-001",
            "--summary",
            "done",
            "--claim",
            "CSV writer works",
            "--proof",
            "CSV writer works",
        ],
    );

    assert!(complete.contains("feature: csv-export"), "{complete}");
    assert!(
        complete.contains("next: maestro feature show csv-export"),
        "{complete}"
    );
    assert!(
        !complete.contains("maestro feature status"),
        "feature status is not a command: {complete}"
    );
}

#[test]
fn feature_prepare_builds_sequenced_queue_and_claim_next_shows_chain() {
    let temp = setup_repo("maestro-feature-prepare-queue");
    let repo = temp.path();

    run(repo, &["feature", "new", "Serverless news backend"]);
    run(
        repo,
        &[
            "feature",
            "set",
            "serverless-news-backend",
            "--acceptance",
            "GET /articles returns records",
            "--area",
            "api",
        ],
    );
    write_baseline(repo, "serverless-news-backend");
    let accept = run(repo, &["feature", "accept", "serverless-news-backend"]);
    assert!(
        accept.contains("next: maestro feature prepare serverless-news-backend --draft"),
        "{accept}"
    );

    let draft = run(
        repo,
        &["feature", "prepare", "serverless-news-backend", "--draft"],
    );
    assert!(draft.contains("prepare-draft.md"), "{draft}");

    let plan = repo.join("PLAN-serverless-news.md");
    fs::write(
        &plan,
        concat!(
            "## Task T1: Implement protected read handlers\n",
            "check: GET /articles returns compact paginated records\n",
            "check: missing or invalid demo API key is rejected\n",
            "\n",
            "## Task T2: Implement operation handlers\n",
            "after: T1\n",
            "check: POST /collect and POST /retry satisfy the API contract\n",
            "\n",
            "## Task T3: Complete deploy gate\n",
            "after: T2\n",
            "check: VERIFY has expected vs observed evidence\n",
            "blocker: cloud deploy approval required\n",
        ),
    )
    .expect("invariant: prepare plan should be writable");

    let prepare = run(
        repo,
        &[
            "feature",
            "prepare",
            "serverless-news-backend",
            "--from",
            plan.to_str().expect("invariant: plan path should be UTF-8"),
        ],
    );
    assert!(prepare.contains("prepared 3 task(s)"), "{prepare}");
    assert!(
        prepare.contains("started serverless-news-backend -> in_progress"),
        "{prepare}"
    );
    assert!(prepare.contains("task-002 ready / blocked"), "{prepare}");
    assert!(
        prepare.contains("after dependency: T1 (task-001) verified"),
        "{prepare}"
    );
    assert!(
        prepare.contains("cloud deploy approval required"),
        "{prepare}"
    );
    assert!(
        prepare.contains("next: maestro task claim --next"),
        "{prepare}"
    );

    let task_002 = task_yaml(repo, "task-002");
    assert_eq!(task_002["state"], YamlValue::String("ready".to_string()));
    assert_eq!(
        task_002["blockers"][0]["reason"],
        YamlValue::String("after dependency: T1 (task-001) verified".to_string())
    );

    let claim = run(repo, &["task", "claim", "--next"]);
    assert!(claim.contains("claimed task-001 -> in_progress"), "{claim}");
    assert!(
        claim.contains("feature: serverless-news-backend"),
        "{claim}"
    );
    assert!(claim.contains("chain:"), "{claim}");
    assert!(
        claim.contains("task-001 current  Implement protected read handlers"),
        "{claim}"
    );
    assert!(
        claim.contains("task-002 blocked  Implement operation handlers"),
        "{claim}"
    );
    assert!(claim.contains("acceptance:"), "{claim}");
    assert!(
        claim.contains("- GET /articles returns compact paginated records"),
        "{claim}"
    );
    assert!(!claim.contains("feature title:"), "{claim}");

    let complete = run(
        repo,
        &[
            "task",
            "complete",
            "task-001",
            "--summary",
            "read handlers done",
            "--claim",
            "GET /articles returns compact paginated records",
            "--proof",
            "GET /articles returns compact paginated records",
        ],
    );
    assert!(
        complete.contains("verification passed for task-001"),
        "{complete}"
    );
    assert!(
        complete.contains("next: maestro task claim --next"),
        "{complete}"
    );

    let task_002_after = task_yaml(repo, "task-002");
    assert_ne!(
        task_002_after["blockers"][0]["resolved_at"],
        YamlValue::Null
    );

    let next_claim = run(repo, &["task", "claim", "--next"]);
    assert!(
        next_claim.contains("claimed task-002 -> in_progress"),
        "{next_claim}"
    );
    assert!(
        next_claim.contains("task-001 verified Implement protected read handlers"),
        "{next_claim}"
    );
    assert!(
        next_claim.contains("task-002 current  Implement operation handlers"),
        "{next_claim}"
    );
}

#[test]
fn feature_prepare_does_not_infer_blockers_and_keeps_all_blocked_feature_ready() {
    let temp = setup_repo("maestro-feature-prepare-blockers");
    let repo = temp.path();

    run(repo, &["feature", "new", "No inferred blockers"]);
    run(
        repo,
        &[
            "feature",
            "set",
            "no-inferred-blockers",
            "--acceptance",
            "dependency task exists",
            "--area",
            "setup",
        ],
    );
    write_baseline(repo, "no-inferred-blockers");
    run(repo, &["feature", "accept", "no-inferred-blockers"]);
    let vague_plan = repo.join("PLAN-no-infer.md");
    fs::write(
        &vague_plan,
        concat!(
            "## Task T1: Scaffold dependencies\n",
            "check: package manifest mentions dependency approval required\n",
        ),
    )
    .expect("invariant: vague plan should be writable");
    let vague_prepare = run(
        repo,
        &[
            "feature",
            "prepare",
            "no-inferred-blockers",
            "--from",
            vague_plan
                .to_str()
                .expect("invariant: plan path should be UTF-8"),
        ],
    );
    assert!(vague_prepare.contains("started no-inferred-blockers -> in_progress"));
    let vague_task = task_yaml(repo, "task-001");
    assert_eq!(vague_task["blockers"], YamlValue::Null);

    run(repo, &["feature", "new", "All blocked setup"]);
    run(
        repo,
        &[
            "feature",
            "set",
            "all-blocked-setup",
            "--acceptance",
            "blocked setup is visible",
            "--area",
            "setup",
        ],
    );
    write_baseline(repo, "all-blocked-setup");
    run(repo, &["feature", "accept", "all-blocked-setup"]);
    let blocked_plan = repo.join("PLAN-all-blocked.md");
    fs::write(
        &blocked_plan,
        concat!(
            "## Task T1: Scaffold approved dependencies\n",
            "check: package manifest exists\n",
            "blocker: dependency approval required\n",
        ),
    )
    .expect("invariant: blocked plan should be writable");
    let blocked_prepare = run(
        repo,
        &[
            "feature",
            "prepare",
            "all-blocked-setup",
            "--from",
            blocked_plan
                .to_str()
                .expect("invariant: plan path should be UTF-8"),
        ],
    );
    assert!(
        blocked_prepare.contains("feature remains ready"),
        "{blocked_prepare}"
    );
    assert!(
        blocked_prepare.contains("task-002 ready / blocked"),
        "{blocked_prepare}"
    );
    let feature = run(repo, &["feature", "show", "all-blocked-setup"]);
    assert!(feature.contains("status: ready"), "{feature}");
}
