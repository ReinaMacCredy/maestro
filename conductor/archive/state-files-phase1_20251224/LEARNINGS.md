# LEARNINGS: state-files-phase1_20251224

## Summary
Moved state file creation to Phase 1.3 of /conductor-newtrack, ensuring all 3 state files (metadata.json, .track-progress.json, .fb-progress.json) exist BEFORE spec/plan generation.

## Key Learnings

### Patterns
- **State Files First:** Create metadata.json, .track-progress.json, .fb-progress.json in Phase 1.3 BEFORE any other operations
- **Collective State Validation:** Treat 3 state files as atomic unit - HAS_STATE = 0 (none), 1 (partial), 2 (all)
- **DIAGNOSE_MODE:** Parse --diagnose flag early (Step 0.1) to enable report-only validation

### Gotchas
- State files with partial presence (1 or 2 files) trigger auto-creation of missing files
- Pre-checks for auto-create: both spec.md and plan.md have content, are <30 days old, and have matching track_id headers
- Atomic write pattern: use `$FILE.tmp.$$` then `mv` to prevent corruption

## Linked Commits
- e9f7635 feat(conductor): add track validation system with centralized checks
- 3db567b feat(conductor): move state file creation to phase 1 of newTrack workflow
