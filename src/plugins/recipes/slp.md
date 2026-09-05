# SLP v2: the direct supervised team

This recipe is the short operating guide for SLP. The canonical role contract
is the managed Workspace Pack at `~/maestro/SLP.md`. `maestro team start`
copies those exact bytes to `<project>/.maestro/SLP.md` and pins their version
and SHA-256 for one team generation. A running generation never changes when
the Hub copy changes.

## Topology

```text
Hub Supervisor <-> Team Supervisor
                         <-> Lead
                         <-> Peer 1
                         <-> Peer 2
                  Lead  <-> Peer 1
                  Lead  <-> Peer 2
                Peer 1  <-> Peer 2
```

Every displayed edge is a direct conversation channel. In the supported SLP
flow, the Hub Supervisor reaches a team only through its Team Supervisor. The
Team Supervisor, Lead, and Peers may talk directly within the team workspace.
When you prompt a pane by hand (`herdr agent prompt`), open every prompt with a
plain lowercase sentence, never a word a harness could read as a slash command,
and confirm `agent_status=working` before leaving: a dropped brief looks
identical to a slow start.

Attention has two layers. The first is the self-declared `maestro work note
<id> "<what you need>" --blocked`, which Maestro pushes one seat up. The
second is the team runtime pane (Hub d96, d97): `maestro team start` opens
it beside the Team Supervisor through Maestro's Herdr plugin; it holds the
generation's one Herdr event subscription and resolves every pane event
against the store, with no model judging. A `blocked` pane becomes a
`stall:dialog` entry on the item the seat holds, an idle pane holding ACTIVE
work becomes `stall:silence`, both recorded by the actor `runtime` and pushed
as `[from runtime][<id>] <kind> <evidence>; stop and run: maestro work note
<id> "<what you need>" --blocked` to the stuck pane with a copy to the Team
Supervisor, once per item and kind until the store changes. An idle seat with
nothing to do wakes the seat above with `[attention] <seat> idle`; a role pane
that exits or closes is noted on the team card and wakes the Team Supervisor.
There is no Observer, Advisor, sensor, scheduler, review, or reconcile role
in SLP.

SLP is a cooperative-agent protocol, not a shell security sandbox. Maestro
checks the nine SLP operations at their supported boundaries: Hub operations
must run from the Hub room, while project role operations require the current
generation's stored Herdr pane binding. It does not block native commands,
administrative Maestro commands, or direct Herdr calls; topology and
external-effect limits remain obligations enforced by the Human and host
policy.

## Seat profiles

Every seat is launched as a native harness profile rendered from a
maestro-owned source: `claude --agent maestro-<name>` or
`codex --profile maestro-<name>`. The Workspace Pack names one profile per
seat (`<!-- slp:profile:team-supervisor=team-supervisor -->`, `lead`, `peer`);
the profile is a markdown file (YAML frontmatter `harness`, `model`, `effort`,
`permission` or `sandbox`, `autocompact`, `disallowed_tools`, `description`;
body = the mandate) looked up in `<project>/.maestro/profiles/`, then
`~/maestro/profiles/`, then the shipped copies. `maestro install` renders
every resolvable profile into `~/.claude/agents/maestro-<name>.md`,
`~/.codex/maestro-<name>.config.toml` and `~/.codex/agents/maestro-<name>.toml`;
a seat whose render is missing fails with `PROFILE_NOT_INSTALLED` before any
pane opens. The Team Supervisor and Lead change through a shadowing file
(`~/maestro/profiles/lead.md`); a Peer variant is
`maestro team start --peer-profile <name>` for the generation or
`maestro work add "<objective>" --to <peer> --profile <name>` for one Peer,
and `--to peer-<name>` where `<name>` is a profile composes shared contract +
Peer mandate + that body. A generation pins the pack and every profile it
referenced; editing one mid-generation is refused like a pack edit.

## Roles

### Hub Supervisor

Starts a team, reads cross-team status, records owner or cross-team decisions,
and may emergency-stop a team. It does not manage the Lead or Peers directly.

### Team Supervisor

Owns team coordination and accepts Lead work. It communicates with the Hub,
Lead, and every Peer.

### Lead

Owns technical coordination and accepts Peer work. It creates bounded Peer
work with `maestro work add "<objective>" --to <peer-name>`; that operation
reuses or lazily opens the named Peer.

### Peer

Takes only assigned work, records material notes, and returns results with
proof, blockers, and residual risk in the return body. A Peer never accepts
its own work and never records a settled team decision.

## Public SLP operations

SLP roles use exactly nine operations:

```text
maestro team start
maestro team stop
maestro status [work-id]
maestro work add
maestro work take
maestro work note
maestro work return
maestro work accept
maestro decide
```

Flags configure one of these operations; they are not separate tools. Other
Maestro verbs remain available for repository administration and development,
but are not part of the SLP role toolbelt.

## Work and decisions

Work has exactly four states:

```text
OPEN -> ACTIVE -> RETURNED -> DONE
```

The assignee takes `OPEN` work. The reviewer accepts the return: Lead accepts
Peer work and Team Supervisor accepts Lead work. Rework requires that same
reviewer to run `maestro work note <id> "<specific gap>" --rework`; the grant
belongs only to the current return revision and is consumed when the same
assignee takes it once. Blocked work is returned with a blocker; there is no
`BLOCKED` state. A work item's objective and acceptance contract are immutable.
`maestro work note` adds context only. Changed scope requires a new work item; the
reviewer may close the superseded `OPEN` or `RETURNED` item with
`maestro work accept --outcome cancelled`.

`maestro decide` records one settled immutable choice. Technical scope belongs
to the Lead, team scope to the Team Supervisor, and owner or cross-team scope
to the Hub Supervisor. In a team workspace, `--work wN` links local work. At
Hub, a unique `wN` resolves directly; when several teams contain that id, use
`--work <team-id>:wN`. A later choice replaces an earlier one explicitly with
`--replaces`.

## Runtime pane

The runtime pane is a non-agent process Maestro opens per generation as the
`runtime` entrypoint of its Herdr plugin (`maestro install` links the plugin;
`maestro slp runtime` is its command). It renders the team's pane output on
every event, records stalls and pane loss in-process as the actor `runtime`,
and queues a wake for a seat that is still working until that seat turns
idle. `maestro slp status` reads its pending wakes; a repeated `maestro team start`
reopens it when it is gone; `maestro slp restore`, the plugin's startup hook,
re-attaches it after a Herdr restart; `maestro slp event`, the plugin's
`pane.exited` and `pane.closed` hook, records a role pane loss when no runtime
is subscribed. Its lock and state live in a temporary directory that team
stop deletes. It never takes, returns, accepts, or decides.

## Lifecycle

The Hub Supervisor starts a team from `~/maestro`:

```sh
maestro team start /absolute/project/path "<observable objective>"
```

Start creates one generation-scoped Herdr workspace, exactly one Team
Supervisor, exactly one Lead, and one initial `OPEN` work item for the Lead.
Repeating the same start verifies and repairs required runtime roles without
creating duplicates. A changed objective or peer profile is rejected until
stop.

Normal `maestro team stop <team-id>` changes nothing while unfinished work
exists. After all work is `DONE`, it closes Peers, Lead, the runtime pane and
its temporary directory, Team Supervisor, then the workspace. The Team Supervisor hands
this self-closing sequence to one transient foreground non-agent helper pane
in the Hub; this adds no role or public operation. The team becomes `STOPPED`
only after the workspace is absent. A partial close leaves it `RUNNING`, and
repeating the same stop continues cleanup. Hub may use
`maestro team stop <team-id> --emergency --reason "<why>"`; unfinished work
retains its four-state value but gains immutable abandonment actor, reason,
generation, and time metadata.

The project pack snapshot and durable stores remain after stop. Chat and raw
transcript do not become authority. Record context with `maestro work note`,
settled choices with `maestro decide`, and changed contracts as new work.

## Retired verbs and legacy rows

Retired SLP verbs fail with `SLP_V2_CUTOVER` naming the replacement
operation. Rows written before SLP v2 stay readable as legacy history and
never enter the four-state work model.
