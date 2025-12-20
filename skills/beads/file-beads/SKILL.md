---
name: file-beads
description: File detailed Beads epics and issues from a plan
argument-hint: <plan-description-or-context>
---

# File Beads Epics and Issues from Plan

Convert a plan into Beads epics and issues using **sequential subagents** to keep the main context clean.

> **Why Sequential?** The `bd` CLI does not have file locking or atomic ID generation.
> Parallel `bd create` calls cause race conditions (ID collisions, data corruption).
> Each epic must complete before the next begins.

## Phase 1: Analyze Plan

Review the plan context: `$ARGUMENTS`

If no plan provided, check:
- Recent brainstorming output in current context
- `docs/plans/` directory
- `conductor/tracks/` for spec.md and plan.md files

**Identify for each epic:**

| Field | Description |
|-------|-------------|
| Epic title | Clear workstream name |
| Child tasks | Individual issues under this epic |
| Intra-epic deps | Dependencies within the epic |
| Cross-epic hints | Tasks that depend on other epics (by name, not ID) |
| Priority | 0-4 scale |

## Phase 2: Sequential Dispatch

Dispatch one subagent per epic. **Wait for each to complete before starting the next** to avoid ID collisions.

### Subagent Prompt Template

```markdown
File Epic: "<EPIC_TITLE>"

## Your Task
Create one epic and all its child issues in Beads.

## Epic Context
<PASTE_EPIC_SECTION_FROM_PLAN>

## Steps

1. Create the epic:
   ```bash
   bd create "Epic: <title>" -t epic -p <priority> --json
   ```

2. For each task, create an issue with parent dependency:
   ```bash
   bd create "<task title>" -t <type> -p <priority> --deps bd-<epic-id> --json
   ```
   
   Include in each issue:
   - Clear action-oriented title
   - Acceptance criteria
   - Technical notes if relevant

3. Link intra-epic dependencies:
   ```bash
   bd dep add bd-<child> bd-<blocker> --type blocks --json
   ```

## Return Format

Return ONLY this JSON (no other text):
```json
{
  "epicId": "bd-XXX",
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

### Dispatch Example

```
Task(description: "File Epic: Authentication", prompt: <above template>)
// Wait for result...

Task(description: "File Epic: Database Layer", prompt: <above template>)
// Wait for result...

Task(description: "File Epic: API Endpoints", prompt: <above template>)
// Wait for result...
```

**Execute sequentially** — each subagent must return before dispatching the next.

## Phase 3: Collect & Link Cross-Epic Dependencies

When subagents return:

1. Parse JSON results from each subagent
2. Build ID lookup table:
   ```
   "Authentication" → bd-101
   "Database Layer" → bd-102
   "Setup user table" → bd-105
   ```

3. Resolve cross-epic dependencies:
   ```bash
   bd dep add bd-<from> bd-<to> --type blocks --json
   ```

## Phase 4: Verify & Summarize

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

**Start with:** bd-105 (Setup user table), bd-108 (Init auth config)

**Cross-epic deps linked:** 2
```

## Priority Guide

| Priority | Use For |
|----------|---------|
| 0 | Critical path blockers, security |
| 1 | Core functionality, high value |
| 2 | Standard work (default) |
| 3 | Nice-to-haves, polish |
| 4 | Backlog, future |

## Why Sequential?

- **No race conditions** — `bd` CLI lacks file locking; serial execution prevents ID collisions
- **Context hygiene** — `bd create` output stays in subagent contexts
- **Reliable linking** — Cross-epic dependencies resolve correctly since all IDs are stable
- **Clean summary** — Main agent only sees results, not noise

## Phase 5: HANDOFF Block Output

After filing, save handoff state and output for next session:

### Step 1: Save handoff state to beads

```bash
bd update <epic-id> --notes "HANDOFF_READY: true. PLAN: <plan-path>"
```

Where `<plan-path>` is the source plan (e.g., `conductor/plans/2024-12-20-feature-design.md` or `conductor/tracks/<id>/plan.md`)

### Step 2: Output HANDOFF block to user

```markdown
## HANDOFF

**Command:** `Start epic <epic-id>`
**Epic:** <epic-id> - <epic-title>
**Plan:** <plan-path>
**Ready issues:** <count>
**First task:** <first-issue-id> - <title>

Copy the command above to start a new session.
```

This enables the execution-workflow to detect handoffs and load epic context automatically.
