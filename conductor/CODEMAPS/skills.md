# Skills Codemap

How skills work in this plugin.

## Key Files

| File | Responsibility |
|------|----------------|
| `skills/<name>/SKILL.md` | Skill definition (frontmatter + instructions) |
| `skills/<name>/references/` | Optional supporting docs |
| `lib/skills-core.js` | Shared skill utilities |

## Skill Structure

```yaml
---
name: skill-name
version: "1.0.0"
description: When to use this skill
---

# Skill Name

Instructions for the agent...
```

## Skill Categories

```
CORE WORKFLOW          DEVELOPMENT              UTILITIES
├── conductor          ├── test-driven-dev      ├── codemaps
├── design             ├── using-git-worktrees  ├── dispatching-parallel-agents
├── beads              └── finishing-branch     └── subagent-driven-dev
├── file-beads
└── review-beads
```

## Adding a Skill

1. Create `skills/<kebab-name>/SKILL.md`
2. Add YAML frontmatter with `name`, `version`, `description`
3. Write instructions in markdown body
4. Optional: add `references/` subdirectory for templates

## Skill Loading

Skills are loaded when:
- User says trigger phrase (e.g., `ds`, `tdd`, `debug`)
- User runs slash command (e.g., `/conductor-setup`)
- Agent recognizes matching context

## Gotchas

- Directory name must be kebab-case
- `name` in frontmatter must match directory name
- Keep skills self-contained; minimize cross-references
