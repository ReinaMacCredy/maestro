# Learnings: Orchestrator Skill Improvements

Extracted from threads:
- T-019b7983-7c29-707c-a976-807c3bcf8af2 (design/planning)
- T-019b798e-9d2a-70da-8ffe-b26a9a6f9ef0 (parallel execution)

## Commands

- `bd dep add <child> <parent>` - Wire dependencies after bead creation (not auto-mapped from plan)
- `bv --robot-triage --graph-root <epic-id> --json | jq '.quick_ref'` - Quick status extraction for monitoring
- `bd list --parent=<epic-id> --status=open --json | jq 'length'` - Check for lingering beads before epic close
- `summarize_thread(thread_id=TRACK_THREAD)` - Read track context before each bead
- `send_message(to=[self], thread_id=TRACK_THREAD)` - Self-message learnings for next bead

## Gotchas

- Auto-detect routing requires `metadata.json.beads.planTasks` - verify fb saves this structure
- Lingering beads can remain after epic work complete - add verification before closing epic
- Track threads are ephemeral (scoped to single epic) - don't expect cross-epic persistence
- Worker autonomy: orchestrator workers CAN claim/close beads directly (differs from standard subagent rules)
- Wave execution requires monitoring cross-track dependencies - use Agent Mail for dependency notifications

## Patterns

- **Track Thread Pattern**: Workers use `track:{AGENT_NAME}:{EPIC_ID}` for bead-to-bead context passing
- **Per-Bead Loop Protocol**: START (register, read thread, reserve, claim) → WORK → COMPLETE (close, save context, release) → NEXT
- **Auto-Detect Parallel Routing**: Check planTasks independence → if ≥2 independent beads → route to orchestrator
- **Wave Execution**: Spawn Wave 1 (independent) → monitor completion → spawn Wave N (newly-unblocked) → repeat
- **3-Wave Pattern**: This track used Wave 1 (parallel: Track 1+2), Wave 2 (Track 3 after deps), Wave 3 (Track 4 after all)
