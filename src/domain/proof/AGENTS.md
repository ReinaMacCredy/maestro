# Proof Agent Notes

## OVERVIEW

`src/domain/proof/` owns verification command execution, evidence collection,
freshness checks, reports, and claim matching.

## WHERE TO LOOK

| Task | Location | Notes |
| --- | --- | --- |
| Run verification commands | `commands.rs` | Reads Harness verification config. |
| Verify a task snapshot | `verify_task.rs` | Produces typed verification outcome data. |
| Store/read reports | `attempts.rs`, `restore_journal.rs` | Preserve canonical and attempt-report recovery. |
| Evaluate evidence claims | `claims.rs`, `events.rs` | Claims bind to hook-backed or artifact evidence. |
| Compute freshness/status | `stale.rs`, `proof_status.rs` | Applied/unapplied state derives from Task-owned binding. |

## CONVENTIONS

- Proof owns reports and evidence interpretation; Task owns lifecycle state and
  verification binding application.
- `operations/task_verify/` is the coordinator between Proof and Task.
- Symlink rejection, stale snapshots, rollback, and interrupted promotion are
  part of the contract, not test-only details.

## ANTI-PATTERNS (THIS PROJECT)

- Do not call Task mutation paths directly from Proof.
- Do not replace `acceptance.yaml` with generated or symlinked content.
- Do not make a failed verification move a previously verified task except
  through Task-owned lifecycle logic.
- Do not overwrite canonical `verification.json` with a stale attempt.

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
