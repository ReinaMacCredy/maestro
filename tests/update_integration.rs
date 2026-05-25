mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Result};
use maestro::core::paths::MaestroPaths;
use maestro::skills::bundled::bundled_skills;
use maestro::update::{
    run_update_with_seams, BinaryReplacer, ChecksumVerifier, DownloadedBinary, UpdateDownloader,
    UpdateOptions,
};
use support::TestTempDir;

fn maestro(args: &[&str], cwd: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("invariant: maestro binary should run")
}

fn assert_success(output: &std::process::Output) {
    assert!(
        output.status.success(),
        "expected success\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn update_reextracts_bundled_skills_and_backs_up_edited_skill() {
    let temp_dir = TestTempDir::new("maestro-update-test");
    init_git_marker(temp_dir.path());
    let paths = MaestroPaths::new(temp_dir.path());
    assert_success(&maestro(&["init", "--yes"], temp_dir.path()));

    let skill = bundled_skills()
        .iter()
        .find(|skill| skill.name == "maestro-task")
        .expect("invariant: maestro-task should be bundled");
    let skill_path = paths.skills_dir().join(skill.name).join("SKILL.md");
    fs::write(&skill_path, "edited bundled skill\n")
        .expect("invariant: bundled skill should be editable");

    let update = maestro(&["update"], temp_dir.path());

    assert_success(&update);
    let stdout = String::from_utf8_lossy(&update.stdout);
    assert!(stdout.contains("binary update skipped"));
    assert!(stdout.contains("edited skills backed up"));
    assert_eq!(
        fs::read_to_string(&skill_path).expect("invariant: skill should be readable"),
        skill.contents
    );

    let backup = update_backup_for(&paths, skill.name);
    assert_eq!(
        fs::read_to_string(backup).expect("invariant: backup should be readable"),
        "edited bundled skill\n"
    );
}

#[test]
fn simulated_download_failure_preserves_existing_binary_file() {
    let temp_dir = TestTempDir::new("maestro-update-test");
    let paths = MaestroPaths::new(temp_dir.path());
    let executable_path = temp_dir.path().join("bin").join("maestro");
    fs::create_dir_all(
        executable_path
            .parent()
            .expect("invariant: executable path should have a parent"),
    )
    .expect("invariant: executable parent should be creatable");
    fs::write(&executable_path, "current binary\n")
        .expect("invariant: current binary should be writable");

    let error = run_update_with_seams(
        &UpdateOptions {
            paths: &paths,
            executable_path: &executable_path,
            backup_timestamp: "test",
        },
        &FailingDownloader,
        &NoopVerifier,
        &NoopReplacer,
    )
    .expect_err("invariant: failing downloader should fail update");

    assert!(error.to_string().contains("download failed"));
    assert_eq!(
        fs::read_to_string(executable_path)
            .expect("invariant: current binary should still be readable"),
        "current binary\n"
    );
}

#[test]
fn simulated_replace_failure_preserves_existing_binary_file() {
    let temp_dir = TestTempDir::new("maestro-update-test");
    let paths = MaestroPaths::new(temp_dir.path());
    let executable_path = temp_dir.path().join("bin").join("maestro");
    fs::create_dir_all(
        executable_path
            .parent()
            .expect("invariant: executable path should have a parent"),
    )
    .expect("invariant: executable parent should be creatable");
    fs::write(&executable_path, "current binary\n")
        .expect("invariant: current binary should be writable");

    let error = run_update_with_seams(
        &UpdateOptions {
            paths: &paths,
            executable_path: &executable_path,
            backup_timestamp: "test",
        },
        &CandidateDownloader,
        &NoopVerifier,
        &FailingReplacer,
    )
    .expect_err("invariant: failing replacer should fail update");

    assert!(error.to_string().contains("replace failed"));
    assert_eq!(
        fs::read_to_string(executable_path)
            .expect("invariant: current binary should still be readable"),
        "current binary\n"
    );
}

#[test]
fn schema_mismatch_reports_migrate_and_does_not_mutate_harness_files() {
    let temp_dir = TestTempDir::new("maestro-update-test");
    init_git_marker(temp_dir.path());
    let paths = MaestroPaths::new(temp_dir.path());
    assert_success(&maestro(&["init", "--yes"], temp_dir.path()));

    let harness_yml = paths.harness_dir().join("harness.yml");
    let backlog_yaml = paths.harness_dir().join("backlog.yaml");
    let features_yaml = paths.features_dir().join("features.yaml");
    fs::write(
        &harness_yml,
        "schema_version: maestro.harness.v0\nverify: []\n",
    )
    .expect("invariant: harness schema should be writable");
    let before_harness =
        fs::read_to_string(&harness_yml).expect("invariant: harness should be readable");
    let before_backlog =
        fs::read_to_string(&backlog_yaml).expect("invariant: backlog should be readable");
    let before_features =
        fs::read_to_string(&features_yaml).expect("invariant: features should be readable");

    let update = maestro(&["update"], temp_dir.path());

    assert_success(&update);
    let stdout = String::from_utf8_lossy(&update.stdout);
    assert!(stdout.contains("schema mismatch detected"));
    assert!(stdout.contains("maestro migrate"));
    assert_eq!(
        fs::read_to_string(&harness_yml).expect("invariant: harness should be readable"),
        before_harness
    );
    assert_eq!(
        fs::read_to_string(&backlog_yaml).expect("invariant: backlog should be readable"),
        before_backlog
    );
    assert_eq!(
        fs::read_to_string(&features_yaml).expect("invariant: features should be readable"),
        before_features
    );
}

fn init_git_marker(repo: &Path) {
    fs::create_dir(repo.join(".git")).expect("invariant: .git marker should be creatable");
}

fn update_backup_for(paths: &MaestroPaths, skill_name: &str) -> PathBuf {
    for entry in fs::read_dir(paths.backups_dir()).expect("invariant: backups dir should exist") {
        let entry = entry.expect("invariant: backup entry should be readable");
        let file_name = entry.file_name();
        let file_name = file_name
            .to_str()
            .expect("invariant: backup dir name should be UTF-8");
        if !file_name.ends_with("-update") {
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

    panic!("expected update backup for {skill_name}");
}

struct FailingDownloader;

impl UpdateDownloader for FailingDownloader {
    fn download(&self, _work_dir: &Path) -> Result<DownloadedBinary> {
        bail!("download failed")
    }
}

struct CandidateDownloader;

impl UpdateDownloader for CandidateDownloader {
    fn download(&self, work_dir: &Path) -> Result<DownloadedBinary> {
        let candidate = work_dir.join("candidate-maestro");
        fs::write(&candidate, "replacement binary\n")?;

        Ok(DownloadedBinary::Available(candidate))
    }
}

struct NoopVerifier;

impl ChecksumVerifier for NoopVerifier {
    fn verify(&self, _candidate: &Path) -> Result<()> {
        Ok(())
    }
}

struct NoopReplacer;

impl BinaryReplacer for NoopReplacer {
    fn replace(&self, _current: &Path, _candidate: &Path) -> Result<()> {
        Ok(())
    }
}

struct FailingReplacer;

impl BinaryReplacer for FailingReplacer {
    fn replace(&self, _current: &Path, _candidate: &Path) -> Result<()> {
        bail!("replace failed")
    }
}
