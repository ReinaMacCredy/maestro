# LEARNINGS: Validation Gates

Extracted from track `validation-gates_20251229`.

## Summary

Added 5 validation gates to Maestro lifecycle to verify design alignment and implementation completeness at key checkpoints.

## Key Decisions

| Decision | Reasoning |
|----------|-----------|
| humanlayer format for gates | Consistent structure: Initial Setup → 3-step Process → Guidelines → Checklist |
| LEDGER integration | Central state tracking survives compaction |
| SPEED=WARN, FULL=HALT for critical gates | Balance workflow speed vs quality enforcement |
| Max 2 retries | Prevent infinite loops while allowing self-correction |

## Patterns Discovered

- **5 Validation Gates:** design (DELIVER) → spec (newtrack) → plan-structure (newtrack) → plan-execution (TDD) → completion (finish)
- **Gate Behavior Matrix:** SPEED mode = all WARN; FULL mode = design/plan-execution/completion HALT + retry (max 2)
- **LEDGER Validation State:** Track gates_passed, current_gate, retries, last_failure in frontmatter
- **Humanlayer Format:** Gates use Initial Setup → 3-step Validation Process → Guidelines → Checklist → LEDGER Integration

## Commands

(None new - uses existing bd commands)

## Gotchas

- Validation gates are advisory in SPEED mode (never block)
- Missing source files (product.md, tech-stack.md) WARN + skip that check, don't fail entire validation
- plan.md task checkboxes `- [ ]` become `- [x]` when complete
- Aggregate all warnings in final report, don't fail on first warning

## Files Created

- `skills/conductor/references/validation/lifecycle.md` - Gate routing
- `skills/conductor/references/validation/shared/validate-design.md` (203 lines)
- `skills/conductor/references/validation/shared/validate-spec.md` (245 lines)
- `skills/conductor/references/validation/shared/validate-plan-structure.md` (237 lines)
- `skills/conductor/references/validation/shared/validate-plan-execution.md` (296 lines)
- `skills/conductor/references/validation/shared/validate-completion.md` (156 lines)

## Files Modified

- `skills/conductor/references/ledger/format.md` - Added validation fields
- `skills/design/SKILL.md` - validate-design after DELIVER
- `skills/conductor/references/tdd/cycle.md` - validate-plan-execution after REFACTOR
- `skills/conductor/references/workflows/newtrack.md` - validate-spec + validate-plan-structure
- `skills/conductor/references/finish-workflow.md` - validate-completion in Phase 0
- `conductor/CODEMAPS/overview.md` - Validation Gates section
- `conductor/AGENTS.md` - Added patterns
