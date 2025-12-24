# LEARNINGS: finish-phase4-revision_20251224

## Summary
Revised /conductor-finish from 5-phase to 6-phase workflow by adding Phase 4 (Context Refresh) and simplifying archive options from S/H/K to A/K.

## Key Learnings

### Commands
- `bd compact --analyze --json` - Find beads needing summaries
- `bd compact --apply --id <id> --summary "<text>"` - Apply AI summary

### Patterns
- **6-Phase Finish Workflow:** Pre-flight → Thread Compaction → Beads Compaction → Knowledge Merge → Context Refresh → Archive → CODEMAPS
- **A/K Archive Choice:** Archive (move to archive/) / Keep (stay active) - simplified from S/H/K
- **Context Refresh in Finish:** product.md, tech-stack.md, tracks.md, workflow.md updated during finish, not separate command

### Gotchas
- Phase 4 (Context Refresh) can be skipped with --skip-refresh flag
- User input A/K must be mapped to JSON values "archive"/"keep"
- Atomic writes use $$ (PID) suffix for temp files to prevent collisions

## Linked Commits
- e9f7635 feat(conductor): add track validation system with centralized checks
- db5e4ce feat(conductor): integrate codemaps into conductor workflow
