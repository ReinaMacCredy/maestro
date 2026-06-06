use anyhow::Result;

use crate::domain::extraction;
use crate::domain::harness;
use crate::domain::install;
use crate::foundation::core::paths::{MaestroPaths, announce_repo_root, discover_repo_root};
use crate::interfaces::cli::{Agent, AgentArgs};

/// Execute `maestro install --agent`.
pub fn run(args: AgentArgs) -> Result<()> {
    let agent = install::InstallAgent::from(args.agent());
    let repo_root = discover_repo_root()?;
    announce_repo_root(&repo_root);
    let paths = MaestroPaths::new(repo_root);
    harness::ensure_harness_protocol_exists(&paths)?;
    extraction::ensure_hook_script_exists(&paths)?;
    install::install_agent(&paths, agent)?;
    // The mirror writes above print their diffs; close with a uniform success
    // line plus the per-agent next step so both agents end the same way (T6.4).
    println!("installed maestro {} integration (repo only)", agent.key());
    println!("global skills not synced; agents resolve repo-local skills first");
    println!("next: maestro sync --global-skills");
    if agent.requires_manual_hook_approval() {
        println!("Run /hooks in Codex to approve the maestro hook.");
    } else {
        println!("Claude hooks are active automatically; no approval step needed.");
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
