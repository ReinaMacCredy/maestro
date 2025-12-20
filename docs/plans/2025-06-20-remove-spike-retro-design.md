# Design: Remove Spike & Retro Workflows

**Date:** 2025-06-20  
**Goal:** Simplify plugin for external users by removing low-value complexity

## Decision

Remove `spike-workflow` and `retro-workflow` skills entirely. Keep `verification-before-completion` as the Reflect phase.

## Scope

### Delete Entirely
- `skills/spike-workflow/` directory
- `skills/retro-workflow/` directory
- References to `history/spikes/`, `history/retros/`, `history/threads/`, `history/memory-archive/`

### Keep
- `history/plans/` as the only history subdirectory

### Update References

| File | Change |
|------|--------|
| `README.md` | Remove spike/retro from triggers, skill tables, workflow diagram |
| `TUTORIAL.md` | Remove spike/retro from skill tables, examples, command refs |
| `skills/conductor/SKILL.md` | Reflect phase: `retro-workflow` → `verification-before-completion` |
| `skills/beads/references/WORKFLOWS.md` | Remove "Spike Investigation" pattern |
| `commands/file_beads.md` | Remove spike reference |
| `commands/decompose-task.md` | Remove spike reference |

## Updated Workflow Flow

```
Execute → verification-before-completion → bd close
```

No retrospective phase. Flow ends at close.

## Rationale

1. Spike functionality overlaps with brainstorming
2. Retro adds ceremony without proportional value
3. Verification provides the quality gate; retro was optional anyway
4. Reduces cognitive load for new users
