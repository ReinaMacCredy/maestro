---
name: maestro-council
description: Lead-only council for a hard-to-reverse fork, run by hand with ordinary sub-agents - a neutral brief, sealed independent seats, one premise check on unanimity, one cross-examination round, a draft verdict one more reader audits, and one binding verdict recorded as a decision with its dissent. Never inside a seat.
review-date: 2026-11-28
---
<!-- maestro-skill-version: dev -->

# maestro-council

Use when `maestro-design` intake scores yes on every ROI question including
hard to reverse. The Lead frames, seals, collects, drafts, and decides. It
never performs a seat's analysis itself, and seats never implement. A council
ends at a decision and a handoff note.

## Lead-only guard

Proceed only as the Lead of a running team, or in a
plain session outside a team. Any other seat refuses in one line and returns
to its own assignment. A seat never opens a council.
No Observer seat exists (Hub d98); a council has exactly the seats below.

## Steps

1. Confirm a council is warranted: the fork is hard to reverse and its blast
   radius is wide. A reversible choice is a plain `maestro decision add`.
2. Write the neutral brief from [references/brief.md](references/brief.md).
   Keep the original request verbatim, give every fact its provenance, list
   unverified premises as claims, keep hard constraints apart from
   preferences, and pin the branch, commit and dirty paths. Repair the brief
   until no wording implies a preferred verdict. Choose the output contract
   every seat returns from
   [references/report-format.md](references/report-format.md).
3. Open the seats, sealed. Spawn two or three ordinary sub-agents at once.
   Each gets the same brief and output contract, a model you name, and one
   role line: reason from first principles and recommend the strongest
   answer; test the framing and the shared premises and build at least one
   viable counterfactual; or, only when the case needs it, apply one domain's
   semantics. Put at least one seat on a different model family where the
   harness offers one. Seats are
   analysis only: no edits, no spawning, and no seat sees another seat's
   report or your view. Read nothing until every seat has returned.
4. Collect. Compare `git status --short` with the pinned dirty paths. A seat
   that wrote a file or contacted another seat is COMPROMISED: drop its
   report, name it in the verdict, and open one fresh replacement.
5. Check the shared premise once when the seats agree. Unanimity is not a skip: when every valid seat agrees, open exactly one more sub-agent whose single mandate is to name the shared premise in the brief that drives the common conclusion and test it.
   A verified premise goes to the draft; a falsified one sends a corrected
   brief to a new sealed round.
6. Cross-examine once. For each material dispute, give a fresh sub-agent one
   seat's report, the opposing reports verbatim, and one targeted question.
   It answers `CONCEDE | MAINTAIN | NARROW | REVERSE` with its reason, direct
   evidence, and falsifier. One challenge and one response per disputed
   point; a new factual claim gets one check by a fresh sub-agent, never
   another round.
7. Draft the verdict alone. Reconcile the reports on eight axes: `premise`,
   `mechanism`, `boundary`, `failure`, `reversibility`, `evidence`,
   `authority`, and `proof`. No vote and no averaged confidence; seat count never creates
   authority. Answer the strongest dissent in the draft.
8. Audit the draft once. Give one fresh sub-agent the brief, the reports
   attributed by role only, the draft, and the dissent. It returns
   `CLEAR | REVISE | STOP` with findings. Resolve each material finding by
   revising the draft or dropping the unsupported claim. The auditor never
   replaces the verdict.
9. Record one binding verdict:

   ```
   maestro decision add "<decision>" --rationale "<why; accepted and rejected claims; the Lead's answer to the dissent>" --dissent "<losing view>" --work <id>
   ```

10. Note the handoff on the work item with `maestro work note <id>`: the
    required action and owner, do-not-touch boundaries, validation,
    limitations (a dropped seat, incomplete coverage), and reopen conditions.
    A later implementer receives that note, never the seat reports.

## Stopping rules

- one sealed round, one premise check, one cross-examination, one audit;
- no voting, no group chat, no seat-to-seat prompts;
- no edits by any seat, and no standing council team.
