---
name: file-beads
version: "1.2.0"
description: File detailed Beads epics and issues from a plan
argument-hint: <plan-description-or-context>
---

# File Beads Epics and Issues from Plan

Convert a plan into Beads epics and issues using **parallel subagents** for speed.

## Phase 1: Analyze Plan

Review the plan context: `$ARGUMENTS`

If no plan provided, check:
- Recent `/conductor-design` output in current context
- `conductor/tracks/` for design.md, spec.md, and plan.md files

**Identify for each epic:**

| Field | Description |
|-------|-------------|
| Epic title | Clear workstream name |
| Child tasks | Individual issues under this epic |
| Intra-epic deps | Dependencies within the epic |
| Cross-epic hints | Tasks that depend on other epics (by name, not ID) |
| Priority | 0-4 scale |

## Phase 2: Create Epics First (Sequential)

Create all epics FIRST to get stable IDs before parallelizing child issues:

```bash
bd create "Epic: Authentication" -t epic -p 1 --json
# Returns: {"id": "bd-1", ...}

bd create "Epic: Database Layer" -t epic -p 1 --json
# Returns: {"id": "bd-2", ...}

bd create "Epic: API Endpoints" -t epic -p 2 --json
# Returns: {"id": "bd-3", ...}
```

Record the epic ID mapping:
```
Authentication → bd-1
Database Layer → bd-2
API Endpoints → bd-3
```

## Phase 3: Parallel Dispatch for Child Issues

Now dispatch **ALL subagents in parallel** — each fills one epic with child issues.

### Subagent Prompt Template

```markdown
Fill Epic: "<EPIC_TITLE>" (ID: bd-<EPIC_ID>)

## Your Task
Create all child issues for this epic. The epic already exists.

## Epic Context
<PASTE_EPIC_SECTION_FROM_PLAN>

## Steps

1. For each task, create an issue with parent dependency:
   ```bash
   bd create "<task title>" -t <type> -p <priority> --deps bd-<EPIC_ID> --json
   ```
   
   Include in each issue:
   - Clear action-oriented title
   - Acceptance criteria
   - Technical notes if relevant

2. Link intra-epic dependencies:
   ```bash
   bd dep add bd-<child> bd-<blocker> --type blocks --json
   ```

## Return Format

Return ONLY this JSON (no other text):
```json
{
  "epicId": "bd-<EPIC_ID>",
  "epicTitle": "<title>",
  "issues": [
    {"id": "bd-XXX", "title": "...", "deps": ["bd-XXX"]}
  ],
  "crossEpicDeps": [
    {"issueId": "bd-XXX", "needsLinkTo": "<epic or task name>"}
  ]
}
```
```

### Parallel Dispatch Example

> **IMPORTANT:** You MUST actually invoke the Task tool. Do not just describe or write about dispatching — execute it.

```
// Dispatch ALL at once — epics already exist with stable IDs
Task(description: "Fill Epic: Authentication (bd-1)", prompt: <above template>)
Task(description: "Fill Epic: Database Layer (bd-2)", prompt: <above template>)
Task(description: "Fill Epic: API Endpoints (bd-3)", prompt: <above template>)
```

**All subagents run in parallel** — no waiting between dispatches.

## Phase 4: Collect & Link Cross-Epic Dependencies

When ALL subagents return:

1. Parse JSON results from each subagent
2. Build ID lookup table:
   ```
   "Authentication" → bd-1
   "Setup user table" → bd-5
   "Create auth middleware" → bd-8
   ```

3. Resolve cross-epic dependencies:
   ```bash
   bd dep add bd-<from> bd-<to> --type blocks --json
   ```

## Phase 5: Verify & Summarize

Run verification:

```bash
bd list --json
bd ready --json
```

Check:
- All epics have child issues
- No dependency cycles
- Some issues are ready (unblocked)

### Summary Format

Present to user:

```
## Filed Beads Summary

**Epics created:** 3
**Issues created:** 12

| Epic | Issues | Ready |
|------|--------|-------|
| Authentication | 4 | 2 |
| Database Layer | 5 | 1 |
| API Endpoints | 3 | 0 (blocked) |

**Start with:** bd-5 (Setup user table), bd-8 (Init auth config)

**Cross-epic deps linked:** 2
```

### After Completion

After parallel agents finish filing beads:

1. Summarize what was created (epic ID, issue count)
2. Say: "Beads filed. Say `rb` to review and refine."

## Priority Guide

| Priority | Use For |
|----------|---------|
| 0 | Critical path blockers, security |
| 1 | Core functionality, high value |
| 2 | Standard work (default) |
| 3 | Nice-to-haves, polish |
| 4 | Backlog, future |

## Why This Approach?

- **Epics first (sequential)** — Prevents ID collisions; epic IDs are stable before parallelization
- **Child issues (parallel)** — Each subagent works on a different epic, no conflicts
- **Fast execution** — All epics fill simultaneously
- **Context hygiene** — `bd create` output stays in subagent contexts
- **Reliable linking** — Cross-epic dependencies resolve correctly since all epic IDs known upfront
