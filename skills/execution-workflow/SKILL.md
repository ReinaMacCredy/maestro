# Execution Workflow

Use when: User says `ct`, `claim task`, or wants to claim and implement the next available task from beads.

## Trigger Phrases

- `ct`
- `claim task`
- `claim next task`
- `work on next issue`

## Workflow

### Phase 1: Find Available Work

```bash
bd ready --json
```

If no tasks available:
1. Check for blocked tasks: `bd blocked --json`
2. Report status and ask user what to do next

### Phase 2: Claim Task

1. Select highest priority ready task
2. Mark as in-progress:
   ```bash
   bd update <issue-id> --status in_progress
   ```
3. Read task details:
   ```bash
   bd show <issue-id>
   ```

### Phase 3: Setup Isolation (Optional)

For complex tasks, create isolated worktree:
- Load `using-git-worktrees` skill
- Create worktree for the task

### Phase 4: Implement with TDD

1. Load `test-driven-development` skill
2. Follow RED-GREEN-REFACTOR cycle:
   - Write failing test first
   - Implement minimum code to pass
   - Refactor while green

### Phase 5: Verify Before Claiming Done

1. Load `verification-before-completion` skill
2. Run all verification commands
3. Confirm tests pass, no lint errors

### Phase 6: Update Beads

```bash
bd update <issue-id> --notes "COMPLETED: <summary>. Files changed: <list>"
```

### Phase 7: Finish or Continue

Ask user:
- Continue to next task? → Loop to Phase 1
- Finish branch? → Load `finishing-a-development-branch` skill

## Output Format

```
CLAIMING: <issue-id> - <title>
PRIORITY: <priority>
DEPENDENCIES: <resolved deps>

[Implementation steps...]

STATUS: Complete | In Progress | Blocked
NEXT: <recommendation>
```
