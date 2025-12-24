# Specification: Conductor Track Validation System

## Overview

Implement a two-layer validation architecture for Conductor tracks:
1. **Prevention:** Create state files early in `/conductor-newtrack` (Phase 1.3)
2. **Recovery:** Centralized validation system for pre-flight checks and auto-repair

This ensures tracks are never left in inconsistent state, regardless of interruptions or agent behavior.

## Functional Requirements

### FR-1: State File Schemas

Create JSON schemas for the two missing state files:
- `workflows/schemas/track_progress.schema.json`
- `workflows/schemas/fb_progress.schema.json`

### FR-2: Validation Reference Files

Create inline-able validation logic in `skills/conductor/references/validation/track/`:
- `README.md` - Quick reference
- `checks.md` - Validation logic (inline-able by commands)
- `snippets.md` - Bash code templates
- `recovery.md` - Troubleshooting guide

### FR-3: Reorganize Existing Validation Files

Move existing quality rubrics to `skills/conductor/references/validation/quality/`:
- Move `README.md`, `judge-prompt.md`, `rubrics.md` from current location
- Update `skills/conductor/references/validation/README.md` as index

### FR-4: track_id Validation

Directory name is source of truth. Auto-fix mismatches in:
- `metadata.json.track_id`
- `.track-progress.json.trackId`
- `.fb-progress.json.trackId`

Warn (don't auto-fix) mismatches in content files:
- `design.md`, `spec.md`, `plan.md` headers

### FR-5: File Existence Matrix

Validate file combinations and take appropriate action:
- Empty directory → SKIP + warn
- design.md only → PASS (valid design-only state)
- spec.md + plan.md, no state files → Auto-create state files
- spec.md XOR plan.md → HALT (invalid state)
- All files present → Validate and pass

### FR-6: Auto-Create State Files

When spec.md + plan.md exist but state files missing:
1. Pre-check: Both files have content (size > 0)
2. Pre-check: Both files < 30 days old
3. Pre-check: No track_id mismatch in headers
4. If all pass: Auto-create with defaults, log to repairs

### FR-7: Staleness Detection

When `.fb-progress.json.status = "in_progress"`:
- Warn user with start time
- Offer options: Resume, Reset, Diagnose
- Never auto-reset (require explicit user action)

### FR-8: Audit Trail

Log repairs to `metadata.json.repairs[]` array:
- Keep last 10 entries
- Include: timestamp, action, field, from/to values, by

### FR-9: Update Commands with Phase 0 Validation

Add Phase 0 validation to:
- `commands/conductor/validate.toml` - Reference checks.md
- `commands/conductor/implement.toml` - Add pre-flight
- `commands/conductor/finish.toml` - Add pre-flight

### FR-10: Update Skills to Reference checks.md

Update validation references in:
- `skills/file-beads/SKILL.md` (already done in previous work)
- `skills/review-beads/SKILL.md`

### FR-11: Update workflows/validate.md

Add sections for:
- track_id validation rules
- State file validation
- Auto-repair actions

## Non-Functional Requirements

### NFR-1: Performance

Validation must complete in < 1 second for single track.

### NFR-2: Atomic Writes

All state file updates must use temp file + rename pattern to prevent corruption.

### NFR-3: Forward Compatibility

Unknown fields in JSON files must be preserved (not stripped).

## Acceptance Criteria

- [ ] `workflows/schemas/track_progress.schema.json` exists and is valid
- [ ] `workflows/schemas/fb_progress.schema.json` exists and is valid
- [ ] `workflows/validate.md` updated with track_id and state file sections
- [ ] `skills/conductor/references/validation/track/` folder created with 4 files
- [ ] `skills/conductor/references/validation/quality/` folder created with moved files
- [ ] `commands/conductor/validate.toml` references checks.md
- [ ] `commands/conductor/implement.toml` has Phase 0 validation
- [ ] `commands/conductor/finish.toml` has Phase 0 validation
- [ ] `skills/review-beads/SKILL.md` references checks.md
- [ ] track_id mismatch is auto-fixed in state files
- [ ] Missing state files auto-created when spec+plan exist
- [ ] Corrupted JSON causes HALT
- [ ] spec.md XOR plan.md causes HALT
- [ ] Repairs logged to `metadata.json.repairs[]`

## Out of Scope

- Beads orphan detection (belongs in `/conductor-status`)
- Cross-track dependency validation (belongs in `/conductor-status --all`)
- Lock files for concurrent access (atomic writes sufficient)
- Disk full / read-only filesystem handling
