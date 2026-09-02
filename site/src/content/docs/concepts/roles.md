---
title: Roles
description: The Hub Supervisor, Team Supervisor, Lead, Peer, and Observer authority model used by SLP v2.
---

SLP v2 has five roles. The Human remains the owner above the system, but is not
an additional team seat.

SLP is a cooperative-agent protocol, not a shell security sandbox. Maestro
checks the nine SLP operations at their supported boundaries: Hub operations
must run from the Hub room, while project role operations require the current
generation's stored Herdr pane binding. It does not block native commands,
administrative Maestro commands, or direct Herdr calls; topology and
external-effect limits remain obligations enforced by the Human and host
policy. Within those operations, `MAESTRO_SESSION_ID` and
`MAESTRO_SESSION_PID` do not grant an SLP role; a missing, mismatched, or
prior-generation pane binding fails closed.

## Topology

```mermaid
flowchart TB
  Human["Human owner"] --> Hub["Hub Supervisor"]
  Hub <--> Team["Team Supervisor"]
  Team <--> Lead["Lead"]
  Team <--> PeerA["Peer A"]
  Team <--> PeerB["Peer B"]
  Lead <--> PeerA
  Lead <--> PeerB
  PeerA <--> PeerB
  Observer["Observer"] -. nudge .-> Lead
  Observer -. copy .-> Team
```

Every double arrow is a direct conversation channel in the SLP protocol. The
dotted arrows are one-way nudges pushed by Maestro when the Observer records a
stall. The supported Hub flow has no Hub-to-Lead or Hub-to-Peer channel;
unrestricted Herdr itself is outside that guarantee.

## Authority

| Role | Owns | SLP operations |
| --- | --- | --- |
| Hub Supervisor | team creation, cross-team status, owner decisions, emergency stop | `team start`, emergency `team stop`, `status`, `decide` |
| Team Supervisor | team coordination and acceptance | `team stop`, `status`, `work add`, `work note`, `work accept`, `decide` |
| Lead | technical coordination and Peer acceptance | `status`, `work add`, `work take`, `work note`, `work return`, `work accept`, `decide` |
| Peer | bounded execution and independent judgment | `status`, `work take`, `work note`, `work return` |
| Observer | stall detection | `status`, `work note --stall` |

## Hub Supervisor

The Hub Supervisor lives in `~/maestro`. It starts teams, maintains the
canonical Workspace Pack, reads cross-team status and records owner or
cross-team decisions. It communicates with each team only through that team's
Team Supervisor.

External effects still require Human authority. Recording a decision does not
execute its native tool.

## Team Supervisor

The Team Supervisor is the team's record holder and acceptance owner. It
communicates directly with the Hub Supervisor, Lead and every Peer. It assigns
work to the Lead, accepts Lead returns and stops the team when all work is
done.

## Lead

The Lead owns technical decomposition, integration, verification strategy and
Peer acceptance. It communicates directly with Team Supervisor and every
Peer. A Lead may assign independent work to several Peers without making their
conversation dependent on a stored envelope.

## Peer

A Peer takes work assigned to its identity, records material notes and returns
results. It may speak directly to Team Supervisor, Lead and other Peers. It
does not accept its own work and does not settle team decisions.

## Observer

The Observer is a fourth team seat that `team start` opens after the Lead, on
a small model by default (`gpt-5.6-luna`; `--observer-model` overrides it). It
holds no work. A sentinel process in the team workspace sends it one bounded
packet every five minutes, and at once when a role pane turns blocked: the
open work with its latest entry, the recent tail of every role pane, and the
stall facts Maestro can compute (unchanged age, silence per pane, a repeated
tail line, the Herdr state). The Observer's only mutation is

```sh
maestro work note <work-id> "<evidence>" --stall repeat|silence|dialog
```

Maestro pushes a fixed line to the seat the item waits on and a copy to the
Team Supervisor, and stores without pushing a repeat of the same stall while
the store is unchanged. A nudge carries no code advice; the Observer never
takes, returns, or accepts work.

## Conversation and records

Natural conversation needs no record ID. Chat alone does not mutate work,
authority or accepted decisions. Record these changes before they govern:

- changed objective or acceptance: create new work; the existing contract is immutable;
- material context on the same contract: `maestro work note`;
- settled technical, team or owner choice: `maestro decide`;
- a fact only the seat above can settle, while keeping the item: `maestro work note --blocked`;
- completed or blocked execution: `maestro work return`;
- reviewer acceptance: `maestro work accept`.

## One acceptance boundary above the worker

```mermaid
flowchart LR
  Peer["Peer returns"] --> Lead["Lead accepts"]
  Lead --> Team["Team Supervisor accepts"]
```

This keeps execution and acceptance separate without adding review roles or a
second lifecycle.
