# Plan: Merge knowlegde/ into doc-sync

## Epic 1: Merge knowlegde/ into doc-sync

### Wave 1 (Parallel)
- [ ] **1.1** Create `extraction.md` - Thread extraction pipeline
  - File: `.claude/skills/conductor/references/doc-sync/extraction.md`
  - Source: `knowlegde/SKILL.md` phases 1-3
  
- [ ] **1.2** Create `reconcile.md` - Oracle merge logic
  - File: `.claude/skills/conductor/references/doc-sync/reconcile.md`
  - Source: `knowlegde/SKILL.md` phases 4-6
  
- [ ] **1.3** Migrate `doc-mapping.md`
  - From: `knowlegde/reference/doc-mapping.md`
  - To: `.claude/skills/conductor/references/doc-sync/mapping.md`
  
- [ ] **1.4** Migrate `extraction-prompts.md`
  - From: `knowlegde/reference/extraction-prompts.md`
  - To: `.claude/skills/conductor/references/doc-sync/prompts.md`

### Wave 2 (Sequential)
- [ ] **1.5** Update `integration.md` - Add thread extraction step
  - File: `.claude/skills/conductor/references/doc-sync/integration.md`
  - Depends: 1.1, 1.2, 1.3, 1.4

### Wave 3 (Sequential)
- [ ] **1.6** Delete `knowlegde/` directory
  - Files: `knowlegde/SKILL.md`, `knowlegde/reference/*`
  - Depends: 1.5

### Wave 4 (Sequential)
- [ ] **1.7** Verify no broken references
  - Run: `./scripts/validate-links.sh .`
  - Depends: 1.6

## Track Assignments

| Track | Agent | Beads | File Scope |
|-------|-------|-------|------------|
| A | TBD | 1.1, 1.2, 1.5 | doc-sync/*.md (new logic) |
| B | TBD | 1.3, 1.4 | doc-sync/*.md (migrations) |
| C | TBD | 1.6, 1.7 | knowlegde/ (cleanup) |

## Dependencies

```
1.1 ─┐
1.2 ─┼─→ 1.5 → 1.6 → 1.7
1.3 ─┤
1.4 ─┘
```

## Notes

- Wave 1 tasks are fully parallel (different files)
- Wave 2+ must be sequential (integration depends on all wave 1)
- Cleanup (1.6, 1.7) only after integration verified
