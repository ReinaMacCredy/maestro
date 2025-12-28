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
│   ├── bmad/          └── finishing-branch     └── subagent-driven-dev
│   └── grounding/
└── beads
```

## Grounding System (skills/design/references/grounding/)

```
grounding/
├── tiers.md           # Light/Mini/Standard/Full tier definitions
├── router.md          # Cascading router (repo → web → history)
├── cache.md           # Session cache (5 min TTL)
├── sanitization.md    # Query sanitization for external calls
├── schema.json        # Result schema v1.1
└── impact-scan-prompt.md  # Subagent template for DELIVER phase
```

**Enforcement Levels:** Advisory ⚠️ → Gatekeeper 🚫 → Mandatory 🔒

## BMAD Integration (skills/design/references/bmad/)

```
bmad/
├── agents/            # 25 expert agents
│   ├── core/          # BMad Master (orchestrator)
│   ├── bmm/           # 9 business/management agents
│   └── cis/           # 6 creative/innovation agents
├── workflows/         # 6 CIS workflows
│   ├── party-mode/    # Multi-agent collaboration
│   ├── brainstorming/ # 36 ideation techniques
│   ├── design-thinking/
│   ├── innovation-strategy/
│   ├── problem-solving/
│   └── storytelling/
├── config.yaml        # Maestro-specific settings
├── manifest.yaml      # Agent registry (25 agents)
└── adapter.md         # Path transforms for upstream sync
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
