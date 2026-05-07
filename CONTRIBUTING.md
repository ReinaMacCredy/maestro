# Contributing to Maestro

Maestro is a local-first conductor for multi-agent software engineering.
This document covers what you need to know to make a contribution.

## Quick start

```bash
git clone https://github.com/ReinaMacCredy/maestro.git
cd maestro
bun install
bun run build           # compile dist/maestro
./dist/maestro --version
bun test
```

`bun run release:local` rebuilds and installs the binary to `~/.local/bin/maestro`,
which is the right way to test the version on your `PATH` against the current
source.

## Repository layout

The codebase is feature-first and hexagonal:

- `src/features/<name>/` owns one bounded context with `commands/`, `usecases/`,
  `domain/`, `ports/`, `adapters/`, `services.ts`, and `index.ts`. Cross-feature
  imports must go through `index.ts` only; deep imports are blocked by
  `bun run check:boundaries`.
- `src/infra/` is plumbing: init, doctor, status, install, providers,
  mission-control, config, and git ports/adapters.
- `src/shared/` holds generic utilities with no domain knowledge.
- `src/tui/` renders Mission Control as a read-only dashboard.
- `src/services.ts` is the composition root; `src/index.ts` is the Commander
  entry point.

`AGENTS.md` is the canonical contributor guide and lists every convention. Read
it before opening a substantive PR.

## Conventions

- TypeScript strict mode. `interface` for object shapes, `type` for unions.
  Prefer `unknown` and narrowing over `any`. Public APIs carry explicit
  return types.
- ESM only. Bun-first runtime; Node-only APIs go through `src/shared/lib/`.
- Tests live under `tests/unit/`, `tests/integration/`, and `tests/e2e/`. Mock
  external dependencies, not internal modules. The compiled-binary helper is
  `tests/helpers/run-compiled-cli.ts`; see `tests/AGENTS.md` for guidance on
  picking `./dist/maestro` versus the installed `maestro` on `PATH`.
- Commits follow Conventional Commits: `feat(scope):`, `fix(scope):`,
  `perf(scope):`, `refactor(scope):`, `chore(scope):`. One logical change per
  commit.
- Repo-tracked behavior changes bump the CLI version (`bun run bump patch` or
  `bun run bump feature`). Docs- or comment-only changes do not.
- The bundled skill set under `skills/built-in/` and `skills/bundled/` is the
  source of truth for what agents see. The embeds at
  `src/infra/domain/built-in-skill-templates.ts` and
  `bundled-skill-templates.ts` are generated; do not hand-edit. Run
  `bun run sync:built-in-skills` and `bun run sync:bundled-skills` after
  editing skill content.

## Required checks before opening a PR

```bash
bun run build
bun run check:boundaries
bun run check:skills
bun run check:bundled-skills
bun test
```

For TUI changes, also run:

```bash
./dist/maestro mission-control --render-check --size 120x40
bun tui:dev --screen all --size 120x40
```

CI runs `maestro ci verify` against every PR and posts a GitHub Check. The
check is the merge gate; local Maestro is advisory. See
[`docs/ci-integration.md`](docs/ci-integration.md) for the full reference.

## Architectural patterns

When adding a new behavior, the layered shape is:

1. Define a port (`src/features/<name>/ports/`) — pure interface, no
   implementation.
2. Implement an adapter (`src/features/<name>/adapters/`) against the
   filesystem, git, or external APIs.
3. Write a use-case (`src/features/<name>/usecases/`) that takes the port as a
   dependency and returns plain data.
4. Wire the adapter in `src/services.ts`.
5. Expose a thin command (`src/features/<name>/commands/`) that calls the
   use-case.
6. Add unit tests for the use-case, integration tests for the adapter, and an
   e2e test against the compiled binary when behavior is end-user visible.

Existing features are reasonable templates: `evidence`, `verdict`, `task`, and
`skills` each illustrate the pattern at increasing complexity.

## Filing issues

When reporting a bug, include:

- `maestro --version`
- A minimal reproduction in a fresh repo (`maestro init` in `/tmp/foo`).
- The exact command, full output, and expected vs observed behavior.
- Whether you exercised `./dist/maestro` (the build artifact) or the installed
  `maestro` on `PATH` — these can drift.

For security-sensitive reports, see [SECURITY.md](SECURITY.md).

## License

Contributions are accepted under the [MIT License](LICENSE).
