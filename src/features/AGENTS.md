# Features

Use this file with the parent [AGENTS.md](../AGENTS.md). `src/features/` owns bounded contexts and their public surfaces.

## STRUCTURE
- Typical feature layout: `commands/`, `usecases/`, `domain/`, `ports/`, `adapters/`, `services.ts`, `index.ts`.
- `mission/` is the exception: it contains nested `feature/`, `validation/`, and `checkpoint/` subtrees under one public surface.

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Public imports | `<feature>/index.ts` | Only surface siblings should consume |
| Local wiring | `<feature>/services.ts` | Feature-local factory |
| Storage and I/O | `<feature>/adapters/`, `<feature>/ports/` | Keep side effects explicit |
| CLI command shape | `<feature>/commands/` | Only for features with user-facing commands |

## CONVENTIONS
- Cross-feature imports go through `@/features/<name>` only.
- Keep feature-owned behavior inside the owning feature.
- `agent` may depend on `mission`, `memory`, and `handoff` through public surfaces only.
- `bundle` may depend on `mission`, `reply`, `handoff`, and `session` through public surfaces only.

## ANTI-PATTERNS
- Deep imports into another feature's internal folders.
- Moving plumbing into features when it belongs in `infra/`.
- Moving generic helpers into features when they belong in `shared/`.

<!-- AGENTS-HIERARCHY:START -->
## AGENTS Hierarchy
Parent:
- [../AGENTS.md](../AGENTS.md)

Children:
- [mission/AGENTS.md](mission/AGENTS.md)
- [task/AGENTS.md](task/AGENTS.md)

Managed by `init-deep`. Edit outside this block.
<!-- AGENTS-HIERARCHY:END -->
