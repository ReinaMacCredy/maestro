# Specification: Unified Handoff System

## Overview

Consolidate the scattered handoff system (6 files, ~1100 lines) into a unified command structure (2 files, ~330 lines) while adding missing features: parallel execution check, Beads sync, and progress tracking in metadata.json.

## Functional Requirements

### FR1: Unified Command
- Single `/conductor-handoff` command replaces `/create_handoff` and `/resume_handoff`
- Subcommands: `create`, `resume`, or auto-detect (default)
- Backward-compatible aliases maintained

### FR2: Auto-Detect Mode
- If first message of session AND recent handoff exists (<7 days): RESUME mode
- Otherwise: CREATE mode
- User can override with explicit subcommand

### FR3: Parallel Execution Check (CREATE)
- Before creating handoff, check for `parallel_state.json`
- If found, prompt: [A] Wait | [B] Proceed (include state) | [C] Cancel
- Include parallel worker state in handoff if proceeding

### FR4: Beads Sync (CREATE)
- After Agent Mail send, run `bd update <epic> --notes "..."`
- Structured note format: COMPLETED, KEY DECISIONS, IN PROGRESS, NEXT, BLOCKER, HANDOFF
- Run `bd sync` to persist immediately
- Graceful fallback if `bd` unavailable

### FR5: Progress Tracking in metadata.json
- New `handoff` section in metadata.json
- Track: status, section_count, progress_percent, last_handoff, history[]
- Update on both create and resume operations

### FR6: Beads Context on Resume
- Run `bd show <epic>` to load current beads state
- Display progress % and ready tasks
- Graceful fallback if `bd` unavailable

### FR7: Enhanced Phase 0.5 in /conductor-implement
- Run `/conductor-handoff resume` internally
- Load beads context
- Display progress percentage
- Create epic-start handoff

## Non-Functional Requirements

### NFR1: File Consolidation
- Delete 5 scattered files (~1100 lines removed)
- Create 2 unified files (~330 lines added)
- Net reduction: ~770 lines

### NFR2: Backward Compatibility
- `/create_handoff` alias → `/conductor-handoff create`
- `/resume_handoff` alias → `/conductor-handoff resume`
- Existing handoff files remain readable

### NFR3: Graceful Degradation
- Agent Mail unavailable: markdown-only fallback
- Beads CLI unavailable: skip sync, log warning
- Parallel state missing: proceed normally

## Acceptance Criteria

- [ ] `/conductor-handoff` auto-detects mode correctly
- [ ] `/conductor-handoff create` runs all 9 steps including parallel check and beads sync
- [ ] `/conductor-handoff resume` runs all 9 steps including beads context
- [ ] metadata.json.handoff section updates on create and resume
- [ ] `/conductor-implement` Phase 0.5 displays progress %
- [ ] All 5 old handoff files deleted
- [ ] template.md preserved unchanged
- [ ] Aliases work: `/create_handoff`, `/resume_handoff`

## Out of Scope

- Changing handoff file format (template.md unchanged)
- Changing Agent Mail integration
- Adding new triggers beyond existing 6
- Modifying other metadata.json sections
