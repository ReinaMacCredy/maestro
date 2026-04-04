# Maestro CLI

Read `AGENTS.md` at the project root for full conventions (types, naming, async, testing, release, Mission Control contracts, agent-optimized TUI preview).

## Key paths
- Source entry: `src/index.ts`
- TUI rendering: `src/tui/app/render.ts`, `src/tui/app/render-check.ts`
- Commands: `src/commands/`
- Tests: `tests/unit/`, `tests/integration/`
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
