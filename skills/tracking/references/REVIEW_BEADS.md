
# Review and Refine Beads Issues

Review, proofread, and polish filed Beads epics and issues using **parallel subagents** for speed.

## Phase 0: Pre-Check & Track Discovery

### 0.1 Track Integrity Validation

Before any operations, validate track integrity per `skills/conductor/validation/track-checks.md`:

1. **Validate JSON files:** All state files must parse correctly (HALT on corruption)
2. **track_id validation:** Auto-fix mismatches in state files (directory name is source of truth)
3. **File existence matrix:** Verify track has valid file combination

**If validation fails:** HALT and report the issue.

### 0.2 Scan for Track Context

If `$ARGUMENTS` contains a track_id:
- Look for `.fb-progress.json` at `conductor/tracks/<track_id>/.fb-progress.json`

If no track_id provided, scan all tracks:
```bash
find conductor/tracks -name ".fb-progress.json" -type f 2>/dev/null
```

**If multiple tracks found:**
```
Found beads in multiple tracks:
1. auth_20251223 (5 epics, filed 2h ago)
2. api_20251223 (3 epics, filed 1d ago)

Which track to review? [1/2/all]
```

### 0.3 Check Track Directory Exists

Before reading progress file, verify the track directory exists:

```bash
test -d conductor/tracks/<track_id>
```

**If track directory missing (Edge Case 3: Deleted Track):**
```
⚠️ Track '<track_id>' deleted. Reviewing beads without track context.
```
- Continue without track context
- Use `bd list -t epic --json` to find epics directly
- Skip progress file operations

### 0.4 Check Progress File Status

Read `.fb-progress.json` from track directory:

```bash
cat conductor/tracks/<track_id>/.fb-progress.json
```

**Status checks:**

| Status | Behavior |
|--------|----------|
| `status: "in_progress"` | Warn: "Beads filing incomplete. Wait for fb to finish or resume with `fb <track_id>`." Consider halting. |
| `status: "failed"` | Warn: "Previous fb run failed. Resume with `fb <track_id>` before reviewing." |
| `status: "complete"` | Proceed with review |
| File not found | Warn: "No beads found for this track. Run `fb` first." HALT. |

**If proceeding:** Extract epic IDs from progress file for focused review.

### 0.5 Sync Stale Progress (Edge Case 4)

Before proceeding, compare progress file against current beads state:

```bash
bd list -t epic --json
```

**Compare:**
- Epic `updated_at` (from beads) vs progress file `lastVerified` timestamp
- If epic `updated_at` > progress `lastVerified` → needs re-sync
- If epic in progress file but missing from `bd list` → remove from progress

**If stale (auto-correct with diff):**
```
ℹ️ Progress file synced with beads:
  - Removed: bd-3 (Epic: Auth) — no longer exists
  - Removed: bd-4 (Epic: API) — no longer exists
  Continuing with 3 epics.
```

Update progress file with corrected epic list and new `lastVerified` timestamp.

### 0.6 Initialize Review State

Create/update `.fb-progress.json` with review tracking:

```json
{
  "trackId": "<track_id>",
  "status": "complete",
  "reviewStatus": "in_progress",
  "reviewStartedAt": "<timestamp>",
  "reviewThreadId": "<current-thread-id>",
  ...
}
```

## Phase 1: Load & Distribute

Get all epics from track or arguments:

**If track context available:**
```bash
# Get epic IDs from progress file
jq '.epics[].id' conductor/tracks/<track_id>/.fb-progress.json
```

**Otherwise:**
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
Review Epic: "<EPIC_TITLE>" (ID: <EPIC_ID>)

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

## After Review - Mark as Reviewed

After reviewing each issue, add the "reviewed" label:
```bash
bd update <id> --label reviewed --json
```

## Return Format

**CRITICAL: Validate your JSON before returning.**

Return ONLY this JSON (no other text):
```json
{
  "epicId": "<EPIC_ID>",
  "epicTitle": "<title>",
  "issuesReviewed": 5,
  "issuesUpdated": 3,
  "changes": [
    {"id": "<issue-id>", "change": "Clarified title"},
    {"id": "<issue-id>", "change": "Added acceptance criteria"}
  ],
  "concerns": [
    {"id": "<issue-id>", "issue": "Needs user input on scope"}
  ],
  "crossEpicIssues": [
    {"id": "<issue-id>", "issue": "May conflict with Epic Y task Z"}
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
bd dep remove <from> <to> --json
```

### 3.2 Validate Cross-Epic Links

Review `crossEpicIssues` from all subagents:
- Check if flagged conflicts are real
- Verify cross-epic dependencies make sense
- Add missing cross-epic deps if needed:
  ```bash
  bd dep add <from> <to> --type blocks --json
  ```

### 3.3 Check for Orphans (Edge Case 2)

```bash
bd list --json
```

Verify:
- All issues belong to an epic (have parent dep)
- No dangling references to deleted issues

**If orphan beads found (warn and include):**
```
⚠️ Found 1 epic without track origin. Including in review.
  (This may indicate fb didn't complete properly)
```
- Do NOT skip orphan beads
- Include them in review anyway
- Flag in summary as potential issue

### 3.4 Verify Critical Path

```bash
bd ready --json
```

Check:
- Some issues are ready (unblocked)
- Critical path items have correct priorities
- Parallelization opportunities preserved

### 3.5 Update Progress File

Update `.fb-progress.json` with review state:

```json
{
  "trackId": "<track_id>",
  "status": "complete",
  "reviewStatus": "complete",
  "lastVerified": "<timestamp>",
  
  "epics": [
    {
      "id": "bd-1",
      "title": "Epic: Authentication",
      "status": "complete",
      "createdAt": "<timestamp>",
      "reviewed": true,
      "reviewedAt": "<timestamp>"
    }
  ],
  ...
}
```

## Phase 4: Summary & Handoff

### 4.1 Collect All Concerns

Aggregate `concerns` from all subagent results into a single list:

```
**Needs User Input:**
- bd-12: Scope unclear - should this include admin users?
- bd-18: Technical approach - use Redis or in-memory cache?
- bd-25: Priority unclear - is this blocking release?
```

**Do not pause mid-review for user input.** Complete the full review first, then present all concerns together.

### 4.2 Summary Format

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

**Needs User Input (3):**
- bd-12: Scope unclear - should this include admin users?
- bd-18: Technical approach - use Redis or in-memory cache?
- bd-25: Priority unclear - is this blocking release?

**Ready for implementation:** 6 issues
```

### 4.3 HANDOFF Block (REQUIRED)

**You MUST output this block — do not skip:**

1. Update workflow state in metadata.json:
   ```bash
   jq --arg timestamp "<current-timestamp>" \
      '.workflow.state = "REVIEWED" | .workflow.history += [{"state": "REVIEWED", "at": $timestamp, "command": "rb"}]' \
      "conductor/tracks/<track_id>/metadata.json" > "conductor/tracks/<track_id>/metadata.json.tmp.$$" && mv "conductor/tracks/<track_id>/metadata.json.tmp.$$" "conductor/tracks/<track_id>/metadata.json"
   ```

2. Display completion with suggestion:
   ```
   ┌─────────────────────────────────────────┐
   │ ✓ Beads reviewed                        │
   │                                         │
   │ Epics reviewed: N                       │
   │ Issues updated: N                       │
   │ Ready to start: N epics in parallel     │
   │                                         │
   │ → Next: Start epic {first-epic-id}      │
   │   Or: bd ready (show all ready tasks)   │
   │   Alt: /conductor-implement <track_id>  │
   └─────────────────────────────────────────┘
   ```

3. Include handoff info:
   ```markdown
   ## HANDOFF

   **Command:** `Start epic <first-epic-id>`
   **Epics:** <count> epics reviewed
   **Ready issues:** <count>
   **First task:** <first-ready-issue-id> - <title>

   Copy the command above to start a new session.
   ```

### 4.4 Completion Message

Say: **"Issues reviewed. Run `/conductor-implement` to start execution."**

## Priority Guide

| Priority | Use For |
|----------|---------|
| 0 | Critical path blockers, security |
| 1 | Core functionality, high value |
| 2 | Standard work (default) |
| 3 | Nice-to-haves, polish |
| 4 | Backlog, future |

## Error Recovery

| Error | Recovery |
|-------|----------|
| Progress file not found | Suggest running `fb` first |
| fb still in progress | Wait or suggest resuming fb |
| Multiple tracks found | Prompt user to select |
| Subagent returns invalid JSON | Retry once, then handle in main agent |
| Epic has no child issues | Flag as concern, continue |
| Track directory deleted | Warn, review beads without track context |
| Orphan beads found | Warn, include in review anyway |
| Stale progress file | Auto-sync with beads, show diff of changes |

## Why This Approach?

- **Track-aware** — Discovers track context from progress files
- **Pre-flight checks** — Ensures beads are filed before review
- **Parallel reviews** — Each epic reviewed simultaneously
- **Cross-epic validation** — Catches inter-epic issues after parallel phase
- **Two-layer tracking** — Progress file + beads labels for audit trail
- **Deferred concerns** — Collects all user-input items, presents at end
- **No conflicts** — Each subagent updates different epic's issues
- **Fast execution** — All epics reviewed at once
- **Comprehensive** — Both intra-epic and cross-epic quality checks
