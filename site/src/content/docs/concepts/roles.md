---
title: Roles
description: The Human, Supervisor, Lead, Advisor, Observer, and Peer authority model used by SLP.
---

Read the complete role contract from the installed runtime:

```sh
maestro recipe show slp
```

## Human

The Human owns purpose, priority, risk, and every external effect, including
push, publish, deploy, send, spend, and delete. The Human creates, replaces,
and revokes the Supervisor and Leads, and accepts at the owner boundary.

## Supervisor

The Supervisor lives in `~/maestro` and is the owner's embodiment across
registered projects. It filters attention, governs in the owner's name, and
carries Human authority in full: every authority listed for the Human above is
the room's to exercise, the external effects included. It normally works
through each project's Lead, and does not dispatch a Peer directly, edit
project code, or accept a technical candidate.

Full authority includes intervening in any team to stop or correct an error:
freezing work, overriding or superseding a team decision, redirecting or
replacing a `supervisor-<team>` or a Lead, and ordering a correction. A code
correction still goes through that team's Lead and its lanes unless the room
explicitly takes a lane over.

Every external effect runs only behind the room's gate: push, tag, release,
publish, deploy, `maestro update`, remotes, deletion, machine config. The gate
is a locked room decision that names the exact candidate and the verified
evidence, never straight from a Lead's prompt, with the command and its output
recorded. The authority is the room's; the gate is what makes it safe to hold.

The room has exactly one Supervisor. `~/maestro/IDENTITY.md` binds its owner,
project scope, reporting target, observation boundary, raw transcript access,
write authority, acceptance authority, recovery or replacement lease, the
external-effect gate, and review date. Raw transcript access stays denied: the
room reads stores and handbacks, not panes.

Write authority and acceptance authority are soft-audited, and so is the gate:
nothing in the runtime refuses a push from a room that skipped its decision.
The binding is the contract, not a gate in code.

The installer also denies Claude's `Agent` and `Task` tools in the Supervisor
room. Codex has no equivalent hook and remains bound by the role contract.

## Lead

A session started in a repository working tree is that repository's Lead. The
Lead owns the project outcome, problem framing, the smallest sufficient
topology, one write owner per moving scope, dependencies, integration,
candidate identity, verification strategy, and engineering acceptance inside
the Human lease.

A scope has exactly one active Lead. A large project may have several scopes,
each with its own Lead, but the root Lead owns integration and release. A scope
dependency travels as a work item and handback, not as a second Lead on the
same moving scope.

## Peer

A pane the Lead opened with a dispatch becomes a Peer when it accepts that
stored contract. The Peer owns independent judgment or bounded delivery and
proof for its own writes. It does not own topology, scope beyond the
assignment, project acceptance, or external effects.

A Peer may return `DONE`, `BLOCKED`, `UNTESTABLE`, `UNKNOWN`, `FAILED`,
`CHALLENGE`, `REOPEN_REQUEST`, `DEPENDENCY_REQUEST`, or `COUNCIL_REQUEST`.
Unknown and partial results stay explicit; they are never rounded up to
acceptance.

## Teams

A managed team generation is one deterministic Herdr workspace named
`team-<team>-g<generation>`, backed by a Room-ledger snapshot and receipts. A
session reads membership from that generation's inspected workspace identity,
never from cwd. Cwd decides which project store a command reads; it is not
readiness evidence.

| name | what it is | writes? |
|---|---|---|
| `supervisor-<team>` | the active generation's authorizer and record holder | yes, records |
| `lead-<repo basename>` | Lead of that repository | yes, in that repository |
| `observer-<team>` | judgment for bounded sensor packets and one packet-capability verdict | only the scoped review verdict |
| foreground sensor | deterministic evidence collector in its own pane; not a model seat | only packet delivery through the lifecycle service |
| temporary Advisor | one decision-focused consultation started and stopped by TeamRuntime | no project authority |
| `peer-<dispatch id>` | one bounded assignment | inside its mutation boundary |

Advisor and dispatch Peers are not baseline readiness members. Advisor exists
only for a bounded request from the team Supervisor or registered Lead, returns
one recommendation, and stops at the declared condition. It receives no work,
lease, decision, reconcile, or store authority.

Sensor and judgment are separate. The foreground sensor evaluates the fixed
semantic triggers and sends capped, deduplicated evidence packets. Observer
receives those packets, not a continuous whole-team transcript. It cannot
start or stop roles, change assignments, repair resources, or use general
store verbs; it can submit at most one finding through the packet's live
`team review raise` capability. `supervisor-<team>` clears that finding with
rationale or escalates it into a separate `DEGRADED` health receipt.

The team's baseline is usable only at `ACTIVE / READY / CLEAR`, whose derived
verdict is `OPERABLE`. `REVIEW_HOLD` stops consequential acceptance,
`DRAINING` permits only bounded return/release/recovery work, and every
non-active stage is `CLOSED`. Status is the last snapshot; health and
await-ready perform fresh runtime inspection. Inspection never silently
restarts a failed role.

While active, `supervisor-<team>` authorizes routine reconcile, review
resolution, and stop. A foreground Room session executes runtime mutations so
receipts distinguish `requestedBy` from `executedBy`. Room emergency override
is rejected while the team Supervisor is reachable; proved absence or explicit
owner intervention requires separate reason and evidence.

Between a team and the room there is one channel, and it runs upward:
`supervisor-<team>` reports to the room with
`herdr agent prompt supervisor "[from supervisor-<team>][report|ask|done w<room-id>] ..."`.
Leads, advisors, observers and peers never prompt the room, and the room reaches
a team only through its `supervisor-<team>`, except for a Lead it opened and
still owns. A misrouted report fails closed: a supervisor answers a
`[from lead]` prompt from a Lead it does not own with the single line
`not my supervisor: send to supervisor-<team>`, neither verifying nor recording
it, because absorbing it would leave that team's record holder never learning
the work closed.

The room at `~/maestro` is its own workspace and opens no team agent there.
TeamRuntime owns generation-scoped workspace, role, bootstrap prompt, sensor,
inspection, reconcile, and shutdown behavior. Pane labels or a legacy
workspace without a lifecycle generation remain unmanaged and cannot establish
readiness.

The complete lifecycle and commands are in
[Supervised teams](/guides/supervised-teams/).

## SLP topology

```mermaid
flowchart TB
  Human --> Supervisor
  Human --> Lead
  Supervisor -. "owner authority through Lead" .-> Lead
  Lead --> PeerA["Peer: bounded scope A"]
  Lead --> PeerB["Peer: bounded scope B"]
  Lead --> PeerC["Peer: independent review"]
```

The topology has one write owner per moving scope. The Supervisor is not the
writer or the technical acceptance owner while a Lead holds the scope; when it
intervenes it says so and takes the lane over rather than writing beside the
Lead. Peers do not create sub-topology unless
the assignment grants it, and Human decisions or scope changes reach Peers
through the Lead. Two sessions holding parent work in one repository are
split-brain; the later session stops and reads `maestro status`.

## Lead handoff

A Lead continues or is replaced only through a frozen packet at a bounded stop
point. The packet records the objective, scope, current state, current write
owner, accepted decisions, failed approaches, successful patterns, evidence
index, active risks and blockers, and exact resume point. A narrative summary
without those fields is incomplete.

Each receipt is drafted as
`maestro decision draft "<receipt> <bundle-id>" --work <id>` and then locked.
Its literal first token is `packet_ready`, `successor_authorized`,
`successor_acknowledged`, or `predecessor_released`.

```mermaid
flowchart LR
  Ready[packet_ready] --> Authorized[successor_authorized]
  Authorized --> Acknowledged[successor_acknowledged]
  Acknowledged --> Released[predecessor_released]
```

The outgoing Lead records `packet_ready`. The owner, through the Supervisor,
records `successor_authorized`; the successor may reject an incomplete packet
before recording `successor_acknowledged`; the owner then records
`predecessor_released`. The predecessor stops writing at release.
Packet completeness, receipt order, and break-before-make are soft-audited.
