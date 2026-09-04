---
name: maestro-design
description: Settle unknowns and lock decisions before implementation - pick the mode per unknown (grill, research, prototype, model, wayfind), recall past bundles, walk one fork at a time, record every settled choice with a rationale, and open the bundle only when a Full trigger holds.
review-date: 2026-11-28
---
<!-- maestro-skill-version: dev -->

# maestro-design

Use when the request, acceptance, authority, or implementation boundary is
unsettled. Design is human-guided whenever a choice changes what will be
built. Do not begin implementation until the relevant decisions are locked and
the user has approved the resulting scope. Design is read-only toward
production code and authorizes nothing.

## Mode per unknown

Identify what kind of unknown blocks progress, then load only the reference
that resolves it:

| The unknown | Mode | Reference |
|---|---|---|
| Decisions only the user can make, several and interdependent | grill | [references/grilling.md](references/grilling.md) |
| A decision owned by someone not in the conversation | route to `maestro-questionnaire` | |
| A fact in docs, APIs, or source outside this repo | research | `maestro-explore` (research mode) |
| "Does this state model feel right?" or "What should it look like?" | prototype | `maestro-explore` (prototype mode) |
| Fuzzy terminology, or a hard-to-reverse choice worth recording | model | [references/domain-modeling.md](references/domain-modeling.md) |
| The effort exceeds one session and is wrapped in fog, or the user does not know what to do next | wayfind | [references/wayfinder.md](references/wayfinder.md) |

Facts are yours to find; decisions are the user's. Never ask the user for
anything you could look up. Modes compose: grill runs with the glossary in
hand; wayfind dispatches grill, research, and prototype per child work item.
Each design pass must close at least one fork; a pass that closes none
surfaces the blocker to the user instead of looping.

## Intake

Pin the problem before choosing a method:

> For [who], reach [observable outcome] within [boundary], because [impact],
> without [excluded effect].

Route uncertainty to a lane:

| Current uncertainty | Lane |
|---|---|
| state unknown | scout, no-write |
| several architectures | two or three decision lanes |
| contract clear | delivery |
| candidate needs breaking | challenge |
| hard-to-reverse fork | council |

Score the ROI of independent judgment from 0 to 2 for each question:

1. Would wrong framing be costly?
2. Is the decision hard to reverse?
3. Is the domain new to the owner?
4. Can independent judgment produce a materially different option?
5. Is human attention fragmented?

Route totals of 0-2 to the direct session, 3-5 to a Lead plus one peer, 6-8 to
several lanes, and 9-10 to a council. Before launching any lane, pin the
branch and commit, active writers, and dirty paths.

## Recall pass first

Before proposing anything, search the store for precedent:

```
maestro search "<topic keyword>"        # hits labeled work/decision/note/bundle
maestro bundle show <id>                # deep-read a bundle hit
maestro decision list                   # locked choices that bind this scope
```

A past bundle that settled the same fork is evidence; cite it instead of
re-deriving the argument.

## Working method

- Read the current `maestro work show`, linked decisions, notes, and source.
- Present ONE unresolved fork at a time with a concrete recommendation.
- Record each settled fork immediately:
  `maestro decision draft "<choice>" --rationale "<why, with the rejected alternative>" --work <id>`
  then `maestro decision lock <id>`. Supersede an old decision with
  `--supersedes`; never rewrite its history.
- Keep acceptance, non-goals, and authority visible on the work item.
- Do not edit code during a design-only engagement.

## Council

Use a council only for a hard-to-reverse fork with wide blast radius. Dispatch
two or three fresh-context decision lanes with the same neutral brief and no
hint of the Lead's preference. Seal the council: the Lead reads no view until
all views have returned. File one dispatch per lane on the same work item; every
envelope line stays identical across lanes except `lane:`, and the shared
question is each dispatch's objective.

Reconcile the views through eight axes: `premise`, `mechanism`, `boundary`,
`failure`, `reversibility`, `evidence`, `authority`, and `proof`. Never count
votes. Draft the result with `maestro decision draft --rationale`, preserving
the losing side's dissent in the rationale.

The candidate under review stays frozen. A new finding creates a new candidate
instead of silently changing the stable candidate mid-review.

## Readiness gate and exit

Count the material forks still open before writing anything down. More than
two open forks: keep grilling (or route the unknown by the mode table); too
foggy to state even the problem: wayfind. Forks the conversation already
settled are synthesized, never re-asked. An external claim entering a decision
(API behavior, library semantics, versions) comes from research against
primary sources, never from memory.

Then exit by tier (`maestro-bundle` tier rule):

- Light: design ends with a work item whose acceptance fits in one sentence,
  `maestro work add "<title>" --acceptance "<observable result>"`, plus the
  locked decisions. No SPEC, no red-test list; the work is verified
  inline by `maestro-work`. A quickfix never reaches design.
- Full: `maestro bundle open <id> --work <workId>`, then fill SPEC.md as a
  pure contract: Problem, Solution, Scope, Anti-goals (each traces to a real
  risk in this repo and gets a matching VERIFY.md check; an anti-goal that
  cannot be checked is a wish, not a constraint), Decisions (ids only,
  `maestro bundle show <id>` renders them), and Red tests: one failing test at
  an accepted seam per risk the SPEC names, nothing beyond that list. Work
  with no executable seam (docs, config) or behavior-preserving work
  (upgrades, refactors) lists VERIFY.md scenarios, a readback, diff, or
  captured baseline, not tests. Draft the VERIFY.md rows from the red list and
  anti-goals; seed NOTES.md with Current State, Next Action, and `Base:`.

If the contract would need guessing to write a red test or an acceptance
sentence, the fork is not settled: return to the walk. Finish with the next
fork, an explicit implementation gate, or a named blocker. A SPEC authorizes
nothing; implementation starts only on the user's explicit request.

For unattended/away-mode design constraints, read
[references/unattended.md](references/unattended.md).
