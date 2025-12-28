# Verification Rollback

When verification fails, you have options.

## Rollback Options

### Option 1: Fix Forward
Most common - fix the failing verification.

```bash
# See the failure
npm test

# Fix the issue
# ...

# Re-verify
npm test
```

### Option 2: Git Revert
When changes are wrong, revert completely.

```bash
# Revert last commit
git revert HEAD

# Or reset to known-good state
git reset --hard <commit>
```

### Option 3: Partial Rollback
When some changes are good, others bad.

```bash
# Interactive revert
git checkout <commit> -- path/to/file

# Or manually undo specific changes
```

## Red Flags

**Never:**
- Claim success without re-verification after "fix"
- Suppress warnings/errors in verification commands
- Change verification thresholds to pass
- Skip re-running full suite after any change

**Always:**
- Run complete verification after any rollback
- Document why rollback was needed
- Verify the rollback itself worked
