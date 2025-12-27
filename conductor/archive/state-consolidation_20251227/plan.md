# Plan: State Consolidation + Continuity Integration

## Overview

| Metric | Value |
|--------|-------|
| Epics | 3 |
| Tasks | 21 |
| Files | 46 |
| Est. Lines | ~1365 |

## Epic 1: Metadata Consolidation

**Goal:** Merge .track-progress.json + .fb-progress.json → metadata.json

### Tasks

#### Phase 1: Schema Update

- [ ] **1.1** Update `skills/conductor/references/schemas/metadata.schema.json`
  - Add `generation` section (status, specCreatedAt, planCreatedAt, rbCompletedAt)
  - Add `beads` section (status, epicId, epics, issues, planTasks, beadToTask, crossTrackDeps, reviewStatus, reviewedAt)
  - ~80 lines

- [ ] **1.2** Delete `skills/conductor/references/schemas/track_progress.schema.json`
  - Remove file entirely
  - -77 lines

- [ ] **1.3** Delete `skills/conductor/references/schemas/fb_progress.schema.json`
  - Remove file entirely
  - -220 lines

#### Phase 2: Workflow Updates

- [ ] **1.4** Update `skills/conductor/references/workflows/newtrack.md`
  - Remove .track-progress.json creation
  - Remove .fb-progress.json creation
  - Write generation section to metadata.json
  - ~50 lines changed

- [ ] **1.5** Update `skills/conductor/references/commands/newTrack.toml`
  - Update Phase 1.3 to write single file
  - Remove 2 file creation blocks
  - ~40 lines

- [ ] **1.6** Update `skills/conductor/references/workflows/validate.md`
  - Remove .track-progress.json checks
  - Remove .fb-progress.json checks
  - Update validation to check metadata.json sections
  - ~30 lines

#### Phase 3: Beads Integration

- [ ] **1.7** Update `skills/beads/references/FILE_BEADS.md`
  - Write to metadata.json.beads instead of .fb-progress.json
  - Update file existence checks
  - ~15 lines

- [ ] **1.8** Update `skills/conductor/references/beads-facade.md`
  - Update planTasks location reference
  - ~15 lines

- [ ] **1.9** Update `skills/conductor/references/conductor/track-init-beads.md`
  - Write to metadata.json.beads
  - ~30 lines

- [ ] **1.10** Update `skills/conductor/references/beads-integration.md`
  - Update state file references
  - Remove .fb-progress.json section
  - ~20 lines

#### Phase 4: Validation & Docs

- [ ] **1.11** Update `skills/conductor/references/validation/track/checks.md`
  - Remove 2 file checks
  - Add metadata.json.generation check
  - Add metadata.json.beads check
  - ~50 lines

- [ ] **1.12** Update `skills/conductor/references/validation/track/snippets.md`
  - Remove 2 template sections
  - ~80 lines

- [ ] **1.13** Update documentation files
  - `AGENTS.md` - Update state file table
  - `SETUP_GUIDE.md` - Update state file table
  - `TUTORIAL.md` - Update references
  - `conductor/CODEMAPS/conductor.md` - Update file list
  - `skills/conductor/SKILL.md` - Update state file docs
  - ~50 lines total

---

## Epic 2: Session State Consolidation

**Goal:** Eliminate session-state_*.json → Use LEDGER.md frontmatter

### Tasks

#### Phase 1: LEDGER Format Extension

- [ ] **2.1** Update `skills/continuity/references/ledger-format.md`
  - Add bound_track field
  - Add bound_bead field
  - Add mode field (SA/MA)
  - Add tdd_phase field
  - Add heartbeat field
  - ~30 lines

- [ ] **2.2** Update `skills/continuity/SKILL.md`
  - Document extended frontmatter
  - ~15 lines

#### Phase 2: Preflight/Session Updates

- [ ] **2.3** Update `skills/conductor/references/conductor/preflight-beads.md`
  - Read mode from LEDGER.md frontmatter
  - Write mode to LEDGER.md frontmatter
  - Remove session-state_*.json creation
  - ~60 lines

- [ ] **2.4** Update `skills/conductor/references/conductor/beads-session.md`
  - Update session file references to LEDGER.md
  - ~30 lines

- [ ] **2.5** Update `skills/conductor/references/conductor/tdd-checkpoints-beads.md`
  - Write tdd_phase to LEDGER.md frontmatter
  - ~20 lines

#### Phase 3: Cleanup

- [ ] **2.6** Update `skills/conductor/references/beads-integration.md`
  - Remove session-state section
  - Update references
  - ~20 lines

- [ ] **2.7** Update documentation
  - Remove session-state from state file tables
  - Update AGENTS.md, SETUP_GUIDE.md, TUTORIAL.md
  - ~20 lines

---

## Epic 3: Continuity ↔ Conductor Integration

**Goal:** Chain continuity to implement/finish workflows

**Depends on:** Epic 2 (LEDGER format extension)

### Tasks

#### Phase 1: Implement Integration

- [x] **3.1** Update `skills/conductor/references/workflows/implement.md`
  - Add Phase 0.5: Continuity Load
  - Include track binding check
  - Include auto-archive on track switch
  - ~50 lines

#### Phase 2: Finish Integration

- [x] **3.2** Update `skills/conductor/references/finish-workflow.md`
  - Add Phase 6.5: Continuity Handoff
  - Create handoff with trigger=track-complete
  - Delete LEDGER.md after archive
  - ~40 lines

#### Phase 3: Design Session Integration

- [x] **3.3** Update `skills/design/SKILL.md`
  - Add continuity load at Session Initialization
  - Load LEDGER.md if exists
  - Display prior context
  - ~15 lines

#### Phase 4: Continuity Skill Updates

- [x] **3.4** Update `skills/continuity/SKILL.md`
  - Add "Conductor Integration" section
  - Document auto-triggers
  - Document track binding
  - Document non-blocking guarantee
  - ~40 lines

- [x] **3.5** Update `skills/continuity/references/ledger-format.md`
  - Document bound_track semantics
  - Add Track Binding section
  - ~20 lines

#### Phase 5: Documentation

- [x] **3.6** Update `conductor/AGENTS.md`
  - Add gotcha about track-switch auto-archive
  - Add pattern about continuity chain
  - ~10 lines

---

## Dependency Graph

```
Epic 1 (Metadata)    Epic 2 (Session)
      │                    │
      │                    ▼
      │              Epic 3 (Continuity)
      │                    │
      └───────────────────►│
                           ▼
                      COMPLETE
```

## Execution Notes

1. **Epic 1 and 2 can run in parallel**
2. **Epic 3 requires Epic 2** (LEDGER format must be extended first)
3. **Epic 3 benefits from Epic 1** (can read from consolidated metadata.json)

## Verification

After each epic:
- [ ] Run `/conductor-validate` on a test track
- [ ] Verify no broken references with `./scripts/validate-links.sh .`
- [ ] Test happy path workflow: ds → newtrack → fb → implement → finish
