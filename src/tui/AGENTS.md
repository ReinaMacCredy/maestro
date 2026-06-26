# Mission Control TUI

Use this file with the parent [AGENTS.md](../AGENTS.md) and read `README.md` here before major changes. `src/tui/` is the projection/render layer for Mission Control.

## STRUCTURE
- `state/` for snapshot types, snapshot building, and reducer logic
- `app/` for preview-state and application wiring
- `opentui/` for interactive loop, preview rendering, and components
- `shared/` and `lib/` for TUI-only helpers

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Displayed data | `state/snapshot.ts`, `state/types.ts` | Read-model first |
| Keyboard/modal behavior | `state/reducer.ts` | Reducer owns UI state |
| Preview routing | `app/preview-state.ts` | Deterministic preview state |
| Interactive actions | `opentui/app/interactive.tsx` | Writes belong here, not in snapshot building |
| Screen layout | `opentui/components/` | Keep presentation thin |

## CONVENTIONS
- `buildSnapshot()` and `buildHomeSnapshot()` stay read-only except for the explicitly gated reply-ingest path.
- Preview and render-check flows should remain deterministic and agent-friendly with explicit `--size`.
- Put derived display fields in the snapshot layer or builders, not ad hoc component state.

## ANTI-PATTERNS
- Hiding writes or recovery in snapshot construction or preview rendering.
- Domain logic drifting into screen components.
- Replacing reducer-driven UI state with ad hoc mutable component state.

<!-- AGENTS-HIERARCHY:START -->
## AGENTS Hierarchy
Parent:
- [../AGENTS.md](../AGENTS.md)

Children:
- none

Managed by `init-deep`. Edit outside this block.
<!-- AGENTS-HIERARCHY:END -->
