---
name: maestro-design
description: Resolve material unknowns blocking the next authorized slice, using research, grilling, prototypes, models, or wayfinding. Record durable decisions and apply the shared workflow tier rule.
review-date: 2026-11-28
---
<!-- maestro-skill-version: dev -->

# maestro-design

Use when a material choice blocks the next slice. Read
[Decisions and readiness](~/maestro/WORKFLOW.md#decisions-and-readiness) and
[Authorization boundaries](~/maestro/WORKFLOW.md#authorization-boundaries).
Design is read-only toward production code and authorizes nothing.

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

Facts are yours to find; material product and scope choices are the user's.
Never ask the user for anything you could look up. Modes compose: grill runs with the glossary in
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
- Present a blocking user-owned fork with a concrete recommendation;
  resolve reversible implementation details within the approved scope directly.
- Record durable decisions under WORKFLOW.md's threshold:
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

Check readiness of the next bounded slice, not the number of open questions
about the whole project. Keep later questions visible without blocking an
independent slice. If the problem itself is unclear, wayfind. Forks already
settled are synthesized, never re-asked. An external claim entering a decision
(API behavior, library semantics, versions) comes from research against
primary sources, never from memory.

Then exit by [Tiers](~/maestro/WORKFLOW.md#tiers):

- Light: design ends with a work item with clear acceptance,
  `maestro work add "<title>" --acceptance "<observable result>" --kind <kind>`,
  plus any durable decisions. Kind routes the policies: `feature`, `task`,
  `bug`, `chore`, `implement` are execution units; `idea` and `research` are
  scope notes under a parent and never hold it open. The why lives in the
  title or acceptance; when it needs a paragraph, add
  `maestro work note <id> "why: <paragraph>"`, and record findings from the
  research mode as `research: <finding>` notes (what `policy-research` reads
  when enabled). No SPEC is required; the work is verified inline by
  `maestro-work`. A quickfix never reaches design.
- Full: `maestro bundle open <id> --work <workId>`, opened in the
  store whose checkout will change (a walk run in the Hub room still opens
  its bundle where the code lives; note the bundle on the Hub map and the map
  on the bundle's work item), then fill SPEC.md as a pure contract: Problem,
  Solution, Scope, Anti-goals (each traces to a real risk in this repo and gets a matching
  VERIFY.md check; an anti-goal that cannot be checked is a wish, not a
  constraint), Decisions (ids only, Hub decisions as `hub:<id>`;
  `maestro bundle show <id>` renders them). Plan checks using
  [Testing discipline](~/maestro/WORKFLOW.md#testing-discipline), including
  existing checks and necessary new tests, not a test quota. Draft VERIFY.md
  from acceptance, relevant risks, and anti-goals; seed NOTES.md with Current
  State, Next Action, original authorization, and `Base:`.

If the next slice's acceptance or authority needs guessing, resolve that
blocker. Otherwise continue implementation when the user's original request
already authorizes it; do not ask again merely because design is complete.
For a design-only request, finish with the proposed scope and implementation
gate. A SPEC authorizes nothing.

For unattended/away-mode design constraints, read
[references/unattended.md](references/unattended.md).
