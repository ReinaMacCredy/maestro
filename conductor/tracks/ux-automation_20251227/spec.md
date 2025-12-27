---
track_id: ux-automation_20251227
version: 1.0
status: draft
---

# Spec: UX Automation & State Machine

## Overview

Add a workflow state machine to Conductor that tracks progress through the development pipeline and provides smart "what's next" suggestions after each command. Includes auto-archive for `/conductor-finish` and git branch preflight for `/conductor-newtrack`.

## Requirements

### R1: Workflow State Machine

**R1.1** Add `workflow` object to `metadata.json` with fields:
- `state`: Current workflow state (enum: INIT, DESIGNED, TRACKED, FILED, REVIEWED, IMPLEMENTING, DONE, ARCHIVED)
- `history`: Array of state transitions with timestamp and command
- `branch`: Git branch name (if created via preflight)
- `archived`: Boolean indicating if track has been archived
- `keep`: Boolean indicating if `--keep` flag was used

**R1.2** State transitions must be validated:
- STRICT transitions HALT on violation
- SOFT transitions WARN and proceed

**R1.3** Valid state transitions:
| From | To | Type |
|------|----|------|
| INIT → DESIGNED | STRICT |
| DESIGNED → TRACKED | STRICT |
| TRACKED → FILED | STRICT |
| FILED → REVIEWED | STRICT |
| REVIEWED → IMPLEMENTING | STRICT |
| IMPLEMENTING → IMPLEMENTING | SOFT |
| IMPLEMENTING → DONE | STRICT |
| DONE → ARCHIVED | STRICT |
| DONE → IMPLEMENTING | SOFT |

### R2: Smart Suggestions

**R2.1** Every command must end with a suggestion block:
```
┌─────────────────────────────────────────┐
│ ✓ {command} completed                   │
│                                         │
│ → Next: {primary_suggestion}            │
│   Alt: {alt_suggestion}                 │
└─────────────────────────────────────────┘
```

**R2.2** Suggestions derived from current workflow state:
| State | Primary Suggestion |
|-------|-------------------|
| INIT | `ds (start design)` |
| DESIGNED | `/conductor-newtrack {id}` |
| TRACKED | `fb (file beads)` |
| FILED | `rb (review beads)` |
| REVIEWED | `bd ready (start work)` |
| IMPLEMENTING | `{next-task-title}` |
| DONE | `finish branch` |
| ARCHIVED | `ds (start new work)` |

### R3: Auto-Archive

**R3.1** `/conductor-finish` auto-archives by default (no A/K prompt)

**R3.2** `--keep` flag prevents archiving:
- Track stays in `tracks/`
- `workflow.state` = DONE (not ARCHIVED)
- `workflow.keep` = true

**R3.3** Pre-archive validation:
- HALT if any beads are open (unless `--force`)
- Show count of open beads in error message

### R4: Git Preflight

**R4.1** `/conductor-newtrack` checks git branch before proceeding:
- If on `main` or `master` AND clean → prompt to create `feat/{track_id}`
- If on `main` or `master` AND dirty → HALT with "Uncommitted changes"
- If on feature branch → proceed silently

**R4.2** Branch creation:
- Check if `feat/{track_id}` exists
- If exists, offer `feat/{track_id}-v2`
- Run `git fetch origin` before creating branch
- Store branch name in `workflow.branch`

### R5: Shared Reference Files

**R5.1** Create `skills/conductor/references/shared/state-machine.md`:
- State enum definition
- Valid transitions table
- Transition validation logic

**R5.2** Create `skills/conductor/references/shared/suggestions.md`:
- State → suggestion mapping
- Output format template

**R5.3** Create `skills/conductor/references/shared/git-preflight.md`:
- Branch detection logic
- Clean/dirty check
- Branch creation flow

## Acceptance Criteria

### AC1: State Machine
- [ ] `metadata.json` includes `workflow` object with correct schema
- [ ] State transitions are logged to `workflow.history`
- [ ] Invalid STRICT transitions HALT with error
- [ ] Invalid SOFT transitions WARN and continue

### AC2: Suggestions
- [ ] `ds` ends with `→ Next: /conductor-newtrack {id}`
- [ ] `/conductor-newtrack` ends with `→ Next: fb (file beads)`
- [ ] `fb` ends with `→ Next: rb (review beads)`
- [ ] `rb` ends with `→ Next: bd ready`
- [ ] `/conductor-finish` ends with `→ Next: ds (start new work)`

### AC3: Auto-Archive
- [ ] `/conductor-finish` archives without prompting A/K
- [ ] `--keep` flag works and stores in metadata
- [ ] Open beads cause HALT with count shown
- [ ] `--force` bypasses open beads check

### AC4: Git Preflight
- [ ] Running on `main` with clean status prompts for branch
- [ ] Running on `main` with dirty status HALTs
- [ ] Running on `master` behaves same as `main`
- [ ] Running on feature branch proceeds silently
- [ ] Existing branch offers `-v2` suffix

### AC5: Files Created
- [ ] `shared/state-machine.md` exists with enum and transitions
- [ ] `shared/suggestions.md` exists with mapping table
- [ ] `shared/git-preflight.md` exists with detection logic

## Non-Functional Requirements

- **Backward compatible**: Existing tracks without `workflow` object continue to work
- **No new dependencies**: Uses existing Git and `bd` CLI
- **Graceful degradation**: If state is missing, infer from artifacts

## Out of Scope

- `--quiet` flag for suppressing suggestions
- `/conductor-unarchive` recovery command
- State machine visualization UI
- Automatic state inference for legacy tracks
