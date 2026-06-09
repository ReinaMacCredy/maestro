#![cfg(unix)]

mod support;

use std::fs;
use std::path::Path;
use std::process::Command;

use maestro::domain::skills::catalog::skills;
use support::TestTempDir;

fn maestro(args: &[&str], cwd: &Path, home: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .current_dir(cwd)
        .env("HOME", home)
        .output()
        .expect("invariant: compiled maestro binary should be runnable in global skill tests")
}

fn assert_success(output: &std::process::Output) {
    assert!(
        output.status.success(),
        "expected success\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_failure(output: &std::process::Output) {
    assert!(
        !output.status.success(),
        "expected failure\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn init_git_marker(repo: &Path) {
    fs::create_dir(repo.join(".git")).expect("invariant: .git marker should be creatable");
}

fn init_repo(repo: &Path, home: &Path) {
    init_git_marker(repo);
    assert_success(&maestro(&["init", "--yes"], repo, home));
}

fn init_install_prereqs(repo: &Path) {
    fs::create_dir(repo.join(".git")).expect("invariant: .git marker should be creatable");
    fs::create_dir_all(repo.join(".maestro/harness"))
        .expect("invariant: harness dir should be creatable");
    fs::write(
        repo.join(".maestro/harness/HARNESS.md"),
        "# Maestro Harness Protocol\n",
    )
    .expect("invariant: harness protocol should be writable");
    fs::create_dir_all(repo.join(".maestro/hooks"))
        .expect("invariant: hooks dir should be creatable");
    fs::write(
        repo.join(".maestro/hooks/record.sh"),
        "# maestro:hook-version: 1.0.0\nexec maestro hook record\n",
    )
    .expect("invariant: hook recorder script should be writable");
}

fn bundled_task_skill_md() -> String {
    skills()
        .iter()
        .find(|skill| skill.name == "maestro-task")
        .expect("invariant: maestro-task should be bundled")
        .skill_md()
        .to_string()
}

fn assert_symlink_target(path: &Path, target: &Path) {
    let metadata = fs::symlink_metadata(path).expect("invariant: symlink should exist");
    assert!(
        metadata.file_type().is_symlink(),
        "{} is not a symlink",
        path.display()
    );
    assert_eq!(
        fs::read_link(path).expect("invariant: symlink should be readable"),
        target
    );
}

#[test]
fn install_points_to_explicit_sync_which_creates_global_cache_lock_and_supported_agent_links() {
    let temp = TestTempDir::new("maestro-global-skills-test");
    let repo = temp.path().join("repo");
    let home = temp.path().join("home");
    fs::create_dir(&repo).expect("invariant: repo should be creatable");
    fs::create_dir(&home).expect("invariant: home should be creatable");
    init_repo(&repo, &home);
    fs::write(
        repo.join(".maestro/skills/maestro-task/SKILL.md"),
        "repo-local edit must not feed global cache\n",
    )
    .expect("invariant: repo-local skill should be editable");

    let output = maestro(&["install", "--agent", "codex"], &repo, &home);

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("installed maestro codex integration (repo only)"),
        "{stdout}"
    );
    assert!(stdout.contains("global skills not synced"), "{stdout}");
    assert!(
        stdout.contains("next: maestro sync --global-skills"),
        "{stdout}"
    );
    assert!(
        !home.join(".maestro/skills/maestro-task/SKILL.md").exists(),
        "install must not write global skill cache"
    );

    let sync = maestro(&["sync", "--global-skills"], &repo, &home);
    assert_success(&sync);
    let sync_stdout = String::from_utf8_lossy(&sync.stdout);
    assert!(
        sync_stdout.contains("global Maestro skills synced for all supported agents:"),
        "{sync_stdout}"
    );
    assert!(sync_stdout.contains(&format!(
        "cache: {}",
        home.join(".maestro/skills").display()
    )));
    assert!(sync_stdout.contains(&format!(
        "codex root: {}",
        home.join(".agents/skills").display()
    )));
    assert!(sync_stdout.contains(&format!(
        "claude root: {}",
        home.join(".claude/skills").display()
    )));
    assert!(
        sync_stdout.contains("~/.codex/skills skipped"),
        "{sync_stdout}"
    );

    assert_eq!(
        fs::read_to_string(home.join(".maestro/skills/maestro-task/SKILL.md"))
            .expect("invariant: global task skill should be readable"),
        bundled_task_skill_md()
    );
    assert_symlink_target(
        &home.join(".agents/skills/maestro-task"),
        &home.join(".maestro/skills/maestro-task"),
    );
    assert_symlink_target(
        &home.join(".claude/skills/maestro-task"),
        &home.join(".maestro/skills/maestro-task"),
    );
    assert!(!home.join(".codex/skills/maestro-task").exists());

    let lock = fs::read_to_string(home.join(".maestro/skills-lock.yaml"))
        .expect("invariant: global lock should be readable");
    assert!(lock.contains("schema_version: maestro.global_skills_lock.v1"));
    assert!(lock.contains("codex:maestro-task"));
    assert!(lock.contains("display_path:"));
    assert!(lock.contains("resolved_path:"));

    assert!(repo.join(".codex/config.toml").is_file());
    assert!(repo.join(".codex/skills").is_symlink());
    assert!(!repo.join(".claude/settings.local.json").exists());

    let uninstall = maestro(&["uninstall", "--agent", "codex"], &repo, &home);
    assert_success(&uninstall);
    assert!(
        home.join(".maestro/skills/maestro-task/SKILL.md").is_file(),
        "repo-local uninstall must not remove global skills"
    );
}

#[test]
fn install_leaves_global_collision_for_explicit_sync() {
    let temp = TestTempDir::new("maestro-global-skills-test");
    let repo = temp.path().join("repo");
    let home = temp.path().join("home");
    fs::create_dir(&repo).expect("invariant: repo should be creatable");
    fs::create_dir(&home).expect("invariant: home should be creatable");
    init_install_prereqs(&repo);
    fs::create_dir_all(home.join(".agents/skills"))
        .expect("invariant: global root should be creatable");
    fs::write(home.join(".agents/skills/maestro-task"), "user skill\n")
        .expect("invariant: collision should be writable");

    let output = maestro(&["install", "--agent", "codex"], &repo, &home);

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("installed maestro codex integration (repo only)"),
        "{stdout}"
    );
    assert!(
        stdout.contains("next: maestro sync --global-skills"),
        "{stdout}"
    );
    assert!(repo.join(".maestro/install-lock.yaml").exists());
    assert!(repo.join(".codex/config.toml").exists());
    assert!(repo.join(".codex/skills").is_symlink());
    assert!(!home.join(".maestro/skills/maestro-task/SKILL.md").exists());

    let sync = maestro(&["sync", "--global-skills"], &repo, &home);

    assert_failure(&sync);
    let sync_output = format!(
        "{}{}",
        String::from_utf8_lossy(&sync.stdout),
        String::from_utf8_lossy(&sync.stderr)
    );
    assert!(
        sync_output.contains("refusing global skill install"),
        "{sync_output}"
    );
    assert!(sync_output.contains("maestro-task"), "{sync_output}");
}

#[test]
fn sync_global_skills_refreshes_global_cache_without_touching_repo_local_skills() {
    let temp = TestTempDir::new("maestro-global-skills-test");
    let repo = temp.path().join("repo");
    let home = temp.path().join("home");
    fs::create_dir(&repo).expect("invariant: repo should be creatable");
    fs::create_dir(&home).expect("invariant: home should be creatable");
    init_repo(&repo, &home);
    fs::write(
        repo.join(".maestro/skills/maestro-task/SKILL.md"),
        "repo-local edit must remain\n",
    )
    .expect("invariant: repo-local skill should be editable");
    assert_success(&maestro(&["sync", "--global-skills"], &repo, &home));
    fs::remove_file(home.join(".maestro/skills/maestro-task/SKILL.md"))
        .expect("invariant: global skill should be removable");

    let output = maestro(&["sync", "--global-skills"], &repo, &home);

    assert_success(&output);
    assert_eq!(
        fs::read_to_string(home.join(".maestro/skills/maestro-task/SKILL.md"))
            .expect("invariant: global task skill should be readable"),
        bundled_task_skill_md()
    );
    assert_eq!(
        fs::read_to_string(repo.join(".maestro/skills/maestro-task/SKILL.md"))
            .expect("invariant: repo-local task skill should be readable"),
        "repo-local edit must remain\n"
    );
}

#[test]
fn update_check_does_not_mutate_global_skills_but_update_refreshes_existing_global_lock() {
    let temp = TestTempDir::new("maestro-global-skills-test");
    let repo = temp.path().join("repo");
    let home = temp.path().join("home");
    fs::create_dir(&repo).expect("invariant: repo should be creatable");
    fs::create_dir(&home).expect("invariant: home should be creatable");
    init_repo(&repo, &home);
    assert_success(&maestro(&["sync", "--global-skills"], &repo, &home));
    let global_task = home.join(".maestro/skills/maestro-task/SKILL.md");
    fs::remove_file(&global_task).expect("invariant: global skill should be removable");

    let check = maestro(&["upgrade", "--check"], &repo, &home);

    assert_success(&check);
    assert!(
        !global_task.exists(),
        "update --check must not restore or mutate global skills"
    );

    let update = maestro(&["upgrade"], &repo, &home);

    assert_success(&update);
    assert_eq!(
        fs::read_to_string(&global_task).expect("invariant: global task skill should be readable"),
        bundled_task_skill_md()
    );
    assert!(
        String::from_utf8_lossy(&update.stdout)
            .contains("global Maestro skills synced for all supported agents:"),
        "update should report the global refresh"
    );
}
