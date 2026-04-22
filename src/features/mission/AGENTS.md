# Mission Feature

Use with the parent [AGENTS.md](../AGENTS.md). `src/features/mission/` is the exception to standard feature layout: it contains nested `feature/`, `validation/`, and `checkpoint/` subtrees under one public surface.

## STRUCTURE
```text
mission/
├── feature/       # Feature sub-domain (assignments, prompts, replies)
├── validation/    # Assertion sub-domain (pass/fail/blocked/waived)
├── checkpoint/    # Snapshot save/restore sub-domain
├── commands/
├── usecases/
├── domain/
├── ports/
├── adapters/
└── index.ts       # Aggregates all subtrees
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Mission lifecycle | `domain/mission-validators.ts`, `usecases/` | `draft` → `approved` → `executing` → `sealed` |
| Feature assignments | `feature/` | Agent type, verification steps, dependencies |
| Assertions | `validation/` | Tied to features, updated via `validate` command |
| Checkpoints | `checkpoint/` | Timestamped mission snapshots |
| Mission Control read model | `src/tui/state/snapshot.ts` | Not in this feature; consumes mission state |

## CONVENTIONS
- `mission/` aggregates `feature/`, `validation/`, and `checkpoint/` through its `index.ts`.
- Mission-scoped artifacts live under `.maestro/missions/<mission-id>/`.
- `mission create --file` expects a JSON plan with milestones and features.

## ANTI-PATTERNS
- Deep-importing from `mission/feature/` or `mission/validation/` instead of through `@/features/mission`.
- Adding write logic to Mission Control snapshot paths (which should stay read-only).

<!-- AGENTS-HIERARCHY:START -->
## AGENTS Hierarchy
Parent:
- [../AGENTS.md](../AGENTS.md)

Children:
- none

Managed by `init-deep`. Edit outside this block.
<!-- AGENTS-HIERARCHY:END -->
