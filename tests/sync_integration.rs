mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use maestro::domain::skills::catalog::skills;
use maestro::foundation::core::paths::MaestroPaths;
use support::TestTempDir;

fn maestro(args: &[&str], cwd: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("invariant: compiled maestro binary should be runnable in sync tests")
}

fn init_git_marker(repo: &Path) {
    fs::create_dir(repo.join(".git")).expect("invariant: .git marker should be creatable");
}

fn init(repo: &Path) {
    let output = maestro(&["init", "--yes"], repo);
    assert!(
        output.status.success(),
        "init --yes failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn task_skill_md(paths: &MaestroPaths) -> PathBuf {
    paths.skills_dir().join("maestro-task").join("SKILL.md")
}

fn bundled_task_skill_md() -> String {
    skills()
        .iter()
        .find(|skill| skill.name == "maestro-task")
        .expect("invariant: maestro-task should be bundled")
        .skill_md()
        .to_string()
}

/// Locate the SKILL.md backup sync wrote for `skill_name`. Sync shares the
/// Update extraction mode, so its backup directories carry the `-update` suffix.
fn sync_backup_for(paths: &MaestroPaths, skill_name: &str) -> PathBuf {
    for entry in fs::read_dir(paths.backups_dir()).expect("invariant: backups dir should exist") {
        let entry = entry.expect("invariant: backup entry should be readable");
        let name = entry.file_name();
        let name = name
            .to_str()
            .expect("invariant: backup dir name should be UTF-8");
        if !name.ends_with("-update") {
            continue;
        }
        let candidate = entry
            .path()
            .join(".maestro")
            .join("skills")
            .join(skill_name)
            .join("SKILL.md");
        if candidate.exists() {
            return candidate;
        }
    }
    panic!("expected a sync backup for {skill_name}");
}

#[test]
fn sync_reports_already_current_and_is_idempotent() {
    let temp = TestTempDir::new("maestro-sync-test");
    init_git_marker(temp.path());
    init(temp.path());

    let output = maestro(&["sync"], temp.path());
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("synced: 0 refreshed, 0 created,") && stdout.contains("already current"),
        "{stdout}"
    );

    // A second run is a safe no-op with the same summary and exit 0.
    let again = maestro(&["sync"], temp.path());
    assert!(again.status.success());
    assert!(String::from_utf8_lossy(&again.stdout).contains("synced: 0 refreshed, 0 created,"));
}

#[test]
fn sync_dry_run_previews_drift_without_writing() {
    let temp = TestTempDir::new("maestro-sync-test");
    init_git_marker(temp.path());
    init(temp.path());
    let paths = MaestroPaths::new(temp.path());
    let skill = task_skill_md(&paths);

    // Overwrite with a versionless file: the Update gate reads no version and
    // treats it as drift.
    fs::write(&skill, "edited bundled skill\n").expect("invariant: skill should be writable");

    let output = maestro(&["sync", "--dry-run"], temp.path());
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("maestro sync would resync:"), "{stdout}");
    assert!(stdout.contains("refresh  maestro-task"), "{stdout}");

    // Dry-run wrote nothing: the edit stands and no backup directory exists.
    assert_eq!(
        fs::read_to_string(&skill).expect("invariant: skill should be readable"),
        "edited bundled skill\n"
    );
    assert!(
        !paths.backups_dir().exists(),
        "dry-run must not create backups"
    );
}

#[test]
fn sync_refreshes_drifted_resource_and_backs_up_the_edit() {
    let temp = TestTempDir::new("maestro-sync-test");
    init_git_marker(temp.path());
    init(temp.path());
    let paths = MaestroPaths::new(temp.path());
    let skill = task_skill_md(&paths);
    fs::write(&skill, "edited bundled skill\n").expect("invariant: skill should be writable");

    let output = maestro(&["sync"], temp.path());
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("refresh  maestro-task"), "{stdout}");
    assert!(stdout.contains("1 refreshed"), "{stdout}");
    assert!(stdout.contains("edited files backed up"), "{stdout}");

    // The drifted file is restored to the bundled content...
    assert_eq!(
        fs::read_to_string(&skill).expect("invariant: skill should be readable"),
        bundled_task_skill_md()
    );
    // ...and the local edit survives in the backup.
    let backup = sync_backup_for(&paths, "maestro-task");
    assert_eq!(
        fs::read_to_string(backup).expect("invariant: backup should be readable"),
        "edited bundled skill\n"
    );
}

#[test]
fn sync_preserves_a_local_edit_when_the_version_matches() {
    let temp = TestTempDir::new("maestro-sync-test");
    init_git_marker(temp.path());
    init(temp.path());
    let paths = MaestroPaths::new(temp.path());
    let skill = task_skill_md(&paths);

    // Append to the body while keeping the version frontmatter intact: the
    // Update gate sees a matching version and preserves the edit.
    let edited = format!("{}\n<!-- local note -->\n", bundled_task_skill_md());
    fs::write(&skill, &edited).expect("invariant: skill should be writable");

    let output = maestro(&["sync"], temp.path());
    assert!(output.status.success());
    assert_eq!(
        fs::read_to_string(&skill).expect("invariant: skill should be readable"),
        edited,
        "sync must preserve an edit whose version still matches"
    );
}

#[test]
fn sync_requires_an_initialized_maestro() {
    let temp = TestTempDir::new("maestro-sync-test");
    init_git_marker(temp.path()); // a repo, but no `.maestro`

    let output = maestro(&["sync"], temp.path());
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("run `maestro init` first"));
}
