---
title: Attention and brief
description: Read administrative project summaries at the Hub; a running team raises attention with its own blocked notes instead.
---

`attention` and `brief` are Hub and development administration tools. They are
not part of the nine-operation SLP role toolbelt and do not monitor a running
team's transcript; a team seat raises attention itself with `work note
--blocked`.

## Attention

```sh
maestro attention
maestro attention --json
```

Attention computes findings from the current store when called. It can surface
stale development work, unresolved design decisions, repeated failed attempts,
pending lessons, and scope collisions.

It does not run continuously, open a pane, prompt an agent, change work state,
or claim semantic awareness between calls. Inside a running SLP team,
`maestro status` is the authoritative current-state view.

Threshold flags tune administrative stale-work and decision checks:

```sh
maestro attention --stale 30 --decision-stale 24
```

## Brief

```sh
cd ~/maestro
maestro brief
```

Brief reads the Hub registry and summarizes project stores without mutating
them. It is useful when Hub Supervisor needs a cross-project overview before
starting a team or recording an owner decision.

Brief is not a team control plane. Hub communicates with a running team through
Team Supervisor, and team work remains in that project's store.

## The runtime pane and Hub attention are separate

Inside a team, attention has two layers and the Hub `attention` verb is
neither. The first is the seat's own `work note <id> "<what you need>"
--blocked`, which Maestro pushes one seat up. The second is the generation's
runtime pane, the `maestro slp runtime` process that `team start` opens
beside the Team Supervisor through Maestro's Herdr plugin (Hub d96, d97). It
resolves Herdr pane events against the store with no model and writes as the
actor `runtime`:

- a role pane that Herdr reports `blocked` (a harness dialog such as a trust
  or permission prompt; seats have no question tool, so a question for you is
  a `--blocked` note, never a dialog) becomes a
  `stall:dialog` entry on the item that seat holds, and an `idle` pane that
  still holds ACTIVE work becomes `stall:silence`; each is pushed to the stuck
  pane as `[from runtime][<id>] dialog|silence <evidence>; stop and run:
  maestro work note <id> "<what you need>" --blocked`, with a copy to the
  Team Supervisor, once per item and kind until the item's latest entry
  changes;
- an idle seat holding nothing wakes the seat above with `[attention] <seat>
  idle`, once per pane until the team's activity log advances; a role pane
  that exits or closes is noted on the team card and wakes the Team
  Supervisor with `[attention] <seat> pane exited|closed`.

The wakes the nine operations push are separate from the runtime and go out
as each operation commits: `work add --to` wakes the assignee's pane with
`[from <role>][<id> OPEN] <objective>; read: maestro status <id>` whether the
pane was just opened or already acknowledged (d840); `work return`, `work
accept` and `work note --rework` wake their counterpart the same way. A wake
for a seat that is still working waits in the runtime's queue for that seat's
next idle. The Team Supervisor's wakes to the Hub go to the Hub Supervisor
pane `team start` recorded (d841), or to a Hub agent named `supervisor`; a
wake that resolves to neither is dropped rather than queued forever, and
`maestro slp status` from a team pane lists it as `unreachable` next to the
wakes still pending. The store stays the truth either way.

The Hub `attention` verb stays an on-call administrative read and never sees
a pane. See [Attention](/guides/supervised-teams/#attention).
