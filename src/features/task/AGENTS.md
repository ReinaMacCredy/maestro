# Task Feature

Use with the parent [AGENTS.md](../AGENTS.md). `src/features/task/` is the daily queue system with contracts, continuations, and JSONL storage.

## STRUCTURE
```text
task/
├── commands/           # task.command.ts (main), contract.command.ts
├── usecases/           # CRUD, claim, block, ready, plan
├── usecases/contract/  # Contract use cases (new, edit, lock, amend, verdict)
├── domain/             # Task types, validators, state machines, contract types
├── ports/              # task-store.port.ts, contract-store.port.ts
├── adapters/           # jsonl-task-store.adapter.ts, fs-contract-store.adapter.ts
└── index.ts            # Public surface
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Task lifecycle | `domain/task-state.ts`, `usecases/*.usecase.ts` | `pending` → `in_progress` → `completed` |
| Contract system | `domain/contract/`, `commands/contract.command.ts` | Intent/scope/verdict machine |
| JSONL storage | `adapters/jsonl-task-store.adapter.ts` | Atomic append with locking |
| Continuations | `ports/task-store.port.ts` (summary fields) | `currentState`, `nextAction`, `keyDecisions` |
| Batch planning | `usecases/plan-tasks.usecase.ts` | Atomic batch with name resolution |

## CONVENTIONS
- Contract lifecycle: `draft` → `locked`/`amended` → `fulfilled`/`broken`, with `discarded` early exit.
- Task status: `pending`, `in_progress`, `completed`. Legacy statuses collapse on read.
- Blocking is symmetric: updating `blockedBy` auto-updates the blocked task's `blocks`.
- `task ready` returns only pending, unblocked, unassigned tasks.

## ANTI-PATTERNS
- Treating `task` and `mission` as interchangeable (they are separate systems).
- Bypassing the contract store to edit contract files directly.
- Assuming `task update --status completed` succeeds without checking blockers.

<!-- AGENTS-HIERARCHY:START -->
## AGENTS Hierarchy
Parent:
- [../AGENTS.md](../AGENTS.md)

Children:
- none

Managed by `init-deep`. Edit outside this block.
<!-- AGENTS-HIERARCHY:END -->
