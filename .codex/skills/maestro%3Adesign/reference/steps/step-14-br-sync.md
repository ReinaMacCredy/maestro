# Step 14: Plan-to-BR Sync

**Progress: Step 14 of 16** -- Next: Readiness Gate

## Goal
If Beads (BR) is available and initialized, sync the approved plan into BR epics and issues for state tracking. Otherwise skip entirely.

## Execution Rules
- This step is CONDITIONAL -- skip if `.beads/` does not exist or `br` is not installed
- If skipping, announce: "BR sync skipped (no .beads/ directory or br not installed)." and proceed
- If running, follow the protocol in `maestro:implement/reference/plan-to-br-sync.md`
- Do NOT initialize BR here -- that is handled by `/maestro:setup` or AGENTS.md detection

## Context Boundaries
- Approved plan at `.maestro/tracks/{track_id}/plan.md`
- metadata.json exists (will be updated with `beads_epic_id` and `beads_issue_map`)
- BR sync protocol at the `maestro:implement` skill's `reference/plan-to-br-sync.md`

## Sync Sequence

1. **Check Prerequisites**
   ```bash
   # Both conditions must be true to proceed
   [ -d ".beads" ] && command -v br > /dev/null 2>&1
   ```
   If either fails: announce skip, proceed to next step.

2. **Read Sync Protocol**
   Read `maestro:implement/reference/plan-to-br-sync.md` (in the implement skill, not locally).

3. **Execute Sync**
   Follow the protocol:
   a. Parse plan.md for phases and tasks
   b. Create BR epic for the track
   c. Create BR issues per task
   d. Set dependencies (sequential within phase, cross-phase)
   e. Validate no circular dependencies
   f. Store `beads_epic_id` and `beads_issue_map` in metadata.json

4. **Stage Beads State**
   ```bash
   br sync --flush-only
   ```
   The `.beads/` directory will be committed in step 16.

5. **Report**
   - "BR sync complete: epic {epic_id}, {N} issues created."
   - Or if sync failed: "BR sync failed: {error}. Falling back to plan.md-only mode."

## Error Handling
- If `br create` fails: report error, do NOT set `beads_epic_id`, continue without BR
- If cycle detection finds issues: warn user, attempt automatic resolution, re-validate
- If any step fails after epic creation: clean up incomplete epic, remove `beads_epic_id`

## Quality Checks
- [ok] Prerequisites checked before attempting sync
- [ok] Graceful skip when BR not available
- [ok] Epic and issues created per protocol
- [ok] metadata.json updated with BR fields (or fields omitted on skip/failure)
- [ok] `.beads/` state flushed for commit

## Anti-patterns
- [x] Running `br init` here -- BR bootstrap is not this step's job
- [x] Failing hard when BR is not installed -- must skip gracefully
- [x] Leaving orphaned epic on sync failure -- clean up on error
- [x] Setting `beads_epic_id` when sync actually failed

## Next Step
Read and follow `reference/steps/step-15-readiness.md`.
