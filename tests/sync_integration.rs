mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use maestro::foundation::core::paths::MaestroPaths;
use support::TestTempDir;

fn maestro(args: &[&str], cwd: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .current_dir(cwd)
        .env("HOME", cwd.join("home"))
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

fn install_claude(repo: &Path) {
    let output = maestro(&["install", "--agent", "claude"], repo);
    assert!(
        output.status.success(),
        "install --agent claude failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn record_sh(paths: &MaestroPaths) -> PathBuf {
    paths.hooks_dir().join("record.sh")
}

/// Locate the record.sh backup sync wrote. Sync shares the Update extraction
/// mode, so its backup directories carry the `-update` suffix.
fn sync_backup_for_hook(paths: &MaestroPaths) -> PathBuf {
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
            .join("hooks")
            .join("record.sh");
        if candidate.exists() {
            return candidate;
        }
    }
    panic!("expected a sync backup for record.sh");
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
    let record = record_sh(&paths);

    // Overwrite with a versionless script: the Update gate reads no
    // `# maestro:hook-version:` marker and treats it as drift.
    fs::write(&record, "edited hook script\n").expect("invariant: hook script should be writable");

    let output = maestro(&["sync", "--dry-run"], temp.path());
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("maestro sync would resync:"), "{stdout}");
    assert!(stdout.contains("refresh  record.sh"), "{stdout}");
    assert!(stdout.contains("skip     HARNESS.md"), "{stdout}");

    // Dry-run wrote nothing: the edit stands and no backup directory exists.
    assert_eq!(
        fs::read_to_string(&record).expect("invariant: hook script should be readable"),
        "edited hook script\n"
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
    let record = record_sh(&paths);
    // The freshly extracted script is the bundled content sync restores to (the
    // binary path is already pinned in, so a re-extract reproduces it exactly).
    let bundled = fs::read_to_string(&record).expect("invariant: hook script should be readable");
    fs::write(&record, "edited hook script\n").expect("invariant: hook script should be writable");

    let output = maestro(&["sync"], temp.path());
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("refresh  record.sh"), "{stdout}");
    assert!(stdout.contains("1 refreshed"), "{stdout}");
    assert!(stdout.contains("edited files backed up"), "{stdout}");

    // The drifted file is restored to the bundled content...
    assert_eq!(
        fs::read_to_string(&record).expect("invariant: hook script should be readable"),
        bundled
    );
    // ...and the local edit survives in the backup.
    let backup = sync_backup_for_hook(&paths);
    assert_eq!(
        fs::read_to_string(backup).expect("invariant: backup should be readable"),
        "edited hook script\n"
    );
}

#[test]
fn sync_preserves_a_local_edit_when_the_version_matches() {
    let temp = TestTempDir::new("maestro-sync-test");
    init_git_marker(temp.path());
    init(temp.path());
    let paths = MaestroPaths::new(temp.path());
    let record = record_sh(&paths);

    // Append a comment while keeping the `# maestro:hook-version:` marker intact:
    // the Update gate sees a matching version and preserves the edit.
    let bundled = fs::read_to_string(&record).expect("invariant: hook script should be readable");
    let edited = format!("{bundled}\n# local note\n");
    fs::write(&record, &edited).expect("invariant: hook script should be writable");

    let output = maestro(&["sync"], temp.path());
    assert!(output.status.success());
    assert_eq!(
        fs::read_to_string(&record).expect("invariant: hook script should be readable"),
        edited,
        "sync must preserve an edit whose version still matches"
    );
}

/// True when sync wrote a mirror-block backup (operation suffix `-sync`).
fn sync_mirror_backup_exists(paths: &MaestroPaths, relative_path: &str) -> bool {
    let Ok(entries) = fs::read_dir(paths.backups_dir()) else {
        return false;
    };
    for entry in entries {
        let entry = entry.expect("invariant: backup entry should be readable");
        let name = entry.file_name();
        let name = name
            .to_str()
            .expect("invariant: backup dir name should be UTF-8");
        if name.ends_with("-sync") && entry.path().join(relative_path).exists() {
            return true;
        }
    }
    false
}

#[test]
fn sync_resyncs_a_drifted_managed_mirror_block_and_preserves_user_content() {
    let temp = TestTempDir::new("maestro-sync-test");
    init_git_marker(temp.path());
    init(temp.path());
    install_claude(temp.path());
    let claude = temp.path().join("CLAUDE.md");

    // Drift the managed block body and wrap it in user-owned content on both sides.
    fs::write(
        &claude,
        "# My notes\n\n<!-- maestro:start -->\nstale content\n<!-- maestro:end -->\n\nkeep me\n",
    )
    .expect("invariant: CLAUDE.md should be writable");

    let output = maestro(&["sync"], temp.path());
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("mirror blocks resynced:"), "{stdout}");
    assert!(stdout.contains("CLAUDE.md"), "{stdout}");

    let restored = fs::read_to_string(&claude).expect("invariant: CLAUDE.md should be readable");
    // The block is restored to shipped content...
    assert!(
        restored.contains("@.maestro/harness/HARNESS.md"),
        "block restored: {restored}"
    );
    assert!(
        !restored.contains("stale content"),
        "drift gone: {restored}"
    );
    // ...and the user content outside the markers survives.
    assert!(restored.starts_with("# My notes"), "{restored}");
    assert!(restored.contains("keep me"), "{restored}");
    // The pre-sync copy is backed up.
    let paths = MaestroPaths::new(temp.path());
    assert!(
        sync_mirror_backup_exists(&paths, "CLAUDE.md"),
        "a drifted mirror block should be backed up under .maestro/backups/<ts>-sync/"
    );
}

#[test]
fn sync_leaves_a_freshly_installed_mirror_block_untouched() {
    let temp = TestTempDir::new("maestro-sync-test");
    init_git_marker(temp.path());
    init(temp.path());
    install_claude(temp.path());
    let claude = temp.path().join("CLAUDE.md");
    let agents = temp.path().join("AGENTS.md");
    let claude_before =
        fs::read_to_string(&claude).expect("invariant: CLAUDE.md should be readable");
    let agents_before =
        fs::read_to_string(&agents).expect("invariant: AGENTS.md should be readable");

    // This feature does not change the block bodies, only HARNESS.md, so on a
    // freshly installed repo sync must be a pure no-op for the mirror blocks.
    let output = maestro(&["sync"], temp.path());
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("mirror blocks resynced:"),
        "a matching block must not be reported as resynced: {stdout}"
    );

    assert_eq!(
        fs::read_to_string(&claude).expect("invariant: CLAUDE.md should be readable"),
        claude_before,
        "sync must not rewrite a matching CLAUDE.md block"
    );
    assert_eq!(
        fs::read_to_string(&agents).expect("invariant: AGENTS.md should be readable"),
        agents_before
    );
    let paths = MaestroPaths::new(temp.path());
    assert!(
        !sync_mirror_backup_exists(&paths, "CLAUDE.md"),
        "a no-op sync must not back up the mirror block"
    );
}

#[test]
fn sync_skips_a_mirror_file_without_the_markers() {
    let temp = TestTempDir::new("maestro-sync-test");
    init_git_marker(temp.path());
    init(temp.path());
    install_claude(temp.path());
    let claude = temp.path().join("CLAUDE.md");

    // The user removed the managed block entirely; sync must not re-add it.
    fs::write(&claude, "# Just my own notes, no maestro block\n")
        .expect("invariant: CLAUDE.md should be writable");

    let output = maestro(&["sync"], temp.path());
    assert!(output.status.success());
    assert_eq!(
        fs::read_to_string(&claude).expect("invariant: CLAUDE.md should be readable"),
        "# Just my own notes, no maestro block\n",
        "sync only refreshes blocks that already exist"
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
