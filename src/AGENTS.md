# Source Tree

Use this file with the repo-root [AGENTS.md](../AGENTS.md). `src/` is feature-first and keeps CLI plumbing, generic utilities, and Mission Control as separate seams.

## STRUCTURE
```text
src/
├── features/    # bounded contexts
├── infra/       # CLI plumbing and shared adapters
├── shared/      # generic utilities only
├── tui/         # Mission Control projection + rendering
├── index.ts     # commander root
└── services.ts  # composition root
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Add or inspect a user command | `index.ts`, `features/*/index.ts`, `infra/commands/` | Keep registration thin |
| Wire dependencies | `services.ts`, `features/*/services.ts` | Composition only |
| Change product behavior | `features/` | Owning feature first |
| Change generic filesystem/shell/yaml helpers | `shared/` | No product-domain logic here |
| Change Mission Control | `tui/`, `infra/commands/mission-control.command.ts` | Read `tui/README.md` first |

## CONVENTIONS
- Cross-feature imports go through `@/features/<name>` only.
- Keep feature logic in the owning feature, plumbing in `infra/`, and generic helpers in `shared/`.
- `src/index.ts` and `src/services.ts` stay thin.
- Mission Control snapshot builders remain read-only; recovery and workflow mutation stay out of `buildSnapshot()` and `buildHomeSnapshot()`.
- `skills/built-in/` syncs into `src/infra/domain/built-in-skill-templates.ts`; do not hand-edit the generated file from inside `src/`.

## COMMON CHECKS
- `bun run build`
- `bun run typecheck`
- `bun run check:boundaries`
- For TUI work:
  - `./dist/maestro mission-control --preview --size 120x40 --format plain`
  - `./dist/maestro mission-control --render-check --size 120x40`

## ANTI-PATTERNS
- Deep imports into another feature's internal folders.
- Feature logic in the composition root.
- Domain logic in `shared/`.

<!-- AGENTS-HIERARCHY:START -->
## AGENTS Hierarchy
Parent:
- [../AGENTS.md](../AGENTS.md)

Children:
- [features/AGENTS.md](features/AGENTS.md)
- [infra/AGENTS.md](infra/AGENTS.md)
- [shared/AGENTS.md](shared/AGENTS.md)
- [tui/AGENTS.md](tui/AGENTS.md)

Managed by `init-deep`. Edit outside this block.
<!-- AGENTS-HIERARCHY:END -->
