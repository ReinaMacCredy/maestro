# Grilling

Apply [Decisions and readiness](~/maestro/WORKFLOW.md#decisions-and-readiness).
Interview on material user-owned choices blocking the next slice, not routine
implementation details. Map their dependencies as a **design tree**.

Work the tree in **rounds**. The **frontier** is every decision whose
prerequisites are already settled — the questions you can ask _now_ without
guessing at answers you haven't heard yet. Ask the frontier in plain prose,
one decision at a time when the forks are heavy, or as a short numbered list
when they are independent and light. Then wait for the user's answers before
the next round.

For each question:

- Lead with **what this does** — the consequence of the decision in this
  repo, before any option list.
- Present the options as prose, each with its concrete trade-off. When a
  layout or structure is easier to see than to describe, include a small
  ASCII sketch per option.
- End with a `my rec:` line — your recommended answer and the one-line why.
- No emoji, no batching questions that depend on each other's answers. When
  the harness offers a question card, use it: one decision per card, "what
  this does" first, the sketch per option, the `my rec:` line on every fork.
- Record durable decisions under the shared workflow's threshold:
  `maestro decision draft "<choice>" --rationale "<why + rejected alternative>" --work <id>`
  then `maestro decision lock <id>`. An answer to a fork is
  never an implementation order, even when the chosen option is
  itself an artifact (a script, a schema, a prototype); building starts only
  on an explicit request.

Each round the user answers reshapes the tree — settled decisions push the
frontier outward and unblock questions that depended on them. Recompute the
frontier and ask the next round. A question whose answer depends on another
question still open in this round belongs to a _later_ round, not this one.

Finding _facts_ is your job, never the user's. Investigate directly; delegate
only when independent work or context isolation warrants it. Do not ask the
user for anything you could look up yourself.
Don't block on it: a running exploration is an unsettled prerequisite, so
only the questions downstream of it wait for the sub-agent to report — ask
the rest of the frontier now. The _decisions_ are the user's — put each to
them and wait.

Finish when the next slice meets the shared readiness rule. Record deferred
questions; they need not all be settled now. Continue only within verified
implementation authority. A design-only request stops at the design result.
