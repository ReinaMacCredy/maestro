---
title: SLP setup and storage
description: Set up one supervised team and understand which state belongs to the Hub, the project, and the runtime.
---

SLP uses one canonical Workspace Pack, one Hub store, one project store, and
one ephemeral Herdr workspace per running generation. The split is deliberate:
the Hub knows which teams exist, while each project owns its own work and
decisions.

SLP is a cooperative-agent protocol, not a shell security sandbox. Maestro
checks the nine SLP operations at their supported boundaries: Hub operations
must run from the Hub room, while project role operations require the current
generation's stored Herdr pane binding. It does not block native commands,
administrative Maestro commands, or direct Herdr calls; topology and
external-effect limits remain obligations enforced by the Human and host
policy.

## Before you start

Install Maestro from a checkout or with the installer, then install Herdr as
described in [Install](/getting-started/install/). A healthy installation has a
Supervisor room at `~/maestro` and the canonical pack at `~/maestro/SLP.md`.

Run the read-only checks from a project:

```sh
maestro version
maestro doctor
```

## Start a team

The Hub Supervisor starts a team from `~/maestro`:

```sh
cd ~/maestro
maestro team start /absolute/path/to/project "<observable objective>"
```

That single operation:

1. reads the exact bytes of `~/maestro/SLP.md`;
2. records the pack version and SHA-256 in the Hub store;
3. copies the pack to `<project>/.maestro/SLP.md`;
4. creates one generation-scoped Herdr workspace;
5. opens the Team Supervisor and Lead with only their relevant pack sections;
6. creates the initial `OPEN` work item assigned to the Lead.

Both `team start` and `work add --to` block while a pane opens and acknowledges
its contract, normally under a minute, and print their phases on stderr
(`starting`, `waiting for acknowledgement (up to 30s)`, `ready in`); do not
re-run either while it is still running. Every pane must already trust the
project directory: the first start in a fresh directory fails with
`TRUST_DIALOG` naming the harness and directory. Open that directory once with
that harness, accept its trust dialog, and rerun. Repeating the same
`team start` against a running generation touches only what is missing: roles
whose pane is still alive are left alone (`already acknowledged in <pane>; left
alone`), closed panes are reopened and acknowledged again, and the START record
in both stores is refreshed to the current pane ids.

Hand-offs push a wake-up: after `work return`, `work accept`, and `work note
--rework` commit, Maestro sends one line to the counterpart pane through Herdr
(`[from lead][w1 RETURNED] <summary>; read: maestro status w1`). The store
stays the truth; a push that fails prints a warning and nothing else changes.
The Team Supervisor closes with `maestro team stop <team-id> --reason
"<report>"`: the Hub sees `<team> g<n> STOPPED (supervisor): <report>` in
`maestro status`, and the same line is pushed to the Hub only when its room
agent is named `supervisor`.

The printed result contains the team generation and initial work ID. The Lead
takes that work from its own pane:

```sh
maestro work take <work-id>
```

## Storage map

| Location | Owner | What it stores | Lifetime |
| --- | --- | --- | --- |
| `~/maestro/SLP.md` | Hub owner | Canonical shared contract, the profile marker per seat, Hub Supervisor section, Watch rules | Seeded only when absent; install and update preserve owner edits; edits affect the next generation only |
| `~/maestro/.maestro/maestro.db` | Hub | Team ID, project path, generation, pack version/digest, runtime role identities, owner/cross-team decisions, lifecycle and minimal activity | Durable |
| `<project>/.maestro/SLP.md` | Project | Exact managed snapshot used by the active generation | Remains after stop; replaced at the next start |
| `<git-common-root>/.maestro/maestro.db` | Project | Checkout-scoped team bindings and roles, work, notes, returns, acceptances, team/technical decisions and minimal activity | Durable and shared by linked worktrees; every read is filtered to the current checkout |
| `<project>/.maestro/profiles/`, `~/maestro/profiles/` | Project, Hub owner | Profile files (frontmatter + mandate) that shadow the shipped seat, council and node profiles by name | Durable; a running generation pins the ones it references |
| `~/.claude/agents/maestro-*.md`, `~/.codex/maestro-*.config.toml`, `~/.codex/agents/maestro-*.toml` | `maestro install` | Rendered launch bundles for every resolvable profile; only `maestro-*` files are written or removed | Rewritten by every install, removed by uninstall |
| Herdr workspace `slp-<team>-g<n>` | Runtime | Team Supervisor, Lead, Peers and the optional Watch Pane | Exists only while the generation runs |
| `<OS temp>/maestro-slp-<uid>/<project-hash>/<team>/g<n>/` | Runtime | Rolling labelled Watch output and generation temporary data | Temporary; deleted at team stop |

Never edit either SQLite store by hand. Use Maestro operations so current state
and minimal activity remain in the same transaction.

### Seat profiles

`~/maestro/SLP.md` is pack version 3 and opens with one profile marker per
seat:

```
<!-- slp:version=3 -->
<!-- slp:profile:team-supervisor=team-supervisor -->
<!-- slp:profile:lead=lead -->
<!-- slp:profile:peer=peer -->
```

A profile is one markdown file: YAML frontmatter (`harness: claude|codex`,
`model`, `effort: low|medium|high|xhigh`, `permission` or `sandbox`,
`autocompact`, `disallowed_tools`, `description`) and a body that is the
seat's mandate. Lookup is `<project>/.maestro/profiles/<name>.md`, then
`~/maestro/profiles/<name>.md`, then the shipped copy; the first hit wins, so
a Lead on Claude Opus is a `~/maestro/profiles/lead.md` shadow, not a flag.
`maestro install` renders every resolvable profile into
`~/.claude/agents/maestro-<name>.md` (`claude --agent maestro-<name>`),
`~/.codex/maestro-<name>.config.toml` (`codex --profile maestro-<name>`) and
the Codex sub-agent file `~/.codex/agents/maestro-<name>.toml`; a seat profile
renders shared contract + mandate, any other profile also renders as
`maestro-peer-<name>` (shared contract + Peer mandate + its body) for
`work add --to peer-<name>`. `team start --peer-profile <name>` picks the Peer
profile for one generation; `work add --to <peer> --profile <name>` picks it
for one Peer. A version-2 pack (`slp:model` markers) fails `team start` with
`INVALID_SLP_PACK` naming the marker change; a marker naming a profile that
does not exist fails with `PROFILE_NOT_FOUND`; a profile whose render is
missing fails with `PROFILE_NOT_INSTALLED` naming `maestro install`.

The project snapshot is managed, inspectable and not automatically committed.
A repository may version it as project policy, but agents must not edit it
while its generation is running. The digest in the Hub and project binding is
what detects drift.

## What is durable

Durable state is intentionally small:

- current team generation and role binding;
- immutable work objectives and acceptance contracts, states, notes, returns and acceptances;
- immutable decisions and their replacements;
- lifecycle state and minimal `who did what to which target and when` activity;
- the pinned Workspace Pack snapshot.

Chat and raw transcript are not durable authority. Notes preserve context but
cannot change a work objective or acceptance contract. Changed scope requires
new work; settled choices are recorded with `decide` before they govern work.

## What is temporary

Herdr owns the live workspace and panes. The optional Watch Pane is a
foreground, non-agent multiplexer. It has no model, prompt, authority, store
write or intervention behavior. Its directory is runtime-owned under the OS
temporary directory, is not an archival contract, and is deleted when the team
stops.

## Pack changes and generations

An active generation is pinned to its project snapshot. Editing the Hub pack
does not change a running team. Stop the current generation, then start the
team again to materialize the new bytes as a new generation.

Starting an identical running team verifies it and restores a missing required
role without creating duplicates. A changed objective or peer profile is
rejected until the current generation stops, and so is an edit to any profile
the generation pinned.

Normal stop requires every work item to be `DONE`:

```sh
maestro team stop <team-id>
```

The snapshot and durable records remain after stop. Raw transcript and runtime
resources do not.

## Files SLP does not manage during normal work

`team start` does not rewrite project `AGENTS.md` or `CLAUDE.md`, and it does
not copy a skills tree into the project. The Workspace Pack plus the profile
files it names are the complete generation contract, and the rendered
`maestro-*` launch bundles live under the harness directories in `$HOME`.
