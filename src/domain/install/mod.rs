use std::collections::BTreeSet;

use anyhow::{bail, Context, Result};

use crate::domain::harness;
use crate::domain::skills::symlink::{SkillSymlink, SKILLS_SYMLINK_TARGET};
use crate::foundation::core::paths::MaestroPaths;

mod hooks;
mod lock;
mod mirrors;

pub use lock::{AgentInstall, FileOwnership, InstallLock, InstallState, MirrorKind};
pub use mirrors::{mirror_plan, MirrorPlan};

use lock::remove_lock_file;
use mirrors::{prepare_mirrors, write_prepared_mirrors};

/// Agent integrations supported by V1 install.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstallAgent {
    /// Claude Code.
    Claude,
    /// Codex CLI.
    Codex,
}

impl InstallAgent {
    /// Stable lockfile key for the agent.
    pub fn key(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
        }
    }

    /// Whether this agent requires manual hook approval after install.
    pub fn requires_manual_hook_approval(self) -> bool {
        self == Self::Codex
    }
}

/// Install one agent integration into the repository.
pub fn install_agent(paths: &MaestroPaths, agent: InstallAgent) -> Result<()> {
    install_agent_with_writer(paths, agent, write_prepared_mirrors)
}

fn install_agent_with_writer<F>(
    paths: &MaestroPaths,
    agent: InstallAgent,
    write_mirrors: F,
) -> Result<()>
where
    F: FnOnce(
        &MaestroPaths,
        &mirrors::PreparedMirrors,
    ) -> std::result::Result<(), mirrors::MirrorWriteFailure>,
{
    harness::ensure_harness_protocol_exists(paths)?;
    let lock_path = paths.install_lock_file();
    let mut lock = InstallLock::load(&lock_path)?;
    let previous_install = lock.agents.get(agent.key()).cloned();
    let previous_lock = lock.clone();
    let previous_lock_existed = lock_path.exists();
    let prepared = prepare_mirrors(paths, agent, timestamp()?, previous_install.as_ref())?;

    let mut pending_install = prepared.install.clone();
    pending_install.mark_pending();
    lock.set_agent(agent, pending_install);
    lock.save(&lock_path)?;

    if let Err(error) = write_mirrors(paths, &prepared) {
        if error.rollback_completed() {
            restore_install_lock(&lock_path, &previous_lock, previous_lock_existed)?;
        }
        return Err(error.into_error());
    }

    let mut committed_install = prepared.install;
    committed_install.mark_committed();
    lock.set_agent(agent, committed_install);
    lock.save(&lock_path)?;

    Ok(())
}

/// Uninstall one agent integration from the repository.
pub fn uninstall_agent(paths: &MaestroPaths, agent: InstallAgent) -> Result<()> {
    uninstall_agent_with_finalizer(paths, agent, finalize_uninstall_lock)
}

fn uninstall_agent_with_finalizer<F>(
    paths: &MaestroPaths,
    agent: InstallAgent,
    finalize_lock: F,
) -> Result<()>
where
    F: FnOnce(&std::path::Path, &InstallLock) -> Result<()>,
{
    let lock_path = paths.install_lock_file();
    let mut lock = InstallLock::load(&lock_path)?;

    let Some(install) = lock.agents.get(agent.key()).cloned() else {
        return finalize_lock(&lock_path, &lock);
    };

    let previous_lock = lock.clone();
    ensure_uninstallable_install(agent, &install)?;
    if install.state == InstallState::Committed {
        let mut removing_install = install.clone();
        removing_install.mark_removing();
        lock.set_agent(agent, removing_install);
        lock.save(&lock_path)?;
    }
    let still_owned_paths = still_owned_paths(&lock, agent);
    let removal = match mirrors::remove_mirrors(paths, agent, &install, &still_owned_paths) {
        Ok(removal) => removal,
        Err(error) => {
            if install.state == InstallState::Committed {
                return Err(restore_lock_after_failed_uninstall_start(
                    &lock_path,
                    &previous_lock,
                    error,
                ));
            }
            return Err(error);
        }
    };
    lock.remove_agent(agent);

    if let Err(error) = finalize_lock(&lock_path, &lock) {
        return rollback_uninstall_after_lock_failure(&lock_path, &previous_lock, removal, error);
    }

    Ok(())
}

fn finalize_uninstall_lock(lock_path: &std::path::Path, lock: &InstallLock) -> Result<()> {
    if lock.agents.is_empty() {
        remove_lock_file(lock_path)?;
    } else {
        lock.save(lock_path)?;
    }

    Ok(())
}

fn rollback_uninstall_after_lock_failure(
    lock_path: &std::path::Path,
    previous_lock: &InstallLock,
    removal: mirrors::MirrorRemovalRollback,
    error: anyhow::Error,
) -> Result<()> {
    let mut rollback_errors = Vec::new();
    if let Err(rollback_error) = removal.rollback() {
        rollback_errors.push(format!(
            "failed to roll back uninstalled mirrors: {rollback_error}"
        ));
    }
    if let Err(restore_error) = previous_lock.save(lock_path) {
        rollback_errors.push(format!("failed to restore install lock: {restore_error}"));
    }
    if rollback_errors.is_empty() {
        return Err(error);
    }

    bail!("{}; additionally {}", error, rollback_errors.join("; "))
}

pub(crate) fn ensure_uninstallable_install(
    agent: InstallAgent,
    install: &AgentInstall,
) -> Result<()> {
    if install.state == InstallState::Pending {
        bail!(
            "refusing to trust pending {} install ownership; rerun install to recover or replace it",
            agent.key()
        );
    }

    Ok(())
}

pub(crate) fn skill_symlink_for_agent(agent: InstallAgent) -> SkillSymlink {
    match agent {
        InstallAgent::Claude => SkillSymlink {
            relative_path: ".claude/skills",
            target: SKILLS_SYMLINK_TARGET,
        },
        InstallAgent::Codex => SkillSymlink {
            relative_path: ".codex/skills",
            target: SKILLS_SYMLINK_TARGET,
        },
    }
}

fn restore_install_lock(
    lock_path: &std::path::Path,
    previous_lock: &InstallLock,
    previous_lock_existed: bool,
) -> Result<()> {
    if previous_lock_existed {
        previous_lock.save(lock_path)
    } else {
        remove_lock_file(lock_path)
    }
}

fn restore_lock_after_failed_uninstall_start(
    lock_path: &std::path::Path,
    previous_lock: &InstallLock,
    error: anyhow::Error,
) -> anyhow::Error {
    match previous_lock.save(lock_path) {
        Ok(()) => error,
        Err(restore_error) => anyhow::anyhow!(
            "{}; additionally failed to restore install lock after removal failure: {}",
            error,
            restore_error
        ),
    }
}

fn still_owned_paths(lock: &InstallLock, removed_agent: InstallAgent) -> BTreeSet<String> {
    lock.agents
        .iter()
        .filter(|(agent, _)| agent.as_str() != removed_agent.key())
        .flat_map(|(_, install)| install.files.keys().cloned())
        .collect()
}

fn timestamp() -> Result<String> {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .context("system clock is before the Unix epoch")?;
    Ok(duration.as_secs().to_string())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    use anyhow::anyhow;

    use super::*;

    #[test]
    fn rollback_failure_preserves_pending_install_lock() {
        let root = temp_repo("maestro-install-pending-lock-test");
        let paths = MaestroPaths::new(root.clone());

        let error = install_agent_with_writer(&paths, InstallAgent::Codex, |_paths, _prepared| {
            Err(mirrors::MirrorWriteFailure::rollback_failed(
                anyhow!("mirror write failed"),
                anyhow!("rollback failed"),
            ))
        })
        .expect_err("invariant: simulated rollback failure should fail install");

        assert!(error.to_string().contains("rollback failed"));
        let lock = InstallLock::load(&paths.install_lock_file())
            .expect("invariant: pending lock should remain readable");
        assert_eq!(lock.agents["codex"].state, InstallState::Pending);

        fs::remove_dir_all(root).expect("invariant: temp repo should be removable");
    }

    #[test]
    fn final_lock_save_failure_rolls_back_removed_agent_mirrors_and_lock() {
        let root = temp_repo("maestro-install-final-save-rollback-test");
        let paths = MaestroPaths::new(root.clone());
        install_agent(&paths, InstallAgent::Claude).expect("invariant: Claude install should pass");
        install_agent(&paths, InstallAgent::Codex).expect("invariant: Codex install should pass");
        let lock_path = paths.install_lock_file();
        let lock_before =
            fs::read_to_string(&lock_path).expect("invariant: install lock should be readable");
        let claude_settings_path = root.join(".claude/settings.local.json");
        let claude_settings_before = fs::read_to_string(&claude_settings_path)
            .expect("invariant: Claude settings mirror should be readable");

        let error = uninstall_agent_with_finalizer(&paths, InstallAgent::Claude, |_path, _lock| {
            Err(anyhow!("simulated final lock save failure"))
        })
        .expect_err("invariant: simulated final lock save failure should fail uninstall");

        assert!(error
            .to_string()
            .contains("simulated final lock save failure"));
        assert_eq!(
            fs::read_to_string(&lock_path).expect("invariant: install lock should remain"),
            lock_before
        );
        assert_eq!(
            fs::read_to_string(&claude_settings_path)
                .expect("invariant: Claude settings mirror should be restored"),
            claude_settings_before
        );
        assert_eq!(
            fs::read_link(root.join(".claude/skills"))
                .expect("invariant: Claude skill symlink should be restored"),
            Path::new("../.maestro/skills")
        );

        fs::remove_dir_all(root).expect("invariant: temp repo should be removable");
    }

    #[test]
    fn final_lock_remove_failure_rolls_back_last_agent_mirrors_and_lock() {
        let root = temp_repo("maestro-install-final-remove-rollback-test");
        let paths = MaestroPaths::new(root.clone());
        install_agent(&paths, InstallAgent::Codex).expect("invariant: Codex install should pass");
        let lock_path = paths.install_lock_file();
        let lock_before =
            fs::read_to_string(&lock_path).expect("invariant: install lock should be readable");
        let agents_path = root.join("AGENTS.md");
        let agents_before =
            fs::read_to_string(&agents_path).expect("invariant: AGENTS.md should be readable");
        let codex_config_path = root.join(".codex/config.toml");
        let codex_config_before = fs::read_to_string(&codex_config_path)
            .expect("invariant: Codex config mirror should be readable");

        let error = uninstall_agent_with_finalizer(&paths, InstallAgent::Codex, |_path, _lock| {
            Err(anyhow!("simulated final lock remove failure"))
        })
        .expect_err("invariant: simulated final lock remove failure should fail uninstall");

        assert!(error
            .to_string()
            .contains("simulated final lock remove failure"));
        assert_eq!(
            fs::read_to_string(&lock_path).expect("invariant: install lock should remain"),
            lock_before
        );
        assert_eq!(
            fs::read_to_string(&agents_path)
                .expect("invariant: AGENTS.md mirror should be restored"),
            agents_before
        );
        assert_eq!(
            fs::read_to_string(&codex_config_path)
                .expect("invariant: Codex config mirror should be restored"),
            codex_config_before
        );
        assert_eq!(
            fs::read_link(root.join(".codex/skills"))
                .expect("invariant: Codex skill symlink should be restored"),
            Path::new("../.maestro/skills")
        );

        fs::remove_dir_all(root).expect("invariant: temp repo should be removable");
    }

    #[cfg(unix)]
    #[test]
    fn final_lock_failure_restores_file_mirrors_even_when_symlink_rollback_fails() {
        let root = temp_repo("maestro-install-independent-rollback-test");
        let paths = MaestroPaths::new(root.clone());
        install_agent(&paths, InstallAgent::Codex).expect("invariant: Codex install should pass");
        let agents_path = root.join("AGENTS.md");
        let agents_before =
            fs::read_to_string(&agents_path).expect("invariant: AGENTS.md should be readable");

        let error = uninstall_agent_with_finalizer(&paths, InstallAgent::Codex, |_path, _lock| {
            fs::create_dir(root.join(".codex/skills"))
                .expect("invariant: symlink rollback conflict should be creatable");
            Err(anyhow!("simulated final lock remove failure"))
        })
        .expect_err("invariant: simulated final lock remove failure should fail uninstall");

        let message = error.to_string();
        assert!(message.contains("simulated final lock remove failure"));
        assert!(message.contains("failed to roll back symlinks"));
        assert_eq!(
            fs::read_to_string(&agents_path)
                .expect("invariant: AGENTS.md mirror should be restored"),
            agents_before
        );

        fs::remove_dir_all(root).expect("invariant: temp repo should be removable");
    }

    #[cfg(unix)]
    #[test]
    fn removing_retry_preserves_user_edited_restored_json_after_finalizer_failure() {
        let root = temp_repo("maestro-install-json-removing-retry-test");
        let paths = MaestroPaths::new(root.clone());
        fs::create_dir_all(root.join(".codex"))
            .expect("invariant: Codex config dir should be creatable");
        let hooks_path = root.join(".codex/hooks.json");
        let original_hooks = r#"{
  "hooks": {
    "Stop": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "user stop"
          }
        ]
      }
    ]
  },
  "user": true
}
"#;
        fs::write(&hooks_path, original_hooks)
            .expect("invariant: original hooks should be writable");
        install_agent(&paths, InstallAgent::Codex).expect("invariant: Codex install should pass");

        let lock_path = paths.install_lock_file();
        let mut lock = InstallLock::load(&lock_path).expect("invariant: install lock should load");
        lock.agents
            .get_mut("codex")
            .expect("invariant: codex install should exist")
            .mark_removing();
        lock.save(&lock_path)
            .expect("invariant: removing lock should save");

        let claude_path = root.join("CLAUDE.md");
        let error = uninstall_agent_with_finalizer(&paths, InstallAgent::Codex, |_path, _lock| {
            fs::remove_file(&claude_path)
                .expect("invariant: rollback conflict file should be removable");
            fs::create_dir(&claude_path)
                .expect("invariant: rollback conflict dir should be creatable");
            Err(anyhow!("simulated final lock failure"))
        })
        .expect_err("invariant: simulated final lock failure should fail uninstall");

        let message = error.to_string();
        assert!(message.contains("simulated final lock failure"));
        assert!(message.contains("failed to roll back uninstalled mirrors"));
        fs::remove_dir(&claude_path).expect("invariant: rollback conflict dir should be removable");
        fs::write(&claude_path, "# user Claude after interrupted uninstall\n")
            .expect("invariant: conflict path should be repairable");

        let interrupted =
            InstallLock::load(&lock_path).expect("invariant: interrupted lock should load");
        assert_eq!(interrupted.agents["codex"].state, InstallState::Removing);
        let restored_hooks = read_json(&hooks_path);
        assert!(restored_hooks.get("_maestro_managed_keys").is_none());
        assert!(restored_hooks
            .get("_maestro_previous_value_hashes")
            .is_none());
        let original_hooks_json: serde_json::Value =
            serde_json::from_str(original_hooks).expect("invariant: original hooks should parse");
        assert_eq!(
            restored_hooks.get("hooks"),
            original_hooks_json.get("hooks")
        );

        let edited_hooks = r#"{
  "hooks": {
    "Stop": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "user edited stop"
          }
        ]
      }
    ]
  },
  "user": "edited"
}
"#;
        fs::write(&hooks_path, edited_hooks).expect("invariant: edited hooks should be writable");

        uninstall_agent(&paths, InstallAgent::Codex)
            .expect("invariant: removing retry should complete");

        assert!(!lock_path.exists());
        assert_eq!(
            read_json(&hooks_path),
            serde_json::from_str::<serde_json::Value>(edited_hooks)
                .expect("invariant: edited hooks should parse")
        );

        fs::remove_dir_all(root).expect("invariant: temp repo should be removable");
    }

    fn read_json(path: &Path) -> serde_json::Value {
        let contents = fs::read_to_string(path).expect("invariant: JSON file should be readable");
        serde_json::from_str(&contents).expect("invariant: JSON file should parse")
    }

    fn temp_repo(prefix: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("invariant: test clock should be after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("{prefix}-{nanos}"));
        fs::create_dir(&root).expect("invariant: temp repo should be creatable");
        fs::create_dir_all(root.join(".maestro/harness"))
            .expect("invariant: harness dir should be creatable");
        fs::write(
            root.join(".maestro/harness/HARNESS.md"),
            "# Maestro Harness Protocol\n",
        )
        .expect("invariant: harness protocol should be writable");
        root
    }
}
