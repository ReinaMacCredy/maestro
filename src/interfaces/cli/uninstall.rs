use anyhow::Result;

use crate::domain::install;
use crate::foundation::core::paths::{discover_repo_root, MaestroPaths};
use crate::interfaces::cli::AgentArgs;

/// Execute `maestro uninstall --agent`.
pub fn run(args: AgentArgs) -> Result<()> {
    let agent = install::InstallAgent::from(args.agent);
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);
    install::uninstall_agent(&paths, agent)?;

    Ok(())
}
