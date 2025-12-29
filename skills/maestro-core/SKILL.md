---
name: maestro-core
description: Use when any Maestro skill loads - provides skill hierarchy, HALT/DEGRADE policies, and trigger routing rules for orchestration decisions
metadata:
  version: "1.1.0"
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

## Command Routing

> **This is the SINGLE SOURCE OF TRUTH for all routing decisions.**
> Conductor and other skills only execute - they do not route.

### Conductor Commands

| Command | Routes To | Execution |
|---------|-----------|-----------|
| `/conductor-setup` | conductor | Initialize project context |
| `/conductor-design` | design → conductor | Double Diamond → save design.md |
| `/conductor-newtrack` | conductor | Generate spec + plan + file beads |
| `/conductor-implement` | conductor | Execute ONE epic with TDD |
| `/conductor-status` | conductor | Display progress overview |
| `/conductor-revert` | conductor | Git-aware revert |
| `/conductor-revise` | conductor | Update spec/plan mid-track |
| `/conductor-finish` | conductor | Complete track, extract learnings |
| `/conductor-validate` | conductor | Run validation checks |
| `/conductor-block` | conductor → beads | Mark task as blocked |
| `/conductor-skip` | conductor → beads | Skip current task |
| `/conductor-archive` | conductor | Archive completed tracks |
| `/conductor-export` | conductor | Generate export summary |

### Intent Mapping (Natural Language → Command)

| User Intent | Routes To | Command |
|-------------|-----------|---------|
| "Set up this project" / "Initialize conductor" | conductor | `/conductor-setup` |
| "Design a feature" / "Brainstorm X" | design | `/conductor-design` |
| "Create a new feature" / "Add a track for X" | conductor | `/conductor-newtrack` |
| "Start working" / "Implement the feature" | conductor | `/conductor-implement` |
| "What's the status?" / "Show progress" | conductor | `/conductor-status` |
| "Undo that" / "Revert the last task" | conductor | `/conductor-revert` |
| "Check for issues" / "Validate the project" | conductor | `/conductor-validate` |
| "This is blocked" / "Can't proceed" | conductor | `/conductor-block` |
| "Skip this task" | conductor | `/conductor-skip` |
| "Archive completed tracks" | conductor | `/conductor-archive` |
| "Export project summary" | conductor | `/conductor-export` |
| "Spec is wrong" / "Plan needs update" | conductor | `/conductor-revise` |
| "Finish track" / "Complete track" | conductor | `/conductor-finish` |

### Trigger Disambiguation

| Trigger | Context | Routes To |
|---------|---------|-----------|
| `ds` | Any | design |
| `/conductor-*` | Any | conductor (see table above) |
| "design a feature" / "brainstorm" / "think through" | Any | design |
| "research codebase" / "/research" / "understand this code" | Any | conductor (research) |
| "track this work" / "create task for" | `conductor/` exists | conductor |
| "track this work" / "create task for" | no `conductor/` | beads |
| "what's blocking" / "what's ready" | Any | beads |
| `bd ready`, `bd show`, `fb`, `rb` | Any | beads |
| worktree creation | Implementation start | using-git-worktrees |
| "share this skill" | Any | sharing-skills |
| "create a skill" | Any | writing-skills |

### Routing Logic

```
IF explicit command (ds, /conductor-*, bd, /research)
  → Route to named skill

ELSE IF "research" or "understand code" or "document how"
  → Route to conductor (research protocol)

ELSE IF "design" or "brainstorm" or "think through"
  → Route to design

ELSE IF "track" or "create task"
  → IF conductor/ exists → conductor
    ELSE → beads

ELSE IF "blocking" or "ready" or "dependencies"
  → Route to beads

ELSE IF implementation context
  → IF worktree needed → using-git-worktrees
    ELSE → conductor
```

## Beads vs TodoWrite

| Scenario | Use |
|----------|-----|
| Multi-session work | Beads |
| Complex dependencies | Beads |
| Must survive compaction | Beads |
| Single-session tasks | TodoWrite |
| Linear execution | TodoWrite |
| Conversation-scoped only | TodoWrite |

**Rule:** If resuming in 2 weeks would be hard without bd, use bd.

## Double Diamond Routing

| Trigger | Routes To | Phase |
|---------|-----------|-------|
| `ds` | design | Start DISCOVER |
| `/conductor-design` | design | Start DISCOVER |
| "design a feature" / "brainstorm" | design | Start DISCOVER |
| `[A]` at checkpoint | design | Advanced analysis |
| `[P]` at checkpoint | design (Party Mode) | Multi-agent feedback |
| `[C]` at checkpoint | design | Continue to next phase |
| `[↩ Back]` at checkpoint | design | Return to previous phase |
| Design approved | conductor | Save design.md |

### Phase Flow

```
DISCOVER (Diverge) → DEFINE (Converge) → DEVELOP (Diverge) → DELIVER (Converge)
     ↓                    ↓                    ↓                    ↓
  A/P/C               A/P/C                A/P/C               A/P/C
```

### Complexity-Based Routing

| Score | Route | Description |
|-------|-------|-------------|
| < 4 | SPEED MODE | 1-phase quick design, minimal ceremony |
| 4-6 | ASK USER | "[S]peed or [F]ull?" |
| > 6 | FULL MODE | 4-phase Double Diamond with A/P/C |

## Validation System Lifecycle

5 validation gates integrated into workflow:

| Gate | Trigger Point | Enforcement |
|------|---------------|-------------|
| `design` | After DELIVER phase | SPEED=WARN, FULL=HALT |
| `spec` | After spec.md generation | WARN (both modes) |
| `plan-structure` | After plan.md generation | WARN (both modes) |
| `plan-execution` | After TDD REFACTOR | SPEED=WARN, FULL=HALT |
| `completion` | Before `/conductor-finish` | SPEED=WARN, FULL=HALT |

### Gate Routing

```
ds → ... → DELIVER
              ↓
         [design gate] ────→ HALT if fails (FULL mode)
              ↓
    /conductor-newtrack
              ↓
         [spec gate] ────→ WARN only
              ↓
         [plan-structure gate] ────→ WARN only
              ↓
    /conductor-implement
              ↓
         TDD: RED → GREEN → REFACTOR
              ↓
         [plan-execution gate] ────→ HALT if fails (FULL mode)
              ↓
    /conductor-finish
              ↓
         [completion gate] ────→ HALT if fails (FULL mode)
              ↓
         Archive track
```

### LEDGER Validation State

```yaml
validation:
  gates_passed: [design, spec, plan-structure]
  current_gate: plan-execution
  retries: 0          # max 2 before human escalation
  last_failure: null
```

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
- [routing.md](references/routing.md) - Worktree invocation, edge cases, extended details
