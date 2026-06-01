use anyhow::Result;

use crate::domain::install;
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::interfaces::cli::{Agent, AgentArgs};

/// Execute `maestro install --agent`.
pub fn run(args: AgentArgs) -> Result<()> {
    let agent = install::InstallAgent::from(args.agent);
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);
    install::install_agent(&paths, agent)?;

    if agent.requires_manual_hook_approval() {
        println!("Codex hook config written. Run /hooks in Codex to approve the maestro hook.");
    }

    Ok(())
}

impl From<Agent> for install::InstallAgent {
    fn from(agent: Agent) -> Self {
        match agent {
            Agent::Claude => install::InstallAgent::Claude,
            Agent::Codex => install::InstallAgent::Codex,
        }
    }
}
