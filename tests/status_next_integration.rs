pub mod card_support;
mod support;

use std::fs;
use std::path::Path;
use std::process::Command;

use card_support::{card_dir, card_record_path, id_by_title, sole_idea_id, task_record};
use git2::{Repository, Signature};
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

/// Like `setup_repo`, but initializes a real git repository (with an initial
/// commit so HEAD and a branch exist) instead of an empty `.git` marker. The
/// git readout reads via `git2`, which rejects the bare marker, so the git line
/// only renders in a real repo.
fn setup_git_repo(prefix: &str) -> (TestTempDir, Repository) {
    let temp = TestTempDir::new(prefix);
    let repository = Repository::init(temp.path()).expect("invariant: git repo should initialize");
    fs::write(temp.path().join("seed.txt"), "seed\n")
        .expect("invariant: seed file should be writable");
    commit_worktree(&repository, "seed");
    let init = maestro(temp.path(), &["init", "--yes"]);
    assert_success(&init, &["init", "--yes"]);
    let claims_only = maestro(temp.path(), &["harness", "set", "--claims-only"]);
    assert_success(&claims_only, &["harness", "set", "--claims-only"]);
    (temp, repository)
}

/// Commit every non-ignored worktree change (initial commit when HEAD is unborn,
/// otherwise on top of HEAD). `.maestro/` ignore rules are respected, so this
/// drives the code/other dirty count to zero without forcing the card store in.
fn commit_worktree(repository: &Repository, message: &str) {
    let mut index = repository
        .index()
        .expect("invariant: git index should be readable");
    index
        .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .expect("invariant: git index add should succeed");
    index
        .write()
        .expect("invariant: git index write should succeed");
    let tree_id = index
        .write_tree()
        .expect("invariant: git tree write should succeed");
    let tree = repository
        .find_tree(tree_id)
        .expect("invariant: git tree should exist");
    let signature = Signature::now("Maestro Test", "maestro@example.test")
        .expect("invariant: git signature should be constructable");
    let parent = repository
        .head()
        .ok()
        .and_then(|head| head.target())
        .and_then(|oid| repository.find_commit(oid).ok());
    let parents: Vec<&git2::Commit> = parent.iter().collect();
    repository
        .commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &parents,
        )
        .expect("invariant: git commit should succeed");
}

fn run(repo: &Path, args: &[&str]) -> String {
    let output = maestro(repo, args);
    assert_success(&output, args);
    stdout(&output)
}

/// The folded task record reconstructed into the old `task.yaml` shape, so an
/// assertion written against `doc["state"]`/`doc["blockers"]` reads unchanged.
fn task_yaml(repo: &Path, id: &str) -> YamlValue {
    task_record(repo, id)
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
fn proposed_feature_next_hint_tracks_handoff_and_qa_baseline_readiness() {
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
    assert!(status.contains("run: finalize_feature"), "{status}");
    assert!(!status.contains("template: set_contract"), "{status}");

    let feature_list = run(repo, &["feature", "list"]);
    assert!(feature_list.contains("authored-contract"), "{feature_list}");
    assert!(
        feature_list.contains("run: finalize_feature"),
        "{feature_list}"
    );
    assert!(
        !feature_list.contains("template: set_contract"),
        "{feature_list}"
    );

    run(repo, &["feature", "finalize", "authored-contract"]);

    let status = run(repo, &["status"]);
    assert!(status.contains("authored-contract"), "{status}");
    assert!(status.contains("template: qa_baseline"), "{status}");
    assert!(!status.contains("run: finalize_feature"), "{status}");
    assert!(!status.contains("run: accept_feature"), "{status}");

    let feature_show = run(repo, &["feature", "show", "authored-contract"]);
    assert!(
        feature_show.contains("next: maestro qa baseline authored-contract"),
        "{feature_show}"
    );

    run(
        repo,
        &[
            "qa",
            "baseline",
            "authored-contract",
            "--observed",
            "current status output recorded before implementation",
        ],
    );

    let status = run(repo, &["status"]);
    assert!(status.contains("authored-contract"), "{status}");
    assert!(status.contains("run: accept_feature"), "{status}");
    assert!(!status.contains("template: qa_baseline"), "{status}");

    let feature_list = run(repo, &["feature", "list"]);
    assert!(
        feature_list.contains("run: accept_feature"),
        "{feature_list}"
    );

    let feature_show = run(repo, &["feature", "show", "authored-contract"]);
    assert!(
        feature_show.contains("next: maestro feature accept authored-contract"),
        "{feature_show}"
    );
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
fn next_default_reports_best_action_without_mutating_ready_task() {
    let temp = setup_repo("maestro-next-read-only-ready");
    let repo = temp.path();
    run(
        repo,
        &["task", "create", "Ready task", "--check", "ready check"],
    );
    let ready = id_by_title(repo, "Ready task");
    run(repo, &["task", "explore", &ready]);
    run(repo, &["task", "accept", &ready]);

    let next = run(repo, &["next"]);

    assert!(next.contains("run: maestro task claim"), "{next}");
    assert!(next.contains(&format!("task: {ready}")), "{next}");
    assert_eq!(task_yaml(repo, &ready)["state"].as_str(), Some("ready"));
    assert!(task_yaml(repo, &ready)["claimed_by"].is_null());
}

#[test]
fn next_json_uses_next_schema_and_marks_claim_task_auto_safe() {
    let temp = setup_repo("maestro-next-json-ready");
    let repo = temp.path();
    run(
        repo,
        &["task", "create", "Ready task", "--check", "ready check"],
    );
    let ready = id_by_title(repo, "Ready task");
    run(repo, &["task", "explore", &ready]);
    run(repo, &["task", "accept", &ready]);

    let json = run(repo, &["next", "--json"]);
    let parsed: JsonValue = serde_json::from_str(&json).expect("invariant: next JSON should parse");

    assert_eq!(parsed["schema"], "maestro.next.v1");
    assert_eq!(parsed["mode"], "suggest");
    assert_eq!(parsed["next_action"]["kind"], "claim_task");
    assert_eq!(parsed["next_action"]["auto_safe"], true);
    assert_eq!(parsed["next_action"]["task_id"], ready);
}

#[test]
fn next_run_claims_only_auto_safe_ready_task() {
    let temp = setup_repo("maestro-next-run-ready");
    let repo = temp.path();
    run(
        repo,
        &["task", "create", "Ready task", "--check", "ready check"],
    );
    let ready = id_by_title(repo, "Ready task");
    run(repo, &["task", "explore", &ready]);
    run(repo, &["task", "accept", &ready]);

    let next = run(repo, &["next", "--run"]);

    assert!(next.contains("auto-safe: maestro task claim"), "{next}");
    assert_eq!(
        task_yaml(repo, &ready)["state"].as_str(),
        Some("in_progress")
    );
    assert!(!task_yaml(repo, &ready)["claimed_by"].is_null());
}

#[test]
fn next_run_refuses_input_requiring_completion_template() {
    let temp = setup_repo("maestro-next-run-refuses-input");
    let repo = temp.path();
    run(
        repo,
        &["task", "create", "Ready task", "--check", "ready check"],
    );
    let ready = id_by_title(repo, "Ready task");
    run(repo, &["task", "explore", &ready]);
    run(repo, &["task", "accept", &ready]);
    run(repo, &["task", "claim", &ready]);

    let next = maestro(repo, &["next", "--run"]);

    assert_failure(&next, &["next", "--run"]);
    let out = stdout(&next);
    assert!(out.contains("blocked: next action requires input"), "{out}");
    assert!(out.contains("template: maestro task complete"), "{out}");
    assert_eq!(
        task_yaml(repo, &ready)["state"].as_str(),
        Some("in_progress")
    );
}

#[test]
fn next_loop_stops_after_first_blocker_and_reports_transcript() {
    let temp = setup_repo("maestro-next-loop-ready");
    let repo = temp.path();
    run(
        repo,
        &["task", "create", "Ready task", "--check", "ready check"],
    );
    let ready = id_by_title(repo, "Ready task");
    run(repo, &["task", "explore", &ready]);
    run(repo, &["task", "accept", &ready]);

    let next = run(repo, &["next", "--loop", "--max-steps", "5"]);

    assert!(next.contains("step 1/5:"), "{next}");
    assert!(next.contains("auto-safe: maestro task claim"), "{next}");
    assert!(
        next.contains("blocked: next action requires input"),
        "{next}"
    );
    assert_eq!(
        task_yaml(repo, &ready)["state"].as_str(),
        Some("in_progress")
    );
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

    let status = maestro_with_env(
        repo,
        &["status", "--json"],
        &[("MAESTRO_CURRENT_TASK", &draft)],
    );
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
    assert!(status.contains("more: maestro resume"), "{status}");

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
        resume.contains(&format!(
            "run focused gate, then maestro task complete {id}"
        )),
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
    let resume_md = card_dir(repo, &id).join("resume.md");
    let resume_rel = resume_md
        .strip_prefix(repo)
        .expect("invariant: resume artifact lives under the repo");
    assert!(
        written.contains(&format!("wrote: {}", resume_rel.display())),
        "{written}"
    );
    let resume_doc = fs::read_to_string(&resume_md)
        .expect("invariant: explicit resume artifact should be readable");
    assert!(resume_doc.contains("generated_at:"), "{resume_doc}");
    assert!(resume_doc.contains("source references:"), "{resume_doc}");
    let record_rel = card_record_path(repo, &id);
    let record_rel = record_rel
        .strip_prefix(repo)
        .expect("invariant: card record lives under the repo");
    assert!(
        resume_doc.contains(&record_rel.display().to_string()),
        "{resume_doc}"
    );
    assert!(resume_doc.contains("handoff prompt:"), "{resume_doc}");
}

#[test]
fn resume_and_status_show_git_line_and_clean_note_for_close_or_verify_state() {
    let (temp, repository) = setup_git_repo("maestro-git-line-close");
    let repo = temp.path();
    let branch = repository
        .head()
        .expect("invariant: head should exist after the seed commit")
        .shorthand()
        .expect("invariant: the seed commit should be on a named branch")
        .to_string();

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
    fs::write(repo.join("feature_change.rs"), "fn changed() {}\n")
        .expect("invariant: code change should be writable");

    // Leg (a): next verb is `task complete` (close/verify-shaped) AND there are
    // uncommitted code/other changes -> the git line and clean note both show.
    let resume = run(repo, &["resume"]);
    assert!(
        resume.contains(&format!("git: {branch},")),
        "resume git line: {resume}"
    );
    assert!(
        resume.contains("code/other") && resume.contains("maestro-card"),
        "resume git counts: {resume}"
    );
    assert!(
        resume.contains("before the close/verify step"),
        "resume clean-worktree note should show: {resume}"
    );

    let status = run(repo, &["status"]);
    assert!(
        status.contains(&format!("git: {branch},")),
        "status git line: {status}"
    );
    assert!(
        status.contains("code/other") && status.contains("maestro-card"),
        "status git counts: {status}"
    );
    assert!(
        status.contains("before the close/verify step"),
        "status clean-worktree note should show: {status}"
    );

    // Leg (c): commit the worktree so code/other == 0 -> the git line stays but
    // the note drops, even though the next verb is still close/verify-shaped.
    commit_worktree(&repository, "commit worktree");
    let resume_clean = run(repo, &["resume"]);
    assert!(
        resume_clean.contains(&format!("git: {branch},")),
        "resume git line after commit: {resume_clean}"
    );
    assert!(
        !resume_clean.contains("before the close/verify step"),
        "clean worktree must drop the note on resume: {resume_clean}"
    );

    let status_clean = run(repo, &["status"]);
    assert!(
        status_clean.contains(&format!("git: {branch},")),
        "status git line after commit: {status_clean}"
    );
    assert!(
        !status_clean.contains("before the close/verify step"),
        "clean worktree must drop the note on status: {status_clean}"
    );
}

#[test]
fn resume_and_status_omit_clean_note_when_next_verb_is_not_close_shaped() {
    let (temp, _repository) = setup_git_repo("maestro-git-line-not-close");
    let repo = temp.path();
    // A draft task: the next verb is "author checks or explore", not
    // close/verify-shaped, so the clean note stays off even with code dirty.
    run(repo, &["task", "create", "Draft task"]);
    fs::write(repo.join("loose_change.rs"), "fn loose() {}\n")
        .expect("invariant: code change should be writable");

    let resume = run(repo, &["resume"]);
    assert!(resume.contains("git:"), "resume git line present: {resume}");
    assert!(
        !resume.contains("before the close/verify step"),
        "non-close verb must omit the note on resume: {resume}"
    );

    let status = run(repo, &["status"]);
    assert!(status.contains("git:"), "status git line present: {status}");
    assert!(
        !status.contains("before the close/verify step"),
        "non-close verb must omit the note on status: {status}"
    );
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

/// SPEC-archive-memory-2 R3: once something is archived, `resume` surfaces a
/// short memory section -- the freshest INDEX.md lid lines plus a pointer to
/// the full file. An unarchived repo shows no section at all, and `status`
/// stays memory-free either way.
#[test]
fn resume_surfaces_recent_archive_memory() {
    let temp = setup_repo("maestro-resume-memory");
    let repo = temp.path();

    let before = run(repo, &["resume"]);
    assert!(
        !before.contains("memory:"),
        "no archive, no memory section: {before}"
    );

    run(repo, &["feature", "new", "Old export"]);
    run(
        repo,
        &["feature", "cancel", "old-export", "--reason", "scope cut"],
    );
    run(repo, &["feature", "archive", "old-export"]);

    let resume = run(repo, &["resume"]);
    assert!(resume.contains("memory:"), "{resume}");
    assert!(
        resume.contains("old-export: closed -- no outcome recorded"),
        "the lid line surfaces:\n{resume}"
    );
    assert!(
        resume.contains("full lid: .maestro/archive/cards/INDEX.md"),
        "{resume}"
    );

    let status = run(repo, &["status"]);
    assert!(
        !status.contains("memory:"),
        "status stays memory-free: {status}"
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
    let friction = sole_idea_id(repo);
    assert!(
        status.contains(&format!("! friction {friction} over threshold")),
        "{status}"
    );
    assert!(
        status.contains(&format!("apply: maestro harness apply {friction}")),
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
    assert!(
        untabify(&list).contains("ID\t!\tSTATUS\tTYPE\tSEEN\tTITLE"),
        "{list}"
    );
    assert!(
        untabify(&list).contains(&format!(
            "{friction}\t!\tproposed\trecurring_intervention\t9x/3s"
        )),
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
fn hot_verbs_persist_the_detect_stamp_and_self_heal_when_the_cards_tree_changes() {
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
    // D7: the detected friction is persisted solely as an `idea` card (the
    // backlog has no file of its own), and the detect-skip evidence stamp lands
    // post-write in `.maestro/harness/detect-stamp`. The repeated hot verbs above
    // skip re-detection while the stamp matches, so the card carries exactly the
    // 3 sessions of the single detection in its folded `extra.sessions_hit`.
    assert!(
        !repo.join(".maestro/harness/backlog.yaml").exists(),
        "the backlog must not have a file of its own"
    );
    assert!(
        repo.join(".maestro/harness/detect-stamp").is_file(),
        "detection should persist the skip stamp"
    );
    let friction = sole_idea_id(repo);
    let record = task_record(repo, &friction);
    assert_eq!(
        record["sessions_hit"].as_sequence().unwrap().len(),
        3,
        "{record:?}"
    );

    // The stamp covers the whole cards tree, so clearing the persisted friction
    // card invalidates it: the next hot verb re-detects and the friction
    // re-surfaces (self-healing) instead of staying silently skipped. The
    // friction idea lives as an `ideas.yaml` entry, so its record file is the
    // container file itself.
    fs::remove_file(card_record_path(repo, &friction))
        .expect("invariant: friction card should be removable");
    let healed = run(repo, &["status"]);
    assert!(healed.contains("HARNESS FRICTION"), "{healed}");

    // The stamp also covers the harness config -- its thresholds shape what
    // detect proposes -- so an edit invalidates the skip and the re-detection
    // persists a fresh stamp.
    let stamp_path = repo.join(".maestro/harness/detect-stamp");
    let before = fs::read_to_string(&stamp_path).expect("read the stamp");
    assert!(
        before.contains("config="),
        "the stamp covers the config: {before}"
    );
    let config_path = repo.join(".maestro/harness/harness.yml");
    let mut config = fs::read_to_string(&config_path).expect("read harness.yml");
    config.push_str("# threshold review note\n");
    fs::write(&config_path, config).expect("edit harness.yml");
    run(repo, &["status"]);
    let after = fs::read_to_string(&stamp_path).expect("re-read the stamp");
    assert_ne!(before, after, "a config edit invalidates the detect stamp");
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
    assert!(
        human.contains("inspect any: maestro feature show <id>"),
        "{human}"
    );
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
fn ready_to_close_status_json_and_task_next_broader_actions_are_structured() {
    let temp = setup_repo("maestro-ready-to-close-json");
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
    run(repo, &["feature", "finalize", "csv-export"]);
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
        complete
            .contains("next: maestro-card skill (qa-slice) -> replay affected baseline scenarios"),
        "{complete}"
    );
    assert!(
        complete.contains("then: maestro feature close csv-export --outcome \"<outcome>\""),
        "{complete}"
    );

    let status = maestro(repo, &["status", "--json"]);
    assert_success(&status, &["status", "--json"]);
    let status_json: JsonValue =
        serde_json::from_str(&stdout(&status)).expect("invariant: status JSON should parse");
    let ready = &status_json["sections"]["ready_to_close"][0];
    assert_eq!(ready["feature_id"], "csv-export");
    assert_eq!(ready["next_action"]["kind"], "feature_close");
    assert!(ready["next_action"]["command"]["argv"].is_null());
    assert_eq!(
        ready["next_action"]["command"]["argv_template"],
        serde_json::json!([
            "maestro",
            "feature",
            "close",
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
        "feature_ready_to_close"
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

    let before_root_verify = fs::read_to_string(card_record_path(repo, &id))
        .expect("invariant: verified task record should be readable");
    let root_verify = maestro(repo, &["verify", &id]);
    assert_failure(&root_verify, &["verify", &id]);
    let root_verify_err = stderr(&root_verify);
    assert!(
        root_verify_err.contains(&format!("cannot verify task {id}")),
        "{root_verify_err}"
    );
    assert!(
        root_verify_err.contains("expected needs_verification"),
        "{root_verify_err}"
    );
    assert_eq!(
        fs::read_to_string(card_record_path(repo, &id))
            .expect("invariant: verified task record should remain readable"),
        before_root_verify
    );
}

#[test]
fn status_summarizes_other_active_tasks_and_points_to_task_list() {
    let temp = setup_repo("maestro-status-row-limit");
    let repo = temp.path();

    for i in 0..6 {
        run(repo, &["task", "create", &format!("Draft task {i}")]);
    }

    let status = run(repo, &["status"]);

    assert!(
        status.contains("+5 other active tasks: maestro task list"),
        "{status}"
    );
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
    assert!(!list.contains("INSPECT"));
    assert!(list.contains("run: explore"));
    assert!(list.contains("inspect any: maestro task show <id>"));
}

#[test]
fn feature_linked_task_create_drops_inherited_verify_explainer() {
    let temp = setup_repo("maestro-create-feature-linked-handoff");
    let repo = temp.path();

    run(repo, &["feature", "new", "CSV export"]);
    let create = run(
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

    // Computed delta stays: created line, feature binding, one next: pointer.
    assert!(
        create.contains(&format!("created {id} (draft)")),
        "{create}"
    );
    assert!(create.contains("feature: csv-export"), "{create}");
    assert!(create.contains("next:"), "{create}");
    // The standing inherited-verify explainer is gone.
    assert!(
        !create.contains("verify+ inherited from feature:"),
        "{create}"
    );
    assert!(
        !create.contains("task check: optional for feature-linked tasks"),
        "{create}"
    );
}

#[test]
fn task_list_next_column_uses_verify_contract_state_not_only_lifecycle_state() {
    let temp = setup_repo("maestro-list-missing-check");
    let repo = temp.path();

    run(repo, &["task", "create", "Update README"]);
    let id = id_by_title(repo, "Update README");

    let list = run(repo, &["task", "list"]);
    assert!(list.contains("template: add_check"), "{list}");
    assert!(
        list.contains("inspect any: maestro task show <id>"),
        "{list}"
    );
    assert!(
        !untabify(&list).contains(&format!("{id}\tdraft\trun: explore")),
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
    assert!(complete.contains("recorded proof (17 bytes)"));
    assert!(
        !complete.contains("proof: cargo test passes"),
        "task complete must not echo the full proof body:\n{complete}"
    );
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
    run(repo, &["feature", "finalize", "serverless-news-backend"]);
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
    let draft_path = repo.join(".maestro/cards/serverless-news-backend/prepare-draft.md");
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
            "  - covers: ac-1\n",
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
    assert!(
        prepare.contains(&format!("{t2} ready / blocked")),
        "{prepare}"
    );
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

    let covers_args = ["task", "set", t1.as_str(), "--covers", "ac-1"];
    let locked = maestro(repo, &covers_args);
    assert_failure(&locked, &covers_args);
    let locked_stderr = stderr(&locked);
    assert!(
        locked_stderr.contains("covers links cannot be changed after accept"),
        "{locked_stderr}"
    );
    assert!(
        locked_stderr
            .contains("maestro feature verify serverless-news-backend --prove <ac-id> --evidence"),
        "{locked_stderr}"
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
    run(repo, &["feature", "finalize", "no-inferred-blockers"]);
    run(repo, &["feature", "accept", "no-inferred-blockers"]);
    let vague_plan = repo.join("PLAN-no-infer.md");
    fs::write(
        &vague_plan,
        concat!(
            "## Task T1: Scaffold dependencies\n",
            "covers: ac-1\n",
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
    run(repo, &["feature", "finalize", "all-blocked-setup"]);
    run(repo, &["feature", "accept", "all-blocked-setup"]);
    let blocked_plan = repo.join("PLAN-all-blocked.md");
    fs::write(
        &blocked_plan,
        concat!(
            "## Task T1: Scaffold approved dependencies\n",
            "covers: ac-1\n",
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

/// ac-4/5/6: a task left in needs_verification with a failed proof surfaces a
/// concern-only proof line that names the exact repair (`maestro task verify`),
/// on both the default `resume` and `status` surfaces, and the verb pointer is
/// `task verify`, not `query proof`.
#[test]
fn needs_verification_failed_proof_surfaces_verify_repair_on_resume_and_status() {
    let temp = setup_repo("maestro-proof-concern-failed");
    let repo = temp.path();
    run(
        repo,
        &[
            "task",
            "create",
            "Failed proof concern",
            "--check",
            "observable behavior",
        ],
    );
    let id = id_by_title(repo, "Failed proof concern");
    run(repo, &["task", "explore", &id]);
    run(repo, &["task", "accept", &id]);
    run(repo, &["task", "claim", &id]);
    // No backing event: complete fails verification and persists status=failed
    // while the task remains in needs_verification.
    let complete = maestro(
        repo,
        &[
            "task",
            "complete",
            &id,
            "--summary",
            "done",
            "--claim",
            "observable behavior",
        ],
    );
    assert_failure(&complete, &["task", "complete", &id]);

    let resume = run(repo, &["resume"]);
    assert!(
        resume.contains(&format!(
            "proof: failed; fix, then re-verify: maestro task verify {id}"
        )),
        "resume must surface the failed-proof concern line:\n{resume}"
    );
    assert!(
        resume.contains(&format!("recover proof with maestro task verify {id}")),
        "resume next action must point at task verify, not query proof:\n{resume}"
    );

    let status = run(repo, &["status"]);
    assert!(
        status.contains(&format!(
            "proof: failed; fix, then re-verify: maestro task verify {id}"
        )),
        "status must surface the failed-proof concern line:\n{status}"
    );
    assert!(
        status.contains(&format!("maestro task verify {id}")),
        "status next action must point at task verify:\n{status}"
    );
}

/// ac-4/5: a verified task whose proof commit no longer matches HEAD surfaces a
/// stale concern framed as a refresh and naming the commit drift; a still-fresh
/// verified task surfaces no concern line at all.
#[test]
fn verified_stale_proof_surfaces_refresh_repair_via_resume_task() {
    let (temp, repository) = setup_git_repo("maestro-proof-concern-stale");
    let repo = temp.path();
    run(
        repo,
        &[
            "task",
            "create",
            "Stale proof concern",
            "--check",
            "observable behavior",
        ],
    );
    let id = id_by_title(repo, "Stale proof concern");
    run(repo, &["task", "explore", &id]);
    run(repo, &["task", "accept", &id]);
    run(repo, &["task", "claim", &id]);
    run(
        repo,
        &[
            "event",
            "create",
            "--task-id",
            &id,
            "--claim",
            "observable behavior",
        ],
    );
    let complete = run(
        repo,
        &[
            "task",
            "complete",
            &id,
            "--summary",
            "done",
            "--claim",
            "observable behavior",
        ],
    );
    assert!(
        complete.contains(&format!("task verified: {id}")),
        "task should auto-verify to verified:\n{complete}"
    );
    let verified_oid = repository
        .head()
        .ok()
        .and_then(|head| head.target())
        .expect("invariant: HEAD should resolve after verify")
        .to_string();

    // Fresh verified proof: no concern line on the focal verified task.
    let fresh = run(repo, &["resume", "--task", &id]);
    assert!(
        !fresh.contains("proof:"),
        "a fresh verified task must surface no proof concern line:\n{fresh}"
    );

    // Move HEAD so the stored verified_commit no longer matches.
    commit_worktree(&repository, "advance head past the verified commit");
    let new_oid = repository
        .head()
        .ok()
        .and_then(|head| head.target())
        .expect("invariant: HEAD should resolve after advancing")
        .to_string();

    let stale = run(repo, &["resume", "--task", &id]);
    assert!(
        stale.contains(&format!(
            "proof: stale ({}->{}); refresh (HEAD moved, likely no code change): maestro task verify {id}",
            &verified_oid[..7],
            &new_oid[..7],
        )),
        "stale verified proof must surface the refresh repair naming the commit drift:\n{stale}"
    );
}

/// ac-7: `feature close --dry-run` prints a non-blocking advisory naming each
/// verified child task whose recorded proof commit no longer matches HEAD. A
/// feature whose verified children all match HEAD prints none, and the advisory
/// never blocks a dry-run that would otherwise pass.
#[test]
fn feature_close_dry_run_flags_verified_children_at_older_commits_without_blocking() {
    let (temp, repository) = setup_git_repo("maestro-close-advisory-drift");
    let repo = temp.path();

    run(repo, &["feature", "new", "Close advisory"]);
    run(
        repo,
        &[
            "feature",
            "set",
            "close-advisory",
            "--acceptance",
            "advisory behaves",
            "--area",
            "close",
        ],
    );
    let feature_dir = repo.join(".maestro/cards/close-advisory");
    fs::write(
        feature_dir.join("qa.md"),
        "---\namend_log_position: 0\n---\n\n### QA Baseline Contract\n\n- Scenario Matrix:\n  - [bl-001] advisory behaves (covers: ac-1)\n",
    )
    .expect("invariant: qa.md should be writable");
    run(repo, &["feature", "finalize", "close-advisory"]);
    run(repo, &["feature", "accept", "close-advisory"]);
    run(repo, &["feature", "start", "close-advisory"]);

    // A verified child task, recorded at the seed commit. It settles BEFORE the
    // acceptance sweep below, so the sweep is never flagged stale by it.
    run(
        repo,
        &[
            "task",
            "create",
            "Child of advisory",
            "--feature",
            "close-advisory",
        ],
    );
    let child = id_by_title(repo, "Child of advisory");
    run(repo, &["task", "explore", &child]);
    run(repo, &["task", "accept", &child]);
    run(repo, &["task", "claim", &child]);
    let complete = run(
        repo,
        &[
            "task",
            "complete",
            &child,
            "--summary",
            "done",
            "--claim",
            "advisory behaves",
            "--proof",
            "advisory behaves",
        ],
    );
    assert!(
        complete.contains(&format!("task verified: {child}")),
        "child task should auto-verify:\n{complete}"
    );

    // Cover the baseline scenario (clears the QA gate and resolves ac-1 via the
    // counting slice), then run the acceptance sweep last so it stays fresh.
    let mut qa = fs::read_to_string(feature_dir.join("qa.md")).expect("invariant: qa.md readable");
    qa.push_str("\n```yaml\nslices:\n  - scenarios: [\"bl-001\"]\n    evidence: [\"proof for bl-001\"]\n```\n");
    fs::write(feature_dir.join("qa.md"), qa).expect("invariant: qa.md should be writable");
    run(repo, &["feature", "verify", "close-advisory"]);

    // Before HEAD moves: the verified child matches HEAD -> would close, no advisory.
    let fresh = run(repo, &["feature", "close", "close-advisory", "--dry-run"]);
    assert!(
        fresh.contains("would close"),
        "a fully-gated feature should pass the dry-run:\n{fresh}"
    );
    assert!(
        !fresh.contains("verified at older commits"),
        "no commit drift means no advisory:\n{fresh}"
    );

    // Advance HEAD past the child's recorded proof commit.
    commit_worktree(&repository, "advance head past the verified child commit");

    let drifted = run(repo, &["feature", "close", "close-advisory", "--dry-run"]);
    assert!(
        drifted.contains("would close"),
        "the advisory must not block a dry-run that would otherwise pass:\n{drifted}"
    );
    assert!(
        drifted.contains(&format!(
            "verified at older commits (HEAD moved); re-verify if their code changed: {child}"
        )),
        "the dry-run must flag the verified child whose proof commit drifted from HEAD:\n{drifted}"
    );
}
