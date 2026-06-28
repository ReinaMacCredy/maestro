//! `--id-only` on the card-creating verbs must print exactly the new card id
//! on stdout and nothing else, so scripts and agents can capture the id
//! without scraping the human handoff text.

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

/// Run a create verb with `--id-only`, assert stdout is exactly one bare id
/// line with the expected type prefix, and return the id.
fn create_id_only(repo: &Path, args: &[&str], id_prefix: &str) -> String {
    let out = stdout(maestro(args, repo), args);
    assert!(
        out.ends_with('\n') && out.lines().count() == 1,
        "--id-only must print exactly one line, got:\n{out}"
    );
    let id = out.trim_end_matches('\n');
    assert_eq!(id, id.trim(), "id line must carry no padding: {id:?}");
    assert!(
        id.starts_with(id_prefix),
        "expected a {id_prefix}* id, got {id}"
    );
    id.to_string()
}

#[test]
fn task_create_id_only_prints_only_a_resolvable_id() {
    let repo = init_repo("maestro-id-only-task");
    let id = create_id_only(
        repo.path(),
        &["task", "create", "Wire the adapter", "--id-only"],
        "task-",
    );
    let show = stdout(maestro(&["show", &id], repo.path()), &["show", &id]);
    assert!(show.contains("Wire the adapter"), "{show}");
}

#[test]
fn feature_new_id_only_prints_only_the_slug_id() {
    let repo = init_repo("maestro-id-only-feature");
    let args = ["feature", "new", "Alpha Feature", "--id-only"];
    let out = stdout(maestro(&args, repo.path()), &args);
    assert_eq!(out, "alpha-feature\n");
    let show = stdout(
        maestro(&["show", "alpha-feature"], repo.path()),
        &["show", "alpha-feature"],
    );
    assert!(show.contains("Alpha Feature"), "{show}");
}

#[test]
fn feature_new_id_only_still_applies_initial_contract_fields() {
    let repo = init_repo("maestro-id-only-feature-init");
    let args = [
        "feature",
        "new",
        "Quiet Feature",
        "--description",
        "the problem",
        "--question",
        "open one",
        "--id-only",
    ];
    let out = stdout(maestro(&args, repo.path()), &args);
    assert_eq!(out, "quiet-feature\n");
    let spec = stdout(
        maestro(&["feature", "spec", "quiet-feature"], repo.path()),
        &["feature", "spec", "quiet-feature"],
    );
    assert!(spec.contains("the problem"), "{spec}");
    assert!(spec.contains("open one"), "{spec}");
}

#[test]
fn decision_new_id_only_prints_only_the_id_for_open_and_locked() {
    let repo = init_repo("maestro-id-only-decision");

    let open_id = create_id_only(
        repo.path(),
        &["decision", "new", "Pick the parser", "--id-only"],
        "dec-",
    );
    let show = stdout(
        maestro(&["decision", "show", &open_id], repo.path()),
        &["decision", "show", &open_id],
    );
    assert!(show.contains("status: open"), "{show}");

    let locked_id = create_id_only(
        repo.path(),
        &[
            "decision",
            "new",
            "Adopt the funnel",
            "--lock",
            "--decision",
            "route through main.rs",
            "--id-only",
        ],
        "dec-",
    );
    let show = stdout(
        maestro(&["decision", "show", &locked_id], repo.path()),
        &["decision", "show", &locked_id],
    );
    assert!(show.contains("status: locked"), "{show}");
}

#[test]
fn decision_supersede_id_only_prints_only_the_replacement_id() {
    let repo = init_repo("maestro-id-only-decision-supersede");

    let old_id = create_id_only(
        repo.path(),
        &[
            "decision",
            "new",
            "Original ruling",
            "--lock",
            "--decision",
            "keep the original",
            "--id-only",
        ],
        "dec-",
    );
    let new_id = create_id_only(
        repo.path(),
        &[
            "decision",
            "supersede",
            &old_id,
            "--decision",
            "replace the original",
            "--reason",
            "new evidence",
            "--title",
            "Replacement ruling",
            "--id-only",
        ],
        "dec-",
    );
    let show = stdout(
        maestro(&["decision", "show", &new_id], repo.path()),
        &["decision", "show", &new_id],
    );
    assert!(show.contains("status: locked"), "{show}");
    assert!(show.contains(&format!("- {old_id}")), "{show}");
}

#[test]
fn flat_create_id_only_prints_only_a_resolvable_id() {
    let repo = init_repo("maestro-id-only-create");
    let id = create_id_only(
        repo.path(),
        &["create", "-t", "idea", "Faster sync", "--id-only"],
        "idea-",
    );
    let show = stdout(maestro(&["show", &id], repo.path()), &["show", &id]);
    assert!(show.contains("Faster sync"), "{show}");
}
