use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result};

use crate::domain::harness::{self as harness_domain, EscalationPolicy, HarnessConfig};
use crate::domain::run;
use crate::foundation::core::fs::read_to_string_if_exists;
use crate::foundation::core::managed_path::{SymlinkPolicy, managed_path};
use crate::foundation::core::paths::MaestroPaths;

pub fn load_policy(paths: &MaestroPaths) -> Result<EscalationPolicy> {
    Ok(
        load_config(paths)?.map_or_else(EscalationPolicy::disabled, |config| {
            config.escalation_policy()
        }),
    )
}

pub fn load_config(paths: &MaestroPaths) -> Result<Option<HarnessConfig>> {
    let path = managed_path(
        paths,
        ".maestro/harness/harness.yml",
        SymlinkPolicy::RejectAllComponents,
    )?;
    let Some(raw) = read_to_string_if_exists(&path)? else {
        return Ok(None);
    };
    let config: HarnessConfig = serde_yaml::from_str(&raw)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(Some(config))
}

pub fn set_claims_only_verification(paths: &MaestroPaths) -> Result<()> {
    harness_domain::set_claims_only_verification(paths)
}

pub fn evidence_stamp(paths: &MaestroPaths) -> Result<String> {
    let runs = run_event_stamp(paths)?;
    let cards = card_stamp(&managed_path(
        paths,
        ".maestro/cards",
        SymlinkPolicy::RejectAllComponents,
    )?)?;
    let decisions = tree_stamp(&managed_path(
        paths,
        ".maestro/decisions",
        SymlinkPolicy::RejectAllComponents,
    )?)?;
    Ok(format!(
        "runs={}:{};cards={}:{};decisions={}:{}",
        runs.count,
        runs.max_modified_nanos,
        cards.count,
        cards.max_modified_nanos,
        decisions.count,
        decisions.max_modified_nanos
    ))
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Stamp {
    count: u64,
    max_modified_nanos: u128,
}

fn run_event_stamp(paths: &MaestroPaths) -> Result<Stamp> {
    let mut stamp = Stamp::default();
    for log in run::managed_event_logs(paths)? {
        update_stamp_for_path(log.path(), &mut stamp)?;
    }
    Ok(stamp)
}

fn tree_stamp(path: &Path) -> Result<Stamp> {
    let mut stamp = Stamp::default();
    visit_tree(path, &mut stamp)?;
    Ok(stamp)
}

/// Stamp the card store, skipping detect-owned `hb-*` idea cards and the root
/// dir's own mtime: detect's persisted friction output must not invalidate its
/// own skip cache, while any task, decision, or feature card mutation must.
fn card_stamp(cards_dir: &Path) -> Result<Stamp> {
    let mut stamp = Stamp::default();
    let entries = match fs::read_dir(cards_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(stamp),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", cards_dir.display()));
        }
    };
    for entry in entries {
        let entry = entry.with_context(|| format!("failed to list {}", cards_dir.display()))?;
        if entry.file_name().to_string_lossy().starts_with("hb-") {
            continue;
        }
        visit_tree(&entry.path(), &mut stamp)?;
    }
    Ok(stamp)
}

fn visit_tree(path: &Path, stamp: &mut Stamp) -> Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to inspect {}", path.display()));
        }
    };
    update_stamp(&metadata, stamp);
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(path).with_context(|| format!("failed to read {}", path.display()))? {
        let entry = entry.with_context(|| format!("failed to list {}", path.display()))?;
        visit_tree(&entry.path(), stamp)?;
    }
    Ok(())
}

fn update_stamp_for_path(path: &Path, stamp: &mut Stamp) -> Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to inspect {}", path.display()));
        }
    };
    update_stamp(&metadata, stamp);
    Ok(())
}

fn update_stamp(metadata: &fs::Metadata, stamp: &mut Stamp) {
    stamp.count += 1;
    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    stamp.max_modified_nanos = stamp.max_modified_nanos.max(modified);
}
