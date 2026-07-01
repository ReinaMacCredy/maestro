pub mod card_support;
mod support;

use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use card_support::{cards_repo, id_by_title};
use maestro::domain::feature;
use maestro::foundation::core::paths::MaestroPaths;
use maestro::foundation::core::time::format_utc_seconds_rfc3339_millis;
use serde_json::Value;

fn maestro(repo: &Path, args: &[&str]) -> std::process::Output {
    maestro_with_extra_env(repo, args, &[])
}

fn maestro_with_extra_env(
    repo: &Path,
    args: &[&str],
    extra_env: &[(&str, &std::path::Path)],
) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_maestro"));
    command
        .args(args)
        .current_dir(repo)
        .env("MAESTRO_AGENT", "codex")
        .env("MAESTRO_SESSION_ID", "current-session");
    for (key, value) in extra_env {
        command.env(key, value);
    }
    command
        .output()
        .expect("invariant: compiled maestro binary should run in integration tests")
}

fn run_with_home(repo: &Path, args: &[&str]) -> String {
    let home = repo.join(".home");
    fs::create_dir_all(&home).expect("invariant: HOME should be creatable");
    let output = maestro_with_extra_env(repo, args, &[("HOME", home.as_path())]);
    assert!(
        output.status.success(),
        "maestro {args:?} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("invariant: stdout should be UTF-8")
}

fn run_failure_with_home(repo: &Path, args: &[&str]) -> String {
    let home = repo.join(".home");
    fs::create_dir_all(&home).expect("invariant: HOME should be creatable");
    let output = maestro_with_extra_env(repo, args, &[("HOME", home.as_path())]);
    assert!(
        !output.status.success(),
        "maestro {args:?} unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stderr).expect("invariant: stderr should be UTF-8")
}

fn run(repo: &Path, args: &[&str]) -> String {
    let output = maestro(repo, args);
    assert!(
        output.status.success(),
        "maestro {args:?} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("invariant: stdout should be UTF-8")
}

fn run_failure(repo: &Path, args: &[&str]) -> String {
    let output = maestro(repo, args);
    assert!(
        !output.status.success(),
        "maestro {args:?} unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stderr).expect("invariant: stderr should be UTF-8")
}

fn ts_minutes_ago(minutes: u64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("invariant: clock is after the Unix epoch")
        .as_secs();
    format_utc_seconds_rfc3339_millis(now - minutes * 60)
}

fn seed_run(repo: &Path, session: &str, lines: &[String]) {
    let run_dir = repo.join(".maestro/runs").join(session);
    fs::create_dir_all(&run_dir).expect("invariant: run dir should be creatable");
    fs::write(
        run_dir.join("events.jsonl"),
        format!("{}\n", lines.join("\n")),
    )
    .expect("invariant: event log fixture should be writable");
}

fn init_git_marker(repo: &Path) {
    fs::create_dir(repo.join(".git")).expect("invariant: .git marker should be creatable");
}

fn first_token_with_prefix(output: &str, prefix: &str) -> String {
    output
        .split_whitespace()
        .find(|word| word.starts_with(prefix))
        .unwrap_or_else(|| panic!("expected token starting with {prefix} in {output}"))
        .to_string()
}

#[test]
fn complete_harness_readout_surfaces_observability_liveness_and_proof_gaps() {
    let temp = cards_repo("complete-harness-readout");
    let repo = temp.path();

    run(
        repo,
        &[
            "task",
            "create",
            "Needs proof repair",
            "--check",
            "matching proof exists",
        ],
    );
    let task_id = id_by_title(repo, "Needs proof repair");
    run(repo, &["task", "explore", &task_id]);
    run(repo, &["task", "accept", &task_id]);
    run(repo, &["task", "claim", &task_id]);
    let failed_verify = run_failure(
        repo,
        &[
            "task",
            "complete",
            &task_id,
            "--summary",
            "submitted",
            "--claim",
            "matching proof exists",
            "--proof",
            "unrelated evidence",
        ],
    );
    assert!(failed_verify.contains("task remains: needs_verification"));

    let stale_ts = ts_minutes_ago(45);
    seed_run(
        repo,
        "stale-session",
        &[format!(
            r#"{{"event_type":"card_touch","session_id":"stale-session","card_id":"{task_id}","ts":"{stale_ts}"}}"#
        )],
    );

    let status: Value =
        serde_json::from_str(&run(repo, &["status", "--json"])).expect("status json");
    assert_eq!(
        status["complete_harness"]["observability"]["harness_protocol"],
        "missing"
    );
    assert_eq!(
        status["complete_harness"]["observability"]["stale_sessions"],
        1
    );
    assert_eq!(
        status["complete_harness"]["observability"]["proof_gap_tasks"],
        1
    );
    assert_eq!(status["complete_harness"]["status"], "incomplete");

    let resume: Value =
        serde_json::from_str(&run(repo, &["resume", "--json"])).expect("resume json");
    assert_eq!(
        resume["complete_harness"]["observability"]["proof_gap_tasks"],
        1
    );

    let query_run: Value =
        serde_json::from_str(&run(repo, &["query", "run", "--json"])).expect("query run json");
    assert_eq!(
        query_run["complete_harness"]["observability"]["stale_sessions"],
        1
    );

    let active = run(repo, &["active", "--all"]);
    assert!(
        active.contains("harness: observability/liveness incomplete"),
        "{active}"
    );
    assert!(active.contains("stale_sessions=1"), "{active}");
    assert!(active.contains("proof_gaps=1"), "{active}");
}

#[test]
fn hook_trace_coverage_is_reported_on_install_doctor_hook_query_and_task_proof() {
    let temp = support::TestTempDir::new("hook-trace-coverage");
    let repo = temp.path();

    run_with_home(repo, &["init", "--yes"]);
    let install = run_with_home(repo, &["install", "--agent", "codex"]);
    assert!(
        install.contains("harness: hook/trace missing_evidence"),
        "{install}"
    );
    assert!(install.contains("installed_agents=1"), "{install}");

    let doctor = run_with_home(repo, &["doctor"]);
    assert!(doctor.contains("check hook-trace: ok"), "{doctor}");
    assert!(doctor.contains("installed_agents=1"), "{doctor}");

    let hook = run_with_home(
        repo,
        &[
            "hook",
            "record",
            "--event",
            "card_touch",
            "--session",
            "trace-session",
        ],
    );
    assert!(hook.contains("harness: hook/trace partial"), "{hook}");
    assert!(hook.contains("card_touch=1"), "{hook}");

    run_with_home(
        repo,
        &[
            "task",
            "create",
            "Needs traced proof",
            "--check",
            "trace proof exists",
        ],
    );
    let task_id = id_by_title(repo, "Needs traced proof");
    run_with_home(repo, &["task", "explore", &task_id]);
    run_with_home(repo, &["task", "accept", &task_id]);
    run_with_home(repo, &["task", "claim", &task_id]);
    let failed_verify = run_failure_with_home(
        repo,
        &[
            "task",
            "complete",
            &task_id,
            "--summary",
            "submitted",
            "--claim",
            "trace proof exists",
            "--proof",
            "trace proof exists",
        ],
    );
    assert!(failed_verify.contains("task remains: needs_verification"));

    let proof = run_with_home(repo, &["task", "proof", &task_id]);
    assert!(proof.contains("harness: hook/trace complete"), "{proof}");
    assert!(proof.contains("task_proof=1"), "{proof}");

    let query_run: Value = serde_json::from_str(&run_with_home(repo, &["query", "run", "--json"]))
        .expect("query run json");
    assert_eq!(
        query_run["complete_harness"]["hook_trace"]["hook_wiring"],
        "installed"
    );
    assert!(
        query_run["complete_harness"]["hook_trace"]["card_touch_events"]
            .as_u64()
            .is_some_and(|count| count >= 1),
        "{query_run}"
    );
    assert_eq!(
        query_run["complete_harness"]["hook_trace"]["task_proof_events"],
        1
    );
}

#[test]
fn runtime_provider_tool_boundaries_are_reported_without_overclaiming_provider_model() {
    let temp = support::TestTempDir::new("runtime-boundary");
    let repo = temp.path();
    fs::write(
        repo.join("Cargo.toml"),
        "[package]\nname = \"runtime-boundary\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("invariant: Cargo.toml should be writable");

    run_with_home(repo, &["init", "--yes"]);
    let install = run_with_home(repo, &["install", "--agent", "codex"]);
    assert!(
        install.contains("harness: runtime/tool partial"),
        "{install}"
    );
    assert!(install.contains("stack=rust"), "{install}");
    assert!(install.contains("provider_model=unverified"), "{install}");

    let status: Value =
        serde_json::from_str(&run_with_home(repo, &["status", "--json"])).expect("status json");
    assert_eq!(
        status["complete_harness"]["runtime_boundary"]["stack_kind"],
        "rust"
    );
    assert_eq!(
        status["complete_harness"]["runtime_boundary"]["current_agent_runtime"],
        "codex"
    );
    assert_eq!(
        status["complete_harness"]["runtime_boundary"]["verify_commands"],
        3
    );
    assert_eq!(
        status["complete_harness"]["runtime_boundary"]["provider_model"]["status"],
        "unverified"
    );

    let resume = run_with_home(repo, &["resume"]);
    assert!(resume.contains("harness: runtime/tool partial"), "{resume}");

    let doctor = run_with_home(repo, &["doctor"]);
    assert!(doctor.contains("check runtime-boundary: ok"), "{doctor}");
    assert!(doctor.contains("stack=rust"), "{doctor}");

    let sync = run_with_home(repo, &["sync", "--dry-run"]);
    assert!(sync.contains("harness: runtime/tool partial"), "{sync}");
    assert!(sync.contains("mcp=available"), "{sync}");
}

#[test]
fn security_gates_reuse_task_feature_qa_decision_and_waiver_surfaces() {
    let temp = support::TestTempDir::new("security-gates");
    let repo = temp.path();
    init_git_marker(repo);

    run_with_home(repo, &["init", "--yes"]);
    run_with_home(repo, &["feature", "new", "Security Gate Harness"]);
    let feature_id = id_by_title(repo, "Security Gate Harness");
    run_with_home(
        repo,
        &[
            "feature",
            "set",
            &feature_id,
            "--acceptance",
            "release push requires explicit approval",
            "--area",
            "src",
        ],
    );
    run_with_home(repo, &["feature", "reconcile", &feature_id]);
    run_with_home(repo, &["feature", "finalize", &feature_id]);
    let qa = run_with_home(
        repo,
        &[
            "qa",
            "baseline",
            &feature_id,
            "--observed",
            "release gate baseline captured",
        ],
    );
    assert!(
        qa.contains("harness: security gate QA path captures required proof"),
        "{qa}"
    );
    let qa_artifact = feature::read_sidecar_text(&MaestroPaths::new(repo), &feature_id, "qa.md")
        .expect("qa artifact should be readable")
        .expect("qa artifact should exist");
    assert!(qa_artifact.contains("- Security gates:"), "{qa_artifact}");
    assert!(
        qa_artifact.contains("release_publish_push"),
        "{qa_artifact}"
    );
    run_with_home(repo, &["feature", "accept", &feature_id]);

    let created = run_with_home(
        repo,
        &[
            "task",
            "create",
            "Publish release",
            "--feature",
            &feature_id,
            "--covers",
            "ac-1",
            "--risk",
            "release_publish_push",
            "--check",
            "release approval is recorded",
        ],
    );
    assert!(
        created.contains("security_gate: release_publish_push"),
        "{created}"
    );
    let task_id = id_by_title(repo, "Publish release");
    let show = run_with_home(repo, &["task", "show", &task_id]);
    assert!(show.contains("risk: release_publish_push"), "{show}");
    assert!(
        show.contains("security_gate: release_publish_push"),
        "{show}"
    );
    run_with_home(
        repo,
        &[
            "task",
            "block",
            &task_id,
            "--reason",
            "waiting for explicit release approval",
        ],
    );

    let decision = run_with_home(
        repo,
        &[
            "decision",
            "new",
            "Choose release security gate",
            "--feature",
            &feature_id,
            "--context",
            "release_publish_push needs a human approval path",
            "--lock",
            "--decision",
            "require explicit approval before release publish push",
            "--rejected",
            "auto-publish without approval: too risky",
        ],
    );
    assert!(
        decision.contains("harness: security gate policy path"),
        "{decision}"
    );

    run_with_home(repo, &["feature", "start", &feature_id]);
    let waived = run_with_home(
        repo,
        &[
            "feature",
            "verify",
            &feature_id,
            "--waive",
            "ac-1",
            "--reason",
            "fixture does not actually push a release",
            "--no-close",
        ],
    );
    assert!(
        waived.contains("harness: security gate path proof=feature verify --prove"),
        "{waived}"
    );

    let status: Value =
        serde_json::from_str(&run_with_home(repo, &["status", "--json"])).expect("status json");
    assert_eq!(
        status["complete_harness"]["security_gates"]["status"],
        "complete"
    );
    assert_eq!(
        status["complete_harness"]["security_gates"]["classes"]
            .as_array()
            .expect("classes array")
            .len(),
        6
    );
    assert_eq!(
        status["complete_harness"]["security_gates"]["declared_risky_tasks"],
        1
    );
    assert_eq!(
        status["complete_harness"]["security_gates"]["blocked_risky_tasks"],
        1
    );
    assert_eq!(
        status["complete_harness"]["security_gates"]["qa_artifacts"],
        1
    );
    assert_eq!(
        status["complete_harness"]["security_gates"]["decision_records"],
        1
    );

    let doctor = run_with_home(repo, &["doctor"]);
    assert!(doctor.contains("check security-gates: ok"), "{doctor}");
    assert!(doctor.contains("release_publish_push"), "{doctor}");

    let resume = run_with_home(repo, &["resume"]);
    assert!(
        resume.contains("harness: security gates complete"),
        "{resume}"
    );
}

#[test]
fn structured_guardrails_report_decision_memory_scorer_task_check_and_agents_harness() {
    let temp = support::TestTempDir::new("structured-guardrails");
    let repo = temp.path();
    init_git_marker(repo);

    run_with_home(repo, &["init", "--yes"]);
    fs::write(
        repo.join("AGENTS.md"),
        "# Agent Notes\n\nGuardrail fixture for complete harness readout.\n",
    )
    .expect("AGENTS.md fixture should be writable");
    let intervention = run_with_home(
        repo,
        &[
            "event",
            "intervention",
            "--note",
            "Promote this correction into a durable guardrail",
            "--topic",
            "guardrail-promotion",
            "--run",
            "guardrail-run",
        ],
    );
    assert!(
        intervention.contains("recorded intervention event"),
        "{intervention}"
    );

    let suggestion = run_with_home(
        repo,
        &[
            "memory",
            "suggest",
            "create",
            "--source-ref",
            "run_event:guardrail-run",
            "--signal-type",
            "user_correction",
            "--summary",
            "Verify guardrail readout before handoff",
        ],
    );
    let suggestion_id = first_token_with_prefix(&suggestion, "msug-");
    let memory = run_with_home(repo, &["memory", "create", "--from", &suggestion_id]);
    assert!(
        memory.contains("harness: guardrail rule=memory_promotion_gate"),
        "{memory}"
    );
    let memory_id = first_token_with_prefix(&memory, "mem-");
    let memory_list = run_with_home(repo, &["memory", "list", "--all"]);
    assert!(
        memory_list.contains("harness: guardrail rule=memory_promotion_gate"),
        "{memory_list}"
    );
    let scorer = run_with_home(repo, &["scorer", "list", "--memory", &memory_id]);
    assert!(
        scorer.contains("harness: guardrail rule=scorer_receipt_gate"),
        "{scorer}"
    );

    let task = run_with_home(
        repo,
        &[
            "task",
            "create",
            "Check guardrail readout",
            "--check",
            "guardrail row is visible",
        ],
    );
    assert!(
        task.contains("harness: guardrail rule=task_check_verify_contract"),
        "{task}"
    );
    let task_id = id_by_title(repo, "Check guardrail readout");
    let task_show = run_with_home(repo, &["task", "show", &task_id]);
    assert!(
        task_show.contains("rule=task_check_verify_contract"),
        "{task_show}"
    );

    let decision = run_with_home(
        repo,
        &[
            "decision",
            "new",
            "Promote guardrail policy",
            "--context",
            "intervention should become a Memory candidate before policy promotion",
            "--lock",
            "--decision",
            "promote only through Memory gate and scorer/review evidence",
            "--rejected",
            "chat-only reminder: not durable",
        ],
    );
    assert!(
        decision.contains("harness: guardrail rule=decision_policy_gate"),
        "{decision}"
    );

    let status: Value =
        serde_json::from_str(&run_with_home(repo, &["status", "--json"])).expect("status json");
    assert_eq!(
        status["complete_harness"]["guardrails"]["status"],
        "complete"
    );
    assert_eq!(
        status["complete_harness"]["guardrails"]["intervention_events"],
        1
    );
    assert_eq!(
        status["complete_harness"]["guardrails"]["candidate_rules"],
        1
    );
    assert_eq!(
        status["complete_harness"]["guardrails"]["task_check_rules"],
        1
    );
    assert_eq!(
        status["complete_harness"]["guardrails"]["agents_harness_sources"],
        2
    );
    assert_eq!(
        status["complete_harness"]["guardrails"]["promotion_lifecycle"][0],
        "intervention"
    );
    assert_eq!(
        status["complete_harness"]["guardrails"]["promotion_lifecycle"][3],
        "promoted_or_rejected_rule"
    );
    assert!(
        status["complete_harness"]["guardrails"]["rules"]
            .as_array()
            .expect("rules array")
            .iter()
            .any(|rule| rule["bypass_policy"].as_str().is_some()
                && rule["source"].as_str().is_some()
                && rule["evidence"].as_array().is_some()
                && rule["severity"].as_str().is_some()),
        "{status}"
    );

    let doctor = run_with_home(repo, &["doctor"]);
    assert!(doctor.contains("check guardrails: ok"), "{doctor}");
    assert!(doctor.contains("rule_ids="), "{doctor}");

    let resume = run_with_home(repo, &["resume"]);
    assert!(resume.contains("harness: guardrails complete"), "{resume}");
}

#[test]
fn passive_scheduler_stance_is_reported_on_loop_next_watch_active_and_query_run() {
    let temp = support::TestTempDir::new("passive-scheduler");
    let repo = temp.path();
    init_git_marker(repo);

    run_with_home(repo, &["init", "--yes"]);
    run_with_home(
        repo,
        &[
            "task",
            "create",
            "Scheduled local work",
            "--check",
            "scheduler readout is visible",
        ],
    );
    let task_id = id_by_title(repo, "Scheduled local work");
    run_with_home(repo, &["task", "explore", &task_id]);
    run_with_home(repo, &["task", "accept", &task_id]);

    let recent_ts = ts_minutes_ago(1);
    let stale_ts = ts_minutes_ago(45);
    seed_run(
        repo,
        "heartbeat-session",
        &[format!(
            r#"{{"event_type":"card_touch","session_id":"heartbeat-session","card_id":"{task_id}","ts":"{recent_ts}"}}"#
        )],
    );
    seed_run(
        repo,
        "dead-session",
        &[format!(
            r#"{{"event_type":"card_touch","session_id":"dead-session","card_id":"{task_id}","ts":"{stale_ts}"}}"#
        )],
    );

    let next = run_with_home(repo, &["next"]);
    assert!(
        next.contains("harness: scheduler degraded (stance=passive_local_first"),
        "{next}"
    );
    assert!(next.contains("dead_runs=1"), "{next}");

    let active = run_with_home(repo, &["active", "--all"]);
    assert!(
        active.contains("harness: scheduler degraded (stance=passive_local_first"),
        "{active}"
    );
    assert!(active.contains("stale_sessions=1"), "{active}");

    let watch = run_with_home(repo, &["watch", "snapshot"]);
    assert!(
        watch.contains("harness: scheduler degraded (stance=passive_local_first"),
        "{watch}"
    );

    let query_run: Value = serde_json::from_str(&run_with_home(repo, &["query", "run", "--json"]))
        .expect("query run json");
    assert_eq!(
        query_run["complete_harness"]["scheduler"]["stance"],
        "passive_local_first"
    );
    assert_eq!(query_run["complete_harness"]["scheduler"]["owner"], "none");
    assert_eq!(query_run["complete_harness"]["scheduler"]["dead_runs"], 1);
    assert!(
        query_run["complete_harness"]["scheduler"]["heartbeat_events"]
            .as_u64()
            .is_some_and(|count| count >= 2),
        "{query_run}"
    );
    assert!(
        query_run["complete_harness"]["scheduler"]["surfaces"]
            .as_array()
            .expect("surfaces array")
            .iter()
            .any(|surface| surface == "loop"),
        "{query_run}"
    );

    let loop_lease: Value =
        serde_json::from_str(&run_with_home(repo, &["loop", "work-lease", "--json"]))
            .expect("work lease json");
    assert_eq!(loop_lease["scheduler"]["stance"], "passive_local_first");
    assert_eq!(loop_lease["scheduler"]["owner"], "none");
    assert_eq!(loop_lease["scheduler"]["dead_runs"], 1);
}

#[test]
fn integrated_proof_matrix_maps_each_gap_to_existing_surfaces_honestly() {
    let temp = support::TestTempDir::new("complete-proof-matrix");
    let repo = temp.path();
    init_git_marker(repo);

    run_with_home(repo, &["init", "--yes"]);
    fs::write(
        repo.join("AGENTS.md"),
        "# Agent Notes\n\nProof matrix fixture for complete harness readout.\n",
    )
    .expect("AGENTS.md fixture should be writable");

    let query_run: Value = serde_json::from_str(&run_with_home(repo, &["query", "run", "--json"]))
        .expect("query run json");
    let matrix = query_run["complete_harness"]["proof_matrix"]
        .as_array()
        .expect("proof_matrix array");
    assert_eq!(matrix.len(), 6);

    let gaps = matrix
        .iter()
        .map(|row| row["gap"].as_str().expect("gap"))
        .collect::<Vec<_>>();
    assert!(gaps.contains(&"observability_liveness"), "{gaps:?}");
    assert!(gaps.contains(&"hook_trace_coverage"), "{gaps:?}");
    assert!(gaps.contains(&"runtime_provider_tool_boundary"), "{gaps:?}");
    assert!(gaps.contains(&"risky_action_security_gates"), "{gaps:?}");
    assert!(gaps.contains(&"structured_guardrails"), "{gaps:?}");
    assert!(gaps.contains(&"passive_scheduler_liveness"), "{gaps:?}");

    let statuses = matrix
        .iter()
        .map(|row| row["status"].as_str().expect("status"))
        .collect::<Vec<_>>();
    assert!(statuses.contains(&"complete"), "{statuses:?}");
    assert!(statuses.contains(&"partial"), "{statuses:?}");
    assert!(statuses.contains(&"incomplete"), "{statuses:?}");

    for row in matrix {
        assert!(row["owning_surface"].as_str().is_some(), "{row}");
        assert!(row["honest_limit"].as_str().is_some(), "{row}");
        assert!(
            row["evidence"]
                .as_array()
                .is_some_and(|items| !items.is_empty()),
            "{row}"
        );
        assert!(
            row["inspect"]
                .as_array()
                .is_some_and(|items| !items.is_empty()),
            "{row}"
        );
        assert!(
            !row["owning_surface"]
                .as_str()
                .expect("owning surface")
                .contains("harness command"),
            "{row}"
        );
    }

    let hook_row = matrix
        .iter()
        .find(|row| row["gap"] == "hook_trace_coverage")
        .expect("hook row");
    assert_eq!(hook_row["status"], "incomplete");
    assert!(
        hook_row["owning_surface"]
            .as_str()
            .expect("owning surface")
            .contains("doctor, install, hook"),
        "{hook_row}"
    );

    let status = run_with_home(repo, &["status"]);
    assert!(status.contains("harness: proof matrix rows=6"), "{status}");

    let doctor = run_with_home(repo, &["doctor"]);
    assert!(doctor.contains("check proof-matrix: ok"), "{doctor}");
}
