mod support;

use std::fs;
#[cfg(unix)]
use std::os::unix::fs as unix_fs;
use std::process::Command;

use support::TestTempDir;

const BUNDLED_SKILLS: [&str; 7] = [
    "maestro-task",
    "maestro-feature",
    "maestro-setup",
    "maestro-verify",
    "maestro-design",
    "qa-baseline",
    "qa-slice",
];

fn maestro(args: &[&str], cwd: &std::path::Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("invariant: compiled maestro binary should be runnable in init tests")
}

fn init_git_marker(repo: &std::path::Path) {
    fs::create_dir(repo.join(".git")).expect("invariant: .git marker should be creatable");
}

#[test]
fn init_operation_public_surface_resolves() {
    let run: fn(
        &maestro::operations::init::InitOptions,
    ) -> anyhow::Result<maestro::operations::init::InitOutcome> = maestro::operations::init::run;
    let render: fn(&maestro::operations::init::InitPlan) -> String =
        maestro::operations::init::render_dry_run;

    let _ = (run, render);
}

#[test]
fn init_dry_run_prints_tree_without_writing() {
    let temp_dir = TestTempDir::new("maestro-init-test");
    init_git_marker(temp_dir.path());

    let output = maestro(&["init", "--dry-run"], temp_dir.path());

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("invariant: stdout should be UTF-8");
    assert!(stdout.contains("maestro init would create:"));
    // HARNESS.md is now extraction-owned (like skills and the hook script), so it
    // is no longer enumerated in the InitPlan dry-run; harness.yml still is.
    assert!(stdout.contains(".maestro/harness/harness.yml"));
    assert!(!temp_dir.path().join(".maestro").exists());
}

#[test]
fn init_creates_minimal_artifact_tree() {
    let temp_dir = TestTempDir::new("maestro-init-test");
    init_git_marker(temp_dir.path());
    fs::write(
        temp_dir.path().join("Cargo.toml"),
        "[package]\nname = \"demo\"\n",
    )
    .expect("invariant: Cargo.toml should be writable");

    let output = maestro(&["init", "--yes"], temp_dir.path());

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(temp_dir
        .path()
        .join(".maestro/harness/HARNESS.md")
        .is_file());
    assert!(temp_dir.path().join(".maestro/hooks/record.sh").is_file());
    assert!(temp_dir
        .path()
        .join(".maestro/harness/harness.yml")
        .is_file());
    assert!(temp_dir
        .path()
        .join(".maestro/harness/backlog.yaml")
        .is_file());
    assert!(temp_dir.path().join(".maestro/features").is_dir());
    assert!(!temp_dir
        .path()
        .join(".maestro/features/features.yaml")
        .exists());
    assert!(temp_dir.path().join(".maestro/decisions").is_dir());
    assert!(temp_dir.path().join(".maestro/skills").is_dir());
    for skill in BUNDLED_SKILLS {
        assert!(temp_dir
            .path()
            .join(".maestro/skills")
            .join(skill)
            .join("SKILL.md")
            .is_file());
    }
    assert!(!temp_dir.path().join(".maestro/skill-index.yaml").exists());
    assert!(!temp_dir
        .path()
        .join(".maestro/skills/skill-index.yaml")
        .exists());

    let harness_yml = fs::read_to_string(temp_dir.path().join(".maestro/harness/harness.yml"))
        .expect("invariant: harness.yml should be readable");
    assert!(harness_yml.contains("schema_version: maestro.harness.v1"));
    assert!(harness_yml.contains("kind: rust"));
}

#[test]
fn init_merge_preserves_existing_files() {
    let temp_dir = TestTempDir::new("maestro-init-test");
    init_git_marker(temp_dir.path());
    let harness = temp_dir.path().join(".maestro/harness/HARNESS.md");
    fs::create_dir_all(
        harness
            .parent()
            .expect("invariant: harness path should have parent"),
    )
    .expect("invariant: harness directory should be creatable");
    fs::write(&harness, "custom\n").expect("invariant: existing harness should be writable");

    let output = maestro(&["init", "--merge"], temp_dir.path());

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(&harness).expect("invariant: harness should be readable"),
        "custom\n"
    );
}

#[test]
fn init_merge_accepts_yes_for_scripted_brownfield_runs() {
    let temp_dir = TestTempDir::new("maestro-init-test");
    init_git_marker(temp_dir.path());
    let harness = temp_dir.path().join(".maestro/harness/HARNESS.md");
    fs::create_dir_all(
        harness
            .parent()
            .expect("invariant: harness path should have parent"),
    )
    .expect("invariant: harness directory should be creatable");
    fs::write(&harness, "custom\n").expect("invariant: existing harness should be writable");

    let output = maestro(&["init", "--merge", "--yes"], temp_dir.path());

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(&harness).expect("invariant: harness should be readable"),
        "custom\n"
    );
}

#[test]
fn init_force_overwrites_existing_file_with_backup() {
    let temp_dir = TestTempDir::new("maestro-init-test");
    init_git_marker(temp_dir.path());
    let harness = temp_dir.path().join(".maestro/harness/HARNESS.md");
    fs::create_dir_all(
        harness
            .parent()
            .expect("invariant: harness path should have parent"),
    )
    .expect("invariant: harness directory should be creatable");
    fs::write(&harness, "custom\n").expect("invariant: existing harness should be writable");

    let output = maestro(&["init", "--force"], temp_dir.path());

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let rewritten = fs::read_to_string(&harness).expect("invariant: harness should be readable");
    assert!(rewritten.contains("# Maestro Harness Protocol"));

    let backups = fs::read_dir(temp_dir.path().join(".maestro/backups"))
        .expect("invariant: backup root should exist")
        .count();
    assert!(backups > 0);
}

#[test]
fn init_force_groups_multiple_backups_in_one_operation_directory() {
    let temp_dir = TestTempDir::new("maestro-init-test");
    init_git_marker(temp_dir.path());
    let harness = temp_dir.path().join(".maestro/harness/HARNESS.md");
    let backlog = temp_dir.path().join(".maestro/harness/backlog.yaml");
    fs::create_dir_all(
        harness
            .parent()
            .expect("invariant: harness path should have parent"),
    )
    .expect("invariant: harness directory should be creatable");
    fs::write(&harness, "custom harness\n").expect("invariant: harness should be writable");
    fs::write(&backlog, "custom backlog\n").expect("invariant: backlog should be writable");

    let output = maestro(&["init", "--force"], temp_dir.path());

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let backup_dirs = fs::read_dir(temp_dir.path().join(".maestro/backups"))
        .expect("invariant: backup root should exist")
        .collect::<Result<Vec<_>, _>>()
        .expect("invariant: backups should be readable");
    assert_eq!(backup_dirs.len(), 1);
    let backup_dir = backup_dirs[0].path();
    assert!(backup_dir.join(".maestro/harness/HARNESS.md").is_file());
    assert!(backup_dir.join(".maestro/harness/backlog.yaml").is_file());
}

#[test]
fn init_refuses_existing_file_without_merge_or_force() {
    let temp_dir = TestTempDir::new("maestro-init-test");
    init_git_marker(temp_dir.path());
    let harness = temp_dir.path().join(".maestro/harness/HARNESS.md");
    fs::create_dir_all(
        harness
            .parent()
            .expect("invariant: harness path should have parent"),
    )
    .expect("invariant: harness directory should be creatable");
    fs::write(&harness, "custom\n").expect("invariant: existing harness should be writable");

    let output = maestro(&["init"], temp_dir.path());

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("invariant: stderr should be UTF-8");
    assert!(stderr.contains("already exists"));
}

#[cfg(unix)]
#[test]
fn init_rejects_symlinked_maestro_root() {
    let temp_dir = TestTempDir::new("maestro-init-test");
    init_git_marker(temp_dir.path());
    let external = TestTempDir::new("maestro-init-external");
    unix_fs::symlink(external.path(), temp_dir.path().join(".maestro"))
        .expect("invariant: symlinked maestro root should be creatable");

    let output = maestro(&["init", "--yes"], temp_dir.path());

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("symlink"));
    assert!(!external.path().join("harness/harness.yml").exists());
}

#[cfg(unix)]
#[test]
fn init_rejects_symlinked_managed_subdirectory() {
    let temp_dir = TestTempDir::new("maestro-init-test");
    init_git_marker(temp_dir.path());
    let external = TestTempDir::new("maestro-init-external");
    fs::create_dir_all(temp_dir.path().join(".maestro"))
        .expect("invariant: maestro dir should be writable");
    unix_fs::symlink(external.path(), temp_dir.path().join(".maestro/harness"))
        .expect("invariant: symlinked harness dir should be creatable");

    let output = maestro(&["init", "--yes"], temp_dir.path());

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("symlink"));
    assert!(!external.path().join("harness.yml").exists());
}

#[test]
fn init_bootstraps_empty_directory_without_git_marker() {
    let temp_dir = TestTempDir::new("maestro-init-empty-test");

    let output = maestro(&["init", "--yes"], temp_dir.path());

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(temp_dir
        .path()
        .join(".maestro/harness/HARNESS.md")
        .is_file());
    assert!(temp_dir.path().join(".maestro/features").is_dir());
}

#[test]
fn init_preflights_bundled_skill_conflicts_before_writing_harness() {
    let temp_dir = TestTempDir::new("maestro-init-test");
    init_git_marker(temp_dir.path());
    let skill = temp_dir
        .path()
        .join(".maestro/skills/maestro-task/SKILL.md");
    fs::create_dir_all(
        skill
            .parent()
            .expect("invariant: skill path should have a parent"),
    )
    .expect("invariant: skill parent should be writable");
    fs::write(&skill, "custom skill\n").expect("invariant: skill should be writable");

    let output = maestro(&["init", "--yes"], temp_dir.path());

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("already exists"));
    assert!(!temp_dir
        .path()
        .join(".maestro/harness/harness.yml")
        .exists());
    assert_eq!(
        fs::read_to_string(skill).expect("invariant: skill should remain readable"),
        "custom skill\n"
    );
}
