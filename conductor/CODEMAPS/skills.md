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
CORE WORKFLOW              DEVELOPMENT              UTILITIES
├── conductor              ├── using-git-worktrees  ├── sharing-skills
│   ├── references/        └── writing-skills       
│   ├── research/            
│   ├── coordination/     
│   ├── tdd/              
│   ├── verification/     
│   ├── doc-sync/         
│   ├── handoff/          
│   └── finish/           
├── orchestrator           
│   └── references/        
│       ├── patterns/      
│       └── examples/      
├── design             
│   └── bmad/          
└── beads
```

**7 Core Skills:**
- **orchestrator**: Multi-agent parallel execution with autonomous workers + **file-scope routing**
- **conductor**: Planning + execution + **research protocol** + **file-scope detection**
- **design**: Double Diamond + Party Mode + Research verification
- **beads**: Issue tracking + persistent memory + **auto-orchestration trigger**
- **using-git-worktrees**: Isolated development environments
- **writing-skills**: Skill creation guide + dependency documentation
- **sharing-skills**: Upstream contribution

## Skill Hierarchy

Routing and fallback policies are defined in [AGENTS.md](../AGENTS.md).

| Level | Skill | Role |
|-------|-------|------|
| 1 | conductor | Track orchestration, workflow state, **research** |
| 2 | orchestrator | **Multi-agent parallel execution** |
| 3 | design | Design sessions (Double Diamond) |
| 4 | beads | Issue tracking, dependencies |
| 5 | specialized | worktrees, sharing, writing |

**Higher levels override lower levels on conflicts.**

## Research Protocol (skills/orchestrator/agents/)

> **Agent directory for multi-agent parallel execution.**

```
agents/
├── README.md              # Agent index + routing table
├── research/              # Information gathering agents
│   ├── codebase-locator.md    # Find WHERE files exist
│   ├── codebase-analyzer.md   # Understand HOW code works
│   ├── pattern-finder.md      # Find existing conventions
│   ├── impact-assessor.md     # Assess change impact
│   ├── web-researcher.md      # External docs
│   └── github-researcher.md   # GitHub-specific research
├── review/                # Code quality agents
│   ├── security-reviewer.md   # Security vulnerability analysis
│   ├── code-reviewer.md       # Code quality review
│   ├── pr-reviewer.md         # Pull request review
│   ├── spec-reviewer.md       # Specification validation
│   └── oracle.md              # 6-dimension design audit at CP4
├── planning/              # Design decision agents
│   ├── plan-agent.md          # Create plans
│   └── validate-agent.md      # Validate plans/specs
├── execution/             # Implementation agents
│   ├── implement-agent.md     # TDD implementation
│   └── worker-agent.md        # Autonomous worker
└── debug/                 # Investigation agents
    └── debug-agent.md         # Root cause analysis
```

**All agents include mandatory `send_message()` call before returning.**

## Thin Router Pattern (AGENTS.md)

Main thread stays clean: understand intent → route to specialist → display summary.

```
User Request → AGENTS.md (routing) → Task(agent) → send_message() → Summary
```

**Key files:**
- `AGENTS.md` - Thin router section (~70 lines)
- `skills/orchestrator/references/intent-routing.md` - Intent → agent mapping
- `skills/orchestrator/references/agent-routing.md` - Spawn patterns

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

**Lazy References (Token Optimization):**

Orchestrator uses trigger-based reference loading to reduce pre-spawn tokens:

| Phase | Reference Loaded |
|-------|------------------|
| Always | SKILL.md only (~400 tokens) |
| Phase 3 (Initialize) | agent-mail.md |
| Phase 4 (Spawn) | worker-prompt.md |
| Phase 6 (Handle Issues) | agent-coordination.md |

See `skills/orchestrator/SKILL.md` `## Lazy References` section.

## Gotchas

- Directory name must be kebab-case
- `name` in frontmatter must match directory name
- Keep skills self-contained; minimize cross-references
- Research ALWAYS runs (no skip conditions)
