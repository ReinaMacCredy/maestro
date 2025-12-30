---
name: maestro-core
description: Use when any Maestro skill loads - provides skill hierarchy, HALT/DEGRADE policies, and trigger routing rules for orchestration decisions
metadata:
  version: "1.2.0"
---

# Maestro Core - Central Orchestrator

> **Load this skill first when using any Maestro skill. Defines hierarchy, fallback policies, and routing.**

## Skill Hierarchy

| Level | Skill | Role |
|-------|-------|------|
| 1 | maestro-core | Routing, fallback policy |
| 2 | conductor | Track orchestration, research protocol |
| 3 | orchestrator | Multi-agent parallel execution |
| 4 | design | Double Diamond |
| 5 | beads | Issue tracking |
| 6 | specialized | worktrees, sharing, writing |

## Fallback Policy

| Condition | Action |
|-----------|--------|
| `bd` unavailable | HALT |
| `conductor/` missing | DEGRADE |
| Village MCP unavailable | DEGRADE |

## Command Routing

### Conductor Commands

| Command | Routes To |
|---------|-----------|
| `ds`, `/conductor-design` | design → conductor |
| `/conductor-setup` | conductor |
| `/conductor-newtrack` | conductor |
| `ci`, `/conductor-implement` | conductor (auto-routes to orchestrator if Track Assignments exist) |
| `co`,`/conductor-orchestrate` | orchestrator (direct) |
| `cf`,`/conductor-finish` | conductor |
| `/conductor-status`, `-revert`, `-revise` | conductor |
| `/conductor-validate` | conductor |
| `/conductor-block`, `-skip` | conductor → beads |
| `/conductor-archive` | conductor |
| `/conductor-export` | conductor |
| `/research` | conductor (research) |

### Handoff Commands

| Command | Routes To |
|---------|-----------|
| `/create_handoff` | conductor (handoff) |
| `/resume_handoff` | conductor (handoff) |
| `/conductor-handoff` | conductor (handoff) |

### Doc-Sync Commands

| Command | Routes To |
|---------|-----------|
| `/doc-sync` | doc-sync |
| `/doc-sync --dry-run` | doc-sync (preview) |
| `/doc-sync --force` | doc-sync (apply all) |

### Beads Commands

| Command | Routes To |
|---------|-----------|
| `bd`, `bd ready`, `bd show` | beads |
| `fb`, `file-beads` | beads (file beads from plan) |
| `rb`, `review-beads` | beads (review filed beads) |

### Specialized Skills

| Trigger | Routes To |
|---------|-----------|
| "create skill", "write skill", "build skill" | writing-skills |
| "share skill", "contribute skill", "PR skill" | sharing-skills |
| "worktree", "isolated branch", "parallel branch" | using-git-worktrees |
| "run parallel", "spawn workers", "dispatch agents" | orchestrator |

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

### Command-Based Routing

```
IF explicit command (/conductor-*, /doc-sync, /create_handoff, etc.)
  → Route to named skill/workflow
  → EXCEPTION: `ci`/`/conductor-implement` checks Track Assignments first
    → If Track Assignments in plan.md → orchestrator
    → Else → conductor (sequential)

ELSE IF "design" or "brainstorm" or "think through"
  → design

ELSE IF "research" or "understand code" or "document how"
  → conductor (research protocol)

ELSE IF "handoff" or "save session" or "resume session"
  → conductor (handoff)

ELSE IF "sync docs" or "update documentation"
  → doc-sync

ELSE IF "run parallel" or "spawn workers" or "dispatch agents"
  → orchestrator

ELSE IF plan.md has "Track Assignments" section
  → orchestrator

ELSE IF "track" or "create task"
  → IF conductor/ exists → conductor
    ELSE → beads

ELSE IF "blocking" or "ready" or "dependencies"
  → beads

ELSE IF "create skill" or "write skill"
  → writing-skills

ELSE IF "share skill" or "contribute"
  → sharing-skills

ELSE IF "worktree" or "isolated branch"
  → using-git-worktrees
```

### Cross-Cutting Flows (Always-On)

These flows run automatically at specific workflow points:

#### Research Protocol Flow

```
ds (session start)
  → Auto-Research Context (Locator + Pattern + CODEMAPS)
      ↓
DISCOVER → DEFINE (Advisory ⚠️)
  → Locator + Pattern agents
      ↓
DEFINE → DEVELOP (Advisory ⚠️)
  → Locator + Pattern agents
      ↓
DEVELOP → DELIVER (Gatekeeper 🚫)
  → All 4 agents (Locator + Analyzer + Pattern + Web)
      ↓
DELIVER → Complete (Mandatory 🔒)
  → All 5 agents (+ Impact)
      ↓
Pre-newtrack
  → Full research verification
```

**Rule:** Research ALWAYS runs. No skip conditions. Parallel agents are fast.

#### Validation Gates Flow

```
ds → ... → DELIVER
              ↓
         [design gate] ────→ SPEED=WARN, FULL=HALT
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
         [plan-execution gate] ────→ SPEED=WARN, FULL=HALT
              ↓
    /conductor-finish
              ↓
         [completion gate] ────→ SPEED=WARN, FULL=HALT
              ↓
         Archive track
```

**State tracking in metadata.json:**
```yaml
validation:
  gates_passed: [design, spec, plan-structure]
  current_gate: plan-execution
  retries: 0          # max 2 before human escalation
  last_failure: null
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
| `/conductor-orchestrate` | `orchestrator/SKILL.md` + `orchestrator/references/workflow.md` |
| `/conductor-finish` | `conductor/references/finish-workflow.md` |
| `/create_handoff`, `/resume_handoff` | `conductor/references/handoff/` |
| `/doc-sync` | `conductor/references/doc-sync/` |
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
| **Session start** | Auto-load handoffs (see below) |
| `ds` | Load context |
| `/conductor-implement` | Load + bind track/bead |
| `/conductor-finish` | Handoff + archive |

### Auto-Load Handoffs (First Message)

**On first message of any session**, before processing the user's request:

1. Check if `conductor/handoffs/` exists
2. Scan for recent handoffs (< 7 days old)
3. If found:
   ```
   📋 Prior session context found:
   
   • [track-name] (2h ago) - trigger: summary
   
   Loading context...
   ```
4. Load the most recent handoff silently
5. Proceed with user's request

**Skip conditions:**
- User explicitly says "fresh start" or "new session"
- No `conductor/` directory exists
- All handoffs are > 7 days old (show stale warning instead)

**Stale handoff behavior:**
```
⚠️ Stale handoff found (12 days old):
   [track-name] - design-end

Load anyway? [Y/n/skip]
```

This ensures session continuity in Amp without requiring manual `/resume_handoff`.

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
