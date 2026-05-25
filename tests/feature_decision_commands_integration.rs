mod support;

use std::fs;
use std::os::unix::fs as unix_fs;
use std::path::Path;
use std::process::Command;

use maestro::core::fs::ensure_dir;
use maestro::decisions::template::decision_markdown;
use support::TestTempDir;

fn maestro(args: &[&str], cwd: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("invariant: compiled maestro binary should be runnable in integration tests")
}

fn init_git_marker(repo: &Path) {
    fs::create_dir(repo.join(".git")).expect("invariant: .git marker should be creatable");
}

fn stdout(output: std::process::Output, args: &[&str]) -> String {
    assert!(
        output.status.success(),
        "maestro {:?} failed\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout).expect("invariant: stdout should be UTF-8")
}

fn assert_failure(output: std::process::Output, args: &[&str]) -> String {
    assert!(
        !output.status.success(),
        "maestro {:?} unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stderr).expect("invariant: stderr should be UTF-8")
}

#[test]
fn feature_lifecycle_views_compute_task_counts_from_task_yaml() {
    let temp_dir = TestTempDir::new("maestro-feature-command-test");
    init_git_marker(temp_dir.path());
    stdout(
        maestro(&["init", "--yes"], temp_dir.path()),
        &["init", "--yes"],
    );

    let create_output = stdout(
        maestro(&["feature", "new", "Billing CSV export"], temp_dir.path()),
        &["feature", "new", "Billing CSV export"],
    );
    assert!(create_output.contains("created feature billing-csv-export"));

    let tasks_dir = temp_dir.path().join(".maestro/tasks");
    write_task(&tasks_dir, "task-001", "billing-csv-export", "verified");
    write_task(&tasks_dir, "task-002", "billing-csv-export", "ready");
    write_task(&tasks_dir, "task-003", "other-feature", "verified");

    let show_output = stdout(
        maestro(&["feature", "show", "billing-csv-export"], temp_dir.path()),
        &["feature", "show", "billing-csv-export"],
    );
    assert!(show_output.contains("id: billing-csv-export"));
    assert!(show_output.contains("status: proposed"));
    assert!(show_output.contains("tasks_total: 2"));
    assert!(show_output.contains("tasks_verified: 1"));

    stdout(
        maestro(&["feature", "edit", "billing-csv-export"], temp_dir.path()),
        &["feature", "edit", "billing-csv-export"],
    );
    let show_after_edit = stdout(
        maestro(&["feature", "show", "billing-csv-export"], temp_dir.path()),
        &["feature", "show", "billing-csv-export"],
    );
    assert!(show_after_edit.contains("status: in_progress"));

    stdout(
        maestro(&["feature", "ship", "billing-csv-export"], temp_dir.path()),
        &["feature", "ship", "billing-csv-export"],
    );
    let show_after_ship = stdout(
        maestro(&["feature", "show", "billing-csv-export"], temp_dir.path()),
        &["feature", "show", "billing-csv-export"],
    );
    assert!(show_after_ship.contains("status: shipped"));

    stdout(
        maestro(
            &["feature", "cancel", "billing-csv-export"],
            temp_dir.path(),
        ),
        &["feature", "cancel", "billing-csv-export"],
    );
    let list_output = stdout(
        maestro(&["feature", "list"], temp_dir.path()),
        &["feature", "list"],
    );
    assert!(list_output.contains("billing-csv-export"));
    assert!(list_output.contains("cancelled"));
    assert!(list_output.contains("tasks=2"));
    assert!(list_output.contains("verified=1"));
}

#[test]
fn decision_new_list_show_auto_increment_and_preserve_template() {
    let temp_dir = TestTempDir::new("maestro-decision-command-test");
    init_git_marker(temp_dir.path());
    stdout(
        maestro(&["init", "--yes"], temp_dir.path()),
        &["init", "--yes"],
    );

    let decisions_dir = temp_dir.path().join(".maestro/decisions");
    ensure_dir(&decisions_dir).expect("invariant: decisions directory should be creatable");
    fs::write(
        decisions_dir.join("decision-007-existing.md"),
        decision_markdown(7, "Existing"),
    )
    .expect("invariant: seed decision should be writable");

    let title = "Use single HARNESS.md instead of three adapter files";
    let new_output = stdout(
        maestro(&["decision", "new", title], temp_dir.path()),
        &["decision", "new", title],
    );
    assert!(new_output.contains("created decision decision-008"));

    let expected_file =
        decisions_dir.join("decision-008-use-single-harness-md-instead-of-three-adapter-files.md");
    assert!(expected_file.is_file());
    assert_eq!(
        fs::read_to_string(&expected_file).expect("invariant: decision file should be readable"),
        decision_markdown(8, title)
    );

    let list_output = stdout(
        maestro(&["decision", "list"], temp_dir.path()),
        &["decision", "list"],
    );
    assert!(list_output.contains("decision-007-existing"));
    assert!(
        list_output.contains("decision-008-use-single-harness-md-instead-of-three-adapter-files")
    );

    let show_output = stdout(
        maestro(
            &[
                "decision",
                "show",
                "decision-008-use-single-harness-md-instead-of-three-adapter-files",
            ],
            temp_dir.path(),
        ),
        &[
            "decision",
            "show",
            "decision-008-use-single-harness-md-instead-of-three-adapter-files",
        ],
    );
    assert_eq!(show_output, decision_markdown(8, title));
}

#[test]
fn decision_show_rejects_path_traversal_ids() {
    let temp_dir = TestTempDir::new("maestro-decision-command-traversal-test");
    init_git_marker(temp_dir.path());
    stdout(
        maestro(&["init", "--yes"], temp_dir.path()),
        &["init", "--yes"],
    );

    let stderr = assert_failure(
        maestro(&["decision", "show", "../outside.md"], temp_dir.path()),
        &["decision", "show", "../outside.md"],
    );
    assert!(stderr.contains("invalid decision id"));

    let decisions_dir = temp_dir.path().join(".maestro/decisions");
    let nested_dir = decisions_dir.join("nested");
    fs::create_dir(&nested_dir).expect("invariant: nested decisions dir should be creatable");
    fs::write(nested_dir.join("secret.md"), "secret\n")
        .expect("invariant: nested decision file should be writable");
    let nested_stderr = assert_failure(
        maestro(&["decision", "show", "nested/secret.md"], temp_dir.path()),
        &["decision", "show", "nested/secret.md"],
    );
    assert!(nested_stderr.contains("invalid decision id"));

    fs::write(temp_dir.path().join("outside.md"), "secret\n")
        .expect("invariant: outside file should be writable");
    unix_fs::symlink(
        temp_dir.path().join("outside.md"),
        decisions_dir.join("decision-001-leak.md"),
    )
    .expect("invariant: decision symlink should be creatable");
    let symlink_stderr = assert_failure(
        maestro(&["decision", "show", "decision-001-leak"], temp_dir.path()),
        &["decision", "show", "decision-001-leak"],
    );
    assert!(symlink_stderr.contains("not found"));
}

fn write_task(tasks_dir: &Path, id: &str, feature_id: &str, state: &str) {
    let task_dir = tasks_dir.join(id);
    ensure_dir(&task_dir).expect("invariant: task directory should be creatable");
    fs::write(
        task_dir.join("task.yaml"),
        format!("feature_id: {feature_id}\nstate: {state}\n"),
    )
    .expect("invariant: task yaml should be writable");
}
