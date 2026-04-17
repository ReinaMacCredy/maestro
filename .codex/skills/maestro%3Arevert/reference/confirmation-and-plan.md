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

**Safety measures**:
- Backup tag: `pre-revert-{timestamp}` at `{current_sha7}`
- Strategy: `git revert` (additive, history preserved)
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

### Danger Zone Confirmation (when destructive operations are involved)

If the revert workflow involves any destructive operation (git reset, force push, branch delete), replace the standard Confirmation 2 with:

```
[!] This revert involves a destructive operation: {operation}

What will be lost:
{bullet list -- see reference/danger-zones.md for per-operation details}

Safety measures in place:
- Backup tag: {tag_name}
- {other measures}

To proceed, type: "{confirmation phrase}"
To cancel: "cancel"
```

The confirmation phrase must be specific to the operation (not just "yes"):
- `git reset --hard`: "I understand this destroys uncommitted work"
- `git push --force`: "I confirm this branch has no other contributors"
- `git branch -D`: "I accept the deletion of branch {name}"

---

## Rollback-of-Rollback Confirmation

When the user wants to undo a previous revert, present this confirmation:

```
## Undo Revert

**Original revert scope**: {scope} of track '{description}'
**Revert commits to undo**: {list}
**Strategy**: {revert-the-revert | cherry-pick originals | reset to backup tag}

This will re-apply the original changes that were previously reverted.

After this operation:
- Plan state will be updated: `[ ]` --> `[x] {sha}` for re-applied tasks
- Track status will be restored
- Tests will be re-run

Proceed with undo? (yes/no)
```

---

## Step 11: Summary

```
## Revert Complete

**Scope**: {track | phase N | task name}
**Track**: {track_description}
**Commits reverted**: {count} ({impl_count} implementation, {plan_count} plan-update, {track_count} track creation)
**Duplicates removed**: {dedup_count} cherry-pick duplicates excluded
**Tests**: {pass | fail}
**Backup tag**: pre-revert-{timestamp}

**Plan state updated**: {N} tasks reset to `[ ]`

**Next**:
- `/maestro:implement {track_id}` -- Re-implement reverted tasks
- `/maestro:status` -- Check overall progress
- To undo this revert: `git revert --no-edit {revert_sha}` (or see Rollback-of-Rollback in SKILL.md)
```
