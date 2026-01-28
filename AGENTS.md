# AGENTS.md - Maestro Plugin

Atlas workflow plugin: Interview-driven planning, Task()-based delegation, TDD execution.

## Project Detection

| Condition | Result |
|-----------|--------|
| `.atlas/` exists | Use Atlas workflow |
| `.beads/` exists | Use Beads tracking |
| Neither | Standalone mode |

## Decision Trees

### bd vs TodoWrite

```
bd available?
├─ YES → Use bd CLI
└─ NO  → HALT (do not use TodoWrite as fallback)
```

### Execution Mode

Always FULL mode via orchestrator. Even single tasks spawn 1 worker for consistency.

## Commands Quick Reference

### Planning

| Trigger | Action |
|---------|--------|
| `/atlas-plan <request>` | Start Prometheus interview mode |
| `@plan`, `ultraplan` | Prometheus interview mode |
| `@metis` | Pre-planning consultation |
| `@momus` | Plan review |

### Execution

| Trigger | Action |
|---------|--------|
| `/atlas-work` | Execute plan via orchestrator |
| `/ralph-loop` | Autonomous execution (Ralph loop) |
| `/cancel-ralph` | Stop Ralph loop |
| `@tdd` | Enter TDD mode (atlas-kraken) |
| `finish branch` | Finalize and merge/PR |

### Research

| Trigger | Action |
|---------|--------|
| `@oracle` | Strategic advisor (opus) |
| `@explore` | Codebase search |
| `@librarian` | External docs/research |
| `@review` | Code quality review |
| `@docs` | Documentation writer |

### Beads

| Command | Action |
|---------|--------|
| `fb` | File beads from plan |
| `rb` | Review beads |
| `bd status` | Show ready + in_progress |
| `bd show <id>` | Read task context |
| `bd update <id> --status in_progress` | Claim task |
| `bd close <id> --reason <completed\|skipped\|blocked>` | Close task |
| `bd sync` | Sync to git |

## Session Protocol

### First Message

1. Check `.atlas/plans/` for active plans
2. If found: display plan status and available work
3. Skip if: "fresh start", no `.atlas/`, or plans stale

### Session Start

```bash
bd ready --json                      # Find work
bd show <id>                         # Read context
bd update <id> --status in_progress  # Claim
```

### During Session

- TDD checkpoints tracked by default
- Orchestrator delegates ALL work (never edits directly)

### Session End

```bash
bd update <id> --notes "COMPLETED: X. NEXT: Y"
bd close <id> --reason completed
bd sync
```

### Ralph (Autonomous Mode)

| Phase | Action |
|-------|--------|
| Start | `/ralph-loop` activates autonomous execution |
| During | Ralph iterates through tasks, updates progress |
| End | Detection of `<promise>DONE</promise>` stops loop |

**Exclusive Lock:** Manual commands blocked while Ralph is active.

## Fallback Policy

| Condition | Action |
|-----------|--------|
| `bd` unavailable | HALT |
| `.atlas/` missing | DEGRADE (standalone) |
| Agent Mail unavailable | HALT |

## Skill Discipline

**RULE:** Atlas skill auto-loads for all `@keyword` triggers and `/atlas-*` commands.

**RULE:** Check skills BEFORE ANY RESPONSE. 1% chance = invoke Skill tool.

### Red Flags (Rationalizing)

| Thought | Reality |
|---------|---------|
| "Just a simple question" | Questions are tasks. Check. |
| "Need more context first" | Skill check BEFORE clarifying. |
| "Let me explore first" | Skills tell HOW to explore. |
| "Doesn't need formal skill" | If skill exists, use it. |
| "I remember this skill" | Skills evolve. Re-read. |
| "Skill is overkill" | Simple → complex. Use it. |

### Skill Priority

1. **Atlas workflow** (`@plan`, `/atlas-plan`) → planning and execution
2. **Specialized agents** (`@oracle`, `@explore`, `@librarian`) → research
3. **Implementation** (`@tdd`, `/atlas-work`) → execution

### Skill Types

| Type | Behavior |
|------|----------|
| Rigid (TDD) | Follow exactly |
| Flexible (patterns) | Adapt to context |

## Directory Structure

```
.atlas/
├── plans/                    # Committed work plans
├── drafts/                   # Interview drafts
├── notepads/                 # Wisdom per plan
└── boulder.json              # Active execution state

.claude/
├── agents/                   # Agent definitions (symlinks)
├── commands/                 # Slash commands
├── hooks/                    # Hook configuration
├── plans/                    # Generated execution plans
├── scripts/                  # Hook scripts
└── skills/
    └── atlas/                # Main workflow skill
        └── references/
            └── agents/       # Atlas agent definitions
```

## Build/Test

```bash
cat .claude-plugin/plugin.json | jq .   # Validate manifest
```

## Code Style

- Skills: Markdown + YAML frontmatter (`name`, `description` required)
- Directories: kebab-case
- SKILL.md name must match directory

## Versioning

| Type | Method |
|------|--------|
| Plugin | CI auto-bump (`feat:` minor, `fix:` patch, `feat!:` major while 0.x) |
| Skill | Manual frontmatter update |
| Skip CI | `[skip ci]` in commit |

**Pre-1.0 Semantics**: While version is 0.x.x, breaking changes (`feat!:` or `release:major`) bump MINOR not MAJOR. This prevents accidental 1.0.0 release.

## Critical Rules

- Use `--json` with `bd` for structured output
- Use `--robot-*` with `bv` (bare `bv` hangs)
- Never write production code without failing test first
- Always commit `.beads/` with code changes
- Orchestrator NEVER edits directly - always delegates via Task()

## Detailed References

| Topic | Path |
|-------|------|
| Atlas workflow | [.claude/skills/atlas/SKILL.md](.claude/skills/atlas/SKILL.md) |
| Agent definitions | [.claude/skills/atlas/references/agents/](.claude/skills/atlas/references/agents/) |
| Router & keywords | [.claude/skills/atlas/references/workflows/router.md](.claude/skills/atlas/references/workflows/router.md) |
| Planning workflow | [.claude/skills/atlas/references/workflows/prometheus.md](.claude/skills/atlas/references/workflows/prometheus.md) |
| Execution workflow | [.claude/skills/atlas/references/workflows/execution.md](.claude/skills/atlas/references/workflows/execution.md) |
| Delegation guide | [.claude/skills/atlas/references/guides/delegation.md](.claude/skills/atlas/references/guides/delegation.md) |
