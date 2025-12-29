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
ORCHESTRATION          CORE WORKFLOW          DEVELOPMENT              UTILITIES
├── maestro-core       ├── conductor          ├── using-git-worktrees  ├── sharing-skills
│   └── references/    │   ├── references/    └── writing-skills       
│       ├── hierarchy.md   │   ├── prompts/          
│       └── routing.md     │   ├── coordination/     
│                          │   ├── tdd/              
│                          │   ├── verification/     
│                          │   ├── doc-sync/         
│                          │   ├── ledger/           
│                          │   └── finish/           
│                      ├── design             
│                      │   ├── bmad/          
│                      │   └── grounding/     
│                      └── beads
```

**7 Core Skills (after maestro-core_20251229):**
- **maestro-core**: Central orchestrator (hierarchy, HALT/DEGRADE, routing)
- **conductor**: Planning + execution (absorbed 9 skills into references/)
- **design**: Double Diamond + Party Mode + Grounding
- **beads**: Issue tracking + persistent memory
- **using-git-worktrees**: Isolated development environments
- **writing-skills**: Skill creation guide + dependency documentation
- **sharing-skills**: Upstream contribution

## Skill Hierarchy (maestro-core)

| Level | Skill | Role |
|-------|-------|------|
| 1 | maestro-core | Routing decisions, fallback policy |
| 2 | conductor | Track orchestration, workflow state |
| 3 | design | Design sessions (Double Diamond) |
| 4 | beads | Issue tracking, dependencies |
| 5 | specialized | worktrees, sharing, writing |

**Higher levels override lower levels on conflicts.**

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

**Enforcement Levels:** Advisory (warn) → Gatekeeper (block if missing) → Mandatory (block if fails)

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
