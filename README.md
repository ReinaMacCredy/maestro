# maestro

Maestro is a local-first coordination system for human and agent work. It keeps
durable work, decisions, sessions, evidence, dispatches, and handbacks in each
repository's shared Git root. It is written in TypeScript, runs on Bun, and does
not require a background service.

Version 0.108.0 is the first TypeScript release. It continues the version line
after 0.107.x, the final Rust release.

## Three layers

- **Mechanism kernel** owns the SQLite store, event log, sessions, CLI dispatch,
  plugin loading, and readiness projection. It does not impose workflow policy.
- **Plugins** provide verbs and optional policy gates. Repositories enable or
  disable policies such as proof and breakdown in `.maestro/config`.
- **Recipes and skills** provide prompt-first working methods as Markdown. Use
  `maestro recipe list` to browse recipes and `maestro recipe show <name>` to
  read one without copying it into a repository.

## Install, update, and remove

Maestro is distributed from source. From a Maestro checkout, run:

```sh
bun bin/maestro.ts install
maestro version
```

Install copies the runtime to `~/.maestro/runtime`, writes the shim at
`~/.local/bin/maestro`, records the source checkout in
`~/.maestro/source.json`, and wires the current repository. When replacing an
older executable, it preserves that executable as `maestro-legacy` if no
rollback executable already exists.

Install also scaffolds `~/maestro`, the Supervisor room, and registers the
current repository there. It materializes four managed skills under
`~/maestro/skills`: `maestro-bundle`, `maestro-design`, `maestro-work`, and
`maestro-verify`. The installer links those skills for Claude without
overwriting unmanaged skills.

Use `maestro update` to fetch the recorded source checkout, accept only a
fast-forward, and resync the runtime. It refuses dirty, diverged, missing, or
unreachable sources without partially updating the runtime. Use
`maestro install` from the source checkout for an offline resync.

Use `maestro uninstall` to remove Maestro-managed hooks, settings keys, and
mirror blocks from the current repository. It is idempotent and does not delete
repository data, the machine runtime, the shim, or the Supervisor room.

`maestro doctor` inspects the shim, runtime stamp, recorded source, repository
wiring, permissions, and store access without repairing them. A healthy report
exits zero; a reported problem names the next command when the repair is
mechanical.

## Roles and lanes

Maestro uses three durable agent roles:

- The **Supervisor** lives in `~/maestro`. It represents the owner across
  projects, filters attention, and works through each project's Lead. It does
  not edit project code, dispatch Peers directly, or accept technical work.
- A repository session is its **Lead**. The Lead owns the project outcome,
  contracts, topology, integration, and technical acceptance.
- A pane opened with a dispatch is a **Peer**. The Peer owns independent
  judgment or bounded delivery inside the stored contract, then returns a
  handback with layered evidence.

Read the full authority model with `maestro recipe show slp`.

Lanes are Herdr panes, not subprocess agents created by Maestro. A Lead stores
the lane contract with `maestro dispatch open`, the Peer takes it with
`maestro dispatch accept`, and the Peer returns a packet with
`maestro handback file`. Herdr owns pane creation, agent startup, prompting,
wake-up, and pane closure; Maestro owns the durable contract and evidence.
The room's `~/maestro/lane.md` contains the complete lane procedure.

The four lane types are `scout` for no-write discovery, `decision` for a
recommendation, `delivery` for bounded writes, and `challenge` for trying to
break a premise or candidate. Concurrent dispatches on one work item form a
council. The council stays sealed until every member returns, so no view can
bias another and work cannot begin on a partial result.

## Work, decisions, and evidence

- `maestro status` shows session identity, held work, and live peers;
  `maestro ready` shows work that can start and the gates blocking other work.
- `maestro work` manages work trees, dependencies, leases, notes, cancellation,
  claims, and proof.
- `maestro decision` records draft, locked, and superseded choices with their
  rationale and work links. Supersession takes effect when the replacement is
  locked, not while it is still a draft.
- `maestro dispatch` stores lane contracts and council state;
  `maestro handback` stores shape-checked return packets.
- `maestro search` searches native work, decisions, notes, events, bundles, and
  imported Rust records.

Failed commands emit a JSON error envelope on stderr and exit nonzero. Empty or
whitespace-only required arguments are rejected rather than interpreted as
missing identities or targets.

## Verb tour

- `maestro status` shows sessions and leases; `maestro ready` shows startable
  and gated work.
- `maestro work add|start|note|done|show|list` manages the work lifecycle.
- `maestro decision draft|lock|show|list` manages durable choices.
- `maestro dispatch open|accept|show|list` stores lane contracts, while
  `maestro handback file|show` stores and reads return packets.
- `maestro attention` scans the current repository and `maestro brief`
  summarizes every registered repository.
- `maestro recipe list|show` serves methods; `maestro plugin list|enable|disable`
  manages the configured extension set.
- `maestro import rust` imports preserved Rust data; `maestro legacy show`
  reads imported cards and files.
- `maestro install`, `maestro update`, `maestro uninstall`, and
  `maestro doctor` manage and diagnose the source-installed runtime.
- `maestro version` reports the package version and installed commit.

## Attention and brief

`maestro attention` computes current attention packets at read time. It detects
stalled leases, repeated failures, stale decisions, scope collisions,
unreturned dispatches, and returned handbacks that have not been reviewed. It
records no mailbox message and runs no daemon.

`maestro brief` reads the registry in `~/maestro/registry`, opens each project
in observer mode, and reports only what needs attention. Missing repositories
are named and skipped. When every registered project is running normally, the
brief says so in one line. The `hm` shell function focuses the Supervisor room
and prints this brief; it does not start an agent.

## Observer mode

Set `MAESTRO_READ_ONLY=1` to run Maestro as an observer. Pure commands such as
status, search, recipes, and read-only list/show operations remain available.
Mutating commands fail with `READ_ONLY`; external plugins are not loaded; and
session, lease, and liveness state is not persisted. Search fails closed if its
index cannot be refreshed rather than returning stale results as current.

## Harness integration

`maestro install` writes managed adapters for Claude and Codex and merges only
the managed hook entries. `SessionStart` and `UserPromptSubmit` record the
session and print its current brief. Small managed blocks in `CLAUDE.md` and
`AGENTS.md` point agents to status, ready work, and recipes. No hook sends
mail, pushes a dispatch into another session, or delivers PostToolUse packets.

## Recipes, skills, and plugins

`maestro recipe list` and `maestro recipe show <name>` serve the shipped
Markdown methods. The four installed skills drive bundle, design, work, and
verification lifecycles. `maestro plugin` lists and manages built-in, global,
and repository plugins; policy plugins remain removable instead of being
baked into the kernel.

Use `maestro help` for the complete verb list and `maestro <verb> --help` for
the current syntax and flags.

## Rust-era data

The last Rust stores are preserved under `legacy/rust/`. Import the card store
read-only with:

```sh
maestro import rust --path legacy/rust/store.sqlite
```

Add `--promote` to create native work, decisions, and provenance notes:

```sh
maestro import rust --path legacy/rust/store.sqlite --promote
```

Promotion preserves card kinds, terminal outcomes, decision links,
supersession chronology, and receipt provenance. Orphan receipts are skipped
and counted. The `legacy_map` table makes repeated promotion idempotent.

Archived Rust snapshots can also be imported for search:

```sh
maestro import rust --path legacy/rust/archive-cards.sqlite
```

Bun decodes zstd snapshot payloads when possible and falls back to stored
search text when it cannot. Imported records remain available through
`maestro search` and `maestro legacy show <id>`. See
[`legacy/rust/README.md`](legacy/rust/README.md) for the preserved datasets and
their exact counts.
