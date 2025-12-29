# Orchestrator Workflow

6-phase protocol for multi-agent parallel execution.

## Phase 1: Read Plan

Parse Track Assignments from plan.md:

```python
# Read from conductor track
plan = Read("conductor/tracks/<track-id>/plan.md")
metadata = Read("conductor/tracks/<track-id>/metadata.json")

# Extract from Track Assignments section:
EPIC_ID = metadata.beads.epicId
TRACKS = parse_track_assignments(plan)
# Result:
# [
#   { track: 1, agent: "BlueLake", tasks: ["1.1.1", "1.1.2"], scope: "skills/orchestrator/**", depends_on: [] },
#   { track: 2, agent: "GreenCastle", tasks: ["2.1.1", "2.2.1"], scope: "skills/maestro-core/**", depends_on: ["1.2.3"] },
# ]

CROSS_DEPS = metadata.beads.crossTrackDeps
# [{ from: "1.2.3", to: "2.1.1" }]
```

### Parsing Track Assignments Table

| Track | Agent | Tasks | File Scope | Depends On |
|-------|-------|-------|------------|------------|
| 1 | BlueLake | 1.1.*, 1.2.* | skills/orchestrator/** | - |
| 2 | GreenCastle | 2.1.*, 2.2.* | skills/maestro-core/** | 1.2.3 |

Map tasks to bead IDs using `metadata.json.beads.planTasks`.

## Phase 2: Initialize Agent Mail

```python
# 1. Ensure project exists
ensure_project(human_key="<absolute-project-path>")

# 2. Register orchestrator
register_agent(
  project_key="<path>",
  name="<OrchestratorName>",  # Auto-generated or from config
  program="amp",
  model="<model>",
  task_description="Orchestrator for <epic-id>"
)

# 3. Create epic thread
send_message(
  project_key="<path>",
  sender_name="<OrchestratorName>",
  to=["<all-worker-names>"],
  thread_id="<epic-id>",
  subject="EPIC STARTED: <title>",
  body_md="Spawning workers for tracks..."
)
```

## Phase 3: Spawn Worker Subagents

Spawn all workers in parallel using Task() tool:

```python
# For each track in TRACKS:
Task(
  description="Worker {agent}: Track {track_n} - {description}",
  prompt=worker_prompt.format(
    AGENT_NAME=track.agent,
    TRACK_N=track.track,
    EPIC_ID=epic_id,
    TASK_LIST=", ".join(track.tasks),
    BEAD_LIST=", ".join([planTasks[t] for t in track.tasks]),
    FILE_SCOPE=track.scope,
    ORCHESTRATOR=orchestrator_name,
    PROJECT_PATH=project_path,
    DEPENDS_ON=track.depends_on
  )
)
```

See [worker-prompt.md](worker-prompt.md) for complete template.

### Parallel vs Sequential

- **Independent tracks**: Spawn all workers simultaneously
- **Dependent tracks**: Worker prompt includes dependency to wait for

Workers check inbox for dependency completion before starting blocked beads.

## Phase 4: Monitor Progress

Poll for updates while workers execute:

```python
while not all_complete:
  # Check for progress messages
  messages = search_messages(
    project_key="<path>",
    query=epic_id,
    limit=20
  )
  
  # Check for blockers
  blockers = fetch_inbox(
    project_key="<path>",
    agent_name="<OrchestratorName>",
    urgent_only=True
  )
  
  # Check bead status
  status = bash("bv --robot-triage --graph-root <epic-id> | jq '.quick_ref'")
  
  # Wait before next poll
  sleep(30)  # 30 second interval
```

### Progress Indicators

```text
📊 Epic Progress: 12/26 beads complete
├── Track 1 (BlueLake): 6/6 ✓
├── Track 2 (GreenCastle): 4/5 [~]
└── Track 3 (RedStone): 2/15 [~]
```

## Phase 5: Handle Cross-Track Issues

### Blocker Resolution

When worker reports blocker:

```python
# 1. Read blocker message
blocker = fetch_inbox(urgent_only=True)[0]

# 2. Assess and respond
reply_message(
  project_key="<path>",
  message_id=blocker.id,
  sender_name="<OrchestratorName>",
  body_md="Resolution: ..."
)
```

### File Conflict Resolution

When two workers need same file:

```python
send_message(
  project_key="<path>",
  sender_name="<OrchestratorName>",
  to=["<Holder>"],
  thread_id="<epic-id>",
  subject="File conflict resolution",
  body_md="<Requester> needs <files>. Can you release?"
)
```

### Cross-Track Dependency Notification

When Track 1 completes task needed by Track 2:

```python
# Worker 1 sends:
send_message(
  to=["<Worker2>"],
  thread_id="<epic-id>",
  subject="[DEP] 1.2.3 COMPLETE - Track 2 unblocked",
  body_md="Task 1.2.3 complete. Track 2 can proceed with 2.1.1."
)
```

## Phase 6: Epic Completion

### Verify All Complete

```python
# Check via beads
status = bash("bv --robot-triage --graph-root <epic-id> | jq '.quick_ref'")
assert status.open_count == 0

# Or via bd
open_beads = bash("bd list --status=open --parent=<epic-id> --json | jq 'length'")
assert open_beads == "0"
```

### Send Summary

```python
send_message(
  project_key="<path>",
  sender_name="<OrchestratorName>",
  to=all_workers,
  thread_id=epic_id,
  subject="EPIC COMPLETE: <title>",
  body_md="""
## Summary

- **Duration**: X hours
- **Tracks**: 3 complete
- **Beads**: 26 closed

### Per-Track Summary

#### Track 1 (BlueLake)
- Created skills/orchestrator/ directory structure
- Created SKILL.md, workflow.md, worker-prompt.md

#### Track 2 (GreenCastle)
- Updated maestro-core hierarchy
- Added /conductor-orchestrate routing

#### Track 3 (RedStone)
- Updated CODEMAPS
- Updated AGENTS.md

### Files Changed
- skills/orchestrator/SKILL.md
- skills/orchestrator/references/*.md
- skills/maestro-core/SKILL.md
- ...
"""
)
```

### Close Epic

```python
bash("bd close <epic-id> --reason 'All tracks complete'")
```

## Graceful Fallback

If Agent Mail MCP is unavailable:

```python
try:
  ensure_project(...)
except McpUnavailable:
  print("⚠️ Agent coordination unavailable - falling back to sequential")
  # Route to standard /conductor-implement
  return implement_sequential(track_id)
```

## Timing Constraints

| Constraint | Value | Action on Breach |
|------------|-------|------------------|
| Worker heartbeat | Every 5 min | Mark worker as stale after 10 min |
| Cross-dep timeout | 30 min | Escalate to orchestrator |
| Monitor interval | 30 sec | Poll inbox and beads |
| Total epic timeout | None | Manual intervention |

## State Tracking

Orchestrator maintains state in `implement_state.json`:

```json
{
  "execution_mode": "PARALLEL_DISPATCH",
  "orchestrator_name": "PurpleMountain",
  "workers": {
    "BlueLake": { "track": 1, "status": "complete", "beads_closed": 6 },
    "GreenCastle": { "track": 2, "status": "in_progress", "current_bead": "my-workflow:3-3cmw.14" },
    "RedStone": { "track": 3, "status": "waiting", "blocked_by": "my-workflow:3-3cmw.14" }
  },
  "started_at": "2025-12-30T01:30:00Z",
  "last_poll": "2025-12-30T02:15:00Z"
}
```
