# maestro

TypeScript on Bun, released as v0.108.0 (the first TypeScript release; 0.107.x
was the Rust line, whose stores are preserved under `legacy/rust/`). Work
happens on `main`; the `rewrite-maestro-in-typescript` branch and the
`.maestro/worktree/rewrite-ts` worktree are retired.

Layout: `src/kernel/` is mechanism only (store, event log, sessions, cli,
loader); `src/plugins/` holds verbs and policies; `src/plugins/recipes/` and
`src/plugins/skills/` are prompt-first markdown; `bin/maestro.ts` is the entry;
`scripts/install.sh` is the curl installer; `site/` is the Starlight docs site
published by `.github/workflows/pages.yml`; `tests/` is `bun:test`.

Method: `maestro recipe show work` for depth, `~/maestro/skills/maestro-bundle`
for the tier rule (quickfix: a one-sentence diff with no Full trigger, done
directly with inline verification and no record; Light: a work item; Full: a
bundle). Bundles live in `.spec-workflow/` (`MEMORY.md` index, `active/`,
`archive/`, `adr/`); append to a bundle's NOTES.md, never rewrite its history.

Rules:

- Toolchain: bun only (runtime, `bun:sqlite`, `bun:test`). Install from a
  checkout with `bun bin/maestro.ts install`; `maestro update` follows that
  checkout, fast-forward only.
- Kernel stays free of policy vocabulary (CI gate A2); no daemon, scheduler, or
  detached process (A1); no escape-hatch flags such as `lean` or `--lane light`
  (A3). Run `bun test`, `bunx tsc --noEmit`, and those greps before pushing.
- Test-first: red test, minimal change, green; the full suite runs after
  install, from the checkout being installed.
- Never hand-edit the maestro managed block below or the `.claude/` and
  `.codex/` wiring; the installer owns them.
- Commit per verified step; never push, tag, or release unasked.

<!-- maestro:begin -->
A session in this repository is its Lead; panes it opens with a dispatch are Peers; the room at ~/maestro is the Supervisor. Roles: `maestro recipe show slp`.
Live maestro state is injected by hooks. Use `maestro status` for the current session view and `maestro ready` for available work.
Track work with `maestro work add|start|done`; method depth: `maestro recipe show work`.
If no harness hook fired, run `maestro hook record --event SessionStart` and read the brief from stdout.
Failed commands print a JSON error envelope on stderr and exit nonzero; when the fix is mechanical, the message names the next command to run.
<!-- maestro:end -->
