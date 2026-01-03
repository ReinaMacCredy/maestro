# Workflow Chain

Complete pipeline from idea to implementation.

## Phase Flow

```
┌─────────────┐         ┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│   DESIGN    │────┐    │    SPEC     │───▶│    PLAN     │───▶│  IMPLEMENT  │
│     ds      │    │    │  newtrack   │    │     fb      │    │   ci / co   │
└─────────────┘    │    └─────────────┘    └─────────────┘    └─────────────┘
       │           │           │                  │                  │
       ▼           │           ▼                  ▼                  ▼
  design.md        │      spec.md            plan.md            code + tests
       │           │     + plan.md           + beads
       │           │           ▲
       │           │           │
┌─────────────┐    │           │
│  PLANNING   │────┘           │
│     pl      │────────────────┘
└─────────────┘
       │
       ▼
  design.md (execution-focused)
```

### Dual Entry Points

| Entry | Trigger | Focus | When to Use |
|-------|---------|-------|-------------|
| Design | `ds` | Exploratory | Unclear requirements, discovery needed |
| Planning | `pl` | Execution | Known scope, ready to implement |

Both paths produce `design.md` and flow into `/conductor-newtrack`.

## Commands Per Phase

### Design Phase
- `ds` or `/conductor-design` - Start Double Diamond session
- Outputs: `conductor/tracks/<id>/design.md`

### Spec Phase  
- `/conductor-newtrack <id>` - Generate spec and plan from design
- Outputs: `spec.md`, `plan.md`, `metadata.json`

### Plan Phase
- `fb` or `file-beads` - File beads from plan.md tasks
- `rb` or `review-beads` - Review filed beads
- Outputs: Beads in `.beads/`

### Implement Phase
- `ci` or `/conductor-implement` - Sequential execution
- `co` or `/conductor-orchestrate` - Parallel execution with workers
- `bd ready` - Find next available work
- `/conductor-finish` - Complete and archive track

## A/P/C Checkpoints

At each phase end in design:
- **[A]** Advanced - deeper analysis
- **[P]** Party - multi-agent feedback  
- **[C]** Continue - proceed to next phase

## Execution Routing

```
ci triggered
    │
    ▼
Track Assignments in plan.md?
    │
 ┌──┴──┐
YES    NO
 │      │
 ▼      ▼
 co    sequential
```
