mod support;

use std::fs;
#[cfg(unix)]
use std::os::unix::fs as unix_fs;
use std::process::Command;

use support::TestTempDir;

fn maestro(args: &[&str], cwd: &std::path::Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("invariant: compiled maestro binary should be runnable in init tests")
}

fn maestro_with_clean_agent_env(
    args: &[&str],
    cwd: &std::path::Path,
    envs: &[(&str, &str)],
) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_maestro"));
    command.args(args).current_dir(cwd);
    for key in [
        "MAESTRO_AGENT",
        "CLAUDECODE",
        "CLAUDE_CODE",
        "CODEX_CLI",
        "CODEX_SANDBOX",
        "CODEX_THREAD_ID",
        "CLAUDE_SESSION_ID",
        "CLAUDECODE_SESSION_ID",
        "CLAUDE_CODE_SESSION_ID",
    ] {
        command.env_remove(key);
    }
    for (key, value) in envs {
        command.env(key, value);
    }
    command
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
    let render: fn(
        &maestro::operations::init::InitPlan,
        &[maestro::domain::extraction::FolderPreview],
    ) -> String = maestro::operations::init::render_dry_run;

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
    assert!(stdout.contains("after init:"));
    assert!(stdout.contains("resume: maestro status"));
    assert!(stdout.contains("safety: dry-run writes nothing"));
    // HARNESS.md is now extraction-owned (like skills and the hook script), so it
    // is no longer enumerated in the InitPlan dry-run; harness.yml still is.
    assert!(stdout.contains(".maestro/harness/harness.yml"));
    assert!(!temp_dir.path().join(".maestro").exists());
}

#[test]
fn init_dry_run_previews_bundled_extraction() {
    let temp_dir = TestTempDir::new("maestro-init-test");
    init_git_marker(temp_dir.path());

    let output = maestro(&["init", "--dry-run"], temp_dir.path());

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("invariant: stdout should be UTF-8");
    // The dry-run reuses the extraction-preview machinery, so every bundled
    // resource is enumerated with its `create` verb (the tree is empty here).
    // Skills are global-only (ac-1), so they no longer appear here.
    assert!(stdout.contains("create   HARNESS.md"), "{stdout}");
    assert!(stdout.contains("create   RECOVERY.md"), "{stdout}");
    assert!(stdout.contains("create   record.sh"), "{stdout}");
    assert!(stdout.contains("check setup: maestro doctor"), "{stdout}");
    assert!(!temp_dir.path().join(".maestro").exists());
}

#[test]
fn init_agent_handoff_uses_detected_agent_or_choice_fallback() {
    let fallback = TestTempDir::new("maestro-init-agent-fallback");
    init_git_marker(fallback.path());
    let fallback_output = maestro_with_clean_agent_env(&["init", "--yes"], fallback.path(), &[]);
    assert!(fallback_output.status.success());
    let fallback_stdout =
        String::from_utf8(fallback_output.stdout).expect("invariant: stdout should be UTF-8");
    assert!(
        fallback_stdout.contains("wire agent: maestro install --agent <claude|codex|droid>"),
        "{fallback_stdout}"
    );

    let claude = TestTempDir::new("maestro-init-agent-claude");
    init_git_marker(claude.path());
    let claude_output = maestro_with_clean_agent_env(
        &["init", "--yes"],
        claude.path(),
        &[("MAESTRO_AGENT", "claude")],
    );
    assert!(claude_output.status.success());
    let claude_stdout =
        String::from_utf8(claude_output.stdout).expect("invariant: stdout should be UTF-8");
    assert!(
        claude_stdout.contains("wire agent: maestro install --agent claude"),
        "{claude_stdout}"
    );
}

#[test]
fn init_merge_hints_sync_when_a_folder_is_behind() {
    let temp_dir = TestTempDir::new("maestro-init-test");
    init_git_marker(temp_dir.path());

    let first = maestro(&["init", "--yes"], temp_dir.path());
    assert!(
        first.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&first.stderr)
    );

    // Drift a managed resource into a versionless state: merge preserves it, but
    // it is now behind the binary's shipped version.
    let record = temp_dir.path().join(".maestro/hooks/record.sh");
    fs::write(&record, "edited hook script\n").expect("invariant: hook script should be writable");

    let output = maestro(&["init", "--merge"], temp_dir.path());

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("invariant: stdout should be UTF-8");
    assert!(stdout.contains("behind this maestro version"), "{stdout}");
    assert!(stdout.contains("maestro sync"), "{stdout}");
    // Merge keeps the local edit; the hint just points at `sync` to refresh it.
    assert_eq!(
        fs::read_to_string(&record).expect("invariant: hook script should be readable"),
        "edited hook script\n"
    );
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

    let output = maestro_with_clean_agent_env(
        &["init", "--yes"],
        temp_dir.path(),
        &[("MAESTRO_AGENT", "codex")],
    );

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        temp_dir
            .path()
            .join(".maestro/harness/HARNESS.md")
            .is_file()
    );
    assert!(temp_dir.path().join(".maestro/hooks/record.sh").is_file());
    assert!(
        temp_dir
            .path()
            .join(".maestro/harness/harness.yml")
            .is_file()
    );
    assert!(
        !temp_dir
            .path()
            .join(".maestro/harness/backlog.yaml")
            .exists(),
        "the backlog has no file of its own; items live as idea cards (D7)"
    );
    assert!(temp_dir.path().join(".maestro/cards").is_dir());
    // ac-1: skills are global-only; init extracts no per-repo skills directory.
    assert!(
        !temp_dir.path().join(".maestro/skills").exists(),
        "init must not create a per-repo .maestro/skills directory"
    );
    assert!(!temp_dir.path().join(".maestro/skill-index.yaml").exists());
    let stdout = String::from_utf8(output.stdout).expect("invariant: stdout should be UTF-8");
    assert!(stdout.contains("next:"), "{stdout}");
    assert!(stdout.contains("check setup: maestro doctor"), "{stdout}");
    assert!(
        stdout.contains("wire agent: maestro install --agent codex"),
        "{stdout}"
    );
    assert!(stdout.contains("resume: maestro status"), "{stdout}");

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
fn init_yes_is_idempotent_keeps_edits_and_restores_missing() {
    let temp_dir = TestTempDir::new("maestro-init-test");
    init_git_marker(temp_dir.path());

    // First run scaffolds the full tree.
    let first = maestro(&["init", "--yes"], temp_dir.path());
    assert!(
        first.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&first.stderr)
    );

    // Locally customize a managed resource file, and delete a managed init file.
    let record = temp_dir.path().join(".maestro/hooks/record.sh");
    fs::write(&record, "custom hook script\n").expect("invariant: hook script should be writable");
    let harness_yml = temp_dir.path().join(".maestro/harness/harness.yml");
    fs::remove_file(&harness_yml).expect("invariant: harness.yml should be removable");

    // Re-running `init --yes` is idempotent: exit 0, keep the local edit, and
    // restore the missing managed file.
    let second = maestro(&["init", "--yes"], temp_dir.path());
    assert!(
        second.status.success(),
        "re-init --yes must be idempotent; stderr: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert_eq!(
        fs::read_to_string(&record).expect("invariant: hook script should remain readable"),
        "custom hook script\n",
        "merge must preserve the local edit"
    );
    assert!(
        harness_yml.is_file(),
        "merge must restore the removed managed file"
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
    let config = temp_dir.path().join(".maestro/harness/harness.yml");
    fs::create_dir_all(
        harness
            .parent()
            .expect("invariant: harness path should have parent"),
    )
    .expect("invariant: harness directory should be creatable");
    fs::write(&harness, "custom harness\n").expect("invariant: harness should be writable");
    fs::write(&config, "custom config\n").expect("invariant: config should be writable");

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
    assert!(backup_dir.join(".maestro/harness/harness.yml").is_file());
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
    assert!(
        temp_dir
            .path()
            .join(".maestro/harness/HARNESS.md")
            .is_file()
    );
    assert!(temp_dir.path().join(".maestro/cards").is_dir());
}

#[test]
fn init_preflights_bundled_resource_conflicts_before_writing_harness() {
    let temp_dir = TestTempDir::new("maestro-init-test");
    init_git_marker(temp_dir.path());
    let record = temp_dir.path().join(".maestro/hooks/record.sh");
    fs::create_dir_all(
        record
            .parent()
            .expect("invariant: hook path should have a parent"),
    )
    .expect("invariant: hooks dir should be writable");
    fs::write(&record, "custom hook script\n").expect("invariant: hook script should be writable");

    // Bare `init` (Create mode) preflights the conflict; `--yes`/`--merge` would
    // instead keep the existing file and succeed (see the idempotency test).
    let output = maestro(&["init"], temp_dir.path());

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("already exists"));
    assert!(
        !temp_dir
            .path()
            .join(".maestro/harness/harness.yml")
            .exists()
    );
    assert_eq!(
        fs::read_to_string(record).expect("invariant: hook script should remain readable"),
        "custom hook script\n"
    );
}
