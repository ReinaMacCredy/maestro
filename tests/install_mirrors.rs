mod support;

use std::collections::BTreeSet;
use std::fs;

use maestro::core::paths::MaestroPaths;
use maestro::install::lock::{AgentInstall, FileOwnership, InstallLock, MirrorKind};
use maestro::install::mirrors::{apply_mirrors, mirror_plan, remove_mirrors};
use maestro::install::InstallAgent;
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
    assert_hook_shape(&hook_plan.contents, false);
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
            && plan.contents.contains("maestro hook record")
    }));
    let hook_plan = plans
        .iter()
        .find(|plan| plan.relative_path == ".codex/hooks.json")
        .expect("invariant: Codex hook plan should exist");
    assert_eq!(hook_plan.managed_keys, vec!["hooks"]);
    assert_hook_shape(&hook_plan.contents, true);
}

#[test]
fn apply_mirrors_preserves_user_content_and_records_ownership() {
    let temp_dir = TestTempDir::new("maestro-install-test");
    fs::create_dir(temp_dir.path().join(".git")).expect("invariant: git marker should be writable");
    fs::write(temp_dir.path().join("CLAUDE.md"), "# User\n")
        .expect("invariant: user CLAUDE.md should be writable");
    let paths = MaestroPaths::new(temp_dir.path().to_path_buf());

    let install = apply_mirrors(
        &paths,
        InstallAgent::Claude,
        "2026-05-25T10:00:00Z".to_string(),
    )
    .expect("invariant: mirrors should apply");

    let claude = fs::read_to_string(temp_dir.path().join("CLAUDE.md"))
        .expect("invariant: CLAUDE.md should be readable");
    assert!(claude.starts_with("# User\n"));
    assert!(claude.contains("<!-- maestro:start -->"));
    assert!(install.files.contains_key("CLAUDE.md"));
    assert!(matches!(
        install.files[".claude/settings.local.json"].kind,
        MirrorKind::JsonManagedKeys
    ));
}

#[cfg(unix)]
#[test]
fn apply_mirrors_creates_skill_symlink_and_records_ownership() {
    let temp_dir = TestTempDir::new("maestro-install-test");
    fs::create_dir(temp_dir.path().join(".git")).expect("invariant: git marker should be writable");
    let paths = MaestroPaths::new(temp_dir.path().to_path_buf());

    let install = apply_mirrors(
        &paths,
        InstallAgent::Claude,
        "2026-05-25T10:00:00Z".to_string(),
    )
    .expect("invariant: mirrors should apply");

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
    fs::create_dir(temp_dir.path().join(".git")).expect("invariant: git marker should be writable");
    fs::create_dir_all(temp_dir.path().join(".codex/skills"))
        .expect("invariant: user skill tree should be writable");
    let paths = MaestroPaths::new(temp_dir.path().to_path_buf());

    let error = apply_mirrors(
        &paths,
        InstallAgent::Codex,
        "2026-05-25T10:00:00Z".to_string(),
    )
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
    fs::create_dir(temp_dir.path().join(".git")).expect("invariant: git marker should be writable");
    fs::write(temp_dir.path().join("CLAUDE.md"), "# User Claude\n")
        .expect("invariant: user CLAUDE.md should be writable");
    fs::write(temp_dir.path().join("AGENTS.md"), "# User Agents\n")
        .expect("invariant: user AGENTS.md should be writable");
    let paths = MaestroPaths::new(temp_dir.path().to_path_buf());

    apply_mirrors(
        &paths,
        InstallAgent::Claude,
        "2026-05-25T10:00:00Z".to_string(),
    )
    .expect("invariant: mirrors should apply");

    let backup_root = temp_dir.path().join(".maestro/backups");
    let backup_dirs = fs::read_dir(&backup_root)
        .expect("invariant: backup root should exist")
        .collect::<Result<Vec<_>, _>>()
        .expect("invariant: backups should be readable");
    assert_eq!(backup_dirs.len(), 1);
    assert!(backup_dirs[0].path().join("CLAUDE.md").is_file());
    assert!(backup_dirs[0].path().join("AGENTS.md").is_file());

    apply_mirrors(
        &paths,
        InstallAgent::Claude,
        "2026-05-25T10:00:01Z".to_string(),
    )
    .expect("invariant: no-op mirrors should reapply");

    let backup_dirs_after_noop = fs::read_dir(&backup_root)
        .expect("invariant: backup root should still exist")
        .count();
    assert_eq!(backup_dirs_after_noop, 1);
}

#[test]
fn remove_mirrors_removes_only_owned_content() {
    let temp_dir = TestTempDir::new("maestro-install-test");
    fs::create_dir(temp_dir.path().join(".git")).expect("invariant: git marker should be writable");
    fs::write(temp_dir.path().join("AGENTS.md"), "# User\n")
        .expect("invariant: user AGENTS.md should be writable");
    let paths = MaestroPaths::new(temp_dir.path().to_path_buf());
    let install = apply_mirrors(
        &paths,
        InstallAgent::Codex,
        "2026-05-25T10:00:00Z".to_string(),
    )
    .expect("invariant: mirrors should apply");

    remove_mirrors(&paths, InstallAgent::Codex, &install, &BTreeSet::new())
        .expect("invariant: mirrors should uninstall");

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
    fs::create_dir(temp_dir.path().join(".git")).expect("invariant: git marker should be writable");
    fs::create_dir_all(temp_dir.path().join(".codex"))
        .expect("invariant: codex dir should be writable");
    let hooks_path = temp_dir.path().join(".codex/hooks.json");
    fs::write(
        &hooks_path,
        "{\n  \"_maestro_managed_keys\": [\"hooks\"],\n  \"hooks\": {\"Stop\": []}\n}\n",
    )
    .expect("invariant: hooks should be writable");
    let paths = MaestroPaths::new(temp_dir.path().to_path_buf());

    let install = apply_mirrors(
        &paths,
        InstallAgent::Codex,
        "2026-05-25T10:00:00Z".to_string(),
    )
    .expect("invariant: mirrors should apply");
    remove_mirrors(&paths, InstallAgent::Codex, &install, &BTreeSet::new())
        .expect("invariant: mirrors should uninstall");

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
}

fn assert_hook_shape(contents: &str, expect_timeout: bool) {
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
            Some(&serde_json::json!("maestro hook record"))
        );
        if expect_timeout {
            assert_eq!(command.get("timeout"), Some(&serde_json::json!(5)));
        } else {
            assert!(command.get("timeout").is_none());
        }
    }
}
