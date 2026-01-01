---
name: orchestrator
description: Multi-agent parallel execution with autonomous workers. Use when plan.md has Track Assignments section or user triggers /conductor-orchestrate, "run parallel", "spawn workers".
metadata:
  version: "1.0.0"
---

# Orchestrator - Multi-Agent Parallel Execution

> **Spawn autonomous workers to execute tracks in parallel using Agent Mail coordination.**

## Prerequisites

Routing and fallback policies are defined in [AGENTS.md](../../AGENTS.md).

## When to Use

**Primary:** `/conductor-implement` auto-routes here when:
- Plan.md has **Track Assignments** section
- Complexity scoring passes (see [design routing heuristics](../design/references/design-routing-heuristics.md))

**Direct:** Also available via:
- User runs `/conductor-orchestrate` or `co`
- User says "run parallel", "spawn workers", "dispatch agents"

## Auto-Orchestration Integration

When triggered from `fb` (file beads) auto-orchestration:

1. Track Assignments are **auto-generated** from beads dependency graph
2. No manual Track Assignments section needed in plan.md
3. Orchestrator receives assignments via in-memory call, not file parsing

### Auto-Generated vs Manual

| Source | How Detected | Behavior |
|--------|--------------|----------|
| Auto-generated | Called from fb Phase 6 | Assignments passed in-memory |
| Manual | User runs `/conductor-orchestrate` | Parse from plan.md |

Both flows converge at Phase 3 (Spawn Workers).

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              ORCHESTRATOR                                   │
│                              (Main Agent)                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│  1. Read plan.md Track Assignments                                          │
│  2. Initialize Agent Mail                                                   │
│  3. Spawn workers via Task()                                                │
│  4. Monitor progress via fetch_inbox                                        │
│  5. Handle cross-track blockers                                             │
│  6. Announce completion                                                     │
└─────────────────────────────────────────────────────────────────────────────┘
           │
           │ Task() spawns parallel workers
           ▼
┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐
│  Worker A        │  │  Worker B        │  │  Worker C        │
│  Track 1         │  │  Track 2         │  │  Track 3         │
├──────────────────┤  ├──────────────────┤  ├──────────────────┤
│  For each bead:  │  │  For each bead:  │  │  For each bead:  │
│  • Reserve files │  │  • Reserve files │  │  • Reserve files │
│  • bd claim      │  │  • bd claim      │  │  • bd claim      │
│  • Do work       │  │  • Do work       │  │  • Do work       │
│  • bd close      │  │  • bd close      │  │  • bd close      │
│  • Report mail   │  │  • Report mail   │  │  • Report mail   │
└──────────────────┘  └──────────────────┘  └──────────────────┘
           │                   │                   │
           └───────────────────┼───────────────────┘
                               ▼
                    ┌─────────────────────┐
                    │     Agent Mail      │
                    │  ─────────────────  │
                    │  Epic Thread:       │
                    │  • Progress reports │
                    │  • Bead completions │
                    │  • Blockers         │
                    └─────────────────────┘
```

## Key Difference from /conductor-implement

| Aspect | /conductor-implement | /conductor-orchestrate |
|--------|---------------------|----------------------|
| Execution | Sequential, main agent | Parallel, worker subagents |
| bd access | Main agent only | **Workers CAN claim/close** |
| Coordination | N/A | Agent Mail MCP (Full) or Task return (Light) |
| File locking | N/A | file_reservation_paths |
| Context | In-memory | Track threads (persistent) |

## Coordination Modes

Orchestrator supports two modes based on task complexity and Agent Mail availability:

| Mode | Agent Mail | Heartbeats | Use Case |
|------|------------|------------|----------|
| **Light** | Not required | No | Simple parallel tasks, no cross-deps, tasks <10 min |
| **Full** | Required | Yes (>10 min) | Complex coordination, blockers, cross-track deps |

### Mode Selection

```python
# Auto-select mode based on conditions
if not agent_mail_available():
    mode = "LIGHT"  # Fallback
elif has_cross_track_deps(TRACKS):
    mode = "FULL"   # Need coordination
elif max_estimated_duration(TRACKS) < 10:  # minutes
    mode = "LIGHT"  # Simple tasks
else:
    mode = "FULL"   # Default for complex work
```

### Light Mode Behavior

- Workers execute via Task() and return structured results
- No Agent Mail registration, messaging, or heartbeats
- Orchestrator collects results from Task() return values
- Cross-track deps handled via Task() sequencing (spawn dependent tracks after blockers complete)

### Full Mode Behavior

- Full Agent Mail protocol (register, message, heartbeat)
- Real-time progress monitoring via fetch_inbox
- Cross-track dependency notifications
- Blocker resolution via reply_message

## 8-Phase Workflow

See [references/workflow.md](references/workflow.md) for full protocol:

0. **Preflight** - Session identity, detect active sessions, conflict warnings (NEW)
1. **Read Plan** - Parse Track Assignments from plan.md
2. **Validate** - Health check Agent Mail (HALT if unavailable)
3. **Initialize** - ensure_project, register_agent, create epic thread
4. **Spawn Workers** - Task() for each track (parallel, with routing)
5. **Monitor + Verify** - fetch_inbox, verify worker summaries
6. **Resolve** - reply_message for blockers, file conflicts
7. **Complete** - Verify, send summary, close epic, rb review

## Agent Routing

### Intent → Agent Mapping

The orchestrator routes tasks to specialized agents based on intent keywords. See [references/intent-routing.md](references/intent-routing.md) for the complete mapping.

| Intent Keywords | Agent Type | File Reservation |
|-----------------|------------|------------------|
| `research`, `find`, `locate` | Research | None (read-only) |
| `review`, `check`, `audit` | Review | None (read-only) |
| `plan`, `design`, `architect` | Planning | `conductor/tracks/**` |
| `implement`, `build`, `create` | Execution | Task-specific scope |
| `fix`, `debug`, `investigate` | Debug | None (read-only) |

### Agent Categories

| Category | Agents | Directory |
|----------|--------|-----------|
| Research | Locator, Analyzer, Pattern, Web | [agents/research/](agents/research/) |
| Review | CodeReview, SecurityAudit, PerformanceReview | [agents/review/](agents/review/) |
| Planning | Architect, Planner | [agents/planning/](agents/planning/) |
| Execution | Implementer, Modifier, Fixer, Refactorer | [agents/execution/](agents/execution/) |
| Debug | Debugger, Tracer | [agents/debug/](agents/debug/) |

See [agents/README.md](agents/README.md) for complete agent index and profiles.

## Session Brain (Phase 0)

The orchestrator includes a "session brain" that coordinates multiple Amp sessions:

- **Auto-registration**: Sessions register identity with Agent Mail on startup
- **Conflict detection**: Warns when sessions work on same track/files/beads
- **Stale takeover**: Prompts to take over inactive sessions (>10 min)
- **Always-on**: Preflight runs automatically on /conductor-implement and /conductor-orchestrate

### Session Identity Format

- Internal: `{BaseAgent}-{timestamp}` (unique, e.g., `BlueLake-1735689600`)
- Display: `{BaseAgent} (session HH:MM)` (human-readable, e.g., `BlueLake (session 10:30)`)

See [references/preflight.md](references/preflight.md) for protocol details.

### Routing Reference

- [agent-routing.md](references/agent-routing.md) - Routing tables, spawn patterns, file reservations
- [intent-routing.md](references/intent-routing.md) - Intent → agent type mappings
- [summary-protocol.md](references/summary-protocol.md) - Required summary format

## Worker Protocol

Workers follow different protocols based on coordination mode:

### Full Mode (4-Step Protocol)

```
┌─────────────────────────────────────────────────────────────┐
│  STEP 1: INITIALIZE (macro_start_session - FIRST action)   │
│  STEP 2: EXECUTE    (bd update/close - claim and work)     │
│  STEP 3: REPORT     (send_message - MANDATORY summary)     │
│  STEP 4: CLEANUP    (release_file_reservations)            │
└─────────────────────────────────────────────────────────────┘
```

**Key rules:**
- ✅ STEP 1 must be FIRST action (orchestrator pre-registered you)
- ✅ STEP 3 must happen BEFORE returning (non-negotiable)
- ✅ Workers CAN use `bd update` and `bd close` directly
- ⏭️ Heartbeats only for tasks >10 minutes

### Light Mode (3-Step Protocol)

```
┌─────────────────────────────────────────────────────────────┐
│  STEP 1: EXECUTE  (bd update/close - claim and work)       │
│  STEP 2: RETURN   (structured result via Task() return)    │
│  STEP 3: (none)   (no Agent Mail, no reservations)         │
└─────────────────────────────────────────────────────────────┘
```

**Light mode rules:**
- ❌ No Agent Mail registration or messaging
- ❌ No file reservations (rely on file scope isolation)
- ❌ No heartbeats
- ✅ Return structured summary via Task() return value

### Task Return Format (Light Mode Fallback)

When Agent Mail unavailable, workers return structured results:

```python
return {
    "status": "SUCCEEDED",  # or "PARTIAL" or "FAILED"
    "files_changed": [
        {"path": "path/to/file.ts", "action": "added"},
        {"path": "path/to/other.ts", "action": "modified"}
    ],
    "key_decisions": [
        {"decision": "Used X pattern", "rationale": "because Y"}
    ],
    "issues": [],  # Empty if none
    "beads_closed": ["bd-101", "bd-102"]
}
```

Orchestrator collects these returns and aggregates into final summary.

See [references/worker-prompt.md](references/worker-prompt.md) for complete protocol details.

## Agent Mail Protocol

### Orchestrator Registration (Phase 2)

On spawn, the orchestrator MUST:
1. Register itself
2. **Pre-register ALL workers** before spawning them

```python
# 1. Ensure project exists
ensure_project(human_key="/path/to/project")

# 2. Register orchestrator identity
register_agent(
    project_key="/path/to/project",
    name="OrchestratorName",  # Auto-generated adjective+noun
    program="amp",
    model="claude-sonnet-4-20250514",
    task_description=f"Orchestrator for epic {epic_id}"
)

# 3. Pre-register ALL workers (CRITICAL - do this BEFORE spawning)
for track in TRACKS:
    register_agent(
        project_key="/path/to/project",
        name=track.agent,  # e.g., "BlueStar", "GreenMountain"
        program="amp",
        model="claude-sonnet-4-20250514",
        task_description=f"Worker for Track {track.track}"
    )

# Now send_message to workers will succeed
```

> **Why pre-register?** `send_message` validates recipients exist. Without pre-registration, messaging workers fails with "recipients not registered" error.

### Inbox Fetch Pattern

Check inbox for context from prior sessions and worker updates:

```python
# On session start - load prior context
messages = fetch_inbox(
    project_key="/path/to/project",
    agent_name="OrchestratorName",
    include_bodies=True,
    limit=20
)

# Process prior context
for msg in messages:
    if "[TRACK COMPLETE]" in msg.subject:
        mark_track_complete(msg)
    elif "[BLOCKER]" in msg.subject:
        handle_blocker(msg)
    elif "[HEARTBEAT]" in msg.subject:
        update_worker_status(msg)
```

### Mandatory Summary Protocol

All workers (including orchestrator) MUST send a summary before returning:

```python
send_message(
    project_key="/path/to/project",
    sender_name="OrchestratorName",
    to=all_workers,
    thread_id=epic_id,
    subject="EPIC COMPLETE: {title}",
    body_md="""
## Status
SUCCEEDED

## Files Changed
- path/to/file.ts (added)

## Key Decisions
- Decision: rationale

## Issues (if any)
None
"""
)
```

See [references/summary-protocol.md](references/summary-protocol.md) for complete format.

## plan.md Extended Format

```markdown
## Orchestration Config

epic_id: bd-xxx
max_workers: 3
mode: autonomous

## Track Assignments

| Track | Agent | Beads | File Scope | Depends On |
|-------|-------|-------|------------|------------|
| 1 | BlueLake | bd-101, bd-102 | src/api/** | - |
| 2 | GreenCastle | bd-201, bd-202 | src/web/** | bd-102 |
| 3 | RedStone | bd-301 | docs/** | bd-202 |

### Cross-Track Dependencies
- Track 2 waits for bd-102 (from Track 1)
- Track 3 waits for bd-202 (from Track 2)
```

## Fallback Behavior

If Agent Mail unavailable:

```text
⚠️ Agent coordination unavailable - falling back to sequential execution
```

Routes to standard `/conductor-implement` instead.

## Configuration

| Setting | Default | Description |
|---------|---------|-------------|
| max_workers | 3 | Maximum parallel workers |
| heartbeat_interval | 5 min | Worker heartbeat frequency |
| stale_threshold | 10 min | When to consider worker stale |
| cross_dep_timeout | 30 min | Max wait for cross-track dependency |

## Quick Reference

| Action | Tool |
|--------|------|
| Parse plan.md | Read("conductor/tracks/<id>/plan.md") |
| Initialize | ensure_project, register_agent |
| Spawn workers | Task() for each track |
| Monitor | fetch_inbox, search_messages |
| Resolve blockers | reply_message |
| Complete | Verify via bv, send summary, bd close epic |

## References

- [workflow.md](references/workflow.md) - 8-phase protocol
- [worker-prompt.md](references/worker-prompt.md) - Worker template
- [preparation.md](references/preparation.md) - bv --robot-triage preparation
- [monitoring.md](references/monitoring.md) - Agent Mail monitoring
- [patterns/](references/patterns/) - Coordination patterns

## Directory Structure

```
skills/orchestrator/
├── SKILL.md           # This file
├── agents/            # Agent profiles by category
│   ├── research/      # Locator, Analyzer, Pattern, Web, GitHub
│   ├── review/        # CodeReview, SecurityAudit, PerformanceReview
│   ├── planning/      # Architect, Planner
│   ├── execution/     # Implementer, Modifier, Fixer, Refactorer
│   └── debug/         # Debugger, Tracer
├── references/        # Workflow documentation
│   ├── workflow.md    # 8-phase protocol
│   ├── preflight.md   # Session Brain preflight
│   ├── worker-prompt.md
│   └── patterns/
└── scripts/           # Session brain utilities
    └── preflight.py   # Preflight protocol implementation
```
