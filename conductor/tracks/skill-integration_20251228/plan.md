# Skill Integration — Implementation Plan

## Overview

| Attribute | Value |
|-----------|-------|
| Track ID | skill-integration_20251228 |
| Approach | Big Bang (single PR) |
| Total Tasks | 14 |
| Estimated Effort | Medium |

---

## Phase 1: Scaffold Structure

### Task 1.1: Create reference directories
- [ ] Create `skills/conductor/references/prompts/`
- [ ] Create `skills/conductor/references/coordination/`
- [ ] Create `skills/conductor/references/tdd/`
- [ ] Create `skills/conductor/references/verification/`
- [ ] Create `skills/conductor/references/doc-sync/`
- [ ] Create `skills/conductor/references/ledger/`
- [ ] Create `skills/conductor/references/finish/`
- [ ] Commit: `chore: scaffold conductor/references/ structure`

**Files:** 7 new directories

---

## Phase 2: Move Reference Files

### Task 2.1: Move prompts from subagent-driven-development
- [ ] `git mv skills/subagent-driven-development/implementer-prompt.md skills/conductor/references/prompts/`
- [ ] `git mv skills/subagent-driven-development/spec-reviewer-prompt.md skills/conductor/references/prompts/`
- [ ] `git mv skills/subagent-driven-development/code-quality-reviewer-prompt.md skills/conductor/references/prompts/`
- [ ] Commit: `refactor: move prompts from subagent-dev`

**Files:** 3 moved

### Task 2.2: Move coordination from dispatching-parallel-agents
- [ ] `git mv skills/dispatching-parallel-agents/references/agent-coordination/* skills/conductor/references/coordination/`
- [ ] Commit: `refactor: move coordination from dispatching`

**Files:** ~5 moved (patterns/, examples/, workflow.md)

### Task 2.3: Move continuity to ledger
- [ ] `git mv skills/continuity/references/ledger-format.md skills/conductor/references/ledger/format.md`
- [ ] `git mv skills/continuity/references/handoff-format.md skills/conductor/references/ledger/handoff.md`
- [ ] `git mv skills/continuity/references/amp-setup.md skills/conductor/references/ledger/amp-setup.md`
- [ ] Commit: `refactor: move continuity to ledger/`

**Files:** 3 moved

### Task 2.4: Move doc-sync references
- [ ] `git mv skills/doc-sync/references/* skills/conductor/references/doc-sync/`
- [ ] Commit: `refactor: move doc-sync references`

**Files:** ~4 moved

---

## Phase 3: Extract and Create Content

### Task 3.1: Extract TDD content
- [ ] Read `skills/test-driven-development/SKILL.md`
- [ ] Create `skills/conductor/references/tdd/cycle.md` with TDD cycle content
- [ ] Create `skills/conductor/references/tdd/gates.md` with enforcement gates
- [ ] Commit: `refactor: extract TDD content to conductor`

**Files:** 2 created

### Task 3.2: Extract verification content
- [ ] Read `skills/verification-before-completion/SKILL.md`
- [ ] Create `skills/conductor/references/verification/gate.md` with gate logic
- [ ] Create `skills/conductor/references/verification/rollback.md` with rollback options
- [ ] Commit: `refactor: extract verification content to conductor`

**Files:** 2 created

### Task 3.3: Extract finishing-branch content
- [ ] Read `skills/finishing-a-development-branch/SKILL.md`
- [ ] Create `skills/conductor/references/finish/branch-options.md` with Merge/PR/Clean options
- [ ] Create `skills/conductor/references/finish/cleanup.md` with cleanup procedures
- [ ] Commit: `refactor: extract finishing-branch to conductor`

**Files:** 2 created

---

## Phase 4: LEDGER System

### Task 4.1: Create LEDGER.log format documentation
- [ ] Create `skills/conductor/references/ledger/log-format.md` with format spec
- [ ] Create `skills/conductor/references/ledger/rotation.md` with rotation logic
- [ ] Create `skills/conductor/references/ledger/recovery.md` with resume logic
- [ ] Commit: `feat: LEDGER.log format documentation`

**Files:** 3 created

### Task 4.2: Archive existing LEDGER.md
- [ ] Check if `conductor/sessions/active/LEDGER.md` exists
- [ ] If exists: `mv LEDGER.md LEDGER.md.backup`
- [ ] Create fresh `conductor/sessions/active/LEDGER.log` with header
- [ ] Commit: `feat: initialize LEDGER.log fresh start`

**Files:** 1 archived, 1 created

---

## Phase 5: Update Conductor SKILL.md

### Task 5.1: Refactor conductor SKILL.md to overview
- [ ] Read current `skills/conductor/SKILL.md`
- [ ] Create slim version (~100 lines) with:
  - Overview
  - Triggers
  - Workflow diagram link
  - References links to all subdirs
- [ ] Commit: `refactor: conductor SKILL.md to overview only`

**Files:** 1 modified

---

## Phase 6: Update External References

### Task 6.1: Update writing-skills references
- [ ] Read `skills/writing-skills/SKILL.md`
- [ ] Find references to `test-driven-development`
- [ ] Update to `conductor/references/tdd/`
- [ ] Commit: `fix: update writing-skills references`

**Files:** 1 modified

### Task 6.2: Move discipline rules to AGENTS.md
- [ ] Read `skills/using-superpowers/SKILL.md`
- [ ] Extract discipline rules section
- [ ] Append to `AGENTS.md` under new `## Skill Discipline` section
- [ ] Commit: `refactor: move discipline rules to AGENTS.md`

**Files:** 1 modified

---

## Phase 7: Delete Merged Skills

### Task 7.1: Verify all content moved
- [ ] Grep for references to deleted skills
- [ ] Verify file counts match
- [ ] Document any remaining references

### Task 7.2: Delete skill directories
- [ ] `rm -rf skills/create-plan`
- [ ] `rm -rf skills/dispatching-parallel-agents`
- [ ] `rm -rf skills/subagent-driven-development`
- [ ] `rm -rf skills/test-driven-development`
- [ ] `rm -rf skills/verification-before-completion`
- [ ] `rm -rf skills/doc-sync`
- [ ] `rm -rf skills/continuity`
- [ ] `rm -rf skills/finishing-a-development-branch`
- [ ] `rm -rf skills/using-superpowers`
- [ ] Commit: `chore: delete 9 merged skills`

**Directories deleted:** 9

---

## Phase 8: Verification

### Task 8.1: Smoke test
- [ ] Verify `ls skills/ | wc -l` = 6
- [ ] Verify `ls skills/conductor/references/` shows 9 dirs
- [ ] Verify no broken references: `grep -r "skills/create-plan\|skills/dispatching\|skills/subagent\|skills/test-driven\|skills/verification\|skills/doc-sync\|skills/continuity\|skills/finishing\|skills/using-superpowers" .`
- [ ] Test LEDGER.log write/read
- [ ] Commit: `test: verify skill integration complete`

---

## Summary

| Phase | Tasks | Files Affected |
|-------|-------|----------------|
| 1. Scaffold | 1 | 7 dirs |
| 2. Move refs | 4 | ~15 files |
| 3. Extract content | 3 | 6 files |
| 4. LEDGER | 2 | 4 files |
| 5. Conductor SKILL.md | 1 | 1 file |
| 6. External refs | 2 | 2 files |
| 7. Delete | 2 | 9 dirs |
| 8. Verify | 1 | - |
| **Total** | **16** | **~35 files, 16 dirs** |

---

## Rollback Plan

If issues found after merge:
```bash
git revert HEAD~14..HEAD  # Revert all 14 commits
# OR
git reset --hard <commit-before-merge>
```
