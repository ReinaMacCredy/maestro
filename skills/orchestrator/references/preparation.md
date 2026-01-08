# Preparation Phase - "Dọn Cỗ"

> **Before dispatching workers, prepare beads with proper dependencies and assignments.**

## Overview

"Dọn cỗ" (Vietnamese: "prepare the feast") - set up beads so workers can claim and execute autonomously.

## Prerequisites

- Plan.md with Track Assignments section
- Beads filed from plan (`fb` command)
- `bd` CLI available
- Agent Mail available (verified via `agent-mail.js health-check`)

## Preparation Steps

### 1. Initialize Agent Mail

Before any orchestration, verify Agent Mail is available:

```bash
# Check Agent Mail health
result=$(toolboxes/agent-mail/agent-mail.js health-check reason:"Orchestrator preflight - preparation phase")
if [ $? -ne 0 ]; then
    echo "❌ HALT: Agent Mail unavailable - cannot orchestrate"
    exit 1
fi

# Ensure project exists
toolboxes/agent-mail/agent-mail.js ensure-project human_key:"$PROJECT_PATH"
```

### 2. Triage Beads

Use `bd ready` to assess bead readiness:

```bash
bd ready --json
```

Output structure:
```json
{
  "ready": [
    {
      "id": "my-workflow:3-3cmw.1",
      "title": "Task 1.1.1",
      "status": "open",
      "blocked_by": []
    }
  ],
  "blocked": [
    {
      "id": "my-workflow:3-3cmw.5",
      "title": "Task 1.2.1",
      "blocked_by": ["my-workflow:3-3cmw.1"]
    }
  ]
}
```

### 2. Map Tasks to Beads

Extract `planTasks` mapping from metadata.json:

```python
metadata = Read("conductor/tracks/<track-id>/metadata.json")
plan_tasks = metadata.beads.planTasks
# { "1.1.1": "my-workflow:3-3cmw.1", "1.1.2": "my-workflow:3-3cmw.2", ... }
```

### 3. Verify Cross-Track Dependencies

Check that cross-track dependencies are wired:

```bash
bd show <bead-id> --json | jq '.dependencies'
```

If dependencies missing, add them:

```bash
bd dep add <child-bead> <parent-bead>
```

### 4. Register Workers with Agent Mail

Before spawning, pre-register all workers:

```bash
# Register each worker
for track in tracks; do
    toolboxes/agent-mail/agent-mail.js register-agent \
        project_key:"$PROJECT_PATH" \
        program:"amp" \
        model:"$MODEL" \
        name:"${track.agent_name}" \
        task_description:"Worker for Track ${track.number}: ${track.description}"
done
```

Plan.md Track Assignments specifies worker assignments:

| Track | Agent | Tasks | File Scope | Depends On |
|-------|-------|-------|------------|------------|
| 1 | BlueLake | 1.1.*, 1.2.* | skills/orchestrator/** | - |
| 2 | GreenCastle | 2.1.*, 2.2.* | skills/design/** | 1.2.3 |

Orchestrator parses this table to build worker assignments.

### 5. Reserve Files for Workers

Set up file reservations to prevent conflicts:

```bash
# Reserve files for each worker
for track in tracks; do
    toolboxes/agent-mail/agent-mail.js file-reservation-paths \
        project_key:"$PROJECT_PATH" \
        agent_name:"${track.agent_name}" \
        paths:"[\"${track.file_scope}\"]" \
        ttl_seconds:7200 \
        exclusive:true \
        reason:"Track ${track.number}: ${track.description}"
done
```

### 6. Validate Ready State

Before spawning workers, verify:

```bash
# All beads exist
bd list --parent=<epic-id> --json | jq 'length'
# Expected: matches plan task count

# At least one bead per track is ready
bd ready --json | jq '.ready[].id'
```

## Preparation Output

After preparation, orchestrator has:

1. **TRACKS** - List of track assignments with bead IDs
2. **CROSS_DEPS** - Cross-track dependency map
3. **READY_BEADS** - Beads ready to work (no blockers)
4. **plan_tasks** - Task ID to bead ID mapping

## Troubleshooting

### No Beads Found

```
❌ No beads found for epic <id>
```

Run `fb` to file beads from plan:

```bash
fb  # Opens bead filing workflow
```

### Dependencies Missing

```
⚠️ Cross-track dependency not wired: 1.2.3 → 2.1.1
```

Wire manually:

```bash
bd dep add <child-bead-id> <parent-bead-id>
```

### Worker Track Has No Ready Beads

```
⚠️ Track 2 has no ready beads - all blocked
```

Check dependencies:

```bash
bd show <bead-id> --json | jq '.blocked_by'
```

Worker will wait for dependency notification via Agent Mail.

## Best Practices

1. **File beads before orchestration** - Run `fb` before `/conductor-orchestrate`
2. **Wire dependencies explicitly** - Cross-track deps need `bd dep add`
3. **Use `bd ready --json`** - For structured bead status output
4. **Pre-register workers** - Use `agent-mail.js register-agent` before spawning
5. **Reserve files early** - Use `agent-mail.js file-reservation-paths` to prevent conflicts
6. **Check readiness** - At least one bead per track should be ready initially
