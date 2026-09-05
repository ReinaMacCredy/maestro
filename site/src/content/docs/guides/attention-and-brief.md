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

Inside a team, attention is the seat's own `work note <id> "<what you need>"
--blocked`, which Maestro pushes one seat up, plus the generation's runtime
pane (Hub d96, d97), which resolves Herdr pane events against the store and
records `stall:dialog`, `stall:silence` and pane loss as the actor `runtime`.
The Hub `attention` verb stays an on-call administrative read and never sees
a pane. See [Attention](/guides/supervised-teams/#attention).
