# Design: Auto-Orchestration After Filing Beads

## Problem Statement

After filing beads (`fb`), users must manually run `/conductor-orchestrate` with manually-defined Track Assignments. We need an **automatic flow** that analyzes the beads dependency graph and spawns sub-agents immediately.

## Solution

Extend `fb` to automatically trigger orchestration after beads are filed:

```
fb → auto-analyze graph → generate Track Assignments → spawn workers → rb final review
```

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         fb (enhanced)                           │
├─────────────────────────────────────────────────────────────────┤
│  Phase 1-5: File beads from plan (existing)                     │
│  Phase 6: Auto-orchestration (NEW)                              │
│           ├─ 6.1 Query graph: bv --robot-triage --graph-root    │
│           ├─ 6.2 Generate Track Assignments from ready/blocked  │
│           ├─ 6.3 Mark metadata.json.beads.orchestrated = true   │
│           └─ 6.4 Call orchestrator                              │
│  Phase 7: Workers execute (parallel)                            │
│           └─ Each worker: claim → work → close → report         │
│  Phase 8: Main waits for all workers complete                   │
│  Phase 9: Main spawns rb sub-agent for final review             │
└─────────────────────────────────────────────────────────────────┘
```

## Track Assignment Generation

Uses `bv --robot-triage --graph-root <epic-id> --json`:

```json
{
  "quick_ref": { "open_count": 5, "blocked_count": 2, "ready_count": 3 },
  "beads": [
    { "id": "bd-1", "ready": true, "blocked_by": [] },
    { "id": "bd-2", "ready": false, "blocked_by": ["bd-1"] },
    { "id": "bd-3", "ready": true, "blocked_by": [] },
    { "id": "bd-4", "ready": false, "blocked_by": ["bd-2", "bd-3"] },
    { "id": "bd-5", "ready": false, "blocked_by": ["bd-4"] }
  ]
}
```

**Algorithm:**
1. Group ready beads (no blockers) into parallel tracks
2. Beads with blockers assigned to track of their primary blocker
3. Respect `max_workers` limit (merge smallest tracks if exceeded)

**Output:**
```markdown
## Track Assignments

| Track | Beads | Depends On |
|-------|-------|------------|
| 1 | bd-1, bd-2 | - |
| 2 | bd-3 | - |
| 3 | bd-4, bd-5 | bd-2, bd-3 |
```

## State Management

```json
// metadata.json
{
  "beads": {
    "epicId": "bd-100",
    "planTasks": { "1.1": "bd-1", "1.2": "bd-2" },
    "orchestrated": true  // NEW - idempotency flag
  }
}
```

## Idempotency

```python
if metadata.beads.orchestrated:
    # Already orchestrated - skip
    return
```

## Reporting Flow

1. Workers report progress via Agent Mail (real-time)
2. Main agent monitors via `fetch_inbox`
3. On all workers complete → main spawns `rb` sub-agent
4. `rb` sub-agent reviews all completed beads
5. Final summary reported

## Files to Modify

| File | Action |
|------|--------|
| `skills/beads/references/FILE_BEADS.md` | Add Phase 6: Auto-Orchestrate |
| `skills/beads/references/auto-orchestrate.md` | NEW: Algorithm + integration |
| `skills/orchestrator/SKILL.md` | Accept auto-generated Track Assignments |
| `conductor/AGENTS.md` | Add learnings |

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| 1 | After `fb` completes, orchestration starts automatically |
| 2 | Beads with no deps run in parallel |
| 3 | Beads with deps wait for blockers |
| 4 | Idempotent: re-running fb skips if already orchestrated |
| 5 | After all workers complete, `rb` runs for final review |
| 6 | Fallback: if Agent Mail unavailable, run sequential |

## Assumptions

- `bv --robot-triage` always available
- Agent Mail MCP available (fallback to sequential if not)
- `max_workers` default: 3

## Out of Scope

- Manual `--no-orchestrate` flag (not needed)
- Changes to beads CLI itself
- New MCP tools
