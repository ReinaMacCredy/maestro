# Beads Workflow Integration

This project uses [beads_viewer](https://github.com/Dicklesworthstone/beads_viewer) for issue tracking. Issues are stored in `.beads/` and tracked in git.

## Beads-Conductor Integration

Conductor commands automatically manage beads lifecycle via a **facade pattern**:

| Integration Point | Conductor Command | Beads Action |
|-------------------|-------------------|--------------|
| Preflight | All commands | Validate bd availability |
| Track Init | `/conductor-newtrack` | Create epic + issues from plan.md |
| Claim | `/conductor-implement` | `bd update --status in_progress` |
| Close | `/conductor-implement` | `bd close --reason completed` |
| Sync | All (session end) | `bd sync` with retry |
| Compact | `/conductor-finish` | AI summaries for closed issues |
| Cleanup | `/conductor-finish` | Remove oldest when >150 closed |

**Zero manual bd commands** in the happy path - Conductor handles everything.

## Essential Commands

```bash
# View issues (launches TUI - avoid in automated sessions)
bv

# CLI commands for agents (use these instead)
bd ready              # Show issues ready to work (no blockers)
bd list --status=open # All open issues
bd show <id>          # Full issue details with dependencies
bd create --title="..." --type=task --priority=2
bd update <id> --status=in_progress
bd close <id> --reason="Completed"
bd close <id1> <id2>  # Close multiple issues at once
bd sync               # Commit and push changes

# Cleanup commands (used by /conductor-finish Phase 2)
bd compact --analyze --json      # Find issues needing summary
bd compact --apply --id <id> --summary "text"  # Add AI summary
bd count --status closed --json  # Count closed issues
bd cleanup --older-than 0 --limit <n> --force  # Remove oldest closed
```

## Session Protocol

**Session Start (automatic handoff load):**
On first message of any session, before processing request:
1. Check `conductor/handoffs/` for recent handoffs (< 7 days)
2. If found, auto-load and display: `📋 Prior session context: [track] (Xh ago)`
3. Skip if user says "fresh start" or no conductor/ exists

```bash
# Preflight runs automatically via Conductor commands
# If manual, check bd availability first:
bd version            # Verify bd is available

# Find and claim work:
bd ready --json       # Get available tasks
bd update <id> --status in_progress
```

**During Session:**
- Update heartbeat every 5 minutes (automatic)
- TDD checkpoints enabled by default (use `--no-tdd` to disable)
- Close tasks with reason: `completed`, `skipped`, or `blocked`

**Session End:**
```bash
bd close <id> --reason completed  # Close current task
bd sync                           # Sync beads to git
git add <files> && git commit -m "..."  # Commit code changes
git push                          # Push to remote
```

## State Files

| File | Location | Purpose |
|------|----------|---------|
| `metadata.json` | `conductor/tracks/<id>/` | Track info, validation state, beads mapping |
| `*.md handoffs` | `conductor/handoffs/<track>/` | Session handoffs (git-committed, shareable) |
| `index.md` | `conductor/handoffs/<track>/` | Handoff log per track |
| `session-lock_<track>.json` | `.conductor/` | Concurrent session prevention |
| `pending_*.jsonl` | `.conductor/` | Failed operations for replay |
| `metrics.jsonl` | `.conductor/` | Usage metrics (append-only) |

## Metrics Logging

Usage events are logged to `.conductor/metrics.jsonl`:

```jsonl
{"event": "tdd_cycle", "taskId": "bd-42", "phase": "GREEN", "duration": 180, "timestamp": "..."}
{"event": "manual_bd", "command": "bd show", "timestamp": "..."}
```

Run `scripts/beads-metrics-summary.sh` for weekly summary.

## Key Concepts

- **Dependencies**: Issues can block other issues. `bd ready` shows only unblocked work.
- **Priority**: P0=critical, P1=high, P2=medium, P3=low, P4=backlog (use numbers, not words)
- **Types**: task, bug, feature, epic, question, docs
- **Blocking**: `bd dep add <issue> <depends-on>` to add dependencies
- **planTasks Mapping**: Bidirectional mapping between plan task IDs and bead IDs in `metadata.json.beads`

## Ralph Integration (Autonomous Mode)

When `fb` (file beads) creates issues from plan.md, it also populates `metadata.json.ralph` for autonomous execution via `ca`:

```json
{
  "ralph": {
    "enabled": true,
    "active": false,
    "stories": {
      "task-1": { "id": "task-1", "title": "Add auth endpoint", "priority": 1, "passes": false, "beadId": "my-project-abc1" },
      "task-2": { "id": "task-2", "title": "Add login UI", "priority": 2, "passes": false, "beadId": "my-project-def2" }
    }
  }
}
```

**Population rules:**
- Stories keyed by `.planTasks` task ID (not bead ID)
- `beadId` references the mapped bead for commit traceability
- `passes: false` initially; set to `true` when story completes
- `priority` derived from task order in plan.md

**Idempotency:** Re-running `fb` preserves existing `passes` values - only new tasks are added with `passes: false`.

**Usage:** Run `ca` to start Ralph autonomous loop after `fb` completes.

## References

> **Cross-skill reference:** Load the [conductor](../../conductor/SKILL.md) skill for:
> - Beads facade API contract
> - All 13 beads integration points
> - Preflight and session workflows
