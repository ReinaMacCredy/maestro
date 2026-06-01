mod support;

use std::fs;
use std::process::Command;

use maestro::domain::install::{
    AgentInstall, FileOwnership, InstallAgent, InstallLock, InstallState, MirrorKind,
};
use maestro::foundation::core::hash::sha256_prefixed;
use maestro::foundation::core::managed_blocks::{ManagedBlockFormat, remove_managed_block};
use support::TestTempDir;

fn maestro(args: &[&str], cwd: &std::path::Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("invariant: compiled maestro binary should be runnable in install tests")
}

fn init_repo(repo: &std::path::Path) {
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

#[test]
fn install_claude_writes_managed_mirrors_and_lock() {
    let temp_dir = TestTempDir::new("maestro-install-cli-test");
    init_repo(temp_dir.path());
    fs::write(temp_dir.path().join("CLAUDE.md"), "# User\n")
        .expect("invariant: CLAUDE.md should be writable");

    let output = maestro(&["install", "--agent", "claude"], temp_dir.path());

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let claude = fs::read_to_string(temp_dir.path().join("CLAUDE.md"))
        .expect("invariant: CLAUDE.md should be readable");
    assert!(claude.contains("# User"));
    assert!(claude.contains("<!-- maestro:start -->"));
    assert!(temp_dir.path().join(".maestro/install-lock.yaml").is_file());
    assert!(
        temp_dir
            .path()
            .join(".claude/settings.local.json")
            .is_file()
    );
}

#[test]
fn install_defaults_to_codex_when_agent_is_omitted() {
    let temp_dir = TestTempDir::new("maestro-install-cli-test");
    init_repo(temp_dir.path());

    let output = maestro(&["install"], temp_dir.path());

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let lock = fs::read_to_string(temp_dir.path().join(".maestro/install-lock.yaml"))
        .expect("invariant: install lock should be readable");
    assert!(lock.contains("codex:"));
    assert!(temp_dir.path().join(".codex/config.toml").is_file());
}

#[test]
fn install_without_harness_fails_with_init_prerequisite() {
    let temp_dir = TestTempDir::new("maestro-install-cli-test");
    fs::create_dir(temp_dir.path().join(".git")).expect("invariant: .git marker should exist");

    let output = maestro(&["install", "--agent", "codex"], temp_dir.path());

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("run `maestro init` first"));
    assert!(!temp_dir.path().join("AGENTS.md").exists());
    assert!(!temp_dir.path().join(".maestro/install-lock.yaml").exists());
}

#[cfg(unix)]
#[test]
fn install_rejects_symlinked_harness_protocol_without_partial_writes() {
    let temp_dir = TestTempDir::new("maestro-install-cli-test");
    let external = TestTempDir::new("maestro-install-external-test");
    fs::create_dir(temp_dir.path().join(".git")).expect("invariant: .git marker should exist");
    fs::create_dir_all(temp_dir.path().join(".maestro/harness"))
        .expect("invariant: harness dir should be creatable");
    fs::write(external.path().join("HARNESS.md"), "# external harness\n")
        .expect("invariant: external harness should be writable");
    std::os::unix::fs::symlink(
        external.path().join("HARNESS.md"),
        temp_dir.path().join(".maestro/harness/HARNESS.md"),
    )
    .expect("invariant: symlinked harness should be creatable");

    let output = maestro(&["install", "--agent", "codex"], temp_dir.path());

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("symlink"));
    assert!(!temp_dir.path().join("AGENTS.md").exists());
    assert!(!temp_dir.path().join(".maestro/install-lock.yaml").exists());
}

#[test]
fn failed_install_does_not_write_partial_mirrors() {
    let temp_dir = TestTempDir::new("maestro-install-cli-test");
    init_repo(temp_dir.path());
    fs::create_dir_all(temp_dir.path().join(".claude"))
        .expect("invariant: claude config dir should be creatable");
    fs::write(temp_dir.path().join(".claude/settings.local.json"), "[]\n")
        .expect("invariant: invalid settings should be writable");

    let output = maestro(&["install", "--agent", "claude"], temp_dir.path());

    assert!(!output.status.success());
    assert!(!temp_dir.path().join("CLAUDE.md").exists());
    assert!(!temp_dir.path().join("AGENTS.md").exists());
    assert!(!temp_dir.path().join(".gitignore").exists());
    assert!(!temp_dir.path().join(".maestro/install-lock.yaml").exists());
}

#[cfg(unix)]
#[test]
fn install_rejects_symlinked_managed_directory_without_partial_writes() {
    let temp_dir = TestTempDir::new("maestro-install-cli-test");
    let external_dir = TestTempDir::new("maestro-install-external-test");
    init_repo(temp_dir.path());
    std::os::unix::fs::symlink(external_dir.path(), temp_dir.path().join(".codex"))
        .expect("invariant: symlinked codex dir should be creatable");

    let output = maestro(&["install", "--agent", "codex"], temp_dir.path());

    assert!(!output.status.success());
    assert!(!temp_dir.path().join("CLAUDE.md").exists());
    assert!(!temp_dir.path().join("AGENTS.md").exists());
    assert!(!temp_dir.path().join(".gitignore").exists());
    assert!(!temp_dir.path().join(".maestro/install-lock.yaml").exists());
    assert!(!external_dir.path().join("hooks.json").exists());
}

#[cfg(unix)]
#[test]
fn install_lock_save_failure_does_not_write_mirrors() {
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = TestTempDir::new("maestro-install-cli-test");
    init_repo(temp_dir.path());
    let maestro_dir = temp_dir.path().join(".maestro");
    let original_permissions = fs::metadata(&maestro_dir)
        .expect("invariant: maestro dir metadata should be readable")
        .permissions();
    fs::set_permissions(&maestro_dir, fs::Permissions::from_mode(0o555))
        .expect("invariant: permissions should be settable");

    let output = maestro(&["install", "--agent", "claude"], temp_dir.path());

    fs::set_permissions(&maestro_dir, original_permissions)
        .expect("invariant: permissions should be restorable");
    assert!(!output.status.success());
    assert!(!temp_dir.path().join("CLAUDE.md").exists());
    assert!(!temp_dir.path().join("AGENTS.md").exists());
    assert!(!temp_dir.path().join(".gitignore").exists());
    assert!(!temp_dir.path().join(".maestro/install-lock.yaml").exists());
}

#[cfg(unix)]
#[test]
fn install_write_failure_rolls_back_mirrors_and_lock() {
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = TestTempDir::new("maestro-install-cli-test");
    init_repo(temp_dir.path());
    fs::write(temp_dir.path().join("CLAUDE.md"), "# User Claude\n")
        .expect("invariant: CLAUDE.md should be writable");
    fs::create_dir(temp_dir.path().join(".claude"))
        .expect("invariant: claude dir should be creatable");
    let original_permissions = fs::metadata(temp_dir.path().join(".claude"))
        .expect("invariant: claude dir metadata should be readable")
        .permissions();
    fs::set_permissions(
        temp_dir.path().join(".claude"),
        fs::Permissions::from_mode(0o555),
    )
    .expect("invariant: permissions should be settable");

    let output = maestro(&["install", "--agent", "claude"], temp_dir.path());

    fs::set_permissions(temp_dir.path().join(".claude"), original_permissions)
        .expect("invariant: permissions should be restorable");
    assert!(!output.status.success());
    let claude = fs::read_to_string(temp_dir.path().join("CLAUDE.md"))
        .expect("invariant: CLAUDE.md should be readable");
    assert_eq!(claude, "# User Claude\n");
    assert!(!temp_dir.path().join("AGENTS.md").exists());
    assert!(!temp_dir.path().join(".gitignore").exists());
    assert!(!temp_dir.path().join(".maestro/install-lock.yaml").exists());
}

#[test]
fn install_codex_prints_manual_hooks_reminder() {
    let temp_dir = TestTempDir::new("maestro-install-cli-test");
    init_repo(temp_dir.path());

    let output = maestro(&["install", "--agent", "codex"], temp_dir.path());

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("invariant: stdout should be UTF-8");
    assert!(stdout.contains("--- a/.codex/hooks.json"));
    assert!(stdout.contains("+++ b/.codex/hooks.json"));
    assert!(stdout.contains("Run /hooks in Codex"));
    assert!(temp_dir.path().join(".codex/hooks.json").is_file());
}

#[test]
fn uninstall_removes_owned_blocks_and_preserves_user_content() {
    let temp_dir = TestTempDir::new("maestro-install-cli-test");
    init_repo(temp_dir.path());
    fs::write(temp_dir.path().join("AGENTS.md"), "# User\n")
        .expect("invariant: AGENTS.md should be writable");

    let install = maestro(&["install", "--agent", "codex"], temp_dir.path());
    assert!(install.status.success());
    let uninstall = maestro(&["uninstall", "--agent", "codex"], temp_dir.path());

    assert!(
        uninstall.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&uninstall.stderr)
    );
    let stdout = String::from_utf8(uninstall.stdout).expect("invariant: stdout should be UTF-8");
    assert!(stdout.contains("--- a/AGENTS.md"));
    assert!(stdout.contains("+++ b/AGENTS.md"));
    let agents = fs::read_to_string(temp_dir.path().join("AGENTS.md"))
        .expect("invariant: AGENTS.md should be readable");
    assert_eq!(agents, "# User\n");
    assert!(!temp_dir.path().join(".maestro/install-lock.yaml").exists());
}

#[test]
fn uninstall_groups_multiple_backups_in_one_operation_directory() {
    let temp_dir = TestTempDir::new("maestro-install-cli-test");
    init_repo(temp_dir.path());
    fs::write(temp_dir.path().join("CLAUDE.md"), "# User Claude\n")
        .expect("invariant: CLAUDE.md should be writable");
    fs::write(temp_dir.path().join("AGENTS.md"), "# User Agents\n")
        .expect("invariant: AGENTS.md should be writable");

    let install = maestro(&["install", "--agent", "claude"], temp_dir.path());
    assert!(install.status.success());
    fs::remove_dir_all(temp_dir.path().join(".maestro/backups"))
        .expect("invariant: install backups should be removable for test isolation");
    let uninstall = maestro(&["uninstall", "--agent", "claude"], temp_dir.path());

    assert!(
        uninstall.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&uninstall.stderr)
    );
    let backup_dirs = fs::read_dir(temp_dir.path().join(".maestro/backups"))
        .expect("invariant: backup root should exist")
        .collect::<Result<Vec<_>, _>>()
        .expect("invariant: backups should be readable");
    assert_eq!(backup_dirs.len(), 1);
    let backup_dir = backup_dirs[0].path();
    assert!(backup_dir.join("CLAUDE.md").is_file());
    assert!(backup_dir.join("AGENTS.md").is_file());
}

#[test]
fn uninstall_restores_preexisting_json_hooks() {
    let temp_dir = TestTempDir::new("maestro-install-cli-test");
    init_repo(temp_dir.path());
    fs::create_dir_all(temp_dir.path().join(".codex"))
        .expect("invariant: codex config dir should be creatable");
    let original_hooks = "{\n  \"hooks\": {\n    \"Stop\": []\n  },\n  \"user\": true\n}\n";
    fs::write(temp_dir.path().join(".codex/hooks.json"), original_hooks)
        .expect("invariant: hooks json should be writable");

    let install = maestro(&["install", "--agent", "codex"], temp_dir.path());
    assert!(install.status.success());
    let uninstall = maestro(&["uninstall", "--agent", "codex"], temp_dir.path());

    assert!(
        uninstall.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&uninstall.stderr)
    );
    let hooks = fs::read_to_string(temp_dir.path().join(".codex/hooks.json"))
        .expect("invariant: hooks json should be readable");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&hooks)
            .expect("invariant: restored hooks should parse"),
        serde_json::from_str::<serde_json::Value>(original_hooks)
            .expect("invariant: original hooks should parse")
    );
}

#[test]
fn uninstall_claude_removes_owned_hooks_and_preserves_user_json_keys() {
    let temp_dir = TestTempDir::new("maestro-install-cli-test");
    init_repo(temp_dir.path());
    fs::create_dir_all(temp_dir.path().join(".claude"))
        .expect("invariant: claude config dir should be creatable");
    fs::write(
        temp_dir.path().join(".claude/settings.local.json"),
        "{\n  \"user\": true\n}\n",
    )
    .expect("invariant: settings json should be writable");

    let install = maestro(&["install", "--agent", "claude"], temp_dir.path());
    assert!(install.status.success());
    let uninstall = maestro(&["uninstall", "--agent", "claude"], temp_dir.path());

    assert!(
        uninstall.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&uninstall.stderr)
    );
    let settings = fs::read_to_string(temp_dir.path().join(".claude/settings.local.json"))
        .expect("invariant: settings json should be readable");
    let parsed = serde_json::from_str::<serde_json::Value>(&settings)
        .expect("invariant: settings json should parse");
    assert_eq!(parsed.get("user"), Some(&serde_json::json!(true)));
    assert!(parsed.get("hooks").is_none());
    assert!(parsed.get("_maestro_managed_keys").is_none());
}

#[test]
fn uninstall_preserves_user_precreated_empty_json_instead_of_deleting_it() {
    // The user owned an empty `.claude/settings.local.json` (`{}`) before install.
    // maestro adds its hooks key, then uninstall strips it back to `{}`. The
    // residue is empty, but the file was NOT created by maestro, so uninstall must
    // restore it rather than delete the husk -- otherwise a routine uninstall
    // silently removes a file the user owned (the locked husk-safety rule).
    let temp_dir = TestTempDir::new("maestro-install-cli-test");
    init_repo(temp_dir.path());
    fs::create_dir_all(temp_dir.path().join(".claude"))
        .expect("invariant: claude config dir should be creatable");
    let settings_path = temp_dir.path().join(".claude/settings.local.json");
    fs::write(&settings_path, "{}\n").expect("invariant: empty settings should be writable");

    let install = maestro(&["install", "--agent", "claude"], temp_dir.path());
    assert!(install.status.success());
    let uninstall = maestro(&["uninstall", "--agent", "claude"], temp_dir.path());

    assert!(
        uninstall.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&uninstall.stderr)
    );
    assert!(
        settings_path.is_file(),
        "user's pre-existing settings.local.json must survive uninstall, not be husk-deleted"
    );
    let settings =
        fs::read_to_string(&settings_path).expect("invariant: settings json should be readable");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&settings)
            .expect("invariant: restored settings should parse"),
        serde_json::json!({})
    );
}

#[test]
fn uninstall_without_lock_does_not_remove_hook_config() {
    let temp_dir = TestTempDir::new("maestro-install-cli-test");
    init_repo(temp_dir.path());
    fs::create_dir_all(temp_dir.path().join(".codex"))
        .expect("invariant: codex config dir should be creatable");
    let hooks_path = temp_dir.path().join(".codex/hooks.json");
    let original_hooks = "{\n  \"_maestro_managed_keys\": [\"hooks\"],\n  \"hooks\": {\n    \"Stop\": []\n  },\n  \"user\": true\n}\n";
    fs::write(&hooks_path, original_hooks).expect("invariant: hooks json should be writable");

    let uninstall = maestro(&["uninstall", "--agent", "codex"], temp_dir.path());

    assert!(
        uninstall.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&uninstall.stderr)
    );
    let hooks = fs::read_to_string(hooks_path).expect("invariant: hooks json should be readable");
    assert_eq!(hooks, original_hooks);
}

#[test]
fn uninstall_targeting_a_not_installed_agent_reports_the_no_op() {
    let temp_dir = TestTempDir::new("maestro-install-cli-test");
    init_repo(temp_dir.path());

    let install = maestro(&["install", "--agent", "claude"], temp_dir.path());
    assert!(
        install.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&install.stderr)
    );

    // A bare `uninstall` defaults to codex; with only claude installed it must
    // surface the no-op rather than exiting silently and leaving hooks wired.
    let uninstall = maestro(&["uninstall", "--agent", "codex"], temp_dir.path());
    assert!(
        uninstall.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&uninstall.stderr)
    );
    let stdout = String::from_utf8(uninstall.stdout).expect("invariant: stdout should be UTF-8");
    assert!(
        stdout.contains("no maestro codex integration was installed"),
        "uninstall should report the no-op instead of staying silent: {stdout}"
    );
    let settings = fs::read_to_string(temp_dir.path().join(".claude/settings.local.json"))
        .expect("invariant: claude settings should remain readable");
    assert!(
        settings.contains("record.sh"),
        "claude hooks must remain wired after a codex-targeted no-op uninstall"
    );
}

#[test]
fn reinstall_preserves_json_restore_snapshot() {
    let temp_dir = TestTempDir::new("maestro-install-cli-test");
    init_repo(temp_dir.path());
    fs::create_dir_all(temp_dir.path().join(".codex"))
        .expect("invariant: codex config dir should be creatable");
    let original_hooks = "{\n  \"hooks\": {\n    \"Stop\": []\n  }\n}\n";
    fs::write(temp_dir.path().join(".codex/hooks.json"), original_hooks)
        .expect("invariant: hooks json should be writable");

    let first_install = maestro(&["install", "--agent", "codex"], temp_dir.path());
    assert!(first_install.status.success());
    let second_install = maestro(&["install", "--agent", "codex"], temp_dir.path());
    assert!(second_install.status.success());
    let uninstall = maestro(&["uninstall", "--agent", "codex"], temp_dir.path());

    assert!(
        uninstall.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&uninstall.stderr)
    );
    let hooks = fs::read_to_string(temp_dir.path().join(".codex/hooks.json"))
        .expect("invariant: hooks json should be readable");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&hooks)
            .expect("invariant: restored hooks should parse"),
        serde_json::from_str::<serde_json::Value>(original_hooks)
            .expect("invariant: original hooks should parse")
    );
}

#[test]
fn reinstall_replaces_pending_install_lock_and_commits_recovered_state() {
    let temp_dir = TestTempDir::new("maestro-install-cli-test");
    init_repo(temp_dir.path());
    let lock_path = temp_dir.path().join(".maestro/install-lock.yaml");
    let mut lock = InstallLock::empty();
    let mut install = AgentInstall::new("interrupted".to_string());
    install.mark_pending();
    install.insert(
        "AGENTS.md",
        FileOwnership::text(MirrorKind::MarkdownManagedBlock, "interrupted\n", false),
    );
    lock.set_agent(InstallAgent::Codex, install);
    lock.save(&lock_path)
        .expect("invariant: pending lock should be writable");

    let reinstall = maestro(&["install", "--agent", "codex"], temp_dir.path());

    assert!(
        reinstall.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&reinstall.stderr)
    );
    let recovered =
        InstallLock::load(&lock_path).expect("invariant: recovered install lock should load");
    assert_eq!(recovered.agents["codex"].state, InstallState::Committed);
    assert!(recovered.agents["codex"].files.contains_key("AGENTS.md"));
    assert!(temp_dir.path().join("AGENTS.md").is_file());
}

#[test]
fn reinstall_from_pending_after_mirror_writes_preserves_json_restore_snapshot() {
    let temp_dir = TestTempDir::new("maestro-install-cli-test");
    init_repo(temp_dir.path());
    fs::create_dir_all(temp_dir.path().join(".codex"))
        .expect("invariant: codex config dir should be creatable");
    let original_hooks = "{\n  \"hooks\": {\n    \"Stop\": []\n  }\n}\n";
    fs::write(temp_dir.path().join(".codex/hooks.json"), original_hooks)
        .expect("invariant: hooks json should be writable");

    let first_install = maestro(&["install", "--agent", "codex"], temp_dir.path());
    assert!(first_install.status.success());
    let lock_path = temp_dir.path().join(".maestro/install-lock.yaml");
    let mut pending_lock =
        InstallLock::load(&lock_path).expect("invariant: install lock should load");
    pending_lock
        .agents
        .get_mut("codex")
        .expect("invariant: codex install should exist")
        .mark_pending();
    pending_lock
        .save(&lock_path)
        .expect("invariant: pending lock should save");

    let reinstall = maestro(&["install", "--agent", "codex"], temp_dir.path());
    assert!(
        reinstall.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&reinstall.stderr)
    );
    let uninstall = maestro(&["uninstall", "--agent", "codex"], temp_dir.path());
    assert!(
        uninstall.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&uninstall.stderr)
    );
    let hooks = fs::read_to_string(temp_dir.path().join(".codex/hooks.json"))
        .expect("invariant: hooks json should be readable");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&hooks)
            .expect("invariant: restored hooks should parse"),
        serde_json::from_str::<serde_json::Value>(original_hooks)
            .expect("invariant: original hooks should parse")
    );
}

#[cfg(unix)]
#[test]
fn uninstall_retries_removing_state_after_mirrors_were_removed() {
    let temp_dir = TestTempDir::new("maestro-install-cli-test");
    init_repo(temp_dir.path());

    let install = maestro(&["install", "--agent", "codex"], temp_dir.path());
    assert!(install.status.success());
    let lock_path = temp_dir.path().join(".maestro/install-lock.yaml");
    let mut lock = InstallLock::load(&lock_path).expect("invariant: install lock should load");
    lock.agents
        .get_mut("codex")
        .expect("invariant: codex install should exist")
        .mark_removing();
    lock.save(&lock_path)
        .expect("invariant: removing lock should save");

    remove_managed_text(
        temp_dir.path().join("AGENTS.md"),
        ManagedBlockFormat::Markdown,
    );
    remove_managed_text(
        temp_dir.path().join("CLAUDE.md"),
        ManagedBlockFormat::Markdown,
    );
    remove_managed_text(
        temp_dir.path().join(".gitignore"),
        ManagedBlockFormat::HashComment,
    );
    remove_managed_text(
        temp_dir.path().join(".codex/config.toml"),
        ManagedBlockFormat::HashComment,
    );
    fs::write(temp_dir.path().join(".codex/hooks.json"), "{}\n")
        .expect("invariant: restored hooks json should be writable");
    fs::remove_file(temp_dir.path().join(".codex/skills"))
        .expect("invariant: removed skill symlink should be removable");

    let retry = maestro(&["uninstall", "--agent", "codex"], temp_dir.path());

    assert!(
        retry.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&retry.stderr)
    );
    assert!(!lock_path.exists());
    let agents = fs::read_to_string(temp_dir.path().join("AGENTS.md"))
        .expect("invariant: AGENTS.md should remain readable");
    assert!(!agents.contains("<!-- maestro:start -->"));
}

#[test]
fn uninstall_refuses_pending_install_lock_and_leaves_files_untouched() {
    let temp_dir = TestTempDir::new("maestro-install-cli-test");
    init_repo(temp_dir.path());
    fs::write(temp_dir.path().join("AGENTS.md"), "pending-owned\n")
        .expect("invariant: pending mirror should be writable");
    let lock_path = temp_dir.path().join(".maestro/install-lock.yaml");
    let mut lock = InstallLock::empty();
    let mut install = AgentInstall::new("interrupted".to_string());
    install.mark_pending();
    install.insert(
        "AGENTS.md",
        FileOwnership::text(MirrorKind::MarkdownManagedBlock, "pending-owned\n", false),
    );
    lock.set_agent(InstallAgent::Codex, install);
    lock.save(&lock_path)
        .expect("invariant: pending lock should be writable");

    let uninstall = maestro(&["uninstall", "--agent", "codex"], temp_dir.path());

    assert!(!uninstall.status.success());
    assert!(String::from_utf8_lossy(&uninstall.stderr).contains("pending codex install"));
    let agents = fs::read_to_string(temp_dir.path().join("AGENTS.md"))
        .expect("invariant: AGENTS.md should remain readable");
    assert_eq!(agents, "pending-owned\n");
    let preserved = InstallLock::load(&lock_path).expect("invariant: lock should remain readable");
    assert_eq!(preserved.agents["codex"].state, InstallState::Pending);
}

#[test]
fn uninstall_refuses_text_mirror_when_current_hash_differs_from_lock() {
    let temp_dir = TestTempDir::new("maestro-install-cli-test");
    init_repo(temp_dir.path());

    let install = maestro(&["install", "--agent", "codex"], temp_dir.path());
    assert!(install.status.success());
    let agents_path = temp_dir.path().join("AGENTS.md");
    let agents = fs::read_to_string(&agents_path)
        .expect("invariant: AGENTS.md should be readable after install");
    let agents = agents.replace(
        "Read .maestro/harness/HARNESS.md first before working in this repo.",
        "tampered managed block",
    );
    fs::write(&agents_path, agents).expect("invariant: AGENTS.md edit should be writable");

    let uninstall = maestro(&["uninstall", "--agent", "codex"], temp_dir.path());

    assert!(!uninstall.status.success());
    assert!(String::from_utf8_lossy(&uninstall.stderr).contains("current contents"));
    let agents_after =
        fs::read_to_string(&agents_path).expect("invariant: AGENTS.md should remain readable");
    assert!(agents_after.contains("<!-- maestro:start -->"));
    assert!(temp_dir.path().join(".maestro/install-lock.yaml").exists());
}

#[test]
fn uninstall_allows_user_edits_outside_text_mirror_managed_block() {
    let temp_dir = TestTempDir::new("maestro-install-cli-test");
    init_repo(temp_dir.path());
    fs::write(temp_dir.path().join("AGENTS.md"), "# User\n")
        .expect("invariant: AGENTS.md should be writable");

    let install = maestro(&["install", "--agent", "codex"], temp_dir.path());
    assert!(install.status.success());
    let agents_path = temp_dir.path().join("AGENTS.md");
    let mut agents = fs::read_to_string(&agents_path)
        .expect("invariant: AGENTS.md should be readable after install");
    agents.push_str("\n# user edit after install\n");
    fs::write(&agents_path, agents).expect("invariant: AGENTS.md edit should be writable");

    let uninstall = maestro(&["uninstall", "--agent", "codex"], temp_dir.path());

    assert!(
        uninstall.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&uninstall.stderr)
    );
    let agents_after =
        fs::read_to_string(&agents_path).expect("invariant: AGENTS.md should remain readable");
    assert!(!agents_after.contains("<!-- maestro:start -->"));
    assert!(agents_after.contains("# User"));
    assert!(agents_after.contains("# user edit after install"));
}

#[test]
fn uninstall_refuses_truncated_current_install_lock_and_leaves_files_untouched() {
    let temp_dir = TestTempDir::new("maestro-install-cli-test");
    init_repo(temp_dir.path());

    let install = maestro(&["install", "--agent", "codex"], temp_dir.path());
    assert!(install.status.success());
    let lock_path = temp_dir.path().join(".maestro/install-lock.yaml");
    let mut lock = InstallLock::load(&lock_path).expect("invariant: install lock should load");
    let codex = lock
        .agents
        .get_mut("codex")
        .expect("invariant: codex install should exist");
    codex
        .files
        .retain(|relative_path, _| relative_path == "AGENTS.md");
    lock.save(&lock_path)
        .expect("invariant: truncated lock should be writable");

    let uninstall = maestro(&["uninstall", "--agent", "codex"], temp_dir.path());

    assert!(!uninstall.status.success());
    assert!(String::from_utf8_lossy(&uninstall.stderr).contains("expected mirror set"));
    let agents_after =
        fs::read_to_string(temp_dir.path().join("AGENTS.md")).expect("AGENTS.md should remain");
    assert!(agents_after.contains("<!-- maestro:start -->"));
    assert!(temp_dir.path().join(".codex/hooks.json").exists());
    assert!(temp_dir.path().join(".codex/config.toml").exists());
    assert!(temp_dir.path().join(".maestro/install-lock.yaml").exists());
}

#[test]
fn uninstall_accepts_legacy_full_file_hash_when_managed_block_is_unchanged() {
    let temp_dir = TestTempDir::new("maestro-install-cli-test");
    init_repo(temp_dir.path());
    fs::write(temp_dir.path().join("AGENTS.md"), "# User\n")
        .expect("invariant: AGENTS.md should be writable");

    let install = maestro(&["install", "--agent", "codex"], temp_dir.path());
    assert!(install.status.success());
    let agents_path = temp_dir.path().join("AGENTS.md");
    let original_agents = fs::read_to_string(&agents_path)
        .expect("invariant: AGENTS.md should be readable after install");
    let lock_path = temp_dir.path().join(".maestro/install-lock.yaml");
    let mut lock = InstallLock::load(&lock_path).expect("invariant: install lock should load");
    lock.agents
        .get_mut("codex")
        .expect("invariant: codex install should exist")
        .files
        .get_mut("AGENTS.md")
        .expect("invariant: AGENTS.md ownership should exist")
        .content_hash = Some(legacy_text_hash(&original_agents));
    lock.save(&lock_path)
        .expect("invariant: legacy lock should be writable");
    let mut edited_agents = original_agents;
    edited_agents.push_str("\n# user edit after legacy install\n");
    fs::write(&agents_path, edited_agents).expect("invariant: AGENTS.md edit should be writable");

    let uninstall = maestro(&["uninstall", "--agent", "codex"], temp_dir.path());

    assert!(
        uninstall.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&uninstall.stderr)
    );
    let agents_after =
        fs::read_to_string(&agents_path).expect("invariant: AGENTS.md should remain readable");
    assert!(!agents_after.contains("<!-- maestro:start -->"));
    assert!(agents_after.contains("# User"));
    assert!(agents_after.contains("# user edit after legacy install"));
}

#[test]
fn uninstall_refuses_legacy_full_file_hash_when_managed_block_changed() {
    let temp_dir = TestTempDir::new("maestro-install-cli-test");
    init_repo(temp_dir.path());

    let install = maestro(&["install", "--agent", "codex"], temp_dir.path());
    assert!(install.status.success());
    let agents_path = temp_dir.path().join("AGENTS.md");
    let original_agents = fs::read_to_string(&agents_path)
        .expect("invariant: AGENTS.md should be readable after install");
    let lock_path = temp_dir.path().join(".maestro/install-lock.yaml");
    let mut lock = InstallLock::load(&lock_path).expect("invariant: install lock should load");
    lock.agents
        .get_mut("codex")
        .expect("invariant: codex install should exist")
        .files
        .get_mut("AGENTS.md")
        .expect("invariant: AGENTS.md ownership should exist")
        .content_hash = Some(legacy_text_hash(&original_agents));
    lock.save(&lock_path)
        .expect("invariant: legacy lock should be writable");
    let tampered_agents = original_agents.replace(
        "Read .maestro/harness/HARNESS.md first before working in this repo.",
        "tampered managed block",
    );
    fs::write(&agents_path, tampered_agents)
        .expect("invariant: AGENTS.md tamper should be writable");

    let uninstall = maestro(&["uninstall", "--agent", "codex"], temp_dir.path());

    assert!(!uninstall.status.success());
    assert!(String::from_utf8_lossy(&uninstall.stderr).contains("current contents"));
    let agents_after =
        fs::read_to_string(&agents_path).expect("invariant: AGENTS.md should remain readable");
    assert!(agents_after.contains("tampered managed block"));
    assert!(temp_dir.path().join(".maestro/install-lock.yaml").exists());
}

#[test]
fn uninstall_refuses_forged_strong_full_file_hash_when_managed_block_changed() {
    let temp_dir = TestTempDir::new("maestro-install-cli-test");
    init_repo(temp_dir.path());
    fs::write(temp_dir.path().join("AGENTS.md"), "# User\n")
        .expect("invariant: AGENTS.md should be writable");

    let install = maestro(&["install", "--agent", "codex"], temp_dir.path());
    assert!(install.status.success());
    let agents_path = temp_dir.path().join("AGENTS.md");
    let original_agents = fs::read_to_string(&agents_path)
        .expect("invariant: AGENTS.md should be readable after install");
    let tampered_agents = original_agents.replace(
        "Read .maestro/harness/HARNESS.md first before working in this repo.",
        "tampered managed block",
    );
    fs::write(&agents_path, &tampered_agents)
        .expect("invariant: AGENTS.md tamper should be writable");
    let lock_path = temp_dir.path().join(".maestro/install-lock.yaml");
    let mut lock = InstallLock::load(&lock_path).expect("invariant: install lock should load");
    lock.agents
        .get_mut("codex")
        .expect("invariant: codex install should exist")
        .files
        .get_mut("AGENTS.md")
        .expect("invariant: AGENTS.md ownership should exist")
        .content_hash = Some(sha256_prefixed(tampered_agents.as_bytes()));
    lock.save(&lock_path)
        .expect("invariant: forged strong lock should be writable");

    let uninstall = maestro(&["uninstall", "--agent", "codex"], temp_dir.path());

    assert!(!uninstall.status.success());
    assert!(String::from_utf8_lossy(&uninstall.stderr).contains("current contents"));
    let agents_after =
        fs::read_to_string(&agents_path).expect("invariant: AGENTS.md should remain readable");
    assert!(agents_after.contains("tampered managed block"));
    assert!(agents_after.contains("<!-- maestro:start -->"));
    assert!(temp_dir.path().join(".maestro/install-lock.yaml").exists());
}

#[test]
fn reinstall_does_not_bless_forged_json_restore_snapshot() {
    let temp_dir = TestTempDir::new("maestro-install-cli-test");
    init_repo(temp_dir.path());
    fs::create_dir_all(temp_dir.path().join(".codex"))
        .expect("invariant: codex config dir should be creatable");
    let original_hooks = "{\n  \"hooks\": {\n    \"Stop\": []\n  }\n}\n";
    fs::write(temp_dir.path().join(".codex/hooks.json"), original_hooks)
        .expect("invariant: hooks json should be writable");

    let first_install = maestro(&["install", "--agent", "codex"], temp_dir.path());
    assert!(first_install.status.success());
    fs::write(
        temp_dir.path().join(".maestro/install-lock.yaml"),
        "schema_version: maestro.install_lock.v1\nagents:\n  codex:\n    installed_at: \"2026-05-25T10:00:00Z\"\n    files:\n      .codex/hooks.json:\n        kind: json_managed_keys\n        managed_keys:\n          - hooks\n        previous_values:\n          hooks:\n            Stop:\n              - matcher: \"*\"\n                hooks:\n                  - type: command\n                    command: forged\n      .codex/config.toml:\n        kind: toml_section\n        content_hash: \"len:1:sum:0000000000000000\"\n      AGENTS.md:\n        kind: markdown_managed_block\n        content_hash: \"len:1:sum:0000000000000000\"\n      CLAUDE.md:\n        kind: markdown_managed_block\n        content_hash: \"len:1:sum:0000000000000000\"\n      .gitignore:\n        kind: gitignore_section\n        content_hash: \"len:1:sum:0000000000000000\"\n",
    )
    .expect("invariant: forged lock should be writable");

    let reinstall = maestro(&["install", "--agent", "codex"], temp_dir.path());
    assert!(!reinstall.status.success());
    assert!(
        String::from_utf8_lossy(&reinstall.stderr)
            .contains("managed JSON restore metadata does not match install lock")
    );
    let hooks = fs::read_to_string(temp_dir.path().join(".codex/hooks.json"))
        .expect("invariant: hooks json should be readable");
    assert!(!hooks.contains("forged"));
    assert!(hooks.contains("_maestro_previous_value_hashes"));
}

#[cfg(unix)]
#[test]
fn uninstall_handles_legacy_symlink_lock_entries() {
    let temp_dir = TestTempDir::new("maestro-install-cli-test");
    init_repo(temp_dir.path());
    fs::create_dir_all(temp_dir.path().join(".codex"))
        .expect("invariant: codex dir should be creatable");
    fs::create_dir_all(temp_dir.path().join(".maestro/skills"))
        .expect("invariant: skills dir should be creatable");
    std::os::unix::fs::symlink("../.maestro/skills", temp_dir.path().join(".codex/skills"))
        .expect("invariant: legacy symlink should be creatable");
    fs::create_dir_all(temp_dir.path().join(".maestro"))
        .expect("invariant: maestro dir should be creatable");
    fs::write(
        temp_dir.path().join(".maestro/install-lock.yaml"),
        "schema_version: maestro.install_lock.v1\nagents:\n  codex:\n    installed_at: \"2026-05-25T10:00:00Z\"\n    files:\n      .codex/skills:\n        kind: symlink\n        target: ../.maestro/skills\n",
    )
    .expect("invariant: install lock should be writable");

    let uninstall = maestro(&["uninstall", "--agent", "codex"], temp_dir.path());

    assert!(
        uninstall.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&uninstall.stderr)
    );
    assert!(!temp_dir.path().join(".codex/skills").exists());
    assert!(!temp_dir.path().join(".maestro/install-lock.yaml").exists());
}

#[cfg(unix)]
#[test]
fn uninstall_rejects_forged_symlink_path_outside_repo() {
    let temp_dir = TestTempDir::new("maestro-install-cli-test");
    let external_dir = TestTempDir::new("maestro-install-external-test");
    init_repo(temp_dir.path());
    fs::create_dir_all(temp_dir.path().join(".maestro"))
        .expect("invariant: maestro dir should be creatable");
    std::os::unix::fs::symlink(
        temp_dir.path().join(".maestro"),
        external_dir.path().join("external-skills"),
    )
    .expect("invariant: external symlink should be creatable");
    fs::write(
        temp_dir.path().join(".maestro/install-lock.yaml"),
        "schema_version: maestro.install_lock.v1\nagents:\n  codex:\n    installed_at: \"2026-05-25T10:00:00Z\"\n    files:\n      ../maestro-install-external-test/external-skills:\n        kind: symlink\n        target: ../.maestro/skills\n",
    )
    .expect("invariant: install lock should be writable");

    let uninstall = maestro(&["uninstall", "--agent", "codex"], temp_dir.path());

    assert!(!uninstall.status.success());
    assert!(external_dir.path().join("external-skills").exists());
}

#[test]
fn uninstall_rejects_unexpected_repo_local_lock_entry() {
    let temp_dir = TestTempDir::new("maestro-install-cli-test");
    init_repo(temp_dir.path());
    fs::create_dir_all(temp_dir.path().join(".maestro"))
        .expect("invariant: maestro dir should be creatable");
    fs::write(
        temp_dir.path().join("README.md"),
        "# User\n\n<!-- maestro:start -->\nforged\n<!-- maestro:end -->\n",
    )
    .expect("invariant: README should be writable");
    fs::write(
        temp_dir.path().join(".maestro/install-lock.yaml"),
        "schema_version: maestro.install_lock.v1\nagents:\n  codex:\n    installed_at: \"2026-05-25T10:00:00Z\"\n    files:\n      README.md:\n        kind: markdown_managed_block\n        content_hash: \"len:1:sum:0000000000000000\"\n",
    )
    .expect("invariant: install lock should be writable");

    let uninstall = maestro(&["uninstall", "--agent", "codex"], temp_dir.path());

    assert!(!uninstall.status.success());
    let readme = fs::read_to_string(temp_dir.path().join("README.md"))
        .expect("invariant: README should be readable");
    assert!(readme.contains("forged"));
    assert!(temp_dir.path().join(".maestro/install-lock.yaml").exists());
}

#[test]
fn uninstall_rejects_unexpected_json_keys_in_lock_entry() {
    let temp_dir = TestTempDir::new("maestro-install-cli-test");
    init_repo(temp_dir.path());
    fs::create_dir_all(temp_dir.path().join(".maestro"))
        .expect("invariant: maestro dir should be creatable");
    fs::create_dir_all(temp_dir.path().join(".codex"))
        .expect("invariant: codex dir should be creatable");
    fs::write(
        temp_dir.path().join(".codex/hooks.json"),
        "{\n  \"hooks\": {},\n  \"user\": true,\n  \"_maestro_managed_keys\": [\"hooks\"]\n}\n",
    )
    .expect("invariant: hooks should be writable");
    fs::write(
        temp_dir.path().join(".maestro/install-lock.yaml"),
        "schema_version: maestro.install_lock.v1\nagents:\n  codex:\n    installed_at: \"2026-05-25T10:00:00Z\"\n    files:\n      .codex/hooks.json:\n        kind: json_managed_keys\n        managed_keys:\n          - hooks\n          - user\n        previous_values:\n          user: false\n",
    )
    .expect("invariant: install lock should be writable");

    let uninstall = maestro(&["uninstall", "--agent", "codex"], temp_dir.path());

    assert!(!uninstall.status.success());
    let hooks = fs::read_to_string(temp_dir.path().join(".codex/hooks.json"))
        .expect("invariant: hooks should be readable");
    assert!(hooks.contains("\"user\": true"));
    assert!(temp_dir.path().join(".maestro/install-lock.yaml").exists());
}

#[test]
fn uninstall_rejects_forged_json_previous_value() {
    let temp_dir = TestTempDir::new("maestro-install-cli-test");
    init_repo(temp_dir.path());
    fs::create_dir_all(temp_dir.path().join(".maestro"))
        .expect("invariant: maestro dir should be creatable");
    fs::create_dir_all(temp_dir.path().join(".codex"))
        .expect("invariant: codex dir should be creatable");
    let original = "{\n  \"hooks\": {},\n  \"_maestro_managed_keys\": [\"hooks\"]\n}\n";
    fs::write(temp_dir.path().join(".codex/hooks.json"), original)
        .expect("invariant: hooks should be writable");
    fs::write(
        temp_dir.path().join(".maestro/install-lock.yaml"),
        "schema_version: maestro.install_lock.v1\nagents:\n  codex:\n    installed_at: \"2026-05-25T10:00:00Z\"\n    files:\n      .codex/hooks.json:\n        kind: json_managed_keys\n        managed_keys:\n          - hooks\n        previous_values:\n          hooks:\n            Stop:\n              - matcher: \"*\"\n                hooks:\n                  - type: command\n                    command: forged\n",
    )
    .expect("invariant: install lock should be writable");

    let uninstall = maestro(&["uninstall", "--agent", "codex"], temp_dir.path());

    assert!(!uninstall.status.success());
    let hooks = fs::read_to_string(temp_dir.path().join(".codex/hooks.json"))
        .expect("invariant: hooks should be readable");
    assert_eq!(hooks, original);
    assert!(temp_dir.path().join(".maestro/install-lock.yaml").exists());
}

#[cfg(unix)]
#[test]
fn uninstall_rejects_symlinked_managed_directory() {
    let temp_dir = TestTempDir::new("maestro-install-cli-test");
    let external_dir = TestTempDir::new("maestro-install-external-test");
    init_repo(temp_dir.path());
    fs::create_dir_all(temp_dir.path().join(".maestro"))
        .expect("invariant: maestro dir should be creatable");
    fs::write(external_dir.path().join("hooks.json"), "{\"hooks\": {}}\n")
        .expect("invariant: external hooks should be writable");
    std::os::unix::fs::symlink(external_dir.path(), temp_dir.path().join(".codex"))
        .expect("invariant: symlinked codex dir should be creatable");
    fs::write(
        temp_dir.path().join(".maestro/install-lock.yaml"),
        "schema_version: maestro.install_lock.v1\nagents:\n  codex:\n    installed_at: \"2026-05-25T10:00:00Z\"\n    files:\n      .codex/hooks.json:\n        kind: json_managed_keys\n        managed_keys:\n          - hooks\n",
    )
    .expect("invariant: install lock should be writable");

    let uninstall = maestro(&["uninstall", "--agent", "codex"], temp_dir.path());

    assert!(!uninstall.status.success());
    let hooks = fs::read_to_string(external_dir.path().join("hooks.json"))
        .expect("invariant: external hooks should be readable");
    assert_eq!(hooks, "{\"hooks\": {}}\n");
    assert!(temp_dir.path().join(".maestro/install-lock.yaml").exists());
}

#[test]
fn uninstall_one_agent_preserves_shared_files_owned_by_remaining_agent() {
    let temp_dir = TestTempDir::new("maestro-install-cli-test");
    init_repo(temp_dir.path());

    let claude = maestro(&["install", "--agent", "claude"], temp_dir.path());
    assert!(claude.status.success());
    let codex = maestro(&["install", "--agent", "codex"], temp_dir.path());
    assert!(codex.status.success());
    let uninstall = maestro(&["uninstall", "--agent", "claude"], temp_dir.path());

    assert!(
        uninstall.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&uninstall.stderr)
    );
    let agents = fs::read_to_string(temp_dir.path().join("AGENTS.md"))
        .expect("invariant: AGENTS.md should remain readable");
    assert!(agents.contains("<!-- maestro:start -->"));
    let lock = fs::read_to_string(temp_dir.path().join(".maestro/install-lock.yaml"))
        .expect("invariant: install lock should remain");
    assert!(lock.contains("codex:"));
    assert!(!lock.contains("claude:"));
}

fn legacy_text_hash(content: &str) -> String {
    let byte_sum = content
        .as_bytes()
        .iter()
        .fold(0_u64, |sum, byte| sum.wrapping_add(u64::from(*byte)));
    format!("len:{}:sum:{:016x}", content.len(), byte_sum)
}

fn remove_managed_text(path: std::path::PathBuf, format: ManagedBlockFormat) {
    let contents = fs::read_to_string(&path).expect("invariant: managed mirror should be readable");
    fs::write(path, remove_managed_block(&contents, format))
        .expect("invariant: managed mirror should be writable");
}
