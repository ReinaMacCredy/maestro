# Orchestrator Workflow

8-phase protocol for multi-agent parallel execution.

## Phase 0: Preflight (Session Brain)

Before mode selection, run session coordination preflight.

**Trigger conditions:**
- `/conductor-implement`
- `/conductor-orchestrate`

**Skip conditions:**
- `ds` (design sessions always fresh)
- `bd ready`, `bd show`, `bd list` (query commands)

### 4-Step Protocol

1. **IDENTITY**: Generate session ID, register with Agent Mail
2. **DETECT**: fetch_inbox() for messages from last 30 min, parse [SESSION START], [HEARTBEAT], [SESSION END]
3. **DISPLAY**: Show active sessions, warn on conflicts (track/files/beads)
4. **PROCEED**: No conflicts → continue; Conflicts → prompt user; Stale → takeover prompt

### Timeout Behavior

Agent Mail timeout: 3 seconds
On timeout: Warn and proceed without coordination

### Message Subjects

- `[SESSION START] {display_name}` - Sent on preflight completion
- `[HEARTBEAT] Track {track}` - Sent every 5 min during work
- `[SESSION END] {display_name}` - Sent on session completion

### Conflict Types

| Type | Detection | User Options |
|------|-----------|--------------|
| Track | Same track as active session | [P]roceed / [S]witch / [W]ait |
| File | Overlapping file reservations | [P]roceed / [W]ait |
| Bead | Same bead claimed | Shows "claimed by X" |

### Stale Session Handling

Threshold: 10 minutes since last activity
Options: [T]ake over / [W]ait / [I]gnore

See [preflight.md](preflight.md) for details.

## Mode Selection (Pre-Phase)

Before starting phases, determine coordination mode:

```python
# Auto-select mode based on conditions
def select_mode(TRACKS, CROSS_DEPS):
    # Check Agent Mail availability
    try:
        health_check()
        agent_mail_available = True
    except:
        agent_mail_available = False
    
    # Mode selection logic
    if not agent_mail_available:
        return "LIGHT"  # Fallback - no Agent Mail
    elif len(CROSS_DEPS) > 0:
        return "FULL"   # Need coordination for cross-track deps
    elif all(estimate_duration(t) < 10 for t in TRACKS):
        return "LIGHT"  # Simple short tasks
    else:
        return "FULL"   # Default for complex work

MODE = select_mode(TRACKS, CROSS_DEPS)
```

| Mode | Phases Used | Agent Mail | Heartbeats |
|------|-------------|------------|------------|
| **LIGHT** | 1, 4, 7 (skip 2, 3, 5, 6) | No | No |
| **FULL** | All 7 phases | Yes | Yes (>10 min) |

## Phase Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  Phase 0: Preflight         - Session identity, detect active sessions     │
│  Phase 1: Read Plan         - Parse Track Assignments                      │
│  Phase 2: Validate          - Health check Agent Mail (FULL only)          │
│  Phase 3: Initialize        - Register orchestrator, create epic (FULL)    │
│  Phase 4: Spawn Workers     - Task() for each track (parallel)             │
│  Phase 5: Monitor + Verify  - Poll inbox, verify summaries (FULL only)     │
│  Phase 6: Handle Issues     - Resolve blockers, file conflicts (FULL only) │
│  Phase 7: Complete          - Verify, send summary, close epic, rb review  │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Phase 1: Read Plan (or Accept Auto-Generated)

**Option A: From plan.md (manual orchestration)**

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
#   { track: 2, agent: "GreenCastle", tasks: ["2.1.1", "2.2.1"], scope: "skills/design/**", depends_on: ["1.2.3"] },
# ]

CROSS_DEPS = metadata.beads.crossTrackDeps
# [{ from: "1.2.3", to: "2.1.1" }]
```

**Option B: From auto-orchestration (fb Phase 6)**

```python
# Assignments passed directly from fb
TRACKS = auto_generated_tracks  # Already in correct format
EPIC_ID = auto_generated_epic_id
CROSS_DEPS = auto_generated_cross_deps
```

Both options produce the same TRACKS structure for Phase 4.

### Parsing Track Assignments Table

| Track | Agent | Tasks | File Scope | Depends On |
|-------|-------|-------|------------|------------|
| 1 | BlueLake | 1.1.*, 1.2.* | skills/orchestrator/** | - |
| 2 | GreenCastle | 2.1.*, 2.2.* | skills/design/** | 1.2.3 |

Map tasks to bead IDs using `metadata.json.beads.planTasks`.

## Phase 2: Validate Agent Mail (NEW)

**Before spawning workers, verify Agent Mail is functional:**

```python
# Health check - HALT if unavailable
try:
    health_result = health_check(reason="Pre-spawn orchestrator validation")
    if not health_result.healthy:
        raise Exception("Agent Mail unhealthy")
except Exception as e:
    print("⚠️ Agent Mail unavailable - falling back to sequential")
    return implement_sequential(track_id)  # Route to /conductor-implement
```

**Why this matters:** Your demo showed workers executing without Agent Mail registration. This gate prevents that scenario.

### Validation Checklist

| Check | Action on Fail |
|-------|----------------|
| `health_check()` succeeds | Fall back to sequential |
| `macro_start_session()` succeeds | Fall back to sequential |

## Phase 3: Initialize Agent Mail

Use `macro_start_session` to combine project setup, agent registration, and file reservations in a single call:

```python
# Initialize orchestrator with single macro call
session = macro_start_session(
  human_key="<absolute-project-path>",
  program="amp",
  model="<model>",
  task_description="Orchestrator for <epic-id>",
  file_reservation_paths=["conductor/tracks/<track-id>/**"],  # Reserve planning files
  inbox_limit=10  # Get recent messages
)

# Extract session info
ORCHESTRATOR_NAME = session.agent.name
PROJECT_KEY = session.project.human_key

# Check for any conflicts from recent inbox
for msg in session.inbox:
    if "[SESSION START]" in msg.subject:
        print(f"⚠️ Active session detected: {msg.subject}")

# Create epic thread - send to self (orchestrator)
# Workers join thread via macro_start_session when spawned
send_message(
  project_key=PROJECT_KEY,
  sender_name=ORCHESTRATOR_NAME,
  to=[ORCHESTRATOR_NAME],  # Send to self - workers join via macro_start_session
  thread_id="<epic-id>",
  subject="EPIC STARTED: <title>",
  body_md="""
Spawning workers for parallel execution.

## Track Assignments
| Track | Agent | Scope |
|-------|-------|-------|
| 1 | BlueLake | skills/orchestrator/** |
| 2 | GreenCastle | skills/design/** |

Workers: Follow 4-step protocol in worker-prompt.md
"""
)
```

## Phase 4: Spawn Worker Subagents

### Pre-Dispatch: Assign in Beads

**Before spawning workers, update bead assignments:**

```python
# For each track, assign beads to worker
for track in TRACKS:
    for bead_id in track.beads:
        bash(f"bd update {bead_id} --assignee {track.agent}")
```

This ensures:
- Beads show correct assignee before worker starts
- Other sessions can see who owns what
- Auto-routing skips already-assigned tasks

### Typed ASSIGN Message Format

After updating beads, send typed ASSIGN message to each worker:

```python
for track in TRACKS:
    send_message(
        project_key=PROJECT_KEY,
        sender_name=ORCHESTRATOR_NAME,
        to=[track.agent],
        thread_id=EPIC_ID,
        subject="[ASSIGN] Track {track.letter}: {track.title}",
        body_md=f"""
## Assignment

- **Epic**: {EPIC_ID}
- **Track**: {track.letter} ({track.title})
- **Tasks**: {', '.join(track.tasks)}
- **Beads**: {', '.join(track.beads)}
- **File Scope**: {track.scope}
- **Orchestrator**: {ORCHESTRATOR_NAME}
- **Project Path**: {PROJECT_KEY}

## Context from Design/Spec

{track.context}

## ⚠️ CRITICAL: 4-Step Protocol

1. **INITIALIZE**: `macro_start_session()` with file_reservation_paths
2. **EXECUTE**: `bd update <bead> --status in_progress`, do work, `bd close`
3. **REPORT**: `send_message()` to orchestrator (MANDATORY)
4. **CLEANUP**: `release_file_reservations()`
"""
    )
```

### Epic Thread Creation

The orchestrator creates an epic thread before worker dispatch:

```python
# Create epic thread - orchestrator sends to self
send_message(
    project_key=PROJECT_KEY,
    sender_name=ORCHESTRATOR_NAME,
    to=[ORCHESTRATOR_NAME],
    thread_id=EPIC_ID,
    subject=f"[EPIC START] {epic_title}",
    body_md=f"""
## Epic Started: {epic_title}

Spawning {len(TRACKS)} workers for parallel execution.

### Track Assignments
| Track | Agent | Tasks | File Scope |
|-------|-------|-------|------------|
{track_table_rows}

Workers join this thread via their ASSIGN message.
"""
)
```

### Worker Pre-Registration

Workers self-register via `macro_start_session` when they start. The orchestrator does NOT pre-register—each worker's first action is:

```python
# Worker's first step (in their prompt)
session = macro_start_session(
    human_key=PROJECT_PATH,
    program="amp",
    model=MODEL,
    file_reservation_paths=ASSIGNED_FILE_SCOPE,
    task_description=f"Worker for Track {TRACK_LETTER}: {TRACK_TITLE}"
)
WORKER_NAME = session.agent.name
```

The worker then receives their ASSIGN message in their inbox on startup.

### Mode-Specific Behavior

| Aspect | LIGHT Mode | FULL Mode |
|--------|------------|-----------|
| Worker prompt | Light template (no Agent Mail) | Full 4-step template |
| Worker registration | Skip | Self-register via `macro_start_session` |
| File reservations | Skip (rely on scope isolation) | Via macro_start_session |
| Result collection | Task() return values | Agent Mail messages |
| TDD | Yes (default) | Yes (default) |

**Note:** Workers self-register when they start. The orchestrator does NOT pre-register workers—each worker calls `macro_start_session` as their first step, which handles registration automatically.

**TDD enforcement:** Workers follow RED → GREEN → REFACTOR cycle by default. Pass `--no-tdd` to disable.

See [conductor TDD references](../../conductor/references/tdd/) for:
- [cycle.md](../../conductor/references/tdd/cycle.md) - Full RED/GREEN/REFACTOR workflow
- [gates.md](../../conductor/references/tdd/gates.md) - Enforcement gates and anti-patterns

### Agent Routing

Before spawning, determine agent type based on task intent. See [agent-routing.md](agent-routing.md) for:
- Routing tables by category (Research, Review, Planning, Execution, Debug)
- Spawn patterns for each agent type
- File reservation patterns

### Spawn Logic (FULL Mode)

**⚠️ CRITICAL: Stagger Spawns to Prevent Resource Exhaustion**

When spawning multiple workers, avoid launching all simultaneously. Agent Mail uses file locks for archive operations, and concurrent lock acquisition can exhaust file descriptors (OSError: Too many open files).

**Staggering Strategy:**
- Spawn workers in batches of 2-3 at a time
- Wait for each batch to initialize before spawning next
- For 6+ workers: spawn 3, wait, spawn 3 more

```python
import os

# Staggered spawn - configurable batch size to prevent file descriptor exhaustion
# Tune based on host environment's ulimit settings
MAX_CONCURRENT_SPAWNS = int(os.environ.get("MAX_CONCURRENT_SPAWNS", 3))
expected_workers = []

for i in range(0, len(TRACKS), MAX_CONCURRENT_SPAWNS):
    batch = TRACKS[i:i + MAX_CONCURRENT_SPAWNS]
    
    # Spawn this batch in parallel
    for track in batch:
    agent_type = route_intent(track.description)
    spawn_pattern = get_spawn_pattern(agent_type)
    
    expected_workers.append(track.agent)
    
    Task(
      description=f"Worker {track.agent}: Track {track.track}",
      prompt=spawn_pattern.format(
        AGENT_NAME=track.agent,
        TRACK_N=track.track,
        EPIC_ID=epic_id,
        TASK_LIST=", ".join(track.tasks),
        BEAD_LIST=", ".join([planTasks[t] for t in track.tasks]),
        FILE_SCOPE=track.scope,
        ORCHESTRATOR=ORCHESTRATOR_NAME,  # From Phase 3
        PROJECT_PATH=project_path,
        DEPENDS_ON=track.depends_on,
        MODEL=model,
        MODE="FULL"  # Workers know to use Agent Mail
      )
    )
    
    # Wait for batch to initialize before spawning next batch
    # This prevents file descriptor exhaustion from concurrent lock acquisition
    if i + MAX_CONCURRENT_SPAWNS < len(TRACKS):
        print(f"⏳ Batch {i//MAX_CONCURRENT_SPAWNS + 1} spawned, waiting for initialization...")
        # The wait happens naturally as Task() calls return
```

### Spawn Logic (LIGHT Mode)

Simplified spawn without Agent Mail:

```python
# Collect results directly from Task() returns
worker_results = []

for track in TRACKS:
    result = Task(
      description=f"Worker {track.agent}: Track {track.track}",
      prompt=f"""
You are {track.agent}, a worker for Track {track.track}.

## Assignment
- **Beads**: {track.beads}
- **File Scope**: {track.scope}

## Protocol (LIGHT MODE - No Agent Mail)

1. **Execute beads:**
   ```bash
   bd update <bead-id> --status in_progress
   # ... do work ...
   bd close <bead-id> --reason completed
   ```

2. **Return structured result:**
   ```python
   return {{
       "status": "SUCCEEDED",
       "files_changed": [...],
       "key_decisions": [...],
       "issues": [],
       "beads_closed": [...]
   }}
   ```

NO Agent Mail registration, messaging, or heartbeats required.
"""
    )
    worker_results.append(result)

# Skip to Phase 7 with collected results
```

See [worker-prompt.md](worker-prompt.md) for the 4-step worker protocol (FULL mode).

### Parallel vs Sequential

- **Independent tracks**: Spawn all workers simultaneously
- **Dependent tracks**: Worker prompt includes dependency to wait for

Workers check inbox for dependency completion before starting blocked beads.

## Phase 5: Monitor Progress + Verify Summaries (ENHANCED)

Poll for updates while workers execute, **verify all workers sent summaries**, and re-dispatch when new beads become ready:

```python
wave = 1
active_workers = initial_workers  # From Phase 4
workers_with_summaries = set()

while active_workers or has_ready_beads(EPIC_ID):
    # Wait for current wave to complete
    wait_for_workers(active_workers)
    
    # ──────────────────────────────────────────────────────────
    # NEW: Verify workers sent summaries via Agent Mail
    # ──────────────────────────────────────────────────────────
    inbox = fetch_inbox(
        project_key="<path>",
        agent_name=ORCHESTRATOR_NAME,
        include_bodies=True,
        limit=50
    )
    
    for msg in inbox:
        if "[TRACK COMPLETE]" in msg.subject:
            # Extract agent name from message
            agent = msg.sender_name
            workers_with_summaries.add(agent)
            print(f"✓ Received summary from {agent}")
    
    # Check for missing summaries
    missing = set(expected_workers) - workers_with_summaries
    if missing and not active_workers:
        print(f"⚠️ Missing summaries from: {', '.join(missing)}")
        # Log but don't block - workers may have used fallback mode
    
    # ──────────────────────────────────────────────────────────
    
    # Check for blockers
    blockers = fetch_inbox(
        project_key="<path>",
        agent_name=ORCHESTRATOR_NAME,
        urgent_only=True
    )
    
    for blocker in blockers:
        handle_blocker(blocker)
    
    # Query for newly-ready beads (unblocked by completed work)
    ready_beads = bash(f"bd ready --json | jq '[.[] | select(.epic == \"{EPIC_ID}\")]'")
    
    if ready_beads:
        wave += 1
        print(f"Wave {wave}: {len(ready_beads)} beads now ready")
        
        # Generate new track assignments for this wave
        new_tracks = generate_track_assignments(ready_beads)
        expected_workers.extend([t.agent for t in new_tracks])
        
        # Spawn new workers
        active_workers = spawn_workers(new_tracks)
        
        # Update metadata
        update_wave_state(wave, ready_beads)
    else:
        active_workers = []  # No more work
```

### Wave Execution Display

```text
┌─ WAVE EXECUTION ───────────────────────┐
│ Wave 1: 3 beads (bd-2, bd-3, bd-4)     │
│   → Spawned 3 workers                  │
│   → All completed ✓                    │
│   → Summaries: 3/3 ✓                   │
├────────────────────────────────────────┤
│ Wave 2: 2 beads (bd-5, bd-6)           │
│   → Spawned 2 workers                  │
│   → All completed ✓                    │
│   → Summaries: 2/2 ✓                   │
├────────────────────────────────────────┤
│ All waves complete                     │
│ Total summaries: 5/5 ✓                 │
└────────────────────────────────────────┘
```

### Summary Verification Report (NEW)

After all workers complete:

```text
┌─ SUMMARY VERIFICATION ─────────────────┐
│ Expected: 5 workers                    │
│ Received: 5 summaries                  │
├────────────────────────────────────────┤
│ ✓ BlueLake     - SUCCEEDED             │
│ ✓ GreenCastle  - SUCCEEDED             │
│ ✓ RedStone     - SUCCEEDED             │
│ ✓ PurpleMoon   - PARTIAL (1 blocker)   │
│ ✓ OrangeStar   - SUCCEEDED             │
├────────────────────────────────────────┤
│ Status: All workers reported           │
└────────────────────────────────────────┘
```

### Why Wave Re-dispatch Matters

Without re-dispatch:
- Wave 1 workers complete → beads 2.1, 3.1 become unblocked
- Main agent falls back to sequential execution ❌

With re-dispatch:
- Wave 1 workers complete → check `bd ready --json`
- Newly-ready beads found → spawn Wave 2 workers ✓
- Continues until no more ready beads

### Progress Indicators

```text
📊 Epic Progress: 12/26 beads complete
├── Track 1 (BlueLake): 6/6 ✓
├── Track 2 (GreenCastle): 4/5 [~]
└── Track 3 (RedStone): 2/15 [~]
```

## Phase 6: Handle Cross-Track Issues

### Blocker Resolution

When worker reports blocker:

```python
# 1. Read blocker message
blocker = fetch_inbox(urgent_only=True)[0]

# 2. Assess and respond
reply_message(
  project_key="<path>",
  message_id=blocker.id,
  sender_name=ORCHESTRATOR_NAME,
  body_md="Resolution: ..."
)
```

### File Conflict Resolution

When two workers need same file:

```python
send_message(
  project_key="<path>",
  sender_name=ORCHESTRATOR_NAME,
  to=["<Holder>"],
  thread_id="<epic-id>",
  subject="File conflict resolution",
  body_md="<Requester> needs <files>. Can you release?"
)
```

### Cross-Track Dependency Notification

When Track 1 completes task needed by Track 2:

```python
# Worker 1 sends (handled by worker protocol):
send_message(
  to=["<Worker2>"],
  thread_id="<epic-id>",
  subject="[DEP] 1.2.3 COMPLETE - Track 2 unblocked",
  body_md="Task 1.2.3 complete. Track 2 can proceed with 2.1.1."
)
```

## Phase 7: Epic Completion

### Verify All Child Beads Closed

Before closing epic, verify no lingering beads:

```python
# Check for open child beads
open_beads = bash(f"bd list --parent={epic_id} --status=open --json | jq 'length'")

if int(open_beads) > 0:
    # List lingering beads
    lingering = bash(f"bd list --parent={epic_id} --status=open --json")
    print(f"⚠️ Lingering beads found: {open_beads}")
    print(lingering)
    
    # Prompt user
    choice = prompt("[C]lose all / [S]kip / [A]bort?")
    if choice == 'C':
        bash(f"bd close $(bd list --parent={epic_id} --status=open --json | jq -r '.[].id') --reason completed")
    elif choice == 'A':
        raise Exception("Aborted: lingering beads")
    # Skip continues to close epic
```

### Verify All Complete

```python
# Check via bd CLI
open_beads = bash("bd list --status=open --parent=<epic-id> --json | jq 'length'")
assert open_beads == "0"

# NEW: Verify summary coverage via Agent Mail
missing_summaries = set(expected_workers) - workers_with_summaries
if missing_summaries:
    print(f"⚠️ Workers without summaries: {missing_summaries}")
```

### Send Epic Complete Summary

```python
send_message(
  project_key="<path>",
  sender_name=ORCHESTRATOR_NAME,
  to=all_workers,
  thread_id=epic_id,
  subject="EPIC COMPLETE: <title>",
  body_md="""
## Summary

- **Duration**: X hours
- **Tracks**: 3 complete
- **Beads**: 26 closed
- **Summaries received**: 3/3 ✓

### Per-Track Summary

#### Track 1 (BlueLake)
- Created skills/orchestrator/ directory structure
- Created SKILL.md, workflow.md, worker-prompt.md

#### Track 2 (GreenCastle)
- Updated conductor routing
- Added /conductor-orchestrate routing

#### Track 3 (RedStone)
- Updated CODEMAPS
- Updated AGENTS.md

### Files Changed
- skills/orchestrator/SKILL.md
- skills/orchestrator/references/*.md
- skills/conductor/SKILL.md
- ...
"""
)
```

### Close Epic

```python
bash("bd close <epic-id> --reason 'All tracks complete'")
```

### Spawn rb Sub-Agent for Final Review

```python
Task(
  description="Final review: rb for epic <epic-id>",
  prompt="""
Run rb to review all completed beads for epic <epic-id>.

## Your Task
1. Verify all beads are properly closed
2. Check for any orphaned work or missing implementations
3. Validate acceptance criteria from spec.md
4. Report any issues or concerns

## Expected Output
Summary of review findings and overall quality assessment.
"""
)
```

### Review Completion

After rb finishes:
1. Collect review findings
2. Present completion summary to user
3. Suggest next steps (e.g., `/conductor-finish`)

```
┌─────────────────────────────────────────┐
│ ✓ Auto-Orchestration Complete           │
│                                         │
│ Workers: 3 spawned, 3 complete          │
│ Summaries: 3/3 received ✓               │
│ Beads: 26 closed                        │
│ Review: Passed                          │
│                                         │
│ → Next: /conductor-finish               │
└─────────────────────────────────────────┘
```

## Graceful Fallback

If Agent Mail MCP is unavailable at any phase:

```python
try:
    session = macro_start_session(human_key=project_path, program="amp", model=model)
except McpUnavailable:
    print("⚠️ Agent coordination unavailable - falling back to sequential")
    # Route to standard /conductor-implement
    return implement_sequential(track_id)
```

## Timing Constraints

| Constraint | Value | Action on Breach |
|------------|-------|------------------|
| Worker heartbeat | Every 5 min (optional for <10min tasks) | Mark worker as stale after 10 min |
| Cross-dep timeout | 30 min | Escalate to orchestrator |
| Monitor interval | 30 sec | Poll inbox and beads |
| Summary timeout | 2 min after Task completes | Log warning, continue |
| Total epic timeout | None | Manual intervention |

## State Tracking

Orchestrator maintains state in `implement_state.json`:

```json
{
  "execution_mode": "PARALLEL_DISPATCH",
  "orchestrator_name": "PurpleMountain",
  "expected_workers": ["BlueLake", "GreenCastle", "RedStone"],
  "workers_with_summaries": ["BlueLake", "GreenCastle"],
  "workers": {
    "BlueLake": { "track": 1, "status": "complete", "beads_closed": 6, "summary_received": true },
    "GreenCastle": { "track": 2, "status": "complete", "beads_closed": 5, "summary_received": true },
    "RedStone": { "track": 3, "status": "in_progress", "current_bead": "my-workflow:3-3cmw.14", "summary_received": false }
  },
  "waves": [
    { "wave": 1, "beads": ["bd-2", "bd-3", "bd-4"], "status": "complete" },
    { "wave": 2, "beads": ["bd-5", "bd-6"], "status": "complete" }
  ],
  "started_at": "2025-12-30T01:30:00Z",
  "last_poll": "2025-12-30T02:15:00Z"
}
```

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `MAX_CONCURRENT_SPAWNS` | `3` | Maximum workers to spawn per batch. Tune based on `ulimit -n` (file descriptors). Lower on constrained systems. |

## References

- [agent-routing.md](agent-routing.md) - Agent routing tables, spawn patterns, file reservations
- [worker-prompt.md](worker-prompt.md) - 4-step worker protocol with mandatory summary
- [summary-protocol.md](summary-protocol.md) - Required summary format for all agents
- [intent-routing.md](intent-routing.md) - Intent → agent type mappings
- [Agent Directory](../agents/README.md) - Available agent types and profiles
