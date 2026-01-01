# Design: Orchestrator Mode Test

## Goal

Validate the new Light/Full mode selection and pre-registration improvements in the orchestrator skill.

## What We're Testing

1. **Mode Selection** - Auto-select LIGHT mode (no cross-deps, simple tasks)
2. **Parallel Execution** - 2 workers run simultaneously via Task()
3. **Task Return Fallback** - Collect results via Task() return values (not Agent Mail)
4. **Bead Lifecycle** - Workers claim/close beads via bd CLI

## Track Assignments

| Track | Agent | Task | File Scope |
|-------|-------|------|------------|
| 1 | BlueStar | Count files in skills/ | skills/** |
| 2 | GreenMountain | Count files in conductor/ | conductor/** |

## Expected Behavior

- Mode auto-selects to LIGHT (no cross-track deps, estimated <10 min)
- Workers execute via simplified 3-step protocol (no Agent Mail)
- Each worker returns structured result:
  ```json
  {
    "status": "SUCCEEDED",
    "files_changed": [],
    "key_decisions": [{"decision": "Counted X files", "rationale": "..."}],
    "issues": [],
    "beads_closed": ["<bead-id>"]
  }
  ```
- Orchestrator collects results and closes epic

## Success Criteria

- [ ] Both workers complete successfully
- [ ] No Agent Mail errors (Light mode skips it)
- [ ] Beads closed via bd CLI
- [ ] Results aggregated by orchestrator
