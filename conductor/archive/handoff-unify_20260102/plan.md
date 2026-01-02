# Implementation Plan: Unified Handoff System

## Phase 1: Schema Update

### Task 1.1: Add handoff section to metadata.schema.json
- [x] Add `handoff` object to properties
- [x] Define schema: status, section_count, progress_percent, last_handoff, history[]
- [x] Add defaults
- [x] Update examples

**File:** `.claude/skills/conductor/references/schemas/metadata.schema.json`
**Lines:** +50

## Phase 2: Command Definition

### Task 2.1: Create handoff.toml command file
- [x] Define command name and aliases
- [x] Define subcommands: create, resume
- [x] Define arguments and flags
- [x] Add short description
- [x] Reference workflow file

**File:** `.claude/skills/conductor/references/commands/handoff.toml` (NEW)
**Lines:** ~30

## Phase 3: Unified Workflow

### Task 3.1: Create handoff.md workflow file
- [x] Add header with purpose and prerequisites
- [x] Document auto-detect logic
- [x] Document CREATE mode (9 steps)
  - Step 1: Detect context
  - Step 1a: Parallel worker check (NEW)
  - Step 2: Gather metadata
  - Step 3: Scan for secrets
  - Step 4: Send to Agent Mail
  - Step 5: Beads sync (NEW)
  - Step 6: Write markdown file
  - Step 7: Update metadata.json.handoff (NEW)
  - Step 8: Update index.md
  - Step 9: Touch .last_activity
- [x] Document RESUME mode (9 steps)
  - Step 1: Parse input
  - Step 2: Agent Mail lookup
  - Step 3: File discovery (fallback)
  - Step 4: Load content
  - Step 5: Beads context (NEW)
  - Step 6: Validate state
  - Step 7: Present analysis
  - Step 8: Create todos
  - Step 9: Update metadata.json.handoff.status (NEW)
- [x] Document 6 triggers
- [x] Document idle detection
- [x] Document error handling
- [x] Add examples

**File:** `.claude/skills/conductor/references/workflows/handoff.md` (NEW)
**Lines:** ~300

## Phase 4: Implement Integration

### Task 4.1: Update implement.md Phase 0.5
- [x] Reference new handoff.md workflow
- [x] Add beads context loading (`bd show`)
- [x] Add progress % calculation and display
- [x] Update example output

**File:** `.claude/skills/conductor/references/workflows/implement.md`
**Lines:** +15

## Phase 5: Cleanup

### Task 5.1: Delete old handoff files
- [x] Delete `references/handoff/create.md`
- [x] Delete `references/handoff/resume.md`
- [x] Delete `references/handoff/triggers.md`
- [x] Delete `references/handoff/idle-detection.md`
- [x] Delete `references/handoff/agent-mail-format.md`
- [x] Keep `references/handoff/template.md`

**Files:** 5 deleted, 1 kept
**Lines:** -1100

### Task 5.2: Update SKILL.md entry points
- [x] Replace `/create_handoff` and `/resume_handoff` with `/conductor-handoff`
- [x] Update handoff reference path

**File:** `.claude/skills/conductor/SKILL.md`
**Lines:** ~5

## Phase 6: Documentation

### Task 6.1: Update AGENTS.md learnings
- [x] Add command: `/conductor-handoff` with subcommands
- [x] Add gotcha: Auto-detect uses 7-day threshold
- [x] Add pattern: Beads sync on handoff

**File:** `conductor/AGENTS.md`
**Lines:** +10

---

## Summary

| Phase | Tasks | Files | Est. Time |
|-------|-------|-------|-----------|
| 1 | 1 | 1 edit | 5 min |
| 2 | 1 | 1 new | 5 min |
| 3 | 1 | 1 new | 20 min |
| 4 | 1 | 1 edit | 5 min |
| 5 | 2 | 6 (5 delete, 1 edit) | 5 min |
| 6 | 1 | 1 edit | 3 min |
| **Total** | **7** | **11** | **~43 min** |

## Dependencies

```
Phase 1 ──► Phase 3 (schema needed for workflow)
Phase 2 ──► Phase 3 (command def referenced by workflow)
Phase 3 ──► Phase 4 (workflow needed for implement reference)
Phase 3 ──► Phase 5 (workflow replaces old files)
Phase 5 ──► Phase 6 (cleanup before docs update)
```
