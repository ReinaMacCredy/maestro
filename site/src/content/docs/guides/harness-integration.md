---
title: Harness integration
description: How install wires Claude Code and Codex, and how the session brief reaches an agent.
---

## Claude Code and Codex hooks

`maestro install` writes hooks for Claude Code and Codex into the repository
and merges only Maestro's own entries into each harness's settings.
`SessionStart` and `UserPromptSubmit` record the session and print the
current brief. No hook sends mail, starts a team, prompts another seat, or
streams pane transcript.

The brief is short. It reaches one harness process once, whichever transport
speaks first. It names how to add and close tracked work
(`maestro work add "<title>" --start`, `maestro work done <id> --check
"<command>"`), the work this session holds, any attention findings, and where
the playbooks are. Print it yourself with:

```sh
maestro brief --session
```

If a harness hook did not fire, record the start event explicitly and read the
brief from stdout:

```sh
maestro hook record --event SessionStart
```

## Instruction files

Install adds nothing to the repository's `AGENTS.md` or `CLAUDE.md`. The shared
method lives at `~/maestro/WORKFLOW.md`, and the brief points agents there.

An SLP seat does not read those files either: its contract is the pinned
project `.maestro/SLP.md` plus its seat file, rendered into the seat's own
config dir.

## MCP transport

```sh
maestro mcp serve
```

`mcp serve` is a foreground stdio MCP server that delivers the session brief
as its initialize instructions. It offers no tools and is not a daemon.
