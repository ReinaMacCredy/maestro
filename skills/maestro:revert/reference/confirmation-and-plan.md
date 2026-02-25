# Confirmation and Execution Plan

## Step 5: Present Execution Plan

Show the user exactly what will be reverted:

```
## Revert Plan

**Scope**: {track | phase N | task name}
**Track**: {track_description} ({track_id})

**Commits to revert** (reverse chronological order):
1. `{sha7}` -- {commit message}
2. `{sha7}` -- {commit message} [plan-update]
3. `{sha7}` -- {commit message} [track creation]

**Affected files**:
{list of files changed by these commits}

**Plan updates**:
- {task_name}: `[x] {sha}` --> `[ ]`
- {task_name}: `[x] {sha}` --> `[ ]`
```

Use `[plan-update]` and `[track creation]` labels to distinguish commit types from implementation commits.

---

## Step 6: Multi-Step Confirmation

**Confirmation 1** -- Target:

Ask the user: "Revert {scope} of track '{description}'? This will undo {N} commits."
Options:
- **Yes, continue** -- Show me the execution plan
- **Cancel** -- Abort revert

**Confirmation 2** -- Final go/no-go:

Ask the user: "Ready to execute? This will create revert commits (original commits are preserved in history)."
Options:
- **Execute revert** -- Create revert commits now
- **Revise plan** -- Modify which commits to include or exclude before executing
- **Cancel** -- Abort

If user selects "Revise plan":
- Display the numbered commit list again
- Ask the user: "Enter commit numbers to exclude (e.g. '2,3'), or leave blank to keep all:"
- Remove the specified commits from the list
- Re-display the updated plan and return to Confirmation 2

---

## Step 11: Summary

```
## Revert Complete

**Scope**: {track | phase N | task name}
**Track**: {track_description}
**Commits reverted**: {count} ({impl_count} implementation, {plan_count} plan-update, {track_count} track creation)
**Duplicates removed**: {dedup_count} cherry-pick duplicates excluded
**Tests**: {pass | fail}

**Plan state updated**: {N} tasks reset to `[ ]`

**Next**:
- `/maestro:implement {track_id}` -- Re-implement reverted tasks
- `/maestro:status` -- Check overall progress
```
