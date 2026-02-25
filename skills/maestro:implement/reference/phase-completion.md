# Phase Completion Protocol

## When to Run

Execute this protocol when the last task in a phase is completed (marked `[x]`).

## Steps

### 1. Test Coverage Check

Determine the scope of changes since the phase started:

```bash
# Find the first commit of this phase (from plan.md SHA markers)
git diff --name-only {phase_start_sha}..HEAD
```

For each changed source file:
- Check if a corresponding test file exists
- If not: create a test file with basic tests

Run coverage:
```bash
CI=true {coverage_command}
```

Compare against threshold from `workflow.md`. If below threshold:
- Identify uncovered lines/functions
- Write additional tests to improve coverage
- Re-run coverage to confirm improvement

### 2. Automated Test Execution

Run the full test suite:
```bash
CI=true {test_command}
```

**On success**: Proceed to manual verification.

**On failure**:
- Attempt 1: Read error output, diagnose, fix
- Attempt 2: If still failing, try a different approach
- After 2 failed attempts: HALT and ask user

```
AskUserQuestion(
  questions: [{
    question: "Tests are failing after 2 fix attempts. How should we proceed?",
    header: "Test Failure",
    options: [
      { label: "Show me the errors", description: "I'll help debug" },
      { label: "Skip this check", description: "Continue despite test failures (not recommended)" },
      { label: "Revert phase", description: "Undo all changes in this phase" }
    ],
    multiSelect: false
  }]
)
```

### 3. Manual Verification Plan

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

### 4. User Confirmation

Present the verification plan and wait for approval:

```
AskUserQuestion(
  questions: [{
    question: "Phase {N} is complete. Please verify the manual steps above. All good?",
    header: "Phase {N}",
    options: [
      { label: "Verified, continue", description: "Phase looks good, move to next phase" },
      { label: "Issue found", description: "Something isn't working as expected" }
    ],
    multiSelect: false
  }]
)
```

**If issue found**:
1. Ask user to describe the issue
2. Create a fix task
3. Execute the fix task
4. Re-run this verification protocol

### 5. Record Checkpoint

After user approval:
```bash
# Record the checkpoint SHA
CHECKPOINT_SHA=$(git rev-parse --short HEAD)
```

Store in metadata or plan.md for future reference (used by `/maestro:revert` for phase-level reverts).

## Documentation Sync (Track Completion Only)

After the FINAL phase completes, check if project docs need updating:

1. Read the track's `spec.md`
2. Compare against current `product.md`, `tech-stack.md`, `guidelines.md`
3. If the track introduces new capabilities, technologies, or patterns:
   - Propose specific doc updates
   - Get user approval for each
   - Apply and commit
