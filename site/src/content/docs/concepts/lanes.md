---
title: Lanes
description: Store a bounded Peer contract, deliver it through Herdr, and read the returned handback.
---

Coordination lanes are Herdr panes, never subprocess agents created by Maestro.
The Lead owns topology and delivery; Maestro owns the durable contract and
return packet.

## Lane types

- `scout` performs no-write discovery and reports state.
- `decision` investigates alternatives and recommends without writing.
- `delivery` owns bounded writes inside the declared mutation scope.
- `challenge` tries to break a premise or candidate and returns findings, not fixes.

## Open a dispatch

After the Lead creates the work item and opens an unwatched Herdr lane pane, it
stores the complete contract:

```sh
maestro dispatch open <work-id> --objective "<observable outcome>" --owned-scope "<paths or responsibility>" --excluded-scope "<explicit non-goals>" --mutation "<no-write or write-bounded paths>" --stop-condition "<done or blocked boundary>" --lane delivery --evidence-required "source: <falsifier>" --pane <pane-id> --target-session <session-id>
```

For a Codex pane whose session does not exist until its first turn, the Lead
opens the pane-bound dispatch without `--target-session`, sends the real stored
contract as the first prompt, then verifies the holder after acceptance.

The Lead reads the stored contract and work context before sending it:

```sh
maestro dispatch show <dispatch-id>
maestro dispatch list <work-id>
```

## Accept the lane

The Peer begins by accepting exactly the stored dispatch:

```sh
maestro dispatch accept <dispatch-id>
```

Acceptance binds the session to the Peer role without taking the work write
lease. A delivery Peer starts the assigned work separately when the contract
requires the write lease.

## Return a handback

The Peer stops at the contract boundary and files the complete return packet:

```sh
maestro handback file <dispatch-id> --status DONE --claim "<current belief>" --proof "source: <falsifier>" --assumptions "None" --residual-risks "None" --incidental-findings "None"
```

The Lead reads the handback, checks its evidence, decides whether the work is
complete, and only then closes or reuses the lane pane. A handback is a claim,
not acceptance.

## Herdr procedure

The full `~/maestro/lane.md` procedure has the Lead create an unwatched lane
tab, split panes, start the requested harness, resolve session identity, store
and send the exact dispatch, wait for `working`, then wait for any terminal
Herdr state. Both `idle` and `done` mean to read the handback; `blocked` needs
inspection. A wait that returns without a state is re-armed. No Maestro verb
pushes a brief into a pane or calls Herdr.

## Councils

Concurrent dispatches in one generation on the same work item form a council.
The council remains sealed until every lane returns, which prevents one view
from biasing another. The Lead reconciles the complete set; it does not count
votes. Implementation of the ruling belongs on separate work, not as a later
dispatch that silently changes council membership.
