# Design: State Files Creation in Phase 1 of /conductor-newtrack

## Problem Statement

Tracks can be left in inconsistent state when state files (metadata.json, .track-progress.json, .fb-progress.json) are not created. This happens when:
1. Workflow is interrupted mid-execution
2. Agent skips steps or uses `bd create` directly
3. Manual track creation bypasses workflow

**Root cause discovered:** In thread `T-019b4c0b-e9ae-7650-b2f8-0845df48b214`, the agent:
- Created spec.md and plan.md
- Used `bd create` directly instead of file-beads subagent
- Never created metadata.json, .track-progress.json, .fb-progress.json

## Solution

Move creation of ALL 3 state files to Phase 1.3 of `/conductor-newtrack`, BEFORE any other operations.

### Before (Problematic Order)

```
Phase 2.4 Step 6:  Create directory
Phase 2.4 Step 8:  Create metadata.json      ← TOO LATE
Phase 2.4 Step 9:  Create .track-progress.json ← TOO LATE
Phase 2.4 Step 10: Write spec.md + plan.md
Phase 3:           file-beads → .fb-progress.json ← SEPARATE PHASE
```

### After (Fixed Order)

```
Phase 1.3 (FIRST):
├── Create directory
├── Create metadata.json (status: initializing)
├── Create .track-progress.json (status: initializing)
├── Create .fb-progress.json (status: pending)
└── CHECKPOINT: Verify all exist

Phase 2.4 (After spec/plan):
├── UPDATE metadata.json (status: planned)
└── UPDATE .track-progress.json (status: plan_done)

Phase 3 (file-beads):
├── VALIDATE all state files exist (HALT if missing)
└── UPDATE .fb-progress.json (status: complete)
```

## Key Design Decisions

### 1. Create All 3 State Files Together

Even `.fb-progress.json` is created upfront with `status: "pending"` instead of waiting for file-beads phase. This ensures:
- Consistent state from the start
- file-beads can validate and UPDATE rather than CREATE
- Resume capability works correctly

### 2. Validate Before File-Beads

file-beads skill now validates all state files exist before proceeding:
- metadata.json
- .track-progress.json
- .fb-progress.json
- plan.md

If any missing: HALT with clear message.

### 3. Auto-Fix track_id Mismatch

If `track_id` in any state file doesn't match directory name:
- Auto-fix: update file to match directory name
- Directory name is source of truth

### 4. spec.md and plan.md Must Exist Together

Invalid states:
- spec.md without plan.md → HALT
- plan.md without spec.md → HALT

Valid states:
- design.md only (ds phase complete)
- design.md + spec.md + plan.md (full workflow)
- spec.md + plan.md without design.md (cn without ds)

### 5. No Auto-Repair of Corrupted Files

If JSON is corrupted:
- HALT with error message
- Do NOT attempt to auto-repair
- User must manually fix or delete

## Files Modified

| File | Change |
|------|--------|
| `commands/conductor/newTrack.toml` | Add Phase 1.3 for state file creation |
| `skills/file-beads/SKILL.md` | Add validation of required state files |
| `skills/conductor/SKILL.md` | Update documentation |

## Stress Test Results

| Edge Case | Handling |
|-----------|----------|
| Empty track dir | Skip (warn) |
| spec.md XOR plan.md | HALT |
| Corrupted JSON | HALT |
| track_id mismatch | Auto-fix |
| Concurrent access | Atomic writes |
| Read-only filesystem | Warn, continue |
| Existing broken tracks | Re-run cn to repair |

## Acceptance Criteria

- [ ] All 3 state files created in Phase 1.3 of cn
- [ ] file-beads validates state files exist before proceeding
- [ ] track_id mismatch is auto-fixed
- [ ] Corrupted JSON causes HALT
- [ ] spec.md XOR plan.md causes HALT
- [ ] Existing broken tracks can be repaired by re-running cn

---

*Design Session: 2024-12-24*
*Thread: T-019b4c5e-39ff-7275-91b6-14855ba2492e*
