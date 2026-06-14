//! Extraction of the bundled code playbook into `.maestro/playbook/`.
//!
//! The playbook is a folder of per-language code styleguides surfaced to agents
//! on demand (the harness protocol points at it; the agent reads only the file
//! for the language it is editing). It rides the shared version-gated extraction
//! core like the harness protocol: `PLAYBOOK.md`'s Markdown frontmatter
//! `version:` is the single anchor that gates the whole folder, so the vendored
//! language files carry no frontmatter of their own and local edits survive
//! `maestro upgrade` until the shipped version changes.

use std::path::PathBuf;

use anyhow::Result;
use include_dir::{Dir, include_dir};

use crate::domain::extraction::extract::{
    Action, FolderPreview, apply_actions, file_action, folder_gate, preview_folder, read_existing,
};
use crate::domain::skills::catalog::frontmatter_version;
use crate::foundation::core::fs::ensure_dir;
use crate::foundation::core::managed_path::{SymlinkPolicy, managed_path};
use crate::foundation::core::paths::MaestroPaths;

pub use crate::domain::extraction::extract::{ExtractMode, ExtractReport};

/// The bundled code playbook tree, embedded at build time.
static PLAYBOOK_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/embedded/playbook");

/// Anchor file whose frontmatter `version:` gates the whole folder.
const PLAYBOOK_MD_NAME: &str = "PLAYBOOK.md";

/// Report/preview name for the playbook folder, reusing the [`FolderPreview`]
/// vocabulary (a skill directory name, `record.sh`, `HARNESS.md`).
const PLAYBOOK_NAME: &str = "playbook";

/// Extract the bundled code playbook into `.maestro/playbook/`.
pub fn extract_playbook(paths: &MaestroPaths, mode: ExtractMode<'_>) -> Result<ExtractReport> {
    managed_path(paths, ".maestro", SymlinkPolicy::RejectAllComponents)?;
    ensure_dir(paths.playbook_dir())?;
    let mut report = ExtractReport::default();
    let actions = plan_playbook(paths, mode)?;

    apply_actions(paths, &actions, &mut report)?;

    Ok(report)
}

/// Validate bundled playbook extraction without writing files.
pub fn validate_playbook(paths: &MaestroPaths, mode: ExtractMode<'_>) -> Result<()> {
    managed_path(paths, ".maestro", SymlinkPolicy::RejectAllComponents)?;
    plan_playbook(paths, mode)?;
    Ok(())
}

/// Preview the playbook folder's fate without writing files.
///
/// Reports ONE folder-level [`FolderPreview`] keyed on the `PLAYBOOK.md` anchor,
/// not one line per vendored language file, so `sync`/`init --dry-run` show the
/// playbook as a single folder the way they show a skill directory.
pub fn preview_playbook(paths: &MaestroPaths, mode: ExtractMode<'_>) -> Result<Vec<FolderPreview>> {
    let anchor_path = playbook_file_path(paths, PLAYBOOK_MD_NAME)?;
    let installed_anchor = read_existing(&anchor_path)?;
    Ok(vec![preview_folder(
        PLAYBOOK_NAME,
        mode,
        installed_anchor.as_deref(),
        shipped_anchor(),
        frontmatter_version,
    )])
}

/// Plan the folder write: one gate derived from the `PLAYBOOK.md` anchor governs
/// every embedded file, mirroring the whole-folder skill gate. The vendored
/// language files carry no version of their own; the anchor is the single
/// source of the folder's version.
fn plan_playbook<'a>(paths: &MaestroPaths, mode: ExtractMode<'a>) -> Result<Vec<Action<'a>>> {
    let anchor_path = playbook_file_path(paths, PLAYBOOK_MD_NAME)?;
    let installed_anchor = read_existing(&anchor_path)?;
    let gate = folder_gate(
        mode,
        installed_anchor.as_deref(),
        shipped_anchor(),
        frontmatter_version,
        &anchor_path,
    )?;

    let mut actions = Vec::new();
    for (name, contents) in embedded_files() {
        let path = playbook_file_path(paths, name)?;
        let existing = read_existing(&path)?;
        actions.push(file_action(name, contents, path, existing, gate)?);
    }
    Ok(actions)
}

/// The shipped contents of the `PLAYBOOK.md` anchor.
fn shipped_anchor() -> &'static str {
    PLAYBOOK_DIR
        .get_file(PLAYBOOK_DIR.path().join(PLAYBOOK_MD_NAME))
        .and_then(|file| file.contents_utf8())
        .expect("invariant: PLAYBOOK.md is embedded and UTF-8")
}

/// Every embedded playbook file as `(basename, bytes)`. The folder is flat by
/// design (the anchor plus one styleguide per language).
fn embedded_files() -> Vec<(&'static str, &'static [u8])> {
    PLAYBOOK_DIR
        .files()
        .map(|file| {
            let name = file
                .path()
                .strip_prefix(PLAYBOOK_DIR.path())
                .ok()
                .and_then(|path| path.to_str())
                .expect("invariant: an embedded playbook file has a UTF-8 basename under its root");
            (name, file.contents())
        })
        .collect()
}

fn playbook_file_path(paths: &MaestroPaths, file_name: &str) -> Result<PathBuf> {
    let relative = format!(".maestro/playbook/{file_name}");
    managed_path(paths, &relative, SymlinkPolicy::RejectAllComponents)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_playbook_declares_a_frontmatter_version() {
        assert_eq!(
            frontmatter_version(shipped_anchor()).as_deref(),
            Some("1.0.0")
        );
    }

    #[test]
    fn bundled_playbook_ships_the_anchor_and_every_language_file() {
        let names: Vec<&str> = embedded_files().into_iter().map(|(name, _)| name).collect();
        for expected in [
            "PLAYBOOK.md",
            "rust.md",
            "cpp.md",
            "csharp.md",
            "dart.md",
            "general.md",
            "go.md",
            "html-css.md",
            "javascript.md",
            "python.md",
            "typescript.md",
        ] {
            assert!(names.contains(&expected), "playbook is missing {expected}");
        }
    }
}
