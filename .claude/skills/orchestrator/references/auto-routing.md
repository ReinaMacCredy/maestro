# Auto-Routing to Parallel Execution

When `/conductor-implement` runs without explicit Track Assignments in plan.md,
the system can auto-detect opportunities for parallel execution.

## Detection Algorithm

1. **Check metadata.json.beads.planTasks** - Maps plan tasks to bead IDs
2. **Verify with `bd list --json`** - Runtime source of truth
3. **Analyze dependency graph** - Find beads with no blockers
4. **Threshold: ≥2 independent beads** - Triggers auto-orchestration

## Flow

```
┌─────────────────────────────────────────────────────────────┐
│                    /conductor-implement                      │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
              ┌───────────────────────────────┐
              │ Check Track Assignments?      │
              └───────────────────────────────┘
                    │               │
                   YES              NO
                    │               │
                    ▼               ▼
           PARALLEL_DISPATCH   ┌─────────────────────┐
                               │ Check metadata.json │
                               │ beads.planTasks?    │
                               └─────────────────────┘
                                    │           │
                                  EXISTS      MISSING
                                    │           │
                                    ▼           ▼
                         ┌──────────────┐   TIER 1/2
                         │ Count beads  │   evaluation
                         │ with no deps │
                         └──────────────┘
                               │
                               ▼
                    ┌──────────────────────┐
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

## Related

- [implement.md](../../conductor/references/workflows/implement.md) - Execution routing Phase 2b
- [workflow.md](workflow.md) - Orchestrator workflow
- [FILE_BEADS.md](../../beads/references/FILE_BEADS.md) - How planTasks is populated
