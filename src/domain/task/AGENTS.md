# Task Domain Agent Notes

## OVERVIEW

`src/domain/task/` owns task-family lifecycle, blockers, acceptance checks,
notes, display read models, doctor reports, and verification bindings.

## WHERE TO LOOK

| Task | Location | Notes |
| --- | --- | --- |
| Public task facade | `mod.rs` | Creation, handles, notes, blockers, transition entrypoints. |
| Persisted task shape | `template.rs` | Acceptance, blockers, proof receipts, verification binding. |
| Lifecycle rules | `lifecycle.rs` | Legal transitions and task-state semantics. |
| Card-backed reads/writes | `cards.rs` | Bridge task records through the card store. |
| Blocker graph | `blockers.rs` | Task/decision/external blocker semantics. |
| Display/read models | `display.rs`, `lookup.rs` | CLI and query-facing projections. |
| Diagnostics | `doctor.rs` | Recoverable task-store health reporting. |

## CONVENTIONS

- Task owns lifecycle state and verification binding application.
- Proof produces outcomes; `operations/task_verify/` coordinates applying them
  back to Task.
- Blocker refs must stay typed; feature/idea cards are not task blockers.
- Notes append rather than rewrite history.

## ANTI-PATTERNS (THIS PROJECT)

- Do not let Proof, Feature, Run, or CLI mutate Task lifecycle directly.
- Do not swallow unreadable id-shaped card refs as generic external blockers.
- Do not bypass task/card snapshot checks for state changes.

## VERIFICATION

Start with `tests/task_commands_integration.rs`,
`tests/task_verify_integration.rs`, and `tests/task_artifacts.rs`. Broaden to
feature, run evidence, and architecture import tests when Task boundaries move.

<!-- AGENTS-HIERARCHY:START -->
## AGENTS Hierarchy
Parent:
- [../AGENTS.md](../AGENTS.md)

Children:
- none

Managed by `init-deep`. Edit outside this block.
<!-- AGENTS-HIERARCHY:END -->
