use anyhow::Result;

use crate::domain::harness::backlog;
use crate::domain::harness::{BacklogConfig, BacklogItem};
use crate::foundation::core::paths::MaestroPaths;

use super::detect;

/// Refresh rule-based proposals into the backlog and return the full backlog.
pub fn refresh(paths: &MaestroPaths) -> Result<BacklogConfig> {
    let proposals = detect::detect(paths)?;
    backlog::refresh(paths, proposals)
}

/// Apply a backlog proposal by marking it applied.
pub fn apply(paths: &MaestroPaths, id: &str) -> Result<BacklogItem> {
    let proposals = detect::detect(paths)?;
    let mut backlog = backlog::load(paths)?;
    backlog::merge_proposals(&mut backlog, proposals);
    let applied = backlog::mark_applied(&mut backlog, id)?;
    backlog::save(paths, &backlog)?;
    Ok(applied)
}
