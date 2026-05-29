mod support;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs};

use anyhow::{bail, Result};
use maestro::core::paths::MaestroPaths;
use maestro::operations::update::{
    detect_schema_mismatches, run_update_with_seams, AtomicBinaryReplacer, BinaryReplacer,
    ChecksumVerifier, DownloadedBinary, ReleaseInfo, UpdateDownloader, UpdateOptions,
    UpdateRequest,
};
use maestro::skills::catalog::skills;
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

    let skill = skills()
        .iter()
        .find(|skill| skill.name == "maestro-task")
        .expect("invariant: maestro-task should be bundled");
    let skill_path = paths.skills_dir().join(skill.name).join("SKILL.md");
    fs::write(&skill_path, "edited bundled skill\n")
        .expect("invariant: bundled skill should be editable");

    let update = maestro(&["update"], temp_dir.path());

    assert_success(&update);
    let stdout = String::from_utf8_lossy(&update.stdout);
    assert!(stdout.contains("Checking for updates..."));
    assert!(stdout.contains("Update unavailable for this build"));
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
    assert!(!paths.maestro_dir().join("update").exists());
}

#[test]
fn unavailable_update_cleans_stale_stage_directory() {
    let temp_dir = TestTempDir::new("maestro-update-test");
    init_git_marker(temp_dir.path());
    let paths = MaestroPaths::new(temp_dir.path());
    assert_success(&maestro(&["init", "--yes"], temp_dir.path()));
    fs::create_dir_all(paths.maestro_dir().join("update/nested"))
        .expect("invariant: stale update dir should be writable");
    fs::write(
        paths.maestro_dir().join("update/nested/candidate"),
        "stale\n",
    )
    .expect("invariant: stale update file should be writable");

    let update = maestro(&["update"], temp_dir.path());

    assert_success(&update);
    assert!(!paths.maestro_dir().join("update").exists());
}

#[test]
fn update_accepts_check_verbose_and_force_flags_without_writing() {
    let temp_dir = TestTempDir::new("maestro-update-test");
    init_git_marker(temp_dir.path());
    let paths = MaestroPaths::new(temp_dir.path());
    assert_success(&maestro(&["init", "--yes"], temp_dir.path()));
    fs::create_dir_all(paths.maestro_dir().join("update/nested"))
        .expect("invariant: stale update dir should be writable");
    fs::write(
        paths.maestro_dir().join("update/nested/candidate"),
        "stale\n",
    )
    .expect("invariant: stale update file should be writable");

    let update = maestro(
        &["update", "--check", "--verbose", "--force"],
        temp_dir.path(),
    );

    assert_success(&update);
    let stdout = String::from_utf8_lossy(&update.stdout);
    assert!(stdout.contains("Checking for updates..."));
    assert!(stdout
        .contains("Update unavailable for this build: running from a local development binary."));
    assert!(
        paths.maestro_dir().join("update/nested/candidate").exists(),
        "--check must not clean or write update staging artifacts"
    );
}

#[test]
fn update_check_auto_check_and_update_preserve_user_owned_harness_artifacts() {
    let temp_dir = TestTempDir::new("maestro-update-test");
    init_git_marker(temp_dir.path());
    let paths = MaestroPaths::new(temp_dir.path());
    assert_success(&maestro(&["init", "--yes"], temp_dir.path()));
    mark_user_owned_harness_artifacts(&paths);
    let before = snapshot_files(&user_owned_harness_artifacts(&paths));

    let check = maestro(&["update", "--check"], temp_dir.path());

    assert_success(&check);
    assert_files_unchanged(&before);

    let path = fake_curl_path_env(
        &temp_dir,
        format!(
            r#"#!/bin/sh
printf '{{"tag_name":"v9.9.9-gfuture","published_at":"2026-05-26T05:16:16.000Z","assets":[{{"name":"{}","browser_download_url":"https://example.test/maestro","size":10}}]}}\n'
"#,
            platform_asset_name()
        ),
    );
    let auto_check = Command::new(env!("CARGO_BIN_EXE_maestro"))
        .arg("doctor")
        .current_dir(temp_dir.path())
        .env("MAESTRO_INSTALL_METHOD", "curl")
        .env("PATH", path)
        .output()
        .expect("invariant: maestro doctor should run");

    assert_success(&auto_check);
    assert_files_unchanged(&before);

    let curl_update_path = fake_curl_path_env(
        &temp_dir,
        format!(
            r#"#!/bin/sh
printf '{{"tag_name":"v{}","published_at":"2026-05-26T05:16:16.000Z","assets":[{{"name":"{}","browser_download_url":"https://example.test/maestro","size":10}}]}}\n'
"#,
            env!("MAESTRO_BUILD_VERSION"),
            platform_asset_name()
        ),
    );
    let curl_update = Command::new(env!("CARGO_BIN_EXE_maestro"))
        .arg("update")
        .current_dir(temp_dir.path())
        .env("MAESTRO_INSTALL_METHOD", "curl")
        .env("PATH", curl_update_path)
        .output()
        .expect("invariant: maestro update should run");

    assert_success(&curl_update);
    assert_files_unchanged(&before);

    let update = maestro(&["update"], temp_dir.path());

    assert_success(&update);
    assert_files_unchanged(&before);
}

#[test]
fn update_reports_manager_commands_for_brew_and_cargo_installs() {
    let temp_dir = TestTempDir::new("maestro-update-test");
    init_git_marker(temp_dir.path());
    assert_success(&maestro(&["init", "--yes"], temp_dir.path()));

    let brew = Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(["update", "--check"])
        .current_dir(temp_dir.path())
        .env("MAESTRO_INSTALL_METHOD", "brew")
        .output()
        .expect("invariant: maestro update should run");
    assert_success(&brew);
    let stdout = String::from_utf8_lossy(&brew.stdout);
    assert!(stdout.contains("Update unavailable for this install"));
    assert!(stdout.contains("brew upgrade maestro"));

    let cargo = Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(["update", "--check"])
        .current_dir(temp_dir.path())
        .env("MAESTRO_INSTALL_METHOD", "cargo")
        .output()
        .expect("invariant: maestro update should run");
    assert_success(&cargo);
    let stdout = String::from_utf8_lossy(&cargo.stdout);
    assert!(stdout.contains("Update unavailable for this install"));
    assert!(stdout.contains("cargo install --locked --force maestro"));
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
            current_version: "0.0.1779700000-gabc123",
            check_only: false,
            force: false,
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
    assert!(!paths.maestro_dir().join("update").exists());
}

#[test]
fn simulated_download_failure_preserves_edited_bundled_skills_and_cleans_stage() {
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
    let skill = skills()
        .iter()
        .find(|skill| skill.name == "maestro-task")
        .expect("invariant: maestro-task should be bundled");
    let skill_path = paths.skills_dir().join(skill.name).join("SKILL.md");
    fs::create_dir_all(
        skill_path
            .parent()
            .expect("invariant: skill path should have a parent"),
    )
    .expect("invariant: skill parent should be creatable");
    fs::write(&skill_path, "edited bundled skill\n")
        .expect("invariant: edited skill should be writable");

    let error = run_update_with_seams(
        &UpdateOptions {
            paths: &paths,
            executable_path: &executable_path,
            backup_timestamp: "test",
            current_version: "0.0.1779700000-gabc123",
            check_only: false,
            force: false,
        },
        &StagingFailingDownloader,
        &NoopVerifier,
        &NoopReplacer,
    )
    .expect_err("invariant: staging downloader should fail update");

    assert!(error.to_string().contains("download failed after staging"));
    assert_eq!(
        fs::read_to_string(skill_path).expect("invariant: edited skill should remain readable"),
        "edited bundled skill\n"
    );
    assert!(!paths.maestro_dir().join("update").exists());
}

#[test]
fn checksum_verification_failure_prevents_binary_replacement() {
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
            current_version: "0.0.1779700000-gabc123",
            check_only: false,
            force: false,
        },
        &CandidateDownloader,
        &FailingVerifier,
        &PanickingReplacer,
    )
    .expect_err("invariant: a failed checksum must abort the update before replacement");

    assert!(
        error.to_string().contains("checksum verification failed"),
        "verification failure should surface its cause: {error}"
    );
    assert_eq!(
        fs::read_to_string(executable_path)
            .expect("invariant: current binary should still be readable"),
        "current binary\n",
        "an unverified candidate must never reach the replacer"
    );
    assert!(!paths.maestro_dir().join("update").exists());
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
            current_version: "0.0.1779700000-gabc123",
            check_only: false,
            force: false,
        },
        &CandidateDownloader,
        &NoopVerifier,
        &FailingReplacer,
    )
    .expect_err("invariant: failing replacer should fail update");

    assert!(error
        .to_string()
        .contains("could not replace the current binary"));
    assert_eq!(
        fs::read_to_string(executable_path)
            .expect("invariant: current binary should still be readable"),
        "current binary\n"
    );
    assert!(!paths.maestro_dir().join("update").exists());
}

#[test]
fn simulated_replace_failure_rolls_back_bundled_skill_writes() {
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
    let skill = skills()
        .iter()
        .find(|skill| skill.name == "maestro-task")
        .expect("invariant: maestro-task should be bundled");
    let skill_path = paths.skills_dir().join(skill.name).join("SKILL.md");
    fs::create_dir_all(
        skill_path
            .parent()
            .expect("invariant: skill path should have a parent"),
    )
    .expect("invariant: skill parent should be creatable");
    fs::write(&skill_path, "edited bundled skill\n")
        .expect("invariant: edited skill should be writable");

    let error = run_update_with_seams(
        &UpdateOptions {
            paths: &paths,
            executable_path: &executable_path,
            backup_timestamp: "test",
            current_version: "0.0.1779700000-gabc123",
            check_only: false,
            force: false,
        },
        &CandidateDownloader,
        &NoopVerifier,
        &FailingReplacer,
    )
    .expect_err("invariant: failing replacer should fail update");

    assert!(error
        .to_string()
        .contains("could not replace the current binary"));
    assert_eq!(
        fs::read_to_string(skill_path).expect("invariant: edited skill should remain readable"),
        "edited bundled skill\n"
    );
    assert!(!paths.maestro_dir().join("update").exists());
}

#[test]
fn schema_mismatch_reports_migrate_and_does_not_mutate_harness_files() {
    let temp_dir = TestTempDir::new("maestro-update-test");
    init_git_marker(temp_dir.path());
    let paths = MaestroPaths::new(temp_dir.path());
    assert_success(&maestro(&["init", "--yes"], temp_dir.path()));

    let harness_yml = paths.harness_dir().join("harness.yml");
    fs::write(
        &harness_yml,
        "schema_version: maestro.harness.v0\nverify: []\n",
    )
    .expect("invariant: harness schema should be writable");
    let before = snapshot_files(&user_owned_harness_artifacts(&paths));

    let update = maestro(&["update"], temp_dir.path());

    assert_success(&update);
    let stdout = String::from_utf8_lossy(&update.stdout);
    assert!(stdout.contains("schema mismatch detected"));
    assert!(stdout.contains("maestro migrate"));
    assert_files_unchanged(&before);
}

#[test]
fn detect_schema_mismatches_reports_advisory_mismatches_without_erroring() {
    let temp_dir = TestTempDir::new("maestro-update-test");
    init_git_marker(temp_dir.path());
    let paths = MaestroPaths::new(temp_dir.path());
    assert_success(&maestro(&["init", "--yes"], temp_dir.path()));

    // A migratable older generation (NeedsMigration) ...
    fs::write(
        paths.harness_dir().join("harness.yml"),
        "schema_version: maestro.harness.v0\nverify: []\n",
    )
    .expect("invariant: harness schema should be writable");
    // ... and an unknown version (Incompatible) must both surface as advisory
    // mismatches; the detector classifies but never aborts.
    fs::write(
        paths.features_dir().join("features.yaml"),
        "schema_version: totally-bogus\nfeatures: []\n",
    )
    .expect("invariant: features schema should be writable");

    let mismatches = detect_schema_mismatches(&paths)
        .expect("invariant: schema-mismatch detection stays advisory and never errors");

    assert!(
        mismatches
            .iter()
            .any(|mismatch| mismatch.found == "maestro.harness.v0"),
        "NeedsMigration gap should be reported as an advisory mismatch: {mismatches:?}"
    );
    assert!(
        mismatches
            .iter()
            .any(|mismatch| mismatch.found == "totally-bogus"),
        "Incompatible gap should be reported as an advisory mismatch: {mismatches:?}"
    );
}

#[test]
fn cli_download_failure_omits_duplicate_anyhow_error_tail() {
    let temp_dir = TestTempDir::new("maestro-update-test");
    init_git_remote(temp_dir.path());
    assert_success(&maestro(&["init", "--yes"], temp_dir.path()));

    let path = fake_curl_path_env(
        &temp_dir,
        format!(
            r#"#!/bin/sh
out=""
want_output=""
for arg in "$@"; do
  if [ -n "$want_output" ]; then out="$arg"; want_output=""; continue; fi
  if [ "$arg" = "--output" ]; then want_output=1; fi
done
if [ -z "$out" ]; then
  printf '{{"tag_name":"v9.9.9-gfuture","published_at":"2026-05-26T05:16:16.000Z","assets":[{{"name":"{}","browser_download_url":"https://example.test/maestro","size":10}}]}}\n'
  exit 0
fi
printf partial > "$out"
echo "curl: (18) transfer closed with outstanding read data remaining" >&2
exit 18
"#,
            platform_asset_name()
        ),
    );
    let output = Command::new(env!("CARGO_BIN_EXE_maestro"))
        .arg("update")
        .current_dir(temp_dir.path())
        .env("MAESTRO_INSTALL_METHOD", "curl")
        .env("PATH", path)
        .output()
        .expect("invariant: maestro update should run");

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stdout.contains("Update failed: download interrupted."));
    assert!(
        !stderr.contains("Error:"),
        "friendly update errors should not be followed by anyhow stderr: {stderr}"
    );
}

#[test]
fn auto_check_reports_available_update_once_per_day_for_curl_installs() {
    let temp_dir = TestTempDir::new("maestro-update-test");
    init_git_marker(temp_dir.path());
    assert_success(&maestro(&["init", "--yes"], temp_dir.path()));

    let path = fake_curl_path_env(
        &temp_dir,
        format!(
            r#"#!/bin/sh
printf '{{"tag_name":"v9.9.9-gfuture","published_at":"2026-05-26T05:16:16.000Z","assets":[{{"name":"{}","browser_download_url":"https://example.test/maestro","size":10}}]}}\n'
"#,
            platform_asset_name()
        ),
    );

    let first = Command::new(env!("CARGO_BIN_EXE_maestro"))
        .arg("doctor")
        .current_dir(temp_dir.path())
        .env("MAESTRO_INSTALL_METHOD", "curl")
        .env("PATH", &path)
        .output()
        .expect("invariant: maestro doctor should run");
    assert_success(&first);
    let stdout = String::from_utf8_lossy(&first.stdout);
    assert!(stdout.contains("Update available: 9.9.9-gfuture"));
    assert!(stdout.contains("Run `maestro update` to install."));

    let second = Command::new(env!("CARGO_BIN_EXE_maestro"))
        .arg("doctor")
        .current_dir(temp_dir.path())
        .env("MAESTRO_INSTALL_METHOD", "curl")
        .env("PATH", path)
        .output()
        .expect("invariant: maestro doctor should run");
    assert_success(&second);
    let stdout = String::from_utf8_lossy(&second.stdout);
    assert!(!stdout.contains("Update available: 9.9.9-gfuture"));
}

#[test]
fn auto_check_does_not_write_or_print_after_init_dry_run() {
    let temp_dir = TestTempDir::new("maestro-update-test");
    init_git_marker(temp_dir.path());
    let path = fake_curl_path_env(
        &temp_dir,
        format!(
            r#"#!/bin/sh
printf '{{"tag_name":"v9.9.9-gfuture","published_at":"2026-05-26T05:16:16.000Z","assets":[{{"name":"{}","browser_download_url":"https://example.test/maestro","size":10}}]}}\n'
"#,
            platform_asset_name()
        ),
    );

    let output = Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(["init", "--dry-run"])
        .current_dir(temp_dir.path())
        .env("MAESTRO_INSTALL_METHOD", "curl")
        .env("PATH", path)
        .output()
        .expect("invariant: maestro init should run");

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("maestro init would create:"));
    assert!(!stdout.contains("Update available:"));
    assert!(!temp_dir.path().join(".maestro").exists());
}

#[cfg(unix)]
#[test]
fn atomic_replacer_preserves_current_binary_permissions() {
    let temp_dir = TestTempDir::new("maestro-update-test");
    let executable_path = temp_dir.path().join("bin").join("maestro");
    let candidate_path = temp_dir.path().join("candidate-maestro");
    fs::create_dir_all(
        executable_path
            .parent()
            .expect("invariant: executable path should have a parent"),
    )
    .expect("invariant: executable parent should be creatable");
    fs::write(&executable_path, "current binary\n")
        .expect("invariant: current binary should be writable");
    fs::set_permissions(&executable_path, fs::Permissions::from_mode(0o755))
        .expect("invariant: current binary permissions should be writable");
    fs::write(&candidate_path, "replacement binary\n")
        .expect("invariant: candidate binary should be writable");
    fs::set_permissions(&candidate_path, fs::Permissions::from_mode(0o600))
        .expect("invariant: candidate permissions should be writable");

    AtomicBinaryReplacer
        .replace(&executable_path, &candidate_path)
        .expect("invariant: replacement should succeed");

    assert_eq!(
        fs::read_to_string(&executable_path).expect("invariant: binary should be readable"),
        "replacement binary\n"
    );
    let mode = fs::metadata(&executable_path)
        .expect("invariant: binary metadata should be readable")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o755);
}

fn init_git_marker(repo: &Path) {
    fs::create_dir(repo.join(".git")).expect("invariant: .git marker should be creatable");
}

fn init_git_remote(repo: &Path) {
    assert_success(
        &Command::new("git")
            .args(["init", "-q"])
            .current_dir(repo)
            .output()
            .expect("invariant: git init should run"),
    );
    assert_success(
        &Command::new("git")
            .args([
                "remote",
                "add",
                "origin",
                "https://github.com/ReinaMacCredy/maestro.git",
            ])
            .current_dir(repo)
            .output()
            .expect("invariant: git remote add should run"),
    )
}

fn platform_asset_name() -> String {
    format!(
        "maestro-{}-{}",
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

fn fake_curl_path_env(temp_dir: &TestTempDir, script: impl AsRef<str>) -> String {
    let fakebin = temp_dir.path().join("fakebin");
    fs::create_dir_all(&fakebin).expect("invariant: fakebin should be creatable");
    let fake_curl = fakebin.join("curl");
    fs::write(&fake_curl, script.as_ref()).expect("invariant: fake curl should be writable");
    #[cfg(unix)]
    fs::set_permissions(&fake_curl, fs::Permissions::from_mode(0o755))
        .expect("invariant: fake curl should be executable");

    let path = env::var_os("PATH").expect("invariant: PATH should be set");
    format!("{}:{}", fakebin.display(), path.to_string_lossy())
}

fn mark_user_owned_harness_artifacts(paths: &MaestroPaths) {
    let harness_protocol = paths.harness_dir().join("HARNESS.md");
    fs::write(
        &harness_protocol,
        "# User-owned Harness Protocol\n\nDo not rewrite this file during update.\n",
    )
    .expect("invariant: harness protocol should be writable");

    for path in [
        paths.harness_dir().join("harness.yml"),
        paths.harness_dir().join("backlog.yaml"),
        paths.features_dir().join("features.yaml"),
    ] {
        let contents = fs::read_to_string(&path)
            .expect("invariant: initialized schema artifact should be readable");
        fs::write(
            &path,
            format!("{contents}\n# user-owned update non-mutation marker\n"),
        )
        .expect("invariant: initialized schema artifact should be writable");
    }
}

fn user_owned_harness_artifacts(paths: &MaestroPaths) -> Vec<PathBuf> {
    vec![
        paths.harness_dir().join("HARNESS.md"),
        paths.harness_dir().join("harness.yml"),
        paths.harness_dir().join("backlog.yaml"),
        paths.features_dir().join("features.yaml"),
    ]
}

fn snapshot_files(paths: &[PathBuf]) -> Vec<(PathBuf, String)> {
    paths
        .iter()
        .map(|path| {
            (
                path.clone(),
                fs::read_to_string(path).expect("invariant: snapshot file should be readable"),
            )
        })
        .collect()
}

fn assert_files_unchanged(snapshot: &[(PathBuf, String)]) {
    for (path, expected) in snapshot {
        let actual = fs::read_to_string(path).expect("invariant: snapshot file should be readable");
        assert_eq!(
            actual.as_str(),
            expected.as_str(),
            "{} should not be rewritten by update flows",
            path.display()
        );
    }
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
    fn download(&self, _request: &UpdateRequest) -> Result<DownloadedBinary> {
        bail!("download failed")
    }
}

struct StagingFailingDownloader;

impl UpdateDownloader for StagingFailingDownloader {
    fn download(&self, request: &UpdateRequest) -> Result<DownloadedBinary> {
        let work_dir = &request.work_dir;
        fs::create_dir_all(work_dir)?;
        fs::write(work_dir.join("partial"), "partial binary\n")?;
        bail!("download failed after staging")
    }
}

struct CandidateDownloader;

impl UpdateDownloader for CandidateDownloader {
    fn download(&self, request: &UpdateRequest) -> Result<DownloadedBinary> {
        let work_dir = &request.work_dir;
        fs::create_dir_all(work_dir)?;
        fs::create_dir_all(work_dir.join("scratch"))?;
        fs::write(work_dir.join("scratch/metadata"), "metadata\n")?;
        let candidate = work_dir.join("candidate-maestro");
        fs::write(&candidate, "replacement binary\n")?;

        Ok(DownloadedBinary::Available {
            path: candidate,
            release: Some(test_release()),
        })
    }
}

fn test_release() -> ReleaseInfo {
    ReleaseInfo {
        version: "0.0.1779772576-g751b94".to_string(),
        released_at: Some("2026-05-26T05:16:16.000Z".to_string()),
        relative_age: Some("1h ago".to_string()),
        size_bytes: Some(25_350_000),
    }
}

struct NoopVerifier;

impl ChecksumVerifier for NoopVerifier {
    fn verify(&self, _candidate: &Path) -> Result<()> {
        Ok(())
    }
}

struct FailingVerifier;

impl ChecksumVerifier for FailingVerifier {
    fn verify(&self, _candidate: &Path) -> Result<()> {
        bail!("checksum verification failed")
    }
}

struct PanickingReplacer;

impl BinaryReplacer for PanickingReplacer {
    fn replace(&self, _current: &Path, _candidate: &Path) -> Result<()> {
        panic!("invariant: replacer must not run when verification fails")
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
