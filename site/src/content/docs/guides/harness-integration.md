---
title: Harness integration
description: How install wires Claude Code, Codex, managed instructions, and MCP.
---

## Claude Code and Codex hooks

`maestro install` writes managed adapters for Claude Code and Codex and merges
only Maestro-owned hook entries. `SessionStart` and `UserPromptSubmit` record
the session and print the current brief. No hook sends mail, starts a team,
prompts another role, or streams pane transcript.

If a harness hook did not fire, record the start event explicitly and read the
brief from stdout:

```sh
maestro hook record --event SessionStart
```

## Managed instruction blocks

Install maintains a small block between `<!-- maestro:begin -->` and
`<!-- maestro:end -->` in `AGENTS.md` and `CLAUDE.md`. It points ordinary
repository sessions to the installed Maestro workflow and current status.

SLP role contracts do not live in that managed block. `team start` injects the
shared section and one role section from the pinned project
`.maestro/SLP.md`; normal team operation never rewrites `AGENTS.md` or
`CLAUDE.md`.

Do not edit that managed block by hand. Re-run the source checkout's installer
when managed wiring needs to be refreshed; unrelated content outside the block
is preserved.

## MCP transport

Start the foreground stdio MCP server with:

```sh
maestro mcp serve
```

The server exposes exactly two meta-tools: `maestro_find` searches live verbs,
flags, descriptions, and recipes; `maestro_run` executes one Maestro verb line
through the normal strict command router. It is a foreground JSON-RPC transport,
not a daemon.
