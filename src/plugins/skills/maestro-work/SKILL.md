---
name: maestro-work
description: Drive one accepted implementation unit - smallest falsifiable behavior, minimum edits, evidence that names the real falsifier; red tests only inside a Full bundle.
review-date: 2026-11-28
---
<!-- maestro-skill-version: dev -->

# maestro-work

Use for one accepted implementation unit. Keep the change inside the work
item's acceptance and authority. If the scope is unclear or must expand, stop
and return to `maestro-design`.

## Tier first, recon second

Decide the tier from the request alone, before reading bundle files, the
store, or the rest of this skill (`maestro-bundle` tier rule):

- quickfix: a one-sentence diff with no Full trigger. Do it directly, verify
  inline with the smallest check that can falsify it, no work item, no test
  demanded, none of the machinery below.
- Light: one session, one branch, acceptance in a sentence. `maestro work
  add|start|done` is the whole record. Verify the changed surface inline; a
  regression test is written only for a real bug being fixed, never as
  ceremony: it reproduces the bug where it shows and lands with the fix,
  in the ordinary bugfix order, not through a red-green loop, and no TDD
  skill is loaded for it. No SPEC, no red list.
- Full: an open bundle. Preconditions before any edit: the user's explicit
  request to implement or fix this scope, and a SPEC whose Scope names this
  work; its Red tests, when present, are the only tests written. A SPEC that predates the requested scope
  (post-ship review findings, hardening follow-ups) counts as missing: return
  to `maestro-design` first. A throwaway prototype not yet approved to port is
  `maestro-explore`'s scope: fix it there without opening production work.

Before writing code, read any language-convention notes the user's setup
provides for the language being edited. Repository conventions override them.

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

No-write names the file boundary only. `maestro dispatch accept` and
`maestro handback file` are the lane's own two writes and are never in the
excluded scope, so a scout or shadow lane can still accept and return.

The canonical parallel shapes are delivery and challenge on the same scope,
or a council of decision lanes run by `maestro-council`.

## Handback

Return this packet when the lane stops. `maestro dispatch accept` leaves the
dispatch claimed, not held, and `maestro handback file` refuses with
`DISPATCH_UNCONFIRMED` until the opener runs `maestro dispatch confirm`, so
ask for the confirm at acceptance rather than at the stop condition.

```text
Status: <DONE | BLOCKED | UNTESTABLE | UNKNOWN | FAILED | CHALLENGE | REOPEN_REQUEST | DEPENDENCY_REQUEST | COUNCIL_REQUEST>
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
2. **Choose** - the smallest behavior falsifiable at the accepted seam. In a
   Full bundle, write the failing test the SPEC's red list names before
   production code and watch it fail for the expected reason; under Light,
   name the inline check that will falsify the change instead. New child work
   gets `--acceptance "<observable result>"`.
3. **Act** - `maestro work start <id>`, then the minimum source and test edits
   for that behavior. Reach for what the repo already uses first: a helper,
   type, component, or installed dependency beats new code, and beats a
   native platform feature the repo has an established equivalent for.
   Minimum means the fewest concepts a maintainer meets at the seam, not the
   fewest lines; a wrapper that hides behavior to shorten a diff is a new
   concept, and the smallest change in the wrong layer is a second bug. A
   bug fix lands once where every caller routes through. Lazy about the
   solution, not about trust-boundary validation, error handling that
   prevents data loss, security, or anything explicitly requested.
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

## Hard rules

- No tests beyond the SPEC's red list (Full) or the real bug's regression
  (Light). After-the-fact confirmatory tests are bloat; add none.
- Never delete, skip, or weaken a failing test to make the suite pass. A
  failing test is information: fix the code or surface the conflict.
- Red must fail on an assertion at the agreed seam, not on a missing symbol.
  A test may name code that does not exist yet only when a locked decision
  already settles that contract's shape; a compile error over an undecided
  call shape is the test inventing the API. Never create the symbol to bridge
  it: treat the unsettled contract as a fork, ask one question, record the
  answer, then write the red test against the decided shape.
- Functionality outside the acceptance, or a fork no decision settled: pause
  the slice, ask the user exactly one question, record the answer with
  `maestro decision draft ... --work <id>` and `lock`, then continue. A fork
  needing more than one question is a `maestro-design` grill pass, not a
  mid-work detour.
- Scope the user cuts mid-loop leaves in the same turn: drop it from the red
  list and VERIFY.md, remove the tests and dead code written for it, and
  record the cut as a decision.
- When the failure's cause is unknown, diagnosis (`maestro-diagnose` steps)
  is the first phase of this authorized fix, done here, not as a separate
  engagement.
- A missing external fact (API behavior, library semantics, version
  differences) is not scope expansion: look it up against primary sources,
  record the finding and its link with `maestro work note <id>`, and
  continue. If the answer contradicts a locked decision, stop and supersede
  the decision first.
- A behavior-preserving change (dependency upgrade, refactor): red is the
  baseline captured via `maestro-explore` before the change; green is the
  baseline reproducing after. Add no new tests for behavior that is not
  changing. A request to change baselined behavior is new scope: re-capture
  that baseline and record which lines changed and why; editing it silently
  to match is weakening a test.
- Generated or vendored files are never the target: fix the generator or pin
  and regenerate.

## Red flags

| The thought | The reality |
|---|---|
| "The test is basically right - I'll adjust the assertion to match the output" | That documents the current bug as expected behavior. Assert from the decision's promise and fix the code. |
| "This one is hard to write red-first; I'll add the test after" | A never-red test proves nothing. Find the seam, or renegotiate it as a decision. |
| "The test doesn't compile - I'll create the missing symbol so it can run" | That lets the test mint the API. If a decision settles the shape, implement toward it in Act; if not, it is a fork to settle first. |
| "While I'm here, this nearby code could use a cleanup" | Not in the acceptance means not in scope. Mention it; do not touch it. |
| "Skipping this failing test unblocks the suite" | A failing test is information. Fix the code or surface the conflict. |
| "I'll batch all five open forks into one message" | One question per unsettled fork mid-loop; more than one is a `maestro-design` grill pass. |
| "It's a small task, but the method says write a test first" | Ceremony scales with tier. Quickfix and Light verify inline; only a Full bundle's named risks get red tests. |

When the scope is done and the checks are green: Light closes with
`maestro work done`; a Full bundle routes to `maestro-verify`.

## Coordination

Isolated lanes and worktrees: [references/worktree.md](references/worktree.md).
Contested files or overlapping sessions:
[references/conflict-handoff.md](references/conflict-handoff.md).
