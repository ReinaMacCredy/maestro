# Specification: Auto-Orchestration After Filing Beads

## Overview

Extend the `fb` (file beads) workflow to automatically trigger orchestration after beads are filed, eliminating the need for manual Track Assignments and `/conductor-orchestrate` invocation.

## Functional Requirements

### FR-1: Auto-Orchestration Trigger
- After `fb` completes filing beads (Phase 5), automatically initiate orchestration
- No user prompt required — fully automatic flow
- Idempotent: if `metadata.json.beads.orchestrated = true`, skip orchestration

### FR-2: Dependency Graph Analysis
- Use `bv --robot-triage --graph-root <epic-id> --json` to analyze beads
- Extract `ready` beads (no blockers) and `blocked_by` relationships
- No fallback needed — `bv --robot-triage` always available

### FR-3: Track Assignment Generation
- Group ready beads into parallel tracks
- Beads with blockers assigned to track of their primary blocker
- Respect `max_workers` limit (default: 3)
- Generate Track Assignments table programmatically

### FR-4: Worker Execution
- Spawn workers via `Task()` for each track
- Workers execute: claim → work → close → report
- Real-time progress via Agent Mail

### FR-5: Final Review
- After all workers complete, main agent spawns `rb` sub-agent
- `rb` sub-agent reviews all completed beads
- Report final summary

### FR-6: State Management
- Add `orchestrated: true` flag to `metadata.json.beads`
- Track completion state for idempotency

## Non-Functional Requirements

### NFR-1: Performance
- Graph analysis via `bv --robot-triage` completes in <5 seconds
- Worker spawn happens in parallel (single `Task()` message)

### NFR-2: Reliability
- If Agent Mail unavailable, fallback to sequential `/conductor-implement`
- Workers handle their own errors and report via Agent Mail

### NFR-3: Observability
- Workers send progress messages to epic thread
- Main agent monitors via `fetch_inbox`

## Acceptance Criteria

| # | Criterion | Verification |
|---|-----------|--------------|
| 1 | After `fb` completes, orchestration starts automatically | Run fb, observe workers spawn |
| 2 | Beads with no deps run in parallel | Check Agent Mail for concurrent progress |
| 3 | Beads with deps wait for blockers | Verify sequential execution via timestamps |
| 4 | Idempotent: re-running fb skips if already orchestrated | Check metadata.json.beads.orchestrated |
| 5 | After all workers complete, `rb` runs for final review | Observe rb sub-agent spawn |
| 6 | Fallback: if Agent Mail unavailable, run sequential | Disable MCP, verify fallback message |

## Out of Scope

- Manual `--no-orchestrate` flag
- Changes to beads CLI itself
- New MCP tools
- Custom graph algorithms (use bv --robot-triage output)
