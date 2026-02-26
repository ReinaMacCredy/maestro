# Phase Completion Protocol

## When to Run

Execute this protocol when the last task in a phase is completed (marked `[x]`).

**BR-based completion detection**: If `metadata.json` has `beads_epic_id`:

```bash
br list --status open --label "phase:{N}-{kebab}" --json
```

If the result is empty (no open issues for this phase), the phase is complete. Falls back to plan.md checkbox counting if no `beads_epic_id`.

## Steps

### 0. Commit Plan State

Stage and commit all accumulated plan.md state changes from this phase:

```bash
git add .maestro/tracks/{track_id}/plan.md
git commit -m "maestro(plan): mark phase {N} tasks complete"
```

**BR mirror**: If `metadata.json` has `beads_epic_id`:
```bash
br sync --flush-only
```

### 1. Automated Test Execution

Run the full test suite:
```bash
CI=true {test_command}
```

**On success**: Proceed to manual verification.

**On failure**:
- Attempt 1: Read error output, diagnose, fix
- Attempt 2: If still failing, try a different approach
- After 2 failed attempts: HALT and ask user

Ask the user: "Tests are failing after 2 fix attempts. How should we proceed?"
Options:
- **Show me the errors** -- I'll help debug
- **Skip this check** -- Continue despite test failures (not recommended)
- **Revert phase** -- Undo all changes in this phase

### 2. Manual Verification Plan

Generate step-by-step verification instructions based on what the phase implemented:

**For frontend changes**:
```
1. Start development server: {dev_server_command}
2. Open browser to: {url}
3. Test: {user action} --> Expected: {expected result}
4. Test: {edge case} --> Expected: {expected result}
```

**For backend/API changes**:
```
1. Start server: {server_command}
2. Test endpoint: curl -X {method} {url} -d '{body}'
   Expected: {response}
3. Test error case: curl -X {method} {url} -d '{invalid_body}'
   Expected: {error_response}
```

**For CLI changes**:
```
1. Run: {command} {args}
   Expected output: {output}
2. Run: {command} {invalid_args}
   Expected error: {error_message}
```

**For library/internal changes**:
```
1. Verify tests pass: {test_command}
2. Verify no regressions in dependent code: {dependent_test_command}
```

### 3. User Confirmation

Present the verification plan and wait for approval:

Ask the user: "Phase {N} is complete. Please verify the manual steps above. All good?"
Options:
- **Verified, continue** -- Phase looks good, move to next phase
- **Issue found** -- Something isn't working as expected

**If issue found**:
1. Ask user to describe the issue
2. Create a fix task
3. Execute the fix task
4. Re-run this verification protocol

### 4. Record Checkpoint

After user approval:
```bash
# Record the checkpoint SHA
CHECKPOINT_SHA=$(git rev-parse --short HEAD)
```

Store in metadata or plan.md for future reference (used by `/maestro:revert` for phase-level reverts).

