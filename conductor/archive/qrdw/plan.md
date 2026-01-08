# Plan: Standalone pl Pipeline Integration

Epic: `my-workflow:3-qrdw`

## Track Assignments

| Track | Agent | Beads | Files | Wave |
|-------|-------|-------|-------|------|
| A | BlueLake | .1, .2 | skills/designing/SKILL.md, skills/designing/references/planning/pipeline.md | 1→2 |
| B | GreenCastle | .10, .9 | skills/maestro-core/references/routing-table.md, skills/designing/references/pipeline.md | 1→2 |
| C | RedMountain | .3, .4 | skills/designing/references/planning/discovery-agents.md, synthesis.md | 3 |
| D | PurpleRiver | .5, .6 | skills/designing/references/planning/spikes.md, decomposition.md | 3 |
| E | OrangeStar | .7, .8 | skills/designing/references/planning/validation.md, track-planning.md | 3 |

## Execution Waves

```
Wave 1: Track A (.1) + Track B (.10) [parallel]
           ↓
Wave 2: Track A (.2) + Track B (.9) [parallel, after .1 completes]
           ↓
Wave 3: Track C + Track D + Track E [parallel, after .2 completes]
```

## File Scope Matrix

| Bead | Primary Files | No Overlap |
|------|---------------|------------|
| .1 | skills/designing/SKILL.md | ✓ |
| .10 | skills/maestro-core/references/routing-table.md | ✓ |
| .2 | skills/designing/references/planning/pipeline.md | ✓ |
| .9 | skills/designing/references/pipeline.md | ✓ |
| .3 | skills/designing/references/planning/discovery-agents.md | ✓ |
| .4 | skills/designing/references/planning/synthesis.md | ✓ |
| .5 | skills/designing/references/planning/spikes.md | ✓ |
| .6 | skills/designing/references/planning/decomposition.md | ✓ |
| .7 | skills/designing/references/planning/validation.md | ✓ |
| .8 | skills/designing/references/planning/track-planning.md | ✓ |

## Verification

- [ ] All beads have clear file scope (no overlap)
- [ ] Dependencies wired via `bd dep add`
- [ ] Wave execution order respects dependencies
