---
track_id: ux-automation_20251227
created: 2025-12-27T20:00:00Z
status: approved
---

# UX Automation & State Machine

## Problem Statement

We are solving fragmented UX in the Conductor workflow pipeline for AI agents and developers because the current flow requires too many manual decisions and lacks guidance on what to do next, causing context loss and friction between workflow stages.

## Success Criteria

| # | Criterion |
|---|-----------|
| 1 | `/conductor-finish` auto-archives (no A/K prompt) |
| 2 | `--keep` flag stores choice in `metadata.json` |
| 3 | Git preflight handles `main` AND `master` branches |
| 4 | Auto-branch prompts with `[Y/n]`, checks for dirty state |
| 5 | All commands end with `→ Next:` suggestion based on workflow state |

## Chosen Approach

**Option C: State Machine Refactor** - Add workflow state tracking to `metadata.json`, derive suggestions automatically.

## Design

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                    WORKFLOW STATE MACHINE                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  INIT ──→ DESIGNED ──→ TRACKED ──→ FILED ──→ IMPLEMENTING      │
│    │         │            │          │            │              │
│    │         │            │          │            ▼              │
│    │         │            │          │         DONE ──→ ARCHIVED │
│    │         │            │          │            │              │
│    │         ▼            ▼          ▼            │              │
│    └──── [ds] ────► [newtrack] ──► [fb] ──► [implement] ◄──────┘ │
│                                      │                           │
│                                      ▼                           │
│                                    [rb]                          │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Components

| Component | Responsibility | Location |
|-----------|---------------|----------|
| **State Store** | Persist workflow state in `metadata.json` | `conductor/tracks/{id}/metadata.json` |
| **State Transition Engine** | Validate transitions, update state | Inline in each command |
| **Suggestion Resolver** | Map state → next action | `shared/suggestions.md` |
| **Git Preflight** | Branch check before state changes | `shared/git-preflight.md` |

### Data Model

**metadata.json additions:**

```json
{
  "track_id": "ux-automation_20251227",
  "status": "in_progress",
  
  "workflow": {
    "state": "FILED",
    "history": [
      {"state": "INIT", "at": "2025-12-27T10:00:00Z", "command": "newtrack"},
      {"state": "DESIGNED", "at": "2025-12-27T10:30:00Z", "command": "ds"},
      {"state": "TRACKED", "at": "2025-12-27T11:00:00Z", "command": "newtrack"},
      {"state": "FILED", "at": "2025-12-27T11:15:00Z", "command": "fb"}
    ],
    "branch": "feat/ux-automation_20251227",
    "archived": false,
    "keep": false
  }
}
```

**State enum:**

| State | Description |
|-------|-------------|
| `INIT` | Track directory created |
| `DESIGNED` | design.md exists |
| `TRACKED` | spec.md + plan.md exist |
| `FILED` | Beads created (fb complete) |
| `REVIEWED` | Beads reviewed (rb complete) |
| `IMPLEMENTING` | At least one bead in_progress |
| `DONE` | All beads closed |
| `ARCHIVED` | Moved to archive/ |

### State Transitions

**Valid transitions:**

| From | To | Trigger | Type |
|------|----|---------|------|
| INIT | DESIGNED | ds completes | STRICT |
| DESIGNED | TRACKED | newtrack completes | STRICT |
| TRACKED | FILED | fb completes | STRICT |
| FILED | REVIEWED | rb completes | STRICT |
| REVIEWED | IMPLEMENTING | bd update --status in_progress | STRICT |
| IMPLEMENTING | IMPLEMENTING | working on tasks | SOFT |
| IMPLEMENTING | DONE | all beads closed | STRICT |
| DONE | ARCHIVED | finish completes | STRICT |
| DONE | IMPLEMENTING | bead reopened | SOFT |

- **STRICT:** HALT on invalid transition
- **SOFT:** WARN only, allow recovery

### Suggestion Resolver

| Current State | Primary Suggestion | Alt Suggestion |
|---------------|-------------------|----------------|
| `INIT` | `→ Next: ds (start design)` | — |
| `DESIGNED` | `→ Next: /conductor-newtrack {id}` | `ds (refine design)` |
| `TRACKED` | `→ Next: fb (file beads)` | `bd ready (if beads exist)` |
| `FILED` | `→ Next: rb (review beads)` | — |
| `REVIEWED` | `→ Next: bd ready (start work)` | — |
| `IMPLEMENTING` | `→ Next: {next-task-title}` | `finish branch (if all done)` |
| `DONE` | `→ Next: finish branch` | — |
| `ARCHIVED` | `→ Next: ds (start new work)` | — |

**Output format:**

```
┌─────────────────────────────────────────┐
│ ✓ {command} completed                   │
│                                         │
│ → Next: {primary_suggestion}            │
│   Alt: {alt_suggestion}                 │
└─────────────────────────────────────────┘
```

### Git Preflight

**Phase 0.5 in newTrack.toml:**

```bash
# 0.5.1 Get current branch
BRANCH=$(git branch --show-current 2>/dev/null)

# 0.5.2 Check if on main/master
if [ "$BRANCH" = "main" ] || [ "$BRANCH" = "master" ]; then
    
    # 0.5.3 Check for uncommitted changes
    if [ -n "$(git status --porcelain)" ]; then
        echo "⚠️ Uncommitted changes on $BRANCH. Commit or stash first."
        exit 1
    fi
    
    # 0.5.4 Prompt for branch creation
    NEW_BRANCH="feat/${track_id}"
    
    # Check if branch exists, use versioned suffix if needed
    if git show-ref --verify --quiet "refs/heads/$NEW_BRANCH"; then
        # Try with -v2 suffix
        NEW_BRANCH="feat/${track_id}-v2"

        # If still exists, try -v3, etc.
        SUFFIX=2
        MAX_RETRIES=10
        while git show-ref --verify --quiet "refs/heads/$NEW_BRANCH"; do
            SUFFIX=$((SUFFIX + 1))
            NEW_BRANCH="feat/${track_id}-v${SUFFIX}"
            if [ "$SUFFIX" -gt "$MAX_RETRIES" ]; then
                echo "Error: Too many existing branches for ${track_id}" >&2
                exit 1
            fi
        done

        echo "Branch feat/${track_id} exists. Using ${NEW_BRANCH}"
    fi
    
    echo "On $BRANCH. Create branch '$NEW_BRANCH'? [Y/n]"
    read -r response
    
    if [ "$response" != "n" ] && [ "$response" != "N" ]; then
        git fetch origin 2>/dev/null
        git checkout -b "$NEW_BRANCH"
        
        # Store in metadata
        meta["workflow"]["branch"] = "$NEW_BRANCH"
    fi
fi
```

### Auto-Archive (finish.toml)

**Phase 5 changes:**

1. Remove A/K prompt - auto-archive by default
2. Add `--keep` flag to opt out
3. Store choice in `metadata.json`
4. Show `→ Next: ds` after completion

```
┌─────────────────────────────────────────┐
│ ✓ Track completed                       │
│                                         │
│ Summary:                                │
│ - Threads processed: N                  │
│ - Beads compacted: N                    │
│ - Learnings added: N                    │
│ - Location: conductor/archive/{id}/     │
│                                         │
│ → Next: ds (start new design)           │
└─────────────────────────────────────────┘
```

### Error Handling

| Scenario | Behavior |
|----------|----------|
| Invalid state transition (STRICT) | HALT with error message |
| Invalid state transition (SOFT) | WARN and proceed |
| Git status dirty on main | HALT: "Uncommitted changes" |
| Branch already exists | Offer `-v2` suffix |
| metadata.json missing | Create with defaults |
| metadata.json corrupted | Backup and recreate |
| Open beads at finish | HALT unless `--force` |

### Testing Strategy

**Test matrix for git preflight:**

| Branch | Git Status | Expected Behavior |
|--------|------------|-------------------|
| `main` | clean | Prompt: "Create feat/{id}? [Y/n]" |
| `main` | dirty | HALT: "Uncommitted changes" |
| `master` | clean | Prompt: "Create feat/{id}? [Y/n]" |
| `master` | dirty | HALT: "Uncommitted changes" |
| `feat/X` | clean | Proceed silently |
| `feat/X` | dirty | Proceed (user's problem) |
| `feat/{id}` exists | clean | Offer `feat/{id}-v2` |

**Test matrix for auto-archive:**

| Beads State | Flag | Expected Behavior |
|-------------|------|-------------------|
| All closed | (none) | Auto-archive to `archive/` |
| All closed | `--keep` | Stay in `tracks/`, state=DONE |
| Some open | (none) | HALT: "N beads still open" |
| Some open | `--force` | Archive anyway (with warning) |

## Grounding Notes

- Verified existing `metadata.json` structure in archive - adding `workflow` object is backward compatible
- Current `status` field kept for human-readable, `workflow.state` for machine logic
- No new external dependencies required

## Risks & Open Questions

- **Concurrent access:** Multiple agents updating state simultaneously (mitigated by SA mode)
- **State recovery:** How to fix corrupted state? (Solution: backup and recreate)
- **Adoption:** Existing tracks won't have workflow state (Solution: infer from artifacts)

## Out of Scope

- `--quiet` flag for suppressing suggestions (deferred to v1.1)
- `/conductor-unarchive` recovery command (deferred)
- Full state machine visualization UI (deferred)
- Shared module extraction (inline first, extract later)

## Implementation Files

| Action | File |
|--------|------|
| ✏️ Modify | `skills/conductor/references/commands/finish.toml` |
| ✏️ Modify | `skills/conductor/references/commands/newTrack.toml` |
| ✏️ Modify | `skills/conductor/references/commands/implement.toml` |
| ➕ Create | `skills/conductor/references/shared/state-machine.md` |
| ➕ Create | `skills/conductor/references/shared/suggestions.md` |
| ➕ Create | `skills/conductor/references/shared/git-preflight.md` |

**Estimated effort:** ~7.5 hours
