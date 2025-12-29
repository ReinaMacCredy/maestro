# Plan: Auto-Continuity for Hookless Agents

## Epic 1: Documentation Updates

### Task 1.1: Update docs/GLOBAL_CONFIG.md
- [ ] Remove "Amp-Specific: Continuity Protocol" section (lines 143-169)
- [ ] Add new "Session Lifecycle (All Agents)" section
- [ ] Include entry point table (ds, /conductor-implement, /conductor-finish)
- [ ] Add "First Message Behavior" explanation
- [ ] Add fallback note for non-Conductor workflows

**Files:** `docs/GLOBAL_CONFIG.md`
**Verification:** Read file, verify no manual continuity commands

### Task 1.2: Update AGENTS.md
- [ ] Remove "Continuity Protocol" section with manual commands (lines 231-238)
- [ ] Keep skill discipline rules (already correct)

**Files:** `AGENTS.md`
**Verification:** `grep "continuity load" AGENTS.md` returns empty

### Task 1.3: Update conductor/AGENTS.md
- [ ] Remove gotcha: "Amp Code doesn't support hooks - use manual..." (line 82)
- [ ] Add new gotcha: "Continuity is automatic via workflow entry points"

**Files:** `conductor/AGENTS.md`
**Verification:** Read file, verify updated gotcha

## Epic 2: Skill Updates

### Task 2.1: Update maestro-core/SKILL.md
- [ ] Add "Session Lifecycle" section after "Prerequisites Pattern"
- [ ] Include entry point table with ledger actions
- [ ] Add "No manual commands" note
- [ ] Link to conductor/references/ledger/

**Files:** `skills/maestro-core/SKILL.md`
**Verification:** Read file, verify new section

### Task 2.2: Update maestro-core/references/hierarchy.md
- [ ] Update "Skill Loading Order" section (line 96)
- [ ] Add step 2: "maestro-core triggers ledger check if workflow context exists"

**Files:** `skills/maestro-core/references/hierarchy.md`
**Verification:** Read file, verify updated loading order

### Task 2.3: Deprecate ledger/amp-setup.md
- [ ] Add deprecation notice at top of file
- [ ] Add "This document is kept for historical reference only"
- [ ] Add "See docs/GLOBAL_CONFIG.md for current approach"

**Files:** `skills/conductor/references/ledger/amp-setup.md`
**Verification:** Read file, verify deprecation notice

### Task 2.4: Update workflows/setup.md
- [ ] Remove/update Phase 8: Continuity Setup that prompts for manual protocol
- [ ] Change to: "Continuity is automatic via workflow entry points"

**Files:** `skills/conductor/references/workflows/setup.md`
**Verification:** Read file, verify no manual prompt

## Epic 3: Verification

### Task 3.1: Grep verification
- [ ] Run: `grep -r "continuity load" docs/ AGENTS.md` for manual instructions
- [ ] Run: `grep -r "run continuity" .` to find remaining manual references
- [ ] Fix any remaining manual instruction references

**Verification:** All greps return 0 matches for user-facing manual instructions

### Task 3.2: Integration test
- [ ] Start new session with existing LEDGER.md
- [ ] Run `ds` and verify context loads
- [ ] Verify no manual command prompts appear

**Verification:** Context loads automatically, no manual prompts

## Summary

| Epic | Tasks | Estimated Hours |
|------|-------|-----------------|
| 1: Documentation | 3 | 1.0 |
| 2: Skill Updates | 4 | 1.0 |
| 3: Verification | 2 | 0.5 |
| **Total** | **9** | **2.5** |
