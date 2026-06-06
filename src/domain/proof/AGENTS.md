# Proof Agent Notes

## OVERVIEW

`src/domain/proof/` owns verification command execution, evidence collection,
freshness checks, verification outcome data, and claim matching.

## WHERE TO LOOK

| Task | Location | Notes |
| --- | --- | --- |
| Run verification commands | `commands.rs` | Reads Harness verification config. |
| Verify a task snapshot | `verify_task.rs` | Produces typed outcome data for the Task-owned `task.yaml#verification` binding. |
| Evaluate evidence claims | `claims.rs`, `events.rs` | Claims bind to hook-backed or artifact evidence. |
| Compute freshness/status | `stale.rs`, `proof_status.rs` | Applied/unapplied state derives from Task-owned binding. |

## CONVENTIONS

- Proof owns verification outcome evaluation and evidence interpretation; Task
  owns lifecycle state and verification binding application.
- `operations/task_verify/` is the coordinator between Proof and Task.
- Symlink rejection for Task-owned files, stale snapshots, and failed Task
  writes are part of the contract, not test-only details.

## ANTI-PATTERNS (THIS PROJECT)

- Do not call Task mutation paths directly from Proof.
- Do not replace Task-owned `task.yaml` fields with generated or symlinked content.
- Do not make a failed verification move a previously verified task except
  through Task-owned lifecycle logic.
- Do not reintroduce canonical `verification.json`, `verification.attempts/`,
  or restore-journal sidecars as active proof state.

## VERIFICATION

Start with `tests/task_verify_integration.rs`; broaden to Task lifecycle tests,
Run evidence tests, and `tests/architecture_imports.rs` when boundaries move.

<!-- AGENTS-HIERARCHY:START -->
## AGENTS Hierarchy
Parent:
- [../AGENTS.md](../AGENTS.md)

Children:
- none

Managed by `init-deep`. Edit outside this block.
<!-- AGENTS-HIERARCHY:END -->
