//! The bundled code playbook, served on demand from the binary.
//!
//! Per-language code styleguides ride embedded in the binary and are served by
//! `maestro playbook` rather than extracted per repo (unlike the harness
//! protocol). `maestro playbook <lang>` prints one guide verbatim; `maestro
//! playbook` with no token prints the index. The guides live in
//! `embedded/playbook/`: `PLAYBOOK.md` is the index prose, each `<lang>.md` is
//! one styleguide. Serving from the binary means the command needs no
//! `.maestro` repo and the guides never drift from what this binary ships.

use anyhow::{Result, bail};
use include_dir::{Dir, include_dir};

/// The bundled code playbook tree, embedded at build time.
static PLAYBOOK_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/embedded/playbook");

/// The index prose file; everything else is a language guide.
const INDEX_NAME: &str = "PLAYBOOK.md";

/// Serve one language guide verbatim by its token (`rust`, `html-css`, ...).
/// An unknown token fails loud with the available list, never a dead end.
pub fn serve(lang: &str) -> Result<&'static str> {
    let file_name = format!("{lang}.md");
    if file_name != INDEX_NAME
        && let Some(body) = PLAYBOOK_DIR
            .get_file(PLAYBOOK_DIR.path().join(&file_name))
            .and_then(|file| file.contents_utf8())
    {
        return Ok(body);
    }
    bail!(
        "unknown code playbook \"{lang}\"; run `maestro playbook` for the index (available: {})",
        languages().join(", ")
    );
}

/// The index: the `PLAYBOOK.md` prose followed by the available guides,
/// enumerated from the embedded tree so the list never drifts from what ships.
pub fn index() -> String {
    let mut out = shipped_index_prose().trim_end().to_string();
    out.push_str("\n\n## Available guides\n\n");
    for lang in languages() {
        out.push_str(&format!("    maestro playbook {lang}\n"));
    }
    out
}

/// Every language token the playbook serves, sorted; the index anchor excluded.
pub fn languages() -> Vec<&'static str> {
    let mut langs: Vec<&'static str> = PLAYBOOK_DIR
        .files()
        .filter_map(|file| {
            let name = file
                .path()
                .strip_prefix(PLAYBOOK_DIR.path())
                .ok()
                .and_then(|path| path.to_str())?;
            (name != INDEX_NAME).then(|| name.strip_suffix(".md").unwrap_or(name))
        })
        .collect();
    langs.sort_unstable();
    langs
}

/// The shipped contents of the `PLAYBOOK.md` index prose.
fn shipped_index_prose() -> &'static str {
    PLAYBOOK_DIR
        .get_file(PLAYBOOK_DIR.path().join(INDEX_NAME))
        .and_then(|file| file.contents_utf8())
        .expect("invariant: PLAYBOOK.md is embedded and UTF-8")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serves_every_language_guide_byte_identical_to_the_embedded_file() {
        for lang in languages() {
            let embedded = PLAYBOOK_DIR
                .get_file(PLAYBOOK_DIR.path().join(format!("{lang}.md")))
                .and_then(|file| file.contents_utf8())
                .unwrap_or_else(|| panic!("embedded guide {lang}.md is missing"));
            assert_eq!(serve(lang).unwrap(), embedded, "{lang} served body drifted");
        }
    }

    #[test]
    fn ships_every_expected_language_guide() {
        let langs = languages();
        for expected in [
            "cpp",
            "csharp",
            "dart",
            "general",
            "go",
            "html-css",
            "javascript",
            "python",
            "rust",
            "typescript",
        ] {
            assert!(langs.contains(&expected), "playbook is missing {expected}");
        }
    }

    #[test]
    fn languages_excludes_the_index_anchor() {
        assert!(!languages().contains(&"PLAYBOOK"));
    }

    #[test]
    fn unknown_token_is_a_loud_error_listing_the_available_guides() {
        let error = serve("nosuchlang").unwrap_err().to_string();
        assert!(error.contains("nosuchlang"), "{error}");
        assert!(error.contains("rust"), "{error}");
    }

    #[test]
    fn the_index_anchor_is_not_servable_as_a_language() {
        assert!(serve("PLAYBOOK").is_err());
    }

    #[test]
    fn index_carries_the_prose_and_every_available_token() {
        let index = index();
        assert!(index.contains("## How to use this"), "{index}");
        assert!(index.contains("Apache License"), "{index}");
        for lang in languages() {
            assert!(
                index.contains(&format!("maestro playbook {lang}")),
                "index omits {lang}"
            );
        }
    }
}
