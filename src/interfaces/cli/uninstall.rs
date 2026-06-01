use anyhow::Result;

use crate::domain::install::{self, UninstallOutcome};
use crate::foundation::core::paths::{MaestroPaths, announce_repo_root, discover_repo_root};
use crate::interfaces::cli::AgentArgs;

/// Execute `maestro uninstall --agent`.
pub fn run(args: AgentArgs) -> Result<()> {
    let agent = install::InstallAgent::from(args.agent);
    let repo_root = discover_repo_root()?;
    announce_repo_root(&repo_root);
    let paths = MaestroPaths::new(repo_root);
    match install::uninstall_agent(&paths, agent)? {
        UninstallOutcome::Removed => {
            println!("uninstalled maestro {} integration", agent.key());
        }
        UninstallOutcome::NotInstalled => {
            println!(
                "no maestro {} integration was installed; nothing to uninstall",
                agent.key()
            );
        }
    }

    Ok(())
}
