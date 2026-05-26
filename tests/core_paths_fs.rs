mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use maestro::foundation::core::error::MaestroError;
use maestro::foundation::core::fs::{ensure_dir, ensure_parent_dir, read_to_string_if_exists};
use maestro::foundation::core::paths::{discover_repo_root, discover_repo_root_from, MaestroPaths};
use maestro::foundation::core::safe_write::{write_atomic, write_string_atomic};
use support::TestTempDir;

#[derive(Debug)]
struct CurrentDirGuard {
    original: PathBuf,
}

impl CurrentDirGuard {
    fn change_to(path: &Path) -> Self {
        let original = std::env::current_dir()
            .expect("invariant: current directory should be readable before test change");
        std::env::set_current_dir(path)
            .expect("invariant: test should be able to change current directory");

        Self { original }
    }
}

impl Drop for CurrentDirGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.original);
    }
}

fn current_dir_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[test]
fn maestro_paths_construct_expected_artifact_dirs() {
    let temp_dir = TestTempDir::new("maestro-core-test");
    let paths = MaestroPaths::new(temp_dir.path().to_path_buf());

    assert_eq!(paths.repo_root(), temp_dir.path());
    assert_eq!(paths.maestro_dir(), temp_dir.path().join(".maestro"));
    assert_eq!(
        paths.harness_dir(),
        temp_dir.path().join(".maestro/harness")
    );
    assert_eq!(
        paths.features_dir(),
        temp_dir.path().join(".maestro/features")
    );
    assert_eq!(
        paths.decisions_dir(),
        temp_dir.path().join(".maestro/decisions")
    );
    assert_eq!(paths.skills_dir(), temp_dir.path().join(".maestro/skills"));
    assert_eq!(
        paths.backups_dir(),
        temp_dir.path().join(".maestro/backups")
    );
}

#[test]
fn discover_repo_root_from_explicit_directory_walks_ancestors() {
    let temp_dir = TestTempDir::new("maestro-core-test");
    let repo_root = temp_dir.path();
    ensure_dir(repo_root.join(".maestro")).expect("invariant: .maestro should be creatable");
    let nested_dir = repo_root.join("src/deep/nested");
    ensure_dir(&nested_dir).expect("invariant: nested test directory should be creatable");

    let discovered = discover_repo_root_from(&nested_dir)
        .expect("invariant: repo root should be discoverable from nested directory");

    assert_eq!(
        discovered,
        repo_root
            .canonicalize()
            .expect("invariant: temp repo root should be canonicalizable")
    );
}

#[test]
fn discover_repo_root_from_explicit_directory_supports_pre_init_git_repo() {
    let temp_dir = TestTempDir::new("maestro-core-test");
    let repo_root = temp_dir.path();
    ensure_dir(repo_root.join(".git")).expect("invariant: .git should be creatable");
    let nested_dir = repo_root.join("src/deep/nested");
    ensure_dir(&nested_dir).expect("invariant: nested test directory should be creatable");

    let discovered = discover_repo_root_from(&nested_dir)
        .expect("invariant: repo root should be discoverable before maestro init");

    assert_eq!(
        discovered,
        repo_root
            .canonicalize()
            .expect("invariant: temp repo root should be canonicalizable")
    );
}

#[test]
fn discover_repo_root_from_explicit_directory_supports_git_file_marker() {
    let temp_dir = TestTempDir::new("maestro-core-test");
    let repo_root = temp_dir.path();
    fs::write(repo_root.join(".git"), "gitdir: ../linked-worktree\n")
        .expect("invariant: .git file marker should be writable");
    let nested_dir = repo_root.join("src/deep/nested");
    ensure_dir(&nested_dir).expect("invariant: nested test directory should be creatable");

    let discovered = discover_repo_root_from(&nested_dir)
        .expect("invariant: repo root should be discoverable from .git file marker");

    assert_eq!(
        discovered,
        repo_root
            .canonicalize()
            .expect("invariant: temp repo root should be canonicalizable")
    );
}

#[test]
fn discover_repo_root_uses_current_working_directory() {
    let _lock = current_dir_lock()
        .lock()
        .expect("invariant: current directory lock should not be poisoned");
    let temp_dir = TestTempDir::new("maestro-core-test");
    let repo_root = temp_dir.path();
    ensure_dir(repo_root.join(".maestro")).expect("invariant: .maestro should be creatable");
    let nested_dir = repo_root.join("worktree/path");
    ensure_dir(&nested_dir).expect("invariant: nested test directory should be creatable");

    let _guard = CurrentDirGuard::change_to(&nested_dir);
    let discovered = discover_repo_root()
        .expect("invariant: repo root should be discoverable from current directory");

    assert_eq!(
        discovered,
        repo_root
            .canonicalize()
            .expect("invariant: temp repo root should be canonicalizable")
    );
}

#[test]
fn discover_repo_root_reports_missing_maestro_directory() {
    let temp_dir = TestTempDir::new("maestro-core-test");

    let error = discover_repo_root_from(temp_dir.path())
        .expect_err("invariant: root discovery should fail without .maestro");

    let typed_error = error
        .downcast_ref::<MaestroError>()
        .expect("invariant: root discovery should return a typed MaestroError");

    assert!(matches!(typed_error, MaestroError::RepoRootNotFound { .. }));
}

#[test]
fn fs_helpers_create_parent_dirs_and_read_optional_utf8_files() {
    let temp_dir = TestTempDir::new("maestro-core-test");
    let target = temp_dir.path().join("nested/parent/file.txt");

    ensure_parent_dir(&target).expect("invariant: parent directories should be creatable");
    write_string_atomic(&target, "hello\n").expect("invariant: atomic string write should succeed");

    let contents = read_to_string_if_exists(&target)
        .expect("invariant: written file should be readable")
        .expect("invariant: written file should exist");
    let missing = read_to_string_if_exists(temp_dir.path().join("missing.txt"))
        .expect("invariant: missing file should not be an error");

    assert_eq!(contents, "hello\n");
    assert_eq!(missing, None);
}

#[test]
fn safe_write_creates_parent_dirs_and_replaces_existing_file() {
    let temp_dir = TestTempDir::new("maestro-core-test");
    let target = temp_dir.path().join("data/state.yaml");

    write_atomic(&target, b"first\n").expect("invariant: first atomic write should succeed");
    write_string_atomic(&target, "second\n")
        .expect("invariant: replacement atomic write should succeed");

    let contents =
        fs::read_to_string(&target).expect("invariant: replaced file should be readable");
    assert_eq!(contents, "second\n");
}

#[test]
fn safe_write_does_not_leave_successful_temp_siblings() {
    let temp_dir = TestTempDir::new("maestro-core-test");
    let target = temp_dir.path().join("artifact.txt");

    write_string_atomic(&target, "content\n").expect("invariant: atomic write should succeed");

    let temp_siblings = fs::read_dir(temp_dir.path())
        .expect("invariant: temp directory should be readable")
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp."))
        .count();

    assert_eq!(temp_siblings, 0);
}
