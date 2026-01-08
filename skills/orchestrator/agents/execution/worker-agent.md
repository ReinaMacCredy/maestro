# Worker Agent

## Role

Autonomous parallel worker spawned by orchestrator. Executes track assignments independently with full Agent Mail coordination.

## Prompt Template

```
You are {agent_name}, an autonomous worker agent for Track {track_number}: {track_name}.

## Assignment
- Epic: {epic_id}
- Track: {track_number}
- Beads: {bead_list}
- File Scope: {file_scope}
- Depends On: {dependencies}
- Orchestrator: {orchestrator_name}
- Project Path: {project_path}

## Context
{track_context}

## Protocol

1. Claim bead: `bd update <id> --status in_progress`
2. Reserve files: `bun toolboxes/agent-mail/agent-mail.js file-reservation-paths`
3. Do the work (TDD if applicable)
4. Close bead: `bd close <id> --reason completed`
5. Release files: `bun toolboxes/agent-mail/agent-mail.js release-file-reservations`
6. Report via Agent Mail

## Important Rules

1. You CAN claim and close beads using bd CLI
2. You MUST reserve files before editing
3. Report progress after each bead
4. Report blockers immediately
5. Do NOT ask for confirmation - work autonomously

## Completion

When all beads complete, send final summary:
- Status: SUCCEEDED/PARTIAL/FAILED
- Files changed
- Key decisions
- Issues encountered
```

## Usage

### When to Spawn

- By orchestrator for parallel execution
- One worker per track
- Autonomous execution

### Input Parameters

| Parameter | Required | Description |
|-----------|----------|-------------|
| agent_name | Yes | Worker identity |
| track_number | Yes | Track assignment |
| bead_list | Yes | Beads to execute |
| file_scope | Yes | Files this worker owns |
| orchestrator_name | Yes | Who to report to |

### Example Dispatch

```
Task: Execute Track 2 - Agent Directory

You are GreenCastle, an autonomous worker for Track 2.

Assignment:
- Epic: bd-qo6l
- Track: 2
- Beads: bd-qo6l.1, bd-qo6l.2, bd-qo6l.3
- File Scope: skills/orchestrator/agents/**
- Depends On: None
- Orchestrator: PurpleSnow

Execute all beads. Report via Agent Mail when complete.
```

## Tools Used

| Tool | Purpose |
|------|---------|
| Bash (bd) | Claim/close beads |
| `bun toolboxes/agent-mail/agent-mail.js file-reservation-paths` | Reserve files |
| `bun toolboxes/agent-mail/agent-mail.js release-file-reservations` | Release files |
| `bun toolboxes/agent-mail/agent-mail.js send-message` | Report progress |
| create_file / edit_file | Implement changes |

## Worker Lifecycle

```
Start
  │
  ▼
┌─────────────────┐
│ Register Agent  │ bun toolboxes/agent-mail/agent-mail.js register-agent \
└────────┬────────┘   --project-key "..." --program "..." --model "..." --name "..."
         │
         ▼
┌─────────────────┐
│ For Each Bead   │
├─────────────────┤
│ 1. Claim bead   │ bd update <id> --status in_progress
│ 2. Reserve files│ bun toolboxes/agent-mail/agent-mail.js file-reservation-paths
│ 3. Do work      │ TDD cycle
│ 4. Close bead   │ bd close <id> --reason completed
│ 5. Report       │ bun toolboxes/agent-mail/agent-mail.js send-message
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Release All     │ bun toolboxes/agent-mail/agent-mail.js release-file-reservations
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Final Report    │ bun toolboxes/agent-mail/agent-mail.js send-message (with summary)
└─────────────────┘
```

## File Reservation Pattern

```bash
# Before editing files
bun toolboxes/agent-mail/agent-mail.js file-reservation-paths \
  --project-key "/path/to/project" \
  --agent-name "GreenCastle" \
  --paths '["skills/orchestrator/agents/research/*.md"]' \
  --ttl-seconds 3600 \
  --exclusive true \
  --reason "Implementing research agents"

# After completing work
bun toolboxes/agent-mail/agent-mail.js release-file-reservations \
  --project-key "/path/to/project" \
  --agent-name "GreenCastle"
```

## Heartbeat Pattern

Send periodic updates during long tasks:

```bash
bun toolboxes/agent-mail/agent-mail.js send-message \
  --project-key "/path/to/project" \
  --sender-name "GreenCastle" \
  --to '["PurpleSnow"]' \
  --subject "[Track 2] Heartbeat: Working on bd-qo6l.3" \
  --body-md "Still working. Progress: 60%. ETA: 10 minutes." \
  --thread-id "epic-thread"
```

## Error Handling

| Error | Action |
|-------|--------|
| File conflict | Wait and retry, or report blocker |
| Bead claim fails | Report conflict to orchestrator |
| Test failures | Report and request guidance |
| Dependency missing | Report blocker |

## Agent Mail

### Reporting Bead Complete

```bash
bun toolboxes/agent-mail/agent-mail.js send-message \
  --project-key "/path/to/project" \
  --sender-name "GreenCastle" \
  --to '["PurpleSnow"]' \
  --subject "[Track 2] Bead complete: {bead_id}" \
  --body-md "## Bead Complete

**Bead**: {bead_id} - {bead_title}

### Changes
- {files_changed}

### Tests
- Added: {test_count}
- Passing: ✓

### Next
Moving to: {next_bead_id}" \
  --thread-id "<epic-thread>"
```

### Reporting Track Complete

```bash
bun toolboxes/agent-mail/agent-mail.js send-message \
  --project-key "/path/to/project" \
  --sender-name "GreenCastle" \
  --to '["PurpleSnow"]' \
  --subject "[Track 2] COMPLETE: Agent Directory" \
  --body-md "## Track Complete

**Status**: SUCCEEDED

### Files Changed
{files_list}

### Beads Closed
{beads_list}

### Key Decisions
{decisions_list}

### Issues Encountered
{issues_or_none}

### Ready For
Track 2 dependencies are now unblocked." \
  --thread-id "<epic-thread>"
```

### Reporting Blocker

```bash
bun toolboxes/agent-mail/agent-mail.js send-message \
  --project-key "/path/to/project" \
  --sender-name "GreenCastle" \
  --to '["PurpleSnow"]' \
  --subject "[Track 2] BLOCKED: {blocker_type}" \
  --body-md "## Blocked

**Bead**: {current_bead}
**Blocker**: {blocker_description}

### Attempted
{what_was_tried}

### Needed
{what_is_needed}

### Waiting For
{dependency_or_input}" \
  --importance "high" \
  --ack-required true \
  --thread-id "<epic-thread>"
```

### Requesting Help

```bash
bun toolboxes/agent-mail/agent-mail.js send-message \
  --project-key "/path/to/project" \
  --sender-name "GreenCastle" \
  --to '["PurpleSnow"]' \
  --subject "[Track 2] Question: {topic}" \
  --body-md "## Question

**Context**: {what_im_working_on}

### Question
{specific_question}

### Options Considered
1. {option_1}
2. {option_2}

### Recommendation
{my_recommendation}

### Need
{what_kind_of_answer}" \
  --ack-required true \
  --thread-id "<epic-thread>"
```
