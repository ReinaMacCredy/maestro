//! The bundled loop-orchestration recipes, served on demand from the binary.
//!
//! Each recipe is the HOW of running one fan-out or loop pattern: how to
//! dispatch the agents, collect their results through the verbs, and stop. The
//! WHEN -- the judgment to reach for a pattern -- lives in the skills; this
//! catalog carries the mechanics. `maestro loop show <name>` prints one recipe
//! verbatim; `maestro loop` (or `loop list`) prints the index with a one-line
//! when-to-use per recipe. The recipes live in `embedded/loop/`: `LOOP.md` is
//! the index prose, each `<name>.md` is one recipe. Serving from the binary
//! means the command needs no `.maestro` repo and the recipes never drift from
//! what this binary ships.
//!
//! The module is named `loop_recipes` rather than `loop` because `loop` is a
//! reserved Rust keyword; the CLI subcommand is still `maestro loop`.

use anyhow::{Result, bail};
use include_dir::{Dir, include_dir};

/// The bundled loop-recipe tree, embedded at build time.
static LOOP_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/embedded/loop");

/// The index prose file; everything else is a recipe.
const INDEX_NAME: &str = "LOOP.md";

/// Serve one recipe verbatim by its name (`feature-fan-out`, ...). An unknown
/// name fails loud with the available list, never a dead end.
pub fn serve(name: &str) -> Result<&'static str> {
    let file_name = format!("{name}.md");
    if file_name != INDEX_NAME
        && let Some(body) = LOOP_DIR
            .get_file(LOOP_DIR.path().join(&file_name))
            .and_then(|file| file.contents_utf8())
    {
        return Ok(body);
    }
    bail!(
        "unknown loop recipe \"{name}\"; run `maestro loop` for the index (available: {})",
        recipes().join(", ")
    );
}

/// The index: the `LOOP.md` prose followed by the recipes with a one-line
/// when-to-use each, enumerated from the embedded tree so the list never
/// drifts from what ships.
pub fn index() -> String {
    let mut out = shipped_index_prose().trim_end().to_string();
    out.push_str("\n\n## Recipes\n\n");
    for name in recipes() {
        out.push_str(&format!("    {name}  --  {}\n", when(name)));
    }
    out
}

/// Every recipe name the catalog serves, sorted; the index anchor excluded.
pub fn recipes() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = LOOP_DIR
        .files()
        .filter_map(|file| {
            let name = file
                .path()
                .strip_prefix(LOOP_DIR.path())
                .ok()
                .and_then(|path| path.to_str())?;
            (name != INDEX_NAME).then(|| name.strip_suffix(".md").unwrap_or(name))
        })
        .collect();
    names.sort_unstable();
    names
}

/// The one-line when-to-use for a recipe: the text after `WHEN:` on its first
/// matching line, for the index listing.
fn when(name: &str) -> &'static str {
    LOOP_DIR
        .get_file(LOOP_DIR.path().join(format!("{name}.md")))
        .and_then(|file| file.contents_utf8())
        .and_then(|body| body.lines().find_map(|line| line.strip_prefix("WHEN:")))
        .map(str::trim)
        .unwrap_or_default()
}

/// The shipped contents of the `LOOP.md` index prose.
fn shipped_index_prose() -> &'static str {
    LOOP_DIR
        .get_file(LOOP_DIR.path().join(INDEX_NAME))
        .and_then(|file| file.contents_utf8())
        .expect("invariant: LOOP.md is embedded and UTF-8")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serves_every_recipe_byte_identical_to_the_embedded_file() {
        for name in recipes() {
            let embedded = LOOP_DIR
                .get_file(LOOP_DIR.path().join(format!("{name}.md")))
                .and_then(|file| file.contents_utf8())
                .unwrap_or_else(|| panic!("embedded recipe {name}.md is missing"));
            assert_eq!(serve(name).unwrap(), embedded, "{name} served body drifted");
        }
    }

    #[test]
    fn ships_every_expected_recipe() {
        let names = recipes();
        for expected in [
            "adversarial-fan-out",
            "feature-fan-out",
            "generate-and-filter",
            "intake-triage",
            "loop-until-done",
            "unattended-loop",
        ] {
            assert!(names.contains(&expected), "loop catalog is missing {expected}");
        }
        assert_eq!(names.len(), 6, "v1 ships exactly 6 recipes");
    }

    #[test]
    fn recipes_excludes_the_index_anchor() {
        assert!(!recipes().contains(&"LOOP"));
    }

    #[test]
    fn index_lists_every_recipe_with_a_when_line() {
        let idx = index();
        for name in recipes() {
            assert!(idx.contains(name), "index lists {name}");
            assert!(!when(name).is_empty(), "{name} has a WHEN line");
        }
    }

    #[test]
    fn unknown_recipe_is_a_loud_error_listing_the_available_recipes() {
        let error = serve("no-such-recipe").unwrap_err().to_string();
        assert!(error.contains("no-such-recipe"), "{error}");
        assert!(error.contains("feature-fan-out"), "{error}");
    }
}
