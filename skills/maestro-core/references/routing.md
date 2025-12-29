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

1. Check hierarchy level (maestro-core > conductor > orchestrator > design > beads > specialized)
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

## Orchestrator Invocation Points

### Wrapper Pattern: /conductor-implement Auto-Routes

**`/conductor-implement` is a wrapper** that automatically routes to `/conductor-orchestrate` when parallel execution is appropriate.

```
/conductor-implement
        ↓
  [Phase 2b: Routing]
        ↓
  ┌─────────────────────────────────────────┐
  │ Has "## Track Assignments" in plan.md?  │
  │   YES → PARALLEL_DISPATCH               │
  │   NO  → Check TIER 1/2 scoring          │
  └─────────────────────────────────────────┘
        ↓
  ┌─────────────────────────────────────────┐
  │ PARALLEL_DISPATCH?                      │
  │   YES → Hand off to orchestrator        │
  │   NO  → Continue sequential (Phase 3)   │
  └─────────────────────────────────────────┘
```

### User Should Use `/conductor-implement` (Not `/conductor-orchestrate`)

| User Types | Result |
|------------|--------|
| `/conductor-implement` | Auto-routes based on plan.md |
| `/conductor-orchestrate` | Also works (direct orchestrator) |
| `ci` | Alias for /conductor-implement |
| `co` | Alias for /conductor-orchestrate |

**Recommendation:** Always use `/conductor-implement` (or `ci`). It will route to orchestrator automatically when Track Assignments exist.

### Routing Priority

1. **Track Assignments exists** → PARALLEL_DISPATCH (immediate)
2. **Agent Mail unavailable** → SINGLE_AGENT (cannot coordinate)
3. **TIER 1 + TIER 2 pass** → PARALLEL_DISPATCH
4. **Otherwise** → SINGLE_AGENT

### Orchestrator Fallback

When orchestrator cannot proceed:

| Condition | Fallback |
|-----------|----------|
| Agent Mail unavailable | DEGRADE to sequential in same agent |
| Worker spawn fails | Retry once, then DEGRADE to sequential |

### Orchestrator Workflow Integration

```
/conductor-design → /conductor-newtrack → /conductor-implement
                                                   ↓
                                          [Phase 2b Routing]
                                                   ↓
                                    ┌──────────────┴──────────────┐
                                    ↓                             ↓
                           PARALLEL_DISPATCH               SINGLE_AGENT
                           (orchestrator)                  (sequential)
                                    │                             │
                                    └──────────────┬──────────────┘
                                                   ↓
                                          /conductor-finish
```

### Cross-Track Dependency Handling

Orchestrator monitors and resolves cross-track dependencies:

1. Worker completes blocking bead → sends `[DEP] <bead-id> COMPLETE` message
2. Waiting worker polls inbox for dependency notification
3. Orchestrator mediates if timeout (30 min default)
4. Force unblock option for manual intervention
