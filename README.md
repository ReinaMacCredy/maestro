# maestro

Maestro is a local-first CLI for keeping human and agent work coordinated inside
a repository. It stores durable work, decisions, sessions, evidence, and
messages in the repository's shared Git root, then delivers the current state to
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

## Install and update

Run the first install from a Maestro source checkout. The installer preserves
the previous executable as `maestro-legacy`, copies the Bun runtime to
`~/.maestro/runtime`, writes the shim at `~/.local/bin/maestro`, and wires the
current repository:

```sh
bun bin/maestro.ts install
maestro version
```

To update after the rewrite is on `main`, pull that checkout and reinstall from
it. `maestro install` recognizes the Maestro source checkout and resyncs the
runtime:

```sh
git switch main
git pull --ff-only origin main
maestro install
maestro version
```

## Verb tour

- `maestro status` shows the current session view; `maestro ready` shows work
  that can start now.
- `maestro work add|start|note|done|show|list` manages the work tree, dependency
  edges, acceptance, leases, and evidence.
- `maestro decision draft|lock|show|list` records choices with their own
  lifecycle.
- `maestro msg send|read` uses the repository mailbox shared by live sessions.
- `maestro plugin list|enable|disable|new|add|remove` manages built-in and local
  extensions.
- `maestro recipe list|show` serves the deeper working methods; `maestro watch`
  renders live state.
- `maestro install` refreshes the runtime and repository wiring. `maestro
  version`, `maestro --version`, and `maestro -v` report the same install
  identity.

`maestro help` and the Markdown recipes are the deeper command and method
references.

## Claude and Codex hooks

`maestro install` writes harness-specific adapters under `.claude/hooks/` and
`.codex/hooks/`, merges their settings, and injects live state on session start
and the next prompt. It also maintains small pointer blocks in `CLAUDE.md` and
`AGENTS.md`. Claude consumes its configured commands directly; review Codex
hook trust with `/hooks` after installation.
Do not set `MAESTRO_SESSION_PID` manually: sessions anchor to the live agent host process automatically, and the environment variable exists for tests.
