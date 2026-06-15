# Operations Agent Notes

## OVERVIEW

`src/operations/` coordinates workflows that legitimately cross domain
boundaries.

## WHERE TO LOOK

| Task | Location | Notes |
| --- | --- | --- |
| Initialize a repo | `init/` | Creates Harness-adjacent startup artifacts without hidden daemon state. |
| Verify a task | `task_verify/` | Coordinates Proof report production and Task outcome application. |
| Update binary/resources | `update/` | Keeps passive check, update, replacement, and schema drift separate. |
| Sync repo-local artifacts | `sync/` | Keep dry-run/check behavior and extraction boundaries explicit. |
| Harness proposals | `harness/` | Read source artifacts; backlog refresh must not apply protocol changes. |

## CONVENTIONS

- Operations may depend on domain facades and foundation helpers, not
  interfaces.
- If an operation mutates user-owned artifacts, preserve dry-run/check behavior,
  backups, rollback, and explicit intent.
- Keep operation root facades as the public entrypoint; leaf files are private
  implementation details unless documented otherwise.

## ANTI-PATTERNS (THIS PROJECT)

- Do not let operations import CLI/MCP/TUI adapters.
- Do not route Update into Harness write surfaces for passive drift checks.
- Do not add migration-style direct writes unless the target domain loader,
  backup, rollback, and compatibility story are documented and tested.

## VERIFICATION

Use the operation-specific integration tests in `TESTING.md`. Run
`tests/architecture_imports.rs` when dependency direction, facades, or safety
boundaries move.

<!-- AGENTS-HIERARCHY:START -->
## AGENTS Hierarchy
Parent:
- [../AGENTS.md](../AGENTS.md)

Children:
- none

Managed by `init-deep`. Edit outside this block.
<!-- AGENTS-HIERARCHY:END -->
