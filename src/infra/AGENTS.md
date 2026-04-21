# Infra

Use this file with the parent [AGENTS.md](../AGENTS.md). `src/infra/` owns shared CLI plumbing, adapters, config/git surfaces, and sanctioned cross-feature command seams.

## STRUCTURE
- `commands/` for top-level handlers such as init, doctor, install, update, uninstall, and mission-control
- `usecases/` for shared operational flows
- `adapters/` and `ports/` for config, git, and filesystem edges
- `domain/` for infra-only types and templates

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Command behavior | `commands/` | Delegate to use cases/services, keep handlers thin |
| Install/update flows | `commands/*install*`, `usecases/install-*` | User-facing binary lifecycle |
| Mission Control entry | `commands/mission-control.command.ts` | Read-only dashboard seam |
| Shared templates/config types | `domain/` | Includes generated built-in skill embed target |

## CONVENTIONS
- Infra owns plumbing and cross-feature orchestration, not feature business rules.
- `mission-control.command.ts` is a sanctioned cross-feature read-only seam.
- Preview, JSON, and render-check paths stay inspection-only.

## ANTI-PATTERNS
- Feature-owned domain logic in infra.
- Hidden writes in read-only command paths.
- Hand-editing generated template output in `domain/`.

<!-- AGENTS-HIERARCHY:START -->
## AGENTS Hierarchy
Parent:
- [../AGENTS.md](../AGENTS.md)

Children:
- none

Managed by `init-deep`. Edit outside this block.
<!-- AGENTS-HIERARCHY:END -->
