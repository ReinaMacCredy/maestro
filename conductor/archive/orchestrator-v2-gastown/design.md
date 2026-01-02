# Design: Orchestrator v2 - Gas Town Philosophy

**Track ID:** orchestrator-v2-gastown
**Created:** 2026-01-03
**Status:** Design Complete

## Problem Statement

Current orchestrator lacks durability, self-propulsion, and structured coordination. Work state lives in session memory, workers wait for commands instead of self-driving, and orchestration pollutes git history.

## Inspiration

Steve Yegge's [Gas Town](https://github.com/steveyegge/gastown) orchestrator philosophy:
- **Hooks**: Persistent work assignment that survives crashes
- **MEOW Stack**: Molecular Expression Of Work (formulas → protomolecules → molecules → wisps)
- **Specialized Roles**: Mayor, Witness, Refinery, Deacon, Polecats
- **Nondeterministic Idempotence**: Work eventually completes via retry
- **Propulsion Principle**: "If hook has work, RUN IT"

## Design Goals

| Goal | Description |
|------|-------------|
| **Durable Work** | Tasks survive crashes via Beads assignment + Agent Mail signals |
| **Self-Propulsion** | Workers check inbox/beads on start and execute immediately |
| **Ephemeral Orchestration** | Wisps for patrol/monitoring without git noise |
| **Typed Coordination** | Structured message protocol (YAML frontmatter) |
| **Integrated Monitoring** | Witness patrol inside orchestrator loop |

## What We're NOT Doing

| Gas Town Feature | Why Excluded |
|------------------|--------------|
| Hook mechanism | Amp has no persistent sessions |
| Deacon daemon | No background process in Amp |
| tmux management | Out of scope (Amp manages sessions) |
| Multi-rig (Mayor) | Single project focus for now |

## Solution: Hybrid Hook Pattern

Since Amp doesn't have persistent sessions like Gas Town's tmux, we use:

```
┌─────────────────────────────────────────────────────────────┐
│                    SEPARATION OF CONCERNS                   │
├─────────────────────────────────────────────────────────────┤
│  AGENT MAIL = Coordination & Signals                        │
│  • "Wake up, you have work" (ASSIGN, WAKE)                  │
│  • "I'm blocked on X" (BLOCKED)                             │
│  • "I finished, here's summary" (COMPLETED)                 │
├─────────────────────────────────────────────────────────────┤
│  BEADS = Work State & Assignment                            │
│  • What tasks exist (--assignee field)                      │
│  • Who owns what (--stale query)                            │
│  • Status, dependencies, notes                              │
└─────────────────────────────────────────────────────────────┘
```

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      ORCHESTRATOR v2                            │
├─────────────────────────────────────────────────────────────────┤
│  BEADS LAYER (State)                                            │
│  • assignee field    • stale detection   • wisp support         │
├─────────────────────────────────────────────────────────────────┤
│  PROTOCOL LAYER (Coordination)                                  │
│  • 11 message types  • YAML frontmatter  • Epic-scoped threads  │
├─────────────────────────────────────────────────────────────────┤
│  CONTROL LAYER                                                  │
│  • Dispatcher (assign + spawn)  • Witness Patrol (monitoring)   │
├─────────────────────────────────────────────────────────────────┤
│  WORKERS                          REFINERY                      │
│  • Self-propelling Task()s       • Post-completion review       │
└─────────────────────────────────────────────────────────────────┘
```

## Key Innovations

### 1. Message Protocol (11 Types)

| Type | Direction | Purpose |
|------|-----------|---------|
| ASSIGN | Orch → Worker | Assign tasks |
| WAKE | Orch → Worker | Signal to check beads |
| PING/PONG | Bidirectional | Health check |
| PROGRESS | Worker → Orch | Status update |
| BLOCKED | Worker → Orch | Cannot proceed |
| COMPLETED | Worker → Orch | Task done |
| FAILED | Worker → Orch | Task failed |
| STEAL | Orch → Worker | Take extra work |
| RELEASE | Worker → Orch | Give back work |
| ESCALATE | Any → Orch | Needs human |

### 2. Wisp Pattern (Ephemeral Beads)

```json
{
  "id": "W-047",
  "title": "Patrol run",
  "ephemeral": true,
  "status": "closed"
}
```

- Created with `bd create --wisp`
- Not committed to git
- Burned after use: `bd burn W-047`
- Optional digest: `bd squash W-047 --into=PATROL-LOG`

### 3. Witness Patrol (Integrated)

```
PATROL CYCLE (every 5min, exponential backoff):
├─ CHECK 1: Stale tasks (in_progress > 30min)
├─ CHECK 2: Unblocked tasks (dependency completed)
├─ CHECK 3: Load balance (redistribute if imbalance > 2)
└─ CHECK 4: Orphaned tasks (assignee=null)
```

### 4. Atomic Bead Claiming

```bash
bd update T-001 --status=in_progress --assignee=WorkerA --expect-status=open
```

New `--expect-status` flag for conditional updates (race safety).

## Edge Cases Covered

1. **Worker Crash**: Stale detection → PING → reassign if no PONG
2. **Reassignment**: Work stealing for load balancing
3. **Dependency Unblocks**: WAKE signal when blocker completes
4. **File Conflicts**: File reservation with BLOCKED message

## Success Criteria

| Criterion | Metric |
|-----------|--------|
| Crash Recovery | Worker crash → task reassigned within 35min |
| Self-Propulsion | Worker starts executing within 30s of session start |
| Git Cleanliness | Zero wisp beads in git history |
| Message Parsing | 100% structured messages parseable |

## Oracle Audit Findings

1. ✅ Need formal message catalog → Added to spec
2. ✅ Atomic bead claim semantics → `--expect-status` flag
3. ✅ Patrol/recovery command → `/conductor-patrol`
4. ✅ Refinery role documentation → Added to spec
5. 📋 Testing/simulation mode → Future enhancement
6. 📋 Observability metrics → Future enhancement
7. 📋 Cross-epic conflicts → Future enhancement
