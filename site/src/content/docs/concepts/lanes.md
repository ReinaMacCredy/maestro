---
title: Team collaboration
description: Direct conversation, recorded work boundaries, rework, and blind design in SLP v2.
---

SLP collaboration uses direct conversation plus a small durable work record.
There is no separate lane, envelope, return-packet or council state machine.

## Talk directly, record material change

Team Supervisor, Lead and Peers can communicate directly along the team
topology. A message needs no work ID unless the people involved need it for
clarity.

Conversation does not silently change authority or state. Before a changed
objective, acceptance condition or settled choice governs work, record it:

```sh
maestro work note <work-id> "<material change>"
maestro decide "<settled choice>" --why "<reason>" --work <work-id>
```

## Assign and return work

A Team Supervisor creates Lead work. A Lead names the Peer when creating Peer
work:

```sh
maestro work add "<objective>"
maestro work add "<objective>" --to peer-api
```

The assigned Lead or Peer then owns the execution cycle:

```sh
maestro work take <work-id>
maestro work note <work-id> "<material progress or changed fact>"
maestro work return <work-id> "<result; proof; blocker; residual risk>"
```

The reviewer accepts separately:

```sh
maestro work accept <work-id>
```

## Rework and blockers

Returned work stays `RETURNED` until accepted. If it needs another pass, the
reviewer records the specific gap and the same assignee takes it again:

```sh
maestro work note <work-id> "rework: <specific gap and acceptance condition>"
maestro work take <work-id>
```

A blocker uses the same path. The assignee returns it with the blocker in the
body; the team resolves the dependency through conversation or a note, then
the assignee retakes it. SLP adds no `BLOCKED` or `REOPENED` state.

## Blind design

Blind design is a Lead-managed collaboration pattern, not runtime state:

1. Create separate work for two or three Peers with the same neutral question.
2. Ask each Peer to return independently before sharing another view.
3. Compare all returned work after every independent view is available.
4. Record the selected choice, rationale and dissent with `maestro decide`.
5. Create separate implementation work after the decision.

Peers may talk directly in ordinary work. The temporary no-sharing boundary
exists only because the Lead explicitly chose blind design for that question.

## External effects

Push, publish, deploy, send, spend and delete use their native tools. When
owner authority is required, Hub Supervisor records the exact decision and
Team Supervisor relays it down the topology. A Maestro record never executes
the external effect by itself.
