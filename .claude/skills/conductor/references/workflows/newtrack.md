# New Track Workflow

## Purpose
Create a new track (feature, bug fix, or chore) with comprehensive specification, implementation plan, and optionally file beads issues for execution.

## Prerequisites
- Conductor environment initialized (setup complete)
- Required files exist:
  - `conductor/tech-stack.md`
  - `conductor/workflow.md`
  - `conductor/product.md`

## Flags & Arguments

| Flag | Alias | Description |
|------|-------|-------------|
| `--no-beads` | `-nb` | Generate spec+plan only, skip beads filing |
| `--plan-only` | `-po` | Alias for `--no-beads` |
| `--force` | | Overwrite existing track or remove stale locks |

**Examples:**
- `auth_20251223` → track_input="auth_20251223", skip_beads=false, force=false
- `--no-beads auth_20251223` → skip_beads=true
- `auth_20251223 --force` → force=true
- `-nb -po auth_20251223` → Error: "Cannot use both --no-beads and --plan-only (they're aliases)."

## State Management

This workflow uses consolidated state in `metadata.json`:

| Section | Purpose |
|---------|---------|
| `metadata.json.generation` | Spec/plan generation state |
| `metadata.json.beads` | Beads filing state (resume capability) |
| `.fb-progress.lock` | Concurrent session lock (30min timeout) |

## Workflow Steps

### Phase 1: Setup Verification

1. **Parse Flags**
   - Extract `--no-beads`, `--plan-only`, `--force` from arguments
   - Error if both `-nb` and `-po` used together

2. **Check Required Files**
   - Verify all prerequisite files exist
   - If missing: Halt with message to run `/conductor:setup`

### Phase 2: Track Initialization

1. **Get Track Description**
   - If arguments contain track_id format: check for existing `design.md`
   - If `design.md` exists: derive spec from design (skip interactive questions)
   - If no arguments: prompt user for description

2. **Infer Track Type**
   - Analyze description automatically
   - Types: `feature`, `bug`, `chore`, `refactor`
   - Do NOT ask user to classify

3. **Check for Existing Track**
   - List existing tracks in `conductor/tracks/`
   - If proposed name matches existing:
     - With `--force`: overwrite existing track
     - Without `--force`: auto-increment with `-v2`, `-v3` suffix

### Phase 3: Specification Generation

1. **Announce Goal**
   > "I'll guide you through questions to build a comprehensive specification."

2. **Question Phase** (skip if design.md exists)
   - **For Features**: 3-5 questions
   - **For Bugs/Chores**: 2-3 questions

3. **Question Guidelines**
   - Ask sequentially (one at a time)
   - Provide 2-3 suggested options
   - Always include "Type your own answer" option
   - Reference `product.md`, `tech-stack.md` for context

4. **Draft `spec.md`**
   - Sections: Overview, Functional Requirements, Non-Functional Requirements, Acceptance Criteria, Out of Scope

5. **User Confirmation**
   - Present draft for review
   - Loop until approved

6. **Validation Gate: validate-spec**
   
   After spec.md is confirmed, run spec validation:
   
   - **Load gate**: `../validation/shared/validate-spec.md`
   - **Run validation**: Check spec vs design.md for requirement capture
   - **Update metadata.json**: Add `spec` to `validation.gates_passed` or log failure
   - **Behavior**: WARN on failure (both SPEED and FULL modes), continue to plan generation

   ```text
   ┌─ VALIDATION GATE: spec ─────────────────────────┐
   │ Status: [PASS] | [WARN]                         │
   │                                                 │
   │ Checks:                                         │
   │ ✓ All design decisions captured                 │
   │ ✓ No scope creep (items not in design)          │
   │ ✓ Requirements are unambiguous                  │
   │                                                 │
   │ metadata.json: gates_passed: [..., spec]        │
   └─────────────────────────────────────────────────┘
   ```

### Phase 4: Plan Generation

1. **Announce Goal**
   > "Now I will create an implementation plan based on the specification."

2. **Generate Plan**
   - Read confirmed `spec.md`
   - Read `conductor/workflow.md` for methodology
   - Structure: Phases → Tasks → Sub-tasks
   - Include status markers `[ ]`

3. **TDD Task Structure** (if workflow requires)
   ```markdown
   - [ ] Task: [Feature Name]
     - [ ] Write failing tests
     - [ ] Implement to pass tests
     - [ ] Refactor
   ```

4. **Phase Completion Tasks**
   - Check workflow for Phase Completion Protocol
   - If exists, append verification task to each phase

5. **User Confirmation**
   - Present draft for review
   - Loop until approved

6. **Validation Gate: validate-plan-structure**
   
   After plan.md is confirmed, run plan structure validation:
   
   - **Load gate**: `../validation/shared/validate-plan-structure.md`
   - **Run validation**: Check tasks have acceptance criteria, atomic tasks, verification section
   - **Update metadata.json**: Add `plan-structure` to `validation.gates_passed` or log failure
   - **Behavior**: WARN on failure (both SPEED and FULL modes), continue to artifact creation

   ```text
   ┌─ VALIDATION GATE: plan-structure ───────────────┐
   │ Status: [PASS] | [WARN]                         │
   │                                                 │
   │ Checks:                                         │
   │ ✓ All tasks have acceptance criteria            │
   │ ✓ Tasks are atomic (1-2 hours)                  │
   │ ✓ "Automated Verification" section exists       │
   │                                                 │
   │ metadata.json: gates_passed: [..., plan-structure] │
   └─────────────────────────────────────────────────┘
   ```

### Phase 4.5: File Scope Analysis

After plan is confirmed, analyze tasks for parallel execution potential.

1. **Extract File Scopes**
   
   Call [file-scope-extractor](../file-scope-extractor.md) on plan tasks:
   
   ```python
   for task in plan_tasks:
       file_scopes[task.id] = extract_files(task)
   ```
   
   - Parse explicit file declarations (`File: ...`)
   - Extract backtick-wrapped paths
   - Infer from task titles where possible

2. **Group by File Overlap**
   
   Call [parallel-grouping](../parallel-grouping.md) algorithm:
   
   ```python
   tracks = group_by_file_scope(tasks_with_scopes)
   ```
   
   - Tasks touching same files → same track (sequential)
   - Tasks touching different files → separate tracks (parallel)

3. **Check Parallel Threshold**
   
   | Groups Found | Action |
   |--------------|--------|
   | 0-1 | Skip (sequential execution) |
   | ≥2 | Generate Track Assignments |

4. **Generate Track Assignments** (if ≥2 groups)
   
   Append section to plan.md:
   
   ```markdown
   ## Track Assignments
   
   | Track | Beads | Depends On |
   |-------|-------|------------|
   | A | 1.1, 1.2 | - |
   | B | 2.1, 2.2 | - |
   | C | 3.1 | 1.2 |
   ```

5. **Update metadata.json**
   
   ```json
   {
     "beads": {
       "fileScopes": {
         "1.1": ["src/api/auth.ts"],
         "2.1": ["lib/utils/**"]
       }
     },
     "orchestrated": true
   }
   ```
   
   - Add `fileScopes` to beads section
   - Set `orchestrated: true` to signal parallel mode

6. **Display Confirmation**
   
   ```
   📊 Parallel execution detected:
   - Track A: 2 tasks (src/api/)
   - Track B: 2 tasks (lib/)
   - Track C: 1 task (schemas/)
   
   Track Assignments added to plan.md.
   ```

### Phase 5: Create Artifacts

1. **Generate Track ID**
   - Format: `shortname_YYYYMMDD`
   - If collision: auto-increment with `-v2`, `-v3` suffix

2. **Capture Thread ID**
   - Extract from Amp Thread URL environment variable

3. **Ask for Priority/Dependencies/Estimate** (optional)
   - Priority: critical, high, medium (default), low
   - Dependencies: select from incomplete tracks
   - Estimate: hours or skip

4. **Create Directory**
   ```
   conductor/tracks/<track_id>/
   ```

5. **Create `metadata.json`**
   ```json
   {
     "track_id": "<track_id>",
     "type": "feature",
     "status": "new",
     "priority": "medium",
     "depends_on": [],
     "estimated_hours": null,
     "created_at": "YYYY-MM-DDTHH:MM:SSZ",
     "updated_at": "YYYY-MM-DDTHH:MM:SSZ",
     "description": "<description>",
     "has_design": true,
     "threads": [
       {
         "id": "<thread-id>",
         "action": "newtrack",
         "timestamp": "YYYY-MM-DDTHH:MM:SSZ"
       }
     ],
     "artifacts": {
       "design": false,
       "spec": true,
       "plan": true,
       "beads": false
     }
   }
   ```

6. **Create `.track-progress.json`**
   *(Deprecated - now stored in metadata.json.generation)*

7. **Write Files**
   - `spec.md`: confirmed specification
   - `plan.md`: confirmed plan

8. **Update Tracks File**
   - Append to `conductor/tracks.md`

### Phase 6: Beads Filing (Optional)

**Skip if `--no-beads` or `--plan-only` flag set.**

This phase uses the [track-init-beads.md](../track-init-beads.md) workflow.

#### Step 1: Check Lock File

```bash
FB_LOCK="conductor/tracks/${TRACK_ID}/.fb-progress.lock"

if [[ -f "$FB_LOCK" ]]; then
  FILE_TIME=$(stat -f %m "$FB_LOCK" 2>/dev/null || stat -c %Y "$FB_LOCK")
  LOCK_AGE=$(( $(date +%s) - FILE_TIME ))
  
  if [[ $LOCK_AGE -lt 1800 ]]; then  # < 30 min
    if [[ "$FORCE" != "true" ]]; then
      echo "ERROR: Lock file exists (${LOCK_AGE}s old). Use --force to override."
      exit 1
    fi
  fi
  rm "$FB_LOCK"
fi

# Create new lock
echo "{\"agentId\": \"$THREAD_ID\", \"lockedAt\": \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\"}" > "$FB_LOCK"
```

#### Step 2: Validate Plan Structure

Run plan validation from [track-init-beads.md](../track-init-beads.md):

1. **Parse tasks** from plan.md
2. **Validate** structure (unique IDs, valid deps)
3. **On validation failure:**
   - If `--strict`: HALT immediately
   - Else: Show R/S/M prompt

**R/S/M Prompt (Structure Issues):**
```
⚠️ Plan structure issues detected:
- <issue 1>
- <issue 2>

Options:
  [R]eformat - Auto-fix and continue
  [S]kip - Skip beads filing (plan-only)
  [M]anual - Abort for manual fix

Choice [R/S/M]:
```

#### Step 3: Check Existing Beads

Detect if beads already exist for this track:

```bash
FB_PROGRESS="conductor/tracks/${TRACK_ID}/.fb-progress.json"

if [[ -f "$FB_PROGRESS" ]]; then
  EXISTING_COUNT=$(jq '.issues // [] | length' "$FB_PROGRESS")
  
  if [[ "$EXISTING_COUNT" -gt 0 ]]; then
    # Show R/S/M prompt for existing beads
  fi
fi
```

**R/S/M Prompt (Existing Beads):**
```
Existing beads found for track:
- 1 epic, 12 issues
- Last updated: 2025-12-24

Options:
  [R]eplace - Delete existing, create fresh
  [S]kip - Keep existing, skip filing
  [M]erge - Link new tasks to existing beads

Choice [R/S/M]:
```

| Choice | Action |
|--------|--------|
| **R**eplace | Close existing beads, create new |
| **S**kip | Keep existing, update mapping only |
| **M**erge | Match tasks by title, create only new ones |

#### Step 4: Create Epic

```bash
# Extract title from plan header
EPIC_TITLE="Epic: $(head -1 plan.md | sed 's/^# //')"

# Create epic bead
bd create "$EPIC_TITLE" -t epic -p 0
```

#### Step 5: Create Issues

For each task in plan.md:

```bash
bd create "<task-title>" -t task -p <priority>
bd dep add <issue-id> <epic-id>
```

**Priority Mapping:**

| Plan Phase | Priority |
|------------|----------|
| Phase 1 (Foundation) | P0 (0) |
| Phase 2 (Core) | P0 (0) |
| Phase 3 (Extensions) | P1 (1) |
| Phase 4 (Polish) | P2 (2) |

#### Step 6: Wire Dependencies

Link issues based on `depends:` in plan tasks:

```bash
# For each task with dependencies
bd dep add <issue-id> <dependency-issue-id>
```

#### Step 7: Update metadata.json.beads

Update the beads section in metadata.json with planTasks mapping:

```json
{
  "beads": {
    "status": "complete",
    "startedAt": "...",
    "epicId": "<epic-id>",
    "epics": [{"id": "<epic-id>", "title": "...", "status": "created", "createdAt": "..."}],
    "issues": ["<issue-1>", "<issue-2>", ...],
    "planTasks": {
      "1.1.1": "<issue-id-1>",
      "1.1.2": "<issue-id-2>"
    },
    "beadToTask": {
      "<issue-id-1>": "1.1.1",
      "<issue-id-2>": "1.1.2"
    },
    "crossTrackDeps": [],
    "reviewStatus": null,
    "reviewedAt": null
  }
}
```

#### Step 8: Update metadata.json

```bash
jq '.artifacts.beads = true' metadata.json > tmp.$$ && mv tmp.$$ metadata.json
```

#### Step 9: Release Lock

```bash
rm "$FB_LOCK"
```

#### Alternative: Subagent Dispatch

For complex tracks, dispatch subagents:

```
Task(
  description: "File beads from plan.md",
  prompt: "Load beads skill. Run fb on conductor/tracks/<track>/plan.md. 
           Return epic ID and issue count."
)
```

Then optionally:

```
Task(
  description: "Review filed beads",
  prompt: "Load beads skill. Run rb to review beads for track <track>."
)
```

### Phase 7: Completion & Handoff

1. **Update Final State**
   - Update `metadata.json.generation.status` to `complete`

2. **Create Design-End Handoff**
   
   Automatically create handoff with trigger `design-end`:
   
   ```
   handoff_dir = conductor/handoffs/<track_id>/
   
   1. Create handoff directory if not exists
   2. Create index.md if not exists
   3. Create handoff file: YYYY-MM-DD_HH-MM-SS-mmm_<track>_design-end.md
   4. Include:
      - Key design decisions from design.md
      - Spec summary and approach rationale
      - Constraints identified
      - Next steps: "Start implementation with `bd ready`"
   5. Append to index.md
   6. Touch conductor/.last_activity
   ```
   
   See [../handoff/create.md](../handoff/create.md) for full workflow.

3. **Display Handoff**

   **If beads were filed:**
   ```
   ━━━ TRACK COMPLETE ━━━
   Track: <track_id>
   Spec: conductor/tracks/<track_id>/spec.md
   Plan: conductor/tracks/<track_id>/plan.md
   Beads: X epics, Y issues filed

   Ready issues: Z
   First task: <first-ready-issue-id> - <title>

   Next: `rb` to review beads, or `/conductor-implement <track_id>` to start
   ```

   **If beads were skipped:**
   ```
   ━━━ TRACK CREATED ━━━
   Track: <track_id>
   Spec: conductor/tracks/<track_id>/spec.md
   Plan: conductor/tracks/<track_id>/plan.md

   Next: Say `fb` to file beads, or `/conductor-implement <track_id>` to start manually.
   ```

## Error Handling

| Scenario | Behavior |
|----------|----------|
| Setup not complete | Halt, direct to `/conductor:setup` |
| Track exists + no --force | Auto-increment with `-v2` suffix |
| Track exists + --force | Overwrite existing track |
| Flag conflict (-nb + -po) | Error: "They're aliases" |
| Lock file exists (< 30min) | Error unless --force |
| Lock file exists (>= 30min) | Auto-remove stale lock |
| Empty plan | Ask: "Plan has no tasks. Continue anyway? [y/N]" |
| Plan validation fails + --strict | HALT with exit 1 |
| Plan validation fails | Show R/S/M prompt |
| Existing beads found | Show R/S/M prompt |
| bd create fails | Retry 3x, then HALT |
| Dependency wiring fails | Log warning, continue |
| fb subagent fails | Log warning, skip rb, continue to handoff |
| rb subagent fails | Log warning, continue to handoff |
| Thread ID unavailable | Skip thread tracking, continue |
| Tool call fails | Halt, announce failure, await instructions |

## Output Artifacts

```
conductor/
├── tracks.md (updated)
└── tracks/
    └── <track_id>/
        ├── metadata.json (includes generation + beads sections)
        ├── spec.md
        ├── plan.md
        └── .fb-progress.lock (temporary, during filing)
```

## References

- [Track Init Beads Workflow](../track-init-beads.md) - Detailed beads filing process
- [Beads Facade](../beads-facade.md) - API contract
- [Beads Integration](../beads-integration.md) - All 13 integration points
