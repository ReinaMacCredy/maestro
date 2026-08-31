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

Every displayed edge is a direct conversation channel. The Hub Supervisor
reaches a team only through its Team Supervisor. The Team Supervisor, Lead,
and Peers may talk directly within the team workspace.

There is no Observer, Advisor, sensor, scheduler, daemon, packet, review, or
reconcile role in SLP.

## Roles

### Hub Supervisor

Starts a team, reads cross-team status, records owner or cross-team decisions,
and may emergency-stop a team. It does not manage the Lead or Peers directly.

### Team Supervisor

Owns team coordination and accepts Lead work. It communicates with the Hub,
Lead, and every Peer. It may open the optional Watch Pane with Herdr pane
control when continuous raw-output visibility is useful.

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

The assignee takes `OPEN` or returned work. The reviewer accepts the return:
Lead accepts Peer work and Team Supervisor accepts Lead work. Rework is a
reviewer note followed by the same assignee taking `RETURNED` work again.
Blocked work is returned with a blocker; there is no `BLOCKED` state.

`maestro decide` records one settled immutable choice. Technical scope belongs
to the Lead, team scope to the Team Supervisor, and owner or cross-team scope
to the Hub Supervisor. In a team workspace, `--work wN` links local work. At
Hub, a unique `wN` resolves directly; when several teams contain that id, use
`--work <team-id>:wN`. A later choice replaces an earlier one explicitly with
`--replaces`.

## Watch Pane

Watch is an optional foreground, non-agent process in the team workspace. It
labels and refreshes currently available raw output from the Team Supervisor,
Lead, and Peers. It has no model, prompt, store write, gate, intervention, or
decision authority. Its rolling transcript is runtime-only and is deleted
when the team stops. Watch failure never blocks work.

## Lifecycle

The Hub Supervisor starts a team from `~/maestro`:

```sh
maestro team start /absolute/project/path "<observable objective>"
```

Start creates one generation-scoped Herdr workspace, exactly one Team
Supervisor, exactly one Lead, and one initial `OPEN` work item for the Lead.
Repeating the same start verifies and repairs required runtime roles without
creating duplicates. A changed objective or model configuration is rejected
until stop.

Normal `maestro team stop <team-id>` changes nothing while unfinished work
exists. After all work is `DONE`, it closes Peers, Lead, Watch and its
transcript, Team Supervisor, then the workspace. The Team Supervisor hands
this self-closing sequence to one transient foreground non-agent helper pane
in the Hub; this adds no role or public operation. The team becomes `STOPPED`
only after the workspace is absent. A partial close leaves it `RUNNING`, and
repeating the same stop continues cleanup. Hub may use
`maestro team stop <team-id> --emergency`; unfinished work retains its state.

The project pack snapshot and durable stores remain after stop. Chat and raw
transcript do not become authority; record material changes with
`maestro work note` or `maestro decide` before acting on them.

## Hard cut

SLP v2 does not wrap or dual-write the previous lifecycle. Removed SLP verbs
fail with the corresponding new operation. Previous rows remain readable as
legacy history and never enter the four-state work model.
