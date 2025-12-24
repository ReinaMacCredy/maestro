# Implementation Plan: Conductor Track Validation System

## Phase 1: Create JSON Schemas

- [ ] Task: Create track_progress.schema.json
  - [ ] Define schema for .track-progress.json
  - [ ] Include: trackId, status, specCreatedAt, planCreatedAt, threadId
  - [ ] Add validation for status enum values
  - [ ] Write to `workflows/schemas/track_progress.schema.json`

- [ ] Task: Create fb_progress.schema.json
  - [ ] Define schema for .fb-progress.json
  - [ ] Include: trackId, status, startedAt, threadId, resumeFrom, epics, issues, crossTrackDeps, lastError
  - [ ] Add validation for status enum and array types
  - [ ] Write to `workflows/schemas/fb_progress.schema.json`

- [ ] Task: Conductor - User Manual Verification 'Phase 1' (Protocol in workflow.md)

## Phase 2: Reorganize Validation Files

- [ ] Task: Create quality/ subfolder
  - [ ] Create `skills/conductor/references/validation/quality/` directory
  - [ ] Move existing `README.md` to `quality/README.md`
  - [ ] Move existing `judge-prompt.md` to `quality/judge-prompt.md`
  - [ ] Move existing `rubrics.md` to `quality/rubrics.md`

- [ ] Task: Update validation README
  - [ ] Update `skills/conductor/references/validation/README.md`
  - [ ] Add index for quality/ and track/ subsystems

- [ ] Task: Conductor - User Manual Verification 'Phase 2' (Protocol in workflow.md)

## Phase 3: Create Track Validation Files

- [ ] Task: Create track/README.md
  - [ ] Write quick reference with validation modes
  - [ ] Include file list and purpose

- [ ] Task: Create track/checks.md
  - [ ] Write Phase 0 validation logic
  - [ ] Include: resolve track path, check directory, file existence matrix
  - [ ] Include: validate JSON, auto-create state files, auto-fix track_id
  - [ ] Include: staleness detection, diagnose mode

- [ ] Task: Create track/snippets.md
  - [ ] Add state file templates (metadata.json, .track-progress.json, .fb-progress.json)
  - [ ] Add repair log entry template
  - [ ] Add atomic write pattern

- [ ] Task: Create track/recovery.md
  - [ ] Add quick fixes table
  - [ ] Add recovery scenarios (copied track, interrupted workflow, manual creation)
  - [ ] Add diagnose mode documentation

- [ ] Task: Conductor - User Manual Verification 'Phase 3' (Protocol in workflow.md)

## Phase 4: Update workflows/validate.md

- [ ] Task: Add track_id validation section
  - [ ] Document source of truth (directory name)
  - [ ] List auto-fix vs warn behaviors

- [ ] Task: Add state file validation section
  - [ ] Document file existence matrix
  - [ ] Document auto-create logic and pre-checks
  - [ ] Document HALT conditions

- [ ] Task: Add auto-repair section
  - [ ] List repairable vs non-repairable issues
  - [ ] Document audit trail format

- [ ] Task: Conductor - User Manual Verification 'Phase 4' (Protocol in workflow.md)

## Phase 5: Update Commands

- [ ] Task: Update validate.toml
  - [ ] Add reference to `skills/conductor/references/validation/track/checks.md`
  - [ ] Add per-track validation call in Section 2.2

- [ ] Task: Update implement.toml
  - [ ] Add Phase 0: Track Validation before task execution
  - [ ] Reference checks.md for validation logic

- [ ] Task: Update finish.toml
  - [ ] Add Phase 0: Track Validation before archiving
  - [ ] Reference checks.md for validation logic

- [ ] Task: Conductor - User Manual Verification 'Phase 5' (Protocol in workflow.md)

## Phase 6: Update Skills

- [ ] Task: Update review-beads SKILL.md
  - [ ] Add Phase 0.3 validation (similar to file-beads)
  - [ ] Reference checks.md for validation logic

- [ ] Task: Update conductor SKILL.md
  - [ ] Update Track Integrity Validation section
  - [ ] Add reference to new validation files

- [ ] Task: Conductor - User Manual Verification 'Phase 6' (Protocol in workflow.md)

## Phase 7: Final Verification

- [ ] Task: Test validation on existing tracks
  - [ ] Run validation on clean track
  - [ ] Run validation on track with missing state files
  - [ ] Run validation on track with track_id mismatch
  - [ ] Verify auto-repair and audit trail

- [ ] Task: Update acceptance criteria in design.md
  - [ ] Mark completed criteria
  - [ ] Document any deviations

- [ ] Task: Conductor - User Manual Verification 'Phase 7' (Protocol in workflow.md)
