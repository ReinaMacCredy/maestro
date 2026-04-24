# Handoff Feature

Use with the parent [AGENTS.md](../AGENTS.md). `src/features/handoff/` owns launch packets, external-agent startup prompts, pickup consumption, and task ownership transfer.

## STRUCTURE
```text
handoff/
├── commands/     # `maestro handoff` launch, pickup, list, show
├── usecases/     # prompt generation, launch, pickup, reconciliation
├── domain/       # record types, display state, project scoping
├── adapters/     # store plus Claude/Codex launch adapters
├── ports/        # handoff store and launcher boundaries
└── index.ts      # public surface for infra, bundle, and task integrations
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| CLI behavior | `commands/handoff.command.ts` | Resolves linked tasks and current pickup actor |
| Prompt content | `usecases/build-handoff-prompt.usecase.ts` | Keep receiver instructions concrete and task-scoped |
| Launch records | `adapters/handoff-store.adapter.ts`, `domain/handoff-types.ts` | Records are durable packet state |
| Pickup semantics | `usecases/pickup-handoff.usecase.ts` | Consumes packet, claims linked task, transfers contract ownership |
| Open/completed display | `domain/handoff-state.ts`, `usecases/reconcile-handoff-record.usecase.ts` | Completed linked tasks close open-looking packets |
| Project scoping | `domain/project-scope.ts` | Task-linked pickup must run from the source project unless standalone |

## CONVENTIONS
- Launch records start as `launching`, move to `launched`, `completed`, or `failed`, and pickup marks them consumed.
- `maestro handoff pickup --id <id> --json` is injected into launch prompts and should remain the first receiver action.
- Task-linked pickup changes task ownership and contract ownership together; surface warnings instead of hiding partial transfer failures.
- Standalone pickup consumes the packet without resuming a task.
- Worktree handoffs derive branch names through the git adapter; keep path and branch safety there.

## ANTI-PATTERNS
- Do not guess among multiple open handoffs; list candidates and require an id.
- Do not resume a task-linked packet from another project without `--standalone`.
- Do not write task recovery logic in list/show beyond explicit reconciliation of already-completed linked tasks.

<!-- AGENTS-HIERARCHY:START -->
## AGENTS Hierarchy
Parent:
- [../AGENTS.md](../AGENTS.md)

Children:
- none

Managed by `init-deep`. Edit outside this block.
<!-- AGENTS-HIERARCHY:END -->
