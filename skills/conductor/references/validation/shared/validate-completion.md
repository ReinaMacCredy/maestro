---
description: "Validate track completion readiness, verify all beads closed, identify blockers"
---

# Gate 5: Validate Completion

Validates that a track is ready for `/conductor-finish`. All implementation work must be complete, all beads closed, and the repository in a clean state.

## Initial Setup

### Context Determination

1. **Track Location**
   - From LEDGER.md `bound_track` if in active session
   - From provided path argument
   - Auto-detect from current directory

2. **Gather Completion Evidence**
   ```bash
   # Beads status
   bd list --status=open --json
   bd list --status=in_progress --json
   bd list --status=closed --json | jq 'length'

   # Plan status
   grep -E '^\s*- \[(x| )\]' conductor/tracks/<track-id>/plan.md

   # Git status
   git status --porcelain
   git log origin/main..HEAD --oneline

   # Documentation files
   ls -la README.md CHANGELOG.md docs/
   ```

## Validation Process

### Step 1: Context Discovery

- Read `conductor/sessions/active/LEDGER.md` for bound track
- Locate `conductor/tracks/<track-id>/plan.md`
- Identify track metadata from `metadata.json`

### Step 2: Systematic Validation

**Beads Check:**
- Count open beads (must be 0)
- Count in_progress beads (must be 0)
- Verify all expected beads are closed

**Plan Check:**
- All phases marked `[x]`
- No incomplete tasks `[ ]`
- Epic completion verified

**Git Check:**
- No uncommitted changes
- No unpushed commits
- Clean working directory

**Documentation Check:**
- README updated with new features
- CHANGELOG updated with version notes
- API docs updated (if applicable)

### Step 3: Generate Validation Report

```markdown
### Validation Report: Completion

**Status:** ✅ READY | ❌ NOT READY

#### Beads Status
| Status | Count | IDs |
|--------|-------|-----|
| Open | 0 | - |
| In Progress | 0 | - |
| Closed | 12 | ... |

#### Plan Status
| Phase | Status | Notes |
|-------|--------|-------|
| Epic 1 | ✅ [x] | Complete |
| Epic 2 | ✅ [x] | Complete |

#### Git Status
- [ ] No uncommitted changes
- [ ] No unpushed commits

#### Documentation
- [ ] README updated
- [ ] CHANGELOG updated
- [ ] API docs updated (if applicable)

#### Blockers
- Blocker 1 (if any)

#### Ready for Archive?
✅ Yes / ❌ No (reason)
```

## Important Guidelines

1. **All beads must be closed** - No open or in_progress beads allowed
2. **All phases must be [x]** - Every task in plan.md marked complete
3. **Clean git state** - No uncommitted changes, no unpushed commits
4. **Documentation matters** - README, CHANGELOG must reflect work done

## Validation Checklist

Before approving for `/conductor-finish`:

- [ ] No open beads
- [ ] No in_progress beads
- [ ] All plan phases marked `[x]`
- [ ] No uncommitted changes
- [ ] No unpushed commits
- [ ] Documentation updated
- [ ] Ready for `/conductor-finish`

## LEDGER Integration

When validation completes:

```yaml
# Update LEDGER.md
validation_gate: completion
validation_status: READY | NOT_READY
blockers:
  - "description of blocker"
last_validated: 2025-12-29T10:00:00Z
```

## Relationship to Other Commands

```
All implementation complete
        ↓
All beads closed (bd close)
        ↓
validate-completion (Gate 5)
        ↓
/conductor-finish
        ↓
Archive track
```

**Prerequisites:**
- All implementation tasks completed
- All beads in closed status
- Git repository clean and pushed

**Enables:**
- `/conductor-finish` - Complete and archive the track
- Track archival to `conductor/archive/`
- Session handoff generation
