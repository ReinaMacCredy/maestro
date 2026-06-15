//! An id lookup that fails while a near-match card exists must ride the
//! main.rs funnel: the not-found error keeps its text and exit code, plus one
//! `fix: did you mean <id>?` line. The id is never fuzzy-resolved.

mod support;

use std::fs;
use std::path::Path;
use std::process::Command;

use support::TestTempDir;

fn maestro(args: &[&str], cwd: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("invariant: compiled maestro binary should be runnable in integration tests")
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

fn init_repo(prefix: &str) -> TestTempDir {
    let temp_dir = TestTempDir::new(prefix);
    fs::create_dir(temp_dir.path().join(".git")).expect("invariant: .git marker creatable");
    stdout(
        maestro(&["init", "--yes"], temp_dir.path()),
        &["init", "--yes"],
    );
    temp_dir
}

/// Corrupt the trailing `-hex4` nonce so the id misses while staying one
/// plausible typo away from the real card.
fn wrong_hash(id: &str) -> String {
    let stem = &id[..id.len() - 4];
    let nonce = if id.ends_with("0000") { "1111" } else { "0000" };
    format!("{stem}{nonce}")
}

#[test]
fn decision_lock_with_a_wrong_hash_id_hints_the_near_match() {
    let repo = init_repo("maestro-dym-decision");
    let create = ["decision", "new", "Pick the parser", "--id-only"];
    let real = stdout(maestro(&create, repo.path()), &create);
    let real = real.trim();
    let wrong = wrong_hash(real);

    let args = [
        "decision",
        "lock",
        wrong.as_str(),
        "--decision",
        "use clap",
        "--rejected",
        "hand-rolled: too brittle",
    ];
    let output = maestro(&args, repo.path());
    assert_eq!(output.status.code(), Some(1), "exit code must stay 1");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(&format!("decision not found: {wrong}")),
        "{stderr}"
    );
    assert!(
        stderr.contains(&format!("fix: did you mean {real}?")),
        "{stderr}"
    );
}

#[test]
fn task_lookup_with_a_wrong_hash_id_hints_the_near_match() {
    let repo = init_repo("maestro-dym-task");
    let create = ["task", "create", "Wire the adapter", "--id-only"];
    let real = stdout(maestro(&create, repo.path()), &create);
    let real = real.trim();
    let wrong = wrong_hash(real);

    let args = ["task", "explore", wrong.as_str()];
    let output = maestro(&args, repo.path());
    assert_eq!(output.status.code(), Some(1), "exit code must stay 1");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(&format!("task not found: {wrong}")),
        "{stderr}"
    );
    assert!(
        stderr.contains(&format!("fix: did you mean {real}?")),
        "{stderr}"
    );
}

#[test]
fn a_lookup_with_no_near_match_stays_a_plain_not_found() {
    let repo = init_repo("maestro-dym-none");
    let create = ["decision", "new", "Pick the parser", "--id-only"];
    stdout(maestro(&create, repo.path()), &create);

    let args = [
        "decision",
        "lock",
        "dec-completely-unrelated-zzzz",
        "--decision",
        "x",
        "--rejected",
        "y: z",
    ];
    let output = maestro(&args, repo.path());
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("decision not found: dec-completely-unrelated-zzzz"),
        "{stderr}"
    );
    assert!(!stderr.contains("did you mean"), "{stderr}");
}
