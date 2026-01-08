# Auto-Orchestration After Beads Filing

## Overview

Auto-orchestration triggers **automatically after `fb` completes** filing beads from a plan. It analyzes the dependency graph, generates Track Assignments, and dispatches parallel workers—eliminating manual orchestration setup.

### When It Triggers

1. `fb` completes successfully (all beads filed)
2. `metadata.json.beads.orchestrated` is `false` or absent
3. Epic has multiple beads (single-bead epics skip orchestration)

### What It Does

1. Queries dependency graph via `bv --robot-triage`
2. Generates Track Assignments from ready/blocked beads
3. Marks `metadata.json.beads.orchestrated = true`
4. Dispatches workers via orchestrator skill
5. Spawns `rb` sub-agent for final review after workers complete

## Graph Analysis Algorithm

### Query Command

```bash
bv --robot-triage --graph-root <epic-id> --json
```

### Output Structure

```json
{
  "quick_ref": "Epic bd-1: 5 beads, 3 ready, 2 blocked",
  "beads": [
    {
      "id": "bd-1",
      "title": "Epic: Feature X",
      "type": "epic",
      "priority": 2,
      "ready": true,
      "blocked_by": []
    },
    {
      "id": "bd-2",
      "title": "Add API endpoint",
      "type": "task",
      "priority": 2,
      "ready": true,
      "blocked_by": []
    },
    {
      "id": "bd-3",
      "title": "Add database schema",
      "type": "task",
      "priority": 2,
      "ready": true,
      "blocked_by": []
    },
    {
      "id": "bd-4",
      "title": "Add frontend component",
      "type": "task",
      "priority": 2,
      "ready": false,
      "blocked_by": ["bd-2"]
    },
    {
      "id": "bd-5",
      "title": "Integration tests",
      "type": "task",
      "priority": 2,
      "ready": false,
      "blocked_by": ["bd-2", "bd-3"]
    }
  ]
}
```

### Key Fields

| Field | Description |
|-------|-------------|
| `id` | Bead identifier (e.g., `bd-2`) |
| `ready` | `true` if no blockers, can start immediately |
| `blocked_by` | Array of bead IDs that must complete first |

## Track Assignment Generation

### Algorithm

```python
def generate_track_assignments(beads, max_workers=3):
    # Filter out epic itself (only process child tasks)
    tasks = [b for b in beads if b.type != 'epic']
    
    # Group ready beads (no blockers)
    ready = [b for b in tasks if b.ready]
    blocked = [b for b in tasks if not b.ready]
    
    # Create initial tracks from ready beads
    # Each ready bead starts its own track
    tracks = [[b.id] for b in ready]
    track_map = {b.id: i for i, b in enumerate(ready)}
    
    # Assign blocked beads to track of primary blocker
    for b in blocked:
        primary_blocker = b.blocked_by[0]
        if primary_blocker in track_map:
            track_idx = track_map[primary_blocker]
            tracks[track_idx].append(b.id)
            track_map[b.id] = track_idx
        else:
            # Blocker not in any track (cross-epic dep)
            # Create new track for this bead
            tracks.append([b.id])
            track_map[b.id] = len(tracks) - 1
    
    # Merge if exceeds max_workers
    while len(tracks) > max_workers:
        merge_smallest_two_tracks(tracks)
    
    return tracks

def merge_smallest_two_tracks(tracks):
    # Sort by length, merge two smallest
    tracks.sort(key=len)
    smallest = tracks.pop(0)
    tracks[0] = tracks[0] + smallest
```

### Example Transformation

**Input beads:**
- `bd-2`: ready (no blockers)
- `bd-3`: ready (no blockers)
- `bd-4`: blocked by `bd-2`
- `bd-5`: blocked by `bd-2`, `bd-3`

**Generated tracks (max_workers=3):**

| Track | Beads | Depends On |
|-------|-------|------------|
| 1 | bd-2, bd-4 | - |
| 2 | bd-3 | - |
| 3 | bd-5 | bd-2, bd-3 |

**Note:** `bd-5` depends on both `bd-2` and `bd-3`. The algorithm treats the first entry in `blocked_by` (here `bd-2`) as the primary blocker and ensures `bd-5` is scheduled only after that track completes, but it may place `bd-5` on its own track (Track 3 here) when it has multiple blockers from different tracks.

### Output Format

```markdown
## Track Assignments

| Track | Beads | Depends On |
|-------|-------|------------|
| 1 | bd-2, bd-4 | - |
| 2 | bd-3 | - |
| 3 | bd-5 | bd-2, bd-3 |
```

## Idempotency Check

Before running auto-orchestration, check:

```javascript
// Read metadata.json
const metadata = JSON.parse(fs.readFileSync('conductor/tracks/<track>/metadata.json'));

// Check if already orchestrated
if (metadata.beads?.orchestrated === true) {
    console.log("Already orchestrated, skipping");
    return;
}

// Proceed with orchestration...

// After successful dispatch, mark as orchestrated
metadata.beads.orchestrated = true;
metadata.beads.orchestratedAt = new Date().toISOString();
fs.writeFileSync('conductor/tracks/<track>/metadata.json', JSON.stringify(metadata, null, 2));
```

### metadata.json.beads Structure

```json
{
  "beads": {
    "status": "complete",
    "epicId": "bd-1",
    "epics": ["bd-1"],
    "issues": ["bd-2", "bd-3", "bd-4", "bd-5"],
    "planTasks": {
      "1.1": "bd-2",
      "1.2": "bd-3",
      "1.3": "bd-4",
      "1.4": "bd-5"
    },
    "orchestrated": false,
    "orchestratedAt": null,
    "reviewStatus": null
  }
}
```

## Worker Dispatch

After generating Track Assignments, call orchestrator:

### Dispatch Flow

```
1. Generate Track Assignments markdown
2. Inject into plan.md (or pass directly to orchestrator)
3. Call orchestrator skill with:
   - track_id
   - assignments table
   - max_workers limit
4. Orchestrator spawns workers per track
5. Workers execute beads in dependency order
6. Main agent monitors via bv --robot-status
```

### Orchestrator Integration

```markdown
<!-- AUTO-GENERATED: Do not edit manually -->
## Track Assignments

| Track | Beads | Depends On |
|-------|-------|------------|
| 1 | bd-2, bd-4 | - |
| 2 | bd-3 | - |
| 3 | bd-5 | bd-2, bd-3 |

_Generated by auto-orchestration at 2025-12-30T10:00:00Z_
```

### Worker Task Prompt Template

Each worker receives:

```markdown
## Task: Execute Track {N} Beads

Execute the following beads in order, respecting dependencies:
- {bead-ids}

### Protocol
1. For each bead: `bd show <id>` → implement → `bd close <id> --reason completed`
2. Use TDD cycle (RED-GREEN-REFACTOR) unless `--no-tdd`
3. Return structured result when complete

### Blocked Beads
Wait for dependencies before starting:
- {bead-id}: blocked by {blockers}
```

## Re-dispatch Loop (Wave Execution)

After workers complete a wave, **check for newly-unblocked beads and dispatch again**.

### Why This Matters

When beads have dependencies:
- Wave 1: Ready beads (1.1, 1.2, 1.3) execute in parallel
- Wave 1 completes → beads 2.1, 3.1 become unblocked
- **Without re-dispatch:** Agent falls back to sequential execution ❌
- **With re-dispatch:** New parallel wave spawns automatically ✓

### Re-dispatch Algorithm

```python
def execute_with_redispatch(epic_id, max_workers=3):
    wave = 1
    
    while True:
        # Get currently ready beads
        ready_beads = query_ready_beads(epic_id)
        
        if not ready_beads:
            # No more work - all beads completed or blocked
            break
        
        print(f"Wave {wave}: Dispatching {len(ready_beads)} beads")
        
        # Generate tracks for this wave
        tracks = generate_track_assignments(ready_beads, max_workers)
        
        # Spawn workers for this wave
        workers = spawn_workers(tracks)
        
        # Wait for all workers to complete
        wait_for_completion(workers)
        
        wave += 1
    
    # All waves complete - spawn rb for final review
    spawn_rb_subagent(epic_id)

def query_ready_beads(epic_id):
    """Query beads that are ready (not blocked, not closed)"""
    result = bash(f"bd ready --json")
    beads = json.loads(result)
    # Filter to this epic's beads only
    return [b for b in beads if b['epic'] == epic_id]
```

### Wave Execution Display

```text
┌─ WAVE EXECUTION ───────────────────────┐
│ Wave 1: 2 beads (bd-2, bd-3)           │
│   → Spawned 2 workers                  │
│   → All completed ✓                    │
├────────────────────────────────────────┤
│ Wave 2: 2 beads (bd-4, bd-5)           │
│   → Spawned 2 workers                  │
│   → All completed ✓                    │
├────────────────────────────────────────┤
│ Wave 3: 1 bead (bd-6)                  │
│   → Spawned 1 worker                   │
│   → Completed ✓                        │
├────────────────────────────────────────┤
│ All waves complete → Running rb        │
└────────────────────────────────────────┘
```

### State Tracking

Track wave progress in `metadata.json`:

```json
{
  "beads": {
    "orchestrated": true,
    "orchestratedAt": "2025-12-30T10:00:00Z",
    "waves": [
      {"wave": 1, "beads": ["bd-2", "bd-3", "bd-4"], "completedAt": "..."},
      {"wave": 2, "beads": ["bd-5", "bd-6"], "completedAt": "..."},
      {"wave": 3, "beads": ["bd-7"], "completedAt": "..."}
    ],
    "currentWave": null,
    "reviewStatus": "passed"
  }
}
```

## Final Review Phase

After **all waves** complete, spawn `rb` sub-agent:

### Trigger Condition

```python
# Check all waves completed (no more ready beads)
ready_beads = query_ready_beads(epic_id)
all_waves_done = len(ready_beads) == 0

if all_waves_done:
    spawn_rb_subagent(track_id)
```

### rb Sub-Agent Task

```markdown
## Task: Review Completed Beads

Run `rb` (review beads) to verify:
1. All beads properly closed
2. Code quality meets standards
3. Tests pass
4. No regressions introduced

### On Success
Mark track ready for `/conductor-finish`

### On Issues Found
Report issues, reopen affected beads if needed
```

### Post-Review State

```json
{
  "beads": {
    "orchestrated": true,
    "orchestratedAt": "2025-12-30T10:00:00Z",
    "reviewStatus": "passed",
    "reviewedAt": "2025-12-30T12:30:00Z"
  }
}
```

## Fallback: Sequential Execution

If Agent Mail MCP is unavailable (no parallel workers):

### Detection

```bash
# Check if agent_mail MCP available
mcp list | grep agent_mail
```

### Fallback Behavior

When Agent Mail is unavailable, the system halts orchestration (per maestro-core fallback policy). Manual sequential execution is possible:

```markdown
⚠️ Agent Mail unavailable - orchestration halted

Manual sequential execution if needed:
1. Get ready beads: `bd ready --json`
2. For each ready bead:
   a. Claim: `bd update <id> --status in_progress`
   b. Implement with TDD
   c. Close: `bd close <id> --reason completed`
3. Repeat until all beads closed
4. Run `rb` for final review
```

## Complete Flow Diagram

```
fb completes
    │
    ▼
Check metadata.json.beads.orchestrated
    │
    ├── true ──► Skip (already done)
    │
    └── false
          │
          ▼
    Check Agent Mail available?
          │
          ├── No ──► Sequential /conductor-implement
          │               │
          │               ▼
          │           Run rb manually
          │
          └── Yes
                │
                ▼
    ┌─────────────────────────────────────┐
    │         WAVE EXECUTION LOOP         │
    └─────────────────────────────────────┘
                │
                ▼
    Query: bd ready --json (filter to epic)
                │
                ├── No ready beads ──► Exit loop
                │
                └── Ready beads exist
                      │
                      ▼
                Generate Track Assignments for wave
                      │
                      ▼
                Dispatch parallel workers
                      │
                      ▼
                Workers execute tracks
                      │
                      ▼
                All workers complete
                      │
                      ▼
                Update metadata.json.beads.waves
                      │
                      └──► Loop back to query
                │
                ▼
    All waves complete
                │
                ▼
    Spawn rb sub-agent
                │
                ▼
    Review complete
```

## References

- [Orchestrator Skill](../../orchestrator/SKILL.md) - Multi-agent dispatch
- [Review Beads](REVIEW_BEADS.md) - `rb` command reference
- [File Beads](FILE_BEADS.md) - `fb` command reference

> **Cross-skill reference:** Load the [conductor](../../conductor/SKILL.md) skill for beads-integration details.
