---
name: maestro-design
description: Settle unknowns and lock decisions before implementation - recall past bundles, walk one fork at a time, record every settled choice with a rationale.
review-date: 2026-11-28
---
<!-- maestro-skill-version: dev -->

# maestro-design

Use when the request, acceptance, authority, or implementation boundary is
unsettled. Design is human-guided whenever a choice changes what will be
built. Do not begin implementation until the relevant decisions are locked and
the user has approved the resulting scope.

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

## Exit

Design ends with a SPEC (pure contract in the bundle) plus a red-test list:
one failing test per decided behavior at an accepted seam. If the contract
would need guessing to write a test, the fork is not settled - return to the
walk. Finish with the next fork, an explicit implementation gate, or a named
blocker.

For unattended/away-mode design constraints, read
[references/unattended.md](references/unattended.md).
