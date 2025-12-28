# Branch Options

When implementation is complete and tests pass, choose how to integrate.

## Step 1: Verify Tests

**Before presenting options, verify tests pass:**

```bash
npm test / cargo test / pytest / go test ./...
```

**If tests fail:** Stop. Fix before proceeding.

## Step 2: Present Options

```
Implementation complete. What would you like to do?

1. Merge back to <base-branch> locally
2. Push and create a Pull Request
3. Keep the branch as-is (I'll handle it later)
4. Discard this work

Which option?
```

## Step 3: Execute Choice

### Option 1: Merge Locally

```bash
git checkout <base-branch>
git pull
git merge <feature-branch>
<test command>  # Verify on merged result
git branch -d <feature-branch>
```

### Option 2: Push and Create PR

```bash
git push -u origin <feature-branch>
gh pr create --title "<title>" --body "## Summary
<2-3 bullets of what changed>

## Test Plan
- [ ] <verification steps>"
```

### Option 3: Keep As-Is

Report: "Keeping branch <name>. Worktree preserved at <path>."

Don't cleanup worktree.

### Option 4: Discard

**Confirm first:**
```
This will permanently delete:
- Branch <name>
- All commits: <commit-list>
- Worktree at <path>

Type 'discard' to confirm.
```

If confirmed:
```bash
git checkout <base-branch>
git branch -D <feature-branch>
```

## Quick Reference

| Option | Merge | Push | Keep Worktree | Cleanup Branch |
|--------|-------|------|---------------|----------------|
| 1. Merge locally | ✓ | - | - | ✓ |
| 2. Create PR | - | ✓ | ✓ | - |
| 3. Keep as-is | - | - | ✓ | - |
| 4. Discard | - | - | - | ✓ (force) |
