---
title: Attention and brief
description: Read administrative project summaries at the Hub; a running team is watched by its sentinel and Observer instead.
---

`attention` and `brief` are Hub and development administration tools. They are
not part of the nine-operation SLP role toolbelt and do not monitor a running
team's transcript; that is the sentinel's job.

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

## Watch Pane, sentinel and Observer are separate

An optional Watch Pane is foreground runtime support owned by Team Supervisor.
It displays rolling labelled pane output and has no store authority. Attention
reads durable administrative records; Watch reads temporary runtime output.
Neither silently changes the other.

Each team generation also runs a sentinel that prompts the Observer seat every
five minutes with the open items and every role pane's recent tail; the
Observer records `work note --stall` and Maestro pushes the nudge. That is the
runtime watcher. Attention stays an on-call administrative read and never sees
a pane. See [Observer and sentinel](/guides/supervised-teams/#observer-and-sentinel).
