mod support;

use std::fs;
use std::os::unix::fs as unix_fs;
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
    fs::create_dir_all(repo.join(".maestro/verdicts")).expect("invariant: verdicts dir");
    fs::create_dir_all(repo.join(".maestro/handoffs")).expect("invariant: handoffs dir");
    fs::create_dir_all(repo.join(".maestro/plans")).expect("invariant: plans dir");
    fs::create_dir_all(repo.join(".maestro/intake")).expect("invariant: intake dir");
    fs::create_dir_all(repo.join(".maestro/evidence/session-1")).expect("invariant: evidence dir");
    fs::create_dir_all(repo.join(".maestro/features")).expect("invariant: features dir");
    fs::create_dir_all(repo.join(".maestro/decisions")).expect("invariant: decisions dir");
    fs::create_dir_all(repo.join(".maestro/policies")).expect("invariant: policies dir");
    fs::create_dir_all(repo.join(".maestro/workflows")).expect("invariant: workflows dir");
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
        repo.join(".maestro/verdicts/task-001.json"),
        concat!(
            "{\"verdict\":\"pass\",",
            "\"verified_at\":\"3\",",
            "\"verified_commit\":\"abc123\",",
            "\"verified_by_run\":\"run-1\",",
            "\"task_contract_hash\":\"task-hash\",",
            "\"acceptance_hash\":\"acceptance-hash\",",
            "\"checks_hash\":\"checks-hash\"}\n"
        ),
    )
    .expect("invariant: verdict should be writable");
    fs::write(repo.join(".maestro/handoffs/task-001.md"), "# Handoff\n")
        .expect("invariant: handoff should be writable");
    fs::write(repo.join(".maestro/plans/plan-001.md"), "# Plan\n")
        .expect("invariant: plan should be writable");
    fs::write(
        repo.join(".maestro/intake/task-001.yaml"),
        concat!(
            "raw_request: demo\n",
            "input_type: user_request\n",
            "affected_areas:\n",
            "  - cli\n",
            "open_questions:\n",
            "  - confirm rollout\n"
        ),
    )
    .expect("invariant: intake should be writable");
    fs::write(
        repo.join(".maestro/evidence/session-1/events.jsonl"),
        [0, 159, 146, 150],
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
    fs::write(
        repo.join(".maestro/workflows/default.yaml"),
        "steps:\n  - task list\n",
    )
    .expect("invariant: workflow should be writable");
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
    assert!(out.contains(".maestro/raw/archived/verdicts/task-001.json"));
    assert!(out.contains(".maestro/raw/archived/handoffs/task-001.md"));
    assert!(out.contains(".maestro/raw/archived/plans/plan-001.md"));
    assert!(out.contains(".maestro/raw/archived/intake/task-001.yaml"));
    assert!(out.contains(".maestro/runs/migrated/session-1/events.jsonl"));
    assert!(out.contains(".maestro/decisions/decision-001-old-choice.md"));
    assert!(out.contains("cargo test"));
    assert!(out.contains("workflow:"));
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
    assert!(task_raw.contains("raw_request: demo"));
    assert!(task_raw.contains("input_type: user_request"));
    assert!(task_raw.contains("migrated handoff: Handoff"));
    assert!(task_raw.contains("verified_commit: abc123"));
    assert!(task_raw.contains("acceptance_hash: acceptance-hash"));
    let features = fs::read_to_string(repo.join(".maestro/features/features.yaml"))
        .expect("features readable");
    assert!(!features.contains("tasks:"));
    assert!(repo
        .join(".maestro/raw/archived/missions/missions.jsonl")
        .exists());
    assert!(repo
        .join(".maestro/raw/archived/verdicts/task-001.json")
        .exists());
    assert!(repo
        .join(".maestro/raw/archived/handoffs/task-001.md")
        .exists());
    assert!(repo
        .join(".maestro/raw/archived/plans/plan-001.md")
        .exists());
    assert!(repo
        .join(".maestro/raw/archived/intake/task-001.yaml")
        .exists());
    assert!(repo
        .join(".maestro/runs/migrated/session-1/events.jsonl")
        .exists());
    assert_eq!(
        fs::read(repo.join(".maestro/runs/migrated/session-1/events.jsonl"))
            .expect("invariant: migrated binary evidence readable"),
        vec![0, 159, 146, 150]
    );
    assert!(repo
        .join(".maestro/decisions/decision-001-old-choice.md")
        .exists());
    assert!(repo.join(".maestro/harness/harness.yml").exists());
    let harness =
        fs::read_to_string(repo.join(".maestro/harness/harness.yml")).expect("harness readable");
    assert!(harness.contains("workflow:"));
    assert!(harness.contains("task list"));
    let backup_files = backup_files(repo);
    assert!(backup_files
        .iter()
        .any(|path| path.ends_with(".maestro/tasks/tasks.jsonl")));
    assert!(backup_files
        .iter()
        .any(|path| path.ends_with(".maestro/missions/missions.jsonl")));
    assert!(backup_files
        .iter()
        .any(|path| path.ends_with(".maestro/features/features.yaml")));
}

fn backup_files(repo: &Path) -> Vec<String> {
    let mut files = Vec::new();
    collect_files(repo, &repo.join(".maestro/backups"), &mut files);
    files.sort();
    files
}

fn collect_files(repo: &Path, dir: &Path, files: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(repo, &path, files);
        } else if path.is_file() {
            files.push(
                path.strip_prefix(repo)
                    .unwrap_or(&path)
                    .display()
                    .to_string(),
            );
        }
    }
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

#[test]
fn migrate_rejects_task_ids_that_escape_task_directory() {
    let temp = setup_old_repo();
    let repo = temp.path();
    fs::write(
        repo.join(".maestro/tasks/tasks.jsonl"),
        "{\"id\":\"../../pwn\",\"title\":\"Bad task\",\"state\":\"ready\"}\n",
    )
    .expect("invariant: old task should be writable");

    let output = maestro(repo, &["migrate", "--check"]);
    assert_failure(&output, &["migrate", "--check"]);
    assert!(stderr(&output).contains("invalid migrated task id"));
    assert!(!repo.join("pwn-bad-task").exists());
}

#[test]
fn migrate_rolls_back_written_files_when_later_write_fails() {
    let temp = setup_old_repo();
    let repo = temp.path();
    fs::write(repo.join(".maestro/raw"), "not a directory")
        .expect("invariant: blocking raw path should write");

    let output = maestro(repo, &["migrate"]);
    assert_failure(&output, &["migrate"]);
    assert!(!repo
        .join(".maestro/tasks/task-001-migrate-old-task/task.yaml")
        .exists());
    assert!(!repo
        .join(".maestro/tasks/task-001-migrate-old-task/acceptance.yaml")
        .exists());
}

#[cfg(unix)]
#[test]
fn migrate_refuses_symlinked_target_roots() {
    let temp = setup_old_repo();
    let repo = temp.path();
    let external = TestTempDir::new("maestro-migrate-external-runs");
    fs::remove_dir_all(repo.join(".maestro/runs")).ok();
    unix_fs::symlink(external.path(), repo.join(".maestro/runs"))
        .expect("invariant: symlink should be creatable");

    let output = maestro(repo, &["migrate", "--check"]);
    assert_failure(&output, &["migrate", "--check"]);
    assert!(stderr(&output).contains("symlink"));
}

#[cfg(unix)]
#[test]
fn migrate_refuses_symlinked_source_roots() {
    let temp = setup_old_repo();
    let repo = temp.path();
    let external = TestTempDir::new("maestro-migrate-external-missions");
    fs::write(external.path().join("secret.txt"), "secret").expect("invariant: external secret");
    fs::remove_dir_all(repo.join(".maestro/missions")).ok();
    unix_fs::symlink(external.path(), repo.join(".maestro/missions"))
        .expect("invariant: symlink should be creatable");

    let output = maestro(repo, &["migrate", "--check"]);
    assert_failure(&output, &["migrate", "--check"]);
    assert!(stderr(&output).contains("symlink"));
    assert!(!stdout(&output).contains("secret"));
}
