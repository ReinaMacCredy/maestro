use serde::Deserialize;

/// A skill Maestro ships and refreshes, distinct from user-added skills under
/// `.maestro/skills/`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Skill {
    /// Directory name under `.maestro/skills/`.
    pub name: &'static str,
    /// Complete `SKILL.md` contents.
    pub contents: &'static str,
}

const MAESTRO_TASK: &str = include_str!("../../../resources/skills/maestro-task/SKILL.md");
const MAESTRO_SETUP: &str = include_str!("../../../resources/skills/maestro-setup/SKILL.md");
const MAESTRO_VERIFY: &str = include_str!("../../../resources/skills/maestro-verify/SKILL.md");
const MAESTRO_DESIGN: &str = include_str!("../../../resources/skills/maestro-design/SKILL.md");

const SKILLS: [Skill; 4] = [
    Skill {
        name: "maestro-task",
        contents: MAESTRO_TASK,
    },
    Skill {
        name: "maestro-setup",
        contents: MAESTRO_SETUP,
    },
    Skill {
        name: "maestro-verify",
        contents: MAESTRO_VERIFY,
    },
    Skill {
        name: "maestro-design",
        contents: MAESTRO_DESIGN,
    },
];

/// Return the skills Maestro ships and refreshes, in extraction order. These are
/// distinct from user-added skills under `.maestro/skills/`.
pub fn skills() -> &'static [Skill] {
    &SKILLS
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
        // The exact per-skill version is pinned by tests/skills_version_guard.rs;
        // here we only assert the parser extracts a non-empty version from every
        // shipped skill, so bumping one skill's version does not break this test.
        for skill in skills() {
            let version = frontmatter_version(skill.contents);
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
