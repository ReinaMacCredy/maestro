# Design: Merge knowlegde/ into doc-sync

## Problem Statement

Two separate documentation systems exist:
1. `knowlegde/` - Extracts knowledge from Amp threads
2. `doc-sync/` - Scans code changes and updates docs

These should be unified into a single pipeline in Phase 7 of `/conductor-finish`.

## Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Approach | Merge into doc-sync | Single pipeline, no skill-to-skill calls |
| Thread trigger | Always | Never miss insights, +30-60s acceptable |
| Merge strategy | Oracle reconcile | AI decides conflicts per-case |
| knowlegde/ fate | Delete | Content migrated, skill removed |

## Architecture

```
/conductor-finish
    └── Phase 7: doc-sync
        ├── 1. Code Scanner (existing)
        │   └── scanner.md, detector.md
        ├── 2. Thread Extractor (NEW)
        │   └── extraction.md ← from knowlegde/
        ├── 3. Oracle Reconcile (NEW)
        │   └── reconcile.md
        └── 4. Apply Updates (existing)
            └── updater.md
```

## File Changes

### Create
- `skills/conductor/references/doc-sync/extraction.md` - Thread extraction pipeline
- `skills/conductor/references/doc-sync/reconcile.md` - Oracle merge logic

### Migrate
- `knowlegde/reference/doc-mapping.md` → `doc-sync/mapping.md`
- `knowlegde/reference/extraction-prompts.md` → `doc-sync/prompts.md`

### Modify
- `skills/conductor/references/doc-sync/integration.md` - Add thread extraction step

### Delete
- `knowlegde/` directory (after migration complete)

## Thread Extraction Pipeline

```
1. find_thread after:{track_start_date} file:{track_files}
2. Spawn parallel Task agents (2-3 threads each)
3. Each Task: read_thread → extract topics JSON
4. Oracle synthesizes all extractions
5. Return unified topic list
```

## Oracle Reconcile Logic

```
Input:
- CODE_CHANGES: [{file, change_type, summary}]
- THREAD_TOPICS: [{name, summary, decisions, patterns}]
- CURRENT_DOCS: [{file, content}]

Output:
- UPDATES: [{file, section, action, content, rationale}]
```

## Success Criteria

- [ ] Thread extraction runs on every `/conductor-finish`
- [ ] Code changes and thread topics merged by Oracle
- [ ] knowlegde/ skill deleted
- [ ] No broken references
- [ ] Doc-sync integration.md updated
