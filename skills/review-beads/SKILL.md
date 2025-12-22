---
name: review-beads
version: "1.2.1"
description: Review, proofread, and refine filed Beads epics and issues
argument-hint: [optional: specific epic or issue IDs to focus on]
---

# Review and Refine Beads Issues

Review, proofread, and polish filed Beads epics and issues using **parallel subagents** for speed.

## Phase 1: Load & Distribute

Get all epics and prepare for parallel review:

```bash
bd list -t epic --json
```

If specific IDs were provided (`$ARGUMENTS`), focus on those. Otherwise, review all epics.

**For each epic, gather its child issues:**
```bash
bd show <epic-id> --json
```

## Phase 2: Parallel Epic Reviews

Dispatch **ALL subagents in parallel** — each reviews one epic and its children.

### Subagent Prompt Template

```markdown
Review Epic: "<EPIC_TITLE>" (ID: bd-<EPIC_ID>)

## Your Task
Review and refine this epic and all its child issues.

## Issues to Review
<LIST_OF_ISSUE_IDS_AND_TITLES>

## Review Checklist

For EACH issue, verify:

### Clarity
- [ ] Title is action-oriented and specific
- [ ] Description is clear and unambiguous
- [ ] A developer unfamiliar with the codebase could understand
- [ ] No jargon without explanation

### Completeness
- [ ] Acceptance criteria are defined and testable
- [ ] Technical implementation hints provided where helpful
- [ ] Relevant file paths or modules mentioned
- [ ] Edge cases and error handling considered

### Dependencies
- [ ] All blocking dependencies are linked
- [ ] Dependencies are minimal (not over-constrained)

### Scope
- [ ] Issue is appropriately sized (not too large)
- [ ] No duplicate or overlapping issues

### Priority
- [ ] Priority reflects actual importance

## Common Fixes

1. **Vague titles**: "Fix bug" → "Fix null pointer in UserService.getProfile"
2. **Missing context**: Add relevant file paths, function names
3. **Implicit knowledge**: Make assumptions explicit
4. **Missing acceptance criteria**: Add "Done when..." statements

## Update Commands

```bash
bd update <id> --title "Improved title" --json
bd update <id> --description "New description" --json
bd update <id> --acceptance "Acceptance criteria" --json
bd update <id> --priority <new-priority> --json
```

## Return Format

Return ONLY this JSON (no other text):
```json
{
  "epicId": "bd-<EPIC_ID>",
  "epicTitle": "<title>",
  "issuesReviewed": 5,
  "issuesUpdated": 3,
  "changes": [
    {"id": "bd-XXX", "change": "Clarified title"},
    {"id": "bd-XXX", "change": "Added acceptance criteria"}
  ],
  "concerns": [
    {"id": "bd-XXX", "issue": "Needs user input on scope"}
  ],
  "crossEpicIssues": [
    {"id": "bd-XXX", "issue": "May conflict with Epic Y task Z"}
  ]
}
```
```

### Parallel Dispatch Example

> **IMPORTANT:** You MUST actually invoke the Task tool. Do not just describe or write about dispatching — execute it.

```
// Dispatch ALL at once
Task(description: "Review Epic: Authentication (bd-1)", prompt: <above template>)
Task(description: "Review Epic: Database Layer (bd-2)", prompt: <above template>)
Task(description: "Review Epic: API Endpoints (bd-3)", prompt: <above template>)
```

**All subagents run in parallel** — no waiting between dispatches.

## Phase 3: Cross-Epic Validation

When ALL subagents return, perform cross-epic validation:

### 3.1 Check for Cycles

```bash
bd dep cycles --json
```

If cycles found, fix them:
```bash
bd dep remove bd-<from> bd-<to> --json
```

### 3.2 Validate Cross-Epic Links

Review `crossEpicIssues` from all subagents:
- Check if flagged conflicts are real
- Verify cross-epic dependencies make sense
- Add missing cross-epic deps if needed:
  ```bash
  bd dep add bd-<from> bd-<to> --type blocks --json
  ```

### 3.3 Check for Orphans

```bash
bd list --json
```

Verify:
- All issues belong to an epic (have parent dep)
- No dangling references to deleted issues

### 3.4 Verify Critical Path

```bash
bd ready --json
```

Check:
- Some issues are ready (unblocked)
- Critical path items have correct priorities
- Parallelization opportunities preserved

## Phase 4: Summary & Handoff

### Summary Format

Present combined results:

```
## Review Summary

**Epics reviewed:** 3
**Issues reviewed:** 15
**Issues updated:** 8

| Epic | Reviewed | Updated | Concerns |
|------|----------|---------|----------|
| Authentication | 5 | 3 | 1 |
| Database Layer | 6 | 4 | 0 |
| API Endpoints | 4 | 1 | 2 |

**Cross-epic validation:**
- Cycles: 0
- Orphans: 0
- Cross-deps verified: 4

**Remaining concerns:**
- bd-12: Needs user input on scope
- bd-18: May conflict with auth middleware

**Ready for implementation:** 6 issues
```

### HANDOFF Block (REQUIRED)

**You MUST output this block — do not skip:**

```markdown
## HANDOFF

**Command:** `Start epic <first-epic-id>`
**Epics:** <count> epics reviewed
**Ready issues:** <count>
**First task:** <first-ready-issue-id> - <title>

Copy the command above to start a new session.
```

### Completion Message

Say: **"Issues reviewed. Run `/conductor-implement` to start execution."**

## Priority Guide

| Priority | Use For |
|----------|---------|
| 0 | Critical path blockers, security |
| 1 | Core functionality, high value |
| 2 | Standard work (default) |
| 3 | Nice-to-haves, polish |
| 4 | Backlog, future |

## Why This Approach?

- **Parallel reviews** — Each epic reviewed simultaneously
- **Cross-epic validation** — Catches inter-epic issues after parallel phase
- **No conflicts** — Each subagent updates different epic's issues
- **Fast execution** — All epics reviewed at once
- **Comprehensive** — Both intra-epic and cross-epic quality checks
