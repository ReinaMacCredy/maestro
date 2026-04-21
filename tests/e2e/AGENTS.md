# Compiled E2E Tests

Use this file with the parent [AGENTS.md](../AGENTS.md). `tests/e2e/` is for compiled `./dist/maestro` behavior, not source-run behavior.

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Compiled runner | `../helpers/run-compiled-cli.ts` | Builds/caches `dist/maestro` per process |
| Mission Control compiled checks | `mission-control-e2e.test.ts` | Preview + render-check surface |
| Task/session compiled checks | `task-compiled-e2e.test.ts`, related files | Watch ambient env carefully |

## CONVENTIONS
- Refresh the compiled binary before trusting this surface.
- Assert the compiled CLI contract, not the installed `maestro` on `PATH`, unless the test explicitly targets the installed binary.
- Scrub `CLAUDECODE`, `CODEX_THREAD_ID`, and similar env when asserting no-session behavior.

## ANTI-PATTERNS
- Using source-run helpers here.
- Assuming the installed `maestro` binary is the same artifact as `./dist/maestro`.
- Leaving ambient session env in place for negative-path assertions.

<!-- AGENTS-HIERARCHY:START -->
## AGENTS Hierarchy
Parent:
- [../AGENTS.md](../AGENTS.md)

Children:
- none

Managed by `init-deep`. Edit outside this block.
<!-- AGENTS-HIERARCHY:END -->
