# Example: Orchestrate Three-Track Epic

Complete example of parallel dispatch for a 3-track epic.

## Scenario

Implementing orchestrator skill with these tracks:

| Track | Agent | Tasks | File Scope | Depends On |
|-------|-------|-------|------------|------------|
| 1 | BlueLake | 1.1.*, 1.2.* | skills/orchestrator/** | - |
| 2 | GreenCastle | 2.1.*, 2.2.* | skills/design/** | 1.2.3 |
| 3 | RedStone | 3.*, 4.* | conductor/CODEMAPS/** | 2.2.2 |

## Phase 1: Read Plan

```python
plan = Read("conductor/tracks/orchestrator-skill_20251230/plan.md")
metadata = Read("conductor/tracks/orchestrator-skill_20251230/metadata.json")

TRACKS = [
  { "track": 1, "agent": "BlueLake", "tasks": ["1.1.1", "1.1.2", "1.2.1"], "scope": "skills/orchestrator/**", "depends_on": [] },
  { "track": 2, "agent": "GreenCastle", "tasks": ["2.1.1", "2.2.1"], "scope": "skills/design/**", "depends_on": ["1.2.3"] },
  { "track": 3, "agent": "RedStone", "tasks": ["3.1.1", "4.1.1"], "scope": "conductor/CODEMAPS/**", "depends_on": ["2.2.2"] }
]

EPIC_ID = metadata["beads"]["epicId"]  # "my-workflow:3-3cmw"
```

## Phase 2: Initialize Agent Mail

```python
# Initialize project and register orchestrator
ensure_project(human_key="/path/to/workspace")

register_agent(
  project_key="/path/to/workspace",
  name="PurpleMountain",
  program="amp",
  model="claude-sonnet-4-20250514",
  task_description="Orchestrator for my-workflow:3-3cmw"
)

# Announce epic start to all workers
send_message(
  project_key="/path/to/workspace",
  sender_name="PurpleMountain",
  to=["BlueLake", "GreenCastle", "RedStone"],
  thread_id="my-workflow:3-3cmw",
  subject="EPIC STARTED: Orchestrator Skill",
  body_md="Spawning 3 workers for parallel execution..."
)
```

## Phase 3: Spawn Workers

Workers follow the simplified 4-step protocol from [worker-prompt.md](../worker-prompt.md):

1. **INITIALIZE** - `macro_start_session()` (FIRST action, no exceptions)
2. **EXECUTE** - `bd update` → work → `bd close` for each bead
3. **REPORT** - `send_message()` (MANDATORY before returning)
4. **CLEANUP** - `release_file_reservations()`

```python
# Pre-register ALL workers before spawning (prevents "recipient not found" errors)
for track in TRACKS:
    register_agent(
        project_key="/path/to/workspace",
        name=track.agent,
        program="amp",
        model="claude-sonnet-4-20250514",
        task_description=f"Worker for Track {track.track}"
    )

# Now spawn workers in parallel
Task(
  description="Worker BlueLake: Track 1 - Skill Setup",
  prompt="""
You are BlueLake, an autonomous worker agent for Track 1.

## Assignment
- **Epic**: my-workflow:3-3cmw
- **Track**: 1
- **Beads**: my-workflow:3-3cmw.1, my-workflow:3-3cmw.2, my-workflow:3-3cmw.3
- **File Scope**: skills/orchestrator/**
- **Orchestrator**: PurpleMountain
- **Project Path**: /path/to/workspace

## ⚠️ CRITICAL: 4-Step Protocol (MANDATORY)

### STEP 1: INITIALIZE (FIRST ACTION - NO EXCEPTIONS)
macro_start_session(
  human_key="/path/to/workspace",
  program="amp",
  model="claude-sonnet-4-20250514",
  agent_name="BlueLake",
  file_reservation_paths=["skills/orchestrator/**"],
  task_description="Worker for Track 1"
)

### STEP 2: EXECUTE
for bead_id in ["my-workflow:3-3cmw.1", "my-workflow:3-3cmw.2", "my-workflow:3-3cmw.3"]:
    bash(f"bd update {bead_id} --status in_progress")
    # ... do work ...
    bash(f"bd close {bead_id} --reason completed")

### STEP 3: REPORT (MANDATORY - send_message BEFORE returning)
send_message(
  project_key="/path/to/workspace",
  sender_name="BlueLake",
  to=["PurpleMountain"],
  thread_id="my-workflow:3-3cmw",
  subject="[TRACK COMPLETE] Track 1",
  body_md="## Status\nSUCCEEDED\n\n## Files Changed\n- skills/orchestrator/SKILL.md (added)\n..."
)

### STEP 4: CLEANUP
release_file_reservations(project_key="/path/to/workspace", agent_name="BlueLake")
  """
)

# Similar for GreenCastle (Track 2) and RedStone (Track 3)
# Key difference: they check inbox for [DEP] messages before starting

```

## Phase 4: Monitor Progress

```python
state = {
  "BlueLake": { "status": "in_progress", "beads_closed": 0 },
  "GreenCastle": { "status": "waiting", "blocked_by": "1.2.3" },
  "RedStone": { "status": "waiting", "blocked_by": "2.2.2" }
}

while not all_complete():
    # Check for progress
    messages = search_messages(
        project_key="/path/to/workspace",
        query="my-workflow:3-3cmw COMPLETE",
        limit=50
    )
    
    # Update state from messages
    for msg in messages:
        if "[COMPLETE]" in msg.subject:
            update_bead_count(msg)
        if "[TRACK COMPLETE]" in msg.subject:
            mark_track_complete(msg.sender)
    
    # Check for blockers
    urgent = fetch_inbox(urgent_only=True)
    for blocker in urgent:
        handle_blocker(blocker)
    
    # Display progress
    print(f"📊 Epic Progress: {total_closed}/{total_beads}")
    
    sleep(30)
```

## Phase 5: Handle Blockers

### Cross-Track Dependency Timeout

```python
# GreenCastle waiting >30 min for BlueLake
send_message(
  to=["BlueLake"],
  subject="[PING] Task 1.2.3 status?",
  body_md="GreenCastle waiting. Please update status.",
  importance="urgent"
)
```

### File Conflict

```python
# RedStone needs AGENTS.md but BlueLake has it
send_message(
  to=["BlueLake"],
  subject="File conflict: AGENTS.md",
  body_md="RedStone needs AGENTS.md. Can you release?",
  importance="high"
)
```

## Phase 6: Complete

```python
# Verify all beads closed
status = bash("bd list --parent=my-workflow:3-3cmw --status=open --json | jq 'length'")
assert status == "0"

# Send summary
send_message(
  to=["BlueLake", "GreenCastle", "RedStone"],
  thread_id="my-workflow:3-3cmw",
  subject="EPIC COMPLETE: Orchestrator Skill",
  body_md="""
## Summary

- **Duration**: 2.5 hours
- **Tracks**: 3 complete
- **Beads**: 26 closed

### Per-Track
- Track 1 (BlueLake): 6/6 ✅
- Track 2 (GreenCastle): 5/5 ✅
- Track 3 (RedStone): 15/15 ✅
  """
)

# Close epic
bash("bd close my-workflow:3-3cmw --reason 'All tracks complete'")
```

## Timeline

```
0:00  - Spawn workers
0:05  - BlueLake starts Track 1
0:05  - GreenCastle, RedStone waiting
0:45  - BlueLake completes 1.2.3, notifies GreenCastle
0:50  - GreenCastle starts Track 2
1:30  - GreenCastle completes 2.2.2, notifies RedStone
1:35  - RedStone starts Track 3
1:45  - BlueLake completes Track 1
2:00  - GreenCastle completes Track 2
2:30  - RedStone completes Track 3
2:30  - Epic complete
```
