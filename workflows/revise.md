# Conductor Revise Workflow

This document defines the revise workflow for updating specs and plans when implementation reveals issues.

## Overview

Use `/conductor-revise` when:
- Implementation reveals spec issues (requirements wrong/incomplete)
- Plan needs adjustment (tasks to add/remove/modify)
- Scope changes mid-track
- Requirements evolve during development

## Revision Types

| Type | When to Use | What Changes |
|------|-------------|--------------|
| **Spec** | Requirements wrong or misunderstood | `spec.md` |
| **Plan** | Tasks need adding/removing/reordering | `plan.md` |
| **Both** | Significant scope change | `spec.md` + `plan.md` |
| **Design** | Architecture/approach fundamentally wrong | `design.md` + `spec.md` + `plan.md` |

## Workflow Steps

### 1. Identify Active Track

- Find current track (marked `[~]` in tracks.md)
- If no active track, ask user which track to revise
- Load `spec.md` and `plan.md` for context

### 2. Determine Revision Type

Ask user what needs revision:

```
What needs to be revised?
1. Spec - Requirements changed or were misunderstood
2. Plan - Tasks need to be added, removed, or modified
3. Both - Significant scope change affecting spec and plan
```

### 3. Gather Revision Context

Ask targeted questions based on revision type:

**For Spec Revisions:**
- What was discovered during implementation?
- Which requirements were wrong/incomplete?
- Are there new requirements to add?
- Should any requirements be removed?

**For Plan Revisions:**
- Which tasks are affected?
- Are there new tasks to add?
- Should any tasks be removed or reordered?
- Do task estimates need adjustment?

### 4. Create Revision Record

Create/append to `conductor/tracks/<track_id>/revisions.md`:

```markdown
## Revision [N] - [Date]

**Type:** Spec | Plan | Both
**Trigger:** [What prompted the revision]
**Phase:** [Current phase when revision occurred]
**Task:** [Current task when revision occurred]

### Changes Made

#### Spec Changes
- [List of spec changes]

#### Plan Changes
- Added: [new tasks]
- Removed: [removed tasks]
- Modified: [changed tasks]

### Rationale
[Why these changes were necessary]

### Impact
- Tasks affected: [count]
- Estimated effort change: [increase/decrease/same]
```

### 5. Update Spec (if applicable)

1. Present proposed changes to `spec.md`
2. Ask for approval
3. Apply changes
4. Add revision marker at top of spec:
   ```markdown
   > **Last Revised:** [Date] - See [revisions.md](revisions.md) for history
   ```

### 6. Update Plan (if applicable)

1. Present proposed changes to `plan.md`
2. Ask for approval
3. Apply changes:
   - New tasks: Insert at appropriate position with `[ ]`
   - Removed tasks: Mark as `[-] [REMOVED: reason]`
   - Modified tasks: Update description, keep status
4. Add revision marker at top of plan:
   ```markdown
   > **Last Revised:** [Date] - See [revisions.md](revisions.md) for history
   ```

### 7. Update Implementation State

If `implement_state.json` exists, update:
```json
{
  "last_revision": "ISO timestamp",
  "revision_count": n,
  "tasks_added": n,
  "tasks_removed": n
}
```

### 8. Commit Revision

```bash
git add conductor/tracks/<track_id>/
git commit -m "conductor(revise): Update spec/plan for <track_id>

Revision #N: [brief description]
- [key changes]"
```

### 9. Announce

```
Revision complete for track `<track_id>`:
- Spec: [updated/unchanged]
- Plan: [+N tasks, -M tasks, ~P modified]

Run `/conductor-implement` to continue with updated plan.
```

## Integration with Implement

During `/conductor-implement`, when an issue is encountered:

```
Issue Analysis Decision Tree:
├─→ Implementation bug? (typo, logic error, missing import)
│   → Fix it and continue
│
├─→ Spec issue? (requirement wrong, missing, or impossible)
│   → Trigger Revise workflow for spec
│   → Update spec.md, log in revisions.md
│   → Then fix implementation
│
├─→ Plan issue? (missing task, wrong order, task too big)
│   → Trigger Revise workflow for plan
│   → Update plan.md, log in revisions.md
│   → Then continue with updated plan
│
└─→ Blocked? (external dependency, need user input)
    → Mark as blocked, suggest /conductor-block
```

**Agent must announce:** "This issue reveals [spec/plan problem | implementation bug]. [Action taken]."

## Error Handling

| Scenario | Action |
|----------|--------|
| No active track | Ask which track to revise |
| Track completed | Warn, suggest creating new track |
| No changes needed | Report "No revision required" |
| Conflicting changes | Present conflicts, ask for resolution |

---

## Beads Integration

When revising specs/plans, affected beads must be updated.

### Triggering Bead Updates

After plan changes are applied, run the [revise-reopen-beads.md](conductor/revise-reopen-beads.md) workflow:

1. **Identify affected beads:**
   - Tasks added → create new beads
   - Tasks modified → update bead notes
   - Tasks removed → close orphan beads

2. **Handle closed beads:**
   - If bead exists and is closed → reopen with history
   - If bead was deleted (cleaned up) → create new with lineage

3. **Update mapping:**
   - Add new task → bead mappings to `.fb-progress.json`
   - Preserve `beadToTask` reverse mapping

### Bead Reopen Flow

```bash
# After plan changes
revise_reopen_workflow "$TRACK_ID" "spec revision"

# Reports:
#   - X beads reopened
#   - Y new beads created  
#   - Z mappings updated
```

### History Preservation

When reopening closed beads, preserve completion history:

```
ORIGINAL COMPLETION:
COMPLETED: Implemented feature X
KEY DECISION: Used approach Y

---
REOPENED: 2025-12-25T10:00:00Z
REASON: spec revision - requirements changed
```

### Lineage for Cleaned-Up Beads

When original bead was deleted by cleanup:

```json
{
  "title": "Rework: Original task title",
  "description": "Reopened from cleaned-up my-workflow:3-old1",
  "metadata": {
    "originalBeadId": "my-workflow:3-old1",
    "reopenedAt": "2025-12-25T10:00:00Z"
  }
}
```

---

## References

- [Revise Reopen Beads](conductor/revise-reopen-beads.md) - Detailed reopen workflow
- [Status Sync Beads](conductor/status-sync-beads.md) - Discrepancy detection
- [Beads Integration](../skills/conductor/references/beads-integration.md) - Point 13
