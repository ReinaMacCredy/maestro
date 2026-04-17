# Safety Checks -- Extended Pre-Flight Validation

This document extends the mandatory safety pre-checks in SKILL.md with deeper validation for edge cases and complex environments.

## Environment Checks

### Git version

```bash
git --version
```

Minimum required: Git 2.25+ (for `git restore`, `--pathspec-from-file`).
Recommended: Git 2.38+ (for `--update-refs` in rebase, improved merge-ort).

### Repository state

```bash
# Confirm we are inside a git repo
git rev-parse --is-inside-work-tree

# Check for bare repo (cannot revert in bare repos)
git rev-parse --is-bare-repository  # Must be "false"

# Check for shallow clone (may be missing commit history)
git rev-parse --is-shallow-repository
```

If shallow: warn that SHAs from older commits may not be available. Suggest `git fetch --unshallow` first.

### In-progress operations

```bash
# Rebase in progress
test -d "$(git rev-parse --git-dir)/rebase-merge" && echo "[!] REBASE IN PROGRESS"
test -d "$(git rev-parse --git-dir)/rebase-apply" && echo "[!] REBASE IN PROGRESS"

# Merge in progress
test -f "$(git rev-parse --git-dir)/MERGE_HEAD" && echo "[!] MERGE IN PROGRESS"

# Cherry-pick in progress
test -f "$(git rev-parse --git-dir)/CHERRY_PICK_HEAD" && echo "[!] CHERRY-PICK IN PROGRESS"

# Bisect in progress
test -f "$(git rev-parse --git-dir)/BISECT_LOG" && echo "[!] BISECT IN PROGRESS"

# Revert already in progress
test -f "$(git rev-parse --git-dir)/REVERT_HEAD" && echo "[!] REVERT IN PROGRESS"
```

If any operation is in progress: STOP. The user must complete or abort it first:
- Rebase: `git rebase --abort`
- Merge: `git merge --abort`
- Cherry-pick: `git cherry-pick --abort`
- Bisect: `git bisect reset`
- Revert: `git revert --abort`

---

## Remote State Detection

### Pushed vs local-only commits

```bash
# Get upstream tracking branch
upstream=$(git rev-parse --abbrev-ref @{upstream} 2>/dev/null)

if [ -z "$upstream" ]; then
  echo "[!] No upstream tracking branch. All commits treated as local-only."
  echo "    Confirm with user before proceeding."
else
  # Commits on local but not remote
  local_only=$(git log --oneline "$upstream"..HEAD)

  # Commits on remote but not local
  remote_only=$(git log --oneline HEAD.."$upstream")

  if [ -n "$remote_only" ]; then
    echo "[!] Remote has commits not in local. Pull first."
  fi
fi
```

### Fetch before comparing

```bash
# Always fetch to get current remote state
git fetch --quiet

# Then compare
git log --oneline origin/$(git branch --show-current)..HEAD
```

If remote is ahead of local: warn that reverting without pulling first may cause conflicts on next push.

### Force push detection

```bash
# Check if any target SHAs exist on remote
for sha in {sha_list}; do
  git branch -r --contains "$sha" 2>/dev/null
done
```

If SHAs exist on remote branches: `git revert` is the only safe option. Do NOT suggest `git reset`.

---

## Stash Management

### Pre-revert stash protocol

If working tree is dirty and user wants to proceed:

```bash
# Stash with descriptive message
git stash push -m "pre-revert-$(date +%Y%m%d-%H%M%S): saving before revert of {scope}"

# Verify stash was created
git stash list | head -1
```

### Post-revert stash recovery

After revert completes:

```bash
# List stashes to find the pre-revert one
git stash list | grep "pre-revert"

# Pop the stash (apply + remove)
git stash pop stash@{N}

# If pop conflicts: apply without removing, resolve, then drop
git stash apply stash@{N}
# ... resolve conflicts ...
git stash drop stash@{N}
```

### Stash expiration warning

Stashes do not expire by default, but `git gc` can prune unreachable stash entries. If a revert spans multiple days, verify stash still exists before attempting recovery.

---

## Branch Protection

### Protected branches

Check if the current branch has push restrictions:

```bash
# Check local branch protection (if configured)
git config --get branch.$(git branch --show-current).pushRemote

# Check if branch is the default branch
default_branch=$(git symbolic-ref refs/remotes/origin/HEAD 2>/dev/null | sed 's@^refs/remotes/origin/@@')
current_branch=$(git branch --show-current)

if [ "$current_branch" = "$default_branch" ]; then
  echo "[!] You are on the default branch ($default_branch)."
  echo "    Revert commits will be pushed directly to the default branch."
  echo "    Consider creating a revert branch and using a PR instead."
fi
```

### Revert via PR (recommended for protected branches)

```bash
# Create a revert branch
git checkout -b revert/{track_id}-$(date +%Y%m%d)

# Execute reverts on this branch
git revert --no-edit {sha1} {sha2} ...

# Push and create PR
git push -u origin revert/{track_id}-$(date +%Y%m%d)
```

This approach:
- Preserves branch protection rules
- Allows code review of the revert
- CI runs on the revert before it reaches the default branch

---

## Submodule Considerations

### Detect submodules

```bash
git submodule status 2>/dev/null
```

If submodules exist and any target commits modified `.gitmodules` or submodule paths:

1. Warn: "Reverting commits that modify submodules may leave submodules in an inconsistent state."
2. After revert: `git submodule update --init --recursive`
3. Verify submodule state: `git submodule status` (no `+` prefix = clean)

### Submodule pointer reverts

If a commit only changed a submodule pointer (the SHA the parent repo references):
- The revert will change the pointer back to the old SHA
- Run `git submodule update` to checkout the old submodule state
- The submodule's own history is unaffected

---

## CI Pipeline State

### Check for running pipelines

Before reverting, check if CI is running on the current branch:

```bash
# GitHub Actions
gh run list --branch $(git branch --show-current) --status in_progress --json databaseId,status 2>/dev/null

# Generic: check if test suite is currently running
# (project-specific -- adapt to your CI)
```

If CI is running:
- Warn: "CI pipeline is in progress. Reverting now will trigger a new pipeline."
- The user may want to wait for CI to complete first (to compare before/after)

### Post-revert CI

After revert, CI should run automatically on push. If it does not:
```bash
# Manually trigger if needed (GitHub Actions)
gh workflow run {workflow_name} --ref $(git branch --show-current)
```

---

## Pre-Flight Checklist Summary

Run this complete checklist before executing any revert:

```
[ ] Git version >= 2.25
[ ] Inside a git work tree (not bare)
[ ] Not a shallow clone (or fetched full history)
[ ] No in-progress operations (rebase, merge, cherry-pick, bisect, revert)
[ ] Working tree is clean (or stashed with descriptive message)
[ ] On the correct branch
[ ] Remote state is known (fetched recently)
[ ] Backup tag created at current HEAD
[ ] Branch protection considered (default branch? use PR?)
[ ] Submodule state checked (if applicable)
[ ] CI pipeline state noted
```

All items checked: proceed with revert.
Any item failed: resolve before continuing.
