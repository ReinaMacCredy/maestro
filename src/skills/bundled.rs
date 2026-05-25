/// A skill embedded in the Maestro binary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BundledSkill {
    /// Directory name under `.maestro/skills/`.
    pub name: &'static str,
    /// Human-readable skill description.
    pub description: &'static str,
    /// Complete `SKILL.md` contents.
    pub contents: &'static str,
}

const MAESTRO_TASK: &str = r#"---
name: maestro-task
description: Feature and task workflow layer for operating the Maestro harness.
---

# Maestro Task

Use this skill when creating, claiming, updating, blocking, or completing Maestro tasks.

Start by reading `.maestro/harness/HARNESS.md`, then inspect the relevant task and feature
artifacts before changing state. Prefer Maestro CLI verbs for durable updates, preserve evidence,
and keep task status transitions explicit.
"#;

const MAESTRO_SETUP: &str = r#"---
name: maestro-setup
description: Initial setup and harness tuning protocol for a Maestro-enabled repository.
---

# Maestro Setup

Use this skill after `maestro init` to tune the repository harness.

Inspect the repo structure, build and test commands, existing agent instructions, and current
workflow constraints. Update harness guidance only from verified repository evidence, and keep
setup changes small enough for future agents to trust and maintain.
"#;

const MAESTRO_VERIFY: &str = r#"---
name: maestro-verify
description: Verification protocol for Maestro tasks and feature work.
---

# Maestro Verify

Use this skill when proving a task or feature is complete.

Identify the smallest checks that can falsify the change, run them from the repository root, and
record exact commands and outcomes. If verification cannot run, state the blocker and the remaining
risk instead of marking the work complete.
"#;

const MAESTRO_DESIGN: &str = r#"---
name: maestro-design
description: Spec authoring and design grilling protocol for Maestro work.
---

# Maestro Design

Use this skill when turning a rough idea into a Maestro-ready spec or task plan.

Clarify the user-visible outcome, constraints, non-goals, acceptance checks, and rollout risks.
Prefer concrete examples and repository evidence over generic architecture language, then hand off a
plan that can be implemented and verified in small steps.
"#;

const BUNDLED_SKILLS: [BundledSkill; 4] = [
    BundledSkill {
        name: "maestro-task",
        description: "Feature and task workflow layer for operating the Maestro harness.",
        contents: MAESTRO_TASK,
    },
    BundledSkill {
        name: "maestro-setup",
        description: "Initial setup and harness tuning protocol for a Maestro-enabled repository.",
        contents: MAESTRO_SETUP,
    },
    BundledSkill {
        name: "maestro-verify",
        description: "Verification protocol for Maestro tasks and feature work.",
        contents: MAESTRO_VERIFY,
    },
    BundledSkill {
        name: "maestro-design",
        description: "Spec authoring and design grilling protocol for Maestro work.",
        contents: MAESTRO_DESIGN,
    },
];

/// Return the bundled skills in extraction order.
pub fn bundled_skills() -> &'static [BundledSkill] {
    &BUNDLED_SKILLS
}
