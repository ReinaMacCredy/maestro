# Plan: Codemaps Integration into Conductor

## Overview

Integrate codemaps functionality into the Conductor workflow across 4 phases.

## Epic 1: Core Infrastructure

Move templates and establish the foundation.

### Tasks

- [ ] **1.1** Move `skills/codemaps/references/CODEMAPS_TEMPLATE.md` to `skills/conductor/references/CODEMAPS_TEMPLATE.md`
- [ ] **1.2** Move demo `CODEMAPS/` directory from project root to `conductor/CODEMAPS/`
- [ ] **1.3** Create `.meta.json` schema in `conductor/CODEMAPS/.meta.json` with generation metadata
- [ ] **1.4** Update `skills/conductor/SKILL.md` to document CODEMAPS integration and `--skip-codemaps` flag

### Verification
- Template exists at new location
- Demo codemaps moved to conductor/
- .meta.json has valid structure

---

## Epic 2: Setup Integration

Add CODEMAPS generation to `/conductor-setup`.

### Tasks

- [ ] **2.1** Update `workflows/setup.md` to add Phase 7: CODEMAPS Generation
  - Analyze project structure
  - Generate overview.md (always)
  - Generate module codemaps for significant areas
  - Create .meta.json
  - Handle existing CODEMAPS/ (prompt to regenerate)
- [ ] **2.2** Add scale limits to setup workflow (2 levels, 50 files, 10 module maps)
- [ ] **2.3** Add monorepo detection and per-package codemap generation
- [ ] **2.4** Update output artifacts section in setup.md to include CODEMAPS/

### Verification
- Run `/conductor-setup` on test project
- Verify conductor/CODEMAPS/ created with overview.md
- Verify .meta.json contains valid metadata

---

## Epic 3: Finish & Refresh Integration

Add CODEMAPS regeneration to `/conductor-finish` and `/conductor-refresh`.

### Tasks

- [ ] **3.1** Update `skills/conductor/references/finish-workflow.md` to add CODEMAPS regeneration as Phase 5 (final step)
- [ ] **3.2** Add `--skip-codemaps` flag handling to finish workflow
- [ ] **3.3** Add user-modification check (compare file mtime to .meta.json generated timestamp)
- [ ] **3.4** Update `workflows/refresh.md` to add `codemaps` as a scope option
- [ ] **3.5** Add error handling for refresh without conductor/ directory

### Verification
- Run `/conductor-finish` and verify CODEMAPS regenerated
- Run `/conductor-finish --skip-codemaps` and verify skipped
- Run `/conductor-refresh` with codemaps scope

---

## Epic 4: Design Session Integration & Cleanup

Integrate with `ds` and delete old skill.

### Tasks

- [ ] **4.1** Update `skills/design/SKILL.md` to add CODEMAPS loading at session start
  - Check for conductor/CODEMAPS/
  - Load overview.md + relevant modules if exists
  - Display "📚 Loaded CODEMAPS for context"
  - Warn if missing, continue anyway
- [ ] **4.2** Delete `skills/codemaps/` directory
- [ ] **4.3** Update `README.md` to remove codemaps from Utilities section, add to Conductor output
- [ ] **4.4** Update `AGENTS.md` to remove codemaps from available skills list
- [ ] **4.5** Update `TUTORIAL.md` if codemaps is mentioned (search and update references)

### Verification
- Start `ds` with CODEMAPS present, verify loaded message
- Start `ds` without CODEMAPS, verify warning shown
- Verify `skills/codemaps/` deleted
- Verify documentation updated

---

## Summary

| Epic | Tasks | Focus |
|------|-------|-------|
| 1 | 4 | Core Infrastructure |
| 2 | 4 | Setup Integration |
| 3 | 5 | Finish & Refresh Integration |
| 4 | 5 | Design Session & Cleanup |
| **Total** | **18** | |

## Dependencies

```
Epic 1 → Epic 2 → Epic 3 → Epic 4
         ↓
    (can run 3 & 4 in parallel after 2)
```

## Verification Commands

```bash
# After Epic 1
ls skills/conductor/references/CODEMAPS_TEMPLATE.md
ls conductor/CODEMAPS/

# After Epic 2
# Run /conductor-setup on test project

# After Epic 3
# Run /conductor-finish on test track

# After Epic 4
ls skills/codemaps/  # Should fail (deleted)
grep -r "codemaps" README.md  # Should show conductor section only
```
