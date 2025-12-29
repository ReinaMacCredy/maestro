# Design: Validation Gates

## Problem Statement

Maestro thiếu validation gates để verify design alignment và implementation completeness, dẫn đến gaps giữa plan và output.

## Solution

Tích hợp 5 validation gates vào Maestro lifecycle, routing qua maestro-core, logic trong conductor/references/validation/.

## Validation Gates

| Gate | Trigger | Validates |
|------|---------|-----------|
| 1. design | DELIVER complete | design vs product.md/tech-stack.md |
| 2. spec | spec.md generated | spec vs design alignment |
| 3. plan-structure | plan.md generated | tasks có acceptance criteria |
| 4. plan-execution | TDD REFACTOR complete | implementation vs plan |
| 5. completion | before /conductor-finish | all beads closed, docs updated |

## Architecture

```
conductor/references/validation/
├── lifecycle.md                      # Gate routing + LEDGER integration
├── shared/
│   ├── validate-design.md            # Gate 1
│   ├── validate-spec.md              # Gate 2
│   ├── validate-plan-structure.md    # Gate 3
│   ├── validate-plan-execution.md    # Gate 4 (humanlayer format)
│   └── validate-completion.md        # Gate 5
```

## LEDGER Integration

```yaml
validation:
  gates_passed: []
  current_gate: null
  retries: 0
  last_failure: null
```

## Behavior Matrix

| Mode | Gate Fail | Behavior |
|------|-----------|----------|
| SPEED | Any | WARN + continue |
| FULL | design/completion | HALT + retry (max 2) |
| FULL | spec/plan-structure | WARN + continue |
| FULL | plan-execution | HALT + retry (max 2) |

## Content Format

All `shared/*.md` files follow humanlayer format:
- Initial Setup (determine context, locate artifact, gather evidence)
- Validation Process (3 steps: discovery, systematic validation, report)
- Important Guidelines
- Validation Checklist
- LEDGER Integration
- Relationship to Other Commands

## Files to Create

1. `conductor/references/validation/lifecycle.md`
2. `conductor/references/validation/shared/validate-design.md`
3. `conductor/references/validation/shared/validate-spec.md`
4. `conductor/references/validation/shared/validate-plan-structure.md`
5. `conductor/references/validation/shared/validate-plan-execution.md`
6. `conductor/references/validation/shared/validate-completion.md`

## Files to Edit

1. `skills/conductor/references/ledger/format.md` - Add validation fields
2. `skills/design/SKILL.md` - Call validate-design after DELIVER
3. `skills/conductor/references/tdd/cycle.md` - Call validate-plan-execution after REFACTOR

## Success Criteria

- [ ] Design validation runs after DELIVER, blocks if misaligned (FULL mode)
- [ ] Plan validation runs after TDD, blocks if criteria unmet
- [ ] Max 2 retries before escalate to human
- [ ] Failures logged in LEDGER.md
- [ ] SPEED mode: all gates WARN only
- [ ] Missing files: WARN + continue checking others

## Risks

- Overhead: 5 gates = slower workflow → Mitigated by SPEED mode skip/warn
- Infinite loop: fail → retry → fail → Mitigated by max 2 retries

## Out of Scope

- New skill creation (use existing conductor/references/)
- Auto-fix misalignment (report only, human fix)
- Validate dependencies (use existing `bd dep`)

## References

- [humanlayer validate_plan.md](https://github.com/humanlayer/humanlayer/blob/main/.claude/commands/validate_plan.md)
- [gate.md](../../skills/conductor/references/verification/gate.md)
