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
   - Run [preflight-beads.md](../preflight-beads.md) workflow
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

### Phase 0.5: Handoff Load

**Purpose:** Load prior session context via handoff system.

1. **Load Most Recent Handoff**
   - Run `/resume_handoff` workflow internally
   - Check `conductor/handoffs/<track_id>/` for latest handoff
   - If found: Display context summary
   - If not found: Fresh session (no prior context)

2. **Create Epic-Start Handoff**
   
   Before starting each epic, create handoff with trigger `epic-start`:
   
   ```
   handoff_dir = conductor/handoffs/<track_id>/
   
   1. Create handoff file: YYYY-MM-DD_HH-MM-SS-mmm_<track>_<epic-id>_epic-start.md
   2. Include:
      - Epic scope from plan.md
      - Dependencies from beads
      - Prior context loaded
      - Expected deliverables
   3. Append to index.md
   4. Touch conductor/.last_activity
   ```
   
   See [../handoff/triggers.md](../handoff/triggers.md) for trigger details.

3. **Output:**
   ```
   Handoff: Loaded prior context (3 decisions, 5 modified files)
   Session: Binding to track auth_20251227
   ```

**Non-blocking:** If no handoff found, create fresh session.

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

> **Note:** `/conductor-implement` is a wrapper that auto-routes to `/conductor-orchestrate` when parallel execution is appropriate.

1. **Check Track Assignments (Priority 1)**
   
   ```python
   plan = Read("conductor/tracks/<track-id>/plan.md")
   
   if "## Track Assignments" in plan:
       # Explicit parallel execution requested
       return PARALLEL_DISPATCH
   ```
   
   **If Track Assignments section exists → immediately route to orchestrator.**

2. **Auto-Detect from Beads (Priority 2)**
   
   When no explicit Track Assignments exist, auto-detect parallel opportunities from metadata.json:
   
   ```python
   metadata = Read("conductor/tracks/<track-id>/metadata.json")
   
   # Check if beads section has planTasks mapping
   if "beads" in metadata and "planTasks" in metadata["beads"]:
       # Get bead IDs from metadata
       bead_ids = list(metadata["beads"]["planTasks"].values())
       
       # Verify with bd list at runtime (source of truth)
       live_beads_raw = bash("bd list --json")
       live_beads = {b['id']: b for b in json.loads(live_beads_raw)}
       
       # Analyze dependency graph for independent beads
       independent_beads = []
       for bead_id in bead_ids:
           if bead_id in live_beads and not live_beads[bead_id].get("dependencies"):
               independent_beads.append(bead_id)
       
       # Threshold: 2+ independent beads triggers auto-orchestration
       if len(independent_beads) >= 2:
           # Group by file scope (same directory = same track)
           tracks = group_by_file_scope(independent_beads)
           # Auto-generate Track Assignments and route to orchestrator
           return PARALLEL_DISPATCH
   ```
   
   See [auto-routing.md](../../../orchestrator/references/auto-routing.md) for full algorithm.

3. **Check Agent Mail Availability (Priority 3)**
   
   ```python
   try:
       ensure_project(human_key=PROJECT_PATH)
       AGENT_MAIL_AVAILABLE = True
   except McpUnavailable:
       AGENT_MAIL_AVAILABLE = False
       # Cannot do parallel without coordination
       return SINGLE_AGENT
   ```
   
   **If Agent Mail unavailable → fall back to sequential.**

4. **Evaluate TIER 1** (weighted score, only if no Track Assignments or auto-detect):
   
   | Factor | Weight |
   |--------|--------|
   | Epics > 1 | +2 |
   | [PARALLEL] markers in plan | +3 |
   | Domains > 2 | +2 |
   | Independent tasks > 5 | +1 |
   
   **Threshold:** Score >= 5 to proceed to TIER 2

5. **Evaluate TIER 2** (if TIER 1 passes):
   
   ```python
   (files > 15 AND tasks > 3) OR
   (est_tool_calls > 40) OR
   (est_time > 30 min AND independent_ratio > 0.6)
   ```

6. **Route Decision:**
   
   | Condition | Result |
   |-----------|--------|
   | Track Assignments exists | PARALLEL_DISPATCH |
   | Auto-detect: ≥2 independent beads | PARALLEL_DISPATCH |
   | Agent Mail unavailable | SINGLE_AGENT |
   | TIER 1 FAIL | SINGLE_AGENT |
   | TIER 1 PASS, TIER 2 FAIL | SINGLE_AGENT |
   | TIER 1 PASS, TIER 2 PASS | PARALLEL_DISPATCH |

7. **Display Feedback:**
   ```text
   ┌─ EXECUTION ROUTING ────────────────────┐
   │ Track Assignments: YES                 │
   │ Agent Mail: Available                  │
   │ Result: PARALLEL_DISPATCH              │
   │ → Routing to /conductor-orchestrate    │
   └────────────────────────────────────────┘
   ```
   
   Or for sequential:
   ```text
   ┌─ EXECUTION ROUTING ────────────────────┐
   │ Track Assignments: NO                  │
   │ TIER 1 Score: 3/8                      │
   │ Result: SINGLE_AGENT                   │
   │ → Continuing sequential execution      │
   └────────────────────────────────────────┘
   ```

8. **Update State:**
   
   Add to `implement_state.json`:
   ```json
   {
     "execution_mode": "PARALLEL_DISPATCH",
     "routing_trigger": "track_assignments",
     "routing_evaluation": {
       "has_track_assignments": true,
       "auto_detect_triggered": false,
       "independent_beads_count": null,
       "agent_mail_available": true,
       "tier1_score": null,
       "tier1_pass": null,
       "tier2_pass": null
     }
   }
   ```

9. **Branch Logic:**
   - **SINGLE_AGENT:** Continue to Phase 3 (sequential execution)
   - **PARALLEL_DISPATCH:** Hand off to [orchestrator skill](../../../orchestrator/SKILL.md)
     - Load orchestrator workflow
     - Orchestrator spawns workers via Task()
     - **Wave re-dispatch:** After each wave completes, query `bd ready --json` and spawn new workers for newly-unblocked beads
     - Main agent monitors via Agent Mail

See [orchestrator workflow](../../../orchestrator/references/workflow.md) for parallel execution protocol (including wave re-dispatch).

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
   
   See [beads-session.md](../beads-session.md) for full protocol.

4. **Execute Tasks**
   - Iterate through `plan.md` tasks sequentially
   - For each task, defer to `workflow.md` Task Workflow section
   - Follow TDD cycle from [tdd/cycle.md](../tdd/cycle.md) (default, use `--no-tdd` to disable):
     1. Mark task `[~]` in progress
     2. Write failing tests (Red)
     3. Implement to pass (Green)
     4. Refactor
     5. **Run validation gate: validate-plan-execution** (see [tdd/cycle.md](../tdd/cycle.md#validation-gate-validate-plan-execution))
     6. Verify coverage (>80%)
     7. Commit with conventional message
     8. Attach git note summary
     9. Update `plan.md`: `[~]` → `[x]` + SHA
     10. Commit plan update

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

9. **Create Epic-End Handoff**
   
   After each epic closes, create handoff with trigger `epic-end`:
   
   ```
   handoff_dir = conductor/handoffs/<track_id>/
   
   1. Create handoff file: YYYY-MM-DD_HH-MM-SS-mmm_<track>_<epic-id>_epic-end.md
   2. Include:
      - Work completed in epic
      - Files changed (git diff)
      - Learnings discovered
      - Close reason (completed/skipped/blocked)
      - Next epic to tackle
   3. Append to index.md
   4. Touch conductor/.last_activity
   ```
   
   See [../handoff/triggers.md](../handoff/triggers.md) for trigger details.

10. **Finalize Track**
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

See [beads-session.md](../beads-session.md) for full sync protocol.

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
├── handoffs/
│   └── <track_id>/
│       ├── index.md (handoff log)
│       └── *.md (epic-start/end handoffs)
├── archive/ (if archiving)
│   └── <track_id>/
└── tracks/
    └── <track_id>/
        ├── plan.md (tasks marked complete)
        ├── metadata.json (planTasks mapping + validation state)
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

- [Beads Session Workflow](../beads-session.md) - Claim, close, sync protocol
- [Beads Preflight](../preflight-beads.md) - Session initialization
- [Beads Facade](../beads-facade.md) - API contract
- [Beads Integration](../beads-integration.md) - All 13 integration points
- [Handoff Triggers](../handoff/triggers.md) - Epic-start/end trigger details
- [Create Handoff](../handoff/create.md) - Handoff creation workflow
