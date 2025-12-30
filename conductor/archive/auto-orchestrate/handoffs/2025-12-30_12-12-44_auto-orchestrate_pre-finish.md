# Handoff: auto-orchestrate (pre-finish)

**Track:** auto-orchestrate
**Trigger:** pre-finish
**Created:** 2025-12-30T12:12:44+07:00
**Thread:** T-019b6daa-caa3-77aa-9972-abddfcedabc5

## Summary

Implemented auto-orchestration feature that automatically triggers parallel worker dispatch after `fb` (file beads) completes. This eliminates the need for manual Track Assignments and `/conductor-orchestrate` invocation.

## Changes Made

### Core Files Created/Modified

1. **skills/beads/references/auto-orchestrate.md** (NEW)
   - Graph analysis algorithm using `bv --robot-triage --graph-root <epic-id> --json`
   - Track Assignment generation logic
   - Wave execution with re-dispatch loop
   - Worker dispatch protocol
   - Sequential fallback when Agent Mail unavailable

2. **skills/beads/references/FILE_BEADS.md**
   - Added Phase 6: Auto-Orchestration
   - Added Phase 7: Final Review (rb sub-agent)
   - Idempotency check via `metadata.json.beads.orchestrated`

3. **skills/conductor/references/schemas/metadata.schema.json**
   - Added `orchestrated` boolean field to beads section

4. **skills/orchestrator/SKILL.md**
   - Added Auto-Orchestration Integration section
   - Documented auto-generated vs manual Track Assignments

5. **skills/orchestrator/references/workflow.md**
   - Updated Phase 1 to accept auto-generated assignments
   - Added Phase 4 Wave Re-dispatch loop
   - Added Phase 7: Final Review (rb sub-agent)

6. **conductor/AGENTS.md**
   - Added commands: `bv --robot-triage --graph-root <epic-id> --json`
   - Added gotcha: `metadata.json.beads.orchestrated` for idempotency
   - Added pattern: Wave Execution, Auto-orchestration after fb

## Key Decisions

1. **Wave Execution**: Instead of single dispatch, use re-dispatch loop - after Wave N completes, query `bd ready --json` and spawn Wave N+1 for newly-unblocked beads
2. **Idempotency**: `metadata.json.beads.orchestrated` flag prevents re-running if already orchestrated
3. **Fallback**: If Agent Mail unavailable, graceful fallback to sequential `/conductor-implement`
4. **Final Review**: After all waves complete, spawn `rb` sub-agent for quality verification

## Beads Status

- Root Epic: my-workflow:3-0p92 (CLOSED)
- Child Epics: 4 (all CLOSED)
- Child Tasks: 8 (all CLOSED)

## Verification

All changes align with spec.md acceptance criteria:
- ✅ After fb completes, orchestration starts automatically
- ✅ Beads with no deps run in parallel
- ✅ Beads with deps wait for blockers
- ✅ Idempotent via orchestrated flag
- ✅ rb runs for final review
- ✅ Sequential fallback if Agent Mail unavailable

## Follow-up Work

None identified. Track is ready for completion.
