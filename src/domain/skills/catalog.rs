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
