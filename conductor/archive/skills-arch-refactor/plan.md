# Skills Architecture Refactor - Implementation Plan

**Track ID:** skills-arch-refactor  
**Created:** 2026-01-08  
**Estimated Effort:** 2-3 sessions

---

## Epic Overview

Refactor Maestro skills for clear ownership, gerund naming, and flattened references.

---

## Tasks

### Wave 1: Foundation (Sequential)

#### T1: Update maestro-core routing table
**Effort:** S (30 min)  
**Files:**
- `skills/maestro-core/SKILL.md`
- `skills/maestro-core/references/routing-table.md`

**Steps:**
1. Update skill hierarchy to include new names
2. Update routing table with new triggers
3. Add ownership matrix section
4. Update Related Skills section

**Acceptance:** Routing table reflects designing, tracking, creating-skills

---

### Wave 2: Skill Renames (Parallel)

#### T2: Rename design → designing
**Effort:** M (45 min)  
**Depends:** T1  
**Files:**
- `skills/design/` → `skills/designing/`
- Update SKILL.md frontmatter (name: designing)
- Update all internal references

**Steps:**
1. `git mv skills/design skills/designing`
2. Update frontmatter: `name: designing`
3. Rename `unified-pipeline.md` → `pipeline.md`
4. Update internal links

**Acceptance:** Skill loads with `ds` trigger

---

#### T3: Update conductor (remove design phases)
**Effort:** M (45 min)  
**Depends:** T1  
**Files:**
- `skills/conductor/SKILL.md`
- `skills/conductor/references/`

**Steps:**
1. Remove design phase references
2. Keep only ci, setup, status, revise commands
3. Update to reference `designing` for design-related queries
4. Add pointer to `handoff` for finish/handoff commands

**Acceptance:** Conductor is ci-focused, no design overlap

---

#### T4: Rename beads → tracking
**Effort:** M (45 min)  
**Depends:** T1  
**Files:**
- `skills/beads/` → `skills/tracking/`
- Update SKILL.md frontmatter

**Steps:**
1. `git mv skills/beads skills/tracking`
2. Update frontmatter: `name: tracking`
3. Update description to emphasize persistent memory
4. Update internal references

**Acceptance:** `bd`, `fb`, `rb` triggers load tracking skill

---

#### T5: Merge skill-creator + writing-skills → creating-skills
**Effort:** M (1 hr)  
**Depends:** T1  
**Files:**
- `skills/skill-creator/` (delete after merge)
- `skills/writing-skills/` (delete after merge)
- `skills/creating-skills/` (new)

**Steps:**
1. Create `skills/creating-skills/`
2. Merge SKILL.md content (resolve conflicts)
3. Merge references/ directories
4. Unify description field guidelines
5. Delete old skill directories

**Acceptance:** Single skill for skill authoring, no conflicts

---

### Wave 3: Command Migration (Parallel)

#### T6: Move commands to correct owners
**Effort:** S (30 min)  
**Depends:** T2, T3  
**Files:**
- `skills/designing/SKILL.md` (add cn, newtrack, design)
- `skills/orchestrator/SKILL.md` (add orchestrate command)
- `skills/handoff/SKILL.md` (add finish, handoff commands)

**Steps:**
1. Add `/conductor-newtrack` docs to designing
2. Add `/conductor-design` as alias in designing
3. Add `/conductor-orchestrate` to orchestrator
4. Add `/conductor-finish`, `/conductor-handoff` to handoff
5. Update maestro-core routing table

**Acceptance:** Commands route to correct skills

---

#### T7: Flatten reference hierarchy
**Effort:** L (1.5 hr)  
**Depends:** T2, T3  
**Files:**
- `skills/designing/references/`
- `skills/conductor/references/`
- All cross-skill reference paths

**Steps:**
1. Move nested validation/ content to single files
2. Rename unified-pipeline.md → pipeline.md
3. Eliminate `../conductor/` references in designing
4. Eliminate `../design/` references in conductor
5. Update all internal links

**Acceptance:** `grep -r "../" skills/*/references/` returns only bmad/, agents/

---

### Wave 4: Documentation (Parallel)

#### T8: Update all cross-skill references
**Effort:** M (45 min)  
**Depends:** T4, T5  
**Files:**
- All SKILL.md files with cross-references
- Related Skills sections

**Steps:**
1. Find all `design` references → update to `designing`
2. Find all `beads` references → update to `tracking`
3. Find all `skill-creator`/`writing-skills` refs → update to `creating-skills`
4. Update Related Skills sections in all skills

**Acceptance:** No broken cross-skill links

---

#### T9: Update CODEMAPS
**Effort:** S (30 min)  
**Depends:** T8  
**Files:**
- `conductor/CODEMAPS/skills.md`
- `conductor/CODEMAPS/overview.md`

**Steps:**
1. Update skill names and hierarchy
2. Update file paths
3. Update command mappings
4. Verify accuracy

**Acceptance:** CODEMAPS reflects new structure

---

#### T10: Update AGENTS.md
**Effort:** S (30 min)  
**Depends:** T8  
**Files:**
- `AGENTS.md` (root)
- `conductor/AGENTS.md`

**Steps:**
1. Update skill references
2. Update command tables
3. Update trigger mappings
4. Add any new gotchas/patterns

**Acceptance:** AGENTS.md consistent with new skills

---

## Track Assignments

| Track | Agent | Tasks | Est. Time |
|-------|-------|-------|-----------|
| **Track A** | BlueLake | T1, T9, T10 | 1.5 hr |
| **Track B** | GreenCastle | T2, T7 | 2.25 hr |
| **Track C** | PurpleMountain | T3, T6 | 1.25 hr |
| **Track D** | OrangeRiver | T4, T5, T8 | 2.5 hr |

---

## Execution Order

```
Wave 1: T1 (foundation)
           ↓
Wave 2: T2 ─┬─ T3 ─┬─ T4 ─┬─ T5
            │      │      │
Wave 3: ────┴─ T6 ─┴─ T7 ─┘
                   │
Wave 4: ───────────┴─ T8 ─┬─ T9 ─┬─ T10
```

---

## Verification Checklist

- [ ] `ls skills/` shows: designing, conductor, orchestrator, tracking, handoff, creating-skills, maestro-core, using-git-worktrees, sharing-skills
- [ ] `wc -l skills/*/SKILL.md` all ≤500 lines
- [ ] `grep -r "design/" skills/` returns 0 (except designing/)
- [ ] `grep -r "beads/" skills/` returns 0 (except tracking/)
- [ ] `ds`, `ci`, `co`, `bd`, `ho` triggers work correctly
- [ ] CODEMAPS accurate
- [ ] AGENTS.md consistent
