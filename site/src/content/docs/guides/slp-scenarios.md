---
title: SLP scenarios
description: Eight end-to-end SLP v2 journeys from team start through work, decisions, rework, blocked work, and stop.
---

These scenarios show what each role records. Team conversation itself remains
natural and direct; commands appear only where material state changes.

## 1. Start one project team

From the Hub room, the Hub Supervisor starts the project with one objective:

```sh
cd ~/maestro
maestro team start ~/Code/example "Ship the validated import fix"
```

Maestro pins `~/maestro/SLP.md` into the project, opens Team Supervisor and
Lead, and creates initial `OPEN` work assigned to Lead. There is no second
readiness sequence.

Read the returned team and work identifiers rather than assuming them:

```sh
maestro status
```

## 2. Lead executes the initial work

In the Lead pane:

```sh
maestro work take w1
maestro work note w1 "reproduced: empty CSV loses the final column"
maestro work return w1 "fixed CSV termination; source: focused test and full suite pass; residual risk: none"
```

The Team Supervisor reads the current work and accepts it:

```sh
maestro status w1
maestro work accept w1
```

The separation is intentional: Lead owns the technical result; Team
Supervisor owns acceptance of Lead work.

## 3. Lead assigns bounded work to a Peer

The Lead creates the assignment and names the Peer:

```sh
maestro work add "Add the missing CSV regression test" --to peer-csv
```

Maestro reuses `peer-csv` if it already belongs to this generation, otherwise
it opens the Peer with the generation's pinned Peer contract. The Peer then
runs only the normal work lifecycle:

```sh
maestro work take w2
maestro work return w2 "added the failing case and verified it passes with the fix; source: focused test"
```

The Lead reviews and accepts:

```sh
maestro status w2
maestro work accept w2
```

There is no separate Peer lifecycle, assignment envelope or return packet.

## 4. A Peer is blocked

A blocker is part of the return body, not a fifth state:

```sh
maestro work return w3 "blocked: fixture format is undecided; proof: both current fixtures are accepted; need: choose canonical format"
```

The Lead resolves the missing fact through conversation, records the material
change, and asks the same Peer to continue:

```sh
maestro work note w3 "canonical fixture format is UTF-8 CSV with LF endings" --rework
```

The Peer retakes the returned work:

```sh
maestro work take w3
maestro work return w3 "implemented against the recorded format; source: fixture tests pass"
```

## 5. Returned work needs rework

The reviewer does not create a reopen state. It records the exact gap:

```sh
maestro work note w4 "add the Windows line-ending case before acceptance" --rework
```

The assignee retakes `RETURNED` work, returns the new result, and the reviewer
accepts only after the gap is closed.

```sh
maestro work take w4
maestro work return w4 "added CRLF coverage; source: focused matrix passes"
maestro work accept w4
```

## 6. Blind design with independent Peers

Blind design is a collaboration pattern owned by Lead. It adds no council or
seal state.

```sh
maestro work add "Independently design session incarnation identity" --to peer-incarnation
maestro work add "Independently design session incarnation identity" --to peer-heartbeat
```

Lead asks both Peers not to share views until both have returned. Each Peer
takes and returns its own work normally. After both returns are visible, Lead
compares them and records one settled decision:

```sh
maestro decide "Use a process start-time incarnation with the PID" \
  --why "It rejects PID reuse without introducing a background writer" \
  --work w5
```

Implementation is new work after the decision, never a silent extension of a
design assignment.

## 7. Watch, status, and stop

When Team Supervisor needs continuous situational awareness, it may open one
foreground Watch Pane with existing Herdr pane control. This does not add a
Maestro command. Watch only labels currently available raw output; it never
prompts or intervenes.

Use status for authoritative current state:

```sh
maestro status
maestro status w5
```

Normal stop succeeds only after all work is `DONE`:

```sh
maestro team stop example
```

Maestro closes Peers, Lead, Watch and its raw transcript, then Team Supervisor.
A transient foreground non-agent helper pane in the Hub completes the
self-closing sequence. Only a fully absent team workspace becomes `STOPPED`;
a partial close stays `RUNNING` and the same stop command resumes cleanup. The
project Workspace Pack snapshot and durable records remain. Emergency stop is
Hub authority and explicitly abandons unfinished work in its current state:

```sh
cd ~/maestro
maestro team stop example --emergency --reason "owner cancelled this generation"
```

Each unfinished record gains abandonment actor, reason, generation, and time;
the next generation cannot inherit or mutate it.

## 8. The Lead cannot proceed

The owner started the team, gave the Lead an objective whose first step fails,
and walked away. The Lead's profile tells it what to do when it cannot
proceed: declare it, do not loop. The Lead records what it needs:

```sh
maestro work note w1 "flaky.sh never exits 0 here; need: a reachable mirror or permission to stop retrying" --blocked
```

The Team Supervisor receives `[from lead][w1 BLOCKED] ...; read: maestro status w1`
and either settles the fact through conversation and a note, or escalates to
the Hub with its own `--blocked` note. Between this release and the team
runtime of the herdr-adapter bundle (Hub d97, d98) that self-declared note is
the only attention mechanism: no seat or process watches panes for a stall the
stuck seat does not declare, and `work note --stall` is refused for every pane.

## What not to do

- Do not edit either SQLite store manually.
- Do not edit `<project>/.maestro/SLP.md` during a running generation.
- Do not treat chat or transcript as a decision or acceptance record.
- Do not let a Peer accept its own work.
- Do not send Hub instructions directly to Lead or Peers; Hub speaks through
  Team Supervisor.
- Do not recreate removed lifecycle layers. The compact old-to-new mapping is
  in the [CLI reference](/reference/cli/).
