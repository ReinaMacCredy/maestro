---
title: Configuration
description: Repository plugin configuration, runtime environment variables, and machine layout.
---

## `.maestro/config`

Repository configuration is JSON with a `plugins` array. Each item has a plugin
`name` and optional `disabled` boolean. `maestro install` writes the managed
policy defaults; `maestro plugin enable <name>` and `maestro plugin disable
<name>` own lifecycle changes.

This file lives inside the repository, so it states policy only and can never
grant trust. Whether an external plugin may execute at all is recorded in
`~/.maestro/trust.json` by `maestro plugin trust`.

The default policy state is:

| Plugin | State |
| --- | --- |
| `policy-proof` | enabled |
| `policy-breakdown` | enabled |
| `policy-tdd` | disabled |
| `policy-qa` | disabled |
| `policy-research` | disabled |
| `policy-witness` | disabled |
| `policy-lifecycle` | disabled |

Policy also has a machine layer, `~/.maestro/config`, with the same shape.
The loader reads it before the repository file, and a repository entry wins
for the same plugin name. `maestro install` never writes the machine layer.
The Hub room (`~/maestro`) carries no `.maestro/config`, so it runs the
built-in defaults; those match the table above except `policy-lifecycle`,
which is enabled there and defines no gates.

## Environment variables read by `src/`

The source inventory was generated with the repository's required exhaustive
search:

```sh
rg -o 'process\.env\.[A-Z_]+' src/ | sort -u
```

| Variable | Runtime use |
| --- | --- |
| `HOME` | Resolve user-level runtime, source, plugin, room, and registry paths. |
| `PATH` | Locate installed tools and the shim directory. |
| `SHELL` | Choose `.zshrc` or `.bashrc` for the one managed source line. |
| `MAESTRO_READ_ONLY` | With value `1`, enable fail-closed read-only mode. |
| `MAESTRO_AUTO_UPDATE` | With value `0`, suppress the source/runtime drift advisory. |
| `MAESTRO_INSTALL_REEXEC` | Internal guard for installer re-execution after runtime sync. |
| `MAESTRO_SESSION_ID` | Explicit Maestro session identity, highest precedence. |
| `MAESTRO_SESSION_PID` | Explicit positive host PID for session liveness. |
| `MAESTRO_SESSION_NONE` | With value `1`, disable session persistence for a process. |
| `CODEX_SESSION_ID`, `CODEX_THREAD_ID` | Codex session identity candidates. |
| `CLAUDE_CODE_SESSION_ID`, `CLAUDE_SESSION_ID` | Claude session identity candidates. |
| `CURSOR_SESSION_ID` | Cursor session identity candidate. |
| `CODEX_CI`, `CODEX_SHELL` | Signals used to classify an otherwise unnamed session as Codex. |

The curl installer also reads `MAESTRO_SOURCE_DIR` and `MAESTRO_REF`; those
variables live in `scripts/install.sh`, not under `src/`.

## Machine layout

| Path | Purpose |
| --- | --- |
| `~/.maestro/source/` | Default Git checkout followed by `maestro update`. |
| `~/.maestro/source.json` | Recorded source checkout metadata. |
| `~/.maestro/runtime/` | Installed TypeScript runtime and install stamp. |
| `~/.local/bin/maestro` | Bun shim that loads the installed runtime. |
| `~/maestro/` | Supervisor room. |
| `~/maestro/SLP.md` | Canonical SLP v2 Workspace Pack and model defaults. |
| `~/maestro/.maestro/maestro.db` | Supervisor room store. |
| `~/maestro/registry` | Registered repository paths read by `maestro brief`. |
| `~/maestro/skills/` | The four managed Maestro method skills. |
| `~/maestro/shellrc` | Maestro-owned shell functions sourced by one managed rc line. |
| `<git-common-root>/.maestro/maestro.db` | Repository store shared by linked worktrees. |
| `<project>/.maestro/SLP.md` | Pinned managed Workspace Pack snapshot for the current or most recent team generation. |

## SLP data ownership

The Hub store records team identity, project path, generation, pinned pack
identity, role bindings, lifecycle and minimal activity. The project store
records local team binding, work, notes, returns, acceptances, decisions and
minimal activity.

Herdr owns the live workspace and panes. Raw Watch transcript is temporary and
is deleted at team stop; it has no archive or evidence location. See
[SLP setup and storage](/getting-started/slp-setup/) for the complete map.
