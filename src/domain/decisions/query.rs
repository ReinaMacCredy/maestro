use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};

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
        0 => bail!("decision not found: {id}"),
        1 => Ok(matches[0].path.clone()),
        _ => bail!("decision id {id} is ambiguous"),
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

/// The canonical display id for a decision file: `decision-NNN` when the
/// sequence number parses, else the raw slug for a malformed name. The list
/// views use this so a copied id matches `created decision decision-NNN` and
/// `decision show decision-NNN` rather than echoing the full slug (T8).
pub fn decision_display_id(file_name: &str) -> String {
    match parse_decision_number(file_name) {
        Some(number) => format!("decision-{number:03}"),
        None => decision_id(file_name).to_string(),
    }
}

/// The title from a decision file's `# decision-NNN: Title` heading, or
/// `<untitled>` when the heading is missing or malformed. Shared by the
/// `query decisions` and `decision list` views so both render the same column.
pub fn decision_title(path: &Path) -> Result<String> {
    let raw =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let title = raw
        .lines()
        .find_map(|line| line.strip_prefix("# "))
        .and_then(|heading| heading.split_once(": ").map(|(_, title)| title.to_string()))
        .unwrap_or_else(|| "<untitled>".to_string());
    Ok(title)
}

fn is_decision_file_name(file_name: &str) -> bool {
    file_name.starts_with("decision-") && file_name.ends_with(".md")
}
