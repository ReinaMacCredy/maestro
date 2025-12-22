---
description: Create spec and plan from design.md (or interactive if no design)
argument-hint: [track_id or description]
---

# Conductor New Track

Create spec and plan for: $ARGUMENTS

## 1. Verify Setup

Check these files exist:
- `conductor/product.md`
- `conductor/tech-stack.md`
- `conductor/workflow.md`

If missing, tell user to run `/conductor-setup` first.

## 2. Resolve Track ID

- **Guard:** If `$ARGUMENTS` is empty or whitespace-only, prompt user: "Please provide a track ID or description."
- Check for existing track directory: `ls -d "conductor/tracks/$ARGUMENTS" 2>/dev/null`
- If directory exists:
  - Use `$ARGUMENTS` as `track_id`
- Otherwise:
  - Treat `$ARGUMENTS` as description
  - Derive shortname from description (lowercase, underscores for spaces)
  - **Check for existing tracks with same shortname:**
    ```bash
    matches=$(ls -d conductor/tracks/${shortname}_* 2>/dev/null)
    match_count=$(echo "$matches" | grep -c .)
    ```
  - If `match_count` > 1:
    - List all matching tracks to the user
    - Prompt: "Multiple tracks match '\<shortname\>'. Please specify which one:"
    - Wait for user to select or provide full `track_id`
  - If `match_count` == 1:
    - Use the single match as `track_id`
    - Inform user: "Found existing track: \<track_id\>"
  - If no match:
    - Generate new `track_id`: `shortname_YYYYMMDD` (use today's date)

## 3. Check for Existing Design

- If `conductor/tracks/<track_id>/design.md` exists:
  - Read it completely
  - Extract: track title, type (feature/bug), requirements, constraints, success criteria
  - Treat this design as primary source of truth
  - Only ask follow-up questions if there are obvious gaps or contradictions
- If `design.md` does NOT exist:
  - Fall back to full interactive questioning (section 4b)

## 4. Generate Spec

### 4a. If using design.md:
Generate `spec.md` by structuring content from design:
- **Overview** - Summarize design's high-level intent
- **Functional Requirements** - Extract concrete behaviors and user flows
- **Acceptance Criteria** - Convert success criteria into testable bullets
- **Out of Scope** - Extract or infer non-goals

### 4b. If no design.md (fallback):
Ask 3-5 clarifying questions based on track type:

**Feature**: What does it do? Who uses it? What's the UI? What data is involved?
**Bug**: Steps to reproduce? Expected vs actual behavior? When did it start?

Generate `spec.md` with:
- Overview
- Functional Requirements
- Acceptance Criteria
- Out of Scope

Present for approval, revise if needed.

## 5. Generate Plan

Read `conductor/workflow.md` for task structure (TDD, commit strategy).
Use finalized `spec.md` (and `design.md` if present) to derive phases and tasks.

Generate `plan.md` with phases, tasks, subtasks:
```markdown
# Implementation Plan

## Phase 1: [Name]
- [ ] Task: [Description]
  - [ ] Write tests
  - [ ] Implement
- [ ] Task: Conductor - Phase Verification

## Phase 2: [Name]
...
```

Present for approval, revise if needed.

## 6. Create Track Artifacts

1. If track folder doesn't exist: `mkdir -p conductor/tracks/<track_id>/`
2. Write files:
   - `metadata.json`: `{"track_id": "...", "type": "feature|bug", "status": "new", "created_at": "...", "description": "...", "has_design": true|false}`
   - `spec.md`
   - `plan.md`

## 7. Update Tracks File

Append to `conductor/tracks.md`:
```markdown

---

## [ ] Track: [Description]
*Link: [conductor/tracks/<track_id>/](conductor/tracks/<track_id>/)*
```

## 8. Announce

"Plan approved. Say `fb` to file issues."
