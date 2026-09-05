---
name: maestro-council
description: Lead-only council for a hard-to-reverse fork - a neutral brief, sealed independent seats, one premise verifier on unanimity, bounded verifiers, one cross-examination round, a draft-verdict audit, and one binding verdict recorded as a decision with its dissent. Never inside a seat.
review-date: 2026-11-28
---
<!-- maestro-skill-version: dev -->

# maestro-council

Use when `maestro-design` intake scores yes on every ROI question including
hard to reverse. The Lead is the final arbiter: it frames, seals, collects,
verifies, drafts, and decides. It never performs a seat's analysis itself and
never spawns another Lead. Council ends at a decision and a handoff contract;
seats never implement.

## Lead-only guard

Proceed only as the Lead of a running team, or in a
plain session outside a team. A Peer, Verifier, Auditor, or any other seat
refuses in one line and returns to its own assignment. A seat never opens a
council. No Observer seat exists (Hub d98); a council has exactly the seats
its tier names.

The owner chooses the Lead's model and effort; do not second-guess it. Seat
routing lives in the five seat profiles of bundle slp-profiles (independent,
challenger, specialist, verifier, auditor); this skill carries no routing
table.

## Protocol at a glance

```text
tier    -> the smallest sufficient tier, in one sentence
brief   -> neutral brief + case output contract + framing lint
sealed  -> one bound work item per round 1 seat, read nothing until all return
collect -> seat audit, failure policy
model   -> typed claims with statuses
verify  -> premise verifier on unanimity, bounded verifiers on dispute
cross   -> one challenge and one response per disputed unit
draft   -> the Lead drafts alone
audit   -> auditor by tier
verdict -> decision draft with dissent, handoff note on the work item
```

Re-anchor on this list when unsure which step is active. Announce each phase
in one line.

## Tier

- `lens`: one Independent.
- `debate` (default): Independent + Challenger.
- `debate-with-proof`: Independent + Challenger, Verifiers as needed, audit by
  default.
- `high-risk`: Independent + Challenger at the high-risk profile, optionally
  one Specialist, Verifiers as needed, audit mandatory.

Pick the tier in one sentence. The design ROI score decides only whether a
council happens; it never picks the tier.

## Brief

Write the neutral brief from the template in
[references/brief.md](references/brief.md). The original request stays
verbatim. The snapshot is the branch, commit, and dirty paths, pinned as
`maestro-design` intake already requires; a hash detects drift, it does not
preserve bytes. Design the case output contract for this case's natural
units (findings, options, gates, timeline) from the patterns in
[references/report-format.md](references/report-format.md); never force
`POSITION` or a fixed claim count where it does not fit.

Framing lint: repair the brief until every answer is yes.

- Does it preserve the user's original request?
- Does no wording imply a preferred verdict?
- Does every authoritative fact carry authority or provenance?
- Are unverified premises listed as claims, not facts?
- Are hard constraints separate from preferences?
- Is no option excluded without an authoritative reason?
- Can seats investigate independently within the authorized scope?
- Is the snapshot current where source state matters?
- Does the output contract keep every unit the user expects adjudicated?
- Does no requested heading create filler or seed a conclusion?

Ask the user only when missing authority or scope would change the decision.
After lint, the next action is opening the seats; no more context gathering.

## Sealed round 1

Every core seat receives the identical brief and output contract plus
exactly one role line:

- Independent: reason from first principles, recommend the strongest answer,
  expose the decision-critical assumptions.
- Challenger: test the framing and the shared premises, build at least one
  viable counterfactual, say what it makes unnecessary; do not manufacture
  disagreement.
- Specialist: apply only the requested domain semantics; expertise does not
  override stronger evidence or product authority.

Open every seat before reading any report. Round 1 is sealed: no seat sees
the Lead's view, another report, or another seat's identity, and the Lead
reads nothing until every required seat has returned. Seats are analysis
only: no edits, no spawning, no contact with other seats, no council skill.

## Collect and audit seats

After every required seat returns:

1. Audit the run checkout with `git status --short` against the pinned dirty
   paths, and read `maestro trace <id>` for each seat's work item.
2. A seat that wrote a file, opened work, or contacted another seat is
   `COMPROMISED`: its report is unused and named as such in the verdict
   (Hub d93). Isolation is by profile (Claude `disallowedTools`, Codex
   read-only sandbox under the subagent executor) and audited here; never
   claim a write was technically impossible.
3. Failure policy: one retry with the same brief and snapshot for an
   infrastructure or output-contract failure; a format-only failure gets one
   request for the missing content; a compromised seat gets one fresh
   replacement. `lens` cannot issue a verdict without its seat; `debate`
   tiers continue with one missing core seat only as `DEGRADED`; `high-risk`
   never issues a normal verdict without both core seats.

## Decision model

Reduce valid reports into the smallest model that keeps every natural unit.
Type each material claim when its type sets the evidence bar: `FACT`,
`INFERENCE`, `CAUSAL CLAIM`, `FORECAST`, `VALUE / PREFERENCE`,
`AUTHORITATIVE CONSTRAINT`. Statuses are exactly:

```text
verified | falsified | authoritative | supported inference |
contested inference | unresolved | insufficient coverage | snapshot mismatch
```

Only facts and direct observations are eligible for factual verification.
`insufficient coverage` is never evidence that a claim is false. Do not build
a claim graph or a store; the model lives in the Lead's draft.

## Verify

Unanimity is not a skip: above lens, when every valid seat agrees, open exactly one Verifier whose single mandate is to name the shared premise in the brief that drives the common conclusion and test it.
Verified leads to the Lead draft; falsified opens a second generation with a
corrected brief (Hub d94). Only a material dispute opens the full path below.

For a material factual dispute, open one to three Verifiers, each with one
precise proposition, the authorized sources, and one distinct mandate:
supporting evidence, disconfirming evidence, or coverage audit. Use only the
mandates the proposition needs; identical prompts never form an ensemble vote
(Hub d95). A Verifier returns:

```text
PROPOSITION CHECKED
MANDATE
SOURCES OR LOCATIONS SEARCHED
DIRECT OBSERVATIONS
RESULT: verified | falsified | partial | insufficient coverage | snapshot mismatch
LIMITATIONS
```

A `snapshot mismatch` stops that proposition until the source is refreshed.

## Cross-examination

At most one challenge and one response per disputed unit; never free-form
debate. Under a team the Lead opens a second generation (d688): each seat
receives the other seats' reports verbatim plus one targeted question and
answers by return. Under the subagent executor a seat's own report goes to a
fresh instance with the same question. The response schema:

```text
PROPOSITION_ID
RESPONSE: CONCEDE | MAINTAIN | NARROW | REVERSE
REASON
DIRECT EVIDENCE
NEW CLAIMS, if any
FALSIFIER
```

New material factual claims return to a Verifier; they never reopen debate.

## Lead draft

The Lead, not the seats, decides: the authoritative outcome and hard
constraints; options excluded by verified constraints; verified, falsified,
and unresolved premises; fit under realistic failure modes; robustness if an
assumption is wrong; reversibility; whether serious dissent has stronger
evidence or a decisive falsifier. No vote, no averaged confidence; seat count
never creates authority. Draft before deciding whether an audit is due.

## Audit

- `debate`: only with material dissent, an unresolved high-impact claim, or
  a fragile chain.
- `debate-with-proof`: default.
- `high-risk`: mandatory.

One fresh Auditor receives the brief, every valid report attributed by role
only, the decision model, verified evidence, the draft, and the dissent;
never seat identities or transcripts. It returns, with findings per
[references/report-format.md](references/report-format.md):

```text
AUDIT RESULT: CLEAR | REVISE | STOP
```

Resolve every material finding by revising the draft, dropping the
unsupported claim, or returning the proposition to a Verifier. At most one
audit round; the Auditor never replaces the verdict.

## Verdict and handoff

Record the binding verdict:

```
maestro decision draft "<decision>" --rationale "<why; accepted and rejected claims; the dissent and the Lead's answer to it>" --dissent "<losing view>" --work <id>
maestro decision lock <id>
```

Then note the handoff contract on the work item with `maestro work note <id>`:
required action and owner, do-not-touch boundaries, validation, limitations
(degraded, incomplete coverage, audit skipped), and reopen conditions. When a
bundle follows, the do-not-touch boundaries become its SPEC anti-goals. A
later implementer receives that note, never the seat reports; a fresh
validator checks the implementation against it without reopening the
architecture unless a reopen condition fires.

## Stopping rules

- one sealed round 1;
- one premise Verifier on unanimity, bounded Verifiers on dispute;
- at most one challenge and one response per disputed unit;
- at most one audit round;
- no voting, no group chat, no seat-to-seat prompts;
- no edits by any seat, no new worktree for an ordinary council;
- no daemon, queue, claim graph, or standing council team.

## Run

`graph run council` runs the protocol as the third graph-engine preset,
binding one work item per seat. Until that preset ships, map each seat by
hand:

| Executor | Open a seat | Read its report |
|---|---|---|
| Lead of a running team | `maestro work add "<case> <seat>" --to peer-<seat> --acceptance "<output contract>"` | `maestro work show <id> --notes all` after `work return` |
| Plain session, Claude | the Agent tool with `subagent_type: maestro-<seat>` and the brief as the prompt | the agent's final report |
| Plain session, Codex | `spawn_agent` with agent type `maestro-<seat>` and the brief as the prompt | the agent's final report |

`<seat>` is one of independent, challenger, specialist, verifier, auditor.
The classic `maestro dispatch open --council-members <n>` plus `--council-anchor`
path and the `COUNCIL_REQUEST` handback status stay valid for a plain session
until the preset ships.
