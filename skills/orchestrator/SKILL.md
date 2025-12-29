---
name: orchestrator
description: Multi-agent parallel execution with autonomous workers. Use when plan.md has Track Assignments section or user triggers /conductor-orchestrate, "run parallel", "spawn workers".
metadata:
  version: "1.0.0"
---

# Orchestrator - Multi-Agent Parallel Execution

> **Spawn autonomous workers to execute tracks in parallel using Agent Mail coordination.**

## Prerequisites

**REQUIRED SUB-SKILL:** [maestro-core](../maestro-core/SKILL.md)

Load maestro-core first for orchestration context.

## When to Use

**Primary:** `/conductor-implement` auto-routes here when:
- Plan.md has **Track Assignments** section
- TIER 1 + TIER 2 scoring passes

**Direct:** Also available via:
- User runs `/conductor-orchestrate` or `co`
- User says "run parallel", "spawn workers", "dispatch agents"

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
3. **Spawn Workers** - Task() for each track (parallel)
4. **Monitor** - fetch_inbox, search_messages
5. **Resolve** - reply_message for blockers
6. **Complete** - Verify, send summary, close epic

## Worker Protocol

Workers are **autonomous** - they have full control:

- ✅ `register_agent()` - Identify themselves
- ✅ `bd update/close` - Claim and close beads
- ✅ `file_reservation_paths()` - Reserve files before edit
- ✅ `send_message()` - Report progress, save context

See [references/worker-prompt.md](references/worker-prompt.md) for complete worker template.

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
