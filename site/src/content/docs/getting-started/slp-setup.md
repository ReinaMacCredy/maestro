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

Claude seats log in with one long-lived token. Create it once with
`claude setup-token` (a browser flow) and hand it to Maestro on stdin:

```sh
claude setup-token | maestro install --seat-token
```

`maestro doctor` then reports `seat token: present`; until it does, `team
start` refuses `SEAT_TOKEN_MISSING` for any Claude seat. See
[Seat config dirs](#seat-config-dirs-and-the-token).

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
5. opens the Team Supervisor and Lead as their rendered profiles
   (`claude --agent maestro-<name>` or `codex --profile maestro-<name>`);
6. opens the runtime pane beside the Team Supervisor and records the Hub
   Supervisor's pane as the target of the team's upward wakes;
7. creates the initial `OPEN` work item assigned to the Lead.

Both `team start` and `work add --to` block while a pane opens and acknowledges
its contract, normally under a minute, and print their phases on stderr
(`starting`, `waiting for acknowledgement (up to 30s)`, `ready in`); do not
re-run either while it is still running. A Claude seat trusts the project
directory because you started the team there: before its pane opens, Maestro
records the project as trusted in that seat's `.claude.json` (d853). A Codex
pane must already trust the directory: the first start in a fresh directory
fails with `TRUST_DIALOG` naming the harness and directory. Open that
directory once with that harness, accept its trust dialog, and rerun. Repeating the same
`team start` against a running generation touches only what is missing: roles
whose pane is still alive are left alone (`already acknowledged in <pane>; left
alone`), closed panes are reopened and acknowledged again, and the START record
in both stores is refreshed to the current pane ids.

Hand-offs push a wake-up: after `work add`, `work return`, `work accept`,
and `work note --rework` commit, Maestro sends one line to the counterpart
pane through Herdr (`[from lead][w1 RETURNED] <summary>; read: maestro status
w1`; a new assignment is `[from lead][w2 OPEN] ...` whether the Peer pane was
just opened or already acknowledged). The store stays the truth; a push that
fails prints a warning and nothing else changes. The Team Supervisor closes
with `maestro team stop <team-id> --reason "<report>"`: the Hub sees `<team>
g<n> STOPPED (supervisor): <report>` in `maestro status`, and the same line
is pushed to the Hub Supervisor pane `team start` recorded, or to a room
agent named `supervisor`.

The printed result contains the team generation and initial work ID. The Lead
takes that work from its own pane:

```sh
maestro work take <work-id>
```

## Storage map

| Location | Owner | What it stores | Lifetime |
| --- | --- | --- | --- |
| `~/maestro/SLP.md` | Hub owner | Canonical shared contract with the attention lines a seat may receive, the profile marker per seat, Hub Supervisor section | Seeded only when absent; install and update preserve owner edits; edits affect the next generation only |
| `~/maestro/.maestro/maestro.db` | Hub | Team ID, project path, generation, pack version/digest, runtime role identities, owner/cross-team decisions, lifecycle and minimal activity | Durable |
| `<project>/.maestro/SLP.md` | Project | Exact managed snapshot used by the active generation | Remains after stop; replaced at the next start |
| `<git-common-root>/.maestro/maestro.db` | Project | Checkout-scoped team bindings and roles, work, notes, returns, acceptances, team/technical decisions and minimal activity | Durable and shared by linked worktrees; every read is filtered to the current checkout |
| `<project>/.maestro/profiles/`, `~/maestro/profiles/` | Project, Hub owner | Profile files (frontmatter + mandate) that shadow the shipped seat, council and node profiles by name | Durable; a running generation pins the ones it references |
| `~/.maestro/claude/<seat>/` (`lead`, `peer`, `team-supervisor`) | `maestro install` | One `CLAUDE_CONFIG_DIR` per Claude seat kind: `agents/maestro-*.md` (the seat and, in `peer`, every `peer-*` render), `settings.json` (0600), `skills/` symlinks, `projects` and `plugins` symlinks to `~/.claude`, `.claude.json` | Rewritten by every install (the `.claude.json` only when absent; `team start` and `work add --to` add the project's trust key to it), removed by uninstall |
| `~/.maestro/claude/oauth-token` | `maestro install --seat-token` | The `claude setup-token` token every seat settings `env` carries (0600) | Kept by install, removed by uninstall |
| `~/.claude/agents/maestro-*.md`, `~/.codex/maestro-*.config.toml`, `~/.codex/agents/maestro-*.toml` | `maestro install` | Rendered launch bundles: bare node and council profiles for Claude (the subagent executor), every profile for Codex; only `maestro-*` files are written or removed | Rewritten by every install, removed by uninstall |
| Herdr workspace `slp-<team>-g<n>` | Runtime | Team Supervisor, Lead, Peers and the runtime pane | Exists only while the generation runs |
| `<OS temp>/maestro-slp-<uid>/<project-hash>/<team>/g<n>/` | Runtime | The runtime pane's lock and its pending-wake state | Temporary; deleted at team stop |
| `~/maestro/herdr-plugin.toml` | `maestro install` | The rendered Herdr plugin manifest; the room is linked as the plugin `maestro` so its hooks run from the Hub | Rewritten by every install, removed by uninstall |

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
`autocompact`, `disallowed_tools`, `skills`, `settings` (Claude seat profiles
only), `description`) and a body that is the seat's mandate. Lookup is
`<project>/.maestro/profiles/<name>.md`, then `~/maestro/profiles/<name>.md`,
then the shipped copy; the first hit wins, so a Lead on Claude Opus is a
`~/maestro/profiles/lead.md` shadow, not a flag. `maestro install` renders
every resolvable profile for Codex into `~/.codex/maestro-<name>.config.toml`
(`codex --profile maestro-<name>`) and the Codex sub-agent file
`~/.codex/agents/maestro-<name>.toml`, and for Claude into one agent file: the
three seats and every `peer-<name>` under the seat config dir
`~/.maestro/claude/<seat>/agents/maestro-<name>.md`, bare node and council
profiles under `~/.claude/agents/maestro-<name>.md` for the subagent executor
(`claude --agent maestro-<name>` in both cases). A seat profile renders shared
contract + mandate; any other profile also renders as `maestro-peer-<name>`
(shared contract + Peer mandate + its body, carrying the Peer deny list) for
`work add --to peer-<name>`. `team start --peer-profile <name>` picks the Peer
profile for one generation; `work add --to <peer> --profile <name>` picks it
for one Peer. `work add --to <peer> --fresh` reuses an acknowledged Peer pane
with a fresh harness context (Claude Code `/clear`, Codex `/new`, proven by a
new harness session, then the READY challenge again) before the OPEN wake; it
is refused while that Peer holds ACTIVE work or when the reset does not take
(`PEER_RESET_FAILED`, nothing added). A version-2 pack (`slp:model` markers) fails `team start` with
`INVALID_SLP_PACK` naming the marker change; a marker naming a profile that
does not exist fails with `PROFILE_NOT_FOUND`; a profile whose render is
missing fails with `PROFILE_NOT_INSTALLED` naming `maestro install`. The
three seats are the only profiles the pack names; the Hub Supervisor is the
owner's own agent in `~/maestro`, and there is no Observer seat or marker.

### Seat config dirs and the token

Every Claude seat runs under its own `CLAUDE_CONFIG_DIR`, rendered by
`maestro install` at `~/.maestro/claude/<seat>/` for `lead`, `peer` and
`team-supervisor` (d845). A seat therefore sees none of the owner's
`~/.claude`: no `CLAUDE.md`, no personal skills, no user settings or MCP
servers. The dir holds:

- `agents/`: the seat's rendered profile files; the `peer` dir also holds
  every `peer-<name>` render.
- `settings.json` (0600): the Maestro base merged with the profile's
  `settings:` overlay (a `null` value removes a key). The base is
  `permissions.defaultMode: bypassPermissions`,
  `skipDangerousModePermissionPrompt`, the Herdr `SessionStart` hook copied
  from your user settings (so `--fresh` still proves a new session), an `env`
  block with the seat token and the `CLAUDE_CODE_DISABLE_*` switches for
  workflows, cron, fast mode, bundled skills and git instructions,
  `autoMemoryEnabled: false`, `disableWorkflows`, empty `attribution` and
  `enabledPlugins: {}`. There is no `cleanupPeriodDays`: `projects` is shared
  and a seat start must never delete your transcripts (d849). The base also
  carries `claudeMdExcludes` for your `~/.claude/CLAUDE.md` and
  `~/.claude/rules/**` as absolute paths (d852): your home directory is an
  ancestor of every project under it, so that file would otherwise load into
  the seat as project instructions regardless of `CLAUDE_CONFIG_DIR`, and
  its `@` imports would raise the external-imports dialog in the pane.
- `skills/`: one symlink per name in the union of `skills:` across every
  profile rendered into the dir, resolved in `~/maestro/skills` then
  `~/.claude/skills`; an unknown name fails install naming the profile (d848).
  Shipped defaults: lead `maestro-work, maestro-design, maestro-council,
  maestro-graph, maestro-explore, maestro-diagnose`; peer `maestro-work,
  maestro-explore, maestro-diagnose, maestro-verify`; team-supervisor
  `maestro-work`.
- `projects` and `plugins`: symlinks to `~/.claude/projects` and
  `~/.claude/plugins`, so transcripts and plugins stay shared.
- `.claude.json`: written once with onboarding marked done and
  `mcpServers: {}`; Claude Code owns it afterwards. `team start` and
  `work add --to` add one key before a seat pane opens: the project is marked
  trusted (`projects[<project>].hasTrustDialogAccepted`) when it is not
  already, every other key kept, so a fresh seat never blocks on the
  workspace trust dialog (d853). A project's own external `CLAUDE.md`
  imports are still a question the owner answers in the pane.

Auth is one token from `claude setup-token`, handed to
`maestro install --seat-token` on stdin (d846). It lives in
`~/.maestro/claude/oauth-token` (0600) and in each seat settings `env` as
`CLAUDE_CODE_OAUTH_TOKEN`; it never appears in install output. Without it
install still renders and prints one warning, `maestro doctor` reports
`seat token: missing`, and `team start` or `work add --to` refuse
`SEAT_TOKEN_MISSING` for a Claude seat before any pane opens. The token lasts
a year; doctor prints the date it was written.

`team start` and `work add --to` create a Claude seat pane with
`CLAUDE_CONFIG_DIR=<dir>` in its environment (d850). A tab that already
carries the seat's label but sits at a shell prompt is closed and recreated
with the env rather than reused, since a shell's environment cannot be read or
set afterwards. A pane launched by hand without the env fails at
`herdr agent start` because the agent file exists only in the seat dir: it
never runs silently on your `~/.claude`. Codex seats are unchanged.

The shipped seat deny lists (d847, carried as `disallowed_tools`, shadowable)
take the seatworks set: every seat disallows `Agent`, `Task`, `Workflow`,
`SlashCommand`, `WebSearch`, `TodoWrite`, `EnterPlanMode`, `ExitPlanMode`,
`AskUserQuestion`, `Bash(claude:*)` and `Bash(npx claude:*)`; the Lead adds
`LSP`; the Peer adds `Bash(herdr:*)`, so a Peer reaches the Lead and other
Peers only through recorded work notes and returns. Every composed
`peer-<name>` render carries the Peer list as well (d851). A `Bash(...)`
pattern renders into the seat `settings.json` `permissions.deny` (unioned
across the profiles in that dir and with any `settings:` overlay deny), and
only bare tool names stay on the agent file's `disallowedTools` line, since
Claude Code drops the whole Bash tool when a pattern sits there (d854).

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

Herdr owns the live workspace and panes. The runtime pane is a foreground,
non-agent process that subscribes to Herdr events and records stalls and pane
loss in the store as the actor `runtime`. It has no model and no authority
to take, return, accept or decide. Its directory is runtime-owned under the
OS temporary directory, is not an archival contract, and is deleted when the
team stops.

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
