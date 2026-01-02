# Auto-Routing to Parallel Execution

When `/conductor-implement` runs without explicit Track Assignments in plan.md,
the system can auto-detect opportunities for parallel execution.

## Detection Priority

| Priority | Check | Trigger |
|----------|-------|---------|
| 1 | Track Assignments in plan.md | Explicit parallel dispatch |
| **1.5** | **fileScopes in metadata.json** | **File-scope grouping** |
| 2 | beads.planTasks with ≥2 independent | Bead dependency analysis |

## Detection Algorithm

1. **Check Track Assignments** - Explicit parallel routing (Priority 1)
2. **Check metadata.json.beads.fileScopes** - File-scope based routing (Priority 1.5)
3. **Check metadata.json.beads.planTasks** - Maps plan tasks to bead IDs
4. **Verify with `bd list --json`** - Runtime source of truth
5. **Analyze dependency graph** - Find beads with no blockers
6. **Threshold: ≥2 independent beads** - Triggers auto-orchestration

## Priority 1.5: File Scope Routing

If `metadata.json.beads.fileScopes` exists:

1. **Load file scopes** from metadata.json
2. **Run parallel-grouping** algorithm (see [parallel-grouping.md](../../conductor/references/parallel-grouping.md))
3. **Check threshold**: ≥2 non-overlapping groups → PARALLEL_DISPATCH
4. **Generate Track Assignments** dynamically if not present in plan.md

### Algorithm Reference

- **Extraction**: [file-scope-extractor.md](../../conductor/references/file-scope-extractor.md)
- **Grouping**: [parallel-grouping.md](../../conductor/references/parallel-grouping.md)

## Flow

```
┌─────────────────────────────────────────────────────────────┐
│                    /conductor-implement                      │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
              ┌───────────────────────────────┐
              │ Priority 1: Track Assignments?│
              └───────────────────────────────┘
                    │               │
                   YES              NO
                    │               │
                    ▼               ▼
           PARALLEL_DISPATCH   ┌─────────────────────┐
                               │ Priority 1.5:       │
                               │ beads.fileScopes?   │
                               └─────────────────────┘
                                    │           │
                                  EXISTS      MISSING
                                    │           │
                                    ▼           ▼
                         ┌──────────────┐   ┌─────────────────────┐
                         │ Run parallel │   │ Priority 2:         │
                         │ grouping     │   │ beads.planTasks?    │
                         └──────────────┘   └─────────────────────┘
                               │                 │           │
                               ▼               EXISTS      MISSING
                    ┌──────────────────────┐     │           │
                    │ Groups ≥ 2?          │     ▼           ▼
                    └──────────────────────┘ ┌──────────────┐ TIER 1/2
                         │           │       │ Count beads  │ evaluation
                        YES          NO      │ with no deps │
                         │           │       └──────────────┘
                         ▼           ▼             │
               PARALLEL_DISPATCH   TIER 1/2        ▼
                                   evaluation  ┌──────────────────────┐
                                               │ Independent ≥ 2?     │
                                               └──────────────────────┘
                                                    │           │
                                                   YES          NO
                                                    │           │
                                                    ▼           ▼
                                          PARALLEL_DISPATCH   TIER 1/2
                                                              evaluation
```

## Auto-Generated Track Assignments

When auto-routing triggers:

1. **Query bead dependencies:**
   ```bash
   bd list --json | jq '[.[] | select(.dependencies | length == 0)]'
   ```

2. **Group by file scope:**
   - Extract file paths from bead titles/descriptions
   - Same directory → same track
   - Different top-level directories → different tracks

3. **Generate Track Assignments table:**
   
   | Track | Beads | Depends On |
   |-------|-------|------------|
   | 1 | bd-1, bd-2 | - |
   | 2 | bd-3 | - |
   | 3 | bd-4, bd-5 | bd-2, bd-3 |

4. **Route to orchestrator skill**

5. **Execute in parallel waves**

## Example

Given metadata.json:
```json
{
  "beads": {
    "planTasks": {
      "1.1": "bd-1",
      "1.2": "bd-2",
      "2.1": "bd-3"
    }
  }
}
```

If bd-1 and bd-3 have no dependencies:
- 2 independent beads detected
- Auto-generate 2 tracks
- Route to parallel execution

## State Tracking

When auto-routing triggers, implement_state.json records:
```json
{
  "execution_mode": "PARALLEL_DISPATCH",
  "routing_trigger": "auto_detect",
  "routing_evaluation": {
    "has_track_assignments": false,
    "auto_detect_triggered": true,
    "independent_beads_count": 3,
    "agent_mail_available": true
  }
}
```

## Fallback Behavior

If auto-detection finds < 2 independent beads:
- Continue to TIER 1/2 heuristic evaluation
- May still route to parallel if TIER thresholds met

If Agent Mail unavailable:
- Fall back to sequential execution regardless of bead count

## Assignee Check in Bead Triage

When triaging beads for auto-routing, check the `assignee` field to prevent conflicts:

### Skip Already-Assigned Tasks

```python
def get_available_beads(epic_id: str) -> list:
    """Get beads available for assignment, excluding already-assigned."""
    beads = bash(f"bd list --parent={epic_id} --json")
    
    available = []
    for bead in beads:
        # Skip if already assigned to another worker
        if bead.get("assignee") and bead["assignee"] != current_agent:
            continue
        # Skip if not in ready status
        if bead.get("status") != "ready":
            continue
        available.append(bead)
    
    return available
```

### Triage Algorithm with Assignee Filter

1. **Query beads**: `bd list --parent=<epic-id> --json`
2. **Filter by assignee**: Skip beads where `assignee` is set (already claimed)
3. **Filter by status**: Only include `ready` beads
4. **Filter by dependencies**: Only include beads with no unresolved blockers
5. **Group by file scope**: Cluster remaining beads for parallel dispatch

```
┌─────────────────────────────────────────────────────────────┐
│                       Bead Triage                            │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
               ┌───────────────────────────────┐
               │    Has assignee field set?    │
               └───────────────────────────────┘
                     │               │
                    YES              NO
                     │               │
                     ▼               ▼
               ┌──────────┐   ┌────────────────┐
               │ SKIP     │   │ Check status   │
               │ (claimed)│   │ == "ready"     │
               └──────────┘   └────────────────┘
                                   │
                                   ▼
                        ┌──────────────────────┐
                        │ Check dependencies   │
                        │ all resolved?        │
                        └──────────────────────┘
                              │           │
                             YES          NO
                              │           │
                              ▼           ▼
                        ┌──────────┐  ┌──────────┐
                        │ INCLUDE  │  │ SKIP     │
                        │ (ready)  │  │ (blocked)│
                        └──────────┘  └──────────┘
```

### Handle Reassignment Scenarios

When a worker fails or times out, the orchestrator may need to reassign:

```python
def reassign_bead(bead_id: str, new_agent: str, reason: str):
    """Reassign a bead from a stale/failed worker to a new worker."""
    
    # 1. Clear old assignee and update status
    bash(f"bd update {bead_id} --assignee {new_agent} --status ready")
    
    # 2. Notify new worker via ASSIGN message
    send_message(
        project_key=PROJECT_KEY,
        sender_name=ORCHESTRATOR_NAME,
        to=[new_agent],
        thread_id=EPIC_ID,
        subject=f"[REASSIGN] {bead_id}",
        body_md=f"""
## Reassignment Notice

**Bead**: {bead_id}
**Reason**: {reason}
**Previous Worker**: Timed out / Failed

Please pick up this task following the 4-step protocol.
"""
    )
    
    # 3. Log reassignment
    bash(f"bd update {bead_id} --notes 'Reassigned to {new_agent}: {reason}'")
```

### Reassignment Triggers

| Trigger | Detection | Action |
|---------|-----------|--------|
| Worker timeout | No heartbeat for 10+ min | Prompt orchestrator for reassign |
| Worker crash | Task() returns error | Auto-reassign to new worker |
| Worker blocked | BLOCKED message received | Manual reassign or resolve blocker |
| Explicit release | Worker sends RELEASE message | Return bead to ready pool |

### State After Reassignment

```json
{
  "bead_id": "my-workflow:3-zyci.4.1",
  "assignee": "GreenCastle",
  "previous_assignees": ["BlueLake"],
  "reassignment_history": [
    {
      "from": "BlueLake",
      "to": "GreenCastle", 
      "reason": "timeout",
      "timestamp": "2025-12-30T02:15:00Z"
    }
  ]
}
```

## Related

- [implement.md](../../conductor/references/workflows/implement.md) - Execution routing Phase 2b
- [workflow.md](workflow.md) - Orchestrator workflow
- [FILE_BEADS.md](../../beads/references/FILE_BEADS.md) - How planTasks is populated
