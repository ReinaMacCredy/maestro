---
title: Configuration
description: Repository plugin configuration, runtime environment variables, and machine layout.
---

## `.maestro/config`

Repository configuration is JSON with a `plugins` array. Each item has a plugin
`name` and optional `disabled` boolean. `maestro install` writes the managed
policy defaults; `maestro plugin enable <name>` and `maestro plugin disable
<name>` own lifecycle changes.

The default policy state is:

| Plugin | State |
| --- | --- |
| `policy-proof` | enabled |
| `policy-breakdown` | enabled |
| `policy-card-budget` | disabled |
| `policy-auto-pr` | disabled |

Install writes only the two enabled entries. Enabling `policy-auto-pr` is the
repository's delivery grant: the deliver step commits, pushes the branch and
opens a PR (`maestro recipe show opening-a-pr`); merge, tag and release stay
the owner's. The SLP team plugin, `slp`, is also disabled until you enable it.

Policy also has a machine layer, `~/.maestro/config`, with the same shape.
The loader reads it before the repository file, and a repository entry wins
for the same plugin name. `maestro install` never writes the machine layer.

## Environment variables read by `src/`

The source inventory comes from an exhaustive search:

```sh
rg -o 'process\.env\.[A-Z_]+' src/ | sort -u
```

| Variable | Runtime use |
| --- | --- |
| `HOME` | Resolve user-level runtime, source, room, and registry paths. |
| `PATH` | Locate installed tools and the shim directory. |
| `SHELL` | Choose `.zshrc` or `.bashrc` for the one managed source line. |
| `MAESTRO_READ_ONLY` | With value `1`, enable fail-closed read-only mode. |
| `MAESTRO_AUTO_UPDATE` | With value `0`, suppress the source/runtime drift advisory. |
| `MAESTRO_INSTALL_REEXEC` | Internal guard for installer re-execution after runtime sync. |
| `MAESTRO_ROOM_SCAFFOLD` | Internal guard that lets only the installer run `room mark`. |
| `MAESTRO_SESSION_ID` | Explicit Maestro session identity, highest precedence. |
| `MAESTRO_SESSION_PID` | Explicit positive host PID for session liveness. |
| `MAESTRO_SESSION_NONE` | With value `1`, disable session persistence for a process. |
| `CODEX_SESSION_ID`, `CODEX_THREAD_ID` | Codex session identity candidates. |
| `CLAUDE_CODE_SESSION_ID`, `CLAUDE_SESSION_ID` | Claude session identity candidates. |
| `CURSOR_SESSION_ID` | Cursor session identity candidate. |
| `CODEX_CI`, `CODEX_SHELL` | Signals used to classify an otherwise unnamed session as Codex. |
| `HERDR_PANE_ID` | The Herdr pane an SLP command runs from, read to prove its seat. |

The curl installer also reads `MAESTRO_SOURCE_DIR` and `MAESTRO_REF`; those
variables live in `scripts/install.sh`, not under `src/`.

## Machine layout

| Path | Purpose |
| --- | --- |
| `~/.maestro/source/` | Default Git checkout followed by `maestro update`. |
| `~/.maestro/source.json` | Recorded source checkout metadata. |
| `~/.maestro/runtime/` | Installed TypeScript runtime and install stamp. |
| `~/.local/bin/maestro` | Bun shim that loads the installed runtime. |
| `~/maestro/` | The Hub room. |
| `~/maestro/WORKFLOW.md` | The shared method every repository's agents read. |
| `~/maestro/.maestro/maestro.db` | Hub room store. |
| `~/maestro/registry` | Registered repository paths read by `maestro brief`. |
| `~/maestro/skills/` | The seven managed Maestro skills. |
| `~/maestro/shellrc` | Maestro-owned shell functions sourced by one managed rc line. |
| `~/maestro/SLP.md` | The SLP Workspace Pack, used when the `slp` plugin is enabled. |
| `<git-common-root>/.maestro/maestro.db` | Repository store shared by linked worktrees. |
| `<project>/.maestro/SLP.md` | Pinned copy of the Workspace Pack for the current or most recent team generation. |

## SLP data ownership

With the `slp` plugin enabled, the Hub store records team identity, project
path, generation, pinned pack identity, seat bindings, lifecycle and minimal
activity. The project store records the local team binding, work, notes,
returns, acceptances, decisions and minimal activity.

Herdr owns the live workspace and panes. The runtime pane's lock and queue
state are temporary and are deleted at team stop. See
[SLP setup and storage](/getting-started/slp-setup/) for the complete map.
