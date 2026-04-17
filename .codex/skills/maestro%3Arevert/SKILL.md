---
name: maestro:revert
description: "Git-aware revert of track, phase, or individual task. Safely undoes implementation with plan state rollback."
argument-hint: "<track> [--phase <N>] [--task <name>]"
---

# Revert -- Git-Aware Undo

## Overview

Revert undoes implementation work at track, phase, or task granularity. It creates NEW revert commits -- original history is never destroyed. After reverting, maestro plan state is rolled back so tasks can be re-implemented.

**Core principle:** `git revert` is additive. It records the undo as new history. The original commits remain reachable via reflog and `git log`. This is the safe default.

**If you are thinking about `git reset --hard`, stop.** Read the Danger Zones section first. There are almost always safer alternatives.

## When to Use

| Situation | Scope | Command |
|-----------|-------|---------|
| Task produced wrong result | `--task <name>` | Revert that task's commits |
| Phase approach was wrong | `--phase <N>` | Revert all commits in phase N |
| Track needs complete restart | No scope flag | Revert entire track |
| Single bad commit (known SHA) | Manual | `git revert --no-edit <sha>` |

**Exceptions (ask your human partner):**
- Commits contain secrets or credentials (need history rewrite, not revert)
- Revert would create unresolvable conflicts (consider branch recreation)
- Work was never pushed (local-only reset may be simpler)

## Decision Tree: revert vs reset vs branch recreation

**Start here:** Have the commits been pushed to a remote that others pull from?

```
Commits pushed to shared remote?
  |
  +-- YES --> Use `git revert` (ALWAYS)
  |           Creates new commits that undo the old ones.
  |           Safe for shared history. No force push needed.
  |
  +-- NO (local only) --> How clean do you need it?
        |
        +-- Keep changes staged --> `git reset --soft <target>`
        |   Files stay in index. You can re-commit differently.
        |
        +-- Keep changes unstaged --> `git reset --mixed <target>` (default)
        |   Files in working tree, not staged. Review before re-committing.
        |
        +-- Destroy changes completely --> `git reset --hard <target>`
        |   [DESTRUCTIVE] Working tree matches target. Changes are GONE.
        |   Requires explicit user confirmation. See Danger Zones.
        |
        +-- History is tangled beyond repair --> Branch recreation
            Create new branch from known-good point, cherry-pick what to keep.
            See reference/git-operations.md for branch recreation protocol.
```

**Decision summary:**

| Strategy | Pushed? | Preserves history? | Risk level |
|----------|---------|-------------------|------------|
| `git revert` | Yes or No | Yes (additive) | Safe |
| `git reset --soft` | No only | Partial (moves HEAD) | Low |
| `git reset --mixed` | No only | Partial (moves HEAD) | Low |
| `git reset --hard` | No only | No (destroys changes) | DESTRUCTIVE |
| Branch recreation | Either | Yes (new branch) | Complex |

**Default: always use `git revert` unless you have a specific reason not to.** The other strategies exist for edge cases, not convenience.

## Safety Pre-Checks (MANDATORY)

Before ANY revert operation, run these checks. Do not skip them.

### 1. Clean worktree

```bash
git status --porcelain
```

If output is non-empty: STOP. Uncommitted changes will complicate the revert.
- Stash them: `git stash push -m "pre-revert backup"`
- Or commit them: `git add -A && git commit -m "wip: save before revert"`
- Then proceed.

### 2. Verify branch

```bash
git branch --show-current
```

Confirm you are on the branch where the revert should happen. Reverting on the wrong branch is recoverable but messy.

### 3. Check remote state

```bash
git log --oneline origin/$(git branch --show-current)..HEAD 2>/dev/null
```

If this shows commits: you have local-only work. A `git reset` might be appropriate (see Decision Tree).
If this shows nothing: all commits are pushed. Use `git revert` only.

If the remote tracking branch does not exist, treat all commits as local-only but confirm with the user.

### 4. Create backup tag

```bash
git tag pre-revert-$(date +%Y%m%d-%H%M%S)
```

This lightweight tag marks the current HEAD. If anything goes wrong, you can return here:
```bash
git reset --hard pre-revert-<timestamp>
```

### 5. Verify no in-progress operations

```bash
test -d .git/rebase-merge -o -d .git/rebase-apply && echo "REBASE IN PROGRESS"
test -f .git/MERGE_HEAD && echo "MERGE IN PROGRESS"
test -f .git/CHERRY_PICK_HEAD && echo "CHERRY-PICK IN PROGRESS"
```

If any operation is in progress: STOP. Complete or abort it first.

See `reference/safety-checks.md` for extended pre-flight validation (submodules, CI state, stash management).

## Arguments

`$ARGUMENTS`

- `<track>`: Track name or ID (optional -- if omitted, enter Guided Selection)
- `--phase <N>`: Revert only phase N (optional)
- `--task <name>`: Revert only a specific task (optional)
- No scope flag: revert the entire track

---

## Step-by-Step Workflow

### Step 1: Parse Target Scope

Determine what to revert:
- **Track-level**: No `--phase` or `--task` flag. Revert all commits in the track.
- **Phase-level**: `--phase N` specified. Revert commits from phase N only.
- **Task-level**: `--task <name>` specified. Revert a single task's commit(s).

If no `<track>` argument, proceed to Guided Selection:
1. Read `.maestro/tracks.md` and recent git history: `git log --oneline --since="7 days ago" --grep="maestro"`
2. Present a menu grouped by track: ID, description, status, completed task count
3. If user provides a custom track ID, use that

### Step 2: Locate Track

Match track argument against IDs and descriptions in `.maestro/tracks.md`. If not found: report and stop.

Read the track's `plan.md` and `metadata.json` to understand structure.

### Step 3: Resolve Commit SHAs

**Goal:** Build the complete list of commits to revert for the target scope.

**BR-enhanced path** (if `metadata.json` has `beads_epic_id`):
```bash
br list --status closed --parent {epic_id} --all --json
```
Parse `close_reason` for SHAs (format: `sha:{7char}`). Scope by labels for `--phase`/`--task`.

**Legacy path**: Read `plan.md`, extract `[x] {sha}` markers from the appropriate scope:
- **Track**: All `[x] {sha}` markers
- **Phase N**: Only markers under `## Phase N`
- **Task**: Only the marker for the matching task

**Plan-update commits** (always check):
```bash
git log --oneline --all --grep="maestro(plan): mark task" -- .maestro/tracks/{track_id}/plan.md
```

**Track creation commit** (track-level revert only):
```bash
git log --oneline --all --grep="chore(maestro:new-track): add track {track_id}"
```

If no SHAs found in scope: report "No completed tasks found in the specified scope. Nothing to revert." and stop.

See `reference/git-operations.md` for the full SHA resolution protocol with edge cases.

### Step 4: Git Reconciliation

For each SHA, verify it exists:
```bash
git cat-file -t {sha}
```

**Missing SHA** (rebased/squashed/force-pushed):
```bash
# Try to find replacement by commit message
git log --all --oneline --grep="{original commit message}"
```
If found: offer replacement. If not: skip and warn.

**Merge commit detection:**
```bash
git cat-file -p {sha}  # Check for multiple "parent" lines
```
If merge commit found, ask user: Proceed with `-m 1`, skip merge commits, or cancel.

**Cherry-pick duplicate detection:** Compare commit messages. For identical subjects, compare patches. Remove older duplicate from revert list.

See `reference/git-operations.md` for full reconciliation protocol and edge cases.

### Step 5: Present Execution Plan

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

**Safety**: Backup tag created at `pre-revert-{timestamp}`
```

### Step 6: Confirm

**Confirmation 1** -- Target:
"Revert {scope} of track '{description}'? This will undo {N} commits."
- **Yes, continue**
- **Cancel**

**Confirmation 2** -- Final:
"Ready to execute? This will create revert commits (original commits are preserved in history)."
- **Execute revert**
- **Revise plan** (exclude specific commits)
- **Cancel**

See `reference/confirmation-and-plan.md` for the revision loop and summary format.

### Step 7: Execute Reverts

Revert in **reverse chronological order** (newest first):

```bash
# Standard commits
git revert --no-edit {sha_newest}
git revert --no-edit {sha_next}
# ...continue for each SHA

# Merge commits (if user approved)
git revert --no-edit -m 1 {merge_sha}
```

**CRITICAL:** Validate each `git revert` succeeds before continuing to the next.

**On conflict:**
1. Report: "Merge conflict during revert of {sha}."
2. Show conflicting files: `git diff --name-only --diff-filter=U`
3. Ask user:
   - **Help me resolve** -- Show conflict markers and guide resolution
   - **Abort remaining** -- Stop here (already-reverted commits stay)
   - **Accept theirs** -- Keep current version for conflicting files: `git checkout --theirs {file} && git add {file}`

After resolving: `git revert --continue`

### Steps 8-10: Update Plan State, Registry, Verify

**Plan state** -- Edit `plan.md`: change `[x] {sha}` back to `[ ]` for each reverted task.

```bash
git add .maestro/tracks/{track_id}/plan.md
git commit -m "maestro(revert): update plan state for reverted {scope}"
```

**BR mirror** (if `beads_epic_id` exists):
```bash
br update {issue_id} --status open --json
```

**Registry** (track-level revert only): Update `.maestro/tracks.md` status from `[x]`/`[~]` to `[ ]`. Update `metadata.json` status to `"new"`.

**Verify:**
```bash
CI=true {test_command}
```
Report pass/fail. If tests fail: warn user and offer to debug.

### Step 11: Summary

```
## Revert Complete

**Scope**: {scope}
**Track**: {track_description}
**Commits reverted**: {count} ({impl} implementation, {plan} plan-update, {track} track creation)
**Tests**: {pass | fail}
**Backup tag**: pre-revert-{timestamp}

**Plan state updated**: {N} tasks reset to `[ ]`

**Next**:
- `/maestro:implement {track_id}` -- Re-implement reverted tasks
- `/maestro:status` -- Check overall progress
- To undo this revert: see Rollback-of-Rollback below
```

---

## Concrete Examples

### Example 1: Task-Level Revert (simplest)

A single task "add validation" needs to be undone.

```bash
# Safety pre-checks
git status --porcelain              # Must be empty
git branch --show-current           # Confirm correct branch
git tag pre-revert-20250315-1430    # Backup

# Resolve: plan.md shows [x] a1b2c3d for "add validation"
# Reconcile
git cat-file -t a1b2c3d            # Verify exists

# Execute
git revert --no-edit a1b2c3d

# Update plan state: [x] a1b2c3d --> [ ] in plan.md
git add .maestro/tracks/auth/plan.md
git commit -m "maestro(revert): update plan state for reverted task 'add validation'"

# Verify
bun test
```

### Example 2: Phase-Level Revert (multiple commits)

Phase 2 of the "payments" track had 3 completed tasks. All must be undone.

```bash
# Safety pre-checks (same as above)
git tag pre-revert-20250315-1445

# Resolve: phase 2 has 3 completed tasks
# SHAs: d4e5f6g (newest), b2c3d4e, f7g8h9i (oldest)

# Execute in reverse chronological order
git revert --no-edit d4e5f6g
git revert --no-edit b2c3d4e
git revert --no-edit f7g8h9i

# Update plan state for all 3 tasks
# Edit plan.md: [x] --> [ ] for each
git add .maestro/tracks/payments/plan.md
git commit -m "maestro(revert): update plan state for reverted phase 2"

bun test
```

### Example 3: Track-Level Revert (full teardown)

The entire "notifications" track needs to be undone, including track creation.

```bash
# Safety pre-checks
git tag pre-revert-20250315-1500

# Resolve: 5 implementation commits + 2 plan-update commits + 1 track creation
# Order by date, newest first

# Execute all reverts
git revert --no-edit {sha1}  # newest implementation
git revert --no-edit {sha2}
git revert --no-edit {sha3}
git revert --no-edit {sha4}
git revert --no-edit {sha5}  # oldest implementation
git revert --no-edit {sha6}  # plan-update
git revert --no-edit {sha7}  # plan-update
git revert --no-edit {sha8}  # track creation

# Plan state: all tasks reset to [ ]
# Registry: track status [ ] in tracks.md, metadata status "new"
git add .maestro/tracks/ .maestro/tracks.md
git commit -m "maestro(revert): reset track 'notifications' fully"

bun test
```

---

## Danger Zones

Operations that destroy data or rewrite history. Each requires explicit user confirmation before execution.

| Operation | What it does | Destructive? | Confirmation required |
|-----------|-------------|-------------|----------------------|
| `git revert` | Creates new undo commit | No (additive) | Standard 2-phase |
| `git reset --soft` | Moves HEAD, keeps staged | Moderate | "Confirm local-only reset" |
| `git reset --mixed` | Moves HEAD, unstages | Moderate | "Confirm local-only reset" |
| `git reset --hard` | Moves HEAD, destroys working tree | **YES** | "I understand this destroys uncommitted work" |
| `git push --force` | Overwrites remote history | **YES** | Double confirm + backup tag verified |
| `git branch -D` | Deletes branch (even unmerged) | **YES** | Confirm branch is not needed |

### Rules for destructive operations

1. **Never `git reset --hard` without a backup tag.** The tag must exist BEFORE the reset.
2. **Never `git push --force` to a shared branch.** If others have pulled, their history diverges.
3. **Never delete a branch without verifying it is merged** (or confirming the user accepts the loss).
4. **Reflog is your safety net** -- but it expires (default: 90 days for reachable, 30 days for unreachable). Do not rely on reflog for long-term recovery.
5. **If in doubt, use `git revert`.** It is always safe. The "ugly" revert commits in history are a small price for safety.

### Recovery from destructive operations

```bash
# Find lost commits via reflog
git reflog --all | head -20

# Recover to a reflog entry
git reset --hard HEAD@{N}

# Recover a deleted branch
git reflog | grep "checkout: moving from {branch}"
git checkout -b {branch} {sha_from_reflog}
```

See `reference/danger-zones.md` for the full destructive operations catalog with confirmation dialog templates and recovery procedures for each operation.

---

## Rollback-of-Rollback

Sometimes you revert and then realize the revert was wrong. Here is how to undo a revert.

### Strategy 1: Revert the revert commit (safest)

```bash
# Find the revert commit(s)
git log --oneline --grep="Revert" | head -5

# Revert the revert (re-applies original changes)
git revert --no-edit {revert_commit_sha}
```

This creates a new commit that re-applies the original changes. History shows: original --> revert --> re-apply. Clean and traceable.

**Use when:** You want to fully undo the revert and restore all original changes. Works whether or not the revert was pushed.

### Strategy 2: Cherry-pick original commits

```bash
# If you know the original SHAs (in chronological order, oldest first)
git cherry-pick {original_sha_oldest}
git cherry-pick {original_sha_next}
# ...continue for remaining commits
```

**Use when:** You want to re-apply only SOME of the original commits, not all. Or when the revert commit itself has been amended/squashed and reverting it would not cleanly restore the originals.

### Strategy 3: Reset to backup tag (local-only)

```bash
# If commits were never pushed and backup tag exists
git reset --hard pre-revert-{timestamp}
git tag -d pre-revert-{timestamp}  # Clean up tag after
```

**Use when:** The revert was local-only and you want to pretend it never happened. This is a destructive operation -- see Danger Zones.

### After rollback-of-rollback

Update maestro plan state to reflect re-applied work:
- Change `[ ]` back to `[x] {sha}` in `plan.md` (use the new SHA if cherry-picked)
- Update track status in `tracks.md` if needed
- Re-close BR issues if applicable: `br update {issue_id} --status closed --json`

---

## Relationship to Other Commands

Recommended workflow:

- `/maestro:setup` -- Scaffold project context (run first)
- `/maestro:new-track` -- Create a feature/bug track with spec and plan
- `/maestro:implement` -- Execute the implementation
- `/maestro:review` -- Verify implementation correctness
- `/maestro:status` -- Check progress across all tracks
- `/maestro:revert` -- **You are here.** Undo implementation if needed
- `/maestro:note` -- Capture decisions and context to persistent notepad

Revert is the safety valve for `/maestro:implement`. It undoes commits and resets plan state so you can re-implement with `/maestro:implement`. Use `/maestro:status` after reverting to confirm the track state is correct. Revert depends on atomic commits from implementation -- the cleaner the commit history, the more precise the revert.
