use maestro::domain::task::{AcceptanceFile, TaskRecord, task_markdown};

#[test]
fn task_markdown_renders_title_and_inline_acceptance() {
    let mut task = TaskRecord::draft("task-003", "Add CSV export", "2026-05-25T08:00:00Z");
    task.acceptance = AcceptanceFile::new(
        "task-003",
        vec![
            "CSV export button appears on billing page".to_string(),
            "Clicking export downloads a .csv file".to_string(),
        ],
    );

    let expected_task_md = "# Add CSV export\n\n## Acceptance\n- CSV export button appears on billing page\n- Clicking export downloads a .csv file\n";
    assert_eq!(task_markdown(&task), expected_task_md);
}
