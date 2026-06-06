mod support;

use maestro::domain::task::{AcceptanceFile, TaskRecord, task_markdown};
use maestro::task::template::{load_task, save_task_with_snapshot, write_task_artifacts};
use support::TestTempDir;

#[test]
fn task_artifacts_write_v2_task_markdown_and_inline_acceptance() {
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

    assert!(task_yaml.contains("schema_version: maestro.task.v2"));
    assert!(task_yaml.contains("state: draft"));
    assert!(task_yaml.contains("acceptance_locked: false"));
    assert!(task_yaml.contains("acceptance:"));
    assert!(task_yaml.contains("CSV export button appears on billing page"));
    assert!(task_yaml.contains("Clicking export downloads a .csv file"));
    assert!(task_yaml.contains("verification:"));
    assert!(
        !task_dir.join("acceptance.yaml").exists(),
        "acceptance sidecar should not be written for v2 tasks"
    );
    let expected_task_md = "# Add CSV export\n\n## Acceptance\n- CSV export button appears on billing page\n- Clicking export downloads a .csv file\n";
    assert_eq!(task_md, expected_task_md);
    let (loaded, _) = load_task(&task_dir.join("task.yaml")).expect("invariant: task should load");
    assert_eq!(task_markdown(&loaded), expected_task_md);
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
