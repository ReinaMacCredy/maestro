---
name: maestro-verify
description: Verify and close - cross-check coverage, run the VERIFY table, deliver the verdict, harvest durable lessons into decisions, close the bundle, and never claim remote state from local evidence.
review-date: 2026-11-28
---
<!-- maestro-skill-version: dev -->

# maestro-verify

Use for close, commit, install, push, publish, release, or archive gates.
Local implementation authority does not imply authority for remote or external
state changes.

Precondition: an open bundle with a drafted VERIFY.md. No bundle means the
change is quickfix or Light: verify the changed surface inline and close with
`maestro work done`; this skill's table pass is a Full-tier instrument. The
evidence-layer vocabulary below still applies to any claim at any tier.

## Evidence layers

Proof follows five links. Claim only as far as the last proven link.

- `source` - source-level tests, lint, type checks, or direct inspection.
- `artifact` - the built or packaged output is present and has been read back.
- `installed` - the installed stamp, version, or files match the intended artifact.
- `live` - the running process, pid, or active runtime matches the installed layer.
- `journey` - the real user path reaches the observable outcome end to end.

"Tests pass" is a source claim. A claim that touches install or runtime must
include a readback at that layer. Every proof and VERIFY result lists untested
links explicitly as `NOT TESTED`, never by omission:

```text
proof: "suite 135 pass @ a52bd4a7 (source); runtime stamp readback a52bd4a7 (installed); live: NOT TESTED"
Assumptions not verified: None
Residual risks: None
```

## Verify

- Cross-check coverage before running anything: every behavior in scope has a
  red test that went green, every red test maps to a VERIFY.md scenario or
  repo check, and every scenario traces back to a work item's acceptance or an
  anti-goal. An orphan on any side is a gap - record and surface it, never
  silently proceed past it.
- Run every VERIFY.md scenario against its work item's acceptance/claims and
  fill the Result column; run each anti-goal check (grep, diff, readback).
  Stamp the pass with its date and commit. Results hold this run only: a
  re-run replaces prior results wholesale, and a failed pass leaves its
  one-line `failed:` note on the work item, never accumulated rounds in
  VERIFY.md. The scenario list is frozen once the pass starts: scenarios gain
  results here, never rewrites or removals. A scenario that cannot run as
  written goes back to `maestro-design` for a checkable rewrite - do not
  invent a substitute measurement.
- Run the repo's checks for the touched surface (tests, lint, types, build),
  then freeze and review the task-owned diff: every changed line traces to
  the SPEC's scope or a linked work item; nothing unrelated is staged.
- For risky seams, spot-check assertion strength before filling PASS.
  First check the tests assert the decided contract itself: the decided
  error class, and the message when one was decided - a bare `toThrow()`
  passes on any thrown value, and a substring matcher like
  `toThrow(string)` passes on a changed message; a decided contract no
  assertion pins is a FAIL. Then derive mutants from the record, not at
  random: bend the code toward each alternative the linked decisions
  rejected - the suite must go red each time, and a survivor is a weak or
  missing test and a FAIL of that scenario, not a side note. Last, probe
  each input edge no decision settled (whitespace, case, sign, empty) by
  mutating the code (e.g. insert an `input.trim()`), never by only calling
  the function - a call shows current behavior, a surviving mutant shows no
  test pins it; a suite that stays green under an edge mutant is an open
  fork to record, not a pass. Restore after each mutant.
- Re-read the user's exact delivery authority and target before any gate.
- Select one legal next gate at a time: final verification, independent QA or
  witness, scoped commit, local install, external delivery, or stop. Do not
  bundle gates whose authority differs.
- Read back the actual result: test output, commit hash, installed version. A
  started or interrupted command is not delivery evidence.

For substantial diffs, verify in a fresh context: dispatch a subagent that
reads only the bundle and the diff - the implementer verifying their own work
invites confirmation bias. The subagent never fixes anything: mutants it flips
are reverted before reporting, and on FAIL it records the verdict and stops;
routing back to implementation belongs to the parent turn that holds the
user's ask. A subagent that fails to start or report is a dispatch failure,
not evidence: run the checklist in this session instead of polling for it.

On FAIL, route back to `maestro-work` and leave the exact one-line failed-pass
trace `maestro work note <id> "failed: <one line>"`. The prefix is the literal
lowercase `failed:` followed by one space. A scenario still failing after three implement
passes - counted from the work item's notes across sessions, not this
session's memory - is a design problem, not an implementation one: stop and
re-settle the decision via `maestro-design`.

Read-only review method: [references/audit.md](references/audit.md). When the
failure location is unclear, follow [references/triage.md](references/triage.md).

## Red flags

| The thought | The reality |
|---|---|
| "It obviously passes - running it is a formality" | Scenarios exist because "obviously" has been wrong before. Run every one and record the output. |
| "The scenario can't run as written, but this similar check proves the same thing" | That is a substitute measurement. Route back to `maestro-design` for a checkable rewrite. |
| "The mutant survived, but the code is clearly fine" | A surviving mutant is a weak or missing test, and a FAIL of that scenario. |
| "I wrote this diff - I know it works" | That is the confirmation bias the fresh-context rule exists for. |
| "I'll just fix this small failure while I'm verifying" | Verify delivers a verdict, never fixes. A FAIL routes back to `maestro-work`. |

## Learn, then close

Before closing, harvest what outlives the bundle
([references/learning.md](references/learning.md)): a verified correction or
durable constraint becomes a locked decision or a work note - never only chat.

Close order, on PASS with durable ship or handoff proof:

1. Overwrite NOTES.md one last time with a dated close-out line citing the
   ship evidence (commit hashes or the handoff target).
2. Harvest: any mid-flight choice that is hard to reverse, surprising without
   context, and a real trade-off is a locked decision with its rejected
   alternative; a new domain term is `maestro term add`.
3. `maestro bundle close <id>`: snapshots the trio into the store and archives
   the directory.

The snapshot is the durable memory; after close the directory is disposable
and `maestro search` still recalls the text.

When the verdict passes but the ship commit has not landed yet, do not leave
the close implicit: set NOTES.md Next Action to "commit, then close bundle".
The turn that lands the commit performs the close in that same turn; a PASS
bundle never stays active across sessions. Never close on a FAIL, and never
stage or commit bundle contents as part of the ship commit.

Quality review is separate from verify: verify owns "does it meet the
contract", review owns "is the code good". Light gets a simplification pass
after green; Full gets one correctness review after verify passes, chosen by
risk (a security review when the diff touches auth, secrets, or input
handling). A code change after the verdict re-runs the affected VERIFY.md
scenarios before close.

## Definition of done

Acceptance met, changed surface verified, available test/lint/type/build
checks pass, claims name their falsifier, risky changes carry rollback notes.
Never claim push, release, or publish from local state; those gates are the
user's.
