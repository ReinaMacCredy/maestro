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
        .find(|skill| skill.name == "maestro-card")
        .expect("invariant: maestro-card should be bundled")
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
fn install_syncs_global_cache_lock_and_supported_agent_links() {
    let temp = TestTempDir::new("maestro-global-skills-test");
    let repo = temp.path().join("repo");
    let home = temp.path().join("home");
    fs::create_dir(&repo).expect("invariant: repo should be creatable");
    fs::create_dir(&home).expect("invariant: home should be creatable");
    init_repo(&repo, &home);
    fs::write(
        repo.join(".maestro/skills/maestro-card/SKILL.md"),
        "repo-local edit must not feed global cache\n",
    )
    .expect("invariant: repo-local skill should be editable");

    let output = maestro(&["install", "--agent", "codex"], &repo, &home);

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("installed maestro codex integration"),
        "{stdout}"
    );
    assert!(!stdout.contains("global skills not synced"), "{stdout}");
    assert!(
        !stdout.contains("next: maestro sync --global-skills"),
        "{stdout}"
    );
    assert!(
        stdout.contains("global Maestro skills synced for all supported agents:"),
        "{stdout}"
    );
    assert!(stdout.contains(&format!(
        "cache: {}",
        home.join(".maestro/skills").display()
    )));
    assert!(stdout.contains(&format!(
        "codex root: {}",
        home.join(".agents/skills").display()
    )));
    assert!(stdout.contains(&format!(
        "claude root: {}",
        home.join(".claude/skills").display()
    )));
    assert!(stdout.contains("~/.codex/skills skipped"), "{stdout}");
    assert!(
        stdout.contains("resynced global cache to binary versions:"),
        "{stdout}"
    );
    assert!(stdout.contains("maestro-card  (new)"), "{stdout}");

    assert_eq!(
        fs::read_to_string(home.join(".maestro/skills/maestro-card/SKILL.md"))
            .expect("invariant: global task skill should be readable"),
        bundled_task_skill_md()
    );
    assert_symlink_target(
        &home.join(".agents/skills/maestro-card"),
        &home.join(".maestro/skills/maestro-card"),
    );
    assert_symlink_target(
        &home.join(".claude/skills/maestro-card"),
        &home.join(".maestro/skills/maestro-card"),
    );
    assert!(!home.join(".codex/skills/maestro-card").exists());

    let lock = fs::read_to_string(home.join(".maestro/skills-lock.yaml"))
        .expect("invariant: global lock should be readable");
    assert!(lock.contains("schema_version: maestro.global_skills_lock.v1"));
    assert!(lock.contains("codex:maestro-card"));
    assert!(lock.contains("display_path:"));
    assert!(lock.contains("resolved_path:"));

    assert!(repo.join(".codex/config.toml").is_file());
    assert!(repo.join(".codex/skills").is_symlink());
    assert!(!repo.join(".claude/settings.local.json").exists());

    let uninstall = maestro(&["uninstall", "--agent", "codex"], &repo, &home);
    assert_success(&uninstall);
    assert!(
        home.join(".maestro/skills/maestro-card/SKILL.md").is_file(),
        "repo-local uninstall must not remove global skills"
    );
}

#[test]
fn install_succeeds_with_a_warning_when_global_sync_hits_a_collision() {
    let temp = TestTempDir::new("maestro-global-skills-test");
    let repo = temp.path().join("repo");
    let home = temp.path().join("home");
    fs::create_dir(&repo).expect("invariant: repo should be creatable");
    fs::create_dir(&home).expect("invariant: home should be creatable");
    init_install_prereqs(&repo);
    fs::create_dir_all(home.join(".agents/skills"))
        .expect("invariant: global root should be creatable");
    fs::write(home.join(".agents/skills/maestro-card"), "user skill\n")
        .expect("invariant: collision should be writable");

    let output = maestro(&["install", "--agent", "codex"], &repo, &home);

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("installed maestro codex integration"),
        "{stdout}"
    );
    assert!(
        stdout.contains("warning: global skill sync failed"),
        "{stdout}"
    );
    assert!(stdout.contains("refusing global skill install"), "{stdout}");
    assert!(
        stdout.contains("rerun `maestro sync --global-skills`"),
        "{stdout}"
    );
    assert!(repo.join(".maestro/install-lock.yaml").exists());
    assert!(repo.join(".codex/config.toml").exists());
    assert!(repo.join(".codex/skills").is_symlink());
    assert!(
        !home.join(".maestro/skills/maestro-card/SKILL.md").exists(),
        "failed global sync must not leave cache writes behind"
    );
    assert_eq!(
        fs::read_to_string(home.join(".agents/skills/maestro-card"))
            .expect("invariant: collision should survive install"),
        "user skill\n"
    );

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
    assert!(sync_output.contains("maestro-card"), "{sync_output}");
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
        repo.join(".maestro/skills/maestro-card/SKILL.md"),
        "repo-local edit must remain\n",
    )
    .expect("invariant: repo-local skill should be editable");
    assert_success(&maestro(&["sync", "--global-skills"], &repo, &home));
    fs::remove_file(home.join(".maestro/skills/maestro-card/SKILL.md"))
        .expect("invariant: global skill should be removable");

    let output = maestro(&["sync", "--global-skills"], &repo, &home);

    assert_success(&output);
    assert_eq!(
        fs::read_to_string(home.join(".maestro/skills/maestro-card/SKILL.md"))
            .expect("invariant: global task skill should be readable"),
        bundled_task_skill_md()
    );
    assert_eq!(
        fs::read_to_string(repo.join(".maestro/skills/maestro-card/SKILL.md"))
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
    let global_task = home.join(".maestro/skills/maestro-card/SKILL.md");
    fs::remove_file(&global_task).expect("invariant: global skill should be removable");
    let retired_dir = home.join(".maestro/skills/maestro-retired");
    fs::create_dir_all(&retired_dir).expect("invariant: retired dir should be creatable");
    fs::write(retired_dir.join("SKILL.md"), "retired\n")
        .expect("invariant: retired skill should be writable");
    std::os::unix::fs::symlink(&retired_dir, home.join(".agents/skills/maestro-retired"))
        .expect("invariant: stale link should be creatable");

    let check = maestro(&["upgrade", "--check"], &repo, &home);

    assert_success(&check);
    assert!(
        !global_task.exists(),
        "update --check must not restore or mutate global skills"
    );
    assert!(
        retired_dir.exists(),
        "update --check must not prune retired skills"
    );

    let update = maestro(&["upgrade"], &repo, &home);

    assert_success(&update);
    assert_eq!(
        fs::read_to_string(&global_task).expect("invariant: global task skill should be readable"),
        bundled_task_skill_md()
    );
    let update_stdout = String::from_utf8_lossy(&update.stdout);
    assert!(
        update_stdout.contains("global Maestro skills synced for all supported agents:"),
        "update should report the global refresh\n{update_stdout}"
    );
    assert!(
        update_stdout.contains("pruned 1 retired skill(s): maestro-retired"),
        "{update_stdout}"
    );
    assert!(
        update_stdout.contains("pruned 1 stale skill link(s)"),
        "{update_stdout}"
    );
    assert!(!retired_dir.exists(), "upgrade should prune retired skills");
    assert!(
        fs::symlink_metadata(home.join(".agents/skills/maestro-retired")).is_err(),
        "upgrade should prune stale links"
    );
}
