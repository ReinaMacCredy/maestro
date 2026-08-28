# Contributing

Maestro is a single-maintainer project. Contributions are welcome; the rules
below are what keeps the durable record trustworthy, so a change that ignores
them will be asked to change rather than merged.

## Toolchain

[Bun](https://bun.sh) 1.4.0 or newer, and Git. Nothing else. The runtime,
SQLite driver, and test runner are all Bun's (`bun:sqlite`, `bun:test`); do not
introduce Node-only APIs, a second package manager, or a build step.

```sh
git clone https://github.com/ReinaMacCredy/maestro.git
cd maestro
bun install
bun test
```

Install your checkout over your own machine with `bun bin/maestro.ts install`,
which points `maestro update` at that checkout.

## Layout

- `src/kernel/` is mechanism only: store, event log, sessions, CLI, plugin
  loader, readiness projection. No workflow vocabulary.
- `src/plugins/` holds verbs and policy gates.
- `src/plugins/recipes/` and `src/plugins/skills/` are prompt-first Markdown.
- `bin/maestro.ts` is the entry point, `scripts/install.sh` the curl installer.
- `site/` is the Starlight documentation site.
- `tests/` is `bun:test`.

## Before you open a pull request

Run all four. CI runs the same checks and will fail on any of them.

```sh
bun test
bunx tsc --noEmit
```

Plus the three architectural gates, which are greps CI enforces:

- **A1** no daemon or scheduler: `src/` contains no `setInterval`, no long
  `setTimeout`, no `cron`, no `detached: true`. Maestro has no background
  process; state is computed when a verb runs.
- **A2** mechanism-only kernel: `src/kernel/` contains no policy vocabulary
  (`proof`, `qa`, `tdd`, `test-first`, `research`).
- **A3** no escape-hatch flags: `src/` contains no `lean`, `--lane light`, or
  `--qa`. A gate is either enforced or removed, never bypassed by a flag.

## How a change should arrive

Test-first: a failing test that names the behavior, the smallest change that
makes it pass, then the full suite. A pull request that changes behavior
without a test that fails before it is not reviewable.

One logical change per commit, with a message that says what changed and why.
Do not reformat, rename, or tidy code the change does not touch.

Do not hand-edit installer-owned regions: the `<!-- maestro:begin -->` blocks in
`AGENTS.md` and `CLAUDE.md`, and the `.claude/` and `.codex/` wiring. The
installer writes them.

## Reporting problems

Bugs and feature ideas go to
[GitHub issues](https://github.com/ReinaMacCredy/maestro/issues). Suspected
vulnerabilities go through the private channel in
[SECURITY.md](SECURITY.md), never a public issue.

## License

By contributing you agree that your contribution is licensed under the MIT
license in [LICENSE](LICENSE).
