# Beads-Conductor Integration Design

**Track ID:** beads-conductor-integration_20251225  
**Created:** 2025-12-25  
**Status:** Design Complete  
**Thread:** T-019b5567-ee85-737c-85f0-b30b80e6cac1

## Problem Statement

Beads workflow has full capabilities (session lifecycle, checkpointing, Village coordination, dependencies) but Conductor flow only integrates at 2 points (`fb`, `rb`). This forces users to manually run bd commands, making it easy to forget and causing sync loss between Conductor state and Beads state.

## Goals

1. **Zero manual bd commands** in the happy path
2. **Automatic status sync** - Bead status = actual work status
3. **Compaction survival** - Notes automatically checkpoint
4. **Parallel agent safety** - Village coordination in Task dispatch
5. **Graceful setup** - `/conductor-setup` ensures prerequisites

## Non-Goals

- Change Beads CLI (`bd` commands)
- Change Village MCP server
- New slash commands (use existing)

---

## Architecture

### Dual-Mode Architecture

```
┌──────────────────────────────────────────────────────────────────────┐
│                         DUAL-MODE ARCHITECTURE                        │
├──────────────────────────────────────────────────────────────────────┤
│                                                                       │
│   Session Start                                                       │
│        │                                                              │
│        ▼                                                              │
│   ┌─────────────────┐                                                │
│   │    PREFLIGHT    │ ─── Mode detect ───┬─► SA Mode                 │
│   └────────┬────────┘                    │   (bd CLI)                │
│            │                             │                            │
│            │                             └─► MA Mode                  │
│            │                                 (Village MCP)            │
│            ▼                                                          │
│   ┌─────────────────┐         ┌─────────────────┐                    │
│   │  session-state  │         │  session-state  │                    │
│   │   _<agent>.json │ (SA)    │   _shared.json  │ (MA coordination)  │
│   └─────────────────┘         └─────────────────┘                    │
│                                                                       │
├──────────────────────────────────────────────────────────────────────┤
│                                                                       │
│   SUBAGENT RULES                                                      │
│   ┌───────────────────────────────────────────────────────────┐      │
│   │  ✅ bd show <id> --json     (read)                        │      │
│   │  ✅ bd ready --json         (read)                        │      │
│   │  ✅ bd list --json          (read)                        │      │
│   │  ❌ bd update               (return to main)              │      │
│   │  ❌ bd close                (return to main)              │      │
│   │  ❌ bd create               (return to main)              │      │
│   └───────────────────────────────────────────────────────────┘      │
│                                                                       │
├──────────────────────────────────────────────────────────────────────┤
│                                                                       │
│   HANDOFF PROTOCOL (MA only)                                          │
│   .conductor/handoff_<from>_to_<to>.json                             │
│   24hr TTL, processed in timestamp order                              │
│   ⚠️  Orphan warning: Preflight logs undelivered handoffs >12hr      │
│                                                                       │
└──────────────────────────────────────────────────────────────────────┘
```

### Integration Points (13 total)

| # | Point | Conductor Command | Beads Action | Refinement |
|---|-------|-------------------|--------------|------------|
| 1 | Preflight | All | Mode detect, validate, recover | HALT if bd unavailable |
| 2 | Init/Claim | /conductor-implement | Join team, claim task | SA: `bd update`, MA: `claim()` |
| 3 | Reserve | Task tool | Lock files (MA only) | Subagents read-only |
| 4 | TDD:RED | /conductor-implement | Checkpoint: test written | Opt-in via `--tdd` |
| 5 | TDD:GREEN | /conductor-implement | Checkpoint: test passes | Opt-in via `--tdd` |
| 6 | TDD:REFACTOR | /conductor-implement | Checkpoint: code clean | Opt-in via `--tdd` |
| 7 | Close | /conductor-implement | Complete task | With reason: completed/skipped/blocked |
| 8 | Sync | All (end) | Push to git | Retry 3x, persist unsynced |
| 9 | Compact | /conductor-finish | Summarize closed | AI summary generation |
| 10 | Cleanup | /conductor-finish | Remove old (>150) | Threshold-based |
| 11 | Track-init | /conductor-newtrack | Initialize state | plan.md validation + R/S/M prompt |
| 12 | Status-sync | /conductor-status | Bidirectional sync | Discrepancy detection |
| 13 | Revise/Reopen | /conductor-revise | Reopen closed beads | History preservation |

### State Files

| File | Location | Purpose |
|------|----------|---------|
| session-state_<agent-id>.json | .conductor/ | Per-agent session tracking |
| session-state_shared.json | .conductor/ | MA coordination (who's online, handoffs) |
| handoff_<from>_to_<to>.json | .conductor/ | File-as-message handoff protocol |
| preferences.json | .conductor/ | User preferences |
| metrics.jsonl | .conductor/ | Usage instrumentation (append-only) |
| metadata.json | tracks/<id>/ | Track metadata |
| .track-progress.json | tracks/<id>/ | Spec/plan progress |
| .fb-progress.json | tracks/<id>/ | Beads filing progress + planTasks mapping |

### Session State Schema

```json
// .conductor/session-state_<agent-id>.json
{
  "agentId": "T-abc123",
  "mode": "SA",
  "modeLockedAt": "2025-12-25T10:00:00Z",
  "trackId": "beads-integration_20251225",
  "currentTask": "bd-42",
  "tddPhase": "GREEN",
  "lastUpdated": "2025-12-25T12:00:00Z"
}
```

### .fb-progress.json Schema (Enhanced)

```json
{
  "trackId": "beads-integration_20251225",
  "status": "complete",
  "planTasks": {
    "1.1": "bd-42",
    "1.2": "bd-43"
  },
  "beadToTask": {
    "bd-42": "1.1",
    "bd-43": "1.2"
  },
  "lastVerified": "2025-12-25T12:00:00Z"
}
```

---

## Components (12 total)

**Note:** 13 integration points map to 12 components because the "Reserve" point (file reservation) is part of the Task Tool Injection component (Component 5), not a standalone component.

### Component 1: Preflight System
- Mode detection (MCP vs CLI)
- HALT if bd unavailable (no silent skip)
- Session recovery from crashes
- Health checks (stale agents, orphaned beads, unsynced)
- State file validation

### Component 2: Claim System
- Multi-agent: `init()` → `claim()` (atomic)
- Single-agent: `bd ready` → `bd update --status in_progress`
- Race condition handling
- Parallel task support: `bd update id1 id2 --status in_progress`

### Component 3: TDD Checkpoints (Opt-in)
- Enabled via `--tdd` flag
- RED/GREEN/REFACTOR tracking
- Notes format: COMPLETED/IN_PROGRESS/NEXT
- Session state updates
- Skip if no tests detected

### Component 4: Close/Sync System
- Multi-agent: `done()` with auto-release
- Single-agent: `bd close` → `bd sync`
- Close with reason: `--reason completed|skipped|blocked`
- Retry logic (3 attempts)
- Unsynced state persistence

### Component 5: Task Tool Injection
- Village coordination block for subagents
- Agent identity injection
- Reserve/release protocol
- Subagent read-only bd access

### Component 6: Migration Command
- `/conductor-migrate-beads`
- Scan → Analyze → Confirm → Execute → Verify
- Link existing beads or create new

### Component 7: State Validation
- Session state auto-creation per agent
- Auto-repair on corruption
- Version migration
- Track mismatch detection

### Component 8: Compact/Cleanup
- AI summary generation
- Threshold-based cleanup (>150 closed)
- Village state cleanup

### Component 9: Track Init
- State files creation
- plan.md validation with R/S/M prompt (Reformat/Skip/Manual)
- `--strict` flag for CI (fail on malformed)
- Session-track linking
- Create epic + all issues + wire dependencies

### Component 10: Master Reference (Facade)
- beads-facade.md + beads-integration.md
- Single integration layer
- Contract specification
- Output format specification

### Component 11: Status Sync
- Conductor status ↔ Beads status bidirectional sync
- `/conductor-status` queries both sources
- Detect and report discrepancies
- Suggest reconciliation actions

### Component 12: Revise/Reopen
- `/conductor-revise` integration
- Reopen closed beads when spec/plan changes
- Link new beads to revised plan items
- Preserve original bead history

---

## Facade Contract

```typescript
interface BeadsFacade {
  // Preflight
  checkAvailability(): { 
    available: boolean; 
    version?: string; 
    error?: string 
  }
  
  // Track Init
  createEpicFromPlan(input: {
    trackId: string;
    planPath: string;
    epicTitle: string;
    tasks: Array<{ 
      id: string;
      title: string; 
      priority: 0|1|2|3|4;
      depends?: string[];
    }>;
  }): { 
    epicId: string; 
    taskIds: string[]; 
    planTasksMapping: Record<string, string> 
  }
  
  // Session Sync
  syncToGit(options?: { retries?: number }): { 
    success: boolean; 
    synced: number; 
    unsynced?: string[];
  }
  
  // Claim
  claimTask(taskId: string, mode: 'SA' | 'MA'): {
    success: boolean;
    alreadyClaimed?: boolean;
  }
  
  // Close
  closeTask(taskId: string, reason: 'completed' | 'skipped' | 'blocked'): {
    success: boolean;
  }
  
  // TDD Checkpoint
  updateTddPhase(taskId: string, phase: 'RED' | 'GREEN' | 'REFACTOR'): {
    success: boolean;
  }
}

type FacadeError = {
  code: 'BD_UNAVAILABLE' | 'BD_TIMEOUT' | 'SYNC_FAILED' | 'EPIC_EXISTS' | 'PARSE_ERROR' | 'CLAIM_CONFLICT';
  message: string;
  recoverable: boolean;
}
```

---

## Output Formats

### Compact Mode (Default)
```
📍 bd-42 "Implement auth" | SA mode
✓ RED → GREEN → REFACTOR → closed → synced
```

### Verbose Mode (--verbose)
```
✓ [BEADS:preflight] Mode: SINGLE-AGENT
✓ [BEADS:claim] bd-42 "Implement auth endpoint"
✓ [BEADS:tdd:red] Test written
✓ [BEADS:tdd:green] Test passes
✓ [BEADS:tdd:refactor] Code clean
✓ [BEADS:close] bd-42 completed
✓ [BEADS:sync] Pushed to origin
```

---

## Error Handling

| Point | Success | Failure | Recovery |
|-------|---------|---------|----------|
| Preflight | Continue | HALT | Fix issues, retry |
| Init | Multi-agent | Single-agent | Degraded mode with warning |
| Claim | Got task | None available | Report blocked |
| Reserve | Got lock | Conflict | Main reassigns |
| TDD:RED | Test written | Test error | Debug |
| TDD:GREEN | Test pass | Test fail | Debug |
| TDD:REFACTOR | Code clean | Breaks tests | Revert, retry |
| Close | Done | Command fail | Retry → manual |
| Sync | Pushed | Network fail | Persist unsynced |
| Track-init | Created | plan.md malformed | R/S/M prompt |

---

## Risks & Mitigations

| Risk | Severity | Mitigation |
|------|----------|------------|
| Behavioral change | Critical | Introduction modal, docs |
| Partial failure | Critical | State machine, recovery |
| Backward compat | High | Migration command |
| Missing dependency | High | HALT with clear message |
| Multi-agent race | High | Atomic claim, reserve |
| Network failure | Medium | Retry + persist unsynced |
| plan.md malformed | Medium | Validation + R/S/M prompt |
| Non-TDD workflows | Low | TDD opt-in via `--tdd` |

---

## Files to Create (11 files)

| File | Purpose | Priority | Est. Lines |
|------|---------|----------|------------|
| skills/conductor/references/beads-facade.md | Facade contract, mode detection, error semantics | P0 | 140–180 |
| skills/conductor/references/beads-integration.md | Master reference: all 13 points, full lifecycle | P0 | 200–260 |
| skills/conductor/references/validation/beads/checks.md | Beads validation: `.fb-progress.json` schema, R/S/M prompt | P0 | 120–160 |
| workflows/conductor/preflight-beads.md | Preflight protocol: mode detect, HALT rules, degraded mode | P0 | 130–170 |
| workflows/conductor/beads-session.md | Session lifecycle: SA/MA flows, close reasons, subagent rules | P0 | 160–220 |
| workflows/conductor/track-init-beads.md | Track-init: create epics/issues, planTasksMapping, validation | P0 | 110–150 |
| workflows/conductor/tdd-checkpoints-beads.md | TDD integration: RED/GREEN/REFACTOR to bead notes (opt-in) | P2 | 110–150 |
| workflows/conductor/status-sync-beads.md | Status sync: discrepancy detection, reconciliation | P2 | 110–150 |
| workflows/conductor/revise-reopen-beads.md | Revise/Reopen: affected beads, history preservation | P2 | 110–150 |
| commands/conductor-migrate-beads.md | Migration command: Scan → Analyze → Confirm → Execute → Verify | P1 | 80–120 |
| scripts/beads-metrics-summary.sh | Metrics aggregation script for usage analysis | P1 | 60–100 |

---

## Files to Modify (11 files)

| File | Changes | Priority | Est. Lines |
|------|---------|----------|------------|
| skills/conductor/SKILL.md | Add integration overview, dual-mode, `--tdd` flag, preflight/session-sync | P0 | 70–110 |
| workflows/implement.md | Preflight call, SA/MA paths, TDD checkpoints, close with reason, sync | P0 | 80–120 |
| workflows/newtrack.md | Track validation, beads init, planTasksMapping, existing beads handling | P0 | 70–110 |
| workflows/finish.md | Compaction thresholds (>150), cleanup commands, Village cleanup | P1 | 60–90 |
| workflows/status.md | Status sync, discrepancy display, reconciliation suggestions | P2 | 60–90 |
| workflows/revise.md | Reopen affected beads, history rules, linkage | P2 | 60–90 |
| skills/conductor/references/validation/track/checks.md | Add `.fb-progress.json` validation, hook beads/checks.md, R/S/M prompt | P0 | 70–110 |
| workflows/beads/workflow.md | Conductor integration note, when manual bd is appropriate, MA constraints | P0 | 50–90 |
| AGENTS.md | Refresh Beads section, facade explanation, new checklists, metrics | P0 | 50–80 |
| skills/beads/SKILL.md | Conductor-awareness, facade abstraction, SA/MA behaviors | P1 | 40–70 |
| .claude-plugin/plugin.json | Update description, keywords, new commands | P1 | 20–40 |

---

## Acceptance Criteria

### P0 (Must Have) - 12 criteria
- [ ] Preflight detects mode correctly (SA vs MA)
- [ ] Preflight HALTs if bd unavailable
- [ ] Session state auto-created per agent
- [ ] Session state auto-repaired if corrupted
- [ ] Track state validated before operations
- [ ] TDD:RED checkpoint updates bead notes (when `--tdd`)
- [ ] TDD:GREEN checkpoint updates bead notes (when `--tdd`)
- [ ] TDD:REFACTOR checkpoint updates bead notes (when `--tdd`)
- [ ] Task completion closes bead immediately
- [ ] Session end syncs to git (retry 3x)
- [ ] Unsynced state persisted on network failure
- [ ] Migration command works for existing tracks

### P1 (Should Have) - 12 criteria
- [ ] MA mode: Village init/claim atomic
- [ ] MA mode: File reservation via reserve/release
- [ ] MA mode: Handoff via file-as-message
- [ ] Subagents: Read-only bd access
- [ ] Subagents: Return structured results to main
- [ ] Status sync shows Conductor ↔ Beads discrepancies
- [ ] Status sync suggests reconciliation actions
- [ ] Revise reopens affected beads
- [ ] Revise links new beads to revised plan
- [ ] Compact generates AI summaries
- [ ] Cleanup removes old beads (>150)
- [ ] planTasks bidirectional mapping

### P2 (Nice to Have) - 11 criteria
- [ ] Inline comments in plan.md (`<!-- bd-42 -->`)
- [ ] Mode upgrade command (`/conductor-upgrade-mode`)
- [ ] Graceful degradation (MA without Village)
- [ ] Debug mode (verbose logging)
- [ ] Metrics collection (session duration, task count)
- [ ] Auto-migrate prompt for legacy tracks
- [ ] Stale agent detection (lastSeen > 10 min)
- [ ] Handoff TTL (24hr expiry with warning)
- [ ] Idempotent pending operations
- [ ] Recovery from orphaned state files
- [ ] CODEMAPS update after major changes

---

## Implementation Plan

### Week 1: Foundation
- Component 7: State Validation
- Component 10: Master Reference (beads-facade.md, beads-integration.md)
- Component 1: Preflight System
- workflows/conductor/preflight-beads.md

### Week 2: Core Single-Agent
- Component 2: Claim System
- Component 3: TDD Checkpoints
- Component 4: Close/Sync System
- Component 9: Track Init
- workflows/conductor/beads-session.md
- workflows/conductor/track-init-beads.md

### Week 3: Multi-Agent + Extensions
- Component 5: Task Tool Injection
- Component 6: Migration Command
- Component 11: Status Sync
- Component 12: Revise/Reopen
- workflows/conductor/status-sync-beads.md
- workflows/conductor/revise-reopen-beads.md

### Week 4: Finish + Polish
- Component 8: Compact/Cleanup
- All test scenarios (52 total)
- Documentation updates
- AGENTS.md refresh

---

## Test Scenarios (52 total)

| Category | Count | IDs |
|----------|-------|-----|
| Single-agent happy path | 3 | T1-T3 |
| Multi-agent coordination | 6 | T4-T9 |
| Preflight failures | 5 | T10-T14 |
| Claim edge cases | 9 | T15-T23 |
| TDD checkpoint failures | 9 | T24-T32 |
| Sync failures | 6 | T33-T38 |
| Migration scenarios | 7 | T39-T45 |
| Validation scenarios | 4 | T46-T49 |
| Status sync scenarios | 3 | T50-T52 |
| Clarifications coverage | 8 | T53-T60 |

### Clarifications Test Scenarios (T53-T60)

| ID | Scenario | Expected |
|----|----------|----------|
| T53 | Mode precedence: existing session-state | Use locked mode, ignore preference |
| T54 | Session lock heartbeat stale (>10 min) | Auto-unlock, warn user |
| T55 | Session lock heartbeat active (<10 min) | Prompt C/W/F |
| T56 | First-claim-wins with timestamp tie | Lexicographic agent ID tie-breaker |
| T57 | Pending operations replay (idempotent) | Skip already-applied, replay pending |
| T58 | Handoff file 24hr TTL expiry | Warn before delete, log reason |
| T59 | Graceful degradation MA→SA mid-session | Continue with warning, preserve mode in state |
| T60 | Compaction vs Revise: cleaned-up bead | Create new bead with lineage, update mapping |

---

## Decision Log

| Decision | Rationale |
|----------|-----------|
| Dual-mode (MCP + CLI) | Flexibility, backward compat |
| Facade pattern | Single integration point, easy to test |
| Session state per-agent | Avoid race conditions in MA |
| HALT on bd unavailable | No silent degradation |
| TDD opt-in via `--tdd` | Not all workflows use TDD |
| Close with reason | Support skip/blocked, not just completed |
| plan.md validation | Graceful handling of freeform plans |
| Subagents read-only | Main agent centralizes writes |
| File-as-message handoff | Simple, no contention |
| Compact output default | Token efficiency |

---

## Party Mode Refinements Summary

### Brainstorm (Carson 🧠)
- Mode auto-detect → Lock at start, explicit upgrade
- Shared + private state → Per-agent files
- Subagent queue → Return data to main, no bd writes
- Bidirectional mapping → `.fb-progress.json` is truth

### Stress Test (Dr. Quinn 🔬)
- Mode upgrade needs sync-first precondition
- Subagents need read-only bd access
- Handoff collision fix: include sender in filename
- MA without Village: graceful degradation with warning

### Technical Panel (Winston 🏗️, Amelia 💻, Murat 🧪)
- Facade pattern for single coupling point
- Per-agent files merged on read
- Extensive test suite before ship
- Clear SA/MA boundary

### Strategic Panel (Mary 📊, Victor ⚡)
- Measure before expanding
- Instrument usage patterns
- Data-driven Phase 3 decisions

### Final Panel (John 📋, Maya 🎯, Sophia 📖, Sally 🎨)
- Close immediately (crash-safe)
- "Your progress, protected" positioning
- Skip Claim if needed, never skip Close
- TDD as opt-in, not requirement

---

## Oracle Audit Summary

| Category | Count | P0 | P1 | P2 | Est. Lines |
|----------|-------|----|----|----|-----------:|
| Files to Create | 11 | 6 | 2 | 3 | 1,330–1,810 |
| Files to Modify | 11 | 6 | 3 | 2 | 630–970 |
| **Total** | **22** | **12** | **5** | **5** | **~2,000–2,800** |

---

## Clarifications

### Mode Selection Rules

**Precedence (when both bd and Village available):**

```
1. Check for existing session-state file
   └─► If exists with mode locked → use that mode (no change mid-session)

2. Check user preference (.conductor/preferences.json)
   └─► If "preferredMode": "SA" or "MA" → use preference

3. Check Village MCP availability
   └─► If Village available AND no preference → default to MA
   └─► If Village unavailable → use SA

4. Fallback
   └─► SA mode (always available if bd is available)
```

**HALT vs Degrade:**

| Condition | Action |
|-----------|--------|
| bd unavailable | **HALT** - cannot proceed |
| Village unavailable, session started as SA | Continue SA (no change) |
| Village unavailable, session started as MA | **Degrade to SA** with warning |
| Village flaps mid-session (available → unavailable) | Continue in degraded SA mode |
| bd fails mid-session | Retry 3x → persist unsynced → warn user |

### Mode Upgrade Preconditions

`/conductor-upgrade-mode` (SA → MA) requires:

1. **No in-progress tasks**: All claimed tasks must be closed or released
2. **Sync-first**: `bd sync` must succeed before upgrade
3. **Village available**: Must detect Village MCP
4. **Explicit confirmation**: User must confirm upgrade

```
Pre-upgrade checklist:
- [ ] No tasks in_progress (bd list --status in_progress --json → empty)
- [ ] bd sync succeeds
- [ ] Village MCP responds to ping
- [ ] User confirms: "Upgrade to multi-agent mode? [Y/n]"
```

### Per-Agent State Merge Rules

When reading team state (MA mode), merge per-agent files:

```
.conductor/
├── session-state_T-abc.json  # Agent A
├── session-state_T-def.json  # Agent B
└── session-state_T-ghi.json  # Agent C
```

**Merge algorithm:**

1. Read all `session-state_*.json` files
2. Filter: include only agents with `lastUpdated` < 10 min ago
3. Conflict resolution:
   - Same task claimed by multiple agents → first claim wins (by `modeLockedAt` timestamp; if tied, lexicographic agent ID)
   - Same file reserved by multiple agents → error, escalate to leader
4. Output: unified team state object

```json
{
  "agents": {
    "T-abc": { "status": "online", "currentTask": "bd-42", "lastUpdated": "..." },
    "T-def": { "status": "online", "currentTask": "bd-43", "lastUpdated": "..." },
    "T-ghi": { "status": "stale", "currentTask": null, "lastUpdated": "..." }
  },
  "conflicts": []
}
```

### Migration as Integration Point

Migration is **external to the 13-point lifecycle** but triggered by preflight:

```
Preflight detects:
├── Track exists
├── plan.md exists
├── .fb-progress.json missing OR planTasks empty
└─► Prompt: "Track not integrated with Beads. Run /conductor-migrate-beads? [Y/n]"
```

Migration is NOT an automatic integration point because:
- It requires user confirmation
- It's a one-time operation per track
- It may link to existing beads (user decision)

### Non-Sync bd Failure Handling

| Operation | Retry | Fallback |
|-----------|-------|----------|
| `bd ready` | 1x | Return empty, warn "Could not fetch ready tasks" |
| `bd update` | 3x | Persist to `.conductor/pending_updates.jsonl`, warn user |
| `bd close` | 3x | Persist to `.conductor/pending_closes.jsonl`, warn user |
| `bd show` | 1x | Return cached if available, else error |
| `bd create` | 3x | HALT - cannot create without success |
| `bd sync` | 3x | Persist unsynced, warn user |

**Pending operations file format:**

```jsonl
{"op": "update", "id": "bd-42", "args": ["--status", "in_progress"], "ts": "...", "retries": 3}
{"op": "close", "id": "bd-43", "args": ["--reason", "completed"], "ts": "...", "retries": 3}
```

**Recovery:** On next successful session, preflight checks for pending files and replays.

### Concurrent SA Session Detection

**Problem:** Two SA sessions on same track can cause conflicts.

**Solution:** Session lock file:

```
.conductor/session-lock_<track-id>.json
{
  "agentId": "T-abc",
  "lockedAt": "2025-12-25T10:00:00Z",
  "lastHeartbeat": "2025-12-25T10:25:00Z",
  "pid": 12345  // process ID if available
}
```

**Heartbeat Protocol:**

Active sessions update `lastHeartbeat` every 5 minutes. This enables:
- Early stale detection (heartbeat > 10 min ago = likely abandoned)
- Accurate TTL (lock age vs activity age distinction)

**Behavior:**

1. On session start, check for lock file
2. If lock exists AND `lastHeartbeat` < 10 min ago:
   - Warn: "Another session is active on this track"
   - Prompt: "[C]ontinue anyway / [W]ait / [F]orce unlock"
3. If lock exists AND `lastHeartbeat` > 10 min ago:
   - Auto-unlock (stale session - no recent heartbeat)
   - Warn: "Recovered from stale session lock (no heartbeat for >10 min)"
4. During session, update `lastHeartbeat` every 5 minutes
5. On session end, delete lock file

### Compaction vs Revise Conflict

**Problem:** Bead was compacted/cleaned up, now needs reopening.

**Resolution:**

1. `/conductor-revise` checks if bead exists:
   - If exists → reopen
   - If deleted (cleaned up) → create NEW bead with reference to old ID

2. New bead includes lineage:
   ```json
   {
     "title": "Rework: Original task title",
     "notes": "Reopened from cleaned-up bd-42. Original closed 2025-12-20.",
     "metadata": {
       "originalBeadId": "bd-42",
       "reopenedAt": "2025-12-25T10:00:00Z",
       "reopenReason": "spec revision"
     }
   }
   ```

3. planTasks mapping updates to point to new bead ID

### P2 Criteria Precision

#### Graceful Degradation (P2)

**Definition:** MA mode continues operating when Village becomes unavailable.

**Behavior:**

```
Village becomes unavailable:
├── Log warning: "Village MCP unavailable. Operating in degraded mode."
├── Switch internal operations to bd CLI
├── File reservations: SKIP (cannot enforce)
├── Claim operations: Use bd update (no atomic guarantee)
├── Handoffs: Write to .conductor/handoff_*.json (will be picked up when Village returns)
└── Mode stays "MA" in session state (for recovery)
```

**What DOESN'T work in degraded mode:**
- Atomic task claiming (race possible)
- File reservation enforcement
- Real-time team status

#### Debug Mode (P2)

**Definition:** Verbose logging for troubleshooting.

**Enabled by:** `--debug` flag OR `CONDUCTOR_DEBUG=1` env var

**What gets logged:**

| Category | Log Content | Destination |
|----------|-------------|-------------|
| Preflight | Mode detection steps, version checks | stderr |
| bd commands | Full command + args + exit code + output | `.conductor/debug.log` |
| State file ops | Read/write operations with content | `.conductor/debug.log` |
| Timing | Duration of each operation | `.conductor/debug.log` |
| Errors | Full stack traces | `.conductor/debug.log` |

**Log format:**

```
[2025-12-25T10:30:00.123Z] [DEBUG] [preflight] Checking bd availability...
[2025-12-25T10:30:00.234Z] [DEBUG] [preflight] bd version: 0.5.2
[2025-12-25T10:30:00.345Z] [DEBUG] [bd] Running: bd ready --json
[2025-12-25T10:30:00.456Z] [DEBUG] [bd] Exit code: 0, Output: [...]
```

**Log rotation:** Keep last 5 debug.log files, max 10MB each.

#### Idempotent Pending Operations (P2)

**Definition:** Pending operations can be safely replayed without side effects.

**Idempotency keys:**

```jsonl
{
  "op": "update", 
  "id": "bd-42", 
  "idempotencyKey": "T-abc_1703509200_update_bd-42",
  "args": ["--status", "in_progress"],
  "ts": "2025-12-25T10:00:00Z"
}
```

**Key format:** `<agent-id>_<unix-timestamp>_<operation>_<bead-id>`

**Replay behavior:**

1. Before executing, check if operation already applied:
   - For `update`: check current status matches target
   - For `close`: check bead is already closed
2. If already applied → skip, log "Already applied: <key>"
3. If not applied → execute, log "Replayed: <key>"

**Idempotent operations:**
- `bd update --status X` → idempotent (same status = no-op)
- `bd close` → idempotent (already closed = no-op)
- `bd create` → NOT idempotent (would create duplicate)

**Non-idempotent handling:**
- `bd create` failures are HALT (not persisted for retry)
- User must manually resolve duplicate creation

#### Orphan Handoff Detection (P2)

**Definition:** Handoff files not picked up by target agent within expected time.

**Detection:**

Preflight scans `.conductor/handoff_*.json` and warns if:
- File age > 12 hours (half of 24hr TTL)
- Target agent not seen online since handoff created

**Warning format:**

```
⚠️ Orphan handoff detected:
   From: T-abc → To: T-def
   Created: 12 hours ago
   Content: "Review auth changes before merge"
   Action: Target agent T-def has not been online. Consider reassigning.
```

**Cleanup:**

- At 24hr, log final warning with full content
- Delete file after logging
- Record in metrics.jsonl for analysis

---

## References

- [Beads workflow](../../workflows/beads/workflow.md)
- [Village MCP](https://github.com/LNS2905/mcp-beads-village)
- [Track validation](../../skills/conductor/references/validation/track/checks.md)
