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
- TIER 1 + TIER 2 scoring passes

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
| Coordination | N/A | Agent Mail MCP |
| File locking | N/A | file_reservation_paths |
| Context | In-memory | Track threads (persistent) |

## 6-Phase Workflow

See [references/workflow.md](references/workflow.md) for full protocol:

1. **Read Plan** - Parse Track Assignments from plan.md
2. **Initialize** - ensure_project, register_agent
3. **Spawn Workers** - Task() for each track (parallel, with routing)
4. **Monitor** - fetch_inbox, search_messages
5. **Resolve** - reply_message for blockers
6. **Complete** - Verify, send summary, close epic

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

### Routing Reference

- [agent-routing.md](references/agent-routing.md) - Routing tables, spawn patterns, file reservations
- [intent-routing.md](references/intent-routing.md) - Intent → agent type mappings
- [summary-protocol.md](references/summary-protocol.md) - Required summary format

## Worker Protocol

Workers are **autonomous** - they have full control:

- ✅ `register_agent()` - Identify themselves
- ✅ `bd update/close` - Claim and close beads
- ✅ `file_reservation_paths()` - Reserve files before edit
- ✅ `send_message()` - Report progress, save context (MANDATORY before return)

See [references/worker-prompt.md](references/worker-prompt.md) for complete worker template.

## Agent Mail Protocol

### Orchestrator Self-Registration

On spawn, the orchestrator MUST register itself with Agent Mail:

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
```

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

- [workflow.md](references/workflow.md) - 6-phase protocol
- [worker-prompt.md](references/worker-prompt.md) - Worker template
- [preparation.md](references/preparation.md) - bv --robot-triage preparation
- [monitoring.md](references/monitoring.md) - Agent Mail monitoring
- [patterns/](references/patterns/) - Coordination patterns
