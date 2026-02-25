---
name: maestro:implement
description: "Execute track tasks following TDD workflow. Single-agent by default, --team for parallel Agent Teams, Sub Agent Parallels. Use when ready to implement a planned track."
argument-hint: "[<track-name>] [--team]"
---

# Implement -- Task Execution Engine

> This skill is CLI-agnostic. It works with Claude Code, Codex, Amp, or any AI coding assistant.

Execute tasks from a track's implementation plan, following the configured workflow methodology (TDD or ship-fast). Supports single-agent mode (default) and team mode (`--team`).

Validate the result of every operation. If any step fails, halt and report the failure before continuing.

## Arguments

`$ARGUMENTS`

- `<track-name>`: Match track by name or ID substring. Optional -- auto-selects if only one track is pending.
- `--team`: Enable team mode with parallel workers (kraken/spark).
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
   - Multiple --> ask the user: "Which track do you want to implement?"
     Options:
     - **{track_1_description}** -- ID: {track_1_id} | {task_count} tasks
     - **{track_2_description}** -- ID: {track_2_id} | {task_count} tasks

4. **Confirm selection**:

   Ask the user: "Implement track '{description}'? ({task_count} tasks, {phase_count} phases)"
   Options:
   - **Yes, start** -- Begin implementation
   - **Cancel** -- Go back

## Step 3: Load Context

1. Read track plan: `.maestro/tracks/{track_id}/plan.md`
2. Read track spec: `.maestro/tracks/{track_id}/spec.md`
3. Read workflow config: `.maestro/context/workflow.md`
4. Read tech stack: `.maestro/context/tech-stack.md`
5. Read guidelines: `.maestro/context/guidelines.md` (if exists)
6. Read code style guides: `.maestro/context/code_styleguides/` (if exists)
7. Load skill guidance from track metadata: `.maestro/tracks/{track_id}/metadata.json`
   - Parse the `"skills"` array
   - For each skill entry, use the Skill tool or Read tool to load the skill's SKILL.md by name
   - Extract the content after YAML frontmatter (the actual guidance)
   - Hold in memory as the track's skill guidance registry
   - **Graceful degradation**: If `"skills"` is missing, empty, or a skill is no longer installed, proceed without skill injection. Do not warn or error.

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

**6a.1.5: Load Skill Guidance for Task**

If the track has skills loaded (from Step 3.7):

1. Check if any loaded skill is relevant to the current task by comparing the task title and sub-task descriptions against each skill's description
2. If relevant skills are found, prepend the following to the task's working context:

```
## SKILL GUIDANCE

### {skill-name}
{Full SKILL.md content after frontmatter}

### {another-skill}
{Content}
```

3. If no skills are relevant to this specific task, omit the section entirely
4. This guidance should inform the Red-Green-Refactor cycle -- for example, a Swift testing skill would guide how tests are structured in the Red phase

**Graceful degradation**: If no skills were loaded for this track, skip this step entirely.

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

Ask the user: "Phase {N} complete. All tests pass. Have you verified the manual steps above?"
Options:
- **Verified, continue** -- Proceed to next phase
- **Issue found** -- I found a problem, let's fix it

If issue found: create a fix task, execute it, then re-verify.

---

## Team Mode (--team)

See `reference/team-mode.md` for full protocol.

### Step 6b: Create Team

Create a worker team named "implement-{track_id}" (description: "Implementing {track_description}"). Use whatever team/delegation API your runtime provides.

### Step 7b: Create Tasks

For each task in plan.md, create a task entry:
- **Subject**: "{task_name}"
- **Description**: Include phase number, task description, relevant spec section, workflow methodology, and track ID.
- **Active form**: "Implementing {task_name}"

Set up dependencies between tasks using blocked-by relationships.

### Step 8b: Spawn Workers

Spawn 2-3 workers based on track size:

Spawn a TDD worker (kraken) with the following prompt:

```
You are a TDD implementation worker. Check the task list for available tasks.
Claim one, implement following TDD (Red-Green-Refactor), then check for next
task. Follow the project workflow in .maestro/context/workflow.md.
Track skills: Read the "skills" array from .maestro/tracks/{track_id}/metadata.json.
For each listed skill, load its SKILL.md and apply relevant guidance to your tasks.
```

For quick fix tasks, use a quick-fix worker (spark) instead of kraken.

### Step 9b: Monitor and Verify

As an orchestrator in team mode:
1. Monitor worker progress via the task list
2. After each task completion:
   - Read the changed files
   - Run tests to verify
   - If verification fails: reassign or fix
   - If passes: update plan.md checkbox
3. Commit verified work

### Step 10b: Phase Completion

Same Phase Completion Protocol as single-agent mode.

After all tasks complete:
1. Request shutdown for each worker.
2. Tear down the worker team.

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

### 8.1.5: Record Skill Effectiveness

If the completed track had skills in `metadata.json` (non-empty `"skills"` array):

1. Extract the top 5 keywords from the track's description and task titles
2. Read `.maestro/context/skill-mappings.md` (if it exists)
3. If the file doesn't exist, create it:

```markdown
# Skill Mappings

> Auto-generated by maestro:implement on track completion.
> Used by maestro:new-track to pre-match skills for similar tracks.

## Mappings

| Track Type | Keywords | Skills | Last Used |
|------------|----------|--------|-----------|
```

4. Check if a row already exists with the same track type AND 2+ overlapping keywords:
   - If yes: merge any new skill names into the Skills column, update the Last Used date
   - If no: append a new row

5. Include the file in the Step 8.4 commit:
   ```bash
   git add .maestro/context/skill-mappings.md
   ```

No user confirmation required -- this is automatic on successful track completion.

### 8.2: Documentation Sync

Read the track spec and check if project docs need updating:

1. **Product definition** (`product.md`): Does this track add a new capability?
2. **Tech stack** (`tech-stack.md`): Were new technologies introduced?
3. **Guidelines** (`guidelines.md`): Did this track establish new patterns?

**WARNING: Product guidelines should only be updated for strategic shifts. Most tracks should NOT trigger guidelines changes.**

For each proposed change, show an embedded diff (before/after) in the question:

Ask the user: "This track adds {capability}. Update product.md to include it?\n\nBEFORE:\n{existing_content_excerpt}\n\nAFTER:\n{proposed_content_excerpt}"
Options:
- **Yes, update** -- Add to project documentation
- **Skip** -- Don't update documentation

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

Ask the user: "Track '{description}' is complete. What would you like to do with it?"
Options:
- **Review first** -- Run /maestro:review before deciding
- **Archive** -- Move to .maestro/archive/{track_id}/
- **Keep** -- Leave in tracks/ for reference
- **Delete** -- Permanently remove track files

- **Review first**: Run `/maestro:review {track_id}` and return to this cleanup step afterward
- **Archive**: `mkdir -p .maestro/archive && mv .maestro/tracks/{track_id} .maestro/archive/`
- **Keep**: No action
- **Delete**: First confirm with a second question:

  Ask the user: "WARNING: This will permanently delete the track folder and all its contents. This action cannot be undone. Are you sure you want to delete track '{track_id}'?"
  Options:
  - **Yes, delete permanently** -- Remove all track files
  - **Cancel** -- Go back to cleanup options

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

---

## Relationship to Other Commands

Recommended workflow:

- `/maestro:setup` -- Scaffold project context (run first)
- `/maestro:new-track` -- Create a feature/bug track with spec and plan
- `/maestro:implement` -- **You are here.** Execute the implementation
- `/maestro:review` -- Verify implementation correctness
- `/maestro:status` -- Check progress across all tracks
- `/maestro:revert` -- Undo implementation if needed

Implementation consumes the `plan.md` created by `/maestro:new-track`. Each task produces atomic commits, which `/maestro:review` can analyze to verify correctness against the spec. Run `/maestro:status` to check progress mid-implementation, or `/maestro:revert` to undo if something goes wrong.
