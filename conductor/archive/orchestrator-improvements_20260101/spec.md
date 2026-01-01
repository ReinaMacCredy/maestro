# Specification: Orchestrator Skill Improvements

## Overview

Enhance the orchestrator skill with track threads for bead-to-bead context, per-bead execution loop, AGENTS.md tool preferences, auto-detect parallel routing, and improved monitoring/verification.

## Functional Requirements

### FR1: Track Thread Pattern

- **FR1.1**: Workers shall use thread ID format `track:{AGENT_NAME}:{EPIC_ID}` for per-track context
- **FR1.2**: Workers shall read track thread via `summarize_thread()` before starting each bead
- **FR1.3**: Workers shall self-message learnings, gotchas, and next-bead notes after completing each bead
- **FR1.4**: Track threads shall be ephemeral (scoped to single epic, not persistent across epics)

### FR2: Per-Bead Loop Protocol

- **FR2.1**: Workers shall execute a 4-step loop for EACH bead: START → WORK → COMPLETE → NEXT
- **FR2.2**: START step shall: register agent, read track thread, reserve files, claim bead
- **FR2.3**: WORK step shall: implement task, check inbox periodically for blockers
- **FR2.4**: COMPLETE step shall: close bead, report to orchestrator, save context to track thread, release files
- **FR2.5**: NEXT step shall: loop back to START for next bead in track

### FR3: AGENTS.md Tool Preferences

- **FR3.1**: Worker prompt template shall include explicit "Tool Preferences" section
- **FR3.2**: Tool preferences shall be populated from project's AGENTS.md file
- **FR3.3**: Categories: codebase exploration, file editing, web search, UI components

### FR4: Auto-Detect Parallel Routing

- **FR4.1**: `/conductor-implement` shall check for `## Track Assignments` first (existing behavior)
- **FR4.2**: If no Track Assignments, shall read `metadata.json.beads.planTasks` for bead mappings
- **FR4.3**: Shall verify with `bd list --json` at runtime as source of truth
- **FR4.4**: If ≥2 independent beads (no dependencies between them), shall auto-generate Track Assignments
- **FR4.5**: Auto-generated tracks shall route to orchestrator for parallel execution

### FR5: Enhanced Monitoring

- **FR5.1**: Primary monitoring shall use `bv --robot-triage --graph-root <epic-id>`
- **FR5.2**: Quick status shall be extracted via `jq '.quick_ref'`
- **FR5.3**: Secondary monitoring shall remain `fetch_inbox` and `search_messages`

### FR6: Lingering Beads Verification

- **FR6.1**: Before closing epic, shall verify all child beads are closed
- **FR6.2**: Command: `bd list --parent=<epic-id> --status=open --json`
- **FR6.3**: If open beads remain, shall prompt user with options: close all, skip, or abort

### FR7: Fix planTasks Population

- **FR7.1**: `fb` command shall save `planTasks` mapping to `metadata.json.beads`
- **FR7.2**: `fb` command shall save `beadToTask` reverse mapping
- **FR7.3**: `fb` command shall save `crossTrackDeps` array for cross-track dependencies

## Non-Functional Requirements

- **NFR1**: All changes shall be backward compatible with existing orchestrator usage
- **NFR2**: Worker prompt changes shall fit within token limits
- **NFR3**: Auto-detect routing shall add <2 seconds latency to `/conductor-implement`

## Acceptance Criteria

- [ ] Worker prompt includes track thread read/write operations
- [ ] Worker prompt has explicit per-bead loop (START/WORK/COMPLETE/NEXT)
- [ ] Worker prompt has Tool Preferences section placeholder
- [ ] monitoring.md prioritizes `bv --robot-triage`
- [ ] workflow.md includes lingering beads check before epic close
- [ ] implement.md includes auto-detect parallel logic
- [ ] fb command populates planTasks in metadata.json (verify with existing tracks)

## Out of Scope

- Cross-epic persistent learnings
- Worker retry/respawn on failure
- UI progress display enhancements
