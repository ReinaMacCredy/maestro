# Implement Workflow

## Purpose
Execute tasks from a track's plan following the defined workflow methodology (TDD, commits, documentation).

## Prerequisites
- Conductor environment initialized
- Required files exist:
  - `conductor/tech-stack.md`
  - `conductor/workflow.md`
  - `conductor/product.md`
- At least one track exists in `conductor/tracks/`

## State Management

### State File
`conductor/tracks/<track_id>/implement_state.json` (optional)

### State Values
| Field | Type | Description |
|-------|------|-------------|
| `current_phase` | string | Active phase name |
| `current_task` | string | Active task description |
| `status` | string | `in_progress`, `paused`, `blocked` |
| `last_commit_sha` | string | Last implementation commit |

## Workflow Steps

### Phase 1: Setup Verification

1. **Check Required Files**
   - Verify all prerequisite files exist
   - If missing: Halt with message to run `/conductor:setup`

### Phase 2: Track Selection

1. **Parse Tracks File**
   - Read `conductor/tracks.md`
   - Split by `---` separator
   - Extract: status, description, folder link

2. **Select Track**
   - **If track name provided**:
     - Case-insensitive match against descriptions
     - Confirm selection with user
   - **If no track provided**:
     - Find first non-`[x]` track
     - Auto-select and announce
   - **If all complete**:
     - Announce all tasks done, halt

### Phase 3: Track Implementation

1. **Update Status**
   - Change track status `[ ]` → `[~]` in `tracks.md`

2. **Load Context**
   - Read:
     - `conductor/tracks/<track_id>/plan.md`
     - `conductor/tracks/<track_id>/spec.md`
     - `conductor/workflow.md`

3. **Execute Tasks**
   - Iterate through `plan.md` tasks sequentially
   - For each task, defer to `workflow.md` Task Workflow section
   - Follow TDD cycle if defined:
     1. Mark task `[~]` in progress
     2. Write failing tests (Red)
     3. Implement to pass (Green)
     4. Refactor
     5. Verify coverage (>80%)
     6. Commit with conventional message
     7. Attach git note summary
     8. Update `plan.md`: `[~]` → `[x]` + SHA
     9. Commit plan update

4. **Phase Completion**
   - Execute Phase Completion Protocol from `workflow.md`
   - Includes: test verification, manual verification, checkpoint commit

5. **Finalize Track**
   - Update status `[~]` → `[x]` in `tracks.md`
   - Announce completion

### Phase 4: Documentation Sync

**Trigger**: Only when track reaches `[x]` status

1. **Load Context**
   - Read `spec.md` from completed track
   - Read project documents:
     - `conductor/product.md`
     - `conductor/product-guidelines.md`
     - `conductor/tech-stack.md`

2. **Analyze and Propose Updates**
   - **`product.md`**: If feature impacts product description
   - **`tech-stack.md`**: If technology changes detected
   - **`product-guidelines.md`**: Only for branding/voice changes (rare)

3. **Confirmation Loop**
   - Present proposed changes in diff format
   - Require explicit user approval
   - Apply only approved changes

4. **Report Summary**
   - List what was changed or not changed
   - Explain rationale

### Phase 5: Track Cleanup

1. **Present Options**
   ```
   A) Archive: Move to conductor/archive/
   B) Delete: Permanently remove
   C) Skip: Leave in tracks file
   ```

2. **Execute Choice**
   - **Archive**: Create archive dir, move track, update `tracks.md`
   - **Delete**: Confirm twice, remove, update `tracks.md`
   - **Skip**: No action

## Task Workflow Reference

From `workflow.md`:

1. Select task from `plan.md`
2. Mark `[~]` in progress
3. Write failing tests (Red)
4. Implement to pass (Green)
5. Refactor
6. Verify coverage
7. Document deviations if any
8. Commit code changes
9. Attach git note summary
10. Update `plan.md` with `[x]` and SHA
11. Commit plan update

## Error Handling

| Error | Action |
|-------|--------|
| Setup not complete | Halt, direct to `/conductor:setup` |
| No tracks found | Halt, direct to `/conductor:newTrack` |
| All tracks complete | Announce, halt |
| Test failure | Debug up to 2 attempts, then ask user |
| File read error | Stop, report error |
| Git conflict | Halt, provide resolution steps |

## Output Artifacts

```
conductor/
├── tracks.md (updated statuses)
├── product.md (possibly updated)
├── tech-stack.md (possibly updated)
├── archive/ (if archiving)
│   └── <track_id>/
└── tracks/
    └── <track_id>/
        ├── plan.md (tasks marked complete)
        └── implement_state.json (optional)
```

## Git Artifacts

- Implementation commits with conventional messages
- Plan update commits: `conductor(plan): Mark task 'X' as complete`
- Phase checkpoint commits: `conductor(checkpoint): Checkpoint end of Phase X`
- Git notes attached to commits with detailed summaries
