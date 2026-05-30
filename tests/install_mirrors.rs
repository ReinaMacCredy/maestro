mod support;

use std::fs;

use maestro::domain::install::{
    install_agent, mirror_plan, uninstall_agent, AgentInstall, FileOwnership, InstallAgent,
    InstallLock, InstallState, MirrorKind,
};
use maestro::foundation::core::error::MaestroError;
use maestro::foundation::core::paths::MaestroPaths;
use support::TestTempDir;

const HOOK_EVENTS: [&str; 6] = [
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PermissionRequest",
    "PostToolUse",
    "Stop",
];

#[test]
fn mirror_plan_writes_managed_content_for_claude() {
    let plans = mirror_plan(InstallAgent::Claude).expect("invariant: mirror plan should build");

    assert!(plans.iter().any(|plan| {
        plan.relative_path == "CLAUDE.md" && plan.contents.contains("@.maestro/harness/HARNESS.md")
    }));
    let gitignore_plan = plans
        .iter()
        .find(|plan| plan.relative_path == ".gitignore")
        .expect("invariant: gitignore plan should exist");
    assert!(gitignore_plan.contents.contains(".claude/skills"));
    assert!(gitignore_plan.contents.contains(".codex/skills"));
    assert!(plans.iter().any(|plan| {
        plan.relative_path == ".claude/settings.local.json"
            && plan.contents.contains("\"_maestro_managed_keys\"")
            && plan.contents.contains("\"hooks\"")
    }));
    let hook_plan = plans
        .iter()
        .find(|plan| plan.relative_path == ".claude/settings.local.json")
        .expect("invariant: Claude hook plan should exist");
    assert_eq!(hook_plan.managed_keys, vec!["hooks"]);
    assert_hook_shape(
        &hook_plan.contents,
        false,
        "sh \"$CLAUDE_PROJECT_DIR/.maestro/hooks/record.sh\"",
    );
}

#[test]
fn mirror_plan_writes_codex_hook_timeout_and_trust_related_files() {
    let plans = mirror_plan(InstallAgent::Codex).expect("invariant: mirror plan should build");

    assert!(plans.iter().any(|plan| {
        plan.relative_path == "AGENTS.md"
            && plan
                .contents
                .contains("Read .maestro/harness/HARNESS.md first")
    }));
    assert!(plans.iter().any(|plan| {
        plan.relative_path == ".codex/hooks.json"
            && plan.contents.contains("\"timeout\": 5")
            && plan.contents.contains(".maestro/hooks/record.sh")
    }));
    let hook_plan = plans
        .iter()
        .find(|plan| plan.relative_path == ".codex/hooks.json")
        .expect("invariant: Codex hook plan should exist");
    assert_eq!(hook_plan.managed_keys, vec!["hooks"]);
    assert_hook_shape(
        &hook_plan.contents,
        true,
        "sh \"$(git rev-parse --show-toplevel)/.maestro/hooks/record.sh\"",
    );
}

#[test]
fn apply_mirrors_preserves_user_content_and_records_ownership() {
    let temp_dir = TestTempDir::new("maestro-install-test");
    init_repo(temp_dir.path());
    fs::write(temp_dir.path().join("CLAUDE.md"), "# User\n")
        .expect("invariant: user CLAUDE.md should be writable");
    let paths = MaestroPaths::new(temp_dir.path().to_path_buf());

    install_agent(&paths, InstallAgent::Claude).expect("invariant: mirrors should apply");
    let lock =
        InstallLock::load(&paths.install_lock_file()).expect("invariant: install lock should load");
    let install = &lock.agents["claude"];

    let claude = fs::read_to_string(temp_dir.path().join("CLAUDE.md"))
        .expect("invariant: CLAUDE.md should be readable");
    assert!(claude.starts_with("# User\n"));
    assert!(claude.contains("<!-- maestro:start -->"));
    assert!(install.files.contains_key("CLAUDE.md"));
    assert!(install.files["CLAUDE.md"]
        .content_hash
        .as_deref()
        .is_some_and(|hash| hash.starts_with("sha256:")));
    assert!(matches!(
        install.files[".claude/settings.local.json"].kind,
        MirrorKind::JsonManagedKeys
    ));
}

#[cfg(unix)]
#[test]
fn apply_mirrors_creates_skill_symlink_and_records_ownership() {
    let temp_dir = TestTempDir::new("maestro-install-test");
    init_repo(temp_dir.path());
    let paths = MaestroPaths::new(temp_dir.path().to_path_buf());

    install_agent(&paths, InstallAgent::Claude).expect("invariant: mirrors should apply");
    let lock =
        InstallLock::load(&paths.install_lock_file()).expect("invariant: install lock should load");
    let install = &lock.agents["claude"];

    let target = fs::read_link(temp_dir.path().join(".claude/skills"))
        .expect("invariant: Claude skills mirror should be a symlink");
    assert_eq!(target, std::path::Path::new("../.maestro/skills"));
    let ownership = install
        .files
        .get(".claude/skills")
        .expect("invariant: skill symlink ownership should be recorded");
    assert!(matches!(ownership.kind, MirrorKind::Symlink));
    assert_eq!(ownership.target.as_deref(), Some("../.maestro/skills"));
}

#[cfg(unix)]
#[test]
fn apply_mirrors_refuses_existing_user_skill_tree() {
    let temp_dir = TestTempDir::new("maestro-install-test");
    init_repo(temp_dir.path());
    fs::create_dir_all(temp_dir.path().join(".codex/skills"))
        .expect("invariant: user skill tree should be writable");
    let paths = MaestroPaths::new(temp_dir.path().to_path_buf());

    let error = install_agent(&paths, InstallAgent::Codex)
        .expect_err("existing user skill tree should make install fail");

    assert!(error
        .to_string()
        .contains("refusing to overwrite existing .codex/skills"));
    assert!(temp_dir.path().join(".codex/skills").is_dir());
    assert!(!temp_dir.path().join("AGENTS.md").exists());
}

#[test]
fn apply_mirrors_uses_one_backup_directory_per_operation_and_skips_noop_reapply() {
    let temp_dir = TestTempDir::new("maestro-install-test");
    init_repo(temp_dir.path());
    fs::write(temp_dir.path().join("CLAUDE.md"), "# User Claude\n")
        .expect("invariant: user CLAUDE.md should be writable");
    fs::write(temp_dir.path().join("AGENTS.md"), "# User Agents\n")
        .expect("invariant: user AGENTS.md should be writable");
    let paths = MaestroPaths::new(temp_dir.path().to_path_buf());

    install_agent(&paths, InstallAgent::Claude).expect("invariant: mirrors should apply");

    let backup_root = temp_dir.path().join(".maestro/backups");
    let backup_dirs = fs::read_dir(&backup_root)
        .expect("invariant: backup root should exist")
        .collect::<Result<Vec<_>, _>>()
        .expect("invariant: backups should be readable");
    assert_eq!(backup_dirs.len(), 1);
    assert!(backup_dirs[0].path().join("CLAUDE.md").is_file());
    assert!(backup_dirs[0].path().join("AGENTS.md").is_file());

    install_agent(&paths, InstallAgent::Claude).expect("invariant: no-op mirrors should reapply");

    let backup_dirs_after_noop = fs::read_dir(&backup_root)
        .expect("invariant: backup root should still exist")
        .count();
    assert_eq!(backup_dirs_after_noop, 1);
}

#[test]
fn remove_mirrors_removes_only_owned_content() {
    let temp_dir = TestTempDir::new("maestro-install-test");
    init_repo(temp_dir.path());
    fs::write(temp_dir.path().join("AGENTS.md"), "# User\n")
        .expect("invariant: user AGENTS.md should be writable");
    let paths = MaestroPaths::new(temp_dir.path().to_path_buf());
    install_agent(&paths, InstallAgent::Codex).expect("invariant: mirrors should apply");
    uninstall_agent(&paths, InstallAgent::Codex).expect("invariant: mirrors should uninstall");

    let agents = fs::read_to_string(temp_dir.path().join("AGENTS.md"))
        .expect("invariant: AGENTS.md should be readable");
    assert_eq!(agents, "# User\n");
    let hooks = fs::read_to_string(temp_dir.path().join(".codex/hooks.json"))
        .expect("invariant: hooks json should be readable");
    assert_eq!(hooks, "{}\n");
}

#[test]
fn apply_mirrors_snapshots_preexisting_key_even_with_stale_manifest() {
    let temp_dir = TestTempDir::new("maestro-install-test");
    init_repo(temp_dir.path());
    fs::create_dir_all(temp_dir.path().join(".codex"))
        .expect("invariant: codex dir should be writable");
    let hooks_path = temp_dir.path().join(".codex/hooks.json");
    fs::write(
        &hooks_path,
        "{\n  \"_maestro_managed_keys\": [\"hooks\"],\n  \"hooks\": {\"Stop\": []}\n}\n",
    )
    .expect("invariant: hooks should be writable");
    let paths = MaestroPaths::new(temp_dir.path().to_path_buf());

    install_agent(&paths, InstallAgent::Codex).expect("invariant: mirrors should apply");
    uninstall_agent(&paths, InstallAgent::Codex).expect("invariant: mirrors should uninstall");

    let hooks = fs::read_to_string(hooks_path).expect("invariant: hooks should be readable");
    assert!(hooks.contains("\"Stop\""));
}

#[test]
fn install_lock_round_trips_agent_ownership() {
    let temp_dir = TestTempDir::new("maestro-install-test");
    let lock_path = temp_dir.path().join(".maestro/install-lock.yaml");
    let mut lock = InstallLock::empty();
    let mut install = AgentInstall::new("2026-05-25T10:00:00Z".to_string());
    install.insert(
        "CLAUDE.md",
        FileOwnership::text(MirrorKind::MarkdownManagedBlock, "managed"),
    );
    install.insert(
        ".claude/settings.local.json",
        FileOwnership::json_keys(vec!["hooks".to_string()], Default::default()),
    );
    lock.set_agent(InstallAgent::Claude, install);

    lock.save(&lock_path)
        .expect("invariant: install lock should save");
    let loaded = InstallLock::load(&lock_path).expect("invariant: install lock should load");

    assert_eq!(loaded, lock);
}

#[test]
fn install_lock_rejects_schema_mismatch() {
    let temp_dir = TestTempDir::new("maestro-install-test");
    let lock_path = temp_dir.path().join(".maestro/install-lock.yaml");
    fs::create_dir_all(
        lock_path
            .parent()
            .expect("invariant: lock path should have parent"),
    )
    .expect("invariant: lock parent should be writable");
    fs::write(
        &lock_path,
        "schema_version: maestro.install_lock.v2\nagents: {}\n",
    )
    .expect("invariant: lock should be writable");

    let error = InstallLock::load(&lock_path).expect_err("schema mismatch should fail");

    assert!(error.to_string().contains("schema mismatch"));
    assert!(
        matches!(
            error.downcast_ref::<MaestroError>(),
            Some(MaestroError::SchemaMismatch { .. })
        ),
        "install-lock gate must stay a hard MaestroError::SchemaMismatch, got: {error}"
    );
}

#[test]
fn install_lock_rejects_unknown_schema_version() {
    // The install-lock gate is a non-migratable write path: an unknown /
    // unparseable version classifies as Incompatible and must stop hard.
    let temp_dir = TestTempDir::new("maestro-install-test");
    let lock_path = temp_dir.path().join(".maestro/install-lock.yaml");
    fs::create_dir_all(
        lock_path
            .parent()
            .expect("invariant: lock path should have parent"),
    )
    .expect("invariant: lock parent should be writable");
    fs::write(&lock_path, "schema_version: totally-bogus\nagents: {}\n")
        .expect("invariant: lock should be writable");

    let error = InstallLock::load(&lock_path).expect_err("unknown schema version should fail");

    assert!(
        matches!(
            error.downcast_ref::<MaestroError>(),
            Some(MaestroError::SchemaMismatch { .. })
        ),
        "unknown install-lock version must stop hard, got: {error}"
    );
}

#[test]
fn install_lock_defaults_legacy_agent_state_to_committed() {
    let temp_dir = TestTempDir::new("maestro-install-test");
    let lock_path = temp_dir.path().join(".maestro/install-lock.yaml");
    fs::create_dir_all(
        lock_path
            .parent()
            .expect("invariant: lock path should have parent"),
    )
    .expect("invariant: lock parent should be writable");
    fs::write(
        &lock_path,
        "schema_version: maestro.install_lock.v1\nagents:\n  codex:\n    installed_at: old\n    files: {}\n",
    )
    .expect("invariant: legacy lock should be writable");

    let loaded = InstallLock::load(&lock_path).expect("invariant: legacy lock should load");

    assert_eq!(loaded.agents["codex"].state, InstallState::Committed);
}

fn assert_hook_shape(contents: &str, expect_timeout: bool, expected_command: &str) {
    let value = serde_json::from_str::<serde_json::Value>(contents)
        .expect("invariant: hook mirror should be valid JSON");
    let hooks = value
        .get("hooks")
        .and_then(serde_json::Value::as_object)
        .expect("invariant: hooks should be an object");

    assert_eq!(hooks.len(), HOOK_EVENTS.len());
    for event in HOOK_EVENTS {
        let entry = hooks
            .get(event)
            .and_then(serde_json::Value::as_array)
            .and_then(|entries| entries.first())
            .expect("invariant: hook entry should exist");
        assert_eq!(entry.get("matcher"), Some(&serde_json::json!("*")));
        let command = entry
            .get("hooks")
            .and_then(serde_json::Value::as_array)
            .and_then(|commands| commands.first())
            .expect("invariant: hook command should exist");

        assert_eq!(command.get("type"), Some(&serde_json::json!("command")));
        assert_eq!(
            command.get("command"),
            Some(&serde_json::json!(expected_command))
        );
        if expect_timeout {
            assert_eq!(command.get("timeout"), Some(&serde_json::json!(5)));
        } else {
            assert!(command.get("timeout").is_none());
        }
    }
}

fn init_repo(repo: &std::path::Path) {
    fs::create_dir(repo.join(".git")).expect("invariant: git marker should be writable");
    fs::create_dir_all(repo.join(".maestro/harness"))
        .expect("invariant: harness dir should be writable");
    fs::write(
        repo.join(".maestro/harness/HARNESS.md"),
        "# Maestro Harness Protocol\n",
    )
    .expect("invariant: harness protocol should be writable");
    fs::create_dir_all(repo.join(".maestro/hooks"))
        .expect("invariant: hooks dir should be writable");
    fs::write(
        repo.join(".maestro/hooks/record.sh"),
        "# maestro:hook-version: 1.0.0\nexec maestro hook record\n",
    )
    .expect("invariant: hook recorder script should be writable");
}
