mod support;

use std::fs;
use std::process::Command;

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
    assert!(temp_dir
        .path()
        .join(".claude/settings.local.json")
        .is_file());
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
    fs::create_dir(&maestro_dir).expect("invariant: maestro dir should be creatable");
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
    assert!(reinstall.status.success());
    let uninstall = maestro(&["uninstall", "--agent", "codex"], temp_dir.path());
    assert!(uninstall.status.success());
    let hooks = fs::read_to_string(temp_dir.path().join(".codex/hooks.json"))
        .expect("invariant: hooks json should be readable");
    assert!(!hooks.contains("forged"));
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
