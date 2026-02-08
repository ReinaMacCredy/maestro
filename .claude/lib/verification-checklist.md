---
name: verification-checklist
description: Standard verification protocol for workers and orchestrator. Defines checks, evidence requirements, and staleness policy.
type: internal
---

# Verification Checklist

Standard verification protocol for the orchestrator to use when validating worker output. Workers should reference this checklist when marking tasks complete.

## Standard Checks

| ID | Name | Description | Evidence Type | Required |
|----|------|-------------|---------------|----------|
| BUILD | Build passes | Code compiles without errors | Command output (`build` / `compile`) | Yes |
| TEST | Tests pass | All tests pass, no regressions | Command output (`test` runner) | Yes |
| LINT | Lint clean | No lint or type-check errors | Command output (`lint` / `typecheck`) | Yes |
| FUNCTIONALITY | Features work | Behavior matches acceptance criteria | File reads + command output | Yes |
| TODO | Zero pending | No unfinished tasks remain for this scope | `TaskList()` output | Yes |
| ERROR_FREE | No errors | No unaddressed errors in modified files | Grep for error patterns | Yes |
| ARCHITECT | Lead verified | Orchestrator has independently verified | Orchestrator confirmation | Yes |

## Evidence Requirements

Workers must report evidence for each check using this format:

```
### Check: BUILD
- Command: `bun build`
- Timestamp: 2025-01-15T10:32:00Z
- Result: PASS
- Output: (first 5 lines of output)
```

Include:
1. The exact command run
2. The output timestamp (ISO 8601)
3. Pass/fail status
4. Relevant output (truncated to first 5 lines for brevity)

## Stale Evidence Policy

Command output must be from within the **last 5 minutes**. If evidence is older than 5 minutes, the orchestrator must re-run the verification command for fresh output before accepting the result.

Workers should re-run verification commands immediately before marking a task complete, not rely on earlier output from during implementation.

## Orchestrator Usage

After a worker reports task completion:

1. Read each file the worker claims to have created or modified
2. Run the BUILD, TEST, and LINT checks independently
3. Verify FUNCTIONALITY against the plan's acceptance criteria
4. Confirm TODO via `TaskList()` -- no pending/in_progress tasks for this scope
5. Check ERROR_FREE by scanning modified files for error patterns
6. Mark ARCHITECT as verified only after all other checks pass
