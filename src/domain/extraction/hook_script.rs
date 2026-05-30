//! Extraction of the bundled hook recorder script into `.maestro/hooks/`.
//!
//! Maestro ships one editable shell wrapper, `record.sh`, that forwards to
//! `maestro hook record`. It rides the shared version-gated extraction core: the
//! `# maestro:hook-version:` comment plays the role that `SKILL.md`'s
//! frontmatter `version:` plays for skills, so local edits survive `maestro
//! update` until the shipped version changes.

use std::path::PathBuf;

use anyhow::Result;

use crate::domain::extraction::extract::{
    apply_actions, file_action, folder_gate, read_existing, Action, ExtractMode, ExtractReport,
};
use crate::foundation::core::fs::ensure_dir;
use crate::foundation::core::managed_path::{managed_path, SymlinkPolicy};
use crate::foundation::core::paths::MaestroPaths;

/// The bundled hook recorder script, embedded at build time.
const RECORD_SH: &str = include_str!("../../../resources/hooks/record.sh");

/// Report name and on-disk filename for the bundled hook script.
const RECORD_SH_NAME: &str = "record.sh";

/// Extract the bundled hook recorder script into `.maestro/hooks/`.
pub fn extract_hook_script(paths: &MaestroPaths, mode: ExtractMode<'_>) -> Result<ExtractReport> {
    extract_hook_script_from(paths, RECORD_SH, mode)
}

/// Extract a hook script with explicit contents into `.maestro/hooks/`.
///
/// Exposed so tests can drive the writer with a chosen version; production
/// callers go through [`extract_hook_script`] with the bundled script.
pub fn extract_hook_script_from(
    paths: &MaestroPaths,
    contents: &str,
    mode: ExtractMode<'_>,
) -> Result<ExtractReport> {
    managed_path(paths, ".maestro", SymlinkPolicy::RejectAllComponents)?;
    ensure_dir(paths.hooks_dir())?;
    let mut report = ExtractReport::default();
    let actions = plan_hook_script(paths, contents, mode)?;

    apply_actions(paths, &actions, &mut report)?;

    Ok(report)
}

/// Validate bundled hook script extraction without writing files.
pub fn validate_hook_script(paths: &MaestroPaths, mode: ExtractMode<'_>) -> Result<()> {
    managed_path(paths, ".maestro", SymlinkPolicy::RejectAllComponents)?;
    plan_hook_script(paths, RECORD_SH, mode)?;
    Ok(())
}

/// Plan the single-file hook script write. The version gate keys on the
/// installed script's `# maestro:hook-version:` marker, mirroring the
/// whole-folder skill gate for a one-file resource.
fn plan_hook_script<'a>(
    paths: &MaestroPaths,
    contents: &'a str,
    mode: ExtractMode<'a>,
) -> Result<Vec<Action<'a>>> {
    let path = hook_file_path(paths, RECORD_SH_NAME)?;
    let existing = read_existing(&path)?;
    let gate = folder_gate(
        mode,
        existing.as_deref(),
        contents,
        hook_script_version,
        &path,
    )?;

    Ok(vec![file_action(
        RECORD_SH_NAME,
        contents.as_bytes(),
        path,
        existing,
        gate,
    )?])
}

fn hook_file_path(paths: &MaestroPaths, file_name: &str) -> Result<PathBuf> {
    let relative = format!(".maestro/hooks/{file_name}");
    managed_path(paths, &relative, SymlinkPolicy::RejectAllComponents)
}

/// Read the `maestro:hook-version:` marker from a hook script.
///
/// The hook script is shell, not Markdown, so it carries no frontmatter; the
/// version lives in a `# maestro:hook-version: X` comment. Returns `None` when
/// no such marker is present, which forces a refresh just as a missing skill
/// `version:` does. An installed script is a trust boundary (a user may edit or
/// strip the marker), so this never errors.
pub(crate) fn hook_script_version(contents: &str) -> Option<String> {
    contents.lines().find_map(|line| {
        line.trim_start()
            .strip_prefix("# maestro:hook-version:")
            .map(|version| version.trim().to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_record_script_declares_a_hook_version() {
        assert_eq!(hook_script_version(RECORD_SH).as_deref(), Some("1.0.0"));
    }

    #[test]
    fn hook_script_version_trims_value_and_tolerates_indent() {
        assert_eq!(
            hook_script_version("   # maestro:hook-version:   2.3.4  \n").as_deref(),
            Some("2.3.4")
        );
    }

    #[test]
    fn hook_script_version_is_none_without_a_marker() {
        assert_eq!(hook_script_version("exec maestro hook record\n"), None);
    }
}
