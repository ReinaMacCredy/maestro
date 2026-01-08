# Verification Rollback

## Red Flags - STOP

- Using "should", "probably", "seems to"
- Expressing satisfaction before verification ("Great!", "Perfect!", "Done!", etc.)
- About to commit/push/PR without verification
- Trusting agent success reports
- Relying on partial verification
- Thinking "just this once"
- Tired and wanting work over
- **ANY wording implying success without having run verification**

## Rationalization Prevention

| Excuse | Reality |
|--------|---------|
| "Should work now" | RUN the verification |
| "I'm confident" | Confidence ≠ evidence |
| "Just this once" | No exceptions |
| "Linter passed" | Linter ≠ compiler |
| "Agent said success" | Verify independently |
| "I'm tired" | Exhaustion ≠ excuse |
| "Partial check is enough" | Partial proves nothing |
| "Different words so rule doesn't apply" | Spirit over letter |

## Rollback Options

When verification fails, you have options.

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

## Red Flags for Rollback

**Never:**
- Claim success without re-verification after "fix"
- Suppress warnings/errors in verification commands
- Change verification thresholds to pass
- Skip re-running full suite after any change

**Always:**
- Run complete verification after any rollback
- Document why rollback was needed
- Verify the rollback itself worked
