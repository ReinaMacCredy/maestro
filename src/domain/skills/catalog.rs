use std::sync::OnceLock;

use include_dir::{include_dir, Dir, File};
use serde::Deserialize;

/// A skill Maestro ships and refreshes, distinct from user-added skills under
/// `.maestro/skills/`. A skill is a directory tree: `SKILL.md` plus any sibling
/// files or folders.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Skill {
    /// Directory name under `.maestro/skills/`.
    pub name: &'static str,
    /// Tree files in stable extraction order; `SKILL.md` is one entry.
    pub files: Vec<SkillFile>,
}

/// One file in a skill's directory tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkillFile {
    /// Path relative to the skill directory, e.g. `SKILL.md`,
    /// `reference/x.md`, `scripts/run.sh`.
    pub relative_path: &'static str,
    /// Embedded file bytes. `SKILL.md` is decoded as UTF-8 for frontmatter
    /// parsing via [`Skill::skill_md`]; other files may be binary assets.
    pub contents: &'static [u8],
}

/// The bundled skill trees embedded at build time. Each top-level entry is a
/// skill directory (`SKILL.md` plus optional sibling files/folders).
static SKILLS_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/resources/skills");

/// Stable extraction order. `include_dir` does not document a directory
/// iteration order, so the catalog is sorted by position here (unknown names
/// fall to the end, ordered by name) to keep extraction deterministic and
/// behavior-preserving against the original hand-authored skill order.
const SKILL_ORDER: [&str; 4] = [
    "maestro-task",
    "maestro-setup",
    "maestro-verify",
    "maestro-design",
];

static CATALOG: OnceLock<Vec<Skill>> = OnceLock::new();

/// Return the skills Maestro ships and refreshes, in extraction order. These are
/// distinct from user-added skills under `.maestro/skills/`.
///
/// Built once by walking the embedded [`SKILLS_DIR`] and memoized; the returned
/// slice borrows the `'static` embedded bytes and skill-name strings.
pub fn skills() -> &'static [Skill] {
    CATALOG.get_or_init(build_catalog).as_slice()
}

/// Walk `SKILLS_DIR`: each top-level directory is a skill, and its files (at any
/// depth) become [`SkillFile`] entries keyed by their path within the skill.
fn build_catalog() -> Vec<Skill> {
    let mut skills = SKILLS_DIR
        .dirs()
        .map(|dir| {
            let name = dir
                .path()
                .file_name()
                .and_then(|name| name.to_str())
                .expect("invariant: an embedded skill directory has a UTF-8 name");
            let mut files = collect_files(dir, dir);
            files.sort_by_key(|file| file.relative_path);
            Skill { name, files }
        })
        .collect::<Vec<_>>();
    skills.sort_by_key(|skill| {
        let rank = SKILL_ORDER
            .iter()
            .position(|name| *name == skill.name)
            .unwrap_or(usize::MAX);
        (rank, skill.name)
    });
    skills
}

/// Collect every file under `dir`, recording each path relative to `skill_root`.
fn collect_files(dir: &'static Dir<'static>, skill_root: &'static Dir<'static>) -> Vec<SkillFile> {
    let mut files = dir
        .files()
        .map(|file| skill_file(file, skill_root))
        .collect::<Vec<_>>();
    for subdir in dir.dirs() {
        files.extend(collect_files(subdir, skill_root));
    }
    files
}

fn skill_file(file: &'static File<'static>, skill_root: &'static Dir<'static>) -> SkillFile {
    let relative_path = file
        .path()
        .strip_prefix(skill_root.path())
        .ok()
        .and_then(|path| path.to_str())
        .expect(
            "invariant: an embedded skill file lives under its skill directory with a UTF-8 path",
        );
    SkillFile {
        relative_path,
        contents: file.contents(),
    }
}

impl Skill {
    /// Return the decoded `SKILL.md` contents.
    ///
    /// # Panics
    ///
    /// Panics if the skill has no `SKILL.md` entry or its bytes are not UTF-8.
    /// Both are build-time invariants of the embedded tree (the architecture
    /// guard requires every skill directory to contain a `SKILL.md`), not user
    /// input, so a violation is a bug rather than a recoverable error.
    pub fn skill_md(&self) -> &str {
        let file = self
            .files
            .iter()
            .find(|file| file.relative_path == "SKILL.md")
            .expect("invariant: every bundled skill ships a SKILL.md");
        std::str::from_utf8(file.contents).expect("invariant: SKILL.md is UTF-8")
    }
}

/// Read the `version:` field from a `SKILL.md` frontmatter block.
///
/// Returns `None` when the contents lack a leading `---` fence, the frontmatter
/// is not valid YAML, or no `version` key is present. An installed `SKILL.md` is
/// a trust boundary (a user may edit it into malformed YAML), so this never
/// errors: an unreadable version is treated as absent, which forces a refresh.
pub(crate) fn frontmatter_version(contents: &str) -> Option<String> {
    let body = contents.strip_prefix("---\n")?;
    let end = body.find("\n---")?;

    #[derive(Deserialize)]
    struct Frontmatter {
        // serde defaults a missing `Option` field to `None`, so no `#[serde(default)]`.
        version: Option<String>,
    }

    serde_yaml::from_str::<Frontmatter>(&body[..end])
        .ok()?
        .version
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontmatter_version_reads_shipped_skill_versions() {
        // The exact per-skill version is pinned by tests/resources_version_guard.rs;
        // here we only assert the parser extracts a non-empty version from every
        // shipped skill, so bumping one skill's version does not break this test.
        for skill in skills() {
            let version = frontmatter_version(skill.skill_md());
            assert!(
                version
                    .as_deref()
                    .is_some_and(|version| !version.is_empty()),
                "shipped skill {} should declare a non-empty version, got {version:?}",
                skill.name
            );
        }
    }

    #[test]
    fn every_shipped_skill_has_a_skill_md_entry() {
        for skill in skills() {
            assert!(
                skill
                    .files
                    .iter()
                    .any(|file| file.relative_path == "SKILL.md"),
                "shipped skill {} must contain a SKILL.md entry",
                skill.name
            );
        }
    }

    #[test]
    fn frontmatter_version_is_none_without_a_fence() {
        assert_eq!(frontmatter_version("no frontmatter here\n"), None);
    }

    #[test]
    fn frontmatter_version_is_none_for_malformed_yaml() {
        assert_eq!(frontmatter_version("---\n: : : not yaml\n---\n"), None);
    }

    #[test]
    fn frontmatter_version_is_none_when_version_absent() {
        assert_eq!(frontmatter_version("---\nname: x\n---\n"), None);
    }

    #[test]
    fn frontmatter_version_is_none_without_a_closing_fence() {
        assert_eq!(frontmatter_version("---\nname: x\nversion: 1.0.0\n"), None);
    }
}
