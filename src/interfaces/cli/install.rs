use anyhow::{Context, Result};

use crate::foundation::core::paths::{discover_repo_root, MaestroPaths};
use crate::install::lock::{remove_lock_file, InstallLock};
use crate::install::mirrors::{prepare_mirrors, write_prepared_mirrors};
use crate::install::InstallAgent;
use crate::interfaces::cli::{Agent, AgentArgs};

/// Execute `maestro install --agent`.
pub fn run(args: AgentArgs) -> Result<()> {
    let agent = InstallAgent::from(args.agent);
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);
    let lock_path = paths.install_lock_file();
    let mut lock = InstallLock::load(&lock_path)?;
    let previous_install = lock.agents.get(agent.key()).cloned();
    let previous_lock = lock.clone();
    let previous_lock_existed = lock_path.exists();
    let prepared = prepare_mirrors(&paths, agent, timestamp(), previous_install.as_ref())?;
    lock.set_agent(agent, prepared.install.clone());
    lock.save(&lock_path)?;
    if let Err(error) = write_prepared_mirrors(&paths, &prepared) {
        restore_install_lock(&lock_path, &previous_lock, previous_lock_existed)?;
        return Err(error);
    }

    if agent == InstallAgent::Codex {
        println!("Codex hook config written. Run /hooks in Codex to approve the maestro hook.");
    }

    Ok(())
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

impl From<Agent> for InstallAgent {
    fn from(agent: Agent) -> Self {
        match agent {
            Agent::Claude => Self::Claude,
            Agent::Codex => Self::Codex,
        }
    }
}

fn timestamp() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .context("system clock is before the Unix epoch")
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}
