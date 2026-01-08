# Plan: Beads Consolidation

## Epic 1: Create Workflow Structure

### Task 1.1: Create workflows/beads/ directory
- [ ] `mkdir -p workflows/beads/references`

### Task 1.2: Copy workflow.md
- [ ] Copy `skills/beads/SKILL.md` → `workflows/beads/workflow.md`
- [ ] Strip YAML frontmatter (keep content only)

### Task 1.3: Move references/
- [ ] Move `skills/beads/references/*` → `workflows/beads/references/`
- [ ] Verify 8 files moved

### Task 1.4: Create FILE_BEADS.md
- [ ] Copy `skills/file-beads/SKILL.md` → `workflows/beads/references/FILE_BEADS.md`
- [ ] Strip YAML frontmatter

### Task 1.5: Create REVIEW_BEADS.md
- [ ] Copy `skills/review-beads/SKILL.md` → `workflows/beads/references/REVIEW_BEADS.md`
- [ ] Strip YAML frontmatter

### Task 1.6: Clean CLI_REFERENCE.md
- [ ] Remove lines 281-282 (broken "See also" block)
- [ ] Remove line 370 (broken CONFIG.md/TROUBLESHOOTING.md sentence)
- [ ] Remove line 403 (broken DAEMON.md reference)
- [ ] Remove lines 556-561 (entire broken "See Also" section)

**Verification:** `ls workflows/beads/references/ | wc -l` = 10

---

## Epic 2: Create Thin Skill

### Task 2.1: Create thin skill content
- [ ] Create new `skills/beads/SKILL.md` with thin stub content
- [ ] Include keyword-rich description
- [ ] Include Entry Points table with workflow paths
- [ ] Include Load Workflow instructions

**Content:**
```yaml
---
name: beads
version: "2.0.0"
description: "Beads issue tracking (bd, fb, rb). Use for multi-session work, file-beads, review-beads, filing beads from plan, reviewing beads."
---

# Beads

Issue tracking for multi-session work with dependency graphs.

## Entry Points

| Trigger | Workflow | Action |
|---------|----------|--------|
| `bd`, `beads` | `workflows/beads/workflow.md` | Core CLI operations |
| `fb`, `file-beads` | `workflows/beads/references/FILE_BEADS.md` | File beads from plan |
| `rb`, `review-beads` | `workflows/beads/references/REVIEW_BEADS.md` | Review filed beads |

## Load Workflow

1. Identify trigger from user input
2. Load corresponding workflow file (see table above)
3. Follow instructions in loaded file

## Quick Decision

- **Multi-session work?** → Use beads
- **Single-session linear task?** → Use TodoWrite
```

**Verification:** File is ~50 lines, contains all triggers in description

---

## Epic 3: Delete Old Skills

### Task 3.1: Delete skills/file-beads/
- [ ] `rm -rf skills/file-beads/`

### Task 3.2: Delete skills/review-beads/
- [ ] `rm -rf skills/review-beads/`

### Task 3.3: Delete skills/file-beads/
- [ ] `rm -rf skills/file-beads/`

### Task 3.4: Delete skills/review-beads/
- [ ] `rm -rf skills/review-beads/`

**Verification:** `ls skills/ | grep -E "file-beads|review-beads"` returns empty

---

## Epic 4: Update Documentation

### Task 4.1: Update commands/fb.md
- [ ] Line 7: `file-beads skill` → `beads skill`
- [ ] Line 10: `file-beads skill` → `beads skill`
- [ ] Line 26: `file-beads skill` → `beads skill`

### Task 4.2: Update commands/rb.md
- [ ] Line 7: `review-beads skill` → `beads skill`
- [ ] Line 10: `review-beads skill` → `beads skill`
- [ ] Line 26: `review-beads skill` → `beads skill`

### Task 4.3: Update commands/conductor/newTrack.toml
- [ ] Line 369: `file-beads skill` → `beads skill`
- [ ] Line 400: `review-beads skill` → `beads skill`

### Task 4.4: Update workflows/newtrack.md
- [ ] Line 196: `file-beads skill` → `beads skill`
- [ ] Line 206: `review-beads skill` → `beads skill`

### Task 4.5: Update workflows/README.md
- [ ] Line 29: `FB["fb (file-beads)"]` → `FB["fb"]`
- [ ] Line 30: `RB["rb (review-beads)"]` → `RB["rb"]`

### Task 4.6: Update skills/conductor/SKILL.md
- [ ] Line 347: `file-beads skill:` → `beads skill (fb):`
- [ ] Line 357: `before file-beads` → `before fb`

### Task 4.7: Update skills/design/SKILL.md
- [ ] Line 97: `FB["fb (file-beads)"]` → `FB["fb"]`
- [ ] Line 101: `RB["rb (review-beads)"]` → `RB["rb"]`

### Task 4.8: Update AGENTS.md
- [ ] Lines 99-100: Merge file-beads and review-beads rows into beads row

### Task 4.9: Update README.md
- [ ] Line 144: Remove separate file-beads, review-beads mentions
- [ ] Line 359: `FB["fb (file-beads)"]` → `FB["fb"]`
- [ ] Line 363: `RB["rb (review-beads)"]` → `RB["rb"]`
- [ ] Line 685: Update directory tree
- [ ] Line 710: Keep `rb (review-beads)` for clarity

### Task 4.10: Update CLAUDE.md
- [ ] Line 20: Update directory tree
- [ ] Lines 189-190: Merge table rows
- [ ] Line 209: Update trigger description

### Task 4.11: Update TUTORIAL.md
- [ ] Line 71: Update table
- [ ] Lines 175, 179: Update Mermaid
- [ ] Line 416: Keep `fb (file-beads)` for prose clarity
- [ ] Line 553: Update diagram
- [ ] Line 625: Update table
- [ ] Lines 719-720: Keep parenthetical
- [ ] Line 734: `file-beads skill` → `beads skill`
- [ ] Lines 984-985: Update mapping
- [ ] Line 1016: Keep parenthetical
- [ ] Lines 1215-1216: Update mapping

### Task 4.12: Update docs/PIPELINE_ARCHITECTURE.md
- [ ] Line 29: `FB["fb (file-beads)"]` → `FB["fb"]`
- [ ] Line 33: `RB["rb (review-beads)"]` → `RB["rb"]`
- [ ] Line 230: Update Mermaid
- [ ] Line 236: Update Mermaid

### Task 4.13: Update conductor/CODEMAPS/overview.md
- [ ] Lines 50-51: Merge table rows

### Task 4.14: Update conductor/CODEMAPS/skills.md
- [ ] Lines 34-35: Remove file-beads, review-beads from tree

**Verification:** 
```bash
grep -rn "file-beads\|review-beads" . \
  --include="*.md" --include="*.toml" \
  | grep -v "archive\|CHANGELOG\|track_progress.schema" \
  | wc -l
# Expected: 0 (or minimal in prose parentheticals)
```

---

## Epic 5: Verification

### Task 5.1: Test fb trigger
- [ ] Type `fb` in new session
- [ ] Verify beads skill loads
- [ ] Verify FILE_BEADS.md workflow executes

### Task 5.2: Test rb trigger
- [ ] Type `rb` in new session
- [ ] Verify beads skill loads
- [ ] Verify REVIEW_BEADS.md workflow executes

### Task 5.3: Test bd commands
- [ ] Run `bd ready`
- [ ] Run `bd list`
- [ ] Verify normal operation

### Task 5.4: Final grep verification
- [ ] Run verification grep command
- [ ] Confirm no unexpected old references

### Task 5.5: Commit changes
- [ ] `git add -A`
- [ ] `git commit -m "refactor(beads): consolidate beads, file-beads, review-beads into unified skill + workflow"`

---

## Summary

| Epic | Tasks | Est. Time |
|------|-------|-----------|
| 1. Create Workflow Structure | 6 | 20 min |
| 2. Create Thin Skill | 1 | 5 min |
| 3. Delete Old Skills | 4 | 5 min |
| 4. Update Documentation | 14 | 45 min |
| 5. Verification | 5 | 15 min |
| **Total** | **30** | **~1.5 hours** |
