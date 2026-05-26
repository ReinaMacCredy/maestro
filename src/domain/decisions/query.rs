use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Context, Result};

/// One decision markdown file found under `.maestro/decisions`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecisionEntry {
    pub file_name: String,
    pub path: PathBuf,
}

/// List decision markdown files.
pub fn decision_entries(decisions_dir: &Path) -> Result<Vec<DecisionEntry>> {
    if !decisions_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    for entry in fs::read_dir(decisions_dir)
        .with_context(|| format!("failed to read {}", decisions_dir.display()))?
    {
        let entry = entry
            .with_context(|| format!("failed to read entry in {}", decisions_dir.display()))?;
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", entry.path().display()))?;
        if !file_type.is_file() || file_type.is_symlink() {
            continue;
        }
        let Some(file_name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if is_decision_file_name(&file_name) {
            entries.push(DecisionEntry {
                file_name,
                path: entry.path(),
            });
        }
    }
    entries.sort_by(|left, right| left.file_name.cmp(&right.file_name));
    Ok(entries)
}

/// Resolve a decision id or file name to a markdown path.
pub fn resolve_decision_path(decisions_dir: &Path, id: &str) -> Result<PathBuf> {
    validate_decision_lookup_id(id)?;
    if id.ends_with(".md") {
        let path = decisions_dir.join(id);
        if valid_decision_file(&path)? {
            return Ok(path);
        }
    }

    let direct = decisions_dir.join(format!("{id}.md"));
    if valid_decision_file(&direct)? {
        return Ok(direct);
    }

    let prefix = format!("{id}-");
    let matches = decision_entries(decisions_dir)?
        .into_iter()
        .filter(|entry| entry.file_name.starts_with(&prefix))
        .collect::<Vec<_>>();

    match matches.len() {
        0 => bail!("decision {id} not found"),
        1 => Ok(matches[0].path.clone()),
        _ => bail!("decision {id} is ambiguous"),
    }
}

fn valid_decision_file(path: &Path) -> Result<bool> {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return Ok(false);
    };
    Ok(metadata.is_file() && !metadata.file_type().is_symlink())
}

fn validate_decision_lookup_id(id: &str) -> Result<()> {
    let mut components = Path::new(id).components();
    if id.is_empty()
        || !matches!(components.next(), Some(Component::Normal(_)))
        || components.next().is_some()
    {
        bail!("invalid decision id: {id}");
    }
    Ok(())
}

/// Parse the sequence number from a decision file name.
pub fn parse_decision_number(file_name: &str) -> Option<u32> {
    let number = file_name.strip_prefix("decision-")?.split('-').next()?;
    number.parse::<u32>().ok()
}

/// Return the id portion of a decision file name.
pub fn decision_id(file_name: &str) -> &str {
    file_name.trim_end_matches(".md")
}

fn is_decision_file_name(file_name: &str) -> bool {
    file_name.starts_with("decision-") && file_name.ends_with(".md")
}
