use std::collections::BTreeSet;
use std::fs;
use std::io::ErrorKind;

use anyhow::{bail, Context, Result};

use crate::core::managed_path::{managed_path, SymlinkPolicy};
use crate::core::paths::MaestroPaths;
use crate::core::safe_write::write_string_atomic;
use crate::core::schema::BACKLOG_SCHEMA_VERSION;
use crate::harness::schema::{BacklogConfig, BacklogItem};
use crate::improver::detect;

/// Refresh rule-based proposals into the backlog and return the full backlog.
pub fn refresh(paths: &MaestroPaths) -> Result<BacklogConfig> {
    let mut backlog = load_backlog(paths)?;
    let proposals = detect::detect(paths)?;
    merge_proposals(&mut backlog, proposals);
    save_backlog(paths, &backlog)?;
    Ok(backlog)
}

/// Apply a backlog proposal by marking it applied.
pub fn apply(paths: &MaestroPaths, id: &str) -> Result<BacklogItem> {
    let mut backlog = refresh(paths)?;
    let Some(item) = backlog.items.iter_mut().find(|item| item.id == id) else {
        bail!("backlog item not found: {id}");
    };
    item.status = "applied".to_string();
    let applied = item.clone();
    save_backlog(paths, &backlog)?;
    Ok(applied)
}

/// Load the backlog, returning an empty V1 backlog when it does not exist yet.
pub fn load_backlog(paths: &MaestroPaths) -> Result<BacklogConfig> {
    let path = managed_path(
        paths,
        ".maestro/harness/backlog.yaml",
        SymlinkPolicy::RejectAllComponents,
    )?;
    let raw = match fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(BacklogConfig::empty()),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", path.display()));
        }
    };
    let backlog: BacklogConfig = serde_yaml::from_str(&raw)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    if backlog.schema_version != BACKLOG_SCHEMA_VERSION {
        bail!(
            "schema mismatch for {}: expected {}, found {}",
            path.display(),
            BACKLOG_SCHEMA_VERSION,
            backlog.schema_version
        );
    }
    Ok(backlog)
}

fn save_backlog(paths: &MaestroPaths, backlog: &BacklogConfig) -> Result<()> {
    let path = managed_path(
        paths,
        ".maestro/harness/backlog.yaml",
        SymlinkPolicy::RejectAllComponents,
    )?;
    let raw = serde_yaml::to_string(backlog).context("failed to serialize backlog")?;
    write_string_atomic(&path, &raw).with_context(|| format!("failed to write {}", path.display()))
}

fn merge_proposals(backlog: &mut BacklogConfig, proposals: Vec<BacklogItem>) {
    let mut keys = backlog.items.iter().map(item_key).collect::<BTreeSet<_>>();
    let mut next = next_backlog_number(&backlog.items);

    for mut proposal in proposals {
        let key = item_key(&proposal);
        if keys.contains(&key) {
            continue;
        }
        proposal.id = format!("hb-{next:03}");
        next += 1;
        keys.insert(key);
        backlog.items.push(proposal);
    }
}

fn item_key(item: &BacklogItem) -> String {
    format!("{}\t{}\t{}", item.source, item.item_type, item.title)
}

fn next_backlog_number(items: &[BacklogItem]) -> u32 {
    items
        .iter()
        .filter_map(|item| item.id.strip_prefix("hb-"))
        .filter_map(|number| number.parse::<u32>().ok())
        .max()
        .unwrap_or(0)
        + 1
}
