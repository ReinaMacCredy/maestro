# LEARNINGS: merge-newtrack-fb_20251223

## Summary
Merged file-beads (fb) functionality into /conductor-newtrack as a unified workflow, eliminating the need for separate fb command after track creation.

## Key Learnings

### Patterns
- **Unified Track Creation:** /conductor-newtrack now includes spec generation, plan generation, beads filing, AND review in one flow
- **Parallel Subagent Dispatch:** file-beads and review-beads can run as parallel subagents for efficiency
- **Cross-Epic Validation:** review-beads validates dependencies and blockers across epics

### Commands
- `fb <track_id>` - Standalone beads filing (still available for legacy/repair)
- `rb <track_id>` - Review beads for a track

### Gotchas
- --no-beads / -nb flag skips beads filing (spec + plan only)
- --plan-only / -po is an alias for --no-beads
- Track must have spec.md and plan.md before fb can run

## Linked Commits
- 49edb49 feat: merge /conductor-newtrack and fb into unified flow
- 3db567b feat(conductor): move state file creation to phase 1 of newTrack workflow
