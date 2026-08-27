---
title: Roles
description: The Human, Supervisor, Lead, and Peer authority model used by SLP.
---

Read the complete role contract from the installed runtime:

```sh
maestro recipe show slp
```

## Human

The Human owns purpose, priority, risk, and external effects such as push,
publish, deploy, send, spend, and delete. The Human creates, replaces, and
revokes the Supervisor and Leads, and accepts at the owner boundary.

## Supervisor

The Supervisor lives in `~/maestro` and represents the owner across projects.
It filters attention, governs in the owner's name, and carries Human authority
through each project's Lead. It never dispatches a Peer directly, edits project
code, or accepts a technical candidate.

## Lead

A session started in a repository working tree is that repository's Lead. The
Lead owns the project outcome, problem framing, topology, one write owner per
moving scope, integration, verification strategy, and engineering acceptance
inside the Human lease.

## Peer

A pane the Lead opened with a dispatch becomes a Peer when it accepts that
stored contract. The Peer owns independent judgment or bounded delivery and
its own proof. It does not own topology, scope beyond the assignment, project
acceptance, or external effects.

## Topology invariants

```text
Human
+-- Supervisor
+-- Lead
    +-- Peer A: bounded scope A
    +-- Peer B: bounded scope B
    +-- Peer C: independent review or alternative lane
```

There is one active Lead per project or workspace and one write owner per
moving scope. The Supervisor is never the writer or technical acceptance owner.
Peers do not create sub-topology unless the assignment grants it, and scope
changes reach Peers through the Lead. Two sessions holding parent work in one
repository are split-brain; the later session stops and reads `maestro status`.
