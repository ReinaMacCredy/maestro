---
name: maestro-work
description: The contract every playbook shares - the brief a writer receives, the rules it writes by, the handback it returns, and the store record the holder keeps. Reuse existing checks and add tests only for concrete uncovered behavior or risk.
review-date: 2026-11-28
---
<!-- maestro-skill-version: dev -->

# maestro-work

The matched playbook (`maestro recipe show <name>`, chosen by
[Routing](~/maestro/WORKFLOW.md#routing)) owns the steps of a task. This
skill owns what every playbook shares: the brief a writer receives, the rules
it writes by, the handback it returns, and the record the holder keeps. A
writer is a `delegate` subagent, a lane in a Herdr pane, or the main chat on
a quickfix. Apply [WORKFLOW.md](~/maestro/WORKFLOW.md) for method rules;
pause only the slice blocked by scope or authority.

## Dispatch

When work is handed to a writer (a `delegate` subagent, or a lane in a
Herdr pane in the room), send this envelope:

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
`Excluded scope` or `Mutation`. Name the playbook and step the work serves,
and point at files rather than pasting their content. Standing orders that
apply to every writer of one run go in one file every brief points at.

- `scout` reads and reports state, never writes.
- `delivery` may write and is the only lane that holds the lease; a
  subagent's brief borrows the lane word but never takes the lease.
- `decision` investigates, compares, and recommends without writing.
- `challenge` breaks the premise or candidate and returns findings only, with
  no fixes or redesign.
- `shadow` runs beside the owner without writing and returns comparison
  evidence that is never a candidate or a work write lease.

No-write names the file boundary only. A subagent's holder, the one that
briefed it, reviews its diff and keeps the item.

The canonical parallel shapes are delivery and challenge on the same scope,
or a council of decision lanes run by `maestro-council`.

## Handback

Return this packet when the writer stops.

```text
Status: <DONE | BLOCKED | UNTESTABLE | UNKNOWN | FAILED | CHALLENGE | REOPEN_REQUEST | DEPENDENCY_REQUEST | COUNCIL_REQUEST>
Claim: <what is now believed true>
Proof: <evidence with its layer named>
Assumptions not verified: <items or None>
Residual risks: <items or None>
Incidental findings: <items or None>
```

Unknown is a valid result; it is never rounded up to PASS.

A writer that discovers a dependency stops the mutation that depends on the
new assumption and hands back `DEPENDENCY_REQUEST` with evidence and impact.
The holder re-scopes the work. A never silently becomes A+B+C.

## Record

The holder of a work item keeps its record; a subagent never does.

- New work gets `--acceptance "<observable result>"` and `--kind`:
  `feature`, `task`, `bug`, `chore`, `implement` are execution units; `idea`
  and `research` are scope notes under a parent and never hold it open. A
  parentless item the agent creates carries its why in the title or
  acceptance; a longer why is a `maestro work note <id> "why: <one paragraph>"`.
- `maestro work start <id>`. With `policy-breakdown` enabled it refuses a
  parentless write-like item: pass `--atomic-reason "<why this is one
  unit>"` when it truly is one, otherwise `maestro work add ... --parent
  <id>` first and start the child; a parent with open children never starts.
- A pass that failed gets exactly one line,
  `maestro work note <id> "failed: <one line>"`; the lowercase `failed: `
  prefix is what `maestro attention` counts. For repeated failures, apply
  [Recovery and verification](~/maestro/WORKFLOW.md#recovery-and-verification)
  and record the episode in one `failed:` note carrying:

```text
Attempted: <approaches tried>
Invariant assumed: <belief shared by the attempts>
Exact failure: <literal evidence>
What changed between attempts: <delta>
What did not change: <stable conditions>
Smallest new information needed: <next fact that would change the approach>
```

- Keep a checkpoint on the held item when state or the next action
  meaningfully changes, and before any handoff:
  `maestro work note <id> "checkpoint:\nplaybook: <name> step <n>\nstate: <where it stands>\nnext: <concrete action>\navoid: <what not to repeat>"`.
  Include the base, task-owned dirty paths, original authorization and
  retained gates for a successor. Only the latest one counts; the brief
  prints it back after a compaction, and `maestro handoff` renders it into
  NOTES.md.
- Otherwise note only a reusable correction, decision, or failed approach;
  keep ordinary command output out of notes.
- `maestro work done <id>` with `--claim`/`--proof` naming the real
  falsifier (the check that would have failed if the claim were wrong).
  `--evidence` records one opaque blob. Report pending delivery approval
  separately from completion. In a bundle, run
  `maestro handoff <bundle-id>` before releasing the work item:
  it renders NOTES.md from the store (work, decisions, `failed:`
  and `checkpoint:` notes, base commit); hand-edit only Authority and
  whatever the store cannot derive, never the rendered sections.

When the scope is done and the checks are green: Light closes with
`maestro work done`; a Full bundle routes to `maestro-verify`.

## Writing rules

Before writing code, read the language conventions
(`maestro recipe show style-<lang>`); repository conventions override them.
Make the minimum change (**Laziness Protocol**, `~/maestro/PRINCIPLES.md`).
Run the focused test, then type, lint, and build checks. A suite that takes
minutes runs in the background; its completion notification wakes you, so
never hold the turn on `sleep`, `osascript -e 'delay'`, or a poll loop
against its log. Review the diff against acceptance; confirm the test could
expose the defect.

### Test technique

Use the shared Testing discipline for whether a test is needed. When writing
one, prefer a stable consumer seam so internals stay free to change. Demonstrate
that a plausible wrong implementation fails it, and derive expectations from
the accepted behavior rather than current output.

### Hard rules

- Never delete, skip, or weaken a failing test to make the suite pass. A
  failing test is information: fix the code or surface the conflict.
- A new test's failure must reflect the behavior gap, not an unrelated setup
  failure. Do not invent public behavior merely to get a test to compile.
- For a material choice outside the acceptance, pause that slice and apply
  [Decisions and readiness](~/maestro/WORKFLOW.md#decisions-and-readiness).
  Reversible internal details do not require a user question or a decision lock.
- Scope the user cuts mid-work leaves in the same turn: drop it from the red
  list and VERIFY.md, remove the tests and dead code written for it, and
  record the cut as a decision.
- When the failure's cause is unknown, the holder runs the bug-fix
  playbook's diagnosis steps (`maestro recipe show bug-fix`) first, as part
  of the authorized fix. A writer handed a fix whose cause turns out unknown
  hands back `BLOCKED` with what it found.
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
- A hard-to-reverse fork found mid-work goes back to the holder as
  `COUNCIL_REQUEST`; the Lead runs `maestro-council`.

### Red flags

| The thought | The reality |
|---|---|
| "The test is basically right - I'll adjust the assertion to match the output" | That documents the current bug as expected behavior. Assert from the decision's promise and fix the code. |
| "Another test would make this feel safer" | Name the wrong implementation existing checks miss first; duplicate confirmation is not additional evidence. |
| "The test doesn't compile - I'll create the missing symbol so it can run" | Check the accepted contract first; a test does not authorize new public behavior. |
| "While I'm here, this nearby code could use a cleanup" | Not in the acceptance means not in scope. Mention it; do not touch it. |
| "Skipping this failing test unblocks the suite" | A failing test is information. Fix the code or surface the conflict. |
| "All later design questions must be settled before I start" | Only unresolved choices blocking the next authorized slice stop that slice. |
| "The tier requires another test" | Tier determines record depth, not test count; use the shared Testing discipline. |

## Coordination

Isolated lanes and worktrees: `maestro recipe show worktree`. Contested files
or overlapping sessions: name the contested paths, stop overlapping writes
until one owner is explicit, and let that owner merge with ordinary git and
rerun the affected checks.
