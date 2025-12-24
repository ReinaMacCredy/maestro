# Consolidate Plans Directory to history/

> **SUPERSEDED (2025-12-22):** This design is obsolete. The new workflow uses:
>
> - `/conductor-design` creates `conductor/tracks/<id>/design.md` directly
> - `/conductor-newtrack` creates `spec.md` + `plan.md` in the same track folder
> - All artifacts live in `conductor/tracks/<id>/` (unified per-track location)
> - The `history/plans/` and `docs/plans/` directories are no longer used
> - Archiving moves completed tracks to `conductor/archive/<id>/`
>
> See `2025-12-20-skills-cleanup-design.md` for the updated approach.

## Goal

Unify all workflow artifacts under `history/` for consistency, so skills like `file-beads` can find plans without searching multiple locations.

## Changes

### Directory Structure

**Before:**

```
docs/plans/          # active plans
history/plans/       # archived (per global AGENTS.md)
```

**After:**

```
history/plans/           # active plans
history/plans/archive/   # completed plans (auto-moved on bd close)
```

### Files to Update

| File                                          | Change                                       |
| --------------------------------------------- | -------------------------------------------- |
| `skills/brainstorming/SKILL.md`               | `docs/plans/` → `history/plans/`             |
| `skills/plan-executor/SKILL.md`               | `docs/plans/` → `history/plans/`             |
| `skills/subagent-driven-development/SKILL.md` | `docs/plans/` → `history/plans/`             |
| `skills/beads/SKILL.md`                       | Add archive logic on `bd close`              |
| `skills/beads/file-beads/SKILL.md`            | Add `source_plan` metadata to created issues |
| `TUTORIAL.md`                                 | Update `docs/plans/` reference               |

### Auto-Archive Behavior

1. When `fb` creates issues from a plan, record `source_plan: history/plans/<name>.md` on each issue
2. When `bd close` runs:
   - Check if issue has `source_plan` metadata
   - Query all issues with same `source_plan`
   - If all are closed → move plan to `history/plans/archive/YYYY-MM-DD-<name>.md`

## Tasks

- [ ] Update `skills/brainstorming/SKILL.md`
- [ ] Update `skills/plan-executor/SKILL.md`
- [ ] Update `skills/subagent-driven-development/SKILL.md`
- [ ] Update `skills/beads/file-beads/SKILL.md` - add source_plan tracking
- [ ] Update `skills/beads/SKILL.md` - add archive logic to bd close
- [ ] Update `TUTORIAL.md`
- [ ] Create `history/plans/` and `history/plans/archive/` directories
