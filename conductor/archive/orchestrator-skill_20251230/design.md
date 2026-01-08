# DESIGN: Orchestrator Skill (Maestro Integration)

## Overview

Skill for orchestrating multi-agent parallel execution with autonomous workers,
integrated into the maestro workflow.

## Architecture (Mode B: Autonomous)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              ORCHESTRATOR                                   │
│                              (Main Agent)                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│  1. Read plan.md (from conductor/tracks/<id>/)                               │
│  2. Initialize Agent Mail                                                   │
│  3. Spawn worker subagents via Task()                                       │
│  4. Monitor progress via Agent Mail                                         │
│  5. Handle cross-track blockers                                             │
│  6. Announce completion                                                     │
└─────────────────────────────────────────────────────────────────────────────┘
           │
           │ Task() spawns parallel workers
           ▼
┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐
│  BlueLake        │  │  GreenCastle     │  │  RedStone        │
│  Track 1         │  │  Track 2         │  │  Track 3         │
│  [a → b → c]     │  │  [x → y]         │  │  [m → n → o]     │
├──────────────────┤  ├──────────────────┤  ├──────────────────┤
│  For each bead:  │  │  For each bead:  │  │  For each bead:  │
│  • Reserve files │  │  • Reserve files │  │  • Reserve files │
│  • Do work       │  │  • Do work       │  │  • Do work       │
│  • Report mail   │  │  • Report mail   │  │  • Report mail   │
│  • Next bead     │  │  • Next bead     │  │  • Next bead     │
└──────────────────┘  └──────────────────┘  └──────────────────┘
           │                   │                   │
           └───────────────────┼───────────────────┘
                               ▼
                    ┌─────────────────────┐
                    │     Agent Mail      │
                    │  ─────────────────  │
                    │  Epic Thread:       │
                    │  • Progress reports │
                    │  • Bead completions │
                    │  • Blockers         │
                    │                     │
                    │  Track Threads:     │
                    │  • Bead context     │
                    │  • Learnings        │
                    └─────────────────────┘
```

## Skill Structure

```
skills/orchestrator/
├── SKILL.md                        # Main skill
└── references/
    ├── workflow.md                 # 6-phase protocol
    ├── worker-prompt.md            # Worker template
    ├── preparation.md              # bv --robot-triage
    ├── monitoring.md               # Agent Mail monitoring
    └── patterns/                   # (from coordination/)
        ├── parallel-dispatch.md
        ├── session-lifecycle.md
        └── graceful-fallback.md
```

## Workflow Phases

### Phase 1: Read Plan

```python
# Read from conductor track
Read("conductor/tracks/<track-id>/plan.md")

# Extract from Track Assignments section:
EPIC_ID = metadata.json.beads.epic_id
TRACKS = [
  { agent: "BlueLake", beads: ["bd-101", "bd-102"], scope: "src/api/**" },
  { agent: "GreenCastle", beads: ["bd-201"], scope: "src/web/**" },
]
CROSS_DEPS = { "bd-201": ["bd-102"] }
```

### Phase 2: Initialize Agent Mail

```python
ensure_project(human_key="<absolute-project-path>")

register_agent(
  project_key="<path>",
  name="<OrchestratorName>",
  program="amp",
  model="<model>",
  task_description="Orchestrator for <epic-id>"
)
```

### Phase 3: Spawn Worker Subagents

```python
Task(
  description="Worker BlueLake: Track 1 - <description>",
  prompt=WORKER_PROMPT.format(
    AGENT_NAME="BlueLake",
    TRACK_N=1,
    EPIC_ID=epic_id,
    BEAD_LIST="bd-101, bd-102",
    FILE_SCOPE="src/api/**",
    ORCHESTRATOR="PurpleMountain",
    PROJECT_PATH=project_path
  )
)
```

### Phase 4: Monitor Progress

```python
while not all_complete:
  messages = search_messages(project_key, query=epic_id, limit=20)
  blockers = fetch_inbox(project_key, agent_name, urgent_only=True)
  status = bash("bv --robot-triage --graph-root <epic-id> | jq '.quick_ref'")
```

### Phase 5: Handle Cross-Track Issues

```python
reply_message(message_id=blocker_msg_id, body_md="Resolution: ...")

send_message(
  to=["<Holder>"],
  thread_id="<epic-id>",
  subject="File conflict resolution",
  body_md="<Worker> needs <files>. Can you release?"
)
```

### Phase 6: Epic Completion

```python
open_count = bash("bv --robot-triage --graph-root <epic-id> | jq '.quick_ref.open_count'")
assert open_count == "0"

send_message(to=all_workers, thread_id=epic_id, subject="EPIC COMPLETE", body_md=summary)
bash("bd close <epic-id> --reason 'All tracks complete'")
```

## Worker Prompt Template

```markdown
You are agent {AGENT_NAME} working on Track {TRACK_N} of epic {EPIC_ID}.

## Setup
1. Read {PROJECT_PATH}/AGENTS.md for context
2. This is autonomous mode - you have full control

## Your Assignment
- Track: {TRACK_N}
- Beads (in order): {BEAD_LIST}
- File scope: {FILE_SCOPE}
- Epic thread: {EPIC_ID}
- Track thread: track:{AGENT_NAME}:{EPIC_ID}

## Protocol for EACH bead:

### 1. START BEAD
register_agent(project_key="{PROJECT_PATH}", name="{AGENT_NAME}", task_description="{BEAD_ID}")
summarize_thread(thread_id="track:{AGENT_NAME}:{EPIC_ID}")
file_reservation_paths(paths=["{FILE_SCOPE}"], reason="{BEAD_ID}")
bd update {BEAD_ID} --status in_progress

### 2. WORK
- Implement the bead requirements
- Check inbox periodically: fetch_inbox(agent_name="{AGENT_NAME}")
- If blocked: send_message to orchestrator with importance="high"

### 3. COMPLETE BEAD
bd close {BEAD_ID} --reason "Summary of work"

send_message(
  project_key="{PROJECT_PATH}",
  sender_name="{AGENT_NAME}",
  to=["{ORCHESTRATOR}"],
  thread_id="{EPIC_ID}",
  subject="[{BEAD_ID}] COMPLETE",
  body_md="Done: <summary>. Next: <next-bead>"
)

send_message(
  project_key="{PROJECT_PATH}",
  sender_name="{AGENT_NAME}",
  to=["{AGENT_NAME}"],
  thread_id="track:{AGENT_NAME}:{EPIC_ID}",
  subject="{BEAD_ID} Context",
  body_md="## Learnings\n- ...\n## Gotchas\n- ...\n## Next\n- ..."
)

release_file_reservations()

### 4. NEXT BEAD
- Read track thread for context
- Loop to START BEAD

## When Track Complete
send_message(to=["{ORCHESTRATOR}"], thread_id="{EPIC_ID}", subject="[Track {TRACK_N}] COMPLETE")
Return summary of all work completed.

## Important
- ALWAYS read track thread before each bead
- ALWAYS write context after each bead
- Report blockers immediately
```

## plan.md Extended Format

```markdown
# Implementation Plan: <Title>

## Orchestration Config

epic_id: bd-xxx
max_workers: 3
mode: autonomous

## Track Assignments

| Track | Agent | Beads | File Scope | Depends On |
|-------|-------|-------|------------|------------|
| 1 | BlueLake | bd-101, bd-102 | src/api/** | - |
| 2 | GreenCastle | bd-201, bd-202 | src/web/** | bd-102 |
| 3 | RedStone | bd-301 | docs/** | bd-202 |

### Cross-Track Dependencies
- Track 2 waits for bd-102 (from Track 1)
- Track 3 waits for bd-202 (from Track 2)

---

## Phase 1: ...
```

## maestro-core Updates

### Skill Hierarchy (add Level 3)

| Level | Skill | Role |
|-------|-------|------|
| 1 | maestro-core | Routing decisions, fallback policy |
| 2 | conductor | Track orchestration, workflow state |
| **3** | **orchestrator** | **Multi-agent parallel execution** |
| 4 | design | Design sessions (Double Diamond) |
| 5 | beads | Issue tracking, dependencies |
| 6 | specialized | worktrees, sharing, writing |

### Command Routing (add row)

| Command | Routes To | Execution |
|---------|-----------|-----------|
| `/conductor-orchestrate` | orchestrator | Multi-agent parallel execution |

### Trigger Disambiguation (add)

| Trigger | Context | Routes To |
|---------|---------|-----------|
| `/conductor-orchestrate` | Track has Track Assignments | orchestrator |
| "run parallel" / "spawn workers" | Any | orchestrator |

## Quick Reference

| Phase | Action |
|-------|--------|
| Read Plan | Read("conductor/tracks/<id>/plan.md") |
| Initialize | ensure_project, register_agent |
| Spawn | Task() for each track (parallel) |
| Monitor | fetch_inbox, search_messages |
| Resolve | reply_message for blockers |
| Complete | Verify, send summary, close epic |

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| 1 | `/conductor-orchestrate` spawns parallel workers |
| 2 | Workers self claim/close beads via bd |
| 3 | Agent Mail messages for progress/context |
| 4 | Cross-track deps handled correctly |
| 5 | Graceful fallback if Agent Mail unavailable |
| 6 | plan.md Track Assignments format works |
| 7 | maestro-core routing updated |

## Files to Create/Modify

### Create
- `skills/orchestrator/SKILL.md`
- `skills/orchestrator/references/workflow.md`
- `skills/orchestrator/references/worker-prompt.md`
- `skills/orchestrator/references/preparation.md`
- `skills/orchestrator/references/monitoring.md`
- `skills/orchestrator/references/patterns/parallel-dispatch.md`
- `skills/orchestrator/references/patterns/session-lifecycle.md`
- `skills/orchestrator/references/patterns/graceful-fallback.md`

### Modify
- `skills/maestro-core/SKILL.md` (add hierarchy + routing)
- `skills/maestro-core/references/hierarchy.md`
- `skills/maestro-core/references/routing.md`
- `conductor/CODEMAPS/overview.md`
- `conductor/CODEMAPS/skills.md`

### Move (from coordination/)
- `skills/conductor/references/coordination/patterns/*` → orchestrator/references/patterns/
- `skills/conductor/references/coordination/examples/*` → orchestrator/references/examples/
