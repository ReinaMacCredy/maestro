/// A skill embedded in the Maestro binary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BundledSkill {
    /// Directory name under `.maestro/skills/`.
    pub name: &'static str,
    /// Complete `SKILL.md` contents.
    pub contents: &'static str,
}

const MAESTRO_TASK: &str = include_str!("../../../resources/skills/bundled/maestro-task/SKILL.md");
const MAESTRO_SETUP: &str =
    include_str!("../../../resources/skills/bundled/maestro-setup/SKILL.md");
const MAESTRO_VERIFY: &str =
    include_str!("../../../resources/skills/bundled/maestro-verify/SKILL.md");
const MAESTRO_DESIGN: &str =
    include_str!("../../../resources/skills/bundled/maestro-design/SKILL.md");

const BUNDLED_SKILLS: [BundledSkill; 4] = [
    BundledSkill {
        name: "maestro-task",
        contents: MAESTRO_TASK,
    },
    BundledSkill {
        name: "maestro-setup",
        contents: MAESTRO_SETUP,
    },
    BundledSkill {
        name: "maestro-verify",
        contents: MAESTRO_VERIFY,
    },
    BundledSkill {
        name: "maestro-design",
        contents: MAESTRO_DESIGN,
    },
];

/// Return the bundled skills in extraction order.
pub fn bundled_skills() -> &'static [BundledSkill] {
    &BUNDLED_SKILLS
}
