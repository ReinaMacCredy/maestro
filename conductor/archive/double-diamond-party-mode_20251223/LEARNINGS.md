# LEARNINGS: double-diamond-party-mode_20251223

## Summary
Implemented Double Diamond design methodology with BMAD-style agent feedback (Party Mode) for /conductor-design command.

## Key Learnings

### Patterns
- **Double Diamond Phases:** DISCOVER (diverge) → DEFINE (converge) → DEVELOP (diverge) → DELIVER (converge)
- **A/P/C Checkpoints:** At each phase end: [A] Advanced (deeper analysis), [P] Party (multi-agent feedback), [C] Continue
- **Party Mode:** 2-3 agents selected based on topic from product/technical/creative categories
- **Loop-back:** [↩ Back] option returns to previous phase with context preserved

### Commands
- `ds` - Shorthand trigger for /conductor-design
- `/conductor-design [desc]` - Start Double Diamond design session

### Gotchas
- CODEMAPS loaded at session start for codebase context (if exists)
- Grounding required at phase transitions to verify decisions against codebase
- Party Mode agents reference each other's points (cross-talk pattern)

## Linked Commits
- 6dbc9e2 feat(workflow): implement Double Diamond design process and Party Mode
- 2cec399 feat(design): update mermaid diagrams with BMAD v6-style workflow
