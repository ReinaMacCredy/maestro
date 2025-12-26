# Execution Routing Pattern

<!-- execution-routing v1 -->

Decide between SINGLE_AGENT and PARALLEL_DISPATCH execution modes based on task characteristics.

## When to Evaluate

- After track selection in implement.md Phase 2b
- Before starting the work loop (Phase 3)

## Two-Tier Evaluation

### TIER 1: Weighted Score (Quick Filter)

Fast check to determine if parallel dispatch is even worth considering.

| Factor | Weight | Description |
|--------|--------|-------------|
| Epics > 1 | +2 | Work spans multiple epics |
| [PARALLEL] markers | +3 | Plan explicitly marks parallelizable work |
| Domains > 2 | +2 | Changes touch multiple modules/domains |
| Independent tasks > 5 | +1 | Many tasks with no dependencies |

**TIER 1 Threshold:** Score >= 5 to proceed to TIER 2

```python
TIER1_SCORE = (
    (epics > 1) * 2 +
    (has_parallel_markers) * 3 +
    (domains > 2) * 2 +
    (independent_tasks > 5) * 1
)
# PASS if score >= 5
```

### TIER 2: Compound Conditions (Deep Check)

If TIER 1 passes, evaluate resource requirements:

```python
TIER2_PASS = (
    (files > 15 AND tasks > 3) OR
    (est_tool_calls > 40) OR
    (est_time > 30 AND independent_ratio > 0.6)
)
```

| Condition | Rationale |
|-----------|-----------|
| files > 15 AND tasks > 3 | Large scope benefits from parallelism |
| est_tool_calls > 40 | High tool volume = good parallel candidate |
| est_time > 30 min AND independent_ratio > 0.6 | Long task with mostly independent work |

## Routing Decision

| TIER 1 | TIER 2 | Result |
|--------|--------|--------|
| FAIL (<5) | - | SINGLE_AGENT |
| PASS (>=5) | FAIL | SINGLE_AGENT |
| PASS (>=5) | PASS | PARALLEL_DISPATCH |

## Visible Feedback

Always show routing decision to user:

```text
┌─ EXECUTION ROUTING ────────────────────┐
│ TIER 1 Score: 6/8                      │
│   Epics > 1:        ✓ (+2)             │
│   [PARALLEL]:       ✓ (+3)             │
│   Domains > 2:      ✗ (+0)             │
│   Independent > 5:  ✓ (+1)             │
├────────────────────────────────────────┤
│ TIER 2 Conditions:                     │
│   files > 15 AND tasks > 3: ✓          │
│   est_tool_calls > 40:      ✗          │
│   est_time > 30min:         ✗          │
├────────────────────────────────────────┤
│ Result: PARALLEL_DISPATCH              │
└────────────────────────────────────────┘
```

For SINGLE_AGENT:

```text
┌─ EXECUTION ROUTING ────────────────────┐
│ TIER 1 Score: 3/8                      │
│   Epics > 1:        ✗ (+0)             │
│   [PARALLEL]:       ✗ (+0)             │
│   Domains > 2:      ✓ (+2)             │
│   Independent > 5:  ✓ (+1)             │
├────────────────────────────────────────┤
│ TIER 1 threshold not met (3 < 5)       │
│ Result: SINGLE_AGENT                   │
└────────────────────────────────────────┘
```

## State Persistence

Store result in `implement_state.json`:

```json
{
  "execution_mode": "PARALLEL_DISPATCH",
  "routing_evaluation": {
    "tier1_score": 6,
    "tier1_pass": true,
    "tier2_pass": true,
    "evaluated_at": "2025-12-26T10:00:00Z"
  }
}
```

## Integration with Parallel Dispatch

When result is PARALLEL_DISPATCH:

1. Reference [parallel-dispatch.md](parallel-dispatch.md) for reservation flow
2. Group tasks by domain/module for efficient batching
3. Apply [subagent-prompt.md](subagent-prompt.md) coordination blocks

When result is SINGLE_AGENT:

1. Execute tasks sequentially in work loop
2. No file reservations needed
3. Standard task completion flow

## Examples

### Example 1: Small Feature → SINGLE_AGENT

```text
Task: Add validation to user form
Epics: 1
[PARALLEL]: No
Domains: 1 (frontend)
Independent tasks: 2

TIER 1: 0 + 0 + 0 + 0 = 0 -> FAIL
Result: SINGLE_AGENT
```

### Example 2: Multi-Epic Feature → PARALLEL_DISPATCH

```text
Task: Context Engineering Integration
Epics: 8
[PARALLEL]: Yes (Epics 1, 2, 8)
Domains: 4 (workflows, skills, conductor, docs)
Independent tasks: 7

TIER 1: 2 + 3 + 2 + 1 = 8 -> PASS

Files: 20, Tasks: 15
TIER 2: (20 > 15 AND 15 > 3) = true -> PASS

Result: PARALLEL_DISPATCH
```

### Example 3: Medium Feature, Low Independence → SINGLE_AGENT

```text
Task: Add authentication middleware
Epics: 2
[PARALLEL]: No
Domains: 2 (backend, auth)
Independent tasks: 3

TIER 1: 2 + 0 + 0 + 0 = 2 -> FAIL
Result: SINGLE_AGENT
```

## Manual Override

User can force execution mode:

- `ci --parallel` → Force PARALLEL_DISPATCH
- `ci --sequential` → Force SINGLE_AGENT

Override is logged in routing_evaluation with `override: true`.
