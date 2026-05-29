use std::collections::BTreeSet;
use std::fs;
use std::io::ErrorKind;

use anyhow::{bail, Context, Result};

use crate::domain::harness::schema::{BacklogConfig, BacklogItem};
use crate::foundation::core::managed_path::{managed_path, SymlinkPolicy};
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::safe_write::write_string_atomic;
use crate::foundation::core::schema::{classify, Compat, BACKLOG_SCHEMA_VERSION};

/// Load the Harness backlog, returning an empty V1 backlog when it does not exist.
pub fn load(paths: &MaestroPaths) -> Result<BacklogConfig> {
    let path = backlog_path(paths)?;
    let raw = match fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(BacklogConfig::empty()),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", path.display()));
        }
    };
    let backlog: BacklogConfig = serde_yaml::from_str(&raw)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    validate_schema(&path, &backlog)?;
    Ok(backlog)
}

/// Persist a Harness backlog through the managed Harness path policy.
pub fn save(paths: &MaestroPaths, backlog: &BacklogConfig) -> Result<()> {
    let path = backlog_path(paths)?;
    validate_schema(&path, backlog)?;
    let raw = serde_yaml::to_string(backlog).context("failed to serialize backlog")?;
    write_string_atomic(&path, &raw).with_context(|| format!("failed to write {}", path.display()))
}

/// Refresh proposals into the Harness backlog without applying them.
pub fn refresh(paths: &MaestroPaths, proposals: Vec<BacklogItem>) -> Result<BacklogConfig> {
    let mut backlog = load(paths)?;
    merge_proposals(&mut backlog, proposals);
    save(paths, &backlog)?;
    Ok(backlog)
}

/// Mark one existing backlog item as applied.
pub fn mark_applied(backlog: &mut BacklogConfig, id: &str) -> Result<BacklogItem> {
    let Some(item) = backlog.items.iter_mut().find(|item| item.id == id) else {
        bail!("backlog item not found: {id}");
    };
    item.status = "applied".to_string();
    Ok(item.clone())
}

/// Merge proposals by stable source/type/title key and assign deterministic ids.
pub fn merge_proposals(backlog: &mut BacklogConfig, mut proposals: Vec<BacklogItem>) {
    sanitize_existing_generated_evidence(backlog);
    let mut keys = backlog
        .items
        .iter()
        .map(proposal_key)
        .collect::<BTreeSet<_>>();
    let mut next = next_backlog_number(&backlog.items);
    proposals.sort_by_key(proposal_key);

    for mut proposal in proposals {
        let key = proposal_key(&proposal);
        if keys.contains(&key) {
            if let Some(existing) = backlog
                .items
                .iter_mut()
                .find(|item| proposal_key(item) == key)
            {
                refresh_existing_evidence(existing, &proposal);
            }
            continue;
        }
        proposal.id = format!("hb-{next:03}");
        next += 1;
        keys.insert(key);
        backlog.items.push(proposal);
    }
}

/// Return the stable duplicate-detection key for a backlog proposal.
pub fn proposal_key(item: &BacklogItem) -> String {
    format!("{}\t{}\t{}", item.source, item.item_type, item.title)
}

fn refresh_existing_evidence(existing: &mut BacklogItem, proposal: &BacklogItem) {
    if existing.item_type != "missing_verification" {
        return;
    }

    let mut refreshed = existing
        .evidence
        .iter()
        .filter(|evidence| !is_generated_missing_verification_evidence(evidence))
        .cloned()
        .collect::<Vec<_>>();
    for (index, evidence) in proposal.evidence.iter().enumerate() {
        let evidence = sanitize_missing_verification_evidence(evidence, index);
        if !refreshed.contains(&evidence) {
            refreshed.push(evidence);
        }
    }
    existing.evidence = refreshed;
}

fn sanitize_existing_generated_evidence(backlog: &mut BacklogConfig) {
    for item in &mut backlog.items {
        if item.item_type == "missing_verification" {
            let mut generated_index = 0;
            item.evidence = item
                .evidence
                .iter()
                .map(|evidence| {
                    if is_generated_missing_verification_evidence(evidence) {
                        let sanitized =
                            sanitize_missing_verification_evidence(evidence, generated_index);
                        generated_index += 1;
                        sanitized
                    } else {
                        evidence.to_string()
                    }
                })
                .collect();
        }
    }
}

fn is_generated_missing_verification_evidence(evidence: &str) -> bool {
    is_safe_missing_verification_evidence(evidence)
        || is_legacy_generated_missing_verification_evidence(evidence)
}

fn sanitize_missing_verification_evidence(evidence: &str, index: usize) -> String {
    if let Some(source) = safe_missing_verification_source(evidence) {
        return format!(
            "{} used verification command {} outside harness.yml",
            source,
            index + 1
        );
    }
    let Some((source, detail)) = evidence.split_once(" used ") else {
        return evidence.to_string();
    };
    if !detail.ends_with(" outside harness.yml") {
        return evidence.to_string();
    }
    let source = safe_verification_source(source);
    if source == "verification evidence" {
        return evidence.to_string();
    }
    format!(
        "{} used verification command {} outside harness.yml",
        source,
        index + 1
    )
}

fn is_safe_missing_verification_evidence(evidence: &str) -> bool {
    safe_missing_verification_source(evidence).is_some()
}

fn safe_missing_verification_source(evidence: &str) -> Option<&str> {
    let (source, command) = evidence.split_once(" used ")?;
    let label = command
        .strip_prefix("verification command ")
        .and_then(|label| label.strip_suffix(" outside harness.yml"))?;
    if safe_verification_source(source) == source.trim() && label.parse::<usize>().is_ok() {
        Some(source.trim())
    } else {
        None
    }
}

fn is_legacy_generated_missing_verification_evidence(evidence: &str) -> bool {
    let Some((source, detail)) = evidence.split_once(" used ") else {
        return false;
    };
    detail.ends_with(" outside harness.yml")
        && safe_verification_source(source) != "verification evidence"
}

fn safe_verification_source(source: &str) -> &'static str {
    let source = source.trim();
    if source == "verification.json" {
        return "verification.json";
    }
    if source == "verification.attempts/latest.json" {
        return "verification.attempts/latest.json";
    }
    if source.starts_with("verification.attempts/") {
        return "verification.attempts/archived attempt";
    }
    "verification evidence"
}

fn backlog_path(paths: &MaestroPaths) -> Result<std::path::PathBuf> {
    managed_path(
        paths,
        ".maestro/harness/backlog.yaml",
        SymlinkPolicy::RejectAllComponents,
    )
}

fn validate_schema(path: &std::path::Path, backlog: &BacklogConfig) -> Result<()> {
    if classify(&backlog.schema_version, BACKLOG_SCHEMA_VERSION) != Compat::Exact {
        bail!(
            "schema mismatch for {}: expected {}, found {}",
            path.display(),
            BACKLOG_SCHEMA_VERSION,
            backlog.schema_version
        );
    }
    Ok(())
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
