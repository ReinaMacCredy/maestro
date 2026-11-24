# SLP v2: the direct supervised team

This recipe is the short operating guide for SLP. The canonical role contract
is the managed Workspace Pack at `~/maestro/SLP.md`. `maestro team start`
copies those exact bytes to `<project>/.maestro/SLP.md` and pins their version
and SHA-256 for one team generation. A running generation never changes when
the Hub copy changes.

## Topology

```text
Hub Supervisor <-> Lead
          Lead <-> Peer 1
          Lead <-> Peer 2
        Peer 1 <-> Peer 2
```

Every displayed edge is a conversation channel. In the supported SLP flow, the
Hub Supervisor reaches a team only through its Lead, whose work it reviews. A
Peer reaches the Lead and other Peers through recorded work notes and returns,
which Maestro pushes for it, and its seat denies `herdr` (d847). When you prompt a
pane by hand (`herdr agent prompt`), open every prompt with a
plain lowercase sentence, never a word a harness could read as a slash command,
and confirm `agent_status=working` before leaving: a dropped brief looks
identical to a slow start. Write the brief to a file and send it as an expanded
variable, `mbrief <agent> <file>` from the room's shellrc, never as an inline
double-quoted argument: zsh substitutes commands inside double quotes, so a
brief describing a command runs it on the sender's machine and then vanishes
with a parse error (l26). An instruction that must land before the recipient's
next action goes on a waking operation - `maestro work add`,
`maestro work return`, `maestro work accept`, `maestro work note --rework` -
never on a plain note, and a pane inside a tool call reads nothing until that
call returns (l15).
A Peer
whose agent process is interrupted mid-slice keeps everything that matters,
its files on disk and its lease in the store; recover it with
a resume brief pointing at its own working tree, never a new dispatch and never
a reconstruction.

Attention has two layers. The first is the self-declared `maestro work note
<id> "<what you need>" --blocked`, which Maestro pushes one seat up. The
second is the team runtime pane (Hub d96, d97): `maestro team start` opens
it in the team workspace through Maestro's Herdr plugin; it holds the
generation's one Herdr event subscription and resolves every pane event
against the store, with no model judging. An idle pane holding ACTIVE work
becomes a `stall:silence` entry on that item, recorded by the actor `runtime`
and pushed as `[from runtime][<id>] silence <evidence>; stop and run: maestro
work note <id> "<what you need>" --blocked` to the stuck pane with a copy to
the Hub Supervisor, once per item until the store changes. An idle seat with
nothing to do wakes the seat above with `[attention] <seat> idle`; a role pane
that exits or closes is noted on the team card and wakes the Hub Supervisor.
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
seat (`<!-- slp:profile:lead=lead -->`, `peer`);
the profile is a markdown file (YAML frontmatter `harness`, `model`, `effort`,
`permission` or `sandbox`, `autocompact`, `disallowed_tools`, `description`;
body = the mandate) looked up in `<project>/.maestro/profiles/`, then
`~/maestro/profiles/`, then the shipped copies. `maestro install` renders
every resolvable profile into `~/.claude/agents/maestro-<name>.md`,
`~/.codex/maestro-<name>.config.toml` and `~/.codex/agents/maestro-<name>.toml`;
a seat whose render is missing fails with `PROFILE_NOT_INSTALLED` before any
pane opens. The Lead changes through a shadowing file
(`~/maestro/profiles/lead.md`); a Peer variant is
`maestro team start --peer-profile <name>` for the generation or
`maestro work add "<objective>" --to <peer> --profile <name>` for one Peer,
and `--to peer-<name>` where `<name>` is a profile composes the pack's Peer
block + Peer mandate + that body. That Peer block is the pack's second shared
section (`<!-- slp:shared:peer:begin -->`): every Peer seat reads it in place of
the shared contract the other seats carry, so the words a Peer meets name no
seat and no rung (room d134); a Hub pack without it renders the Peer from its
own shared contract and `maestro install` says which block to copy. A seat's model is a launch-time choice
(`maestro team start --lead-model`, `--peer-model`, and
`maestro work add --to <peer> --model` for one Peer): it rides the launch line
and changes neither the profile nor its digest. A generation pins the pack and
the bytes of every seat profile it launches - `lead` and `peer` - whether or
not it ever opens a Peer; editing one mid-generation is refused like a pack edit, on that
team's next Lead `maestro work add --to <peer>` and on its own
`maestro team start` restore.

## Roles

### Hub Supervisor

Starts a team, reads cross-team status, records owner or cross-team decisions,
reviews and accepts Lead work, and stops a team. It does not manage Peers
directly.

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
Peer work and the Hub Supervisor accepts Lead work. Rework requires that same
reviewer to run `maestro work note <id> "<specific gap>" --rework`; the grant
belongs only to the current return revision and is consumed when the same
assignee takes it once. Blocked work is returned with a blocker; there is no
`BLOCKED` state. A work item's objective and acceptance contract are immutable.
`maestro work note` adds context only. `maestro work add` inside a team refuses
`--acceptance` and `--blocked-by` with `INVALID_OPTION` rather than accepting
them silently: SLP stores no acceptance field and has no gating concept, so
acceptance and ordering belong in the objective text. Both flags keep their
meaning outside a team. Changed scope requires a new work item; the
reviewer may close the superseded `OPEN` or `RETURNED` item with
`maestro work accept --outcome cancelled`.

`maestro decide` records one settled immutable choice. Technical scope belongs
to the Lead, and owner or cross-team scope to the Hub Supervisor. In a team workspace, `--work wN` links local work. At
Hub, a unique `wN` resolves directly; when several teams contain that id, use
`--work <team-id>:wN`. A later choice replaces an earlier one explicitly with
`--replaces`. Read one back by its own id with `maestro status <decision-id>`,
which returns its choice, why and scope, reaches an owner-scope ruling recorded
in the Hub store, and names the store each match came from; `maestro search`
finds a decision by its id or by words from its choice or why.

## Runtime pane

The runtime pane is a non-agent process Maestro opens per generation as the
`runtime` entrypoint of its Herdr plugin (`maestro install` links the plugin;
`maestro slp runtime` is its command). It renders the team's pane output on
every event, records stalls and pane loss in-process as the actor `runtime`,
and queues a wake for a seat that is still working until that seat turns
idle. `maestro slp status` reads its pending wakes; a repeated `maestro team start`
reopens it when it is gone; `maestro slp restore`, the plugin's startup hook,
re-attaches it after a Herdr restart. A team whose pane died is stopped by
hand with `maestro team stop <team-id> --emergency`. Its lock and state live in a temporary directory that team
stop deletes. It never takes, returns, accepts, or decides.

## Lifecycle

The Hub Supervisor starts a team from `~/maestro`:

```sh
maestro team start /absolute/project/path "<observable objective>"
```

Start creates one generation-scoped Herdr workspace, exactly one Lead, the
runtime pane, and one initial `OPEN` work item for the Lead.
Repeating the same start verifies and repairs required runtime roles without
creating duplicates. A changed objective or peer profile is rejected until
stop.

The Hub runs `maestro team stop <team-id> --reason "<closing report>"`, which
changes nothing while unfinished work exists. After all work is `DONE`, it
closes Peers, Lead, the runtime pane and its temporary directory, then the
workspace. The team becomes `STOPPED` only after the workspace is absent. A partial close leaves it `RUNNING`, and
repeating the same stop continues cleanup. Hub may use
`maestro team stop <team-id> --emergency --reason "<why>"`; unfinished work
retains its four-state value but gains immutable abandonment actor, reason,
generation, and time metadata.

The project pack snapshot and durable stores remain after stop. Chat and raw
transcript do not become authority. Record context with `maestro work note`,
settled choices with `maestro decide`, and changed contracts as new work.

## Replaced verbs and legacy rows

Inside a running team's role pane, a classic verb that an SLP operation
replaces fails with `SLP_V2_CUTOVER` naming that operation. Rows written before SLP v2 stay readable as legacy history and
never enter the four-state work model.
