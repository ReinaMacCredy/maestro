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

Reference: [workflows/handoff.md](handoff.md) for full workflow.

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

> **Note:** `/conductor-implement` is a wrapper that auto-routes to `/conductor-orchestrate` when parallel execution is appropriate.

1. **Check Track Assignments (Priority 1)**
   
   ```python
   plan = Read("conductor/tracks/<track-id>/plan.md")
   
   if "## Track Assignments" in plan:
       # Explicit parallel execution requested - parse table directly
       # Skip group_by_file_scope() entirely
       tracks = parse_track_assignments_table(plan)
       return PARALLEL_DISPATCH, tracks
   
   def parse_track_assignments_table(plan_content: str) -> list[Track]:
       """Parse Track Assignments table directly, bypassing file scope analysis.
       
       Table format:
       | Track | Tasks | File Scope | Depends On |
       |-------|-------|------------|------------|
       | A     | 1.1.1 | path/to/file.md | - |
       | B     | 1.2.1, 1.2.2 | other/path.py | A |
       """
       lines = plan_content.split("\n")
       in_table = False
       tracks = []
       
       for line in lines:
           if "## Track Assignments" in line:
               in_table = True
               continue
           if in_table and line.startswith("|") and not line.startswith("| Track") and not line.startswith("|---"):
               parts = [p.strip() for p in line.split("|")[1:-1]]
               if len(parts) >= 4:
                   track_id, tasks, file_scope, depends_on = parts[0], parts[1], parts[2], parts[3]
                   tracks.append(Track(
                       id=track_id,
                       tasks=tasks.split(", "),
                       files=[file_scope],
                       depends_on=depends_on if depends_on != "-" else None
                   ))
           elif in_table and line.startswith("##"):
               break  # End of Track Assignments section
           
           return tracks
           
           def validate_tasks_against_beads(tracks: list[Track], metadata_path: str) -> list[str]:
           """Validate that all task IDs in tracks exist in metadata.json.beads.planTasks.
           
           Returns list of unknown task IDs for warning display.
           """
           metadata = json.loads(Read(metadata_path))
           plan_tasks = metadata.get("beads", {}).get("planTasks", {})
           known_task_ids = set(plan_tasks.keys())
           
           unknown_tasks = []
           for track in tracks:
               for task_id in track.tasks:
                   if task_id not in known_task_ids:
                       unknown_tasks.append(task_id)
           
           return unknown_tasks
           
           # After parsing Track Assignments, validate task IDs
           tracks = parse_track_assignments_table(plan)
           unknown = validate_tasks_against_beads(
               tracks, 
               f"conductor/tracks/{track_id}/metadata.json"
           )
           if unknown:
               print(f"⚠️ WARNING: Track Assignments references unknown tasks: {unknown}")
               print("   These task IDs do not exist in metadata.json.beads.planTasks")
           ```
           
           **If Track Assignments section exists → parse table directly and skip all file scope analysis.**

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
       # Cannot do parallel without coordination - HALT
       print("HALT: Agent Mail required for coordination")
       exit(1)
   ```
   
   **If Agent Mail unavailable → HALT (required for coordination).**

4. **Route Decision:**
   
   | Condition | Result |
   |-----------|--------|
   | Track Assignments exists | PARALLEL_DISPATCH |
   | Auto-detect: ≥2 independent beads | PARALLEL_DISPATCH |
   | Agent Mail unavailable | HALT |
   | Single bead or dependent beads | Sequential via orchestrator |

5. **Confirmation Prompt (before parallel dispatch):**
   
   When routing to PARALLEL_DISPATCH, display confirmation before spawning workers.
   
   **Key:** The `tracks` variable is already populated from Phase 2b Step 1 (`parse_track_assignments_table()`) 
   or Step 2 (auto-detect). Confirmation simply displays pre-parsed track info—no re-analysis needed.
   
   ```python
   if decision == PARALLEL_DISPATCH:
       # tracks already populated from earlier routing step:
       # - Priority 1: parse_track_assignments_table() output
       # - Priority 2: group_by_file_scope() output (auto-detect path)
       # No additional parsing or analysis here—just display.
       
       # Display grouped tracks (pre-parsed from plan.md)
       print("""
       📊 Parallel execution detected:
       """)
       for track in tracks:
           print(f"- Track {track.id}: {len(track.tasks)} task(s) ({track.files[0]})")
       
       # Show dependencies if any
       if has_dependent_tracks(tracks):
           print("\nDependencies:")
           for track in tracks:
               if track.depends_on:
                   print(f"- Track {track.id} depends on Track {track.depends_on}")
       
       print("\nRun parallel? [Y/n]: ")
       
       # Handle response
       response = get_user_input()
       if response.lower() in ['', 'y', 'yes']:
           # Route to orchestrator
           return route_to_orchestrator(tracks)
       else:
           # Fall back to sequential
           print("→ Continuing with sequential execution")
           return SINGLE_AGENT
   ```
   
   **Prompt Format:**
   ```text
   📊 Parallel execution detected:
   - Track 1: 2 tasks (src/api/auth.ts, src/api/login.ts)
   - Track 2: 1 task (src/db/models/)
   - Track 3: 1 task (lib/validation.ts)
   
   Dependencies:
   - Track 3 depends on Track 1
   
   Run parallel? [Y/n]:
   ```
   
   **Response Handling:**
   | Input | Action |
   |-------|--------|
   | `Y`, `y`, `yes`, `[Enter]` | Route to `/conductor-orchestrate` |
   | `N`, `n`, `no` | Continue sequential execution |
   | Other | Re-prompt once, then default to `N` |
   
   See [parallel-grouping.md](../parallel-grouping.md) for grouping algorithm.

8. **Display Feedback:**
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

9. **Update State:**
   
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
   - **Sequential:** Continue to Phase 3 (single bead execution)
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
   
   See [Degradation Signals](../../../beads/references/workflow.md#degradation-signals) for full details.

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
- [Unified Handoff Workflow](handoff.md) - CREATE/RESUME modes, Beads sync, progress tracking
