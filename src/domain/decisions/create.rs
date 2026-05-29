//! Decision create-operation surface.
//!
//! Concentrates the create-op that was previously stranded in the
//! `maestro decision new` adapter: directory creation, auto-increment id
//! allocation, canonical filename + markdown rendering, and the atomic write.
//! id allocation lives with the artifact so a future reader cannot diverge from
//! the CLI on what the next decision number is.

use std::path::Path;

use anyhow::{Context, Result};

use crate::domain::decisions::query::{decision_entries, parse_decision_number};
use crate::domain::decisions::template::{decision_file_name, decision_markdown};
use crate::foundation::core::fs::ensure_dir;
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::safe_write::write_string_atomic;

/// Create a decision markdown file from a title, allocating the next sequence
/// number and persisting the section 7.4 template. Returns the assigned number.
///
/// The title is slugified without validation, matching the established baseline
/// for human-authored ADRs.
///
/// # Errors
///
/// Errors when the decisions directory cannot be created, the existing files
/// cannot be listed, or the new file cannot be written.
pub fn create(paths: &MaestroPaths, title: &str) -> Result<u32> {
    let decisions_dir = paths.decisions_dir();
    ensure_dir(&decisions_dir)?;
    let number = next_decision_number(&decisions_dir)?;
    let path = decisions_dir.join(decision_file_name(number, title));
    let contents = decision_markdown(number, title);
    write_string_atomic(&path, &contents)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(number)
}

/// Allocate the next decision sequence number as one past the highest existing
/// `decision-NNN` file in `decisions_dir`.
///
/// # Errors
///
/// Errors when the decisions directory cannot be listed.
pub(crate) fn next_decision_number(decisions_dir: &Path) -> Result<u32> {
    let mut max_number = 0_u32;
    for entry in decision_entries(decisions_dir)? {
        if let Some(number) = parse_decision_number(&entry.file_name) {
            max_number = max_number.max(number);
        }
    }
    Ok(max_number + 1)
}
