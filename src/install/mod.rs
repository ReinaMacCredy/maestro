//! Temporary compatibility shim for the moved Install domain.

pub use crate::domain::install::{
    install_agent, mirror_plan, uninstall_agent, AgentInstall, FileOwnership, InstallAgent,
    InstallLock, InstallState, MirrorKind, MirrorPlan,
};

pub mod lock {
    use std::fs;
    use std::path::Path;

    use anyhow::{Context, Result};

    pub use crate::domain::install::{
        AgentInstall, FileOwnership, InstallLock, InstallState, MirrorKind,
    };

    /// Remove the lockfile if it exists.
    pub fn remove_lock_file(path: &Path) -> Result<()> {
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error)
                .with_context(|| format!("failed to remove install lock {}", path.display())),
        }
    }
}

pub mod mirrors {
    use std::collections::BTreeSet;

    use anyhow::Result;

    use crate::foundation::core::paths::MaestroPaths;

    use super::{AgentInstall, InstallAgent};

    pub use crate::domain::install::{mirror_plan, MirrorPlan};

    /// Compatibility wrapper for the former mirror writer.
    pub fn apply_mirrors(
        paths: &MaestroPaths,
        agent: InstallAgent,
        installed_at: String,
    ) -> Result<AgentInstall> {
        crate::domain::install::apply_mirrors_for_compat(paths, agent, installed_at)
    }

    /// Compatibility wrapper for the former mirror remover.
    pub fn remove_mirrors(
        paths: &MaestroPaths,
        agent: InstallAgent,
        install: &AgentInstall,
        still_owned_paths: &BTreeSet<String>,
    ) -> Result<()> {
        crate::domain::install::remove_mirrors_for_compat(paths, agent, install, still_owned_paths)
    }
}
