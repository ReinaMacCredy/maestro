---
name: maestro-core
description: Use when any Maestro skill loads - provides skill hierarchy, HALT/DEGRADE policies, and trigger routing rules for orchestration decisions
metadata:
  version: "1.0.0"
---

# Maestro Core - Central Orchestrator

Load this skill first when using any Maestro skill. Defines hierarchy, fallback policies, and routing.

## Quick Start

```
1. Check skill hierarchy (who decides?)
2. Apply HALT/DEGRADE policy
3. Route to correct skill
```

## Skill Hierarchy (5 Levels)

| Level | Skill | Role |
|-------|-------|------|
| 1 | maestro-core | Routing decisions, fallback policy |
| 2 | conductor | Track orchestration, workflow state |
| 3 | design | Design sessions (Double Diamond) |
| 4 | beads | Issue tracking, dependencies |
| 5 | specialized | worktrees, sharing, writing |

Higher levels override lower levels on conflicts. See [hierarchy.md](references/hierarchy.md).

## Fallback Policy

| Condition | Action | Message |
|-----------|--------|---------|
| `bd` CLI unavailable | HALT | ❌ Cannot proceed: bd CLI not found. Install beads_viewer. |
| `conductor/` missing | DEGRADE | ⚠️ Conductor unavailable. Standalone mode. |
| Village MCP unavailable | DEGRADE | ⚠️ Village unavailable. Using single-agent mode. |
| CODEMAPS missing | DEGRADE | ⚠️ No CODEMAPS found. Context limited. |

**Rule:** HALT only for dependencies that block ALL functionality. DEGRADE for optional features.

See [hierarchy.md](references/hierarchy.md) for full matrix.

## Trigger Routing

| Trigger | Context | Routes To |
|---------|---------|-----------|
| `ds`, "design a feature" | Any | design |
| "track this work" | conductor/ exists | conductor |
| "track this work" | no conductor/ | beads |
| "create task for" | conductor/ exists | conductor |
| "what's blocking" | Any | beads |
| worktree creation | Implementation start | using-git-worktrees |

See [routing.md](references/routing.md) for full disambiguation and decision rules.

## Beads vs TodoWrite

**Use Beads (bd) when:**
- Work spans multiple sessions
- Dependencies exist between tasks
- Context must survive compaction

**Use TodoWrite when:**
- Single-session linear execution
- No dependencies
- Conversation-scoped only

See [routing.md](references/routing.md) for decision flowchart.

## Prerequisites Pattern

All Maestro skills should load maestro-core first:

```markdown
## Prerequisites

**REQUIRED SUB-SKILL:** [maestro-core](../maestro-core/SKILL.md)

Load maestro-core first for orchestration context.
```

## Session Lifecycle

Session continuity is **automatic** via Conductor workflow entry points:

| Entry Point | Ledger Action |
|-------------|---------------|
| `ds` | Load prior context before DISCOVER phase |
| `/conductor-implement` | Load + bind to track/bead |
| `/conductor-finish` | Handoff + archive |

**No manual commands needed.** Conductor handles ledger operations at workflow boundaries.

For non-Conductor (ad-hoc) work, ledger operations are skipped to avoid overhead.

See [conductor/references/ledger/](../conductor/references/ledger/) for implementation details.

## References

- [hierarchy.md](references/hierarchy.md) - 5-level details, HALT/DEGRADE matrix
- [routing.md](references/routing.md) - Trigger disambiguation, decision rules
