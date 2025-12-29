# Plan: maestro-core

## Overview

Create `maestro-core` skill and update all existing skills with Prerequisites pattern.

**Total Effort:** 10 hours
**Track ID:** maestro-core

---

## Phase 1: Foundation (3 hours)

Create the maestro-core skill with all reference documentation.

### Epic 1.1: Create maestro-core SKILL.md

- [ ] 1.1.1 Create `skills/maestro-core/` directory structure
- [ ] 1.1.2 Create `skills/maestro-core/SKILL.md` with:
  - Frontmatter (name, description)
  - Quick-start example
  - 5-level hierarchy summary table
  - Fallback policy summary table
  - Links to references
- [ ] 1.1.3 Verify ≤100 lines

### Epic 1.2: Create hierarchy.md

- [ ] 1.2.1 Create `skills/maestro-core/references/` directory
- [ ] 1.2.2 Create `skills/maestro-core/references/hierarchy.md` with:
  - Detailed 5-level hierarchy with descriptions
  - HALT vs DEGRADE decision matrix
  - Message format standards
  - Enforcement rules

### Epic 1.3: Create routing.md

- [ ] 1.3.1 Create `skills/maestro-core/references/routing.md` with:
  - Trigger disambiguation table (10+ rules)
  - Context-aware routing logic
  - Beads vs TodoWrite decision rules
  - Worktree invocation points

---

## Phase 2: Design Update (1.5 hours)

Update design skill to support standalone mode.

### Epic 2.1: Design DEGRADE Mode

- [ ] 2.1.1 Read current `skills/design/SKILL.md`
- [ ] 2.1.2 Add Prerequisites section with maestro-core reference
- [ ] 2.1.3 Change Session Initialization:
  - Replace HALT on missing conductor/ with DEGRADE
  - Add standalone mode warning message
  - Add skip logic for CODEMAPS when missing
- [ ] 2.1.4 Test: run `ds` without conductor/ directory

---

## Phase 3: Skill Updates (3 hours)

Add Prerequisites section to all remaining skills.

### Epic 3.1: Update conductor skill

- [ ] 3.1.1 Add Prerequisites section to `skills/conductor/SKILL.md`
- [ ] 3.1.2 Reference maestro-core with hierarchy level

### Epic 3.2: Update beads skill

- [ ] 3.2.1 Add Prerequisites section to `skills/beads/SKILL.md`
- [ ] 3.2.2 Reference maestro-core with hierarchy level

### Epic 3.3: Update using-git-worktrees skill

- [ ] 3.3.1 Add Prerequisites section to `skills/using-git-worktrees/SKILL.md`
- [ ] 3.3.2 Reference maestro-core with hierarchy level

### Epic 3.4: Update sharing-skills skill

- [ ] 3.4.1 Add Prerequisites section to `skills/sharing-skills/SKILL.md`
- [ ] 3.4.2 Reference maestro-core with hierarchy level

---

## Phase 4: Documentation (1.5 hours)

Update writing-skills with dependency documentation.

### Epic 4.1: Update writing-skills

- [ ] 4.1.1 Add "Skill Dependencies" section to `skills/writing-skills/SKILL.md`:
  - Declaring requirements (`REQUIRED SUB-SKILL:` pattern)
  - Maestro Core integration example
  - Hierarchy levels table
  - HALT vs DEGRADE guidelines
- [ ] 4.1.2 Add example showing Prerequisites section format

---

## Phase 5: Verification (1 hour)

Verify all acceptance criteria.

### Epic 5.1: Final Verification

- [ ] 5.1.1 AC-1: Verify maestro-core/SKILL.md exists
- [ ] 5.1.2 AC-2: Verify ≤100 lines (`wc -l`)
- [ ] 5.1.3 AC-3: Verify hierarchy.md has 5-level table
- [ ] 5.1.4 AC-4: Verify routing.md has trigger table
- [ ] 5.1.5 AC-5: Test `ds` standalone mode
- [ ] 5.1.6 AC-6: Grep for maestro-core in all 5 skills
- [ ] 5.1.7 AC-7: Verify writing-skills has dependency docs

---

## Summary

| Phase | Epic | Tasks | Hours |
|-------|------|-------|-------|
| 1 | 1.1, 1.2, 1.3 | 7 | 3 |
| 2 | 2.1 | 4 | 1.5 |
| 3 | 3.1, 3.2, 3.3, 3.4 | 8 | 3 |
| 4 | 4.1 | 2 | 1.5 |
| 5 | 5.1 | 7 | 1 |
| **Total** | **9 epics** | **28 tasks** | **10** |
