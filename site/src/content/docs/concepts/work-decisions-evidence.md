---
title: Work, decisions, and evidence
description: Keep SLP state small while preserving the result, rationale, and falsifier that matter.
---

SLP v2 durably stores work and settled decisions. Conversation remains natural;
raw transcript is not promoted into an evidence system.

## Work is the coordination unit

Every SLP work item has one objective, one assignee and one current state:

```text
OPEN -> ACTIVE -> RETURNED -> DONE
```

The return body is deliberately compact. Include only what the reviewer needs:

```text
result: <what is now true>
proof: <what could falsify the result>
blocker: <if present>
residual risk: <if present>
```

The objective and acceptance contract are immutable. `work note` preserves
context without changing either one. Changed scope requires a new work item;
the reviewer may cancel the superseded `OPEN` or `RETURNED` item. The reviewer
accepts at the boundary above the worker.

## Decisions are settled in one write

```sh
maestro decide "<choice>" --why "<reason>"
```

A decision is immutable when written. Inside a team workspace, link it with
`--work`; replace an older decision with `--replaces`. Hub decisions remain
owner or cross-team records and may link a unique project work id; qualify a
shared id as `<team-id>:<work-id>`. The old record remains so future readers
can see what changed and why.

Discussion that is not settled stays in chat or a work note. There is no draft
decision lifecycle inside SLP.

## Evidence names its layer

Use the narrowest true claim:

| Layer | What it proves |
| --- | --- |
| `source` | source inspection, test, lint or type check |
| `artifact` | the built or packaged output was read back |
| `installed` | the installed bytes or stamp match the artifact |
| `live` | the running process matches the installed layer |
| `journey` | the real user path reaches the intended result |

Do not claim a higher layer from a lower one. A passing source test does not by
itself prove the installed runtime or user journey.

## Minimal activity, not receipts

A successful state-changing operation updates current state and appends one
minimal internal activity line in the same transaction:

```text
who did what to which target and when
```

Activity is for recovery and debugging. It has no SLP command, carries no raw
transcript, and does not create a receipt or event domain. `status` reads the
current tables directly.

## Store ownership

Hub keeps team generations and pack identity. Each project keeps its work,
notes, returns, acceptances and decisions. See
[SLP setup and storage](/getting-started/slp-setup/) for the exact paths and
lifetime of each record.
