# Implementation Plan: HumanLayer-Inspired Handoff System

## Overview

Replace LEDGER.md/continuity with HumanLayer-inspired handoff system: shareable, standalone, and integrated with Conductor.

**Estimated Total: 8 hours**

---

## Phase 1: Foundation (Infrastructure)

### Epic 1.1: Directory Structure & Cleanup

- [ ] **1.1.1** Create `conductor/handoffs/general/` directory
- [ ] **1.1.2** Create `conductor/handoffs/general/index.md` with empty table
- [ ] **1.1.3** Delete `conductor/sessions/` directory
- [ ] **1.1.4** Delete `skills/conductor/references/ledger/` directory (6 files)
- [ ] **1.1.5** Create `skills/continuity/SKILL.md` deprecation stub

**Acceptance Criteria:**
- `conductor/handoffs/general/index.md` exists with valid markdown table
- `conductor/sessions/` does not exist
- `skills/conductor/references/ledger/` does not exist
- `skills/continuity/SKILL.md` redirects to handoff commands

---

## Phase 2: Core Handoff Logic

### Epic 2.1: Create Handoff Reference

- [x] **2.1.1** Create `skills/conductor/references/handoff/` directory `8f03c0b`
- [x] **2.1.2** Create `template.md` with 4-section template + frontmatter schema `8f03c0b`
- [x] **2.1.3** Create `create.md` with `/create_handoff` workflow: `8f03c0b`
  - Detect context (track or general)
  - Gather git metadata
  - Scan for secrets
  - Write handoff file
  - Append to index.md
  - Update metadata.json.last_activity
- [x] **2.1.4** Create `triggers.md` with 6 trigger definitions and integration points `8f03c0b`
- [x] **2.1.5** Add secrets scanning patterns (hardcoded + configurable + gitleaks) `8f03c0b`

**Acceptance Criteria:**
- All 4 reference files exist in `handoff/`
- Template has correct YAML frontmatter schema
- Create workflow handles all edge cases (no git, no track, secrets)

### Epic 2.2: Resume Handoff Reference

- [x] **2.2.1** Create `resume.md` with `/resume_handoff` workflow: `8f03c0b`
  - Parse input (path, track, or none)
  - Smart discovery (list tracks, auto-select if 1)
  - Load handoff fully
  - Validate git state
  - Present analysis
  - Create todo list
- [x] **2.2.2** Add index auto-repair logic (scan dir, rebuild if corrupted) `8f03c0b`
- [x] **2.2.3** Add stale handoff warning (>7 days) `8f03c0b`
- [x] **2.2.4** Add branch mismatch warning `8f03c0b`

**Acceptance Criteria:**
- Resume workflow handles all 3 input modes
- Auto-repair successfully rebuilds corrupted index
- Warnings display for stale/mismatch scenarios

### Epic 2.3: Idle Detection

- [x] **2.3.1** Create `idle-detection.md` with gap detection logic `8f03c0b`
- [x] **2.3.2** Define `.last_activity` marker file behavior `8f03c0b`
- [x] **2.3.3** Add configurable threshold support in `workflow.md` `8f03c0b`
- [x] **2.3.4** Update `skills/maestro-core/SKILL.md` to include idle detection `8f03c0b`

**Acceptance Criteria:**
- Idle detection triggers after 30min gap
- Threshold configurable via `workflow.md`
- Prompt offers Y/n/skip options

---

## Phase 3: Integration Points

### Epic 3.1: Conductor Command Integration

- [x] **3.1.1** Update `skills/conductor/SKILL.md`: `pending`
  - Add `/create_handoff`, `/resume_handoff`, `/conductor-handoff` triggers
  - Remove LEDGER.md references (line 158-159)
- [x] **3.1.2** Update `references/workflows/newtrack.md`: `pending`
  - Add `design-end` handoff trigger at Phase 7
  - Remove LEDGER refs
- [x] **3.1.3** Update `references/workflows/implement.md`: `pending`
  - Add `epic-start` handoff load before each epic
  - Add `epic-end` handoff create after each epic
  - Replace `continuity load` references
- [x] **3.1.4** Update `references/finish-workflow.md`: `pending`
  - Add `pre-finish` handoff load at Phase 0
  - Add archive logic at Phase 5
  - Replace continuity handoff references
- [x] **3.1.5** Update `references/workflows/setup.md`: `pending`
  - Create `handoffs/general/` instead of `sessions/`

**Acceptance Criteria:**
- All 6 triggers fire at correct integration points
- No LEDGER/continuity references remain in updated files

### Epic 3.2: Validation & State Integration

- [x] **3.2.1** Update `references/validation/lifecycle.md`: `pending`
  - Replace LEDGER state tracking with metadata.json
  - Update gate tracking references
- [x] **3.2.2** Update `references/validation/shared/*.md` (5 files): `pending`
  - Replace LEDGER Integration sections with handoff integration
- [x] **3.2.3** Update `references/conductor/preflight-beads.md`: `pending`
  - Remove LEDGER.md creation
  - Add handoff system initialization
- [x] **3.2.4** Update `references/conductor/beads-session.md`: `pending`
  - Replace LEDGER references with handoff
- [x] **3.2.5** Update `references/conductor/tdd-checkpoints-beads.md`: `pending`
  - Replace LEDGER refs with metadata.json.validation
- [x] **3.2.6** Update `references/tdd/cycle.md`: `pending`
  - Replace LEDGER refs

**Acceptance Criteria:**
- Validation state tracked in metadata.json, not LEDGER
- All 5 validation gate files updated
- No LEDGER references in conductor/ refs

### Epic 3.3: Additional Reference Updates

- [x] **3.3.1** Update `skills/maestro-core/references/hierarchy.md` `pending`
- [x] **3.3.2** Update `skills/beads/references/WORKFLOWS.md` `pending` (no LEDGER refs found)
- [x] **3.3.3** Update `skills/design/SKILL.md` (line 41-50 LEDGER check) `pending`
- [x] **3.3.4** Update `references/beads-facade.md` (state files table) `pending`
- [x] **3.3.5** Update `references/beads-integration.md` `pending`
- [x] **3.3.6** Update `references/coordination/patterns/session-lifecycle.md` `pending` (no LEDGER refs found)
- [x] **3.3.7** Update `references/validation/quality/judge-prompt.md` `pending` (no LEDGER refs found)

**Acceptance Criteria:**
- All files updated with handoff references
- No LEDGER/continuity references remain

---

## Phase 4: Documentation & Testing

### Epic 4.1: Documentation Updates

- [x] **4.1.1** Create `docs/handoff-system.md` user guide
- [x] **4.1.2** Update `AGENTS.md` (remove sessions refs, update continuity section)
- [x] **4.1.3** Update `conductor/AGENTS.md` (remove LEDGER gotchas/patterns)
- [x] **4.1.4** Update `conductor/CODEMAPS/overview.md` (structure diagram)
- [x] **4.1.5** Update `conductor/CODEMAPS/skills.md` (remove ledger/)
- [x] **4.1.6** Update `SETUP_GUIDE.md` (state files table)
- [x] **4.1.7** Update `TUTORIAL.md` (state files table, add handoff example)
- [x] **4.1.8** Update `docs/PIPELINE_ARCHITECTURE.md` (state files table)
- [x] **4.1.9** Update `docs/GLOBAL_CONFIG.md` (continuity refs)
- [x] **4.1.10** Update `README.md` (continuity refs)

**Acceptance Criteria:**
- User guide explains create/resume workflows
- All docs reference handoff system, not LEDGER/continuity

### Epic 4.2: Script Updates & Testing

- [x] **4.2.1** Update `scripts/test-hooks.sh` (remove LEDGER tests)
- [ ] **4.2.2** Manual test: `/create_handoff` with no track
- [ ] **4.2.3** Manual test: `/create_handoff` with active track
- [ ] **4.2.4** Manual test: `/resume_handoff` smart discovery
- [ ] **4.2.5** Manual test: Secrets scanning with `sk-test123`
- [ ] **4.2.6** Manual test: Full track lifecycle (design → finish)
- [ ] **4.2.7** Manual test: Archive on `/conductor-finish`

**Acceptance Criteria:**
- All manual tests pass
- No LEDGER tests in test-hooks.sh

---

## Automated Verification

```bash
# Check directories exist
ls -la conductor/handoffs/general/index.md

# Check deletions
! ls conductor/sessions 2>/dev/null
! ls skills/conductor/references/ledger 2>/dev/null

# Check new files
ls skills/conductor/references/handoff/
ls skills/continuity/SKILL.md

# Check no LEDGER refs in key files
! grep -l "LEDGER" skills/conductor/SKILL.md
! grep -l "LEDGER" skills/maestro-core/SKILL.md
```

---

## Task Dependencies

```
1.1 (Foundation) → 2.1 (Create) → 3.1 (Commands)
                 → 2.2 (Resume) → 3.2 (Validation)
                 → 2.3 (Idle)   → 3.3 (Refs)
                                → 4.1 (Docs)
                                → 4.2 (Testing)
```

---

## Summary

| Phase | Epics | Tasks | Est. Hours |
|-------|-------|-------|------------|
| 1. Foundation | 1 | 5 | 1h |
| 2. Core Logic | 3 | 13 | 3h |
| 3. Integration | 3 | 18 | 2.5h |
| 4. Docs & Testing | 2 | 17 | 1.5h |
| **Total** | **9** | **53** | **8h** |
