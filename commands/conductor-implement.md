---
description: Execute tasks from a track using beads for tracking
argument-hint: [track_id or "Start epic <epic-id>"]
---

# Conductor Implement

Implement track: $ARGUMENTS

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

## 1. Handoff Detection

If `$ARGUMENTS` matches `Start epic <epic-id>`:

1. **Load epic from beads:**
   ```bash
   bd show <epic-id> --json
   ```

2. **Parse notes for plan location:**
   Look for `PLAN: <path>` in notes field
   
3. **Read the plan:**
   ```bash
   cat <plan-path>
   ```

4. **Get ready tasks:**
   ```bash
   bd ready --json
   ```
   Filter to tasks that are children of this epic

5. **Begin execution** at Step 6 with epic context loaded

If no handoff detected, proceed to Step 2.

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

## 5. Update Track Status

In `conductor/tracks.md`, change `## [ ] Track:` to `## [~] Track:` for selected track.

## 6. Find Available Tasks

```bash
bd ready --json
```

Present available tasks to user and claim the next one.

## 7. Execute Task Loop

For each task:

### 7.1 Claim Task

```bash
bd update <issue-id> --status in_progress --json
```

**CRITICAL: Record thread URL for doc-sync integration**

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

### 7.2 Setup Isolation (Optional)

For complex tasks, create isolated worktree:
- Load `using-git-worktrees` skill
- Create worktree for the task

Skip for simple tasks.

### 7.3 TDD Workflow (if workflow.md specifies)

1. Write failing tests for the task
2. Run tests, confirm they fail
3. Implement minimum code to make tests pass
4. Run tests, confirm they pass
5. Refactor if needed (keep tests passing)

### 7.4 Commit Changes

```bash
git add .
git commit -m "feat(<scope>): <description>"
```

### 7.5 Complete Task in Beads (Source of Truth)

**Beads (`bd`) is the Single Source of Truth for task status.**

```bash
# Use heredoc to safely handle summaries containing quotes or special characters
bd close <issue-id> --reason "$(cat <<'EOF'
Implemented: <summary text>. Commit: <sha>
EOF
)" --json
```

If this command fails, stop and report the error. Do not proceed until beads status is updated.

### 7.6 Update plan.md (Best-Effort Sync)

Mark corresponding task in plan.md as complete:
- Change `[ ]` to `[x]`
- Append commit SHA

**Note**: This is for human readability only. If `bd close` succeeded but this update fails, log a warning but do not fail the workflow - Beads status is authoritative.

### 7.7 Check for More Tasks

```bash
bd ready --json
```

- If more tasks ready: Continue loop (step 7.1)
- If no tasks ready but some blocked: Show blockers and wait
- If all tasks done: Proceed to completion

## 8. Phase Verification

At end of each phase (epic completion):

1. Run full test suite
2. Present manual verification steps to user
3. Ask for explicit confirmation: "Does this work as expected?"
4. Create checkpoint commit: `conductor(checkpoint): Phase <name> complete`

## 9. Track Completion

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
