# Example: Orchestrate Three-Track Epic

Complete example of parallel dispatch for a 3-track epic.

## Scenario

Implementing orchestrator skill with these tracks:

| Track | Agent | Tasks | File Scope | Depends On |
|-------|-------|-------|------------|------------|
| 1 | BlueLake | 1.1.*, 1.2.* | skills/orchestrator/** | - |
| 2 | GreenCastle | 2.1.*, 2.2.* | skills/maestro-core/** | 1.2.3 |
| 3 | RedStone | 3.*, 4.* | conductor/CODEMAPS/** | 2.2.2 |

## Phase 1: Read Plan

```python
plan = Read("conductor/tracks/orchestrator-skill_20251230/plan.md")
metadata = Read("conductor/tracks/orchestrator-skill_20251230/metadata.json")

TRACKS = [
  { "track": 1, "agent": "BlueLake", "tasks": ["1.1.1", "1.1.2", "1.2.1"], "scope": "skills/orchestrator/**", "depends_on": [] },
  { "track": 2, "agent": "GreenCastle", "tasks": ["2.1.1", "2.2.1"], "scope": "skills/maestro-core/**", "depends_on": ["1.2.3"] },
  { "track": 3, "agent": "RedStone", "tasks": ["3.1.1", "4.1.1"], "scope": "conductor/CODEMAPS/**", "depends_on": ["2.2.2"] }
]

EPIC_ID = metadata["beads"]["epicId"]  # "my-workflow:3-3cmw"
```

## Phase 2: Initialize Agent Mail

```python
ensure_project(human_key="/path/to/workspace")

register_agent(
  project_key="/path/to/workspace",
  name="PurpleMountain",
  program="amp",
  model="claude-4-sonnet",
  task_description="Orchestrator for my-workflow:3-3cmw"
)

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

```python
# All Task() calls in single message for parallel execution

Task(
  description="Worker BlueLake: Track 1 - Skill Setup",
  prompt="""
You are BlueLake, a worker agent for Track 1.

**Assignment:**
- Track: 1
- Tasks: 1.1.1, 1.1.2, 1.2.1, 1.2.2, 1.2.3
- File Scope: skills/orchestrator/**
- Depends On: (none)

**Protocol:**
1. register_agent(name="BlueLake")
2. For each task: bd update → work → bd close → send_message
3. Heartbeat every 5 minutes
4. When done: send track summary, release reservations

**Cross-Track Notification:**
When you complete task 1.2.3, send:
  Subject: [DEP] 1.2.3 COMPLETE - Track 2 unblocked
  To: GreenCastle
  """
)

Task(
  description="Worker GreenCastle: Track 2 - maestro-core Integration",
  prompt="""
You are GreenCastle, a worker agent for Track 2.

**Assignment:**
- Track: 2
- Tasks: 2.1.1, 2.1.2, 2.2.1, 2.2.2, 2.2.3
- File Scope: skills/maestro-core/**
- Depends On: 1.2.3

**Protocol:**
1. register_agent(name="GreenCastle")
2. Check inbox for [DEP] 1.2.3 COMPLETE before starting
3. For each task: bd update → work → bd close → send_message
4. Heartbeat every 5 minutes
5. When done: send track summary, release reservations

**Cross-Track Notification:**
When you complete task 2.2.2, send:
  Subject: [DEP] 2.2.2 COMPLETE - Track 3 unblocked
  To: RedStone
  """
)

Task(
  description="Worker RedStone: Track 3 - CODEMAPS & Docs",
  prompt="""
You are RedStone, a worker agent for Track 3.

**Assignment:**
- Track: 3
- Tasks: 3.1.1, 3.1.2, 3.2.1, 3.2.2, 4.1.1, 4.1.2, 4.2.1, 4.2.2
- File Scope: conductor/CODEMAPS/**, AGENTS.md
- Depends On: 2.2.2

**Protocol:**
1. register_agent(name="RedStone")
2. Check inbox for [DEP] 2.2.2 COMPLETE before starting
3. For each task: bd update → work → bd close → send_message
4. Heartbeat every 5 minutes
5. When done: send track summary, release reservations
  """
)
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
