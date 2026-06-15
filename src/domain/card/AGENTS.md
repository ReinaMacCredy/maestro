# Card Domain Agent Notes

## OVERVIEW

`src/domain/card/` is the unified card persistence and query seam for features,
tasks, bugs, chores, ideas, and decisions.

## WHERE TO LOOK

| Task | Location | Notes |
| --- | --- | --- |
| Store paths, loads, CAS saves | `store.rs` | Reject unsafe ids and symlinked card dirs before writes. |
| Persisted shape and type enum | `schema.rs` | `CardType` dispatch is intentional; keep matches exhaustive. |
| Read models and ready/list filters | `query.rs` | Coarse status is derived, never stored. |
| Mutating shared fields | `edit.rs` | Preserve snapshot/CAS behavior. |
| Legacy/type payload folding | `fold.rs` | Keep envelope fields authoritative over `extra`. |
| Id lookup and suggestions | `index.rs`, `suggest.rs` | Do not bypass store validation. |

## CONVENTIONS

- The card store is the shared persistence seam; type-specific lifecycle rules
  stay in the owning domain modules.
- Dir-backed cards and entry-backed rosters must remain readable through the
  same query surfaces.
- Reads that power agent JSON contracts stay additive; do not rename existing
  fields silently.
- Keep id minting stable and path-safe.

## ANTI-PATTERNS (THIS PROJECT)

- Do not write card files without loading the snapshot used for CAS.
- Do not make CLI adapters inspect card internals instead of using card/domain
  read models.
- Do not store derived board status as a second source of truth.

## VERIFICATION

Start with `tests/card_commands_integration.rs`,
`tests/card_query_e2e.rs`, and `tests/card_namespace_integration.rs`. Broaden to
feature/task/decision tests and `tests/architecture_imports.rs` when shared
dispatch or ownership moves.

<!-- AGENTS-HIERARCHY:START -->
## AGENTS Hierarchy
Parent:
- [../AGENTS.md](../AGENTS.md)

Children:
- none

Managed by `init-deep`. Edit outside this block.
<!-- AGENTS-HIERARCHY:END -->
