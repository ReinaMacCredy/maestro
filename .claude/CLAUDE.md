# Maestro CLI

Read `AGENTS.md` at the project root for full conventions (types, naming, async, testing, release, Mission Control contracts, agent-optimized TUI preview).

## Key paths
- Source entry: `src/index.ts`
- Composition root: `src/services.ts`
- Features: `src/features/<name>/` (ratchet, handoff, notes, graph, session, memory, mission, worker) with `commands/ usecases/ domain/ ports/ adapters/ services.ts index.ts`
- Infra (plumbing): `src/infra/commands/`, `src/infra/usecases/`, `src/infra/domain/`, `src/infra/ports/`, `src/infra/adapters/`
- Shared primitives: `src/shared/lib/`, `src/shared/domain/`, `src/shared/errors.ts`, `src/shared/version.ts`
- TUI rendering: `src/tui/app/render.ts`, `src/tui/app/render-check.ts`, `src/tui/state/snapshot.ts`
- Tests: `tests/unit/features/`, `tests/unit/infra/`, `tests/unit/shared/`, `tests/unit/tui/`, `tests/integration/`, `tests/e2e/` (compiled-binary and end-to-end flows)
- Build output: `dist/maestro`
- Installed binary: `~/.local/bin/maestro`

## After code changes
```bash
bun run build && ./dist/maestro --version
bun test
```

## After TUI changes
```bash
./dist/maestro mission-control --render-check --size 120x40
bun tui:dev --screen all --size 120x40
```
