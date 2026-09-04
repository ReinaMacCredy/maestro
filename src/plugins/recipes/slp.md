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

The Observer is the only seat outside the work lifecycle: it reads sentinel
packets, may run `maestro status` and record `maestro work note --stall`, and
never holds work. There is no Advisor, sensor, scheduler, review, or
reconcile role in SLP.

SLP is a cooperative-agent protocol, not a shell security sandbox. Maestro
checks the nine SLP operations at their supported boundaries: Hub operations
must run from the Hub room, while project role operations require the current
generation's stored Herdr pane binding. It does not block native commands,
administrative Maestro commands, or direct Herdr calls; topology and
external-effect limits remain obligations enforced by the Human and host
policy.

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
