use std::path::PathBuf;

use anyhow::Result;

use crate::domain::run;
use crate::foundation::core::paths::MaestroPaths;

/// List all managed `.maestro/runs/**/events.jsonl` files.
pub fn managed_event_files(paths: &MaestroPaths) -> Result<Vec<PathBuf>> {
    Ok(run::managed_event_logs(paths)?
        .into_iter()
        .map(|log| log.path().to_path_buf())
        .collect())
}
