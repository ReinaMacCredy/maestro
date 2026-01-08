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
   - Checks Agent Mail availability (HALT if unavailable)
   - Registers agent with Agent Mail
   - Creates session state file
   - Recovers pending operations from crashed sessions

2. **Check Track Beads**
   - Verify `metadata.json` exists with `beads` section for track
   - If missing: Prompt to run `/conductor-newtrack` or `/conductor-migrate-beads`

3. **Output:**
   ```
   Preflight: bd v0.5.2 ✓, Agent Mail ✓
   Session: Created state file for T-abc123
   Track beads: 12 issues, 3 ready
   ```

### Phase 0.5: Handoff Load

**Purpose:** Load prior session context via unified handoff system.

Reference: [handoff skill](../../../handoff/SKILL.md) for full workflow.

0. **Check for Existing Handoffs**
   
   ```bash
   handoff_dir="conductor/handoffs/${track_id}/"
   
   # Skip handoff load if directory missing or empty
   if [[ ! -d "$handoff_dir" ]] || [[ -z "$(ls -A "$handoff_dir" 2>/dev/null)" ]]; then
       echo "ℹ️ No prior handoff - fresh session"
       # Skip directly to Phase 1
       return
   fi
   ```
   
   **Fresh Session Conditions:**
   - `conductor/handoffs/<track>/` directory does not exist
   - Directory exists but contains no files
   
   When either condition is true, skip handoff load entirely and proceed to Phase 1.

1. **Load Most Recent Handoff**
   - Run `/conductor-handoff resume` workflow internally
   - Try Agent Mail first (`summarize_thread`), fall back to files
   - If found: Display context summary
   - If not found: Fresh session (no prior context)

2. **Load Beads Context**
   
   ```bash
   epic_id=$(jq -r '.beads.epicId' "conductor/tracks/${track_id}/metadata.json")
   
   # Get progress
   completed=$(bd list --parent=$epic_id --status=closed --json | jq 'length')
   total=$(bd list --parent=$epic_id --json | jq 'length')
   progress=$((completed * 100 / total))
   
   # Get ready tasks
   ready=$(bd ready --json | jq -r '.[] | select(.parent == "'$epic_id'") | .title')
   ```

3. **Display Progress**
   
   ```
   ┌─ HANDOFF RESUME ─────────────────────────┐
   │ Track: auth-system_20251229              │
   │ Progress: 45% (5/12 tasks)               │
   │ Ready: E2-login-endpoint                 │
   │ Last handoff: 2h ago (epic-end)          │
   │ Loaded: 3 decisions, 5 files             │
   └──────────────────────────────────────────┘
   ```

4. **Create Epic-Start Handoff**
   
   Before starting each epic, run `/conductor-handoff create` with trigger `epic-start`:
   - Includes Beads sync (Step 5 in CREATE workflow)
   - Updates metadata.json.handoff (Step 7 in CREATE workflow)

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

**Purpose:** Determine whether to execute tasks sequentially or in parallel via orchestrator.

> **Note:** `/conductor-implement` auto-routes to `/conductor-orchestrate` when parallel execution is appropriate.

> 💡 **For auto-routing details:** Load `orchestrator` skill, then see `references/auto-routing.md`

#### Detection Priority

| Priority | Check | Trigger |
|----------|-------|---------|
| 1 | `## Track Assignments` in plan.md | Explicit parallel |
| 1.5 | `beads.fileScopes` in metadata.json | File-scope grouping |
| 2 | `beads.planTasks` with ≥2 independent | Bead dependency analysis |

#### Route Decision

| Condition | Result |
|-----------|--------|
| Track Assignments exists | PARALLEL_DISPATCH |
| ≥2 non-overlapping file scope groups | PARALLEL_DISPATCH |
| ≥2 independent beads (no deps) | PARALLEL_DISPATCH |
| Agent Mail unavailable | HALT |
| Otherwise | Sequential execution |

#### Confirmation Prompt

Before parallel dispatch:
```text
📊 Parallel execution detected:
- Track A: 2 tasks (src/api/)
- Track B: 1 task (lib/)

Run parallel? [Y/n]:
```

#### Branch Logic

- **Sequential:** Continue to Phase 3
- **PARALLEL_DISPATCH:** Hand off to [orchestrator skill](../../../orchestrator/SKILL.md)

> 💡 **For orchestrator workflow:** Load `orchestrator` skill, then see `references/workflow.md`

### Phase 3: Track Implementation

1. **Update Status**
   - Change track status `[ ]` → `[~]` in `tracks.md`

2. **Load Context**
   - Read:
     - `conductor/tracks/<track_id>/plan.md`
     - `conductor/tracks/<track_id>/spec.md`
     - `conductor/workflow.md`
     - `metadata.json.beads` for planTasks mapping
   - **Load Styleguides (Smart)**:
     - Always load: `conductor/code_styleguides/general.md`
     - Detect languages from `metadata.json.beads.fileScopes`:
       ```python
       # Extract extensions from all fileScopes
       extensions = set()
       for task_id, paths in metadata["beads"]["fileScopes"].items():
           for path in paths:
               ext = get_extension(path)  # e.g., ".py", ".ts"
               if ext:
                   extensions.add(ext)
       
       # Map extensions to styleguides
       STYLEGUIDE_MAP = {
           ".py": "python.md",
           ".ts": "typescript.md",
           ".tsx": "typescript.md",
           ".js": "javascript.md",
           ".jsx": "javascript.md",
           ".go": "go.md",
           ".html": "html-css.md",
           ".css": "html-css.md",
           ".scss": "html-css.md",
       }
       
       # Load only relevant styleguides
       styleguides = ["general.md"]
       for ext in extensions:
           if ext in STYLEGUIDE_MAP:
               styleguides.append(STYLEGUIDE_MAP[ext])
       
       # Deduplicate and load
       for guide in set(styleguides):
           Read(f"conductor/code_styleguides/{guide}")
       ```
     - Fallback: If no fileScopes, load based on `tech-stack.md` languages

3. **Claim Task (Beads Integration)**
   
   Workers claim tasks via `bd` CLI with Agent Mail coordination:
   ```bash
   bd ready --json                           # Get available tasks
   bd update <task-id> --status in_progress  # Claim task
   file_reservation_paths(paths=["<file>"])  # Reserve files before edit
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
   
   > 💡 **For degradation details:** Load `tracking` skill, then see `references/workflow.md#degradation-signals`

7. **Close Task (Beads Integration)**
   
   After task completion:
   
   ```bash
   bd update <task-id> --notes "COMPLETED: <summary>. KEY DECISION: <if any>"
   bd close <task-id> --reason completed
   release_file_reservations()  # Release reserved files
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
   
   See [handoff skill](../../../handoff/SKILL.md) for trigger details.

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
- [Handoff Skill](../../../handoff/SKILL.md) - CREATE/RESUME modes, Beads sync, progress tracking
