# maestro

TypeScript on Bun since v0.108.0 (0.107.x was the Rust line, whose stores are
preserved under `legacy/rust/`). Work happens on `main`; the
`rewrite-maestro-in-typescript` branch and the `.maestro/worktree/rewrite-ts`
worktree are retired.

Layout: `src/kernel/` is mechanism only (store, event log, sessions, cli,
loader); `src/plugins/` holds verbs and policies; `src/plugins/recipes/` and
`src/plugins/skills/` are prompt-first markdown; `bin/maestro.ts` is the entry;
`scripts/install.sh` is the curl installer; `site/` is the Starlight docs site
published by `.github/workflows/pages.yml`; `tests/` is `bun:test`.

Method: `maestro recipe show work` for depth, `~/maestro/skills/maestro-bundle`
for the tier rule (quickfix: a one-sentence diff with no Full trigger, done
directly with inline verification and no record; Light: a work item; Full: a
bundle). Bundles live in `.maestro/bundle/<id>/` (SPEC/NOTES/VERIFY); NOTES.md
is an overwrite-only handoff, and history lives in the store (`maestro work
note`, `maestro trace`, decisions), never in the file.

Rules:

- Toolchain: bun only (runtime, `bun:sqlite`, `bun:test`). Install from a
  checkout with `bun bin/maestro.ts install`; `maestro update` follows that
  checkout, fast-forward only.
- Kernel stays free of policy vocabulary (CI gate A2); no daemon, scheduler, or
  detached process (A1); no escape-hatch flags such as `lean` or `--lane light`
  (A3). Run `bun test`, `bunx tsc --noEmit`, and those greps before pushing.
- Test-first: red test, minimal change, green; the full suite runs after
  install, from the checkout being installed.
- Never hand-edit the `.claude/` and `.codex/` wiring; the installer owns those
  files.
- Commit per verified step; never push, tag, or release unasked.
