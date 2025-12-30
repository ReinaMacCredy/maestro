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
│       ├── hierarchy.md   │   ├── research/        
│       └── routing.md     │   ├── coordination/     
├── orchestrator           │   ├── tdd/              
│   └── references/        │   ├── verification/     
│       ├── patterns/      │   ├── doc-sync/         
│       └── examples/      │   ├── handoff/          
│                          │   └── finish/           
│                      ├── design             
│                      │   └── bmad/          
│                      └── beads
```

**8 Core Skills (after orchestrator_20251230):**
- **maestro-core**: Central orchestrator (hierarchy, HALT/DEGRADE, routing)
- **orchestrator**: Multi-agent parallel execution with autonomous workers
- **conductor**: Planning + execution + **research protocol**
- **design**: Double Diamond + Party Mode + Research verification
- **beads**: Issue tracking + persistent memory + **auto-orchestration trigger**
- **using-git-worktrees**: Isolated development environments
- **writing-skills**: Skill creation guide + dependency documentation
- **sharing-skills**: Upstream contribution

## Skill Hierarchy (maestro-core)

| Level | Skill | Role |
|-------|-------|------|
| 1 | maestro-core | Routing decisions, fallback policy |
| 2 | conductor | Track orchestration, workflow state, **research** |
| 3 | orchestrator | **Multi-agent parallel execution** |
| 4 | design | Design sessions (Double Diamond) |
| 5 | beads | Issue tracking, dependencies |
| 6 | specialized | worktrees, sharing, writing |

**Higher levels override lower levels on conflicts.**

## Research Protocol (skills/conductor/references/research/)

> **Replaces the old grounding system with parallel sub-agents.**

```
research/
├── protocol.md        # Main research protocol (always runs)
├── agents/            # Parallel sub-agents
│   ├── codebase-locator.md    # Find WHERE files exist
│   ├── codebase-analyzer.md   # Understand HOW code works
│   ├── pattern-finder.md      # Find existing conventions
│   └── web-researcher.md      # External docs (when needed)
└── hooks/             # Auto-trigger integration points
    ├── discover-hook.md   # ds start → research context
    ├── grounding-hook.md  # DEVELOP→DELIVER verification
    └── newtrack-hook.md   # Pre-spec research
```

**Key Difference from Old Grounding:**
- ❌ OLD: Sequential (Grep → finder → web), tiered, skip conditions
- ✅ NEW: Parallel agents, always runs, no skip conditions

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
- User says trigger phrase (e.g., `ds`, `tdd`, `debug`, `/research`, "spawn workers")
- User runs slash command (e.g., `/conductor-setup`, `/conductor-orchestrate`)
- Agent recognizes matching context (e.g., plan.md has Track Assignments)

## Gotchas

- Directory name must be kebab-case
- `name` in frontmatter must match directory name
- Keep skills self-contained; minimize cross-references
- Research ALWAYS runs (no skip conditions)
