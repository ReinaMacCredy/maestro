---
title: Team collaboration
description: How a Lead hands work to Peers, takes returns, asks for rework, and runs a blind design in an SLP team.
---

This page applies when the SLP plugin is enabled (`maestro plugin enable
slp`; see [SLP setup and storage](/getting-started/slp-setup/)).

An SLP team works through a small durable record: work items, notes, returns,
acceptances and decisions. There is no separate envelope, return packet or
council state.

## Record what changes

A work objective and acceptance condition are immutable; changed scope becomes
new work. Record context on the existing item and settled choices explicitly:

```sh
maestro work note <work-id> "<material context>"
maestro decide "<settled choice>" --why "<reason>" --work <work-id>
```

The Lead may also prompt a Peer pane by hand with Herdr, after it has recorded
what the prompt is about. A Peer talks to the Lead only through recorded notes
and returns.

## Assign and return work

`team start` creates the Lead's first item. The Lead opens its own further
items, or names a Peer to hand one off:

```sh
maestro work add "<objective>"
maestro work add "<objective>" --to peer-api
```

The assignee, the Lead or a Peer, then owns the execution cycle:

```sh
maestro work take <work-id>
maestro work note <work-id> "<material progress or changed fact>"
maestro work return <work-id> "<result; proof; blocker; residual risk>"
```

The reviewer accepts separately: the Lead for a Peer's item, the Hub
Supervisor for the Lead's.

```sh
maestro work accept <work-id>
```

## Rework and blockers

Returned work stays `RETURNED` until accepted. If it needs another pass, the
reviewer records the specific gap and the same assignee takes it again:

```sh
maestro work note <work-id> "<specific gap and acceptance condition>" --rework
maestro work take <work-id>
```

A holder that needs a fact from above keeps the item and records
`maestro work note <work-id> "<what I need>" --blocked`. A holder that cannot
continue returns the item with the blocker in the body; after the blocker is
resolved, the reviewer records `--rework` and the assignee retakes it once.
SLP has no `BLOCKED` or `REOPENED` state.

## Blind design

Blind design is a pattern the Lead runs, not runtime state:

1. Create separate work for two or three Peers with the same neutral question.
2. Ask each Peer to return independently, before seeing another view.
3. Compare all returns once every independent view is in.
4. Record the selected choice, rationale and dissent with `maestro decide`.
5. Create separate implementation work after the decision.

## External effects

Push, publish, deploy, send, spend and delete use their native tools. When
your authority is required, the Hub Supervisor records the exact decision and
the Lead acts on it. A Maestro record never runs the external effect by
itself.
