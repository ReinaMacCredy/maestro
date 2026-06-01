mod support;

use std::fs;

use git2::{Repository, Signature};
use maestro::foundation::core::backup::backup_file_with_timestamp;
use maestro::foundation::core::diff::unified_diff;
use maestro::foundation::core::error::MaestroError;
use maestro::foundation::core::git::{dirty, head, snapshot};
use maestro::foundation::core::paths::MaestroPaths;
use support::TestTempDir;

#[test]
fn backup_file_preserves_repo_relative_path_under_operation_directory() {
    let temp_dir = TestTempDir::new("maestro-git-test");
    let paths = MaestroPaths::new(temp_dir.path().to_path_buf());
    let source = temp_dir.path().join("CLAUDE.md");
    fs::write(&source, "user content\n").expect("invariant: source file should be writable");

    let backup = backup_file_with_timestamp(&paths, &source, "install", "12345")
        .expect("invariant: backup should succeed");

    assert_eq!(
        backup,
        temp_dir
            .path()
            .join(".maestro/backups/12345-install/CLAUDE.md")
    );
    assert_eq!(
        fs::read_to_string(&backup).expect("invariant: backup should be readable"),
        "user content\n"
    );
}

#[test]
fn backup_file_rejects_unsafe_operation_path_segments() {
    let temp_dir = TestTempDir::new("maestro-git-test");
    let paths = MaestroPaths::new(temp_dir.path().to_path_buf());
    let source = temp_dir.path().join("CLAUDE.md");
    fs::write(&source, "user content\n").expect("invariant: source file should be writable");

    let error = backup_file_with_timestamp(&paths, &source, "../install", "12345")
        .expect_err("invariant: unsafe operation should fail");
    let typed_error = error
        .downcast_ref::<MaestroError>()
        .expect("invariant: backup should return a typed MaestroError");

    assert!(matches!(
        typed_error,
        MaestroError::InvalidOperationName { .. }
    ));
}

#[test]
fn backup_file_rejects_sources_outside_repo_root() {
    let temp_dir = TestTempDir::new("maestro-git-test");
    let outside_dir = TestTempDir::new("maestro-git-outside-test");
    let paths = MaestroPaths::new(temp_dir.path().to_path_buf());
    let source = outside_dir.path().join("CLAUDE.md");
    fs::write(&source, "user content\n").expect("invariant: source file should be writable");

    let error = backup_file_with_timestamp(&paths, &source, "install", "12345")
        .expect_err("invariant: outside-repo source should fail");
    let typed_error = error
        .downcast_ref::<MaestroError>()
        .expect("invariant: backup should return a typed MaestroError");

    assert!(matches!(
        typed_error,
        MaestroError::OutsideRepository { .. }
    ));
}

#[cfg(unix)]
#[test]
fn backup_file_rejects_symlinked_backup_directory() {
    let temp_dir = TestTempDir::new("maestro-git-test");
    let outside_dir = TestTempDir::new("maestro-git-outside-test");
    let paths = MaestroPaths::new(temp_dir.path().to_path_buf());
    let source = temp_dir.path().join("CLAUDE.md");
    fs::write(&source, "user content\n").expect("invariant: source file should be writable");
    fs::create_dir_all(temp_dir.path().join(".maestro"))
        .expect("invariant: .maestro should be creatable");
    std::os::unix::fs::symlink(outside_dir.path(), temp_dir.path().join(".maestro/backups"))
        .expect("invariant: symlink should be creatable");

    let error = backup_file_with_timestamp(&paths, &source, "install", "12345")
        .expect_err("invariant: symlinked backup directory should fail");
    let typed_error = error
        .downcast_ref::<MaestroError>()
        .expect("invariant: backup should return a typed MaestroError");

    assert!(matches!(
        typed_error,
        MaestroError::BackupPathContainsSymlink { .. }
    ));
}

#[test]
fn backup_file_does_not_overwrite_existing_backup_destination() {
    let temp_dir = TestTempDir::new("maestro-git-test");
    let paths = MaestroPaths::new(temp_dir.path().to_path_buf());
    let source = temp_dir.path().join("CLAUDE.md");
    fs::write(&source, "first\n").expect("invariant: source file should be writable");
    let backup = backup_file_with_timestamp(&paths, &source, "install", "12345")
        .expect("invariant: initial backup should succeed");

    fs::write(&source, "second\n").expect("invariant: source file should be writable");
    let error = backup_file_with_timestamp(&paths, &source, "install", "12345")
        .expect_err("invariant: repeated backup path should fail");

    assert!(
        format!("{error:#}").contains("failed to create"),
        "unexpected error: {error:#}"
    );
    assert_eq!(
        fs::read_to_string(&backup).expect("invariant: first backup should remain readable"),
        "first\n"
    );
}

#[test]
fn unified_diff_renders_before_after_changes() {
    let diff = unified_diff("HARNESS.md", "one\ntwo\n", "one\nthree\n");

    assert!(diff.contains("--- a/HARNESS.md"));
    assert!(diff.contains("+++ b/HARNESS.md"));
    assert!(diff.contains("-two\n"));
    assert!(diff.contains("+three\n"));
}

#[test]
fn git_snapshot_reports_unborn_clean_repo() {
    let temp_dir = TestTempDir::new("maestro-git-test");
    Repository::init(temp_dir.path()).expect("invariant: git repo should initialize");

    let snapshot = snapshot(temp_dir.path()).expect("invariant: git snapshot should load");

    assert_eq!(snapshot.head, None);
    assert!(!snapshot.dirty);
}

#[test]
fn git_helpers_report_head_and_dirty_state() {
    let temp_dir = TestTempDir::new("maestro-git-test");
    let repository =
        Repository::init(temp_dir.path()).expect("invariant: git repo should initialize");
    fs::write(temp_dir.path().join("tracked.txt"), "first\n")
        .expect("invariant: tracked file should be writable");
    commit_all(&repository, "initial");

    assert!(
        head(temp_dir.path())
            .expect("invariant: git head should load")
            .is_some()
    );
    assert!(!dirty(temp_dir.path()).expect("invariant: git status should load"));

    fs::write(temp_dir.path().join("untracked.txt"), "second\n")
        .expect("invariant: untracked file should be writable");

    assert!(dirty(temp_dir.path()).expect("invariant: git status should load"));
}

fn commit_all(repository: &Repository, message: &str) {
    let mut index = repository
        .index()
        .expect("invariant: git index should be readable");
    index
        .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .expect("invariant: git index add should succeed");
    index
        .write()
        .expect("invariant: git index write should succeed");
    let tree_id = index
        .write_tree()
        .expect("invariant: git tree write should succeed");
    let tree = repository
        .find_tree(tree_id)
        .expect("invariant: git tree should exist");
    let signature = Signature::now("Maestro Test", "maestro@example.test")
        .expect("invariant: git signature should be constructable");
    repository
        .commit(Some("HEAD"), &signature, &signature, message, &tree, &[])
        .expect("invariant: git commit should succeed");
}
