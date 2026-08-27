---
name: maestro-work
description: Drive one accepted implementation unit test-first - smallest falsifiable behavior, minimum edits, evidence that names the real falsifier.
---
<!-- maestro-skill-version: dev -->

# maestro-work

Use for one accepted implementation unit. Keep the change inside the work
item's acceptance and authority. If the scope is unclear or must expand, stop
and return to `maestro-design`.

## Dispatch

When work is handed to a lane (a Herdr pane in the room, or a sub-agent where no room exists), send this envelope:

```text
Objective: <observable outcome>
Owned scope: <paths or responsibility>
Excluded scope: <explicit non-goals>
Mutation: <no-write | write-bounded: paths>
Stop condition: <done or blocked boundary>
Lane: scout | decision | delivery | challenge | shadow
Evidence required: <proof and layer>
```

A tiny task may collapse the envelope to three lines, but it never drops
`Excluded scope` or `Mutation`.

- `scout` reads and reports state, never writes.
- `delivery` may write and is the only lane that holds the lease.
- `decision` investigates, compares, and recommends without writing.
- `challenge` breaks the premise or candidate and returns findings only, with
  no fixes or redesign.
- `shadow` runs beside the owner without writing and returns comparison
  evidence that is never a candidate or a work write lease.

The canonical parallel shapes are delivery and challenge on the same scope,
or a council of two or three decision lanes.

## Handback

Return this packet when the lane stops:

```text
Status: <DONE | BLOCKED | UNTESTABLE | UNKNOWN | FAILED | CHALLENGE | REOPEN_REQUEST | DEPENDENCY_REQUEST>
Claim: <what is now believed true>
Proof: <evidence with its layer named>
Assumptions not verified: <items or None>
Residual risks: <items or None>
Incidental findings: <items or None>
```

Unknown is a valid result; it is never rounded up to PASS.

A peer that discovers a dependency stops the mutation that depends on the new
assumption and hands back `DEPENDENCY_REQUEST` with evidence and impact. The
Lead re-scopes the work. A never silently becomes A+B+C.

After two or three failures on the same mechanism, stop and record an episode
packet as `maestro work note <id> "failed: <one line>"`, carrying:

```text
Attempted: <approaches tried>
Invariant assumed: <belief shared by the attempts>
Exact failure: <literal evidence>
What changed between attempts: <delta>
What did not change: <stable conditions>
Smallest new information needed: <next fact that would change the approach>
```

## Loop

1. **Perceive** - `maestro work show <id>`, `maestro ready`, relevant source,
   tests, and repository instructions. Name the task-owned dirty paths before
   editing.
2. **Choose** - the smallest behavior falsifiable at the accepted seam. Write
   the agreed failing test before production code. New child work gets
   `--acceptance "<observable result>"`.
3. **Act** - `maestro work start <id>`, then the minimum source and test edits
   for that behavior. No speculative abstractions or dependencies.
4. **Observe** - run the focused test, then type/lint/build checks. Review the
   diff against acceptance; confirm the test could expose the defect.
5. **Learn** - a pass that failed gets exactly one line,
   `maestro work note <id> "failed: <one line>"`; the lowercase `failed: `
   prefix is what `maestro attention` counts. Otherwise note only a reusable
   correction.
6. **Continue** - `maestro work done <id>` with `--claim`/`--proof` naming the
   real falsifier (the check that would have failed if the claim were wrong).
   In a bundle, overwrite NOTES.md (current state, next action, base commit)
   before releasing the work item.

## Test-first root laws

1. Red tests only transcribe DECIDED behavior at an accepted seam. Having to
   guess the contract means you are still in the design lane - spike, decide,
   then test.
2. Test at the outermost stable seam (here: the CLI) so internals stay free to
   change.
3. Serve the behavior, never the test. A wrong test is shifted openly - make
   the deliberate change and record it with `maestro decision draft` - code is
   never contorted to please a test.
4. A test must be able to kill the bug: no tautologies, no asserting that a
   mock called a mock.
5. No silent test edits. Every shift or deletion of a test records its reason.

Concrete smells and fixes: [references/tdd-antipatterns.md](references/tdd-antipatterns.md).

## Coordination

Isolated lanes and worktrees: [references/worktree.md](references/worktree.md).
Contested files or overlapping sessions:
[references/conflict-handoff.md](references/conflict-handoff.md).
