use anyhow::Result;

use crate::domain::extraction;
use crate::domain::harness;
use crate::domain::install;
use crate::domain::skills;
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
    println!("installed maestro {} integration", agent.key());
    // A failed global sync must not fail the repo install that already landed;
    // warn and name the repair instead.
    match skills::sync_global_skills() {
        Ok(outcome) => print!("{}", skills::render_global_skills_outcome(&outcome)),
        Err(error) => {
            println!("warning: global skill sync failed: {error:#}");
            println!("repair, then rerun `maestro sync --global-skills`");
        }
    }
    match agent {
        install::InstallAgent::Claude => {
            println!("Claude hooks are active automatically; no approval step needed.");
        }
        install::InstallAgent::Codex => {
            println!("Run /hooks in Codex to approve the maestro hook.");
        }
        install::InstallAgent::Droid => {
            println!("Droid hooks were written to .factory/hooks.json.");
        }
    }

    Ok(())
}

impl From<Agent> for install::InstallAgent {
    fn from(agent: Agent) -> Self {
        match agent {
            Agent::Claude => install::InstallAgent::Claude,
            Agent::Codex => install::InstallAgent::Codex,
            Agent::Droid => install::InstallAgent::Droid,
        }
    }
}
