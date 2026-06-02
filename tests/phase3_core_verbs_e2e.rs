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

fn stdout(output: std::process::Output) -> String {
    String::from_utf8(output.stdout).expect("invariant: stdout should be UTF-8")
}

#[test]
fn phase3_core_verbs_demo_path_runs_end_to_end() {
    let temp = TestTempDir::new("maestro-phase3-e2e");
    let repo = temp.path();
    fs::create_dir(repo.join(".git")).expect("invariant: .git marker should be creatable");

    run(repo, &["init", "--yes"]);
    run(repo, &["feature", "new", "Billing CSV export"]);
    run(
        repo,
        &[
            "task",
            "create",
            "Add CSV export",
            "--feature",
            "billing-csv-export",
        ],
    );
    run(repo, &["task", "explore", "task-001"]);
    run(repo, &["task", "accept", "task-001"]);
    run(repo, &["task", "claim", "task-001"]);
    run(
        repo,
        &[
            "task",
            "complete",
            "task-001",
            "--summary",
            "export shipped",
            "--claim",
            "implemented CSV export",
        ],
    );

    let run_dir = repo.join(".maestro/runs/run-001");
    fs::create_dir_all(&run_dir).expect("invariant: run dir should be creatable");
    fs::write(
        run_dir.join("events.jsonl"),
        "{\"task_id\":\"task-001\",\"kind\":\"proof\",\"message\":\"implemented CSV export\"}\n",
    )
    .expect("invariant: events should be writable");

    run(repo, &["task", "verify", "task-001"]);
    run(repo, &["decision", "new", "Use computed query views"]);

    let task_show = stdout(run(repo, &["task", "show", "task-001"]));
    assert!(task_show.contains("state: verified"));

    let feature_list = stdout(run(repo, &["feature", "list"]));
    assert!(feature_list.contains("billing-csv-export"));
    assert!(feature_list.contains("tasks=1"));
    assert!(feature_list.contains("verified=1"));

    let decision_list = stdout(run(repo, &["decision", "list"]));
    assert!(decision_list.contains("decision-001-use-computed-query-views.md"));

    let proof = stdout(run(repo, &["query", "proof", "task-001"]));
    assert!(proof.contains("proof task-001: accepted"));
    assert!(proof.contains("claims: 1/1"));

    let matrix = stdout(run(repo, &["query", "matrix"]));
    assert!(matrix.contains("billing-csv-export"));
    assert!(matrix.contains("task-001"));
    assert!(matrix.contains("accepted"));

    let shell_init = stdout(run_with_env(repo, &["shell-init"], "MAESTRO_SHELL", "bash"));
    assert!(shell_init.contains("export MAESTRO_CURRENT_TASK"));
    assert!(shell_init.contains("unset MAESTRO_CURRENT_TASK"));

    let doctor = stdout(run(repo, &["doctor"]));
    assert!(doctor.contains("doctor: ok"));
}

fn run(repo: &Path, args: &[&str]) -> std::process::Output {
    let output = maestro(repo, args);
    assert_success(&output, args);
    output
}

fn run_with_env(repo: &Path, args: &[&str], key: &str, value: &str) -> std::process::Output {
    let output = Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .current_dir(repo)
        .env(key, value)
        .output()
        .expect("invariant: compiled maestro binary should run in integration tests");
    assert_success(&output, args);
    output
}
