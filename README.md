# maestro

Maestro is a local-first CLI for keeping human and agent work coordinated inside
a repository. It stores durable work, decisions, sessions, evidence, dispatches,
and handbacks in the repository's shared Git root, then delivers the current state to
supported agent harnesses through hooks. It runs on Bun and does not require a
background service.

## Three layers

- **Mechanism kernel** owns CLI dispatch, the SQLite store, event delivery,
  sessions, readiness projection, and plugin loading. It does not impose a
  workflow.
- **Plug-and-play policy plugins** add optional gates such as proof, breakdown,
  TDD, QA, research, and independent witness checks. Enable or disable them per
  repository in `.maestro/config`.
- **Markdown recipes** provide methods on demand without copying protocol prose
  into every repository. Browse them with `maestro recipe list` and read one
  with `maestro recipe show <name>`.

## Install, update, and remove

Run the first install from a Maestro source checkout. The installer preserves
the previous executable as `maestro-legacy`, copies the Bun runtime to
`~/.maestro/runtime`, writes the shim at `~/.local/bin/maestro`, and wires the
current repository:

```sh
bun bin/maestro.ts install
maestro version
```

The source checkout is recorded outside the runtime at
`~/.maestro/source.json`. `maestro update` fetches that checkout's current
upstream, accepts only a fast-forward, and resyncs the runtime. It refuses a
dirty, diverged, unreachable, or stale source without partially updating the
source or runtime:

```sh
maestro update
maestro version
```

`maestro install` remains the offline resync operation and is equivalent to the
old `sync` behavior when run from a source checkout.

`maestro uninstall` removes Maestro-managed hooks, settings keys, mirror
blocks, and wiring from the current repository. It is idempotent and never
deletes `.maestro/maestro.db` or a legacy `.maestro/store.sqlite`. To remove the
machine-level shim and runtime as a separate manual action, run exactly:

```sh
rm ~/.local/bin/maestro
rm -rf ~/.maestro/runtime
```

`maestro doctor` diagnoses the shim target, runtime stamp, recorded source,
current repository wiring, and store access without repairing or changing
them. A healthy report exits zero; every reported issue names its fix command.

Status and hook briefs append a one-line advisory when the installed runtime
commit differs from the recorded source checkout's local HEAD. This check is
offline and never fetches. Set `MAESTRO_AUTO_UPDATE=0` to silence it.

## Verb tour

- `maestro status` shows every session, or only live sessions with `--live`;
  `maestro ready` shows work that can start now.
- `maestro work add|start|note|done|show|list` manages the work tree, dependency
  edges, acceptance, leases, and evidence.
- `maestro decision draft|lock|show|list` records choices with their own
  lifecycle.
- `maestro dispatch open|accept|show|list` and `maestro handback file|show`
  preserve lane contracts and return packets; inspect the archive with
  `maestro dispatch list <work-id>` and `maestro handback show <id>`.
- `maestro plugin list|enable|disable|new|add|remove` manages built-in and local
  extensions.
- `maestro recipe list|show` serves the deeper working methods.
- `maestro install` refreshes the runtime and repository wiring; `maestro
  update`, `maestro uninstall`, and `maestro doctor` complete the distribution
  lifecycle. `maestro version`, `maestro --version`, and `maestro -v` report
  the same install identity.

`maestro help` and the Markdown recipes are the deeper command and method
references.

## Claude and Codex hooks

`maestro install` writes harness-specific adapters under `.claude/hooks/` and
`.codex/hooks/`, merges their settings, and injects live state on session start
and the next prompt. It also maintains small pointer blocks in `CLAUDE.md` and
`AGENTS.md`. Claude consumes its configured commands directly; review Codex
hook trust with `/hooks` after installation.
Do not set `MAESTRO_SESSION_PID` manually: sessions anchor to the live agent host process automatically, and the environment variable exists for tests.
When a sandbox blocks process inspection, Maestro falls back to a 60-minute
session anchor refreshed only by commands that session runs. Two concurrent
sandboxed sessions from the same harness in one worktree cannot be
distinguished; the most recently active session receives subsequent commands.

## Rust-era data

The stores from the Rust line live under `legacy/rust/`. The reference import
keeps its original read-only behavior:

```sh
maestro import rust --path legacy/rust/store.sqlite
```

Add `--promote` once to create native work and decisions. Features, tasks,
ideas, and bugs keep their kind; progress cards become `chore` work. Terminal
success states become `done`, terminal rejection states become `cancelled`, and
all other work remains open without a lease. Decision supersession links and
receipt notes are retained. Receipts whose card no longer exists are skipped
and counted. `legacy_map` makes a repeated promotion report zero created.

`maestro import rust --path legacy/rust/archive-cards.sqlite` imports archived
snapshots as searchable legacy files. Bun's built-in zstd decoder reads the
snapshot payloads; any payload that cannot be decoded falls back to its stored
search text and is counted as skipped. See `legacy/rust/README.md`.
