# Spec: DS-PL Integration

## Overview

Integrate Planning Pipeline (`pl`) into Design Session (`ds`) so that after DELIVER phase completes with Oracle approval, `pl` runs automatically.

## Requirements

### FR1: Auto-Trigger in FULL Mode
- After `ds` DELIVER phase (CP4) with `APPROVED` verdict
- In FULL mode only
- Trigger full 6-phase `pl` pipeline
- Display transition message: "✅ Design approved. Transitioning to Planning Pipeline..."

### FR2: SPEED Mode Behavior
- In SPEED mode, do NOT auto-trigger
- Display suggestion: "Design complete. Run `cn` to create spec + plan."

### FR3: Standalone `pl` Unchanged
- `pl` command continues to work independently
- No mini-DISCOVER added
- Full 6-phase execution as before

### FR4: Failure Recovery
- Update `metadata.json` with planning state per phase
- On failure, show which phase failed
- Allow resume via `cn` command

### FR5: Output Structure
- `design.md` - Updated with pl phases (Sections 2, 3, 5, 6)
- `plan.md` - Created with Track Assignments
- `.beads/` - Filed beads with dependencies

## Technical Spec

### Files to Modify

| File | Change |
|------|--------|
| `design/SKILL.md` | Add step 7 for pl auto-trigger |
| `design/references/double-diamond.md` | Add Post-DELIVER section |
| `maestro-core/references/routing-table.md` | Update ds output description |

### Trigger Logic (Pseudocode)

```
after_cp4_complete(verdict, mode):
  if mode == FULL and verdict == APPROVED:
    display("✅ Design approved. Transitioning to Planning Pipeline...")
    run_pl_pipeline(
      track_dir=current_track,
      design_file=design.md,
      phases=[1,2,3,4,5,6]
    )
  elif mode == SPEED:
    display("Design complete. Run `cn` to create spec + plan.")
  else:  # NEEDS_REVISION
    display("Design needs revision. Fix issues before planning.")
```

### Metadata State Updates

```json
{
  "planning": {
    "state": "discovery|synthesized|verified|decomposed|validated|track_planned",
    "phases_completed": ["discovery", "synthesis"],
    "current_phase": "verification",
    "triggered_by": "ds-auto"
  }
}
```

## Acceptance Criteria

- [ ] `ds` FULL + APPROVED → auto `pl` (6 phases)
- [ ] `ds` SPEED → suggest `cn` only
- [ ] `pl` standalone unchanged
- [ ] Transition message shown
- [ ] `metadata.json` tracks state
- [ ] Failed phase shown on error
- [ ] Resume via `cn` works

## Out of Scope

- Changes to pl phase logic
- Changes to Double Diamond phases
- Mini-DISCOVER for standalone pl
