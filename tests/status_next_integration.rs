mod card_support;
mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use card_support::{id_by_title, task_record};
use maestro::foundation::core::fs::ensure_dir;
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
    let claims_only = maestro(temp.path(), &["harness", "set", "--claims-only"]);
    assert_success(&claims_only, &["harness", "set", "--claims-only"]);
    temp
}

fn run(repo: &Path, args: &[&str]) -> String {
    let output = maestro(repo, args);
    assert_success(&output, args);
    stdout(&output)
}

/// The folded task record (the old `task.yaml` shape) carried under `card.extra`,
/// so an assertion written against `doc["state"]`/`doc["blockers"]` reads unchanged.
fn task_yaml(repo: &Path, id: &str) -> YamlValue {
    task_record(repo, id)
}

/// The flat card directory `.maestro/cards/<id>`; the task record is its `card.yaml`
/// and a written resume artifact lands at `<card_dir>/resume.md`.
fn card_dir(repo: &Path, id: &str) -> PathBuf {
    repo.join(".maestro/cards").join(id)
}

fn write_baseline(repo: &Path, feature_id: &str) {
    let dir = repo.join(".maestro/cards").join(feature_id);
    ensure_dir(&dir).expect("invariant: card directory should be creatable");
    fs::write(
        dir.join("qa.md"),
        "---\namend_log_position: 0\n---\n\n### QA Baseline Contract\n\n- Scenario Matrix:\n  - [bl-001] csv export round-trips\n",
    )
    .expect("invariant: qa.md should be writable");
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

#[test]
fn proposed_feature_with_authored_contract_points_status_and_feature_list_to_qa_baseline() {
    let temp = setup_repo("maestro-status-authored-feature-next");
    let repo = temp.path();

    run(repo, &["feature", "new", "Authored Contract"]);
    run(
        repo,
        &[
            "feature",
            "set",
            "authored-contract",
            "--acceptance",
            "agent can verify the authored contract",
            "--area",
            "status next hints",
        ],
    );

    let status = run(repo, &["status"]);
    assert!(status.contains("authored-contract"), "{status}");
    assert!(status.contains("template: qa_baseline"), "{status}");
    assert!(!status.contains("template: set_contract"), "{status}");

    let feature_list = run(repo, &["feature", "list"]);
    assert!(feature_list.contains("authored-contract"), "{feature_list}");
    assert!(
        feature_list.contains("template: qa_baseline"),
        "{feature_list}"
    );
    assert!(
        !feature_list.contains("template: set_contract"),
        "{feature_list}"
    );
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
    assert!(stdout(&next).contains("no actionable task"));
    assert!(stderr(&next).contains("no actionable task"));
}

#[test]
fn status_and_task_next_choose_current_task_before_ready_queue() {
    let temp = setup_repo("maestro-status-current");
    let repo = temp.path();
    run(
        repo,
        &["task", "create", "Ready task", "--check", "ready check"],
    );
    let ready = id_by_title(repo, "Ready task");
    run(repo, &["task", "explore", &ready]);
    run(repo, &["task", "accept", &ready]);
    run(repo, &["task", "create", "Draft task"]);
    let draft = id_by_title(repo, "Draft task");

    let next = maestro_with_env(repo, &["task", "next"], &[("MAESTRO_CURRENT_TASK", &draft)]);

    assert_success(&next, &["task", "next"]);
    let out = stdout(&next);
    assert!(out.contains(&format!("template: maestro task set {draft} --check")));
    assert!(out.contains(&format!("task: {draft}")));

    let status =
        maestro_with_env(repo, &["status", "--json"], &[("MAESTRO_CURRENT_TASK", &draft)]);
    assert_success(&status, &["status", "--json"]);
    let json: JsonValue =
        serde_json::from_str(&stdout(&status)).expect("invariant: status JSON should parse");
    assert_eq!(json["schema"], "maestro.status.v1");
    assert_eq!(json["current_task"], draft);
    assert_eq!(json["next_action"]["kind"], "add_task_check");
    assert_eq!(json["next_action"]["requires_input"], true);
    assert_eq!(
        json["next_action"]["command"]["display"],
        format!("maestro task set {draft} --check \"<observable result>\"")
    );
    assert!(json["next_action"]["command"]["argv"].is_null());
    assert_eq!(
        json["next_action"]["command"]["argv_template"],
        serde_json::json!([
            "maestro",
            "task",
            "set",
            draft,
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
fn status_points_to_resume_and_resume_default_is_compact_read_only() {
    let temp = setup_repo("maestro-resume-compact");
    let repo = temp.path();
    run(repo, &["feature", "new", "CSV export"]);
    fs::write(
        repo.join("IMPLEMENTATION_NOTES-csv-export.md"),
        "# notes\n\n- resume from here\n",
    )
    .expect("invariant: implementation notes should be writable");
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
    let id = id_by_title(repo, "Implement CSV writer");
    run(repo, &["task", "explore", &id]);
    run(repo, &["task", "accept", &id]);
    run(repo, &["task", "claim", &id]);

    let status = run(repo, &["status"]);
    assert!(status.contains("resume: maestro resume"), "{status}");

    let resume = run(repo, &["resume"]);
    assert!(
        resume.contains(&format!(
            "objective: continue task {id} for feature csv-export"
        )),
        "{resume}"
    );
    assert!(resume.contains("state: in_progress"), "{resume}");
    assert!(resume.contains("blockers: none"), "{resume}");
    assert!(resume.contains("next:"), "{resume}");
    assert!(
        resume.contains(&format!("run focused gate, then maestro task complete {id}")),
        "{resume}"
    );
    assert!(resume.contains("required reads:"), "{resume}");
    assert!(
        resume.contains(&format!("maestro task show {id}"))
            && resume.contains("maestro feature show csv-export")
            && resume.contains("IMPLEMENTATION_NOTES-csv-export.md"),
        "{resume}"
    );
    assert!(resume.contains("guardrails:"), "{resume}");
    assert!(
        resume.contains("preserve unrelated dirty files"),
        "{resume}"
    );
    assert!(
        resume.contains("do not commit planning or notes artifacts unless asked"),
        "{resume}"
    );
    assert!(
        !resume.contains("prior decisions:")
            && !resume.contains("handoff prompt:")
            && !resume.contains("proof history"),
        "default resume should stay compact: {resume}"
    );
    assert!(
        !card_dir(repo, &id).join("resume.md").exists(),
        "default resume must not write a resume artifact"
    );

    let json = run(repo, &["resume", "--json"]);
    let parsed: JsonValue =
        serde_json::from_str(&json).expect("invariant: resume JSON should parse");
    assert_eq!(parsed["schema"], "maestro.resume.v1");
    assert_eq!(parsed["mode"], "compact");
    assert_eq!(parsed["state"], "in_progress");
    assert!(parsed["full"].is_null());
}

#[test]
fn resume_full_handoff_and_write_are_explicit() {
    let temp = setup_repo("maestro-resume-full-write");
    let repo = temp.path();
    run(repo, &["feature", "new", "CSV export"]);
    run(repo, &["decision", "new", "Use compact resume by default"]);
    let decision = id_by_title(repo, "Use compact resume by default");
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
    let id = id_by_title(repo, "Implement CSV writer");
    run(repo, &["task", "explore", &id]);
    run(repo, &["task", "accept", &id]);
    run(repo, &["task", "claim", &id]);

    let full = run(repo, &["resume", "--full"]);
    assert!(full.contains("prior decisions:"), "{full}");
    assert!(
        full.contains(&format!("{decision}: Use compact resume by default")),
        "{full}"
    );
    assert!(full.contains("source references:"), "{full}");
    assert!(!full.contains("handoff prompt:"), "{full}");
    assert!(
        !card_dir(repo, &id).join("resume.md").exists(),
        "--full without --write must not write an artifact"
    );

    let handoff = run(repo, &["resume", "--handoff"]);
    assert!(handoff.contains("handoff prompt:"), "{handoff}");
    assert!(
        !card_dir(repo, &id).join("resume.md").exists(),
        "--handoff without --write must not write an artifact"
    );

    let written = run(repo, &["resume", "--handoff", "--write"]);
    assert!(
        written.contains(&format!("wrote: .maestro/cards/{id}/resume.md")),
        "{written}"
    );
    let resume_md = card_dir(repo, &id).join("resume.md");
    let resume_doc = fs::read_to_string(&resume_md)
        .expect("invariant: explicit resume artifact should be readable");
    assert!(resume_doc.contains("generated_at:"), "{resume_doc}");
    assert!(resume_doc.contains("source references:"), "{resume_doc}");
    assert!(
        resume_doc.contains(&format!(".maestro/cards/{id}/card.yaml")),
        "{resume_doc}"
    );
    assert!(resume_doc.contains("handoff prompt:"), "{resume_doc}");
}

#[test]
fn resume_feature_target_does_not_select_unrelated_tasks() {
    let temp = setup_repo("maestro-resume-feature-target");
    let repo = temp.path();
    run(repo, &["feature", "new", "CSV export"]);
    run(repo, &["feature", "new", "Search ranking"]);
    run(
        repo,
        &[
            "task",
            "create",
            "Tune ranking scorer",
            "--feature",
            "search-ranking",
        ],
    );
    let id = id_by_title(repo, "Tune ranking scorer");
    run(repo, &["task", "explore", &id]);
    run(repo, &["task", "accept", &id]);
    run(repo, &["task", "claim", &id]);

    let resume = run(repo, &["resume", "--feature", "csv-export"]);
    assert!(
        resume.contains("objective: continue feature csv-export"),
        "{resume}"
    );
    assert!(
        resume.contains("maestro feature show csv-export"),
        "{resume}"
    );
    assert!(
        !resume.contains(&id) && !resume.contains("search-ranking"),
        "explicit feature target must not leak unrelated task context: {resume}"
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
    let id = id_by_title(repo, "Ready task");
    run(repo, &["task", "explore", &id]);
    run(repo, &["task", "accept", &id]);

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
    let id = id_by_title(repo, "Ready task");
    run(repo, &["task", "explore", &id]);
    run(repo, &["task", "accept", &id]);

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
        .find(&format!("run: maestro task claim {id}"))
        .expect("invariant: task next should keep normal next action");
    assert!(friction_at < normal_at, "{next}");

    let list = run(repo, &["harness", "list"]);
    assert!(list.contains("ID\t!\tSTATUS\tTYPE\tSEEN\tTITLE"), "{list}");
    assert!(
        list.contains("hb-001\t!\tproposed\trecurring_intervention\t9x/3s"),
        "{list}"
    );

    run(repo, &["task", "claim", &id]);
    let complete = run(
        repo,
        &[
            "task",
            "complete",
            &id,
            "--summary",
            "done",
            "--claim",
            "ready proof",
            "--proof",
            "ready proof",
        ],
    );
    assert!(
        complete.contains(&format!("verification passed for {id}")),
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
    // In card mode the detected friction is persisted as an `idea` card
    // (`.maestro/cards/hb-001`), not in `backlog.yaml.items` (which stays `[]`);
    // its session evidence rides the card's folded `extra.sessions_hit`.
    assert!(backlog_yaml(repo)["items"].as_sequence().unwrap().is_empty());
    let friction = task_record(repo, "hb-001");
    assert_eq!(
        friction["sessions_hit"].as_sequence().unwrap().len(),
        3,
        "{friction:?}"
    );

    // Hot verbs skip re-detection while the evidence stamp is unchanged: clearing
    // the persisted friction card without bumping the stamp must not re-surface it.
    fs::remove_dir_all(repo.join(".maestro/cards/hb-001"))
        .expect("invariant: friction card should be removable");
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
    let id = id_by_title(repo, "Implement CSV writer");

    let status = maestro_with_env(
        repo,
        &["status", "--json"],
        &[
            ("MAESTRO_CURRENT_TASK", &id),
            ("MAESTRO_CURRENT_FEATURE", "wrong-feature"),
        ],
    );
    assert_success(&status, &["status", "--json"]);
    let json: JsonValue =
        serde_json::from_str(&stdout(&status)).expect("invariant: status JSON should parse");

    assert_eq!(json["current_task"], id);
    assert_eq!(json["current_feature"], "csv-export");
    assert_eq!(json["next_action"]["feature_id"], "csv-export");
    assert_eq!(
        json["next_action"]["command"]["argv"],
        serde_json::json!(["maestro", "task", "explore", id])
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
    let id = id_by_title(repo, "Ready task");
    run(repo, &["task", "explore", &id]);
    run(repo, &["task", "accept", &id]);

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
    assert!(next_out.contains(&format!("run: maestro task claim {id}")));
}

#[test]
fn current_ready_task_next_claims_selected_task_not_generic_next() {
    let temp = setup_repo("maestro-status-current-ready-task");
    let repo = temp.path();
    run(
        repo,
        &[
            "task",
            "create",
            "First ready task",
            "--check",
            "first check",
        ],
    );
    let first = id_by_title(repo, "First ready task");
    run(repo, &["task", "explore", &first]);
    run(repo, &["task", "accept", &first]);
    run(
        repo,
        &[
            "task",
            "create",
            "Current ready task",
            "--check",
            "second check",
        ],
    );
    let current = id_by_title(repo, "Current ready task");
    run(repo, &["task", "explore", &current]);
    run(repo, &["task", "accept", &current]);

    let human = maestro_with_env(
        repo,
        &["task", "next"],
        &[("MAESTRO_CURRENT_TASK", &current)],
    );
    assert_success(&human, &["task", "next"]);
    let human_out = stdout(&human);
    assert!(
        human_out.contains(&format!("run: maestro task claim {current}")),
        "{human_out}"
    );
    assert!(
        !human_out.contains("maestro task claim --next"),
        "{human_out}"
    );

    let json = maestro_with_env(
        repo,
        &["task", "next", "--json"],
        &[("MAESTRO_CURRENT_TASK", &current)],
    );
    assert_success(&json, &["task", "next", "--json"]);
    let parsed: JsonValue =
        serde_json::from_str(&stdout(&json)).expect("invariant: task next JSON should parse");
    assert_eq!(parsed["next_action"]["task_id"], current);
    assert_eq!(
        parsed["next_action"]["command"]["argv"],
        serde_json::json!(["maestro", "task", "claim", current])
    );
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
    let id = id_by_title(repo, "Implement CSV writer");
    run(repo, &["task", "explore", &id]);
    run(repo, &["task", "accept", &id]);
    run(repo, &["task", "claim", &id]);
    let complete = run(
        repo,
        &[
            "task",
            "complete",
            &id,
            "--summary",
            "done",
            "--claim",
            "CSV export round-trips",
            "--proof",
            "CSV export round-trips",
        ],
    );
    assert!(
        complete.contains("next: qa-slice skill -> replay affected baseline scenarios"),
        "{complete}"
    );
    assert!(
        complete.contains("then: maestro feature ship csv-export --outcome \"<outcome>\""),
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
    let id = id_by_title(repo, "Manual proof task");
    run(repo, &["task", "explore", &id]);
    run(repo, &["task", "accept", &id]);
    run(repo, &["task", "claim", &id]);
    let complete = maestro(
        repo,
        &[
            "task",
            "complete",
            &id,
            "--summary",
            "done",
            "--claim",
            "manual proof passes",
        ],
    );
    assert_failure(&complete, &["task", "complete", &id]);
    run(
        repo,
        &[
            "event",
            "create",
            "--task-id",
            &id,
            "--claim",
            "manual proof passes",
        ],
    );

    let task_verify = run(repo, &["task", "verify", &id]);
    assert!(task_verify.contains(&format!("verification passed for {id}")));
    assert!(task_verify.contains(&format!("task verified: {id}")));
    assert!(task_verify.contains("next: maestro status"));
    assert!(task_verify.contains(&format!("inspect: maestro task show {id}")));

    let root_verify = run(repo, &["verify", &id]);
    assert!(root_verify.contains(&format!("verification passed for {id}")));
    assert!(root_verify.contains(&format!("task verified: {id}")));
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
    let id = id_by_title(repo, "Add export");

    assert!(create.contains(&format!("created {id} (draft)")));
    assert!(create.contains("verify+ locked:"));
    assert!(create.contains(&format!("next: maestro task explore {id}")));

    let list = run(repo, &["task", "list"]);
    assert!(list.contains("NEXT"));
    assert!(list.contains("INSPECT"));
    assert!(list.contains("run: explore"));
    assert!(list.contains(&format!("maestro task show {id}")));
}

#[test]
fn task_list_next_column_uses_verify_contract_state_not_only_lifecycle_state() {
    let temp = setup_repo("maestro-list-missing-check");
    let repo = temp.path();

    run(repo, &["task", "create", "Update README"]);
    let id = id_by_title(repo, "Update README");

    let list = run(repo, &["task", "list"]);
    assert!(list.contains("template: add_check"), "{list}");
    assert!(list.contains(&format!("maestro task show {id}")), "{list}");
    assert!(
        !list.contains(&format!("{id}\tdraft\trun: explore")),
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
    let id = id_by_title(repo, "Add export");
    run(repo, &["task", "explore", &id]);
    run(repo, &["task", "accept", &id]);
    run(repo, &["task", "claim", &id]);
    let complete = run(
        repo,
        &[
            "task",
            "complete",
            &id,
            "--summary",
            "done",
            "--claim",
            "cargo test passes",
            "--proof",
            "cargo test passes",
        ],
    );

    assert!(complete.contains("auto: recorded task_proof event"));
    assert!(complete.contains(&format!("auto: maestro task verify {id}")));
    assert!(complete.contains(&format!("verification passed for {id}")));
    assert_eq!(
        task_yaml(repo, &id)["state"],
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
    let id = id_by_title(repo, "Implement CSV writer");
    run(repo, &["task", "explore", &id]);
    run(repo, &["task", "accept", &id]);
    run(repo, &["task", "claim", &id]);
    let complete = run(
        repo,
        &[
            "task",
            "complete",
            &id,
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
    let draft_path = repo.join(".maestro/features/serverless-news-backend/prepare-draft.md");
    let draft_contents =
        fs::read_to_string(draft_path).expect("invariant: prepare draft should be readable");
    assert!(
        draft_contents.contains("## Task T1: Implement accepted behavior"),
        "{draft_contents}"
    );
    assert!(
        draft_contents.contains("check: GET /articles returns records"),
        "{draft_contents}"
    );
    assert!(
        !draft_contents.contains("dependency approval required"),
        "{draft_contents}"
    );

    let plan = repo.join("PLAN-serverless-news.md");
    fs::write(
        &plan,
        concat!(
            "# Implementation Plan\n",
            "\n",
            "## Task Plan\n",
            "\n",
            "- Task T1: Implement protected read handlers\n",
            "  - check: GET /articles returns compact paginated records\n",
            "  - check: missing or invalid demo API key is rejected\n",
            "\n",
            "2. T2: Implement operation handlers\n",
            "   - after: T1\n",
            "   - check: POST /collect and POST /retry satisfy the API contract\n",
            "\n",
            "### Task T3 - Complete deploy gate\n",
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
    // The prepare mints opaque ids; recover them by their plan-derived titles. The
    // plan-local labels (T1/T2) stay in the blocker reason text, only the
    // parenthetical id is now opaque.
    let t1 = id_by_title(repo, "Implement protected read handlers");
    let t2 = id_by_title(repo, "Implement operation handlers");
    assert!(prepare.contains("prepared 3 task(s)"), "{prepare}");
    assert!(
        prepare.contains("started serverless-news-backend -> in_progress"),
        "{prepare}"
    );
    assert!(prepare.contains(&format!("{t2} ready / blocked")), "{prepare}");
    assert!(
        prepare.contains(&format!("after dependency: T1 ({t1}) verified")),
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

    let task_002 = task_yaml(repo, &t2);
    assert_eq!(task_002["state"], YamlValue::String("ready".to_string()));
    assert_eq!(
        task_002["blockers"][0]["reason"],
        YamlValue::String(format!("after dependency: T1 ({t1}) verified"))
    );

    let claim = run(repo, &["task", "claim", "--next"]);
    assert!(
        claim.contains(&format!("claimed {t1} -> in_progress")),
        "{claim}"
    );
    assert!(
        claim.contains("feature: serverless-news-backend"),
        "{claim}"
    );
    assert!(claim.contains("chain:"), "{claim}");
    assert!(
        claim.contains(&format!("{t1} current  Implement protected read handlers")),
        "{claim}"
    );
    assert!(
        claim.contains(&format!("{t2} blocked  Implement operation handlers")),
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
            &t1,
            "--summary",
            "read handlers done",
            "--claim",
            "GET /articles returns compact paginated records",
            "--proof",
            "GET /articles returns compact paginated records",
        ],
    );
    assert!(
        complete.contains(&format!("verification passed for {t1}")),
        "{complete}"
    );
    assert!(
        complete.contains("next: maestro task claim --next"),
        "{complete}"
    );

    let task_002_after = task_yaml(repo, &t2);
    assert_ne!(
        task_002_after["blockers"][0]["resolved_at"],
        YamlValue::Null
    );

    let next_claim = run(repo, &["task", "claim", "--next"]);
    assert!(
        next_claim.contains(&format!("claimed {t2} -> in_progress")),
        "{next_claim}"
    );
    assert!(
        next_claim.contains(&format!("{t1} verified Implement protected read handlers")),
        "{next_claim}"
    );
    assert!(
        next_claim.contains(&format!("{t2} current  Implement operation handlers")),
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
    let vague_id = id_by_title(repo, "Scaffold dependencies");
    let vague_task = task_yaml(repo, &vague_id);
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
    let blocked_id = id_by_title(repo, "Scaffold approved dependencies");
    assert!(
        blocked_prepare.contains("feature remains ready"),
        "{blocked_prepare}"
    );
    assert!(
        blocked_prepare.contains(&format!("{blocked_id} ready / blocked")),
        "{blocked_prepare}"
    );
    let feature = run(repo, &["feature", "show", "all-blocked-setup"]);
    assert!(feature.contains("status: ready"), "{feature}");
}
