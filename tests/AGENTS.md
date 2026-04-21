# Test Suite

Use this file with the repo-root [AGENTS.md](../AGENTS.md). The suite is split by execution surface, not by generic test taxonomy.

## STRUCTURE
- `unit/` mirrors source units, scripts, and TUI state.
- `integration/` exercises CLI flows in temp repos with real `.maestro` state.
- `e2e/` hits compiled `./dist/maestro`.
- `helpers/` owns the source-run and compiled-run wrappers.

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Source-run CLI behavior | `helpers/run-cli.ts`, `unit/`, `integration/` | `bun run src/index.ts` surface |
| Compiled CLI behavior | `helpers/run-compiled-cli.ts`, `e2e/` | `./dist/maestro` surface |
| TUI coverage | `unit/tui/`, `e2e/mission-control-e2e.test.ts` | No snapshot-heavy approach |
| Feature-specific behavior | `unit/features/<name>/`, `integration/features/<name>/` | Mirror the owning source boundary |

## CONVENTIONS
- Mock external dependencies, not internal modules.
- Prefer explicit stdout/stderr/exit-code and on-disk artifact assertions over snapshots.
- Use `helpers/run-cli.ts` for source-run flows and `helpers/run-compiled-cli.ts` for compiled-binary flows.
- Refresh `./dist/maestro` with `bun run build` before relying on compiled-binary tests.

## GOTCHAS
- `tests/helpers/command-runner.ts` inherits `process.env`; scrub session-related env vars for no-session assertions.
- Some session-source-path coverage is environment-sensitive; verify the expected local artifact before treating failures as product regressions.

<!-- AGENTS-HIERARCHY:START -->
## AGENTS Hierarchy
Parent:
- [../AGENTS.md](../AGENTS.md)

Children:
- [integration/AGENTS.md](integration/AGENTS.md)
- [e2e/AGENTS.md](e2e/AGENTS.md)

Managed by `init-deep`. Edit outside this block.
<!-- AGENTS-HIERARCHY:END -->
