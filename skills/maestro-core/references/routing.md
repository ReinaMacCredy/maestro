# Routing Details

> **Main routing logic is in [SKILL.md](../SKILL.md).** This file contains extended details.

## Worktree Invocation Points

### When to Create Worktree

| Trigger | Context | Action |
|---------|---------|--------|
| Design approved | `/conductor-design` complete | Suggest worktree |
| Implementation starts | `/conductor-implement` | Create if not exists |
| Parallel agents | Multi-agent dispatch | Required per agent |
| Feature branch needed | Clean isolation required | Create worktree |

### Worktree Workflow

```
1. Design approved
   → "Design approved. Create worktree for implementation?"

2. /conductor-implement
   → Check for worktree
   → If missing: "Create worktree for isolated work?"

3. Parallel dispatch
   → Each subagent gets own worktree
   → Reserve files via Village MCP
```

### Worktree Decision

```
IF parallel agents needed
  → REQUIRED: Each agent needs own worktree

ELSE IF feature isolation needed
  → RECOMMENDED: Create worktree

ELSE IF quick fix in main
  → OPTIONAL: Work directly
```

## Edge Cases

### Multiple Skills Match

When multiple skills could apply:

1. Check hierarchy level (maestro-core > conductor > design > beads > specialized)
2. Higher level skill decides routing
3. If same level, check user intent keywords

### Ambiguous Intent

If intent unclear:

```
"What would you like to do?
1. Design a new feature (ds)
2. Track existing work (bd ready)
3. View project status (/conductor-status)"
```

### No Conductor Directory

When `conductor/` missing:

- Design skill: DEGRADE to standalone mode
- Beads skill: Work normally (standalone)
- Conductor commands: Suggest `/conductor-setup`

## Beads vs TodoWrite Decision Flowchart

```
Will I need this context in 2+ weeks?
  YES → Use Beads (bd)
  NO ↓

Could conversation history get compacted?
  YES → Use Beads (bd)
  NO ↓

Does this have blockers/dependencies?
  YES → Use Beads (bd)
  NO ↓

Will this be done in this session?
  YES → Use TodoWrite
  NO → Use Beads (bd)
```
