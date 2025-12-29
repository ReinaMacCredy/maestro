# Trigger Routing

## Trigger Disambiguation Table

| Trigger Phrase | Context Check | Routes To | Reason |
|----------------|---------------|-----------|--------|
| `ds` | Any | design | Explicit shorthand |
| `/conductor-design` | Any | design | Explicit command |
| "design a feature" | Any | design | Design intent |
| "let's think through X" | Any | design | Exploration intent |
| "brainstorm X" | Any | design | Exploration intent |
| "track this work" | `conductor/` exists | conductor | Track-aware context |
| "track this work" | no `conductor/` | beads | Standalone tracking |
| "create task for" | `conductor/` exists | conductor | Workflow integration |
| "create task for" | no `conductor/` | beads | Direct issue creation |
| "what's blocking" | Any | beads | Dependency query |
| "what's ready" | Any | beads | Ready work query |
| `bd ready`, `bd show` | Any | beads | Explicit CLI |
| "implement the feature" | `conductor/` exists | conductor | Workflow execution |
| "start working" | `conductor/` exists | conductor | Workflow execution |
| worktree creation | Implementation start | using-git-worktrees | Isolation needed |
| "share this skill" | Any | sharing-skills | Contribution flow |
| "create a skill" | Any | writing-skills | Skill authoring |

## Context-Aware Routing Logic

```
IF explicit command (ds, /conductor-*, bd)
  → Route to named skill

ELSE IF "design" or "brainstorm" or "think through"
  → Route to design

ELSE IF "track" or "create task"
  → IF conductor/ exists
      → Route to conductor
    ELSE
      → Route to beads

ELSE IF "blocking" or "ready" or "dependencies"
  → Route to beads

ELSE IF implementation context
  → IF worktree needed
      → Route to using-git-worktrees
    ELSE
      → Route to conductor
```

## Beads vs TodoWrite Decision

### Decision Flowchart

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

### Quick Decision Table

| Scenario | Use |
|----------|-----|
| Multi-session work | Beads |
| Complex dependencies | Beads |
| Must survive compaction | Beads |
| Fuzzy/exploratory work | Beads |
| Single-session tasks | TodoWrite |
| Linear execution | TodoWrite |
| Conversation-scoped only | TodoWrite |

### Rule of Thumb

**If resuming in 2 weeks would be hard without bd, use bd.**

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
