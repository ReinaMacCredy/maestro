---
track_id: merge-newtrack-fb_20251223
created: 2025-12-23
status: approved
thread_id: T-019b4a25-c3eb-773e-ba1d-dfd80ee19092
---

# Merge newTrack and File Beads

## Problem Statement

After design approval, users are presented with two options (`fb` or `/conductor-newtrack`) that appear as alternatives but are actually sequential steps. Users always run `newtrack → fb → rb` in sequence, making the current handoff confusing and adding friction.

## Success Criteria

- Single command (`/conductor-newtrack`) handles: spec → plan → beads → review
- `fb` remains available standalone for re-filing from existing plans
- Robust checkpoint/resume on failure
- Full traceability via thread IDs in metadata

## Chosen Approach

**Subagent Dispatch with Checkpointing**

- newTrack generates spec.md + plan.md, then spawns Task subagent to run fb logic
- Another Task subagent runs rb after fb completes
- `.fb-progress.json` tracks state for resume capability
- Thread IDs captured in `metadata.json` for audit trail

## Design

### Architecture Overview

```
/conductor-newtrack [track_id] [--no-beads|-nb|--plan-only|-po] [--force]

Flow:
├── 1. Check lock file (.fb-progress.lock)
├── 2. Validate setup (product.md, tech-stack.md, workflow.md)
├── 3. Resolve track ID (existing or generate new)
│   └── Handle collision → -v2 suffix
├── 4. Check for existing track → error unless --force
├── 5. Read design.md (or interactive fallback)
├── 6. Generate spec.md → save immediately
├── 7. Generate plan.md → save immediately
├── 8. Update metadata.json with thread ID
├── 9. If --no-beads → stop here
├── 10. Create lock file
├── 11. Task: "Run fb on plan.md" (batched, checkpointed)
├── 12. Task: "Run rb on filed beads"
├── 13. Release lock, update progress
└── 14. Display summary + handoff
```

### Components

#### 1. Modified newTrack.toml

**New flags:**

- `--no-beads` / `-nb`: Generate spec+plan only, skip beads
- `--plan-only` / `-po`: Alias for --no-beads
- `--force`: Overwrite existing track

**New behavior:**

- Captures thread ID from Amp Thread URL
- Invokes fb via Task subagent after plan generation
- Invokes rb via Task subagent after fb completion
- Creates/manages lock and progress files

#### 2. .fb-progress.json

Location: `conductor/tracks/{trackId}/.fb-progress.json`

```json
{
  "trackId": "auth_20251223",
  "status": "complete | in_progress | failed",
  "lastVerified": "2025-12-23T10:00:00Z",

  "epics": [
    {
      "id": "bd-1",
      "title": "Epic: Auth",
      "status": "complete",
      "createdAt": "2025-12-23T10:00:00Z",
      "reviewed": true,
      "reviewedAt": "2025-12-23T11:00:00Z"
    }
  ],

  "issues": ["bd-4", "bd-5", "bd-6"],

  "crossTrackDeps": [{ "from": "bd-3", "to": "api_20251223:bd-7" }],

  "resumeFrom": null,
  "lastError": null
}
```

#### 3. metadata.json (extended)

```json
{
  "trackId": "auth_20251223",
  "type": "feature",
  "status": "new | in_progress | complete | archived",
  "priority": "medium",
  "depends_on": [],
  "estimated_hours": null,
  "created_at": "2025-12-23T10:00:00Z",
  "updated_at": "2025-12-23T10:00:00Z",
  "description": "...",
  "has_design": true,

  "threads": [
    {
      "id": "T-019b4a25-c3eb-773e-ba1d-dfd80ee19092",
      "action": "design",
      "timestamp": "2025-12-23T10:00:00Z"
    }
  ],

  "artifacts": {
    "design": true,
    "spec": true,
    "plan": true,
    "beads": true
  }
}
```

#### 4. Modified fb skill

**New behavior:**

- Writes to `.fb-progress.json` as epics are created
- Batches epics in groups of 5
- Checkpoints after each batch
- Supports resume from checkpoint
- Validates JSON in subagent prompt + retry with hint + fallback

#### 5. Modified rb skill

**New behavior:**

- Scans `conductor/tracks/*/.fb-progress.json` to find track
- Checks progress file status before reviewing
- Updates beads with `--label reviewed`
- Updates progress file with `reviewed: true`
- Handles multiple tracks (asks user)
- Collects "needs input" issues, lists at end

### Data Model & Interfaces

#### File Structure

```
conductor/tracks/{track_id}/
├── metadata.json        # Track info + thread IDs
├── design.md            # From ds
├── spec.md              # From newtrack
├── plan.md              # From newtrack
├── .fb-progress.json    # Beads filing state
├── .fb-progress.lock    # Concurrent session lock
└── .track-progress.json # Spec/plan generation state
```

#### Lock File Format

`.fb-progress.lock`:

```json
{
  "sessionId": "abc123",
  "startedAt": "2025-12-23T10:00:00Z",
  "threadId": "T-019b4a25-..."
}
```

Timeout: 30 minutes (stale lock = crashed session)

### User Flow

**Primary flow:**

```
ds → design.md → /conductor-newtrack → spec.md + plan.md + beads + review
```

**Handoff after design:**

```
━━━ DESIGN COMPLETE ━━━
Track: auth_20251223
File: conductor/tracks/auth_20251223/design.md

Next: `/conductor-newtrack auth_20251223`
      (generates spec + plan + files beads + reviews)

Or:
  `/conductor-newtrack auth_20251223 --no-beads` — spec + plan only
```

**Handoff after newtrack:**

```
━━━ TRACK COMPLETE ━━━
Track: auth_20251223
Spec: conductor/tracks/auth_20251223/spec.md
Plan: conductor/tracks/auth_20251223/plan.md
Beads: 5 epics, 23 issues filed and reviewed

Ready issues: 8
First task: bd-4 (Setup auth config)

Next: `Start epic bd-1` or `/conductor-implement auth_20251223`
```

### Error Handling

| Scenario                                 | Behavior                                                                |
| ---------------------------------------- | ----------------------------------------------------------------------- |
| Track exists                             | Error: "Track exists. Use --force to overwrite."                        |
| Flag conflict (--no-beads + --plan-only) | Error: "Cannot use both flags. They're aliases."                        |
| Lock file exists                         | "Another session filing beads (started Xmin ago). Wait or use --force." |
| Empty plan                               | Ask: "Plan has no tasks. Continue anyway? [y/N]"                        |
| fb subagent fails                        | Read checkpoint, resume from last batch                                 |
| Malformed JSON from subagent             | Retry with hint once, then main agent fallback                          |
| design.md changed after beads filed      | Auto-diff, suggest which beads to update                                |
| Track ID collision                       | Auto-increment with `-v2` suffix                                        |

### Testing Strategy

1. **Unit tests:**

   - Lock file creation/timeout/cleanup
   - Progress file read/write/merge
   - Thread ID extraction from Amp URL

2. **Integration tests:**

   - Full flow: design → newtrack → beads → review
   - Resume after failure mid-fb
   - Resume after failure mid-rb
   - --no-beads flag behavior
   - --force flag behavior

3. **Manual verification:**
   - Cross-track dependencies
   - Multiple concurrent sessions (lock behavior)
   - Large plan (20+ epics) batching

## Edge Cases

| #   | Case                | Decision                                   |
| --- | ------------------- | ------------------------------------------ |
| 1   | Multiple tracks     | Ask user which to review                   |
| 2   | Orphan beads        | Warn, include anyway                       |
| 3   | Deleted track       | Warn, continue without                     |
| 4   | Stale progress      | Timestamp compare, auto-correct, show diff |
| 5   | Concurrent sessions | Lock file with 30min timeout               |
| 6   | Partial rb          | Progress file + beads label                |
| 7   | Empty plan          | Ask user                                   |
| 8   | Flag conflict       | Error                                      |
| 9   | Re-run on existing  | Error, use --force                         |
| 10  | design.md changed   | Auto-diff, suggest beads to update         |
| 11  | rb needs user input | Collect all, list at end                   |
| 12  | Malformed JSON      | Validate + retry + fallback                |
| 13  | Track ID collision  | Auto-increment -v2 suffix                  |
| 14  | Large plan          | Batch in groups of 5                       |
| 15  | Cross-track deps    | Allow, update both progress files          |
| 16  | Interrupted gen     | Keep partial + checkpoint                  |
| 17  | Thread tracking     | Store in metadata.json                     |

## Files to Modify

| File                               | Changes                                              |
| ---------------------------------- | ---------------------------------------------------- |
| `commands/conductor/newTrack.toml` | Add fb/rb integration, flags, checkpoints, thread ID |
| `commands/conductor/design.toml`   | Fix handoff message (remove "or fb" confusion)       |
| `skills/file-beads/SKILL.md`       | Add progress file writing, batching, resume          |
| `skills/review-beads/SKILL.md`     | Add progress file check, label update, track scan    |

## Risks & Open Questions

- **Rate limiting:** Large plans may hit API limits during parallel subagent dispatch. Batching (5 epics) mitigates.
- **Context loss in subagents:** Subagents only have plan.md context. May miss nuances from design discussion.
- **Lock file reliability:** If agent crashes without cleanup, lock persists until timeout.

## Out of Scope

- Changes to beads CLI (`bd` command)
- Changes to TUI viewer (`bv`)
- Multi-user collaboration (single-user workflow only)
- Real-time progress streaming (batch summary only)
