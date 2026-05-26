mod support;

use std::fs;
use std::path::Path;
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

fn setup_old_repo() -> TestTempDir {
    let temp = TestTempDir::new("maestro-migrate");
    let repo = temp.path();
    fs::create_dir(repo.join(".git")).expect("invariant: .git marker should be creatable");
    fs::create_dir_all(repo.join(".maestro/tasks")).expect("invariant: tasks dir");
    fs::create_dir_all(repo.join(".maestro/missions")).expect("invariant: missions dir");
    fs::create_dir_all(repo.join(".maestro/evidence/session-1")).expect("invariant: evidence dir");
    fs::create_dir_all(repo.join(".maestro/features")).expect("invariant: features dir");
    fs::create_dir_all(repo.join(".maestro/decisions")).expect("invariant: decisions dir");
    fs::create_dir_all(repo.join(".maestro/policies")).expect("invariant: policies dir");
    fs::write(
        repo.join(".maestro/tasks/tasks.jsonl"),
        "{\"id\":\"task-001\",\"title\":\"Migrate old task\",\"state\":\"ready\",\"feature_id\":\"feat-one\",\"created_at\":\"1\",\"updated_at\":\"2\"}\n",
    )
    .expect("invariant: old task should be writable");
    fs::write(
        repo.join(".maestro/missions/missions.jsonl"),
        "{\"id\":\"mission-1\"}\n",
    )
    .expect("invariant: mission should be writable");
    fs::write(
        repo.join(".maestro/evidence/session-1/events.jsonl"),
        "{}\n",
    )
    .expect("invariant: evidence should be writable");
    fs::write(
        repo.join(".maestro/features/features.yaml"),
        concat!(
            "schema_version: maestro.feature.v1\n",
            "features:\n",
            "  - id: feat-one\n",
            "    title: Feature One\n",
            "    status: proposed\n",
            "    created_at: '1'\n",
            "    updated_at: '1'\n",
            "    tasks: [task-001]\n"
        ),
    )
    .expect("invariant: features should be writable");
    fs::write(
        repo.join(".maestro/decisions/ADR-001-old-choice.md"),
        "# Old Choice\n\nKeep this decision.\n",
    )
    .expect("invariant: ADR should be writable");
    fs::write(
        repo.join(".maestro/policies/verify.yaml"),
        "commands:\n  - cargo test\n",
    )
    .expect("invariant: policy should be writable");
    temp
}

#[test]
fn migrate_check_prints_diff_without_writing() {
    let temp = setup_old_repo();
    let repo = temp.path();
    let before = fs::read_to_string(repo.join(".maestro/tasks/tasks.jsonl"))
        .expect("invariant: old task should be readable");

    let output = maestro(repo, &["migrate", "--check"]);
    assert_success(&output, &["migrate", "--check"]);
    let out = stdout(&output);
    assert!(out.contains("migration check:"));
    assert!(out.contains(".maestro/tasks/task-001-migrate-old-task/task.yaml"));
    assert!(out.contains(".maestro/raw/archived/missions/missions.jsonl"));
    assert!(out.contains(".maestro/runs/migrated/session-1/events.jsonl"));
    assert!(out.contains(".maestro/decisions/decision-001-old-choice.md"));
    assert!(out.contains("cargo test"));
    assert_eq!(
        fs::read_to_string(repo.join(".maestro/tasks/tasks.jsonl"))
            .expect("invariant: old task should remain readable"),
        before
    );
    assert!(!repo
        .join(".maestro/tasks/task-001-migrate-old-task/task.yaml")
        .exists());
}

#[test]
fn migrate_apply_writes_v1_artifacts_and_backups_sources() {
    let temp = setup_old_repo();
    let repo = temp.path();

    let output = maestro(repo, &["migrate"]);
    assert_success(&output, &["migrate"]);
    assert!(stdout(&output).contains("migration applied:"));

    let task_yaml = repo.join(".maestro/tasks/task-001-migrate-old-task/task.yaml");
    assert!(task_yaml.exists());
    let task_raw = fs::read_to_string(task_yaml).expect("invariant: migrated task readable");
    assert!(task_raw.contains("schema_version: maestro.task.v1"));
    assert!(task_raw.contains("state: ready"));
    let features = fs::read_to_string(repo.join(".maestro/features/features.yaml"))
        .expect("features readable");
    assert!(!features.contains("tasks:"));
    assert!(repo
        .join(".maestro/raw/archived/missions/missions.jsonl")
        .exists());
    assert!(repo
        .join(".maestro/runs/migrated/session-1/events.jsonl")
        .exists());
    assert!(repo
        .join(".maestro/decisions/decision-001-old-choice.md")
        .exists());
    assert!(repo.join(".maestro/harness/harness.yml").exists());
    assert!(repo.join(".maestro/backups").exists());
}

#[test]
fn migrate_refuses_lock_file_without_force() {
    let temp = setup_old_repo();
    let repo = temp.path();
    fs::write(repo.join(".maestro/writer.lock"), "busy").expect("invariant: lock should write");

    let blocked = maestro(repo, &["migrate"]);
    assert_failure(&blocked, &["migrate"]);
    assert!(stderr(&blocked).contains("writer evidence"));

    let forced = maestro(repo, &["migrate", "--force"]);
    assert_success(&forced, &["migrate", "--force"]);
}
