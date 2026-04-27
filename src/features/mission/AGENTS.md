# Mission Feature

Use with the parent [AGENTS.md](../AGENTS.md). `src/features/mission/` keeps a `feature/` sub-tree (its own commands/usecases) and a `reply/` sub-tree (the inbound agent reply contract). Validation, checkpoints, and assertions live directly under the feature root.

## STRUCTURE
```text
mission/
├── feature/       # Feature sub-domain (assignments, prompts) -- has its own commands/usecases
├── reply/         # Inbound agent reply ingest + write
├── commands/      # Mission, milestone, checkpoint, validate, principle, feature commands
├── usecases/      # Mission/milestone/checkpoint/validation lifecycle, mission report, principles
├── domain/
├── ports/         # mission, feature*, assertion, checkpoint, principle stores (*feature is under feature/)
├── adapters/      # FS adapters for the same set
└── index.ts       # Public surface for all of the above plus reply
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Mission lifecycle | `domain/mission-validators.ts`, `usecases/mission-lifecycle.usecase.ts` | `draft` → `approved` → `executing` → `sealed` |
| Feature assignments | `feature/` | Agent type, verification steps, dependencies |
| Assertions | `usecases/validation-lifecycle.usecase.ts` | Tied to features, updated via `validate` command |
| Checkpoints | `usecases/checkpoint-lifecycle.usecase.ts` | Timestamped mission snapshots |
| Reply ingest | `reply/` | Agent inbound contract; idempotent ingest into feature state |
| Mission Control read model | `src/tui/state/snapshot.ts` | Not in this feature; consumes mission state |

## CONVENTIONS
- `mission/` aggregates `feature/` and `reply/` through its `index.ts` alongside its own ports/adapters/usecases.
- Mission-scoped artifacts live under `.maestro/missions/<mission-id>/`.
- `mission create --file` expects a JSON plan with milestones and features.

## ANTI-PATTERNS
- Deep-importing from `mission/feature/` or `mission/reply/` (or any `mission/` internal subpath) instead of through `@/features/mission`.
- Adding write logic to Mission Control snapshot paths (which should stay read-only).

<!-- AGENTS-HIERARCHY:START -->
## AGENTS Hierarchy
Parent:
- [../AGENTS.md](../AGENTS.md)

Children:
- none

Managed by `init-deep`. Edit outside this block.
<!-- AGENTS-HIERARCHY:END -->
