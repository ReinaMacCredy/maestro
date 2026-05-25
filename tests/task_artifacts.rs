mod support;

use maestro::task::template::{
    load_task, save_task_with_snapshot, write_task_artifacts, AcceptanceFile, ProofState,
    TaskRecord, VerificationBinding,
};
use support::TestTempDir;

#[test]
fn task_artifacts_write_v1_task_markdown_and_acceptance_files() {
    let temp_dir = TestTempDir::new("maestro-task-test");
    let tasks_dir = temp_dir.path().join(".maestro/tasks");
    let task = TaskRecord::draft("task-003", "Add CSV export", "2026-05-25T08:00:00Z");
    let acceptance = AcceptanceFile::new(
        "task-003",
        vec![
            "CSV export button appears on billing page".to_string(),
            "Clicking export downloads a .csv file".to_string(),
        ],
    );

    let task_dir = write_task_artifacts(&tasks_dir, &task, &acceptance)
        .expect("invariant: task artifacts should write");

    assert_eq!(task_dir, tasks_dir.join("task-003-add-csv-export"));
    let task_yaml = std::fs::read_to_string(task_dir.join("task.yaml"))
        .expect("invariant: task.yaml should be readable");
    let task_md = std::fs::read_to_string(task_dir.join("task.md"))
        .expect("invariant: task.md should be readable");
    let acceptance_yaml = std::fs::read_to_string(task_dir.join("acceptance.yaml"))
        .expect("invariant: acceptance.yaml should be readable");

    assert!(task_yaml.contains("schema_version: maestro.task.v1"));
    assert!(task_yaml.contains("state: draft"));
    assert!(task_yaml.contains("acceptance_locked: false"));
    assert!(task_yaml.contains("verification:"));
    assert!(task_md.contains("# Add CSV export"));
    assert!(acceptance_yaml.contains("schema_version: maestro.acceptance.v1"));
    assert!(acceptance_yaml.contains("task: task-003"));
}

#[test]
fn optimistic_concurrency_rejects_stale_updated_at_snapshot() {
    let temp_dir = TestTempDir::new("maestro-task-test");
    let tasks_dir = temp_dir.path().join(".maestro/tasks");
    let mut task = TaskRecord::draft("task-003", "Add CSV export", "2026-05-25T08:00:00Z");
    let acceptance = AcceptanceFile::new("task-003", Vec::new());
    let task_dir = write_task_artifacts(&tasks_dir, &task, &acceptance)
        .expect("invariant: task artifacts should write");
    let task_path = task_dir.join("task.yaml");
    let (_, snapshot) = load_task(&task_path).expect("invariant: task should load");

    task.updated_at = "2026-05-25T09:00:00Z".to_string();
    std::fs::write(
        &task_path,
        serde_yaml::to_string(&task).expect("invariant: task serializes"),
    )
    .expect("invariant: task.yaml should be writable");

    let error = save_task_with_snapshot(&task, &snapshot)
        .expect_err("invariant: stale snapshot should be rejected");

    assert!(error.to_string().contains("task was modified"));
}

#[test]
fn optimistic_concurrency_rejects_existing_save_lock() {
    let temp_dir = TestTempDir::new("maestro-task-test");
    let tasks_dir = temp_dir.path().join(".maestro/tasks");
    let task = TaskRecord::draft("task-003", "Add CSV export", "2026-05-25T08:00:00Z");
    let acceptance = AcceptanceFile::new("task-003", Vec::new());
    let task_dir = write_task_artifacts(&tasks_dir, &task, &acceptance)
        .expect("invariant: task artifacts should write");
    let task_path = task_dir.join("task.yaml");
    let (_, snapshot) = load_task(&task_path).expect("invariant: task should load");
    let lock_path = task_dir.join("task.yaml.lock");
    std::fs::write(&lock_path, "").expect("invariant: lock file should be writable");

    let error = save_task_with_snapshot(&task, &snapshot)
        .expect_err("invariant: locked task should be rejected");

    assert!(error.to_string().contains("task is locked"));
    std::fs::remove_file(lock_path).expect("invariant: lock file should be removable");
}

#[test]
fn proof_state_computes_missing_failed_accepted_and_stale() {
    let binding = VerificationBinding::default();
    assert_eq!(
        binding.proof_state(Some("abc"), Some("a"), Some("c"), false),
        ProofState::Missing
    );
    assert_eq!(
        binding.proof_state(Some("abc"), Some("a"), Some("c"), true),
        ProofState::Failed
    );

    let binding = VerificationBinding {
        verified_commit: Some("abc".to_string()),
        acceptance_hash: Some("a".to_string()),
        checks_hash: Some("c".to_string()),
        ..VerificationBinding::default()
    };
    assert_eq!(
        binding.proof_state(Some("abc"), Some("a"), Some("c"), false),
        ProofState::Accepted
    );
    assert_eq!(
        binding.proof_state(Some("def"), Some("a"), Some("c"), false),
        ProofState::Stale
    );
    assert_eq!(
        binding.proof_state(Some("abc"), Some("changed"), Some("c"), false),
        ProofState::Stale
    );
}
