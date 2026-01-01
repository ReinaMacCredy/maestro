# Plan: Skill Restructure to Anthropic Standard

**Track ID:** skill-restructure
**Created:** 2026-01-01
**Estimated:** 4-6 hours

## Overview

Hybrid implementation: Foundation phase (pilot beads), then parallel rollout.

## Phase 0: Directory Cleanup (15 min)

### Epic 0.1: Symlink Setup

- [ ] **0.1.1** Backup `.claude/skills/` contents list
- [ ] **0.1.2** Delete `.claude/skills/` directory contents
- [ ] **0.1.3** Create symlink: `.claude/skills/` → `skills/`
- [ ] **0.1.4** Verify symlink resolves: `ls -la .claude/skills/`

**Files:** `.claude/skills/`

---

## Phase 1: Foundation (1.5 hours)

### Epic 1.1: Clone skill-creator

- [ ] **1.1.1** Create `skills/skill-creator/` directory
- [ ] **1.1.2** Clone `SKILL.md` from Anthropic repo
- [ ] **1.1.3** Clone `LICENSE.txt`
- [ ] **1.1.4** Clone `references/output-patterns.md`
- [ ] **1.1.5** Clone `references/workflows.md`
- [ ] **1.1.6** Clone `scripts/init_skill.py`
- [ ] **1.1.7** Clone `scripts/package_skill.py`
- [ ] **1.1.8** Clone `scripts/quick_validate.py`
- [ ] **1.1.9** Test: `python skills/skill-creator/scripts/quick_validate.py skills/skill-creator`

**Files:** `skills/skill-creator/**`

### Epic 1.2: Create maestro-core Hub

- [ ] **1.2.1** Create `skills/maestro-core/` directory structure
- [ ] **1.2.2** Create `skills/maestro-core/SKILL.md` with:
  - Frontmatter (name, description, version)
  - Core Principles (3-5 bullets)
  - Workflow Chain diagram
  - Routing Table (trigger → skill)
  - Fallback Policies table
  - Quick Reference to references/
  - Related section
- [ ] **1.2.3** Create `skills/maestro-core/references/workflow-chain.md`
- [ ] **1.2.4** Create `skills/maestro-core/references/routing-table.md`
- [ ] **1.2.5** Create `skills/maestro-core/references/glossary.md`
- [ ] **1.2.6** Validate: `wc -l skills/maestro-core/SKILL.md` ≤200

**Files:** `skills/maestro-core/**`

### Epic 1.3: Pilot - Refactor beads

- [ ] **1.3.1** Read current `skills/beads/SKILL.md` (160 lines)
- [ ] **1.3.2** Extract Core Principles from content
- [ ] **1.3.3** Create Quick Reference table
- [ ] **1.3.4** Add Anti-Patterns section
- [ ] **1.3.5** Add Guidelines section
- [ ] **1.3.6** Add Related section (maestro-core, conductor, orchestrator)
- [ ] **1.3.7** Restructure to template format
- [ ] **1.3.8** Validate: `python skills/skill-creator/scripts/quick_validate.py skills/beads`
- [ ] **1.3.9** Validate: `wc -l skills/beads/SKILL.md` ≤100

**Files:** `skills/beads/SKILL.md`

---

## Phase 2: Parallel Rollout (3 hours)

### Track A: Complex Skills (design, conductor)

#### Epic 2.1: Refactor design

- [ ] **2.1.1** Read current `skills/design/SKILL.md` (745 lines)
- [ ] **2.1.2** Extract content for `references/double-diamond.md`
- [ ] **2.1.3** Extract content for `references/apc-checkpoints.md`
- [ ] **2.1.4** Extract content for `references/session-init.md`
- [ ] **2.1.5** Extract Core Principles (5 bullets max)
- [ ] **2.1.6** Create Quick Reference table
- [ ] **2.1.7** Add Anti-Patterns, Guidelines, Related sections
- [ ] **2.1.8** Restructure SKILL.md to template (target: ≤100 lines)
- [ ] **2.1.9** Validate with quick_validate.py

**Files:** `skills/design/SKILL.md`, `skills/design/references/*.md`

#### Epic 2.2: Refactor conductor

- [ ] **2.2.1** Read current `skills/conductor/SKILL.md` (614 lines)
- [ ] **2.2.2** Identify content already in references/ (reuse)
- [ ] **2.2.3** Extract remaining detailed content to references/
- [ ] **2.2.4** Extract Core Principles
- [ ] **2.2.5** Create Quick Reference table
- [ ] **2.2.6** Add Anti-Patterns, Guidelines, Related sections
- [ ] **2.2.7** Restructure SKILL.md to template (target: ≤100 lines)
- [ ] **2.2.8** Validate with quick_validate.py

**Files:** `skills/conductor/SKILL.md`

### Track B: Medium Skill (orchestrator)

#### Epic 2.3: Refactor orchestrator

- [ ] **2.3.1** Read current `skills/orchestrator/SKILL.md` (414 lines)
- [ ] **2.3.2** Identify content already in references/
- [ ] **2.3.3** Extract remaining content to references/
- [ ] **2.3.4** Extract Core Principles
- [ ] **2.3.5** Create Quick Reference table
- [ ] **2.3.6** Add Anti-Patterns, Guidelines, Related sections
- [ ] **2.3.7** Restructure SKILL.md to template (target: ≤100 lines)
- [ ] **2.3.8** Validate with quick_validate.py

**Files:** `skills/orchestrator/SKILL.md`

### Track C: Simple Skills (3 skills)

#### Epic 2.4: Refactor writing-skills

- [ ] **2.4.1** Read current (765 lines)
- [ ] **2.4.2** Extract examples to references/
- [ ] **2.4.3** Restructure to template
- [ ] **2.4.4** Validate

**Files:** `skills/writing-skills/SKILL.md`

#### Epic 2.5: Refactor sharing-skills

- [ ] **2.5.1** Read current (199 lines)
- [ ] **2.5.2** Add template sections
- [ ] **2.5.3** Restructure to template
- [ ] **2.5.4** Validate

**Files:** `skills/sharing-skills/SKILL.md`

#### Epic 2.6: Refactor using-git-worktrees

- [ ] **2.6.1** Read current (219 lines)
- [ ] **2.6.2** Add template sections
- [ ] **2.6.3** Restructure to template
- [ ] **2.6.4** Validate

**Files:** `skills/using-git-worktrees/SKILL.md`

---

## Phase 3: Final Validation (30 min)

### Epic 3.1: Comprehensive Validation

- [ ] **3.1.1** Run `wc -l skills/*/SKILL.md` - all ≤500
- [ ] **3.1.2** Run `quick_validate.py` on all 9 skills
- [ ] **3.1.3** Validate all internal links resolve
- [ ] **3.1.4** Validate Related sections are bidirectional
- [ ] **3.1.5** Test workflow: `ds` triggers design skill
- [ ] **3.1.6** Test workflow: `fb` triggers beads skill
- [ ] **3.1.7** Test workflow: `bd ready` works

**Files:** All skills

---

## Track Assignments

| Track | Agent | Epics | File Scope | Depends On |
|-------|-------|-------|------------|------------|
| 0 | Main | 0.1 | .claude/skills/ | - |
| 1 | Main | 1.1, 1.2, 1.3 | skill-creator/**, maestro-core/**, beads/ | 0 |
| A | Worker-A | 2.1, 2.2 | design/**, conductor/ | 1 |
| B | Worker-B | 2.3 | orchestrator/ | 1 |
| C | Worker-C | 2.4, 2.5, 2.6 | writing-skills/, sharing-skills/, using-git-worktrees/ | 1 |
| 3 | Main | 3.1 | All | A, B, C |

### Cross-Track Dependencies

- Tracks A, B, C depend on Phase 1 completion (maestro-core exists for Related links)
- Phase 3 waits for all parallel tracks

---

## Verification Commands

```bash
# Line counts
wc -l skills/*/SKILL.md

# Validation
python skills/skill-creator/scripts/quick_validate.py skills/beads
python skills/skill-creator/scripts/quick_validate.py skills/maestro-core
# ... repeat for all

# Link validation
grep -r "](references/" skills/ | head -20

# Symlink check
ls -la .claude/skills/
```

---

## Rollback Plan

If issues found:
1. Git restore individual SKILL.md files
2. Symlink can be reverted: `rm .claude/skills && git checkout .claude/skills/`

---

## Summary

| Phase | Epics | Tasks | Est. Time |
|-------|-------|-------|-----------|
| 0. Cleanup | 1 | 4 | 15 min |
| 1. Foundation | 3 | 19 | 1.5 hr |
| 2. Parallel | 6 | 30 | 3 hr |
| 3. Validation | 1 | 7 | 30 min |
| **Total** | **11** | **60** | **5 hr** |
