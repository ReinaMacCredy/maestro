---
title: SLP scenarios
description: Six SLP journeys written from the owner's seat, from enabling the plugin through hand-offs, questions, rework, and stop.
---

Each scenario starts with what you type to your Hub agent, the agent session
you run in `~/maestro`. The commands below it are what the agents run and what
the store then holds. You never have to type them yourself. The scenarios
follow one team from start to stop.

## 1. Turn on teams and start one

You type to your Hub agent:

```text
enable teams for ~/Code/example and start a team to fix the CSV import that drops the last column, with a regression test
```

The Hub agent enables the plugin in the room, then starts the team. `team
start` enables the plugin in the project itself:

```sh
cd ~/maestro
maestro plugin enable slp
maestro team start ~/Code/example "Fix the CSV import that drops the last column and add a regression test; done when both pass with the full suite"
```

Maestro pins `~/maestro/SLP.md` into the project, opens the Lead and the
runtime pane, and creates the first `OPEN` item, `w1`, for the Lead. The Hub
agent reads the team and item ids back instead of guessing them:

```sh
maestro status
```

## 2. The Lead hands a piece to a Peer

You type:

```text
how is the CSV team doing?
```

The Hub agent answers from `maestro status`. Meanwhile the Lead has taken its
item and opened a Peer for the test, a bounded piece that can run beside its
own fix:

```sh
maestro work take w1
maestro work add "Add a regression test for a CSV file without a final newline; done when it fails before the fix and passes after" --to peer-csv
```

Maestro opens `peer-csv` with the pinned Peer seat file and wakes it. The Peer
runs the normal cycle, and the Lead reviews and accepts:

```sh
maestro work take w2
maestro work return w2 "added the case; proof: fails on the parent commit, passes on 3f2a1c0"
maestro status w2
maestro work accept w2
```

## 3. The Lead is blocked and asks you

A question reaches you without your asking. The Lead found a fact it cannot
settle:

```sh
maestro work note w1 "the fixtures disagree on the canonical format; need: UTF-8 with LF, or keep both" --blocked
```

The Hub agent's `maestro status` now lists
`blocked for the Hub: example:w1 ...`. If the choice is one the Hub may make,
it makes it. If it is yours, the Hub agent asks you, and you type:

```text
use UTF-8 with LF everywhere
```

The Hub agent records your answer as a decision on the item, which wakes the
Lead with a `DECIDED` line:

```sh
maestro decide "Canonical fixture format is UTF-8 with LF" --why "owner ruling" --work example:w1 --scope owner
```

Had the Lead gone quiet instead of asking, the runtime pane would have recorded
a `stall:silence` entry on `w1` and pushed a nudge to the Lead's pane telling
it to record a `--blocked` note, with a copy to the Hub.

## 4. You type straight into the Lead's pane

You type into the Lead's pane rather than the Hub's:

```text
also log the row count after each import
```

The Lead records where the instruction came from before acting:

```sh
maestro work note w1 "owner asked: log the row count after each import" --owner
```

The note pushes an `OWNER` line to the Hub. The request is inside the item's
objective, so the Lead does it at once. A request outside the objective would
not widen the item; the Lead would open new work for it instead.

## 5. The Lead returns and you ask for one more pass

The Lead returns its item, which wakes the Hub agent:

```sh
maestro work return w1 "fixed CSV termination, logged row counts; proof: focused test and full suite pass on 3f2a1c0; residual risk: CRLF files untested"
```

The Hub agent tells you the item is ready and names the residual risk. You
type:

```text
the fix must also handle Windows line endings before you accept it
```

The Hub agent records the gap as a rework grant instead of accepting:

```sh
maestro work note w1 "handle CRLF line endings before acceptance" --rework
```

The Lead retakes the same item and returns again, and the Hub agent accepts
once the gap is closed:

```sh
maestro work take w1
maestro work return w1 "added CRLF handling; proof: focused matrix and full suite pass on 9b04e7d"
maestro work accept w1
```

## 6. Stop the team

You type:

```text
the CSV work is finished; stop the team
```

The Hub agent checks that every item is `DONE` and stops the team with a
closing report:

```sh
maestro status
maestro team stop example --reason "CSV import fixed with LF fixtures, CRLF handling and a regression test"
```

Maestro closes the Peers, the Lead, the runtime pane and its directory, then
the workspace. The team is `STOPPED` only once the workspace is gone; a
partial close stays `RUNNING` and the same command resumes cleanup. The
project's pack copy and all records remain.

If you want to drop the work instead, you type:

```text
cancel the CSV team, I changed my mind
```

The Hub agent runs an emergency stop, which keeps each unfinished item in its
current state and marks it abandoned with actor, reason, generation, and time:

```sh
cd ~/maestro
maestro team stop example --emergency --reason "owner cancelled this generation"
```

## What not to do

- Do not edit either SQLite store by hand.
- Do not edit `<project>/.maestro/SLP.md` or a seat file while a generation is running.
- Do not treat chat or a pane transcript as a decision or an acceptance.
- Do not let anyone accept their own work.
- Do not send Hub instructions to a Peer; the Hub speaks to the Lead.
