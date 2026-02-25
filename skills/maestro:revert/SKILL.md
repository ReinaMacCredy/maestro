---
name: maestro:revert
description: "Git-aware revert of track, phase, or individual task. Safely undoes implementation with plan state rollback."
argument-hint: "<track> [--phase <N>] [--task <name>]"
---

# Revert -- Git-Aware Undo

> This skill is CLI-agnostic. It works with Claude Code, Codex, Amp, or any AI coding assistant.

Safely revert implementation work at track, phase, or task granularity. Updates plan state to reflect the rollback.

Validate the result of every operation. If any step fails, halt and report the failure before continuing.

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

If no `<track>` argument, proceed to Guided Selection.

## Step 1a: Guided Selection (when no track argument)

Read `.maestro/tracks.md` and recent maestro git history (`git log --oneline --since="7 days ago" --grep="maestro"`).

Present a menu grouped by track showing ID, description, status, and completed task count. If user provides a custom track ID, use that.

## Step 2: Locate Track

Match track argument against IDs and descriptions in `.maestro/tracks.md`. If not found: report and stop.

## Step 3: Resolve Commit SHAs

Extract implementation SHAs, plan-update commits, and track creation commit (for track-level).
See `reference/git-operations.md` for full SHA resolution protocol (steps 3a-3c).

## Step 4: Git Reconciliation

Verify each SHA exists, detect merge commits and cherry-pick duplicates.
See `reference/git-operations.md` for reconciliation protocol (steps 4a-4b).

## Step 5: Present Execution Plan

Show exactly what will be reverted with commit list, affected files, and plan updates.
See `reference/confirmation-and-plan.md` for the plan format.

## Step 6: Multi-Step Confirmation

Two-phase confirmation with optional plan revision loop.
See `reference/confirmation-and-plan.md` for the confirmation protocol.

## Step 7: Execute Reverts

Revert in reverse chronological order. Handle merge commits and conflicts.
See `reference/git-operations.md` for execution protocol (step 7).

## Steps 8-10: Update Plan State, Registry, and Verify

Reset plan markers, update registry status, run test suite.
See `reference/git-operations.md` for details (steps 8-10).

## Step 11: Summary

Display revert summary with scope, commit counts, test results, and next steps.
See `reference/confirmation-and-plan.md` for the summary format.

---

## Relationship to Other Commands

Recommended workflow:

- `/maestro:setup` -- Scaffold project context (run first)
- `/maestro:new-track` -- Create a feature/bug track with spec and plan
- `/maestro:implement` -- Execute the implementation
- `/maestro:review` -- Verify implementation correctness
- `/maestro:status` -- Check progress across all tracks
- `/maestro:revert` -- **You are here.** Undo implementation if needed
- `/maestro:note` -- Capture decisions and context to persistent notepad

Revert is the safety valve for `/maestro:implement`. It undoes commits and resets plan state so you can re-implement with `/maestro:implement`. Use `/maestro:status` after reverting to confirm the track state is correct. Revert depends on atomic commits from implementation -- the cleaner the commit history, the more precise the revert.
