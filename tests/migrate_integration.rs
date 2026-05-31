mod support;

use std::fs;
use std::os::unix::fs as unix_fs;
use std::path::Path;
use std::process::Command;

use maestro::domain::{decisions, feature, harness, run, task};
use maestro::foundation::core::paths::MaestroPaths;
use maestro::foundation::core::schema::{
    ACCEPTANCE_SCHEMA_VERSION, FEATURE_SCHEMA_VERSION, HARNESS_SCHEMA_VERSION, TASK_SCHEMA_VERSION,
};

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
fn migrate_check_accepts_project_path_without_changing_cwd() {
    let temp = setup_old_repo();
    let repo = temp.path();
    let outside = TestTempDir::new("maestro-migrate-outside");

    let output = maestro(outside.path(), &["migrate", "--check", "--project"]);
    assert_failure(&output, &["migrate", "--check", "--project"]);

    let output = maestro(
        outside.path(),
        &[
            "migrate",
            "--check",
            "--project",
            repo.to_str().expect("invariant: temp path should be UTF-8"),
        ],
    );
    assert_success(&output, &["migrate", "--check", "--project", "<repo>"]);
    assert!(stdout(&output).contains("migration check:"));
    assert!(repo.join(".maestro/tasks/tasks.jsonl").exists());
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

    assert_migrated_artifacts_match_target_contracts(repo);
}

fn assert_migrated_artifacts_match_target_contracts(repo: &Path) {
    let paths = MaestroPaths::new(repo);

    let task = task::load_task_record(&paths.tasks_dir(), "task-001")
        .expect("invariant: migrated Task should load through Task facade");
    assert_eq!(task.schema_version, TASK_SCHEMA_VERSION);
    assert_eq!(task.id, "task-001");
    assert_eq!(task.feature_id.as_deref(), Some("feat-one"));
    assert_eq!(task.verification.verified_commit.as_deref(), Some("abc123"));

    let task_dir = paths.tasks_dir().join(task.directory_name());
    let task_md_raw = fs::read_to_string(task_dir.join("task.md"))
        .expect("invariant: migrated task.md should be readable");
    assert_eq!(
        task_md_raw,
        "# Migrate old task\n\n## Acceptance\nSee acceptance.yaml.\n"
    );
    assert_eq!(task_md_raw, task::task_markdown(&task));

    let acceptance_raw = fs::read_to_string(task_dir.join("acceptance.yaml"))
        .expect("invariant: migrated acceptance should be readable");
    let acceptance: task::AcceptanceFile = serde_yaml::from_str(&acceptance_raw)
        .expect("invariant: migrated acceptance should parse through Task contract");
    assert_eq!(acceptance.schema_version, ACCEPTANCE_SCHEMA_VERSION);
    assert_eq!(acceptance.task, "task-001");

    // The legacy v0.106->v0.8 migration still emits the flat features.yaml
    // registry. Under the per-feature-directory rewrite that output is orphaned
    // legacy (the Feature domain reads .maestro/features/<id>/feature.yaml, not
    // the flat file), and the whole migrate path is removed in the clean-rewrite
    // deletion phase. Assert only migrate's still-true behavior: it writes the
    // flat registry with the expected shape.
    let features_raw = fs::read_to_string(paths.features_dir().join("features.yaml"))
        .expect("invariant: migrated features should be readable");
    let registry: serde_yaml::Value = serde_yaml::from_str(&features_raw)
        .expect("invariant: migrated features should parse as YAML");
    assert_eq!(
        registry["schema_version"].as_str(),
        Some(FEATURE_SCHEMA_VERSION)
    );
    let features = registry["features"]
        .as_sequence()
        .expect("invariant: migrated registry should carry a features sequence");
    assert_eq!(features.len(), 1);
    assert_eq!(features[0]["id"].as_str(), Some("feat-one"));
    let counts = feature::query::count_tasks_for_feature(&paths.tasks_dir(), "feat-one")
        .expect("invariant: migrated feature rollup should read Task projections");
    assert_eq!(counts.total, 1);

    let decisions = decisions::query::decision_entries(&paths.decisions_dir())
        .expect("invariant: migrated decisions should list through Decision query");
    assert_eq!(decisions.len(), 1);
    assert_eq!(decisions[0].file_name, "decision-001-old-choice.md");
    let decision_path =
        decisions::query::resolve_decision_path(&paths.decisions_dir(), "decision-001")
            .expect("invariant: migrated decision should resolve through Decision query");
    assert_eq!(
        decision_path,
        paths.decisions_dir().join("decision-001-old-choice.md")
    );

    let harness_raw = fs::read_to_string(paths.harness_dir().join("harness.yml"))
        .expect("invariant: migrated Harness should be readable");
    let harness: harness::HarnessConfig = serde_yaml::from_str(&harness_raw)
        .expect("invariant: migrated Harness should parse through Harness schema");
    assert_eq!(harness.schema_version, HARNESS_SCHEMA_VERSION);
    assert_eq!(harness.stack.verify, vec!["cargo test".to_string()]);

    let run_logs = run::managed_event_logs(&paths)
        .expect("invariant: migrated Run logs should list through Run read model");
    assert_eq!(run_logs.len(), 1);
    assert_eq!(run_logs[0].session_id(), "session-1");
    assert_eq!(
        run_logs[0].path(),
        paths
            .runs_dir()
            .join("migrated/session-1/events.jsonl")
            .as_path()
    );
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
fn migrate_normalizes_legacy_features_for_current_readers() {
    let temp = setup_old_repo();
    let repo = temp.path();
    fs::write(
        repo.join(".maestro/features/features.yaml"),
        "features:\n  - id: legacy-feature\n    title: Legacy Feature\n    status: draft\n    tasks: [task-001]\n",
    )
    .expect("invariant: legacy features should be writable");

    let output = maestro(repo, &["migrate"]);
    assert_success(&output, &["migrate"]);
    let features = fs::read_to_string(repo.join(".maestro/features/features.yaml"))
        .expect("features readable");
    assert!(features.contains("schema_version: maestro.feature.v1"));
    assert!(features.contains("status: proposed"));
    assert!(!features.contains("status: draft"));
    assert!(features.contains("created_at: '0'") || features.contains("created_at: 0"));
    assert!(!features.contains("tasks:"));

    let snapshot = maestro(repo, &["watch", "snapshot"]);
    assert_success(&snapshot, &["watch", "snapshot"]);
    let init = maestro(repo, &["init", "--merge", "--yes"]);
    assert_success(&init, &["init", "--merge", "--yes"]);
    let snapshot = maestro(repo, &["watch", "snapshot"]);
    assert_success(&snapshot, &["watch", "snapshot"]);
}

#[test]
fn migrate_moves_root_legacy_features_into_v1_location() {
    let temp = setup_old_repo();
    let repo = temp.path();
    fs::remove_file(repo.join(".maestro/features/features.yaml"))
        .expect("invariant: nested features should be removable");
    fs::write(
        repo.join(".maestro/features.yaml"),
        "features:\n  - id: root-feature\n    title: Root Feature\n    tasks: [task-001]\n",
    )
    .expect("invariant: root legacy features should be writable");

    let output = maestro(repo, &["migrate"]);
    assert_success(&output, &["migrate"]);
    let features = fs::read_to_string(repo.join(".maestro/features/features.yaml"))
        .expect("features readable");
    assert!(features.contains("schema_version: maestro.feature.v1"));
    assert!(features.contains("status: proposed"));
    assert!(!features.contains("tasks:"));
    assert!(repo
        .join(".maestro/raw/archived/features/features.yaml")
        .exists());
    assert!(repo
        .join(".maestro/archive/features/features.yaml")
        .exists());
    assert!(!repo.join(".maestro/features.yaml").exists());
}

#[test]
fn migrate_wraps_single_legacy_feature_mapping_into_registry() {
    let temp = setup_old_repo();
    let repo = temp.path();
    fs::remove_file(repo.join(".maestro/features/features.yaml"))
        .expect("invariant: nested features should be removable");
    fs::write(
        repo.join(".maestro/features.yaml"),
        "id: single-feature\ntitle: Single Feature\nstatus: draft\ntasks: [task-001]\n",
    )
    .expect("invariant: root legacy feature should be writable");

    let output = maestro(repo, &["migrate"]);
    assert_success(&output, &["migrate"]);
    let features = fs::read_to_string(repo.join(".maestro/features/features.yaml"))
        .expect("features readable");
    assert!(features.contains("schema_version: maestro.feature.v1"));
    assert!(features.contains("features:"));
    assert!(features.contains("id: single-feature"));
    assert!(features.contains("status: proposed"));
    assert!(!features.contains("status: draft"));
    assert!(!features.contains("tasks:"));

    let snapshot = maestro(repo, &["watch", "snapshot"]);
    assert_success(&snapshot, &["watch", "snapshot"]);
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
