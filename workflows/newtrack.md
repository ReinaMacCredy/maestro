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

This workflow uses multiple state files:

| File | Purpose |
|------|---------|
| `.track-progress.json` | Spec/plan generation state |
| `.fb-progress.json` | Beads filing state (resume capability) |
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
   ```json
   {
     "trackId": "<track_id>",
     "status": "plan_done",
     "specCreatedAt": "...",
     "planCreatedAt": "...",
     "threadId": "<thread-id>"
   }
   ```

7. **Write Files**
   - `spec.md`: confirmed specification
   - `plan.md`: confirmed plan

8. **Update Tracks File**
   - Append to `conductor/tracks.md`

### Phase 6: Beads Filing (Optional)

**Skip if `--no-beads` or `--plan-only` flag set.**

1. **Check Lock File**
   - If `.fb-progress.lock` exists:
     - Age < 30min: Error unless `--force`
     - Age >= 30min: Remove stale lock
   - Create new lock file

2. **Spawn Beads (fb) Subagent**
   ```
   Task(
     description: "File beads from plan.md",
     prompt: "Load beads skill, run fb on plan..."
   )
   ```
   - Updates `.track-progress.json` status to `fb_started` → `fb_done`
   - Updates `metadata.json` with `artifacts.beads: true`

3. **Spawn Beads (rb) Subagent** (if fb succeeded)
   ```
   Task(
     description: "Review filed beads",
     prompt: "Load beads skill, run rb to review beads..."
   )
   ```
   - Updates `.track-progress.json` status to `rb_done`

4. **Release Lock**
   - Remove `.fb-progress.lock`

### Phase 7: Completion & Handoff

1. **Update Final State**
   - Set `.track-progress.json` status to `complete`

2. **Display Handoff**

   **If beads were filed:**
   ```
   ━━━ TRACK COMPLETE ━━━
   Track: <track_id>
   Spec: conductor/tracks/<track_id>/spec.md
   Plan: conductor/tracks/<track_id>/plan.md
   Beads: X epics, Y issues filed and reviewed

   Ready issues: Z
   First task: <first-ready-issue-id> - <title>

   Next: `Start epic <first-epic-id>` or `/conductor-implement <track_id>`
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
        ├── metadata.json
        ├── spec.md
        ├── plan.md
        ├── .track-progress.json
        ├── .fb-progress.json (if beads filed)
        └── .fb-progress.lock (temporary, during filing)
```
