# Learnings: Ralph Integration into Maestro

Track: ralph-integration_20260108  
Completed: 2026-01-08

## Summary

Integrated Ralph (autonomous AI agent loop from snarktank/ralph) as third execution mode (`ca`) in Maestro Conductor, alongside `ci` (implement) and `co` (orchestrate).

## Key Decisions

1. **Native Integration**: Ralph reads/writes directly to `metadata.json.ralph` section (no separate prd.json)
2. **Toolbox Location**: Files in `toolboxes/ralph/` (ralph.sh, prompt.md, README.md)
3. **Exclusive Lock**: `ralph.active` flag prevents concurrent `ci`/`co` execution
4. **Story Structure**: Each story has `title`, `acceptanceCriteria`, `passes`, `notes`

## Architecture

```
ds → design.md → spec.md + plan.md → {
  ci/co (beads) - standard execution
  OR
  metadata.ralph → ca (Ralph loop)
}
```

## Commands Added

| Command | Purpose |
|---------|---------|
| `ca` | Start Ralph autonomous execution |
| `/conductor-autonomous` | Alias for ca |

## Gotchas Discovered

- `ralph.active` lock prevents concurrent `ci`/`co` execution
- `progress.txt` is in track directory, not project root
- Ralph reads/writes `metadata.json.ralph.stories` directly
- Stories must be populated by `ds` before `ca` is available
- Max iterations default is 10, configurable via second argument

## Files Modified

- `skills/conductor/SKILL.md` - Added ca entry point
- `skills/maestro-core/references/routing-table.md` - Added ca routing
- `skills/conductor/references/workflows/autonomous.md` - New workflow doc
- `skills/conductor/references/schemas/metadata.schema.json` - Added ralph section
- `toolboxes/ralph/ralph.sh` - Adapted for Maestro
- `toolboxes/ralph/prompt.md` - Maestro conventions
- `toolboxes/ralph/README.md` - Usage docs
- `AGENTS.md` - ca commands and gotchas
- `conductor/AGENTS.md` - Ralph learnings
