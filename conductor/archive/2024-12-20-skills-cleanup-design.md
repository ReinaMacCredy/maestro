# Skills Cleanup Design

> **SUPERSEDED (2025-12-22):** This design has been implemented with additional changes:
> - `brainstorming` skill REMOVED - merged into conductor as `/conductor-design`
> - `execution-workflow` skill REMOVED - merged into conductor as `/conductor-implement`
> - design.md now lives at `conductor/tracks/<id>/design.md` (single location per track)
> - `conductor/plans/` directory is OBSOLETE - designs go directly into tracks
> - The `bs` and `ct` triggers are deprecated in favor of `/conductor-design` and `/conductor-implement`

**Date:** 2025-12-20
**Status:** Implemented (with modifications)

## Overview

Consolidate and clean up skills in my-workflow plugin to reduce duplication and unify save locations.

## Decisions

### Skills to Remove (4)

| Skill | Reason |
|-------|--------|
| `plan-executor` | Overlaps with `execution-workflow` via beads |
| `condition-based-waiting` | Content already in `systematic-debugging/` |
| `systematic-debugging` | Not linked in workflow (standalone) |

### Skills to Keep (14)

**Core Workflow:**
- `conductor` - Planning with specs/tracks
- `beads` + `file-beads` + `review-beads` - Issue tracking
- `execution-workflow` - Beads-based task claiming
- `finishing-a-development-branch` - End-of-work menu

**Development Practices:**
- `test-driven-development` - TDD workflow
- `verification-before-completion` - Run checks before done

**Agent Coordination:**
- `dispatching-parallel-agents` - Parallel subagents
- `subagent-driven-development` - Plan execution via Task()

**Meta/Utility:**
- `using-superpowers` - Session start skill enforcement
- `using-git-worktrees` - Isolated workspaces
- `writing-skills` - Create/edit skills
- `sharing-skills` - Contribute upstream
- `brainstorming` - Design exploration
- `doc-sync` - Sync AGENTS.md
- `codemaps` - Architecture docs

### Unified Save Location

**Before:**
```
history/
├── plans/      ← brainstorming saves here
├── archive/    ← completed work

conductor/
├── tracks/     ← conductor saves here
```

**After:**
```
conductor/
├── product.md
├── tech-stack.md
├── workflow.md
├── tracks.md
├── plans/               ← brainstorming designs (inbox)
│   └── YYYY-MM-DD-<topic>-design.md
├── tracks/              ← active work (promoted from plans/)
│   └── <id>/
│       ├── design.md    ← copied/moved from plans/
│       ├── spec.md
│       ├── plan.md
│       └── metadata.json
└── archive/             ← completed tracks
    └── <id>/
```

**Flow:**
```
conductor/plans/         → brainstorming designs (inbox)
    ↓
conductor/tracks/<id>/   → active work (design promoted to track)
    ↓
conductor/archive/<id>/  → completed work
```

### Execution Handoff Pattern

Session-based handoff via beads:

```
SESSION 1 (Planning)              SESSION 2 (Execution)
─────────────────────             ────────────────────────
conductor/brainstorm              User pastes HANDOFF block
    ↓                                  ↓
creates design + spec + plan      execution-workflow loads epic
    ↓                                  ↓
fb → creates beads epic           finds linked plan in conductor/tracks/
    ↓                                  ↓
rb → review/refine issues         claims tasks → TDD → verify
    ↓                                  ↓
outputs HANDOFF block             rb → verify threads + doc-sync + archive
```

### Handoff Block Format

At end of planning session:

**Step 1: Save handoff state to beads**
```bash
bd update <epic-id> --notes "HANDOFF_READY: true. PLAN: <plan-path>"
```

**Step 2: Output to user**
```markdown
## HANDOFF

**Command:** `Start epic <epic-id>`
**Epic:** <epic-id>
**Plan:** <plan-path>
**Ready issues:** <count>
**First task:** <first-issue-id> - <title>

Copy the command above to start a new session.
```

### Session 2: Execution Start

User pastes: `Start epic <epic-id>`

Agent does:
```bash
# 1. Load epic
bd show <epic-id> --json

# 2. Parse notes for plan location
# → finds: PLAN: conductor/tracks/<id>/plan.md

# 3. Read the plan
cat <plan-path>

# 4. Get ready tasks
bd ready --json

# 5. Begin execution-workflow
```

## Files to Update

| File | Change |
|------|--------|
| `brainstorming/SKILL.md` | `history/plans/` → `conductor/tracks/<id>/design.md` |
| `subagent-driven-development/SKILL.md` | `history/plans/` → `conductor/tracks/` |
| `beads/SKILL.md` | `history/plans/` → `conductor/plans/`, `archive/` → `conductor/archive/` |
| `TUTORIAL.md` | `history/plans/` → `conductor/tracks/` |

## Files to Delete

- `skills/plan-executor/` (directory)
- `skills/condition-based-waiting/` (directory)
- `skills/systematic-debugging/` (directory)

## Update `review-beads` (rb)

Add workflow integration checks:

### New Checks to Add

1. **Thread URL Verification**
   - For closed/completed issues, verify `THREAD:` exists in notes
   - Flag issues missing thread URLs as incomplete

2. **Doc-Sync Integration**
   - After review, prompt: "Run doc-sync to update AGENTS.md from threads?"
   - Or auto-trigger if all issues in epic are closed

3. **Auto-Archive Prompt**
   - When epic is complete, prompt to archive plan to `conductor/archive/`
   - Move completed track from `conductor/tracks/<id>/` to `conductor/archive/<id>/`

### New Step 7: Workflow Integration Check

```markdown
## Step 7: Workflow Integration Check

For closed/completed issues:

1. **Thread URL check:**
   ```bash
   bd show <id> --json | grep "THREAD:"
   ```
   - If missing: Flag as "Missing thread URL - cannot sync to docs"

2. **Doc-sync readiness:**
   - If all issues in epic are closed with thread URLs
   - Prompt: "Ready for doc-sync? (y/n)"
   - If yes: Trigger doc-sync workflow

3. **Archive prompt:**
   - If epic complete: "Archive to conductor/archive/? (y/n)"
```
