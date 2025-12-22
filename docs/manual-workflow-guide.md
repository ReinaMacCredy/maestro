# Manual Workflow Guide

This guide explains how to work with Conductor commands manually without relying on skills or auto-activation. Use this when you need precise control over the workflow or when skills don't behave as expected.

## Why Manual Mode?

Skills are convenient but can sometimes:
- Skip steps or misinterpret context
- Not follow the exact workflow sequence
- Miss state file updates

Manual command invocation gives you **full control** over each step.

## Prerequisites

Before using any command, ensure:
1. Git is installed and initialized in your project
2. You have write access to the project directory
3. For implementation: `conductor/` directory exists with required files

## Command Workflows

### 1. `/conductor:setup` (or `/conductor-setup`)

**Purpose**: Initialize a new project with Conductor methodology.

**When to use**: First time setting up Conductor in any project.

**Manual workflow**:

```
Step 1: Run the command
   /conductor:setup

Step 2: Answer project type questions
   - Brownfield (existing code) vs Greenfield (new project)
   - If brownfield: approve codebase scan

Step 3: Complete each section (max 5 questions each)
   a) Product Guide → creates product.md
   b) Product Guidelines → creates product-guidelines.md
   c) Tech Stack → creates tech-stack.md
   d) Code Styleguides → copies to code_styleguides/
   e) Workflow → creates workflow.md

Step 4: Create initial track
   - Approve track proposal
   - Review generated spec.md and plan.md

Step 5: Verify artifacts
   conductor/
   ├── setup_state.json
   ├── product.md
   ├── product-guidelines.md
   ├── tech-stack.md
   ├── workflow.md
   ├── tracks.md
   └── tracks/<track_id>/
       ├── metadata.json
       ├── spec.md
       └── plan.md
```

**State file**: `conductor/setup_state.json`
- Resume from any step if interrupted
- Check `last_successful_step` to see progress

**Troubleshooting**:
- If setup stalls: Check `setup_state.json` for current state
- To restart: Delete `conductor/` directory and re-run

---

### 2. `/conductor:newTrack` (or `/conductor-newtrack`)

**Purpose**: Create a new feature or bug fix track.

**When to use**: Starting work on a new feature or bug.

**Manual workflow**:

```
Step 1: Run the command (optionally with description)
   /conductor:newTrack "Add user authentication"
   # or without description for interactive mode
   /conductor:newTrack

Step 2: Define track details
   - Type: feature, bug, or improvement
   - Priority: critical, high, medium, low
   - Dependencies: link to other tracks (optional)
   - Time estimate (optional)

Step 3: Answer specification questions (max 5)
   - Requirements, acceptance criteria
   - Review generated spec.md

Step 4: Answer planning questions (max 5)
   - Implementation approach
   - Review generated plan.md with phases/tasks

Step 5: Approve artifacts
   - Confirm spec and plan look correct
   - Track is added to tracks.md
```

**Generated artifacts**:
```
conductor/tracks/<shortname_YYYYMMDD>/
├── metadata.json   # Track configuration
├── spec.md         # Requirements
└── plan.md         # Implementation plan
```

**Track ID format**: `<shortname>_<YYYYMMDD>` (e.g., `auth_20241219`)

---

### 3. `/conductor:implement` (or `/conductor-implement`)

**Purpose**: Execute tasks from a track's plan.

**When to use**: After approving a track's plan.

**Manual workflow**:

```
Step 1: Run the command
   /conductor:implement
   # or specify track
   /conductor:implement auth_20241219

Step 2: Track selection (if not specified)
   - First non-completed track is auto-selected
   - Confirm selection

Step 3: Check dependencies
   - Warning shown if dependent tracks incomplete
   - Choose to proceed or wait

Step 4: Resume check
   - If implement_state.json exists, resume from last task
   - Otherwise start from first task

Step 5: For each task, follow TDD workflow:
   a) Mark task [~] in progress
   b) Write failing tests (Red)
   c) Implement to pass (Green)
   d) Refactor if needed
   e) Verify coverage (>80%)
   f) Commit with conventional message
   g) Update plan.md: [~] → [x] + SHA

Step 6: Phase completion
   - Run full test suite
   - Manual verification with user
   - Create checkpoint commit

Step 7: Track completion
   - Update tracks.md: [~] → [x]
   - Sync documentation (optional updates to product.md, tech-stack.md)
   - Archive/delete/skip option
```

**State file**: `conductor/tracks/<track_id>/implement_state.json`
- Tracks current phase and task
- Enables pause/resume across sessions

**Status markers in plan.md**:
- `[ ]` - Pending
- `[~]` - In progress
- `[x]` - Completed (with commit SHA)
- `[!]` - Blocked (with reason)

---

### 4. `/conductor:status` (or `/conductor-status`)

**Purpose**: Display project progress overview.

**When to use**: Check current state of all tracks.

**Manual workflow**:

```
Step 1: Run the command
   /conductor:status

Step 2: Review output
   - Overall progress percentage
   - Tracks grouped by priority
   - Current active track and task
   - Blocked items
   - Dependency graph
```

**No state file**: Read-only command.

---

### 5. `/conductor:validate` (or `/conductor-validate`)

**Purpose**: Check project integrity and fix issues.

**When to use**: 
- After manual edits to conductor files
- When something seems broken
- Periodic health check

**Manual workflow**:

```
Step 1: Run the command
   /conductor:validate

Step 2: Review findings
   - Missing files
   - Orphan tracks (in directory but not tracks.md)
   - Invalid metadata
   - Status inconsistencies

Step 3: Choose fix option
   A) Auto-fix all issues
   B) Fix specific issues
   C) Skip (report only)
```

---

### 6. `/conductor:block` (or `/conductor-block`)

**Purpose**: Mark a task as blocked.

**When to use**: Task cannot proceed due to external dependency.

**Manual workflow**:

```
Step 1: Run the command
   /conductor:block

Step 2: Select blocked task
   - Choose from in-progress or pending tasks

Step 3: Provide reason
   - "Waiting for API credentials"
   - "Blocked by team review"

Step 4: Verify update
   - Task marked [!] in plan.md
   - Reason recorded
```

**Blocker format in plan.md**:
```markdown
- [!] Task name [BLOCKED: Waiting for API credentials]
```

---

### 7. `/conductor:skip` (or `/conductor-skip`)

**Purpose**: Skip current task and move to next.

**When to use**: Task is not applicable or should be deferred.

**Manual workflow**:

```
Step 1: Run the command
   /conductor:skip

Step 2: Confirm task to skip

Step 3: Provide justification
   - Recorded in plan.md

Step 4: Implementation moves to next task
```

---

### 8. `/conductor:revise` (or `/conductor-revise`)

**Purpose**: Update spec/plan when implementation reveals issues.

**When to use**: 
- Requirements change mid-implementation
- Plan needs adjustment based on discoveries

**Manual workflow**:

```
Step 1: Run the command
   /conductor:revise

Step 2: Select what to revise
   A) Spec only
   B) Plan only
   C) Both

Step 3: Describe changes needed

Step 4: Review proposed updates
   - Diff view of changes

Step 5: Approve changes
   - Recorded in revisions.md
```

**Revision log**: `conductor/tracks/<track_id>/revisions.md`

---

### 9. `/conductor:revert` (or `/conductor-revert`)

**Purpose**: Git-aware revert of work.

**When to use**: Need to undo implementation work.

**Manual workflow**:

```
Step 1: Run the command
   /conductor:revert

Step 2: Select revert scope
   A) Entire track
   B) Specific phase
   C) Single task

Step 3: Review commits to revert
   - Shows commit list with messages

Step 4: Confirm revert
   - Creates revert commits
   - Updates plan.md status markers
```

---

### 10. `/conductor:archive` (or `/conductor-archive`)

**Purpose**: Move completed tracks to archive.

**When to use**: Clean up after track completion.

**Manual workflow**:

```
Step 1: Run the command
   /conductor:archive

Step 2: Select tracks to archive
   - Only completed [x] tracks shown

Step 3: Confirm

Step 4: Tracks moved to conductor/archive/
```

---

### 11. `/conductor:export` (or `/conductor-export`)

**Purpose**: Generate project summary report.

**When to use**: Documentation, handoff, review.

**Manual workflow**:

```
Step 1: Run the command
   /conductor:export

Step 2: Select export format
   A) Markdown summary
   B) JSON data
   C) Both

Step 3: Choose scope
   A) Full project
   B) Specific tracks

Step 4: Report generated
   - Output to conductor/exports/ or console
```

---

### 12. `/conductor:refresh` (or `/conductor-refresh`)

**Purpose**: Sync context docs with current codebase.

**When to use**: 
- Codebase changed outside Conductor
- Documentation drift detected

**Manual workflow**:

```
Step 1: Run the command
   /conductor:refresh

Step 2: Codebase analysis
   - Scans for changes since last refresh

Step 3: Review proposed updates
   - product.md changes
   - tech-stack.md changes
   - code_styleguides/ updates

Step 4: Approve changes
   - Applied incrementally
   - State saved in refresh_state.json
```

---

## State Files Reference

| File | Purpose | Location |
|------|---------|----------|
| `setup_state.json` | Setup progress | `conductor/` |
| `implement_state.json` | Implementation resume | `conductor/tracks/<id>/` |
| `refresh_state.json` | Refresh progress | `conductor/` |
| `metadata.json` | Track configuration | `conductor/tracks/<id>/` |

## Tips for Manual Usage

1. **Always check state files** before running commands to understand current progress

2. **Use `/conductor:status`** frequently to see the big picture

3. **Run `/conductor:validate`** after manual edits to catch issues

4. **Commit frequently** - each task should have its own commit

5. **Keep plan.md updated** - status markers are the source of truth

6. **Use conventional commits**:
   - `feat(scope): description`
   - `fix(scope): description`
   - `docs(scope): description`
   - `conductor(plan): Mark task complete`

## Common Issues

| Issue | Solution |
|-------|----------|
| Command stalls | Check state file, resume or restart |
| Wrong track selected | Use explicit track ID parameter |
| Task stuck in progress | Manually update `[~]` to `[ ]` in plan.md |
| Dependency loop | Use `/conductor:validate` to detect |
| Missing files | Run `/conductor:setup` or `/conductor:validate` |
