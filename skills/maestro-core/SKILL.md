---
name: maestro-core
description: Use when any Maestro skill loads - provides skill hierarchy, HALT/DEGRADE policies, and trigger routing rules for orchestration decisions
metadata:
  version: "1.1.0"
---

# Maestro Core - Central Orchestrator

> **Load this skill first when using any Maestro skill. Defines hierarchy, fallback policies, and routing.**

## Skill Hierarchy

| Level | Skill | Role |
|-------|-------|------|
| 1 | maestro-core | Routing, fallback policy |
| 2 | conductor | Track orchestration, research protocol |
| 3 | design | Double Diamond |
| 4 | beads | Issue tracking |
| 5 | specialized | worktrees, sharing, writing |

## Fallback Policy

| Condition | Action |
|-----------|--------|
| `bd` unavailable | HALT |
| `conductor/` missing | DEGRADE |
| Village MCP unavailable | DEGRADE |

## Command Routing

| Command | Routes To |
|---------|-----------|
| `ds`, `/conductor-design` | design → conductor |
| `/conductor-setup` | conductor |
| `/conductor-newtrack` | conductor |
| `/conductor-implement` | conductor |
| `/conductor-finish` | conductor |
| `/conductor-status`, `-revert`, `-revise` | conductor |
| `/conductor-block`, `-skip` | conductor → beads |
| `/research` | conductor (research) |
| `bd`, `fb`, `rb` | beads |

## Research Routing

| Trigger | Routes To | Agents |
|---------|-----------|--------|
| `/research`, "research codebase" | conductor | Parallel sub-agents |
| "understand this code" | conductor | Locator + Analyzer |
| "document how X works" | conductor | Pattern + Analyzer |
| DISCOVER→DEFINE transition | conductor | Locator + Pattern |
| DEVELOP→DELIVER transition | conductor | All 4 agents |
| Pre-newtrack | conductor | All 4 agents + Impact |

### Research Agents

| Agent | Role |
|-------|------|
| Locator | Find WHERE files exist |
| Analyzer | Understand HOW code works |
| Pattern | Find existing conventions |
| Web | External docs (when needed) |
| Impact | Assess change impact (DELIVER only) |

## Routing Logic

```
IF explicit command → named skill
ELSE IF "design/brainstorm" → design
ELSE IF "research/understand" → conductor (research)
ELSE IF "track/task" → conductor (if exists) ELSE beads
ELSE IF "blocking/ready" → beads
```

## Double Diamond Routing

```
DISCOVER → DEFINE → DEVELOP → DELIVER
    ↓         ↓         ↓         ↓
  A/P/C     A/P/C     A/P/C     A/P/C
```

| Score | Route |
|-------|-------|
| < 4 | SPEED (1-phase) |
| 4-6 | ASK USER |
| > 6 | FULL (4-phase) |

## Validation Gates

```
ds → DELIVER → [design] → newtrack → [spec] → [plan] → implement → TDD → [execution] → finish → [completion]
```

| Gate | Enforcement |
|------|-------------|
| design, execution, completion | SPEED=WARN, FULL=HALT |
| spec, plan-structure | WARN only |

## Authoritative Workflow Docs

> **Single Source of Truth:** Each command has a detailed workflow file that defines the full behavior including validation gates. Always load the authoritative doc, not summaries.

| Command | Authoritative Doc |
|---------|-------------------|
| `/conductor-setup` | `conductor/references/workflows/setup.md` |
| `/conductor-design` | `design/SKILL.md` + `design/references/` |
| `/conductor-newtrack` | `conductor/references/workflows/newtrack.md` |
| `/conductor-implement` | `conductor/references/workflows/implement.md` |
| `/conductor-finish` | `conductor/references/finish-workflow.md` |
| TDD cycle | `conductor/references/tdd/cycle.md` |

**Validation gate implementations:**
- `validate-design` → `design/SKILL.md` (DELIVER section)
- `validate-spec`, `validate-plan-structure` → `conductor/references/workflows/newtrack.md`
- `validate-plan-execution` → `conductor/references/tdd/cycle.md`
- `validate-completion` → `conductor/references/finish-workflow.md`

**Note:** `conductor/references/workflows.md` is an index only. Do not use it for workflow execution.

## Session Lifecycle

| Entry | Action |
|-------|--------|
| `ds` | Load context |
| `/conductor-implement` | Load + bind track/bead |
| `/conductor-finish` | Handoff + archive |

### Idle Detection

On every user message, before routing:

1. Check `conductor/.last_activity` mtime
2. If gap > 30min (configurable in `workflow.md`):
   ```
   ⏰ It's been X minutes. Create handoff? [Y/n/skip]
   ```
3. Y = create handoff, n = skip once, skip = disable for session

See [conductor/references/handoff/idle-detection.md](../conductor/references/handoff/idle-detection.md).

## Beads vs TodoWrite

| Use Beads | Use TodoWrite |
|-----------|---------------|
| Multi-session | Single-session |
| Dependencies | Linear |
| Survives compaction | Conversation-scoped |

## Prerequisites Pattern

All Maestro skills should load maestro-core first:

```markdown
## Prerequisites

**REQUIRED SUB-SKILL:** [maestro-core](../maestro-core/SKILL.md)

Load maestro-core first for orchestration context.
```

## References

- [hierarchy.md](references/hierarchy.md) - HALT/DEGRADE matrix
- [routing.md](references/routing.md) - Worktree, edge cases
