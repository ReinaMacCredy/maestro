# Implement Workflow

## Purpose
Execute tasks from a track's plan following the defined workflow methodology (TDD, commits, documentation).

## Prerequisites
- Conductor environment initialized
- Required files exist:
  - `conductor/tech-stack.md`
  - `conductor/workflow.md`
  - `conductor/product.md`
- At least one track exists in `conductor/tracks/`
- Beads CLI (`bd`) available (checked by preflight)

## State Management

### State File
`conductor/tracks/<track_id>/implement_state.json` (optional)

### State Values
| Field | Type | Description |
|-------|------|-------------|
| `current_phase` | string | Active phase name |
| `current_task` | string | Active task description |
| `status` | string | `in_progress`, `paused`, `blocked` |
| `last_commit_sha` | string | Last implementation commit |

## Workflow Steps

### Phase 0: Beads Preflight

**REQUIRED:** Run before any implementation work.

1. **Execute Preflight**
   - Run [preflight-beads.md](../conductor/preflight-beads.md) workflow
   - Checks `bd` availability (HALT if unavailable)
   - Detects mode (SA or MA) and locks for session
   - Creates session state file
   - Recovers pending operations from crashed sessions

2. **Check Track Beads**
   - Verify `metadata.json` exists with `beads` section for track
   - If missing: Prompt to run `/conductor-newtrack` or `/conductor-migrate-beads`

3. **Output:**
   ```
   Preflight: bd v0.5.2 ✓, Village ✗ → SA mode
   Session: Created state file for T-abc123
   Track beads: 12 issues, 3 ready
   ```

### Phase 0.5: Continuity Load

**Purpose:** Load prior session context and handle track binding.

1. **Load LEDGER.md**
   - Run `continuity load` workflow
   - Read `conductor/sessions/active/LEDGER.md` if exists
   - Display prior context summary

2. **Check Track Binding**
   - If `bound_track` exists in LEDGER frontmatter:
     - Compare with current track
     - If different: Auto-archive current LEDGER before proceeding
     - Display: `Previous session: <track> → Archived`
   - If same track: Resume context
   - If no bound_track: Fresh session

3. **Bind to Track**
   - Update LEDGER frontmatter: `bound_track: <track_id>`
   - Update `heartbeat` timestamp
   
4. **Output:**
   ```
   Continuity: Loaded prior context (3 decisions, 5 modified files)
   Session: Binding to track auth_20251227
   ```

**Non-blocking:** If LEDGER.md missing or corrupted, create fresh session.

### Phase 1: Setup Verification

1. **Check Required Files**
   - Verify all prerequisite files exist
   - If missing: Halt with message to run `/conductor:setup`

### Phase 2: Track Selection

1. **Parse Tracks File**
   - Read `conductor/tracks.md`
   - Split by `---` separator
   - Extract: status, description, folder link

2. **Select Track**
   - **If track name provided**:
     - Case-insensitive match against descriptions
     - Confirm selection with user
   - **If no track provided**:
     - Find first non-`[x]` track
     - Auto-select and announce
   - **If all complete**:
     - Announce all tasks done, halt

### Phase 2b: Execution Routing

**Purpose:** Determine whether to execute tasks sequentially (SINGLE_AGENT) or in parallel (PARALLEL_DISPATCH).

1. **Evaluate TIER 1** (weighted score):
   
   | Factor | Weight |
   |--------|--------|
   | Epics > 1 | +2 |
   | [PARALLEL] markers in plan | +3 |
   | Domains > 2 | +2 |
   | Independent tasks > 5 | +1 |
   
   **Threshold:** Score >= 5 to proceed to TIER 2

2. **Evaluate TIER 2** (if TIER 1 passes):
   
   ```python
   (files > 15 AND tasks > 3) OR
   (est_tool_calls > 40) OR
   (est_time > 30 min AND independent_ratio > 0.6)
   ```

3. **Route Decision:**
   
   | TIER 1 | TIER 2 | Result |
   |--------|--------|--------|
   | FAIL | - | SINGLE_AGENT |
   | PASS | FAIL | SINGLE_AGENT |
   | PASS | PASS | PARALLEL_DISPATCH |

4. **Display Feedback:**
   ```text
   ┌─ EXECUTION ROUTING ────────────────────┐
   │ TIER 1 Score: 6/8                      │
   │ TIER 2: PASS                           │
   │ Result: PARALLEL_DISPATCH              │
   └────────────────────────────────────────┘
   ```

5. **Update State:**
   
   Add to `implement_state.json`:
   ```json
   {
     "execution_mode": "PARALLEL_DISPATCH",
     "routing_evaluation": {
       "tier1_score": 6,
       "tier1_pass": true,
       "tier2_pass": true
     }
   }
   ```

6. **Branch Logic:**
   - **SINGLE_AGENT:** Continue to Phase 3 (sequential execution)
   - **PARALLEL_DISPATCH:** Use [parallel-dispatch.md](../../../dispatching-parallel-agents/references/agent-coordination/patterns/parallel-dispatch.md) pattern

See [execution-routing.md](../../../dispatching-parallel-agents/references/agent-coordination/patterns/execution-routing.md) for full scoring details.

### Phase 3: Track Implementation

1. **Update Status**
   - Change track status `[ ]` → `[~]` in `tracks.md`

2. **Load Context**
   - Read:
     - `conductor/tracks/<track_id>/plan.md`
     - `conductor/tracks/<track_id>/spec.md`
     - `conductor/workflow.md`
     - `metadata.json.beads` for planTasks mapping

3. **Claim Task (Beads Integration)**
   
   **SA Mode:**
   ```bash
   bd ready --json                           # Get available tasks
   bd update <task-id> --status in_progress  # Claim task
   ```
   
   **MA Mode:**
   ```bash
   claim()                                   # Atomic claim (race-safe)
   reserve path="<file>"                     # Lock files before edit
   ```
   
   See [beads-session.md](../conductor/beads-session.md) for full protocol.

4. **Execute Tasks**
   - Iterate through `plan.md` tasks sequentially
   - For each task, defer to `workflow.md` Task Workflow section
   - Follow TDD cycle (default, use `--no-tdd` to disable):
     1. Mark task `[~]` in progress
     2. Write failing tests (Red)
     3. Implement to pass (Green)
     4. Refactor
     5. Verify coverage (>80%)
     6. Commit with conventional message
     7. Attach git note summary
     8. Update `plan.md`: `[~]` → `[x]` + SHA
     9. Commit plan update

5. **TDD Checkpoints (default, skip with `--no-tdd`)**
   
   Unless `--no-tdd` is provided, update bead notes at each phase:
   
   | Phase | Trigger | Notes Update |
   |-------|---------|--------------|
   | RED | Test written/fails | `IN_PROGRESS: RED phase - writing failing test` |
   | GREEN | Test passes | `IN_PROGRESS: GREEN phase - making test pass` |
   | REFACTOR | Code cleaned | `IN_PROGRESS: REFACTOR phase - cleaning up code` |
   
   ```bash
   # After test passes (GREEN phase)
   bd update <task-id> --notes "IN_PROGRESS: GREEN phase - making test pass"
   ```

6. **Degradation Evaluation (after each task)**
   
   After each task completion, evaluate degradation signals:
   
   | Signal | Threshold | Trigger |
   |--------|-----------|---------|
   | `tool_repeat` | file_write: 3, bash: 3, search: 5, file_read: 10 | Same tool on same target exceeds threshold |
   | `backtrack` | 1 | Revisiting completed task |
   | `quality_drop` | 1 | Test failures increase OR new lint errors |
   | `contradiction` | 1 | Output conflicts with prior Decisions |
   
   **Action:** If 2+ signals fire → trigger context compression
   
   See [Degradation Signals](../../../beads/references/workflow.md#degradation-signals) for full details.

7. **Close Task (Beads Integration)**
   
   After task completion:
   
   **SA Mode:**
   ```bash
   bd update <task-id> --notes "COMPLETED: <summary>. KEY DECISION: <if any>"
   bd close <task-id> --reason completed
   ```
   
   **MA Mode:**
   ```bash
   done taskId="<task-id>" reason="completed"  # Auto-releases reservations
   ```
   
   **Close Reasons:**
   - `completed` - Task finished successfully
   - `skipped` - Task not needed (requirements changed)
   - `blocked` - Cannot proceed, external dependency

8. **Phase Completion**
   - Execute Phase Completion Protocol from `workflow.md`
   - Includes: test verification, manual verification, checkpoint commit

9. **Finalize Track**
   - Update status `[~]` → `[x]` in `tracks.md`
   - Announce completion

### Phase 4: Documentation Sync

**Trigger**: Only when track reaches `[x]` status

1. **Load Context**
   - Read `spec.md` from completed track
   - Read project documents:
     - `conductor/product.md`
     - `conductor/product-guidelines.md`
     - `conductor/tech-stack.md`

2. **Analyze and Propose Updates**
   - **`product.md`**: If feature impacts product description
   - **`tech-stack.md`**: If technology changes detected
   - **`product-guidelines.md`**: Only for branding/voice changes (rare)

3. **Confirmation Loop**
   - Present proposed changes in diff format
   - Require explicit user approval
   - Apply only approved changes

4. **Report Summary**
   - List what was changed or not changed
   - Explain rationale

### Phase 5: Track Cleanup

1. **Present Options**
   ```
   A) Archive: Move to conductor/archive/
   B) Delete: Permanently remove
   C) Skip: Leave in tracks file
   ```

2. **Execute Choice**
   - **Archive**: Create archive dir, move track, update `tracks.md`
   - **Delete**: Confirm twice, remove, update `tracks.md`
   - **Skip**: No action

### Phase 6: Beads Sync

**REQUIRED:** Run at session end.

1. **Sync to Git**
   ```bash
   bd sync
   ```
   
2. **Retry on Failure**
   - Retry up to 3 times with backoff (1s, 2s)
   - On final failure: persist unsynced state to `.conductor/unsynced.json`
   
3. **Session Cleanup**
   - Remove session lock file
   - Update session state file

See [beads-session.md](../conductor/beads-session.md) for full sync protocol.

## Task Workflow Reference

From `workflow.md`:

1. Select task from `plan.md`
2. Mark `[~]` in progress
3. Write failing tests (Red)
4. Implement to pass (Green)
5. Refactor
6. Verify coverage
7. Document deviations if any
8. Commit code changes
9. Attach git note summary
10. Update `plan.md` with `[x]` and SHA
11. Commit plan update

## Error Handling

| Error | Action |
|-------|--------|
| Setup not complete | Halt, direct to `/conductor:setup` |
| No tracks found | Halt, direct to `/conductor:newTrack` |
| All tracks complete | Announce, halt |
| Test failure | Debug up to 2 attempts, then ask user |
| File read error | Stop, report error |
| Git conflict | Halt, provide resolution steps |
| bd unavailable | HALT with install instructions |
| bd command fails | Retry 3x, persist pending operations |
| Claim conflict (MA) | Pick different task or wait |
| Reservation conflict (MA) | Request access or pick different task |

## Output Artifacts

```
conductor/
├── tracks.md (updated statuses)
├── product.md (possibly updated)
├── tech-stack.md (possibly updated)
├── sessions/
│   └── active/
│       └── LEDGER.md (session state in frontmatter)
├── archive/ (if archiving)
│   └── <track_id>/
└── tracks/
    └── <track_id>/
        ├── plan.md (tasks marked complete)
        ├── metadata.json (planTasks mapping in beads section)
        └── implement_state.json (optional)

.conductor/
├── session-lock_<track-id>.json (concurrent session prevention)
├── pending_updates.jsonl (failed operations for retry)
├── pending_closes.jsonl (failed close operations)
└── unsynced.json (sync failures)
```

## Git Artifacts

- Implementation commits with conventional messages
- Plan update commits: `conductor(plan): Mark task 'X' as complete`
- Phase checkpoint commits: `conductor(checkpoint): Checkpoint end of Phase X`
- Git notes attached to commits with detailed summaries

## References

- [Beads Session Workflow](../conductor/beads-session.md) - Claim, close, sync protocol
- [Beads Preflight](../conductor/preflight-beads.md) - Session initialization
- [Beads Facade](../beads-facade.md) - API contract
- [Beads Integration](../beads-integration.md) - All 13 integration points
