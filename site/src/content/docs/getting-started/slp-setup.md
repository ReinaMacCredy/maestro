---
title: SLP setup and storage
description: Enable the SLP team plugin, start one team, and see which state belongs to the Hub, the project, and the runtime.
---

SLP is Maestro's team layer. It runs one Lead and the Peers the Lead opens,
each in its own Herdr pane, and records every hand-off in the store. It is a
built-in plugin named `slp`, and it is off by default.

## Enable the plugin

Turn SLP on in the Hub room and in the project the team will work in:

```sh
cd ~/maestro
maestro plugin enable slp
cd /absolute/path/to/project
maestro plugin enable slp
```

`maestro team start` also enables it in the project it starts a team for.
While the plugin is off, the team verbs (`team`, `slp`, `decide`, `status`,
`work take`, `work return`, `work accept`, and `work add --to`) are refused
with a line that names the enable command. `maestro install` then links no
Herdr plugin and writes no seat directories, and a new store gets no SLP
tables. Turning the plugin off later keeps the SLP records already stored;
they stay readable.

## Before you start

Install Maestro and Herdr as described in [Install](/getting-started/install/).
Run the read-only checks from a project:

```sh
maestro version
maestro doctor
```

Claude seats log in with one long-lived token. Create it once with
`claude setup-token` (a browser flow) and hand it to Maestro on stdin:

```sh
claude setup-token | maestro install --seat-token
```

`maestro doctor` then reports `seat token: present`. Until it does,
`team start` refuses a Claude seat with `SEAT_TOKEN_MISSING`. See
[Seat config dirs](#seat-config-dirs-and-the-token).

SLP is a cooperative-agent protocol, not a shell security sandbox. Maestro
checks the nine SLP operations at their supported boundaries: Hub operations
must run from the Hub room, while project role operations require the current
generation's stored Herdr pane binding. It does not block native commands,
administrative Maestro commands, or direct Herdr calls; topology and
external-effect limits remain obligations enforced by the owner and host
policy.

## Start a team

You start a team by asking your Hub agent, the agent session you run in
`~/maestro`, for one. It runs:

```sh
cd ~/maestro
maestro team start /absolute/path/to/project "<observable objective>"
```

That single operation:

1. reads the exact bytes of `~/maestro/SLP.md`, the Workspace Pack (the
   contract every seat follows);
2. records the pack version and SHA-256 in the Hub store and pins the bytes of
   both seat files, `lead` and `peer`;
3. copies the pack to `<project>/.maestro/SLP.md`;
4. creates one Herdr workspace for this generation (one run of the team);
5. opens the Lead from its seat file and waits for it to acknowledge;
6. opens the runtime pane and records your Hub agent's pane as the target of
   the team's upward wakes;
7. creates the initial `OPEN` work item assigned to the Lead.

`--lead-model <model>` and `--peer-model <model>` choose the model each seat
launches with for this generation. They change only the launch line, never
the seat file or its pinned digest.

`team start` and `work add --to` wait while a pane opens and acknowledges its
contract, normally under a minute, and print their phases on stderr. Do not
re-run either while it is still running. A Claude seat trusts the project
directory because you started the team there: Maestro marks the project
trusted in that seat's `.claude.json` before the pane opens. A Codex pane must
already trust the directory: the first start in a new directory fails with
`TRUST_DIALOG` naming the harness and directory. Open that directory once with
Codex, accept its trust dialog, and rerun.

Repeating the same `team start` against a running generation touches only what
is missing. Live panes are left alone, closed panes are reopened and
acknowledged again, and the start record is refreshed to the current pane ids.

Hand-offs push a wake-up line: after `work add`, `work return`, `work accept`,
`work note --rework`, and `decide --work <id>` commit, Maestro sends one line
to the pane that must act next (`[from lead][w2 OPEN] <objective>; read:
maestro status w2`). The store stays the truth. A push that fails prints a
warning and nothing else changes.

The Lead takes its first item from its own pane:

```sh
maestro work take <work-id>
```

## Storage map

| Location | Owner | What it stores | Lifetime |
| --- | --- | --- | --- |
| `~/maestro/SLP.md` | Hub owner | The Workspace Pack: the shared contract, the Peer's working agreement, and the Hub Supervisor section | Written only when absent; install and update keep your edits; edits reach the next generation only |
| `~/maestro/.maestro/maestro.db` | Hub | Team id, project path, generation, pack version and digest, seat identities, owner and cross-team decisions, lifecycle and minimal activity | Durable |
| `<project>/.maestro/SLP.md` | Project | Exact copy of the pack the running generation uses | Kept after stop; replaced at the next start |
| `<git-common-root>/.maestro/maestro.db` | Project | Team binding, seats, work, notes, returns, acceptances, technical decisions and minimal activity | Durable and shared by linked worktrees; every read is filtered to the current checkout |
| `<project>/.maestro/profiles/`, `~/maestro/profiles/` | Project, Hub owner | Your copies of the seat files `lead.md` and `peer.md` | Durable; a running generation pins the bytes it started with |
| `~/.maestro/claude/<seat>/` (`lead`, `peer`) | `maestro install` | One `CLAUDE_CONFIG_DIR` per Claude seat: the rendered seat, `settings.json` (0600), `skills/` links, `projects` and `plugins` links to `~/.claude`, `.claude.json` | Rewritten by every install (`.claude.json` only when absent), removed by uninstall |
| `~/.maestro/claude/oauth-token` | `maestro install --seat-token` | The `claude setup-token` token every Claude seat uses (0600) | Kept by install, removed by uninstall |
| `~/.codex/maestro-lead.config.toml`, `~/.codex/maestro-peer.config.toml` | `maestro install` | The Codex launch config for each seat | Rewritten by every install, removed by uninstall |
| Herdr workspace `slp-<team>-g<n>` | Runtime | Lead, Peers and the runtime pane | Exists only while the generation runs |
| `<OS temp>/maestro-slp-<uid>/<project-hash>/<team>/g<n>/` | Runtime | The runtime pane's lock and its pending wakes | Temporary; deleted at team stop |
| `~/maestro/herdr-plugin.toml` | `maestro install` | The Herdr plugin manifest; the Hub room is linked as the plugin `maestro` | Rewritten by every install, removed by uninstall |

Never edit either SQLite store by hand. Use Maestro operations so current state
and minimal activity change in the same transaction.

### Seat files

SLP has two seats, each launched from a seat file Maestro ships: `lead.md` and
`peer.md`. A seat file is Markdown. Its YAML frontmatter sets `harness`
(`claude` or `codex`), `model`, `effort`, `permission`, `autocompact`,
`disallowed_tools`, `skills`, `settings`, and `description`; its body is the
seat's mandate. Maestro looks for a seat file in
`<project>/.maestro/profiles/`, then `~/maestro/profiles/`, then the shipped
copy, and uses the first it finds. A copy with frontmatter and no body keeps
the mandate from the layer below, so a Lead on another model is a two-line
`~/maestro/profiles/lead.md`.

`maestro install` renders each seat for Claude into
`~/.maestro/claude/<seat>/agents/maestro-<seat>.md` and for Codex into
`~/.codex/maestro-<seat>.config.toml`. Every rendered seat carries the pack's
shared contract followed by its own mandate; the Peer carries the pack's
shorter working agreement instead. A seat whose render is missing fails with
`SEAT_NOT_INSTALLED` before any pane opens.

`maestro work add "<objective>" --to <peer> --fresh` reuses an acknowledged
Peer pane with a fresh harness context (Claude Code `/clear`, Codex `/new`)
before the `OPEN` wake. It is refused with `PEER_ACTIVE` while that Peer holds
ACTIVE work and with `PEER_RESET_FAILED`, nothing added, when the reset does
not take.

### Seat config dirs and the token

Every Claude seat runs under its own `CLAUDE_CONFIG_DIR`, rendered by
`maestro install` at `~/.maestro/claude/lead/` and `~/.maestro/claude/peer/`.
A seat therefore sees none of your `~/.claude`: no `CLAUDE.md`, no personal
skills, no user settings or MCP servers. The dir holds:

- `agents/`: the rendered seat file.
- `settings.json` (0600): the Maestro base merged with the seat file's
  `settings:` overlay (a `null` value removes a key). The base bypasses
  permission prompts, copies the Herdr `SessionStart` hook from your user
  settings, carries the seat token and switches off workflows, cron, fast
  mode, bundled skills, git instructions and auto memory. It excludes your
  `~/.claude/CLAUDE.md` and `~/.claude/rules/**`, which would otherwise load
  into every project under your home directory.
- `skills/`: one link per name in the seat file's `skills:` list, resolved in
  `~/maestro/skills` then `~/.claude/skills`. An unknown name fails install
  and names the seat.
- `projects` and `plugins`: links to `~/.claude/projects` and
  `~/.claude/plugins`, so transcripts and plugins stay shared.
- `.claude.json`: written once with onboarding marked done and no MCP
  servers. `team start` and `work add --to` add one key before a seat pane
  opens: the project is marked trusted when it is not already.

The token from `claude setup-token` lives in `~/.maestro/claude/oauth-token`
(0600) and in each seat's settings `env` as `CLAUDE_CODE_OAUTH_TOKEN`. It
never appears in install output. Without it, install still renders and prints
one warning, `maestro doctor` reports `seat token: missing`, and a Claude
seat is refused with `SEAT_TOKEN_MISSING` before its pane opens. The token
lasts a year; doctor prints the date it was written.

Each seat file carries a deny list. Every seat denies `SlashCommand`,
`WebSearch`, `EnterPlanMode`, `ExitPlanMode`, `AskUserQuestion`,
`Bash(claude:*)` and `Bash(npx claude:*)`. The Lead also denies `LSP`; the
Peer also denies `Bash(herdr:*)`, so a Peer reaches the Lead and other Peers
only through recorded work notes and returns. A `Bash(...)` pattern is written
into the seat's `settings.json` deny list, because Claude Code drops the whole
Bash tool when a pattern sits on the agent file. The shipped patterns are a
floor: your copy can add to them but never remove one, and a seat file under
`<project>/.maestro/profiles` cannot carry `settings:` or `skills:` at all.

The project copy of the pack is managed and not committed for you. A
repository may version it, but agents must not edit it while its generation is
running. The digest in the Hub and project stores is what detects a change.

## What is durable

- current team generation and seat binding;
- immutable work objectives and acceptance contracts, states, notes, returns
  and acceptances;
- immutable decisions and their replacements;
- lifecycle state and minimal `who did what to which target and when`
  activity;
- the pinned Workspace Pack copy.

Chat and raw transcript are not records. Notes add context but cannot change a
work objective or acceptance contract. Changed scope requires new work, and a
settled choice is recorded with `decide` before it governs work.

## What is temporary

Herdr owns the live workspace and panes. The runtime pane is a foreground
process with no model. It subscribes to Herdr events and records stalls and
pane loss in the store as the actor `runtime`. It cannot take, return, accept
or decide. Its directory lives under the OS temporary directory and is deleted
when the team stops.

## Pack changes and generations

A running generation is pinned to its project copy of the pack and to the
bytes of both seat files. Editing the Hub pack does not change a running team.
Editing a seat file under `~/maestro/profiles/` while a generation runs makes
the Lead's next `work add --to` and the next `team start` restore fail with
`SLP_SNAPSHOT_CHANGED` until the bytes are put back. Make those edits between
generations: stop the team, edit, and start it again.

Starting an identical running team verifies it and restores a missing seat
without creating duplicates. A changed objective or seat model is refused
with `TEAM_RUNNING` until the current generation stops.

The Hub stops a team once every work item is `DONE`:

```sh
cd ~/maestro
maestro team stop <team-id> --reason "<closing report>"
```

The pack copy and the durable records remain after stop. Raw transcript and
runtime files do not.

## Files SLP does not manage

`team start` does not rewrite the project's `AGENTS.md` or `CLAUDE.md`, and it
does not copy a skills tree into the project. The Workspace Pack plus the two
seat files are the whole generation contract, and the rendered seats live
under `~/.maestro/claude/` and `~/.codex/`.
