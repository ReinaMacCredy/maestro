mod support;

use std::collections::BTreeSet;
use std::fs;
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
}

#[test]
fn doctor_fails_on_blocker_cycles() {
    let temp = setup_repo("maestro-doctor-blocker-cycle");
    let repo = temp.path();

    for args in [
        vec!["task", "create", "Task A"],
        vec!["task", "create", "Task B"],
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
}

fn maestro_files(repo: &Path) -> BTreeSet<PathBuf> {
    let mut files = BTreeSet::new();
    collect_files(&repo.join(".maestro"), repo, &mut files);
    files
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
