pub mod card_support;
mod support;

use std::fs;
use std::path::Path;
use std::process::Command;

use card_support::id_by_title;
use support::TestTempDir;

fn maestro(cwd: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .current_dir(cwd)
        // Pin the agent so the demo path is deterministic: the doctor handoff
        // hint derives from agent env vars, which differ between developer
        // sessions (CLAUDECODE) and clean CI environments.
        .env("MAESTRO_AGENT", "codex")
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
    run(repo, &["harness", "set", "--claims-only"]);
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
    // Creation mints an opaque content-hash id; recover it by the unique title.
    let id = id_by_title(repo, "Add CSV export");
    run(repo, &["task", "explore", &id]);
    run(repo, &["task", "accept", &id]);
    run(repo, &["task", "claim", &id]);
    run(
        repo,
        &[
            "task",
            "complete",
            &id,
            "--summary",
            "export shipped",
            "--claim",
            "implemented CSV export",
            "--proof",
            "implemented CSV export",
        ],
    );
    run(repo, &["decision", "new", "Use computed query views"]);
    let decision_id = id_by_title(repo, "Use computed query views");

    let task_show = stdout(run(repo, &["task", "show", &id]));
    assert!(task_show.contains("state: verified"));

    let feature_list = stdout(run(repo, &["feature", "list"]));
    assert!(feature_list.contains("billing-csv-export"));
    assert!(feature_list.contains("NEXT"));
    assert!(feature_list.contains("INSPECT"));
    assert!(feature_list.contains("maestro feature show billing-csv-export"));
    assert!(untabify(&feature_list).contains("\t1\t1\t"));

    let decision_list = stdout(run(repo, &["decision", "list"]));
    assert!(untabify(&decision_list).contains(&format!(
        "{decision_id}\topen\tglobal\tUse computed query views"
    )));

    let proof = stdout(run(repo, &["query", "proof", &id]));
    assert!(proof.contains(&format!("proof {id}: accepted")));
    assert!(proof.contains("claims: 1/1"));

    let matrix = stdout(run(repo, &["query", "matrix"]));
    assert!(matrix.contains("billing-csv-export"));
    assert!(matrix.contains(&id));
    assert!(matrix.contains("accepted"));

    let shell_init = stdout(run_with_env(repo, &["shell-init"], "MAESTRO_SHELL", "bash"));
    assert!(shell_init.contains("export MAESTRO_CURRENT_TASK"));
    assert!(shell_init.contains("unset MAESTRO_CURRENT_TASK"));

    let doctor = stdout(run(repo, &["doctor"]));
    assert!(doctor.contains("doctor: ok"));
    assert!(doctor.contains("next: maestro install --agent codex"));
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

/// Collapse aligned-table padding (runs of 2+ spaces) back to tabs so cell
/// assertions stay width-independent.
fn untabify(output: &str) -> String {
    output
        .lines()
        .map(|line| {
            line.split("  ")
                .map(str::trim)
                .filter(|cell| !cell.is_empty())
                .collect::<Vec<_>>()
                .join("\t")
        })
        .collect::<Vec<_>>()
        .join("\n")
}
