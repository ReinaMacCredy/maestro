---
name: review
description: Post-execution review — verify /work results against plan acceptance criteria.
allowed-tools: Read, Bash, Glob, Grep
---

# Post-Execution Review

Review the results of the most recent `/work` execution against the plan's acceptance criteria.

## Process

### Step 1: Find the Plan

Look for the most recently modified plan in `.maestro/plans/`:
```
Glob(".maestro/plans/*.md")
```

Read the plan file. If no plans exist, report "No plans found. Run /design first."

### Step 2: Extract Acceptance Criteria

Parse the plan for:
- **Objective** — What was the goal?
- **Tasks** — What was supposed to be done?
- **Verification** — What checks were specified?

### Step 3: Verify Each Task

For each task in the plan:
1. Check if the referenced files exist (Glob/Read)
2. Check if the described changes are present (Grep)
3. Note any tasks that appear incomplete

### Step 4: Run Verification Commands

Execute any verification commands specified in the plan's Verification section (e.g., test commands, build commands, lint checks).

### Step 5: Check for Regressions

Run the project's standard checks if identifiable:
- Look for `package.json` scripts (test, build, lint)
- Look for `Makefile` targets
- Look for CI config files

### Step 6: Wisdom Extraction

Check `.maestro/wisdom/` for any new wisdom files created during this execution cycle.

## Output

End with a structured report:
```
## Review: {Plan Name}

### Objective
[Was it met? YES/PARTIAL/NO]

### Task Completion
| Task | Status | Evidence |
|------|--------|----------|
| Task 1 | DONE/PARTIAL/MISSING | [file/test/output] |

### Verification Results
| Check | Result |
|-------|--------|
| [Check name] | PASS/FAIL |

### Issues Found
- [Issue description and impact]

### Verdict: COMPLETE / NEEDS WORK
[Summary of what's done and what remains]
```
