# Spec: State Consolidation + Continuity Integration

## Summary

Consolidate track state files from 3 to 1 per track, eliminate redundant session state file, and integrate continuity with Conductor workflow for automatic session preservation.

## Goals

1. Reduce state file count from 10 types to 7 types
2. Simplify mental model: 1 metadata.json per track instead of 3 files
3. Auto-load session context at `/conductor-implement` start
4. Auto-handoff session state at `/conductor-finish` end
5. Bind session state to current track

## Non-Goals

- Changing beads CLI behavior
- Modifying archived tracks (backward compatible)
- Breaking existing workflows

## Functional Requirements

### FR-1: Metadata Consolidation

#### FR-1.1: Schema Extension
- metadata.json MUST include `generation` section (from .track-progress.json)
- metadata.json MUST include `beads` section (from .fb-progress.json)
- Schema MUST be backward compatible (new fields optional)

#### FR-1.2: Generation Section
```json
"generation": {
  "status": "complete",
  "specCreatedAt": "ISO8601",
  "planCreatedAt": "ISO8601",
  "rbCompletedAt": "ISO8601"
}
```

#### FR-1.3: Beads Section
```json
"beads": {
  "status": "complete",
  "epicId": "string",
  "epics": [...],
  "issues": [...],
  "planTasks": {"task-id": "bead-id"},
  "beadToTask": {"bead-id": "task-id"},
  "crossTrackDeps": [],
  "reviewStatus": "complete",
  "reviewedAt": "ISO8601"
}
```

#### FR-1.4: File Elimination
- .track-progress.json MUST be removed from workflow
- .fb-progress.json MUST be removed from workflow
- Schemas MUST be deleted

### FR-2: Session State Consolidation

#### FR-2.1: LEDGER.md Extension
LEDGER.md frontmatter MUST include:
```yaml
---
bound_track: "track_id or null"
bound_bead: "bead_id or null"
mode: "SA or MA"
tdd_phase: "RED, GREEN, REFACTOR, or null"
heartbeat: "ISO8601"
---
```

#### FR-2.2: Session State Elimination
- session-state_*.json MUST NOT be created
- Preflight MUST read/write LEDGER.md instead
- TDD checkpoints MUST update LEDGER.md

### FR-3: Continuity ↔ Conductor Integration

#### FR-3.1: Implement Phase 0.5
After Beads Preflight:
1. Load LEDGER.md if exists
2. Archive if stale (>24h)
3. Archive if track mismatch (with user message)
4. Update bound_track and bound_bead
5. Display session summary

#### FR-3.2: Finish Phase 6.5
After CODEMAPS:
1. Create handoff with trigger=`track-complete`
2. Delete LEDGER.md
3. Display archive confirmation

#### FR-3.3: Design Session Init
At ds session start:
1. Load LEDGER.md if exists
2. Display prior context (optional)
3. Do NOT bind track

#### FR-3.4: Non-Blocking
All continuity operations MUST:
- Warn on failure, never halt
- Create backup on corruption
- Continue workflow regardless

## Non-Functional Requirements

### NFR-1: Backward Compatibility
- Existing tracks without new fields MUST work
- Missing generation/beads sections treated as empty

### NFR-2: Migration
- Provide migration guide for manual conversion
- Auto-detect old format and suggest migration

### NFR-3: Performance
- No additional file reads beyond current
- Atomic writes with temp files

## Acceptance Criteria

| ID | Criterion | Test |
|----|-----------|------|
| AC-1 | metadata.json contains generation section | Create track, verify field exists |
| AC-2 | metadata.json contains beads section | Run fb, verify planTasks in metadata |
| AC-3 | .track-progress.json not created | Run newtrack, verify file absent |
| AC-4 | .fb-progress.json not created | Run fb, verify file absent |
| AC-5 | session-state_*.json not created | Run implement, verify file absent |
| AC-6 | LEDGER.md has bound_track | Run implement, verify frontmatter |
| AC-7 | Auto-load at implement | Run implement, see context message |
| AC-8 | Auto-handoff at finish | Run finish, see archive message |
| AC-9 | Track switch archives | Switch tracks, see archive message |
| AC-10 | Failure doesn't halt | Corrupt LEDGER, implement continues |

## Dependencies

- None (internal refactor)

## Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Break existing tracks | Low | High | Backward compat + migration guide |
| Schema too large | Medium | Low | TypeScript types, validation |
| Concurrent session conflict | Medium | Medium | Document limitation |
