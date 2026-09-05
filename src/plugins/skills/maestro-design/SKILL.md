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

Weigh the ROI of independent judgment on five questions:

1. Would wrong framing be costly?
2. Is the decision hard to reverse?
3. Is the domain new to the owner?
4. Can independent judgment produce a materially different option?
5. Is human attention fragmented?

Mostly no: the direct session. Mixed: a Lead plus one peer. Mostly yes:
several lanes. Yes on every question including hard to reverse: a council.
Before launching any lane, pin the branch and commit, active writers, and
dirty paths.

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

A hard-to-reverse fork with wide blast radius runs the `maestro-council`
protocol: neutral brief, sealed seats, one premise verifier on unanimity,
bounded verifiers, one cross-examination round, an audit by tier, and one
binding verdict recorded with `maestro decision draft --rationale` carrying
the dissent. The candidate under review stays frozen; a new finding creates
a new candidate.

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
- Full: `maestro bundle open <id> --work <workId>`, opened in the
  store whose checkout will change (a walk run in the Hub room still opens
  its bundle where the code lives; note the bundle on the Hub map and the map
  on the bundle's work item), then fill SPEC.md as a pure contract: Problem,
  Solution, Scope, Anti-goals (each traces to a real risk in this repo and gets a matching
  VERIFY.md check; an anti-goal that cannot be checked is a wish, not a
  constraint), Decisions (ids only, Hub decisions as `hub:<id>`;
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
