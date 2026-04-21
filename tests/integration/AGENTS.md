# Integration Tests

Use this file with the parent [AGENTS.md](../AGENTS.md). `tests/integration/` verifies CLI flows in real temp repos rather than mocked in-memory fixtures.

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Mission lifecycle flows | `features/mission/` | Temp repos + real `.maestro` state |
| Task loop flows | `features/task/` | Real JSONL/task-state assertions |
| Shared CLI helpers | `../helpers/run-cli.ts` | Source-run wrapper |

## CONVENTIONS
- Prefer `mkdtemp`, `git init`, and real filesystem assertions over mocked stores.
- Assert on written `.maestro` state when the command contract is persistence-oriented.
- Use source-run helpers here; compiled-binary checks belong in `../e2e/`.

## ANTI-PATTERNS
- Mocking away the storage or repo boundary you are trying to verify.
- Depending on ambient session env when the scenario is supposed to be clean.
- Using integration tests as a substitute for compiled-binary validation.

<!-- AGENTS-HIERARCHY:START -->
## AGENTS Hierarchy
Parent:
- [../AGENTS.md](../AGENTS.md)

Children:
- none

Managed by `init-deep`. Edit outside this block.
<!-- AGENTS-HIERARCHY:END -->
