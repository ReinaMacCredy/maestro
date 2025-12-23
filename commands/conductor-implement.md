---
description: Execute ONE EPIC from a track using beads for tracking
argument-hint: [track_id or "Start epic <epic-id>"]
---

# Conductor Implement

**IMPORTANT: This command implements ONE EPIC per run.** When the epic is complete, the command prompts the user to either review remaining beads (`rb`) or hand off to the next epic (`Start epic <next-epic-id>`).

Implement: $ARGUMENTS

## Beads Chain Integration

This command follows the **Beads dependency chain** to execute work in the correct order:

```
Track (conductor/tracks/<id>/)
  │
  ├── Epic 1 (bd-101)           ◄── ONE EPIC PER RUN
  │   ├── Task A (bd-102)       ◄── child of Epic 1
  │   ├── Task B (bd-103)       ◄── child of Epic 1, blocks Task C
  │   └── Task C (bd-104)       ◄── blocked by Task B
  │
  ├── Epic 2 (bd-105)           ◄── NEXT RUN (via handoff)
  │   └── ...
  │
  └── Epic 3 (bd-106)           ◄── blocked by Epic 2
      └── ...
```

**Chain rules:**

- `bd ready --json` returns only **unblocked** tasks
- Tasks with `parent == $CURRENT_EPIC` are in scope
- `bd dep tree <epic-id>` shows the full chain
- Cross-epic dependencies handled via handoff blocks

## 0. Pre-flight Checks (Always Run)

**Check required tools:**

```bash
command -v jq >/dev/null 2>&1 || { echo >&2 "Error: jq is required but not installed. Please install it:\n  macOS: brew install jq\n  Ubuntu/Debian: sudo apt-get install jq\n  Fedora: sudo dnf install jq"; exit 1; }
```

**Check these files exist:**

- `conductor/product.md`
- `conductor/tech-stack.md`
- `conductor/workflow.md`

If missing, tell user to run `/conductor-setup` first.

## 1. Select Epic

**This command always works on ONE EPIC at a time.**

### 1.1 If `$ARGUMENTS` matches `Start epic <epic-id>`:

1. **Load epic from beads:**

   ```bash
   bd show <epic-id> --json
   ```

2. **Store epic-id for scoping:**

   ```bash
   CURRENT_EPIC="<epic-id>"
   ```

3. **Parse notes for plan location:**
   Look for `PLAN: <path>` in notes field
4. **Read the plan:**

   ```bash
   cat <plan-path>
   ```

5. **Proceed to Step 4** (Load Context)

### 1.2 If `$ARGUMENTS` is a track_id or empty:

1. **Find the track** in `conductor/tracks.md`
2. **List epics for this track:**

   ```bash
   bd list --type epic --json | jq '[.[] | select(.status != "closed")]'
   ```

3. **If multiple epics:** Present list and ask user which epic to implement
4. **If one epic:** Use that epic
5. **If no epics:** Say "No epics found. Run `fb` first to file beads from plan."

6. **Store selected epic:**
   ```bash
   CURRENT_EPIC="<selected-epic-id>"
   ```

## 2. Select Track

- If `$ARGUMENTS` provided (track_id), find that track in `conductor/tracks.md`
- Otherwise, find first incomplete track (`[ ]` or `[~]`) in `conductor/tracks.md`
- If no tracks found, suggest `/conductor-design` to start

## 3. Check for Beads

Check if beads exist for this track:

```bash
bd list --json
```

- **Filter results to this track** by checking for:

  - Issues with `track_id` matching current track in metadata, OR
  - Issues tagged with `track:<track_id>`, OR
  - Child issues of an epic associated with this track

- If no beads found **for this track**:

  - Say: "No issues found for this track. Run `fb` first to file beads from plan.md."
  - Exit

- If beads exist for this track, continue to next step

## 4. Load Context

Read into context:

- `conductor/tracks/<track_id>/design.md` (if exists)
- `conductor/tracks/<track_id>/spec.md`
- `conductor/tracks/<track_id>/plan.md`
- `conductor/workflow.md`

**Load TDD skill:**
Load the `test-driven-development` skill. This provides full TDD rigor (Iron Law, rationalizations to avoid, verification checklist) for section 8.3.

## 5. Update Track Status

In `conductor/tracks.md`, change `## [ ] Track:` to `## [~] Track:` for selected track.

## 6. Visualize Epic Chain

Before starting work, show the dependency chain:

```bash
# Show epic's dependency tree
bd dep tree $CURRENT_EPIC
```

**Present chain to user:**

```
Epic: Authentication (bd-101)
├── [ready] Setup OAuth config (bd-102)
├── [ready] Create user model (bd-103)
├── [blocked] Implement login flow (bd-104) ← blocked by bd-102, bd-103
└── [blocked] Add JWT refresh (bd-105) ← blocked by bd-104
```

## 7. Find Available Tasks (Epic-Scoped)

```bash
# Get ready tasks and filter to current epic's children only
bd ready --json | jq --arg epic "$CURRENT_EPIC" '[.[] | select(.parent == $epic)]'
```

**CRITICAL:** Only work on tasks that are children of `$CURRENT_EPIC`. Do not process tasks from other epics.

Present available tasks to user and claim the next one.

## 8. Execute Task Loop

For each task:

### 8.1 Claim Task

Get next ready task and claim it:

```bash
# 1. Find next ready task in epic
bd ready --json | jq --arg epic "$CURRENT_EPIC" '[.[] | select(.parent == $epic)][0]'

# 2. Claim it
bd update <issue-id> --status in_progress --json
```

**In Village mode (multi-agent):** Use `claim` MCP tool instead for atomic assignment.

**After claiming, record thread URL:**

This step is REQUIRED - doc-sync relies on thread URLs to extract knowledge.

**Before proceeding, verify:**

- [ ] You have the current Amp thread URL (check Environment section or `$AMP_THREAD_URL`)
- [ ] The URL is valid (format: `https://ampcode.com/threads/T-...` or `http://localhost:.../threads/T-...`)

```bash
# Get thread URL with fallback - skip linking if unavailable
thread_url="${AMP_THREAD_URL:-}"
if [ -z "$thread_url" ]; then
  echo "WARNING: AMP_THREAD_URL not set. Skipping thread linking."
  echo "Thread traceability will be unavailable for this task."
else
  # Use atomic comment instead of read-modify-write on notes
  # Comments are append-only and safe for multi-agent concurrency
  bd comment <issue-id> "THREAD: ${thread_url}"
fi
```

**If thread URL is unavailable:** Continue with a warning. Thread linking enables doc-sync traceability but is not required for task execution.

Read task details:

```bash
bd show <issue-id>
```

Announce: "Working on: <issue-title>"

**Update plan.md status:**
Mark the task as in-progress in the human-readable plan:

```
[ ] Task title  →  [~] Task title
```

### 8.2 Setup Isolation (Optional)

For complex tasks, create isolated worktree:

- Load `using-git-worktrees` skill
- Create worktree for the task

Skip for simple tasks.

### 8.3 TDD Workflow

1. **If TDD skill loaded** (context contains "Iron Law" or "RED-GREEN-REFACTOR"):
   Follow the skill's RED-GREEN-REFACTOR cycle with full rigor.

2. **Fallback** (skill unavailable):
   1. Write failing tests for the task
   2. Run tests, confirm they fail
   3. Implement minimum code to make tests pass
   4. Run tests, confirm they pass
   5. Refactor if needed (keep tests passing)

### 8.4 Self-Check & Issue Handling

After implementing the task (or completing the TDD cycle):

- Run tests, linting, and type checks.
- If issues are found, analyze the root cause using this decision tree.

**Issue Analysis Decision Tree:**

| Issue Type             | Indicators                                                        | Action                                                                                 |
| ---------------------- | ----------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| **Implementation Bug** | Typo, logic error, missing import, test assertion wrong           | Fix directly and continue                                                              |
| **Spec Issue**         | Requirement wrong, missing, impossible, edge case not covered     | Trigger Revise workflow for spec → update `spec.md` → log in `revisions.md` → then fix |
| **Plan Issue**         | Missing task, wrong order, task too big/small, dependency missing | Trigger Revise workflow for plan → update `plan.md` → log in `revisions.md` → continue |
| **Blocked**            | External dependency, need user input, waiting on API              | Mark as blocked, suggest `/conductor-block`                                            |

**Agent MUST announce:**

> This issue reveals [spec/plan problem \| implementation bug]. [Triggering revision \| Fixing directly].

**For Spec/Plan Issues:**

1. Create or append to `conductor/tracks/<track_id>/revisions.md` with:
   - Revision number, date, type (Spec/Plan/Both)
   - What triggered the revision
   - Current phase/task when issue occurred
   - Changes made and rationale
2. Update the relevant document (`spec.md` or `plan.md`).
3. Add a "Last Revised: <date>" marker at the top of the updated file.
4. Commit the revision before continuing to implementation.

### 8.5 Commit Changes

```bash
git add .
git commit -m "feat(<scope>): <description>"
```

### 8.6 Complete Task in Beads (Source of Truth)

**Beads (`bd`) is the Single Source of Truth for task status.**

```bash
# Use heredoc to safely handle summaries containing quotes or special characters
bd close <issue-id> --reason "$(cat <<'EOF'
Implemented: <summary text>. Commit: <sha>
EOF
)" --json
```

If this command fails, stop and report the error. Do not proceed until beads status is updated.

### 8.7 Update plan.md (Best-Effort Sync)

Mark corresponding task in plan.md as complete:

- Change `[ ]` to `[x]`
- Append commit SHA

**Note**: This is for human readability only. If `bd close` succeeded but this update fails, log a warning but do not fail the workflow - Beads status is authoritative.

### 8.8 Check for More Tasks (Epic-Scoped)

```bash
# Only check for tasks within the current epic
bd ready --json | jq --arg epic "$CURRENT_EPIC" '[.[] | select(.parent == $epic)]'
```

- If more tasks ready **in this epic**: Continue loop (step 8.1)
- If no tasks ready but some blocked **in this epic**: Show blockers and wait
- If all tasks in this epic done: **Proceed to Epic Completion (Step 9)**

**DO NOT** continue to tasks from other epics.

## 9. Epic Completion

When all tasks in `$CURRENT_EPIC` are closed:

1. **Close the epic:**

   ```bash
   bd close $CURRENT_EPIC --reason "All tasks completed" --json
   ```

2. **Run verification:**

   - Run full test suite
   - Present manual verification steps to user
   - If issues are found, apply the Issue Analysis Decision Tree from **8.4 Self-Check & Issue Handling**
   - Ask for explicit confirmation: "Does this work as expected?"

3. **Create checkpoint commit:**
   ```bash
   git commit --allow-empty -m "conductor(checkpoint): Epic $CURRENT_EPIC complete"
   ```

## 9. Next Epic Handoff

After epic completion, check for remaining epics:

```bash
bd list --type epic --json | jq '[.[] | select(.status != "closed")]'
```

### 9.1 If more epics exist:

**Present explicit choice to user (MANDATORY - do not auto-continue):**

```
Epic complete. Choose:
1. Say `rb` to review remaining beads (recommended: fewer mistakes, but uses more tokens)
2. Handoff to next epic: Start epic <next-epic-id>
```

**STOP HERE.** Wait for user response. Do not automatically continue to the next epic.

### 9.2 If no more epics:

Proceed to Track Completion (Step 10).

## 10. Track Completion

When all beads closed:

1. Update `conductor/tracks.md`: change `## [~]` to `## [x]`
2. Run `bd list --status closed --json` to show summary
3. Ask user: "Track complete. Archive, Delete, or Keep the track folder?"
4. Announce completion

## Handoff Support

If session ends before completion:

```bash
# Use atomic comment for handoff log (safe for multi-agent concurrency)
bd comment <current-issue-id> "IN_PROGRESS: <what was done>. NEXT: <what remains>. THREAD: <thread-url>"
bd sync
```

To resume in new session:

```
/conductor-implement <track_id>
```

The command will find in-progress and ready issues automatically.

## Status Reference

**Beads statuses:**

- `open` - Not started
- `in_progress` - Currently working
- `closed` - Completed

**Plan.md markers:**

- `[ ]` - Pending
- `[~]` - In Progress
- `[x]` - Completed

## Output Format

When claiming and completing tasks, use this structured format:

```
CLAIMING: <issue-id> - <title>
PRIORITY: <priority>
DEPENDENCIES: <resolved deps>

[Implementation steps...]

STATUS: Complete | In Progress | Blocked
NEXT: <recommendation>
```
