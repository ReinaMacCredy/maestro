# Parallel Dispatch Pattern

Execute independent tasks concurrently using Task tool.

## When to Use
- 2+ tasks with no shared state
- No sequential dependencies
- Independent file modifications

## Pattern
```
Task 1: Handle feature A in src/a/
Task 2: Handle feature B in src/b/
```

See parent skill SKILL.md for full workflow.
