use std::collections::BTreeSet;

use anyhow::Result;

use crate::commands::AgentArgs;
use crate::foundation::core::paths::{discover_repo_root, MaestroPaths};
use crate::install::lock::{remove_lock_file, InstallLock};
use crate::install::mirrors::remove_mirrors;
use crate::install::InstallAgent;

/// Execute `maestro uninstall --agent`.
pub fn run(args: AgentArgs) -> Result<()> {
    let agent = InstallAgent::from(args.agent);
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);
    let lock_path = paths.install_lock_file();
    let mut lock = InstallLock::load(&lock_path)?;

    if let Some(install) = lock.agents.get(agent.key()).cloned() {
        let still_owned_paths = still_owned_paths(&lock, agent);
        remove_mirrors(&paths, agent, &install, &still_owned_paths)?;
        lock.remove_agent(agent);
    }

    if lock.agents.is_empty() {
        remove_lock_file(&lock_path)?;
    } else {
        lock.save(&lock_path)?;
    }

    Ok(())
}

fn still_owned_paths(lock: &InstallLock, removed_agent: InstallAgent) -> BTreeSet<String> {
    lock.agents
        .iter()
        .filter(|(agent, _)| agent.as_str() != removed_agent.key())
        .flat_map(|(_, install)| install.files.keys().cloned())
        .collect()
}
