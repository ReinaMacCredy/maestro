---
name: orchestrator
description: Multi-agent parallel execution with autonomous workers. Use when plan.md has Track Assignments section or user triggers /conductor-orchestrate, "run parallel", "spawn workers".
---

# Orchestrator - Multi-Agent Parallel Execution

> **Spawn autonomous workers to execute tracks in parallel using Agent Mail coordination.**

## Core Principles

- **Pre-register workers** before spawning (Agent Mail validates recipients)
- **Workers own their beads** - can `bd claim/close` directly (unlike sequential mode)
- **File reservations prevent conflicts** - reserve before edit, release on complete
- **Summary before exit** - all workers MUST send completion message
- **TDD by default** - workers follow RED → GREEN → REFACTOR cycle (use `--no-tdd` to disable)

## When to Use

| Trigger | Condition |
|---------|-----------|
| Auto-routed | `/conductor-implement` when plan has Track Assignments |
| Direct | `/conductor-orchestrate` or `co` |
| Phrase | "run parallel", "spawn workers", "dispatch agents" |

## Quick Reference

| Action | Tool |
|--------|------|
| Parse plan.md | `Read("conductor/tracks/<id>/plan.md")` |
| Initialize | `ensure_project`, `register_agent` |
| Spawn workers | `Task()` for each track |
| Monitor | `fetch_inbox`, `search_messages` |
| Resolve blockers | `reply_message` |
| Complete | Verify via `bv`, send summary, `bd close epic` |
| Track threads | `summarize_thread(thread_id=TRACK_THREAD)` |
| Auto-routing | Auto-detect parallel via `metadata.json.beads` |

## 8-Phase Workflow

0. **Preflight** - Session identity, detect active sessions
1. **Read Plan** - Parse Track Assignments from plan.md
2. **Validate** - Health check Agent Mail (HALT if unavailable)
3. **Initialize** - ensure_project, register orchestrator + all workers
4. **Spawn Workers** - Task() for each track (parallel)
5. **Monitor + Verify** - fetch_inbox, verify worker summaries
   - Workers use track threads (`TRACK_THREAD`) for bead-to-bead context
6. **Resolve** - reply_message for blockers
7. **Complete** - Send summary, close epic, `rb` review

See [references/workflow.md](references/workflow.md) for full protocol.

## Agent Routing

| Intent Keywords | Agent Type | File Reservation |
|-----------------|------------|------------------|
| `research`, `find`, `locate` | Research | None (read-only) |
| `review`, `check`, `audit` | Review | None (read-only) |
| `plan`, `design`, `architect` | Planning | `conductor/tracks/**` |
| `implement`, `build`, `create` | Execution | Task-specific scope |
| `fix`, `debug`, `investigate` | Debug | None (read-only) |

See [references/intent-routing.md](references/intent-routing.md) for mappings.

## Anti-Patterns

| ❌ Don't | ✅ Do |
|----------|-------|
| Spawn workers without pre-registration | Register all workers BEFORE spawning |
| Skip completion summary | Always send_message before exit |
| Ignore file reservation conflicts | Wait or resolve before proceeding |
| Use orchestration for simple tasks | Use sequential `/conductor-implement` |

## References

| Topic | File |
|-------|------|
| Full workflow | [workflow.md](references/workflow.md) |
| Architecture | [architecture.md](references/architecture.md) |
| Coordination modes | [coordination-modes.md](references/coordination-modes.md) |
| Agent Mail protocol | [agent-mail.md](references/agent-mail.md) |
| Worker prompt template | [worker-prompt.md](references/worker-prompt.md) |
| Preflight/session brain | [preflight.md](references/preflight.md) |
| Intent routing | [intent-routing.md](references/intent-routing.md) |
| Summary format | [summary-protocol.md](references/summary-protocol.md) |
| Auto-routing | [auto-routing.md](references/auto-routing.md) |

## Related

- [maestro-core](../maestro-core/SKILL.md) - Routing and fallback policies
- [conductor](../conductor/SKILL.md) - Track management, `/conductor-implement`
- [beads](../beads/SKILL.md) - Issue tracking, `bd` commands
