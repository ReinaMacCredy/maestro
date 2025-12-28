# Cleanup Procedures

## Worktree Cleanup

For Options 1 and 4, cleanup the worktree after completion.

```bash
# Check if in worktree
git worktree list | grep $(git branch --show-current)

# If yes, remove it
git worktree remove <worktree-path>
```

**For Option 3 (keep as-is):** Don't cleanup worktree.

## Branch Cleanup

After successful merge (Option 1):
```bash
git branch -d <feature-branch>  # Safe delete (only if merged)
```

After discard (Option 4):
```bash
git branch -D <feature-branch>  # Force delete
```

## Common Mistakes

**Skipping test verification**
- Problem: Merge broken code, create failing PR
- Fix: Always verify tests before offering options

**Open-ended questions**
- Problem: "What should I do next?" → ambiguous
- Fix: Present exactly 4 structured options

**Automatic worktree cleanup**
- Problem: Remove worktree when might need it (Option 2, 3)
- Fix: Only cleanup for Options 1 and 4

**No confirmation for discard**
- Problem: Accidentally delete work
- Fix: Require typed "discard" confirmation

## Red Flags

**Never:**
- Proceed with failing tests
- Merge without verifying tests on result
- Delete work without confirmation
- Force-push without explicit request

**Always:**
- Verify tests before offering options
- Present exactly 4 options
- Get typed confirmation for Option 4
- Clean up worktree for Options 1 & 4 only
