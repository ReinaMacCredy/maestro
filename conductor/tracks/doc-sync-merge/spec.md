# Spec: Merge knowlegde/ into doc-sync

## Summary

Merge the standalone `knowlegde/` skill into `doc-sync` to create a unified documentation pipeline that extracts knowledge from both code changes AND Amp threads.

## Requirements

### Functional
- Thread extraction runs on every `/conductor-finish` (Phase 7)
- Code changes and thread topics merged by Oracle
- `knowlegde/` skill deleted after migration

### Non-Functional
- +30-60s acceptable overhead per finish
- No broken references after deletion

## Success Criteria

- [ ] `doc-sync/extraction.md` exists with thread pipeline
- [ ] `doc-sync/reconcile.md` exists with Oracle merge logic
- [ ] `doc-sync/mapping.md` migrated from knowlegde/
- [ ] `doc-sync/prompts.md` migrated from knowlegde/
- [ ] `doc-sync/integration.md` updated with thread step
- [ ] `knowlegde/` directory deleted
- [ ] No broken links in codebase
