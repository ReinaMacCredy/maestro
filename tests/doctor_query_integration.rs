mod support;

use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs as unix_fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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

fn create_verified_task_with_proof(repo: &Path) {
    for args in [
        vec!["feature", "new", "Billing CSV export"],
        vec![
            "task",
            "create",
            "Add CSV export",
            "--feature",
            "billing-csv-export",
        ],
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
            "implemented CSV export",
        ],
    ] {
        assert_success(&maestro(repo, &args), &args);
    }

    let run_dir = repo.join(".maestro/runs/run-001");
    fs::create_dir_all(&run_dir).expect("invariant: run dir should be creatable");
    fs::write(
        run_dir.join("events.jsonl"),
        concat!(
            "{\"task_id\":\"task-001\",\"kind\":\"proof\",\"message\":\"implemented CSV export\"}\n",
            "{\"kind\":\"UserPromptSubmit\",\"message\":\"actually, check the blocker graph\"}\n"
        ),
    )
    .expect("invariant: events should be writable");

    assert_success(
        &maestro(repo, &["task", "verify", "task-001"]),
        &["task", "verify", "task-001"],
    );
}

#[test]
fn doctor_reports_ok_for_initialized_phase_three_artifacts() {
    let temp = setup_repo("maestro-doctor-ok");
    let repo = temp.path();
    create_verified_task_with_proof(repo);

    let doctor = maestro(repo, &["doctor"]);
    assert_success(&doctor, &["doctor"]);
    let out = stdout(&doctor);

    assert!(out.contains("check harness: ok"));
    assert!(out.contains("check features: ok"));
    assert!(out.contains("check backlog: ok"));
    assert!(out.contains("check task-blockers: ok"));
    assert!(out.contains("doctor: ok"));
}

#[test]
fn doctor_and_task_doctor_fail_on_bad_blocker_graph() {
    let temp = setup_repo("maestro-doctor-bad-blockers");
    let repo = temp.path();

    for args in [
        vec!["task", "create", "Self blocked task"],
        vec!["task", "set", "task-001", "--check", "self check"],
        vec!["task", "explore", "task-001"],
        vec!["task", "accept", "task-001"],
        vec![
            "task",
            "block",
            "task-001",
            "--reason",
            "waiting for itself",
            "--by",
            "task-001",
        ],
    ] {
        assert_success(&maestro(repo, &args), &args);
    }

    let doctor = maestro(repo, &["doctor"]);
    assert_failure(&doctor, &["doctor"]);
    assert!(stderr(&doctor).contains("self-blocking blocker"));

    let task_doctor = maestro(repo, &["task", "doctor"]);
    assert_failure(&task_doctor, &["task", "doctor"]);
    assert!(stderr(&task_doctor).contains("self-blocking blocker"));
    // The report names the remedy so doctor does not just say "wrong" without "what now".
    assert!(stderr(&task_doctor).contains("maestro task unblock"));
    assert!(stderr(&task_doctor).contains("can instead be archived"));
}

#[test]
fn doctor_fails_on_blocker_cycles() {
    let temp = setup_repo("maestro-doctor-blocker-cycle");
    let repo = temp.path();

    for args in [
        vec!["task", "create", "Task A"],
        vec!["task", "create", "Task B"],
        vec!["task", "set", "task-001", "--check", "task a check"],
        vec!["task", "set", "task-002", "--check", "task b check"],
        vec!["task", "explore", "task-001"],
        vec!["task", "accept", "task-001"],
        vec!["task", "explore", "task-002"],
        vec!["task", "accept", "task-002"],
        vec![
            "task",
            "block",
            "task-001",
            "--reason",
            "wait for B",
            "--by",
            "task-002",
        ],
        vec![
            "task",
            "block",
            "task-002",
            "--reason",
            "wait for A",
            "--by",
            "task-001",
        ],
    ] {
        assert_success(&maestro(repo, &args), &args);
    }

    let doctor = maestro(repo, &["doctor"]);
    assert_failure(&doctor, &["doctor"]);
    assert!(stderr(&doctor).contains("blocker cycle detected"));

    let task_doctor = maestro(repo, &["task", "doctor"]);
    assert_failure(&task_doctor, &["task", "doctor"]);
    assert!(stderr(&task_doctor).contains("blocker cycle detected"));
}

#[test]
fn doctor_and_task_doctor_flag_a_dangling_decision_blocker() {
    let temp = setup_repo("maestro-doctor-dangling-decision");
    let repo = temp.path();

    run_success(repo, &["decision", "new", "Adopt CSV schema"]); // decision-001
    for args in [
        vec!["task", "create", "Valid decision blocker"],
        vec!["task", "create", "Dangling decision blocker"],
        vec![
            "task", "block", "task-001", "--reason", "needs ADR", "--by", "decision-001",
        ],
    ] {
        assert_success(&maestro(repo, &args), &args);
    }

    // A resolvable decision blocker does not trip the doctor.
    assert_success(&maestro(repo, &["task", "doctor"]), &["task", "doctor"]);

    assert_success(
        &maestro(
            repo,
            &[
                "task", "block", "task-002", "--reason", "needs ADR", "--by", "decision-999",
            ],
        ),
        &["task", "block", "task-002", "--by", "decision-999"],
    );

    let task_doctor = maestro(repo, &["task", "doctor"]);
    assert_failure(&task_doctor, &["task", "doctor"]);
    assert!(
        stderr(&task_doctor).contains("referencing missing decision decision-999"),
        "{}",
        stderr(&task_doctor)
    );

    let doctor = maestro(repo, &["doctor"]);
    assert_failure(&doctor, &["doctor"]);
    assert!(stderr(&doctor).contains("referencing missing decision"));
}

#[test]
fn doctor_flags_a_deleted_installed_mirror() {
    let temp = setup_repo("maestro-doctor-install-integrity");
    let repo = temp.path();

    // Init'd but no agent installed: no install check, doctor stays ok.
    let pre = maestro(repo, &["doctor"]);
    assert_success(&pre, &["doctor"]);
    assert!(!stdout(&pre).contains("check install"));

    assert_success(
        &maestro(repo, &["install", "--agent", "claude"]),
        &["install", "--agent", "claude"],
    );
    let installed = maestro(repo, &["doctor"]);
    assert_success(&installed, &["doctor"]);
    assert!(stdout(&installed).contains("check install: ok"));

    // Deleting an owned mirror is caught.
    fs::remove_file(repo.join("CLAUDE.md"))
        .expect("invariant: installed CLAUDE.md should be removable");
    let broken = maestro(repo, &["doctor"]);
    assert_failure(&broken, &["doctor"]);
    assert!(
        stderr(&broken).contains("mirror is missing or broken"),
        "{}",
        stderr(&broken)
    );

    // Re-installing repairs it.
    assert_success(
        &maestro(repo, &["install", "--agent", "claude"]),
        &["install", "--agent", "claude"],
    );
    assert_success(&maestro(repo, &["doctor"]), &["doctor"]);
}

#[test]
fn doctor_counts_real_decisions_and_skips_symlinked_entries() {
    let temp = setup_repo("maestro-doctor-decision-symlink");
    let repo = temp.path();

    run_success(repo, &["decision", "new", "Use the domain decision reader"]);

    // Doctor now enumerates decisions through the domain reader, which skips
    // symlinks for parity with resolve_decision_path's symlink rejection. A
    // symlinked decision-*.md must not inflate the count (the previous inline
    // `is_file()` predicate followed the link and counted it).
    let decisions_dir = repo.join(".maestro/decisions");
    let real = decisions_dir.join("decision-001-use-the-domain-decision-reader.md");
    assert!(
        real.is_file(),
        "decision new should create the real decision file"
    );
    unix_fs::symlink(&real, decisions_dir.join("decision-002-symlinked.md"))
        .expect("invariant: symlink should be creatable in test");

    let doctor = maestro(repo, &["doctor"]);
    let out = stdout(&doctor);
    assert!(
        out.contains("check decisions: ok (1 decision file(s))"),
        "doctor should count only the real decision, not the symlink:\n{out}"
    );
}

#[test]
fn query_backlog_reports_empty_state_when_the_backlog_file_is_absent() {
    // R29/R22: a repo with no harness backlog (deleted, or never extracted)
    // must read as an empty backlog, not leak a raw ENOENT + absolute path.
    let temp = setup_repo("maestro-query-backlog-absent");
    let repo = temp.path();
    fs::remove_file(repo.join(".maestro/harness/backlog.yaml"))
        .expect("invariant: the scaffolded backlog should exist before removal");

    let backlog = run_success(repo, &["query", "backlog"]);
    assert!(
        backlog.contains("no backlog items found"),
        "absent backlog should report the empty state, got:\n{backlog}"
    );
    assert!(
        !backlog.contains("failed to read") && !backlog.contains("os error"),
        "absent backlog must not leak a raw io error:\n{backlog}"
    );
}

#[test]
fn query_views_scan_current_artifacts_without_writing_cache_files() {
    let temp = setup_repo("maestro-query-views");
    let repo = temp.path();
    create_verified_task_with_proof(repo);

    run_success(repo, &["decision", "new", "Use computed query views"]);
    fs::write(
        repo.join(".maestro/harness/backlog.yaml"),
        concat!(
            "schema_version: maestro.backlog.v1\n",
            "items:\n",
            "  - id: hb-001\n",
            "    title: Add query regression coverage\n"
        ),
    )
    .expect("invariant: backlog should be writable in test setup");

    let before = maestro_files(repo);

    let decisions = run_success(repo, &["query", "decisions"]);
    assert!(decisions.contains("decision-001-use-computed-query-views.md"));
    assert!(decisions.contains("Use computed query views"));

    let backlog = run_success(repo, &["query", "backlog"]);
    assert!(backlog.contains("hb-001"));
    assert!(backlog.contains("Add query regression coverage"));

    let matrix = run_success(repo, &["query", "matrix"]);
    assert!(matrix.contains("billing-csv-export"));
    assert!(matrix.contains("task-001"));
    assert!(matrix.contains("verified"));
    assert!(matrix.contains("accepted"));

    let friction = run_success(repo, &["query", "friction"]);
    assert!(friction.contains("FRICTION"));
    assert!(friction.contains("events: 2"));
    assert!(friction.contains("corrections: 1"));

    let proof = run_success(repo, &["query", "proof", "task-001"]);
    assert!(proof.contains("proof task-001: accepted"));
    assert!(proof.contains("verification.json"));

    let after = maestro_files(repo);
    assert_eq!(before, after);
    assert!(!repo.join(".maestro/cache").exists());
    assert!(!repo.join(".maestro/tmp").exists());

    fs::write(
        task_dir(repo, "task-001").join("acceptance.yaml"),
        concat!(
            "schema_version: maestro.acceptance.v1\n",
            "task: task-001\n",
            "checks:\n",
            "- changed query matrix proof binding\n",
            "locked_by: maestro\n",
            "locked_at: now\n"
        ),
    )
    .expect("invariant: acceptance should be writable for stale proof setup");
    let before_stale = maestro_files(repo);
    let stale_matrix = run_success(repo, &["query", "matrix"]);
    assert!(stale_matrix.contains("stale"));
    assert_eq!(before_stale, maestro_files(repo));
}

#[test]
fn query_friction_ignores_bad_json_and_symlinked_run_dirs() {
    let temp = setup_repo("maestro-query-friction-bad-events");
    let repo = temp.path();
    let runs_dir = repo.join(".maestro/runs");
    let run_dir = runs_dir.join("run-001");
    fs::create_dir_all(&run_dir).expect("invariant: run dir should be creatable");
    fs::write(
        run_dir.join("events.jsonl"),
        concat!(
            "{\"kind\":\"UserPromptSubmit\",\"message\":\"actually retry\"}\n",
            "not json\n"
        ),
    )
    .expect("invariant: events should be writable");
    let bad_run_dir = runs_dir.join("run-002");
    fs::create_dir_all(&bad_run_dir).expect("invariant: bad run dir should be creatable");
    fs::write(bad_run_dir.join("events.jsonl"), [0xff, b'\n'])
        .expect("invariant: bad events should be writable");
    unix_fs::symlink(&runs_dir, runs_dir.join("loop"))
        .expect("invariant: symlink should be creatable on unix test host");

    let friction = run_success(repo, &["query", "friction"]);
    assert!(friction.contains("events: 1"));
    assert!(friction.contains("corrections: 1"));
}

#[cfg(unix)]
#[test]
fn query_friction_ignores_events_when_maestro_root_is_symlinked() {
    let temp = TestTempDir::new("maestro-query-symlinked-root");
    let repo = temp.path();
    fs::create_dir(repo.join(".git")).expect("invariant: .git marker should be creatable");
    let external = TestTempDir::new("maestro-query-external-root");
    let external_run = external.path().join("runs/run-001");
    fs::create_dir_all(&external_run).expect("invariant: external run dir should be creatable");
    fs::write(
        external_run.join("events.jsonl"),
        "{\"kind\":\"UserPromptSubmit\",\"message\":\"actually retry\"}\n",
    )
    .expect("invariant: external event should be writable");
    unix_fs::symlink(external.path(), repo.join(".maestro"))
        .expect("invariant: symlinked .maestro root should be creatable");

    let friction = run_success(repo, &["query", "friction"]);

    assert!(friction.contains("friction: no events found"));
}

#[test]
fn query_matrix_ignores_symlinked_task_dirs() {
    let temp = setup_repo("maestro-query-symlinked-task");
    let repo = temp.path();
    let external_dir = repo.join("external-task");
    fs::create_dir(&external_dir).expect("invariant: external task dir should be creatable");
    fs::write(
        external_dir.join("task.yaml"),
        "schema_version: maestro.task.v1\nid: task-999\nslug: forged\nfeature_id: forged\nstate: verified\ntitle: Forged task\nacceptance_locked: true\nverification: {}\ncreated_at: now\nupdated_at: now\n",
    )
    .expect("invariant: forged task yaml should be writable");
    fs::create_dir_all(repo.join(".maestro/tasks"))
        .expect("invariant: tasks dir should be creatable");
    unix_fs::symlink(&external_dir, repo.join(".maestro/tasks/task-999-forged"))
        .expect("invariant: task symlink should be creatable");

    let matrix = run_success(repo, &["query", "matrix"]);
    assert!(!matrix.contains("task-999"));
    assert!(!matrix.contains("forged"));
}

#[test]
fn query_proof_reads_an_archived_task_through_the_archive_fallback() {
    let temp = setup_repo("maestro-query-proof-archived");
    let repo = temp.path();

    // A terminal, archived task still owns its proof; `query proof` is a read and
    // must fall through to the archive tree instead of erroring "not found".
    for args in [
        vec!["task", "create", "Retired task"],
        vec!["task", "abandon", "task-001", "--reason", "superseded"],
        vec!["task", "archive", "task-001"],
    ] {
        assert_success(&maestro(repo, &args), &args);
    }
    assert!(repo.join(".maestro/archive/tasks").exists());
    assert!(!repo.join(".maestro/tasks/task-001-retired-task").exists());

    let proof = run_success(repo, &["query", "proof", "task-001"]);
    assert!(proof.contains("proof task-001:"));
}

#[test]
fn query_proof_honors_maestro_current_task_like_task_show() {
    let temp = setup_repo("maestro-query-proof-env");
    let repo = temp.path();
    create_verified_task_with_proof(repo);

    // With no positional id, `query proof` reads MAESTRO_CURRENT_TASK (strict, no
    // single-task auto-detect), mirroring the sibling read view `task show`.
    let from_env = maestro_with_env(
        repo,
        &["query", "proof"],
        &[("MAESTRO_CURRENT_TASK", "task-001")],
    );
    assert_success(&from_env, &["query", "proof"]);
    assert!(stdout(&from_env).contains("proof task-001: accepted"));

    // An empty env value gives the "id required or set MAESTRO_CURRENT_TASK"
    // remedy, not a fall-through.
    let blank = maestro_with_env(repo, &["query", "proof"], &[("MAESTRO_CURRENT_TASK", "")]);
    assert_failure(&blank, &["query", "proof"]);
    assert!(stderr(&blank).contains("MAESTRO_CURRENT_TASK"));
}

#[test]
fn query_matrix_reports_an_empty_state_line_when_no_features_or_tasks_exist() {
    let temp = setup_repo("maestro-query-matrix-empty");
    let repo = temp.path();

    let matrix = run_success(repo, &["query", "matrix"]);
    assert!(matrix.contains("no features or tasks found"));
    assert!(!matrix.contains("FEATURE\tTASK"));
}

fn maestro_files(repo: &Path) -> BTreeSet<PathBuf> {
    let mut files = BTreeSet::new();
    collect_files(&repo.join(".maestro"), repo, &mut files);
    files
}

fn task_dir(repo: &Path, id: &str) -> PathBuf {
    let prefix = format!("{id}-");
    for entry in
        fs::read_dir(repo.join(".maestro/tasks")).expect("invariant: tasks dir should be readable")
    {
        let entry = entry.expect("invariant: task entry should be readable");
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if name.starts_with(&prefix) {
            return entry.path();
        }
    }
    panic!("invariant: task dir should exist for {id}");
}

fn collect_files(dir: &Path, repo: &Path, files: &mut BTreeSet<PathBuf>) {
    for entry in fs::read_dir(dir).expect("invariant: directory should be readable") {
        let entry = entry.expect("invariant: directory entry should be readable");
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, repo, files);
        } else if path.is_file() {
            files.insert(
                path.strip_prefix(repo)
                    .expect("invariant: path should be under repo")
                    .to_path_buf(),
            );
        }
    }
}
