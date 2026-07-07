---
name: maestro-research
version: 1.0.0
description: "Breadth-first Maestro research before design: use for zero-context features, unfamiliar domains, externally pasted ideas, stakeholder-heavy discovery, hosting-unclear work, or when maestro-design needs a fresh research receipt before forks."
---

# Maestro Research

Research is breadth. Design is depth. Use this skill to decide whether a card
has enough valid context to enter `maestro-design`.

Research writes a same-card `research.md` receipt or a skip receipt. It does
not lock decisions, write acceptance criteria, finalize handoffs, create tasks,
start implementation, browse by default, or approve build.

Activate with a known session id:
`maestro hook record --event skill_activation --skill maestro-research --session <session_id>`

Exact command signatures live in `reference/cli.md` when present. If the local
binary does not list a verb, do not invent it.

## Route

Use this skill when the request is zero-context, unfamiliar-domain, externally
pasted, stakeholder-heavy, hosting-unclear, or when `maestro-design` lacks fresh
research.

Skip research only with a durable skip receipt when one condition is true:

- the user explicitly says to skip research
- a settled spec is pasted
- fresh same-problem research already exists
- the work is a small local change in a known domain
- context is clearly settled and recorded with evidence

Fresh research means the same card and the same problem statement, `as_of`
within 7 days, unchanged `invalidates_when`, and a still-valid
`READY_FOR_DESIGN` gate or skip.

## Do

1. Anchor the card and hosting.
   Run `maestro status`, then `maestro active`. If the idea is external,
   sandbox-bound, or hosting-unclear, record that before writing into the current
   repo. Use the same card for research and later design. Exploratory ideas may
   start as Idea cards; intended product work may start as proposed Features.
   Completion criterion: one owning card and one Hosting value are named, or the
   gate is not `READY_FOR_DESIGN`.

2. Map current context.
   Search precedent with `maestro grep "<topic>" corpus:memory`, then read the
   pasted source, relevant repo artifacts, and stakeholder facts already
   available. If evidence answers a question, inspect it instead of asking the
   user. Completion criterion: the brief can cite what is known from artifacts,
   what is assumed, and what remains unknown.

3. Write the research receipt.
   Create or update same-card `research.md`. If replacing prior research,
   preserve the old brief as `research.md.bak` or a `Superseded` pointer, and
   add a Maestro note with the reason and changed facts. Keep exactly one
   current `research.md`.

   Required receipt sections:
   - Research Status: `skipped`, `skip_reason`, `skipped_by`
   - Hosting: `current-repo`, `sandbox-repo`, or `external`, with rationale
   - Problem
   - Users / Stakeholders
   - Current Context
   - Constraints
   - Unknowns: Blocking, Important but non-blocking, Safe to defer
   - Assumptions
   - Landscape
   - Recommended First Design Fork
   - Stakeholder Actions
   - Research Validity: `as_of`, `invalidates_when`
   - Gate

   Completion criterion: every required section is present, even when the
   section says `None`.

4. Set the gate.
   Choose exactly one:
   - `READY_FOR_DESIGN`
   - `NEEDS_STAKEHOLDER`
   - `NEEDS_EVIDENCE`
   - `PIVOT`
   - `STOP`

   `READY_FOR_DESIGN` requires no blocking unknowns, no open stakeholder
   actions, compatible hosting, whitelisted skip evidence when skipped, and one
   concrete first design fork. Duplicate or prior-art discovery gates `STOP` or
   `PIVOT` with a pointer to the existing card.

   Completion criterion: the final response names the gate, the durable receipt,
   and the next Maestro route.

## Receipt Rules

Landscape is a map, not a decision. It may list directions such as browser
extension, omnichannel assistant, or dedicated inbox. The first design fork is
one entry question for depth, such as "Where should Copilot live in the Sales
workflow?"

Stakeholder actions have status:

```text
open | resolved | superseded
```

A resolved answer may create new blockers. If a deferred unknown becomes
acceptance-critical during the first design fork, `maestro-design` may bounce
back to this skill.

High-risk user skips are allowed only as user-directed risk:

```text
skipped: true
skipped_by: user
unresolved_risks:
- <security, legal, customer-impacting, or data risk>
```

## Time Box

- Light: one session, gate emitted by end of turn.
- Standard: at most three blocking stakeholder questions, then gate.
- Deep: explicit user opt-in only. Record the escalation if it happens
  midstream.

## Stop

- Do not lock A/B/C decisions.
- Do not write acceptance criteria.
- Do not finalize or reconcile a design handoff.
- Do not create implementation tasks.
- Do not treat chat-only or wrong-repo research as enough for design.
- Do not emit `READY_FOR_DESIGN` without a first design fork.
- Do not put the full receipt contract in Harness; Harness gets only router
  guidance.

## Examples

Use `reference/examples.md` for regression examples, including Sales Copilot,
skip receipts, stale research, and hosting mismatch.

## Hand-off

`READY_FOR_DESIGN` plus compatible hosting -> `maestro-design` reads
`research.md` first.

`NEEDS_STAKEHOLDER` or `NEEDS_EVIDENCE` -> stay on the same card and resolve the
listed blocker.

`PIVOT` -> update the problem statement or create a linked successor.

`STOP` -> preserve the receipt and archive or close the candidate when the
owning lifecycle allows it.
