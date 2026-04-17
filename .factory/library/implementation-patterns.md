# Implementation Patterns

Established code patterns to preserve while adding Mission Control.

## Command files

- Each command file exports `registerXCommand(program: Command): void`
- Commands typically:
  1. call `getServices()`
  2. invoke a pure usecase
  3. route output through `output(...)`
- Existing commands use `opts.json ?? program.opts().json`; Mission Control should add a helper for root/group/leaf `--json` inheritance rather than inventing a different output pattern

## Usecases

- Keep usecases pure async functions that receive ports explicitly
- Put orchestration logic in usecases, not in commands
- Usecases should return domain types or small DTOs; commands own formatting

## Domain validation

- Existing domain modules use Zod schemas plus small `validateX()` wrappers
- Mission Control should mirror that split: exported schemas plus typed validator helpers
- Error messages should be `MaestroError` instances with concise recovery hints

## Adapters

- Follow the current filesystem adapter pattern:
  - private path helpers
  - `ensureDir()` before writes
  - `readJson()` / `writeJson()` from `src/lib/fs.ts`
- Missing entities should return `undefined` instead of throwing inside adapters
- Validate data before writes and after reads

## Testing patterns

- Unit tests live next to the affected area (`tests/unit/domain`, `tests/unit/adapters`, `tests/unit/usecases`)
- CLI integration tests use Bun subprocesses in temp git repos
- Reuse `tests/helpers/mocks.ts` for port mocks rather than building ad hoc doubles in each test

## Current repo truths that matter

- `src/index.ts` is the main command registration point
- `src/services.ts` is the central dependency wiring point
- `.maestro/` already exists for current product state; Mission Control must extend it rather than inventing a parallel runtime namespace
- `skills/built-in/maestro%3A*` deletions are already present in the working tree; replacing that legacy surface is part of the migration
