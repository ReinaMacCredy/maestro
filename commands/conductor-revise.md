---
description: Update spec/plan when implementation reveals issues
---

# Conductor Revise

Update specifications and plans when implementation reveals issues, requirements change, or scope adjustments are needed.

## 1. Identify Track
- Find active track (marked `[~]` in tracks.md)
- If no active track, ask which track to revise

## 2. Determine Revision Type
Ask what needs revision:
1. **Spec** - Requirements changed or misunderstood
2. **Plan** - Tasks need to be added, removed, or modified
3. **Both** - Significant scope change

## 3. Gather Context
Ask targeted questions about what was discovered and what needs to change.

## 4. Create Revision Record
Append to `conductor/tracks/<track_id>/revisions.md`:
- Revision number, date, type
- What triggered the revision
- Current phase/task when revision occurred
- Changes made (spec and/or plan)
- Rationale and impact

## 5. Update Documents
- Update `spec.md` and/or `plan.md` as needed
- Add "Last Revised" marker at top of updated files
- New tasks: `[ ]`, Removed tasks: `[-] [REMOVED: reason]`

## 6. Commit
```bash
git add conductor/tracks/<track_id>/
git commit -m "conductor(revise): Update spec/plan for <track_id>"
```

## 7. Announce
Report what was revised and suggest `/conductor-implement` to continue.
