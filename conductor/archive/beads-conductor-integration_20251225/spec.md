# Beads-Conductor Integration Specification

**Track ID:** beads-conductor-integration_20251225  
**Version:** 1.0.0  
**Status:** Ready for Implementation

---

## 1. Overview

### 1.1 Purpose

Integrate Beads issue tracking into Conductor workflows to achieve **zero manual bd commands** in the happy path. This specification defines the requirements for a 13-point integration covering the full lifecycle: preflight, session management, task execution, and track completion.

### 1.2 Scope

| In Scope | Out of Scope |
|----------|--------------|
| Preflight mode detection (SA/MA) | Changes to Beads CLI (`bd`) |
| Session lifecycle automation | Changes to Village MCP server |
| TDD checkpoint integration | New slash commands |
| Track initialization with beads | Real-time sync (batch at session end) |
| Status sync and revise/reopen | External issue tracker integration |
| Compact/cleanup at finish | |

### 1.3 Key Terms

| Term | Definition |
|------|------------|
| **SA Mode** | Single-Agent mode using `bd` CLI directly |
| **MA Mode** | Multi-Agent mode using Village MCP for coordination |
| **Facade** | Single integration layer abstracting Beads operations |
| **planTasks** | Mapping of plan items to bead IDs |
| **Preflight** | Validation run at session/command start |

---

## 2. Functional Requirements

### 2.1 Preflight System (FR-001)

**Priority:** P0

The system SHALL:

- FR-001.1: Detect bd CLI availability at session start
- FR-001.2: HALT with clear message if bd is unavailable (no silent skip)
- FR-001.3: Detect Village MCP availability for MA mode
- FR-001.4: Lock mode (SA or MA) for the entire session
- FR-001.5: Create per-agent session state file (`.conductor/session-state_<agent-id>.json`)
- FR-001.6: Recover from crashed sessions by reading stale state files
- FR-001.7: Detect stale agents (lastSeen > 10 min) in MA mode

### 2.2 Session Lifecycle (FR-002)

**Priority:** P0

The system SHALL:

- FR-002.1: Claim tasks via `bd update --status in_progress` (SA) or `claim()` (MA)
- FR-002.2: Support parallel task claiming: `bd update id1 id2 --status in_progress`
- FR-002.3: Close tasks with reason: `--reason completed|skipped|blocked`
- FR-002.4: Sync to git at session end via `bd sync`
- FR-002.5: Retry sync 3 times on failure
- FR-002.6: Persist unsynced state if all retries fail
- FR-002.7: Release all file reservations on session end (MA)

### 2.3 TDD Checkpoints (FR-003)

**Priority:** P1

When `--tdd` flag is enabled, the system SHALL:

- FR-003.1: Update bead notes on RED phase (test written)
- FR-003.2: Update bead notes on GREEN phase (test passes)
- FR-003.3: Update bead notes on REFACTOR phase (code clean)
- FR-003.4: Use notes format: `IN_PROGRESS: <phase> phase`
- FR-003.5: Skip checkpoints if no tests detected

### 2.4 Track Initialization (FR-004)

**Priority:** P0

On `/conductor-newtrack`, the system SHALL:

- FR-004.1: Validate plan.md structure before creating beads
- FR-004.2: Present R/S/M prompt on malformed plan (Reformat/Skip/Manual)
- FR-004.3: Support `--strict` flag to fail on malformed (for CI)
- FR-004.4: Create epic from plan.md title
- FR-004.5: Create issues from plan.md tasks
- FR-004.6: Wire dependencies between issues
- FR-004.7: Update `.fb-progress.json` with planTasks mapping
- FR-004.8: Maintain bidirectional mapping (planTasks + beadToTask)

### 2.5 Subagent Rules (FR-005)

**Priority:** P1

For Task tool subagents, the system SHALL:

- FR-005.1: Allow read-only bd access: `bd show`, `bd ready`, `bd list`
- FR-005.2: Block write operations: `bd update`, `bd close`, `bd create`
- FR-005.3: Require subagents to return structured results to main agent
- FR-005.4: Main agent executes all bead writes

### 2.6 Multi-Agent Coordination (FR-006)

**Priority:** P1

In MA mode, the system SHALL:

- FR-006.1: Use Village `init()` → `claim()` for atomic task claiming
- FR-006.2: Use `reserve()` for file locking before edits
- FR-006.3: Use `release()` or auto-release on `done()`
- FR-006.4: Support file-as-message handoff: `.conductor/handoff_<from>_to_<to>.json`
- FR-006.5: Process handoff files in timestamp order
- FR-006.6: Apply 24hr TTL to handoff files with warning

### 2.7 Status Sync (FR-007)

**Priority:** P2

On `/conductor-status`, the system SHALL:

- FR-007.1: Query both Conductor state and Beads state
- FR-007.2: Compare and detect discrepancies
- FR-007.3: Display discrepancies to user
- FR-007.4: Suggest reconciliation actions

### 2.8 Revise/Reopen (FR-008)

**Priority:** P2

On `/conductor-revise`, the system SHALL:

- FR-008.1: Identify beads affected by spec/plan changes
- FR-008.2: Reopen closed beads that need rework
- FR-008.3: Create new beads for added plan items
- FR-008.4: Preserve original bead history
- FR-008.5: Update planTasks mapping

### 2.9 Compact/Cleanup (FR-009)

**Priority:** P1

On `/conductor-finish`, the system SHALL:

- FR-009.1: Generate AI summaries for closed beads via `bd compact`
- FR-009.2: Trigger cleanup when closed beads > 150
- FR-009.3: Remove oldest closed beads via `bd cleanup`
- FR-009.4: Clean up Village state files

### 2.10 Migration Command (FR-010)

**Priority:** P1

`/conductor-migrate-beads` SHALL:

- FR-010.1: Scan existing tracks without beads integration
- FR-010.2: Analyze plan.md to identify tasks
- FR-010.3: Confirm migration plan with user
- FR-010.4: Execute: create beads or link existing
- FR-010.5: Verify: validate planTasks mapping

---

## 3. Non-Functional Requirements

### 3.1 Performance (NFR-001)

- NFR-001.1: Preflight SHALL complete in < 2 seconds
- NFR-001.2: Track-init SHALL complete in < 10 seconds for plans with < 50 tasks
- NFR-001.3: Sync SHALL complete in < 5 seconds (excluding network latency)

### 3.2 Reliability (NFR-002)

- NFR-002.1: Crash recovery SHALL restore state without data loss
- NFR-002.2: Sync retry logic SHALL persist unsynced operations for later retry
- NFR-002.3: State file corruption SHALL HALT with clear error, not auto-repair

### 3.3 Usability (NFR-003)

- NFR-003.1: Zero manual bd commands in happy path
- NFR-003.2: Clear error messages with recovery actions
- NFR-003.3: Compact output mode by default, verbose with `--verbose`

### 3.4 Compatibility (NFR-004)

- NFR-004.1: Compatible with existing Conductor tracks
- NFR-004.2: Compatible with existing Beads databases
- NFR-004.3: Migration path for pre-integration tracks
- NFR-004.4: Works with bd CLI v0.5.x+

### 3.5 Security (NFR-005)

- NFR-005.1: No secrets stored in state files
- NFR-005.2: Session state files use agent ID, not sensitive data
- NFR-005.3: Handoff files do not contain credentials

---

## 4. Interface Requirements

### 4.1 Facade Contract

```typescript
interface BeadsFacade {
  checkAvailability(): { 
    available: boolean; 
    version?: string; 
    error?: string 
  }
  
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
  
  syncToGit(options?: { retries?: number }): { 
    success: boolean; 
    synced: number; 
    unsynced?: string[];
  }
  
  claimTask(taskId: string, mode: 'SA' | 'MA'): {
    success: boolean;
    alreadyClaimed?: boolean;
  }
  
  closeTask(taskId: string, reason: 'completed' | 'skipped' | 'blocked'): {
    success: boolean;
  }
  
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

### 4.2 State File Schemas

#### Session State (`session-state_<agent-id>.json`)

```json
{
  "agentId": "string (required)",
  "mode": "SA | MA (required)",
  "modeLockedAt": "ISO 8601 timestamp (required)",
  "trackId": "string (optional)",
  "currentTask": "string (optional)",
  "tddPhase": "RED | GREEN | REFACTOR | null",
  "lastUpdated": "ISO 8601 timestamp (required)"
}
```

#### Beads Progress (`.fb-progress.json`)

```json
{
  "trackId": "string (required)",
  "status": "pending | in_progress | complete | failed (required)",
  "startedAt": "ISO 8601 timestamp | null",
  "threadId": "string | null",
  "resumeFrom": "string",
  "epics": ["string"],
  "issues": ["string"],
  "planTasks": { "taskId": "beadId" },
  "beadToTask": { "beadId": "taskId" },
  "crossTrackDeps": ["string"],
  "lastError": "string | null",
  "lastVerified": "ISO 8601 timestamp | null"
}
```

---

## 5. Acceptance Criteria

### 5.1 P0 Criteria (Must Pass for Release)

| ID | Criterion | Verification |
|----|-----------|--------------|
| AC-001 | Preflight detects bd availability | Unit test |
| AC-002 | Preflight HALTs if bd unavailable | Unit test |
| AC-003 | Session state created per agent | Integration test |
| AC-004 | Track-init creates epic from plan | Integration test |
| AC-005 | Track-init creates issues with dependencies | Integration test |
| AC-006 | Close updates bead with reason | Unit test |
| AC-007 | Sync retries 3x on failure | Unit test |
| AC-008 | Sync persists unsynced on final failure | Integration test |
| AC-009 | planTasks mapping correct | Integration test |
| AC-010 | Migration command links existing beads | Integration test |
| AC-011 | TDD checkpoints update notes (when --tdd) | Integration test |
| AC-012 | R/S/M prompt on malformed plan | Integration test |

### 5.2 P1 Criteria (Should Pass)

| ID | Criterion | Verification |
|----|-----------|--------------|
| AC-013 | MA mode uses Village claim | Integration test |
| AC-014 | File reservation in MA mode | Integration test |
| AC-015 | Handoff file-as-message works | Integration test |
| AC-016 | Subagents restricted to read-only | Unit test |
| AC-017 | Compact generates AI summaries | Integration test |
| AC-018 | Cleanup removes old beads | Integration test |

### 5.3 P2 Criteria (Nice to Have)

| ID | Criterion | Verification |
|----|-----------|--------------|
| AC-019 | Status sync detects discrepancies | Integration test |
| AC-020 | Revise reopens affected beads | Integration test |
| AC-021 | Graceful degradation MA→SA | Integration test |
| AC-022 | Metrics logged to metrics.jsonl | Unit test |

---

## 6. Dependencies

### 6.1 External Dependencies

| Dependency | Version | Required |
|------------|---------|----------|
| bd CLI | ≥ 0.5.0 | Yes |
| bv CLI | ≥ 0.3.0 | No (MA mode only) |
| jq | ≥ 1.6 | Yes |
| Git | ≥ 2.0 | Yes |

### 6.2 Internal Dependencies

| Dependency | Purpose |
|------------|---------|
| skills/conductor/SKILL.md | Conductor skill definition |
| workflows/implement.md | Implementation workflow |
| workflows/finish.md | Finish workflow |
| skills/beads/SKILL.md | Beads skill definition |

---

## 7. Risks

| Risk | Severity | Likelihood | Mitigation |
|------|----------|------------|------------|
| Behavioral change confuses users | High | Medium | Documentation, introduction modal |
| Partial failure leaves inconsistent state | High | Low | State machine with recovery |
| MA race conditions | High | Low | Atomic Village operations |
| bd CLI version incompatibility | Medium | Low | Version check in preflight |
| plan.md format variations | Medium | High | Validation + R/S/M prompt |

---

## 8. References

- [Design Document](design.md)
- [Beads Workflow](../../workflows/beads/workflow.md)
- [Track Validation](../../skills/conductor/references/validation/track/checks.md)
- [Village MCP](https://github.com/LNS2905/mcp-beads-village)

---

## Appendix A: Clarifications

### A.1 Mode Selection Precedence

```
1. Existing session-state file → use locked mode
2. User preference (preferences.json) → use preferred mode
3. Village available + no preference → default to MA
4. Fallback → SA mode
```

### A.2 HALT vs Degrade

| Condition | Action |
|-----------|--------|
| bd unavailable | HALT |
| Village unavailable, started as SA | Continue SA |
| Village unavailable, started as MA | Degrade to SA with warning |
| bd fails mid-session | Retry 3x → persist → warn |

### A.3 Mode Upgrade Preconditions

1. No in-progress tasks
2. `bd sync` succeeds
3. Village MCP available
4. User confirmation

### A.4 Non-Sync bd Failure Handling

| Operation | Retry | Fallback |
|-----------|-------|----------|
| `bd ready` | 1x | Return empty, warn |
| `bd update` | 3x | Persist to pending_updates.jsonl |
| `bd close` | 3x | Persist to pending_closes.jsonl |
| `bd create` | 3x | HALT |
| `bd sync` | 3x | Persist unsynced |

### A.5 Concurrent SA Session Detection

Session lock file `.conductor/session-lock_<track-id>.json` with heartbeat:
- Heartbeat < 10 min: Prompt C/W/F (active session)
- Heartbeat > 10 min: Auto-unlock (stale)
- Heartbeat updated every 5 min during active session

### A.6 Compaction vs Revise Conflict

If bead was cleaned up but needs reopening:
- Create NEW bead with lineage reference to original
- Update planTasks mapping to new bead ID
