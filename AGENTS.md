# AGENTS.md - Maestro Plugin

Workflow skills plugin: Conductor, Design, Beads, Orchestrator.

## Project Detection

| Condition | Result |
|-----------|--------|
| `conductor/` exists | Use Conductor workflow |
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
| `ds` | Design session (Double Diamond) |
| `/conductor-setup` | Initialize project context |
| `/conductor-newtrack` | Create spec + plan + beads from design |

### Execution

| Trigger | Action |
|---------|--------|
| `bd ready --json` | Find available work |
| `/conductor-implement <track>` | Execute track with TDD |
| `/conductor-implement --no-tdd` | Execute without TDD |
| `tdd` | Enter TDD mode |
| `finish branch` | Finalize and merge/PR |

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

### Handoffs

| Command | Action |
|---------|--------|
| `/conductor-handoff` | Save session context |
| `/conductor-handoff resume` | Load session context |

### Maintenance

| Trigger | Action |
|---------|--------|
| `/conductor-revise` | Update spec/plan mid-work |
| `/conductor-finish` | Complete track, extract learnings |
| `/conductor-status` | Display progress overview |

## Session Protocol

### First Message

1. Check `conductor/handoffs/` for recent handoffs (< 7 days)
2. If found: `📋 Prior context: [track] (Xh ago)`
3. Skip if: "fresh start", no `conductor/`, or handoffs > 7 days

### Preflight Triggers

| Command | Preflight |
|---------|-----------|
| `/conductor-implement` | ✅ Yes |
| `/conductor-orchestrate` | ✅ Yes |
| `ds` | ❌ Skip |
| `bd ready/show/list` | ❌ Skip |

### Session Start

```bash
bd ready --json                      # Find work
bd show <id>                         # Read context
bd update <id> --status in_progress  # Claim
```

### During Session

- Heartbeat every 5 min (automatic)
- TDD checkpoints tracked by default
- Idle > 30min → prompt for handoff

### Session End

```bash
bd update <id> --notes "COMPLETED: X. NEXT: Y"
bd close <id> --reason completed
bd sync
```

### Session Identity

- Format: `{BaseAgent}-{timestamp}` (internal)
- Registered on `/conductor-implement` or `/conductor-orchestrate`
- Stale threshold: 10 min → takeover prompt

## Fallback Policy

| Condition | Action |
|-----------|--------|
| `bd` unavailable | HALT |
| `conductor/` missing | DEGRADE (standalone) |
| Agent Mail unavailable | HALT |

## Skill Discipline

**RULE:** Load [maestro-core](.claude/skills/maestro-core/SKILL.md) FIRST before any workflow skill for routing table and fallback policies.

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

1. **Process skills** (`ds`, `/conductor-design`) → determine approach
2. **Implementation skills** (`frontend-design`, `mcp-builder`) → guide execution

### Skill Types

| Type | Behavior |
|------|----------|
| Rigid (TDD) | Follow exactly |
| Flexible (patterns) | Adapt to context |

## Directory Structure

```
conductor/
├── product.md, tech-stack.md, workflow.md  # Context
├── CODEMAPS/                               # Architecture
├── handoffs/                               # Session context
└── tracks/<id>/                            # Per-track
    ├── design.md, spec.md, plan.md
    └── metadata.json
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
| Plugin | CI auto-bump (`feat:` minor, `fix:` patch, `feat!:` major) |
| Skill | Manual frontmatter update |
| Skip CI | `[skip ci]` in commit |

## Critical Rules

- Use `--json` with `bd` for structured output
- Use `--robot-*` with `bv` (bare `bv` hangs)
- Never write production code without failing test first
- Always commit `.beads/` with code changes

## Detailed References

| Topic | Path |
|-------|------|
| Beads workflow | [.claude/skills/beads/references/workflow-integration.md](.claude/skills/beads/references/workflow-integration.md) |
| Handoff system | [.claude/skills/conductor/references/workflows/handoff.md](.claude/skills/conductor/references/workflows/handoff.md) |
| Agent coordination | [.claude/skills/orchestrator/references/agent-coordination.md](.claude/skills/orchestrator/references/agent-coordination.md) |
| Router | [.claude/skills/orchestrator/references/router.md](.claude/skills/orchestrator/references/router.md) |
| Beads integration | [.claude/skills/conductor/references/beads-integration.md](.claude/skills/conductor/references/beads-integration.md) |
| TDD checkpoints | [.claude/skills/conductor/references/tdd-checkpoints-beads.md](.claude/skills/conductor/references/tdd-checkpoints-beads.md) |
| Idle detection | [.claude/skills/conductor/references/workflows/handoff.md](.claude/skills/conductor/references/workflows/handoff.md) |
