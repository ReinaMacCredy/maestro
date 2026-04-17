# Danger Zones -- Destructive Operations Catalog

This document catalogs every destructive git operation that may arise during a revert workflow. Each entry includes: what it does, when it is appropriate, what it destroys, and how to recover.

**Rule of thumb:** If an operation cannot be undone with another git command, it is destructive and requires explicit user confirmation.

---

## Destructive Operations

### 1. `git reset --hard <target>`

**What it does:** Moves HEAD, index, AND working tree to `<target>`. All uncommitted changes are destroyed. All commits after `<target>` become unreachable (but exist in reflog temporarily).

**When appropriate:**
- Local-only commits that have NOT been pushed
- You have a backup tag at the current HEAD
- Working tree has no uncommitted changes you want to keep

**What it destroys:**
- Uncommitted changes in working tree (GONE -- not in reflog)
- Staged but uncommitted changes (GONE)
- Commits between `<target>` and old HEAD become unreachable

**Confirmation dialog:**
```
[!] DESTRUCTIVE OPERATION: git reset --hard

This will:
- Discard ALL uncommitted changes (cannot be recovered)
- Move HEAD to {target} ({sha7})
- Make {N} commits unreachable (recoverable via reflog for ~30 days)

Backup tag: {tag_name} at {current_sha7}

Type "I understand this destroys uncommitted work" to proceed:
```

**Recovery:**
```bash
# If backup tag exists
git reset --hard {backup_tag}

# If no backup tag, use reflog (within 30 days)
git reflog
git reset --hard HEAD@{N}  # where N is the entry before the reset

# Uncommitted changes: UNRECOVERABLE
# This is why the backup tag must exist BEFORE the reset
```

---

### 2. `git push --force` / `git push --force-with-lease`

**What it does:** Overwrites remote branch history with local history. Anyone who has pulled the old history will have a diverged state.

**When appropriate:**
- Almost never during a revert workflow
- Only if: you did a local `git reset` and now need to sync the remote
- Only if: you are the sole contributor on this branch
- Prefer `--force-with-lease` over `--force` (fails if remote has new commits you have not seen)

**What it destroys:**
- Remote branch history (old commits become unreachable on remote)
- Other contributors' local state (they must `git fetch && git reset --hard origin/{branch}`)

**Confirmation dialog:**
```
[!] DESTRUCTIVE OPERATION: git push --force

This will overwrite remote history on branch '{branch}'.
Anyone who has pulled this branch will need to force-reset their local copy.

Questions:
1. Is this a shared branch? (If yes: DO NOT force push. Use git revert instead.)
2. Have you verified no one else has pushed to this branch?

Using --force-with-lease for safety (will fail if remote has new commits).

Type "I confirm this branch has no other contributors" to proceed:
```

**Recovery:**
```bash
# On the remote (if you have access to server-side reflog):
# Contact your git hosting admin

# For other contributors who pulled the old history:
git fetch origin
git reset --hard origin/{branch}

# If you need the old remote state:
# Check if any contributor still has it locally
# Or check git hosting's branch protection / audit log
```

---

### 3. `git branch -D <branch>`

**What it does:** Deletes a branch even if it has not been merged. The commits on that branch become unreachable (unless they are on other branches too).

**When appropriate:**
- Cleaning up after a completed track-level revert
- The branch was a feature branch that is no longer needed
- All commits on the branch are either merged or intentionally abandoned

**What it destroys:**
- The branch pointer (commits remain in reflog temporarily)
- If commits are not on any other branch, they become unreachable

**Confirmation dialog:**
```
[!] DESTRUCTIVE OPERATION: git branch -D {branch}

This branch has {N} commits not on any other branch.
Deleting it makes those commits unreachable (recoverable via reflog for ~30 days).

Is this branch fully merged or intentionally abandoned?
- "merged" -- I verified all commits are on another branch
- "abandoned" -- I no longer need these commits
- "cancel" -- Do not delete
```

**Recovery:**
```bash
# Find the branch tip in reflog
git reflog | grep "checkout: moving from {branch}"
# The SHA after "moving from {branch} to" is where {branch} pointed

# Recreate the branch
git branch {branch} {sha}

# Or find it by date
git reflog --date=iso | grep {branch}
```

---

### 4. `git clean -fd`

**What it does:** Removes untracked files and directories from the working tree. They are permanently deleted (not moved to trash, not in git history).

**When appropriate:**
- After a revert leaves orphaned generated files
- Cleaning up build artifacts that are not in `.gitignore`

**What it destroys:**
- Untracked files (not in git, not in reflog, PERMANENTLY GONE)

**Confirmation dialog:**
```
[!] DESTRUCTIVE OPERATION: git clean -fd

This will permanently delete these untracked files:
{list from git clean -n -d}

These files are NOT in git history and CANNOT be recovered.

Proceed? (yes/no)
```

**Recovery:**
- NONE. Untracked files are not recoverable via git.
- Check OS-level backups (Time Machine, etc.)
- Always run `git clean -n -d` (dry run) first to preview what will be deleted

---

### 5. `git stash drop` / `git stash clear`

**What it does:** Permanently removes stash entries. `drop` removes one entry, `clear` removes all.

**When appropriate:**
- After confirming stashed changes are no longer needed
- Post-revert cleanup when the pre-revert stash has been applied

**What it destroys:**
- The stash entry (the commit object remains in reflog briefly, but `git stash list` will not show it)

**Confirmation dialog:**
```
Dropping stash: {stash_message}
This stash contains changes to {N} files.

Confirm drop? (yes/no)
```

**Recovery:**
```bash
# Recently dropped stash (within the same session usually):
git fsck --unreachable | grep commit
# Find the stash commit and apply it:
git stash apply {sha}

# After git gc: UNRECOVERABLE
```

---

## Reflog as Safety Net

The reflog records every HEAD movement. It is your primary recovery mechanism for destructive operations.

### Reflog basics

```bash
# View reflog (most recent first)
git reflog

# View reflog with dates
git reflog --date=iso

# View reflog for a specific branch
git reflog show {branch}

# Find a specific operation
git reflog | grep "reset"
git reflog | grep "revert"
```

### Reflog expiration

| Entry type | Default expiration |
|-----------|-------------------|
| Reachable (on a branch) | 90 days |
| Unreachable (orphaned) | 30 days |
| After `git gc --prune=now` | Immediately |

**Warning:** `git gc` runs automatically. Do not rely on reflog entries surviving more than 30 days for unreachable commits. If you need long-term recovery, use backup tags instead.

### Reflog recovery examples

```bash
# Recover from accidental git reset --hard
git reflog
# Find the entry BEFORE the reset (e.g., HEAD@{1})
git reset --hard HEAD@{1}

# Recover a lost commit
git reflog | grep "{partial commit message}"
git cherry-pick {sha_from_reflog}

# Recover a deleted branch
git reflog | grep "checkout: moving from {branch}"
git branch {branch} {sha}
```

---

## Point of No Return Markers

Some operations have a true point of no return. Know them:

| Operation | Point of no return | Mitigation |
|-----------|-------------------|------------|
| `git reset --hard` | Uncommitted changes are lost immediately | Stash or commit first |
| `git clean -fd` | Untracked files are deleted immediately | `git clean -n` dry run first |
| `git push --force` | Old remote history is overwritten | `--force-with-lease` + backup tag |
| `git gc --prune=now` | Unreachable objects are deleted immediately | Never run manually during revert |
| `git stash drop` after `gc` | Stash object is garbage collected | Apply stash before dropping |

---

## Confirmation Template Reference

For any destructive operation, use this template:

```
[!] DESTRUCTIVE OPERATION: {command}

What this does:
{1-2 sentence description}

What will be lost:
{bullet list of data that cannot be recovered}

Safety measures in place:
{backup tag, stash, etc.}

To proceed, type: "{confirmation phrase}"
To cancel: "cancel"
```

The confirmation phrase must be specific to the operation (not just "yes"). This prevents accidental confirmation. Examples:
- "I understand this destroys uncommitted work"
- "I confirm this branch has no other contributors"
- "I accept the loss of {N} untracked files"
