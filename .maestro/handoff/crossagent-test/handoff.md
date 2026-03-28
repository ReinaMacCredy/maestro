# Cross-Agent Handoff Test

## From
claude

## To
codex

## Objective
Validate that an external agent (Codex) can interact with the maestro CLI harness installed in this project.

## Instructions

You are receiving a handoff from another agent. This project uses `maestro` -- a CLI tool for agent coordination. Follow these steps exactly:

### Step 1: Verify maestro is available
Run: `maestro ping --json`
Record whether it succeeded.

### Step 2: Check project status
Run: `maestro status --json`
Note the current pipeline stage and active feature (if any).

### Step 3: List existing features
Run: `maestro feature-list --json`
Record how many features exist and their names.

### Step 4: Create a test feature
Run: `maestro feature-create codex-handoff-test --json`
Record the feature name and path from the output.

### Step 5: Write your report
Create a file at `.maestro/handoff/crossagent-test/report.md` with:
- Which steps succeeded and which failed
- The JSON output from each step (or the error)
- Any issues you encountered (confusing flags, missing commands, etc.)
- Your agent name and timestamp

### Step 6: Clean up
Run: `maestro feature-complete --json`
This marks the test feature as done.

## Notes
- Always pass `--json` to every maestro command
- If a command fails, record the error and continue to the next step
- Do NOT skip steps -- even if one fails, attempt all of them
