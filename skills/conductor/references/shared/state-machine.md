# Workflow State Machine

Reference for the Conductor workflow state machine.

## State Enum

| State | Description | Entry Trigger |
|-------|-------------|---------------|
| `INIT` | Track directory created, no artifacts yet | `/conductor-newtrack` Phase 1.3 |
| `DESIGNED` | design.md exists and approved | `ds` completes |
| `TRACKED` | spec.md + plan.md exist | `/conductor-newtrack` completes |
| `FILED` | Beads created from plan | `fb` completes |
| `REVIEWED` | Beads reviewed and refined | `rb` completes |
| `IMPLEMENTING` | At least one bead claimed (in_progress) | `bd update --status in_progress` |
| `DONE` | All beads closed | Last bead closed |
| `ARCHIVED` | Track moved to archive/ | `/conductor-finish` completes |

## State Transitions

### Valid Transitions Table

| From | To | Trigger | Type | Enforcement |
|------|----|---------|------|-------------|
| INIT | DESIGNED | `ds` completes | Forward | STRICT |
| DESIGNED | TRACKED | `/conductor-newtrack` completes | Forward | STRICT |
| TRACKED | FILED | `fb` completes | Forward | STRICT |
| FILED | REVIEWED | `rb` completes | Forward | STRICT |
| REVIEWED | IMPLEMENTING | `bd update --status in_progress` | Forward | STRICT |
| IMPLEMENTING | IMPLEMENTING | Working on next task | Loop | SOFT |
| IMPLEMENTING | DONE | All beads closed | Forward | STRICT |
| DONE | ARCHIVED | `/conductor-finish` completes | Forward | STRICT |
| DONE | IMPLEMENTING | Bead reopened | Backward | SOFT |

### Enforcement Types

- **STRICT**: HALT on invalid transition. Must fix state before proceeding.
- **SOFT**: WARN and proceed. Allows recovery from unexpected states.

## Transition Logic

```
function validateTransition(currentState, targetState):
    transition = TRANSITIONS[currentState][targetState]
    
    if transition is undefined:
        return {
            valid: false,
            error: "Invalid transition: {currentState} → {targetState}"
        }
    
    if transition.type == "STRICT":
        # STRICT transitions are valid if they exist in TRANSITIONS table
        pass
    else:  # SOFT
        WARN("Unusual transition: {currentState} → {targetState}")
    
    return { valid: true }
```

## State History

Each transition is logged to `metadata.json.workflow.history`:

```json
{
  "workflow": {
    "state": "IMPLEMENTING",
    "history": [
      {"state": "INIT", "at": "2025-12-27T10:00:00Z", "command": "newtrack"},
      {"state": "DESIGNED", "at": "2025-12-27T10:30:00Z", "command": "ds"},
      {"state": "TRACKED", "at": "2025-12-27T11:00:00Z", "command": "newtrack"},
      {"state": "FILED", "at": "2025-12-27T11:15:00Z", "command": "fb"},
      {"state": "REVIEWED", "at": "2025-12-27T11:20:00Z", "command": "rb"},
      {"state": "IMPLEMENTING", "at": "2025-12-27T11:30:00Z", "command": "implement"}
    ]
  }
}
```

## State Inference

For tracks without `workflow.state` (legacy or corrupted), infer from artifacts:

| Artifacts Present | Inferred State |
|-------------------|----------------|
| Nothing | INIT |
| design.md only | DESIGNED |
| spec.md + plan.md | TRACKED |
| .fb-progress.json.status = "complete" | FILED |
| Beads with "reviewed" label | REVIEWED |
| Any bead in_progress | IMPLEMENTING |
| All beads closed, not archived | DONE |
| Track in archive/ | ARCHIVED |

```
function inferState(trackPath):
    if isInArchive(trackPath):
        return "ARCHIVED"
    
    meta = readMetadata(trackPath)
    if meta.workflow?.state:
        return meta.workflow.state  # Trust explicit state
    
    # Infer from artifacts
    hasDesign = fileExists(trackPath + "/design.md")
    hasSpec = fileExists(trackPath + "/spec.md")
    hasPlan = fileExists(trackPath + "/plan.md")
    fbProgress = readFbProgress(trackPath)
    beads = queryBeads(trackPath)
    
    if beads.all(b => b.status == "closed"):
        return "DONE"
    if beads.any(b => b.status == "in_progress"):
        return "IMPLEMENTING"
    if beads.any(b => b.labels.includes("reviewed")):
        return "REVIEWED"
    if fbProgress?.status == "complete":
        return "FILED"
    if hasSpec && hasPlan:
        return "TRACKED"
    if hasDesign:
        return "DESIGNED"
    
    return "INIT"
```

## Diagram

```
     ┌────────┐
     │  INIT  │
     └───┬────┘
         │ ds
         ▼
  ┌──────────────┐
  │   DESIGNED   │
  └──────┬───────┘
         │ newtrack
         ▼
   ┌───────────┐
   │  TRACKED  │
   └─────┬─────┘
         │ fb
         ▼
    ┌─────────┐
    │  FILED  │
    └────┬────┘
         │ rb
         ▼
  ┌──────────────┐
  │   REVIEWED   │
  └──────┬───────┘
         │ bd update
         ▼
┌─────────────────┐
│  IMPLEMENTING   │◄──┐
└────────┬────────┘   │
         │ all closed │ reopen
         ▼            │
    ┌─────────┐───────┘
    │  DONE   │
    └────┬────┘
         │ finish
         ▼
  ┌──────────────┐
  │   ARCHIVED   │
  └──────────────┘
```
