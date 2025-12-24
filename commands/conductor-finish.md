---
description: Complete a track - extract learnings, refresh context, and archive
---

# Conductor Finish

Complete a track by extracting learnings, compacting beads, refreshing context documents, and archiving.

## Usage

```
/conductor-finish <track_id>
/conductor-finish              # uses active track
```

## Flags

| Flag | Description |
|------|-------------|
| `--with-pr` | Chain to finish-branch skill after completion |
| `--skip-codemaps` | Skip CODEMAPS regeneration (Phase 6) |
| `--skip-refresh` | Skip Context Refresh (Phase 4) |

## Workflow Phases (0-6)

### Phase 0: Validation Pre-Flight
1. Check for existing `finish-state.json` (resume detection)
2. Validate track integrity (spec.md, plan.md exist)
3. Check for open beads/incomplete epics → warn, don't block
4. Create state file if not resuming

### Phase 1: Thread Compaction
1. Discover threads via: beads comments → metadata.json → find_thread
2. Extract learnings using `read_thread`
3. Write `conductor/tracks/<id>/LEARNINGS.md`
4. Smart skip if no threads found

### Phase 2: Beads Compaction
1. Find candidates: `bd compact --analyze --json`
2. Generate AI summaries for closed issues
3. Apply: `bd compact --apply --id <id> --summary "<text>"`
4. Smart skip if no candidates

### Phase 3: Knowledge Merge
1. Parse LEARNINGS.md (Commands, Gotchas, Patterns)
2. Dedupe against existing `conductor/AGENTS.md`
3. Merge new learnings (show diff for review)
4. **Required phase** - stops workflow on failure

### Phase 4: Context Refresh
*Skip if `--skip-refresh` flag provided*

| Document | Action |
|----------|--------|
| `product.md` | Add shipped feature from spec.md |
| `tech-stack.md` | Detect new deps, prompt to add |
| `tracks.md` | Move entry to Completed section |
| `workflow.md` | Detect CI/CD changes, prompt to update |

### Phase 5: Archive
1. Prompt user: **[A]** Archive or **[K]** Keep
2. If Archive: move to `conductor/archive/`
3. Update metadata.json with docSync record
4. Commit all changes
5. Cleanup beads if count > 150

### Phase 6: CODEMAPS Regeneration
*Skip if `--skip-codemaps` flag or no CODEMAPS exist*

1. Check for user-modified files → warn before overwriting
2. Regenerate overview.md and module codemaps
3. Update `.meta.json`

## Resume Capability

State file `finish-state.json` enables resuming interrupted runs:
```
Previous run interrupted at Phase X. Resume? [Y/n]
```

State file deleted on successful completion.

## Error Handling

| Phase | On Failure | Action |
|-------|------------|--------|
| Phase 1 | Thread read fails | Log warning, skip thread, continue |
| Phase 2 | bd compact fails | Log warning, skip issue, continue |
| Phase 3 | Merge fails | **Stop workflow** |
| Phase 4 | File update fails | Report error, suggest manual fix |
| Phase 5 | Commit fails | Report error, suggest manual fix |
| Phase 6 | Regeneration fails | Log warning, continue to completion |

## See Also

- [Full workflow reference](../skills/conductor/references/finish-workflow.md)
- [finish_state.schema.json](../workflows/schemas/finish_state.schema.json)
