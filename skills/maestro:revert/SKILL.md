---
name: maestro:revert
description: "Git-aware revert of track, phase, or individual task. Safely undoes implementation with plan state rollback."
argument-hint: "<track> [--phase <N>] [--task <name>]"
allowed-tools: Read, Bash, Glob, Grep, AskUserQuestion
disable-model-invocation: true
---

# Revert -- Git-Aware Undo

Safely revert implementation work at track, phase, or task granularity. Updates plan state to reflect the rollback.

CRITICAL: You must validate the success of every tool call. If any tool call fails, halt immediately and report the error.

When using AskUserQuestion, immediately call the tool -- do not repeat the question in plain text.

## Arguments

`$ARGUMENTS`

- `<track>`: Track name or ID (optional -- if omitted, enter Guided Selection)
- `--phase <N>`: Revert only phase N (optional)
- `--task <name>`: Revert only a specific task (optional)
- No scope flag: revert the entire track

---

## Step 1: Parse Target Scope

Determine what to revert:
- **Track-level**: No `--phase` or `--task` flag. Revert all commits in the track.
- **Phase-level**: `--phase N` specified. Revert commits from phase N only.
- **Task-level**: `--task <name>` specified. Revert a single task's commit.

If no `<track>` argument was provided, proceed to **Step 1a: Guided Selection** before continuing.

---

## Step 1a: Guided Selection (when no track argument provided)

Scan all tracks for context:

```
Read(file_path: ".maestro/tracks.md")
```

Then for each track found (up to 4), check for in-progress or recently completed items:

```bash
git log --oneline --since="7 days ago" --grep="maestro" -- .maestro/tracks/
```

Build a hierarchical menu grouped by track, showing:
- Track ID and description
- Status: `[ ]` pending, `[~]` in-progress, `[x]` completed
- Number of completed tasks (potential revert candidates)

Present the menu:

```
AskUserQuestion(
  questions: [{
    question: "Which track do you want to revert?",
    header: "Guided Selection",
    options: [
      { label: "{track_id}: {description}", description: "{N} completed tasks, status: {status}" },
      // ... up to 4 tracks
      { label: "Other", description: "Enter a track ID manually" }
    ],
    multiSelect: false
  }]
)
```

If user selects "Other", ask:

```
AskUserQuestion(
  questions: [{
    question: "Enter the track ID to revert:",
    header: "Manual Track Entry"
  }]
)
```

Use the selected or entered value as the `<track>` argument and continue to Step 2.

---

## Step 2: Locate Track

```
Read(file_path: ".maestro/tracks.md")
```

Match the track argument against track IDs and descriptions.

If not found:
- Report: "Track not found. Available tracks: {list}"
- Stop.

---

## Step 3: Resolve Commit SHAs

Read the track's plan:
```
Read(file_path: ".maestro/tracks/{track_id}/plan.md")
```

**3a: Extract implementation SHAs**

Extract `[x] {sha}` markers from the appropriate scope:
- **Track**: All `[x] {sha}` markers
- **Phase N**: Only markers under `## Phase N`
- **Task**: Only the marker for the matching task

**3b: Identify plan-update commits**

Search for commits that marked tasks as done in this track's plan.md (these have the message pattern `maestro(plan): mark task...`):

```bash
git log --oneline --all --grep="maestro(plan): mark task" -- .maestro/tracks/{track_id}/plan.md
```

Add any matching SHAs to the revert list alongside the implementation commits. These must also be reverted to restore plan state accurately.

**3c: Identify track creation commit (track-level revert only)**

If reverting the entire track, search for the commit that introduced the track entry in tracks.md:

```bash
git log --oneline --all --grep="chore(maestro:new-track): add track {track_id}"
```

If found, add this SHA to the revert list so the track entry itself is removed from tracks.md.

If no SHAs found in scope (no implementation SHAs from 3a, and no plan-update commits from 3b):
- Report: "No completed tasks found in the specified scope. Nothing to revert."
- Stop.

---

## Step 4: Git Reconciliation

For each SHA in the combined revert list, verify it exists and hasn't been rewritten:

```bash
git cat-file -t {sha}
```

CRITICAL: Validate this command succeeds for each SHA before continuing.

**If SHA exists**: Add to revert list.

**If SHA is missing** (rebased, squashed, or force-pushed):
- Warn: "Commit {sha} no longer exists (likely rewritten by rebase/squash)."
- Try to find the replacement:
  ```bash
  git log --all --oneline --grep="{original commit message}"
  ```
- If found: offer to use the replacement SHA
- If not found: skip this commit and warn user

**4a: Merge commit detection**

For each SHA that exists, check whether it is a merge commit:

```bash
git cat-file -p {sha}
```

Inspect the output for multiple `parent` lines. If two or more `parent` lines are present, the commit is a merge commit.

If any merge commits are found, warn for each one:

"Commit {sha} is a merge commit. Reverting merge commits may have unexpected results."

Then ask:

```
AskUserQuestion(
  questions: [{
    question: "One or more commits to revert are merge commits ({sha_list}). How should we proceed?",
    header: "Merge Commit Warning",
    options: [
      { label: "Proceed anyway", description: "Attempt git revert with -m 1 for merge commits" },
      { label: "Skip merge commits", description: "Exclude merge commits from the revert list and continue" },
      { label: "Cancel", description: "Abort the revert" }
    ],
    multiSelect: false
  }]
)
```

**4b: Cherry-pick duplicate detection**

After building the full commit list, check for duplicates introduced by cherry-picks. Compare commit messages across all SHAs:

```bash
git log --format="%H %s" {sha1} {sha2} ...
```

For any two commits with identical subject lines, compare their patches:

```bash
git diff {sha_a}^ {sha_a}
git diff {sha_b}^ {sha_b}
```

If patches are substantively identical (same file changes, same hunks), treat them as duplicates. Remove the older duplicate from the revert list and note:

"Deduplicated: {sha_older} is a cherry-pick of {sha_newer} -- keeping only {sha_newer} in the revert list."

---

## Step 5: Present Execution Plan

Show the user exactly what will be reverted:

```
## Revert Plan

**Scope**: {track | phase N | task name}
**Track**: {track_description} ({track_id})

**Commits to revert** (reverse chronological order):
1. `{sha7}` -- {commit message}
2. `{sha7}` -- {commit message} [plan-update]
3. `{sha7}` -- {commit message} [track creation]

**Affected files**:
{list of files changed by these commits}

**Plan updates**:
- {task_name}: `[x] {sha}` --> `[ ]`
- {task_name}: `[x] {sha}` --> `[ ]`
```

Use `[plan-update]` and `[track creation]` labels to distinguish those commit types from implementation commits.

---

## Step 6: Multi-Step Confirmation

**Confirmation 1** -- Target:
```
AskUserQuestion(
  questions: [{
    question: "Revert {scope} of track '{description}'? This will undo {N} commits.",
    header: "Confirm Target",
    options: [
      { label: "Yes, continue", description: "Show me the execution plan" },
      { label: "Cancel", description: "Abort revert" }
    ],
    multiSelect: false
  }]
)
```

**Confirmation 2** -- Final go/no-go (3 options):
```
AskUserQuestion(
  questions: [{
    question: "Ready to execute? This will create revert commits (original commits are preserved in history).",
    header: "Execute",
    options: [
      { label: "Execute revert", description: "Create revert commits now" },
      { label: "Revise plan", description: "Modify which commits to include or exclude before executing" },
      { label: "Cancel", description: "Abort" }
    ],
    multiSelect: false
  }]
)
```

If user selects "Revise plan":
- Display the numbered commit list again
- Ask: "Enter the numbers of commits to EXCLUDE (comma-separated), or press Enter to keep all:"
  ```
  AskUserQuestion(
    questions: [{
      question: "Enter commit numbers to exclude (e.g. '2,3'), or leave blank to keep all:",
      header: "Revise Commit List"
    }]
  )
  ```
- Remove the specified commits from the list
- Re-display the updated plan and return to Confirmation 2

---

## Step 7: Execute Reverts

Revert commits in **reverse chronological order** (newest first).

For standard commits:
```bash
git revert --no-edit {sha_newest}
git revert --no-edit {sha_next}
# ... continue for all SHAs
```

For merge commits (if user chose "Proceed anyway" in Step 4a):
```bash
git revert --no-edit -m 1 {merge_sha}
```

CRITICAL: Validate each `git revert` command succeeds before continuing to the next SHA.

**On conflict**:
1. Report: "Merge conflict during revert of {sha}."
2. Show conflicting files
3. Ask user:
   ```
   AskUserQuestion(
     questions: [{
       question: "Merge conflict in {file}. How should we proceed?",
       header: "Conflict",
       options: [
         { label: "Help me resolve", description: "Show me the conflict and I'll guide resolution" },
         { label: "Abort revert", description: "Cancel remaining reverts (already-reverted commits stay)" },
         { label: "Accept theirs", description: "Keep the current version (discard the revert for this file)" }
       ],
       multiSelect: false
     }]
   )
   ```

---

## Step 8: Update Plan State

Edit `plan.md`: For each reverted implementation task, change `[x] {sha}` back to `[ ]`.

Note: plan-update commits and the track creation commit were already reverted in Step 7 via `git revert`, so their changes to tracked files are handled automatically. Only direct plan.md edits are needed here if any markers were not covered by the reverted plan-update commits.

```bash
git add .maestro/tracks/{track_id}/plan.md
git commit -m "maestro(revert): update plan state for reverted {scope}"
```

CRITICAL: Validate the commit succeeds before continuing.

---

## Step 9: Update Registry (if track-level revert)

If the entire track was reverted and the track creation commit was NOT in the revert list (i.e., user chose to keep the track entry):
- Edit `.maestro/tracks.md`: Change track status from `[x]` or `[~]` to `[ ]`
- Update metadata.json: set `"status": "new"`

```bash
git add .maestro/tracks.md .maestro/tracks/{track_id}/metadata.json
git commit -m "maestro(revert): reset track {track_id} status"
```

If the track creation commit WAS reverted in Step 7, the tracks.md entry was already removed -- skip this step.

---

## Step 10: Verify

Run the test suite to confirm the revert is clean:

```bash
CI=true {test_command}
```

CRITICAL: Validate this command completes. Report its exit code and output summary.

**If tests pass**: Report success.
**If tests fail**: Warn user that the revert introduced test failures and offer to debug.

---

## Step 11: Summary

```
## Revert Complete

**Scope**: {track | phase N | task name}
**Track**: {track_description}
**Commits reverted**: {count} ({impl_count} implementation, {plan_count} plan-update, {track_count} track creation)
**Duplicates removed**: {dedup_count} cherry-pick duplicates excluded
**Tests**: {pass | fail}

**Plan state updated**: {N} tasks reset to `[ ]`

**Next**:
- `/maestro:implement {track_id}` -- Re-implement reverted tasks
- `/maestro:status` -- Check overall progress
```
