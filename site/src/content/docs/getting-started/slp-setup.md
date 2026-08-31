---
title: SLP setup and storage
description: Set up one supervised team and understand which state belongs to the Hub, the project, and the runtime.
---

SLP uses one canonical Workspace Pack, one Hub store, one project store, and
one ephemeral Herdr workspace per running generation. The split is deliberate:
the Hub knows which teams exist, while each project owns its own work and
decisions.

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

The printed result contains the team generation and initial work ID. The Lead
takes that work from its own pane:

```sh
maestro work take <work-id>
```

## Storage map

| Location | Owner | What it stores | Lifetime |
| --- | --- | --- | --- |
| `~/maestro/SLP.md` | Hub | Canonical shared contract, role sections, model defaults, Watch rules | Durable; edits affect the next generation only |
| `~/maestro/.maestro/maestro.db` | Hub | Team ID, project path, generation, pack version/digest, runtime role identities, lifecycle and minimal activity | Durable |
| `<project>/.maestro/SLP.md` | Project | Exact managed snapshot used by the active generation | Remains after stop; replaced at the next start |
| `<git-common-root>/.maestro/maestro.db` | Project | Team binding, roles, work, notes, returns, acceptances, decisions and minimal activity | Durable and shared by linked worktrees |
| Herdr workspace `slp-<team>-g<n>` | Runtime | Team Supervisor, Lead, Peers and optional Watch Pane | Exists only while the generation runs |
| `<OS temp>/maestro-slp-<uid>/<project-hash>/<team>/g<n>/` | Runtime | Rolling labelled Watch output and generation temporary data | Temporary; deleted at team stop |

Never edit either SQLite store by hand. Use Maestro operations so current state
and minimal activity remain in the same transaction.

The project snapshot is managed, inspectable and not automatically committed.
A repository may version it as project policy, but agents must not edit it
while its generation is running. The digest in the Hub and project binding is
what detects drift.

## What is durable

Durable state is intentionally small:

- current team generation and role binding;
- work objectives, states, notes, returns and acceptances;
- immutable decisions and their replacements;
- lifecycle state and minimal `who did what to which target and when` activity;
- the pinned Workspace Pack snapshot.

Chat and raw transcript are not durable authority. A material objective,
acceptance or decision change must be recorded with `work note` or `decide`
before it governs work.

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
role without creating duplicates. A changed objective or model configuration
is rejected until the current generation stops.

Normal stop requires every work item to be `DONE`:

```sh
maestro team stop <team-id>
```

The snapshot and durable records remain after stop. Raw transcript and runtime
resources do not.

## Files SLP does not manage during normal work

`team start` does not rewrite project `AGENTS.md` or `CLAUDE.md`, and it does
not copy a skills tree or separate `lead.md`, `peer.md`, `observer.md`, or
`supervisor.md` files. The one Workspace Pack is the complete generation
contract.
