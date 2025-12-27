# State Consolidation + Continuity Integration Design

## Overview

Consolidate state files and integrate continuity with Conductor workflow for seamless session state preservation.

## Problem Statement

Current workflow has:
1. **10 state file types** - Too many, confusing mental model
2. **No continuity chain** - `/conductor-implement` and `/conductor-finish` don't auto-load/handoff session state
3. **Session ≠ Track binding** - LEDGER.md doesn't know which track is active

## Solution

### Part 1: Metadata Consolidation
Merge 3 files → 1 per track:
- `.track-progress.json` → `metadata.json.generation`
- `.fb-progress.json` → `metadata.json.beads`

### Part 2: Session State Consolidation
Eliminate `session-state_*.json` → Use LEDGER.md frontmatter for session fields

### Part 3: Continuity ↔ Conductor Integration
- Add Phase 0.5 to `/conductor-implement` (auto-load + track binding)
- Add Phase 6.5 to `/conductor-finish` (auto-handoff)
- Add continuity load to `ds` (design sessions)

## Architecture

### Before (10 state files)

```
conductor/tracks/<id>/
├── metadata.json           # Track metadata
├── .track-progress.json    # Spec/plan state (REDUNDANT)
├── .fb-progress.json       # Beads filing state (REDUNDANT)
├── implement_state.json    # Implementation resume
└── finish-state.json       # Finish resume

.conductor/
├── session-state_*.json    # Per-agent session (REDUNDANT with LEDGER)
└── session-lock_*.json     # Lock files

conductor/sessions/active/
└── LEDGER.md               # Session state (no track binding)
```

### After (7 state files)

```
conductor/tracks/<id>/
├── metadata.json           # Track + generation + beads (CONSOLIDATED)
├── implement_state.json    # Implementation resume (keep)
└── finish-state.json       # Finish resume (keep)

.conductor/
└── session-lock_*.json     # Lock files (keep)

conductor/sessions/active/
└── LEDGER.md               # Session + track binding (EXTENDED)
```

## Consolidated metadata.json Schema

```json
{
  "track_id": "payments_20251227",
  "type": "feature",
  "status": "in_progress",
  "created_at": "...",
  "updated_at": "...",
  "description": "...",
  "priority": "medium",
  
  "artifacts": {
    "design": true,
    "spec": true,
    "plan": true,
    "beads": true
  },
  
  "threads": [...],
  
  "workflow": {
    "state": "IMPLEMENTING",
    "history": [...]
  },
  
  "generation": {
    "status": "complete",
    "specCreatedAt": "2025-12-27T10:00:00Z",
    "planCreatedAt": "2025-12-27T10:15:00Z",
    "rbCompletedAt": "2025-12-27T10:30:00Z"
  },
  
  "beads": {
    "status": "complete",
    "epicId": "my-workflow:3-xyz",
    "epics": [...],
    "issues": [...],
    "planTasks": {"task-1": "bd-1"},
    "beadToTask": {"bd-1": "task-1"},
    "crossTrackDeps": [],
    "reviewStatus": "complete",
    "reviewedAt": "..."
  }
}
```

## Extended LEDGER.md Format

```yaml
---
updated: 2025-12-27T10:30:00Z
session_id: T-abc123
platform: amp
bound_track: payments_20251227    # NEW: Track binding
bound_bead: bd-42                 # NEW: Current bead
mode: SA                          # NEW: From session-state
tdd_phase: GREEN                  # NEW: From session-state
heartbeat: 2025-12-27T10:35:00Z   # NEW: From session-state
---

# Session Ledger
...
```

## Continuity ↔ Conductor Integration

### Phase 0.5: Continuity Load (implement.md)

After Beads Preflight, before Setup Verification:

1. Check if LEDGER.md exists
2. If exists and stale (>24h): archive first
3. If bound_track != target_track: auto-archive with message
4. Update LEDGER with track binding
5. Display session context summary

### Phase 6.5: Continuity Handoff (finish-workflow.md)

After CODEMAPS, before completion:

1. Generate handoff from LEDGER + track state
2. Write to archive with trigger=`track-complete`
3. Delete LEDGER.md (track is finished)

### Design Session Init (design/SKILL.md)

At session initialization:

1. Load LEDGER.md if exists
2. Display prior context
3. Don't bind track (design may not have one yet)

## Requirements

### From Party Mode Sessions

| ID | Requirement | Priority |
|----|-------------|----------|
| REQ-C1 | Auto-load LEDGER at `/conductor-implement` start | P0 |
| REQ-C2 | Auto-archive on track switch with clear message | P0 |
| REQ-C3 | Auto-handoff at `/conductor-finish` end | P0 |
| REQ-C4 | Non-blocking (warn, never halt) | P0 |
| REQ-C5 | ds loads LEDGER if exists (optional) | P1 |
| REQ-C6 | Clear user feedback on auto-archive | P1 |

## File Scope

### Epic 1: Metadata Consolidation (25 files, ~900 lines)

**Schemas:**
- `schemas/metadata.schema.json` - Add generation + beads sections
- `schemas/track_progress.schema.json` - DELETE
- `schemas/fb_progress.schema.json` - DELETE

**Workflows:**
- `workflows/newtrack.md` - Write single metadata.json
- `workflows/implement.md` - Read from metadata.json.beads
- `workflows/validate.md` - Remove 2 file checks
- `workflows/revise.md` - Update beads reference

**Validation:**
- `validation/track/checks.md` - Remove 2 file checks
- `validation/track/snippets.md` - Remove 2 templates
- `validation/beads/checks.md` - Update to metadata.json

**Integration:**
- `beads-integration.md`, `beads-facade.md`, `track-init-beads.md`, `migrate-beads.md`

**Skills:**
- `conductor/SKILL.md`, `beads/FILE_BEADS.md`

**Docs:**
- `AGENTS.md`, `SETUP_GUIDE.md`, `TUTORIAL.md`, `CODEMAPS/conductor.md`

### Epic 2: Session State Consolidation (15 files, ~295 lines)

**Continuity:**
- `continuity/references/ledger-format.md` - Add session fields
- `continuity/SKILL.md` - Document extended frontmatter
- `hooks/continuity/src/continuity.ts` - Parse/write session fields

**Conductor/Beads:**
- `conductor/preflight-beads.md` - Use LEDGER instead of session-state
- `conductor/beads-session.md`, `tdd-checkpoints-beads.md`
- `beads-integration.md`, `beads-facade.md`

**Workflows:**
- `workflows/finish.md` - Remove session-state cleanup
- `workflows/implement.md` - Update output artifacts

### Epic 3: Continuity ↔ Conductor Integration (6 files, ~170 lines)

**Workflows:**
- `workflows/implement.md` - Add Phase 0.5
- `finish-workflow.md` - Add Phase 6.5

**Skills:**
- `continuity/SKILL.md` - Add Conductor Integration section
- `continuity/references/ledger-format.md` - Add bound_track
- `design/SKILL.md` - Add continuity load at init

**Docs:**
- `conductor/AGENTS.md` - Add gotcha about track-switch

## Execution Order

```
Epic 1 (Metadata) ─┐
                   ├─► Epic 3 (Continuity ↔ Conductor)
Epic 2 (Session) ──┘
```

Epic 1 and 2 can run in parallel. Epic 3 depends on Epic 2.

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Breaking existing tracks | Migration guide + backward compat |
| LEDGER corruption | Backup + graceful degradation |
| Schema size (~450 lines) | Add TypeScript types, validation tests |
| Rollback needed | Git - single commit per epic |

## Design Session Reference

- Thread: Current session
- Date: 2025-12-27
- Party Mode reviews: 4 rounds (PM, Architect, Developer, Analyst, Test Architect, Problem Solver, Tech Writer, Scrum Master, Quick Flow Dev, UX Designer, Innovation Strategist)
- Phases: DISCOVER → DEFINE → DEVELOP → DELIVER (all complete)
