---
name: maestro:implement
description: "Execute track tasks following TDD workflow. Single-agent by default, --team for parallel Agent Teams. Use when ready to implement a planned track."
argument-hint: "[<track-name>] [--team]"
---

# Implement -- Task Execution Engine

Execute tasks from a track's implementation plan, following the configured workflow methodology (TDD or ship-fast). Supports single-agent mode (default) and Agent Teams mode (`--team`).

**IMPORTANT: Every human-in-the-loop interaction mentioned in this workflow MUST be conducted using the AskUserQuestion tool. Do not ask questions via plain text output.**

**When using AskUserQuestion, immediately call the tool -- do not repeat the question in plain text before the tool call.**

**CRITICAL: You must validate the success of every tool call. If any tool call fails, halt immediately, announce the failure, and await instructions.**

## Arguments

`$ARGUMENTS`

- `<track-name>`: Match track by name or ID substring. Optional -- auto-selects if only one track is pending.
- `--team`: Enable Agent Teams mode with parallel workers (kraken/spark).
- `--resume`: Skip already-completed tasks (marked `[x]`) and continue from next `[ ]` task.

---

## Step 1: Mode Detection

Parse `$ARGUMENTS`:
- If contains `--team` --> team mode (see `reference/team-mode.md`)
- Otherwise --> single-agent mode (default)
- If contains `--resume` --> set resume flag

## Step 2: Track Selection

1. Read `.maestro/tracks.md` for available tracks.

   **Parsing instructions**: Split the tracks.md content by the `---` separator to identify each track section. Parse status markers: `[ ]` = new, `[~]` = in-progress, `[x]` = complete. Also support legacy format `## [ ] Track:` in addition to `- [ ] **Track:`.

2. **If track name given**: Match by:
   - Exact track ID match
   - Substring match on track description (case-insensitive)
   - If multiple matches, ask user to disambiguate

3. **If no track name**:
   - Filter tracks with status `[ ]` (new) or `[~]` (in-progress)
   - 0 tracks --> error: "No pending tracks. Run `/maestro:new-track` to create one."
   - 1 track --> auto-select
   - Multiple --> ask user:
     ```
     AskUserQuestion(
       questions: [{
         question: "Which track do you want to implement?",
         header: "Track",
         options: [
           { label: "{track_1_description}", description: "ID: {track_1_id} | {task_count} tasks" },
           { label: "{track_2_description}", description: "ID: {track_2_id} | {task_count} tasks" }
         ],
         multiSelect: false
       }]
     )
     ```

4. **Confirm selection**:
   ```
   AskUserQuestion(
     questions: [{
       question: "Implement track '{description}'? ({task_count} tasks, {phase_count} phases)",
       header: "Confirm",
       options: [
         { label: "Yes, start", description: "Begin implementation" },
         { label: "Cancel", description: "Go back" }
       ],
       multiSelect: false
     }]
   )
   ```

## Step 3: Load Context

1. Read track plan: `.maestro/tracks/{track_id}/plan.md`
2. Read track spec: `.maestro/tracks/{track_id}/spec.md`
3. Read workflow config: `.maestro/context/workflow.md`
4. Read tech stack: `.maestro/context/tech-stack.md`
5. Read guidelines: `.maestro/context/guidelines.md` (if exists)
6. Read code style guides: `.maestro/context/code_styleguides/` (if exists)

## Step 4: Update Track Status

Edit `.maestro/tracks.md`: Change track marker from `[ ]` to `[~]`.

Update metadata:
```json
{
  "status": "in_progress",
  "updated_at": "{ISO timestamp}"
}
```

## Step 5: Build Task Queue

Parse `plan.md` to extract ordered task list:
- Identify phases (## Phase N)
- Identify tasks within each phase (### Task N.M)
- Identify sub-tasks within each task (- [ ] ...)
- If `--resume`: skip tasks already marked `[x]`

---

## Single-Agent Mode (Default)

### Step 6a: Execute Tasks Sequentially

For each task in the queue, follow the workflow methodology.

#### TDD Methodology (from workflow.md)

**6a.1: Mark Task In Progress**

Edit `plan.md`: Change task checkbox from `[ ]` to `[~]`.

**6a.2: Red Phase -- Write Failing Tests**

1. Identify what to test based on task description and spec
2. Create test file if it doesn't exist
3. Write tests defining expected behavior
4. Run test suite:
   ```bash
   # Use CI=true for watch-mode tools
   CI=true {test_command}
   ```
5. Confirm tests FAIL (this validates they're meaningful)
6. If tests pass unexpectedly: the behavior already exists. Skip to refactor or mark complete.
7. Do NOT proceed to implementation until tests fail.

**6a.3: Green Phase -- Implement to Pass**

1. Write minimum code to make tests pass
2. Run test suite:
   ```bash
   CI=true {test_command}
   ```
3. Confirm tests PASS
4. If tests fail: debug and fix. Max 3 attempts. If still failing, ask user for help.

**6a.4: Refactor (Optional)**

1. Review the implementation for code smells
2. Improve readability and structure
3. Run tests again to confirm still passing
4. Skip if implementation is already clean

**6a.5: Verify Coverage**

If `workflow.md` specifies a coverage threshold:
```bash
CI=true {coverage_command}
```

Check that new code meets the threshold. If not, add more tests.

**6a.6: Check Tech Stack Compliance**

If the task introduced a new library or technology not in `tech-stack.md`:
1. STOP implementation
2. Inform user: "This task uses {new_tech} which isn't in the tech stack."
3. Ask: Add to tech stack or find an alternative?
4. If approved: update `.maestro/context/tech-stack.md`
5. Resume

**6a.7: Commit Code Changes**

```bash
git add {changed_files}
git commit -m "{type}({scope}): {description}"
```

Commit message format:
- `feat(scope):` for new features
- `fix(scope):` for bug fixes
- `refactor(scope):` for refactoring
- `test(scope):` for test-only changes

**6a.8: Attach Summary (if configured)**

If `workflow.md` specifies git notes:
```bash
git notes add -m "Task: {task_name}
Phase: {phase_number}
Changes: {files_changed}
Summary: {what_and_why}" {commit_hash}
```

If commit messages: include summary in the commit message body.

**6a.9: Record Task SHA**

Edit `plan.md`: Change task marker from `[~]` to `[x] {sha}` (first 7 characters of commit hash).

```bash
git add .maestro/tracks/{track_id}/plan.md
git commit -m "maestro(plan): mark task '{task_name}' complete"
```

#### Ship-fast Methodology

Same flow but reordered:
1. Mark in progress
2. Implement the feature/fix
3. Write tests covering the implementation
4. Run tests, verify passing
5. Commit, attach summary, record SHA

### Step 7a: Phase Completion Verification

When the last task in a phase completes, run the Phase Completion Protocol.
See `reference/phase-completion.md` for details.

1. **Test coverage check**: Run coverage for files changed since phase start
2. **Full test execution**: `CI=true {test_command}` -- max 2 fix attempts
3. **Manual verification**: Present step-by-step verification plan to user
4. **User confirmation**: Wait for explicit approval

```
AskUserQuestion(
  questions: [{
    question: "Phase {N} complete. All tests pass. Have you verified the manual steps above?",
    header: "Phase Done",
    options: [
      { label: "Verified, continue", description: "Proceed to next phase" },
      { label: "Issue found", description: "I found a problem, let's fix it" }
    ],
    multiSelect: false
  }]
)
```

If issue found: create a fix task, execute it, then re-verify.

---

## Team Mode (--team)

See `reference/team-mode.md` for full protocol.

### Step 6b: Create Team

```
TeamCreate(
  team_name: "implement-{track_id}",
  description: "Implementing {track_description}"
)
```

### Step 7b: Create Tasks

For each task in plan.md, create a task entry:
```
TaskCreate(
  subject: "{task_name}",
  description: "Phase {N}, Task {M}\n\n{task_description}\n\nSpec: {relevant_spec_section}\nWorkflow: {methodology}\nTrack: {track_id}",
  activeForm: "Implementing {task_name}"
)
```

Set up dependencies between tasks using `addBlockedBy`.

### Step 8b: Spawn Workers

Spawn 2-3 workers based on track size:

```
Task(
  subagent_type: "kraken",
  name: "worker-1",
  team_name: "implement-{track_id}",
  prompt: "You are a TDD implementation worker. Check TaskList for available tasks. Claim one, implement following TDD (Red-Green-Refactor), then check for next task. Follow the project workflow in .maestro/context/workflow.md."
)
```

For quick fix tasks, use `spark` instead of `kraken`.

### Step 9b: Monitor and Verify

As an orchestrator in team mode:
1. Monitor worker progress via TaskList
2. After each task completion:
   - Read the changed files
   - Run tests to verify
   - If verification fails: reassign or fix
   - If passes: update plan.md checkbox
3. Commit verified work

### Step 10b: Phase Completion

Same Phase Completion Protocol as single-agent mode.

After all tasks complete:
```
SendMessage(type: "shutdown_request", recipient: "worker-1")
SendMessage(type: "shutdown_request", recipient: "worker-2")
TeamDelete()
```

---

## Step 8: Track Completion

When ALL phases are complete:

### 8.1: Mark Track Complete

Edit `.maestro/tracks.md`: Change track marker from `[~]` to `[x]`.

Update metadata:
```json
{
  "status": "completed",
  "updated_at": "{ISO timestamp}"
}
```

### 8.2: Documentation Sync

Read the track spec and check if project docs need updating:

1. **Product definition** (`product.md`): Does this track add a new capability?
2. **Tech stack** (`tech-stack.md`): Were new technologies introduced?
3. **Guidelines** (`guidelines.md`): Did this track establish new patterns?

**WARNING: Product guidelines should only be updated for strategic shifts. Most tracks should NOT trigger guidelines changes.**

For each proposed change, show an embedded diff (before/after) in the question field:
```
AskUserQuestion(
  questions: [{
    question: "This track adds {capability}. Update product.md to include it?\n\nBEFORE:\n{existing_content_excerpt}\n\nAFTER:\n{proposed_content_excerpt}",
    header: "Doc Sync",
    options: [
      { label: "Yes, update", description: "Add to project documentation" },
      { label: "Skip", description: "Don't update documentation" }
    ],
    multiSelect: false
  }]
)
```

If approved, make the update and commit:
```bash
git add .maestro/context/{file}
git commit -m "docs(maestro): synchronize docs for track '{track_description}'"
```

After all doc update decisions are made, output a final sync report:
```
## Documentation Sync Report

**Track**: {track_description}

| File | Action | Reason |
|------|--------|--------|
| product.md | Updated | Added {capability} |
| tech-stack.md | Skipped | No new technologies |
| guidelines.md | Skipped | No strategic shift |
```

### 8.3: Track Cleanup

```
AskUserQuestion(
  questions: [{
    question: "Track '{description}' is complete. What would you like to do with it?",
    header: "Cleanup",
    options: [
      { label: "Review first", description: "Run /maestro:review before deciding" },
      { label: "Archive", description: "Move to .maestro/archive/{track_id}/" },
      { label: "Keep", description: "Leave in tracks/ for reference" },
      { label: "Delete", description: "Permanently remove track files" }
    ],
    multiSelect: false
  }]
)
```

- **Review first**: Run `/maestro:review {track_id}` and return to this cleanup step afterward
- **Archive**: `mkdir -p .maestro/archive && mv .maestro/tracks/{track_id} .maestro/archive/`
- **Keep**: No action
- **Delete**: First confirm with a second AskUserQuestion:
  ```
  AskUserQuestion(
    questions: [{
      question: "WARNING: This will permanently delete the track folder and all its contents. This action cannot be undone. Are you sure you want to delete track '{track_id}'?",
      header: "Confirm Delete",
      options: [
        { label: "Yes, delete permanently", description: "Remove all track files" },
        { label: "Cancel", description: "Go back to cleanup options" }
      ],
      multiSelect: false
    }]
  )
  ```
  If confirmed: `rm -rf .maestro/tracks/{track_id}`
  If cancelled: return to the cleanup options question.

### 8.4: Final Commit

```bash
git add .maestro/
git commit -m "chore(maestro): complete track {track_id}"
```

### 8.5: Summary

```
## Track Complete

**{track description}** -- all {task_count} tasks across {phase_count} phases finished.

**Commits**: {list of task SHAs}
**Next**: `/maestro:review {track_id}` to verify, or `/maestro:new-track` for next feature.
```
