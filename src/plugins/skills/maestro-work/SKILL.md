---
name: maestro-work
description: Implement or fix one authorized unit with minimal edits and sufficient evidence. Reuse existing checks and add tests only for concrete uncovered behavior or risk.
review-date: 2026-11-28
---
<!-- maestro-skill-version: dev -->

# maestro-work

Use for one accepted implementation unit. Keep the change inside the work
item's acceptance and authority. Apply [WORKFLOW.md](~/maestro/WORKFLOW.md)
for method rules; pause only the slice blocked by scope or authority.

## Recon and preconditions

Inspect the relevant source and existing checks, then apply
[Tiers](~/maestro/WORKFLOW.md#tiers). Confirm the original implementation
request and accepted scope; for Full, read the matching SPEC. A newly found
in-scope test gap does not require a new design pass. A throwaway prototype
not approved to port remains `maestro-explore`'s scope.

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

For repeated failures, apply
[Recovery and verification](~/maestro/WORKFLOW.md#recovery-and-verification).
Record the episode in one `failed:` work note, carrying:

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
2. **Choose** - the smallest behavior falsifiable at the accepted seam. Apply
   [Testing discipline](~/maestro/WORKFLOW.md#testing-discipline): identify
   existing evidence and the concrete gap before writing any test. New child work
   gets `--acceptance "<observable result>"` and `--kind`: `feature`, `task`,
   `bug`, `chore`, `implement` are execution units; `idea` and `research` are
   scope notes under a parent and never hold it open. A parentless item the
   agent creates carries its why in the title or acceptance; a longer why is a
   `maestro work note <id> "why: <one paragraph>"`.
3. **Act** - `maestro work start <id>`. With `policy-breakdown` enabled it
   refuses a parentless write-like item: pass `--atomic-reason "<why this is
   one unit>"` when it truly is one, otherwise `maestro work add ... --parent
   <id>` first and start the child - a parent with open children never starts.
   Then the minimum source and test edits for that behavior. Reach for what the repo already uses first: a helper,
   type, component, or installed dependency beats new code, and beats a
   native platform feature the repo has an established equivalent for.
   Minimum means the fewest concepts a maintainer meets at the seam, not the
   fewest lines; a wrapper that hides behavior to shorten a diff is a new
   concept, and the smallest change in the wrong layer is a second bug. A
   bug fix lands once where every caller routes through. Lazy about the
   solution, not about trust-boundary validation, error handling that
   prevents data loss, security, or anything explicitly requested.
4. **Observe** - run the focused test, then type/lint/build checks. A suite
   that takes minutes runs in the background; its completion notification wakes
   you, so never hold the turn on `sleep`, `osascript -e 'delay'`, or a poll
   loop against its log. Review the diff against acceptance; confirm the test
   could expose the defect.
5. **Learn** - a pass that failed gets exactly one line,
   `maestro work note <id> "failed: <one line>"`; the lowercase `failed: `
   prefix is what `maestro attention` counts. Otherwise note only a reusable
   correction. Keep a checkpoint on the held item when state or the next
   action meaningfully changes, and before any handoff:
   `maestro work note <id> "checkpoint:\nstate: <where it stands>\nnext: <concrete action>\navoid: <what not to repeat>"`.
   Include the base, task-owned dirty paths, original authorization and
   retained gates for a successor. Only the latest one counts; the brief prints it back after a
   compaction, and `maestro handoff` renders it into NOTES.md.
6. **Continue** - `maestro work done <id>` with `--claim`/`--proof` naming the
   real falsifier (the check that would have failed if the claim were wrong).
   In a bundle, run `maestro handoff <bundle-id>` before releasing the work
   item: it renders NOTES.md from the store (work, decisions, handbacks,
   `failed:` and `checkpoint:` notes, base commit); hand-edit only Authority
   and whatever the store cannot derive, never the rendered sections.

## Test technique

Use the shared Testing discipline for whether a test is needed. When writing
one, prefer a stable consumer seam so internals stay free to change. Demonstrate
that a plausible wrong implementation fails it, and derive expectations from
the accepted behavior rather than current output.

Concrete smells and fixes: [references/tdd-antipatterns.md](references/tdd-antipatterns.md).

## Hard rules

- Never delete, skip, or weaken a failing test to make the suite pass. A
  failing test is information: fix the code or surface the conflict.
- A new test's failure must reflect the behavior gap, not an unrelated setup
  failure. Do not invent public behavior merely to get a test to compile.
- For a material choice outside the acceptance, pause that slice and apply
  [Decisions and readiness](~/maestro/WORKFLOW.md#decisions-and-readiness).
  Reversible internal details do not require a user question or a decision lock.
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
- For a behavior-preserving change, compare existing checks or a captured
  baseline before and after. Apply Testing discipline if a coverage gap is
  discovered. Changing baselined behavior needs the appropriate scope approval;
  do not silently edit expectations to match a regression.
- Generated or vendored files are never the target: fix the generator or pin
  and regenerate.

## Red flags

| The thought | The reality |
|---|---|
| "The test is basically right - I'll adjust the assertion to match the output" | That documents the current bug as expected behavior. Assert from the decision's promise and fix the code. |
| "Another test would make this feel safer" | Name the wrong implementation existing checks miss first; duplicate confirmation is not additional evidence. |
| "The test doesn't compile - I'll create the missing symbol so it can run" | Check the accepted contract first; a test does not authorize new public behavior. |
| "While I'm here, this nearby code could use a cleanup" | Not in the acceptance means not in scope. Mention it; do not touch it. |
| "Skipping this failing test unblocks the suite" | A failing test is information. Fix the code or surface the conflict. |
| "All later design questions must be settled before I start" | Only unresolved choices blocking the next authorized slice stop that slice. |
| "The tier requires another test" | Tier determines record depth, not test count; use the shared Testing discipline. |

When the scope is done and the checks are green: Light closes with
`maestro work done`; a Full bundle routes to `maestro-verify`.

## Coordination

Isolated lanes and worktrees: [references/worktree.md](references/worktree.md).
Contested files or overlapping sessions:
[references/conflict-handoff.md](references/conflict-handoff.md).
