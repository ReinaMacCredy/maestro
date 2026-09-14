---
name: maestro-verify
description: Verify and close - cross-check coverage, run the VERIFY table, deliver the verdict, harvest durable lessons into decisions, close the bundle, and never claim remote state from local evidence.
review-date: 2026-11-28
---
<!-- maestro-skill-version: dev -->

# maestro-verify

Use for verification and close. Read [WORKFLOW.md](~/maestro/WORKFLOW.md)
for testing, recovery, authorization, and completion rules. Delivery actions
such as commit, install, push, or release remain separate gates.

Precondition: an open bundle with a drafted VERIFY.md. Without a bundle,
verify inline and close a tracked item with `maestro work done`; an untracked
quickfix needs no record. This skill's table pass is for Full work. The
evidence-layer vocabulary below still applies to any claim at any tier.

## Evidence layers

Proof follows five links. Claim only as far as the last proven link.

- `source` - source-level tests, lint, type checks, or direct inspection.
- `artifact` - the built or packaged output is present and has been read back.
- `installed` - the installed stamp, version, or files match the intended artifact.
- `live` - the running process, pid, or active runtime matches the installed layer.
- `journey` - the real user path reaches the observable outcome end to end.

"Tests pass" is a source claim. A claim that touches install or runtime must
include a readback at that layer. A suite that is green only on this machine
is not a `source` claim about the repo: before any commit, release, or handback
gate, re-run the touched suite with the developer environment removed
(`HOME=$(mktemp -d)`, `env -u HERDR_ENV`). A test that reads the installed
copy, the room, or a home config passes for you and fails in CI. Every proof
and VERIFY result lists untested links explicitly as `NOT TESTED`, never by
omission:

```text
proof: "suite 135 pass @ a52bd4a7 (source); runtime stamp readback a52bd4a7 (installed); live: NOT TESTED"
Assumptions not verified: None
Residual risks: None
```

## Verify

- Cross-check the evidence plan against acceptance and relevant risks. Map
  existing tests, necessary new checks, readbacks, or baselines to VERIFY.md.
  A missing behavior check is a gap; the absence of a newly written test is not.
- Run every VERIFY.md scenario against its work item's acceptance/claims and
  fill the Result column; run each anti-goal check (grep, diff, readback).
  Stamp the pass with its date and commit. Results hold this run only: a
  re-run replaces prior results wholesale, and a failed pass leaves its
  one-line `failed:` note on the work item, never accumulated rounds in
  VERIFY.md. Apply [Recovery and verification](~/maestro/WORKFLOW.md#recovery-and-verification)
  when a scenario cannot run as written: document and execute an equivalent
  check without changing acceptance, or report the gap if equivalence is unknown.
- Run the repo's checks for the touched surface (tests, lint, types, build),
  then freeze and review the task-owned diff: every changed line traces to
  the SPEC's scope or a linked work item; nothing unrelated is staged.
- For a concrete assertion-strength concern, inspect whether the existing
  check distinguishes the approved outcome from the suspected wrong behavior.
  A focused mutation can establish that; restore it before continuing. Do not
  expand verification into an unrelated edge-case or coverage campaign.
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

On FAIL, leave `maestro work note <id> "failed: <one line>"` and return the
evidence to the implementation owner. Use the shared recovery rule to choose
the next action from the cause, not a failure count. Read prior failed notes
so a new session does not repeat the same uninformative attempt.

Read-only review method: [references/audit.md](references/audit.md). When the
failure location is unclear, follow [references/triage.md](references/triage.md).

## Red flags

| The thought | The reality |
|---|---|
| "It obviously passes - running it is a formality" | Scenarios exist because "obviously" has been wrong before. Run every one and record the output. |
| "The scenario command is stale, so I can skip the check" | Repair it or demonstrate an equivalent measurement; preserve acceptance and record the change. |
| "The mutant survived, but the code is clearly fine" | If the mutant violates acceptance, the check is weak; report the gap rather than filling PASS. |
| "I wrote this diff - I know it works" | That is the confirmation bias the fresh-context rule exists for. |
| "I'll just fix this small failure while I'm verifying" | Verify delivers a verdict, never fixes. A FAIL routes back to `maestro-work`. |

## Learn, then close

Before closing, harvest what outlives the bundle
([references/learning.md](references/learning.md)): a verified correction or
durable constraint becomes a locked decision or a work note - never only chat.

Use [Completion and delivery](~/maestro/WORKFLOW.md#completion-and-delivery)
to decide whether the accepted scope is complete or an authorized transfer is
ready. Close procedure:

1. Run `maestro handoff <bundle-id>` one last time, then add a dated
   close-out line citing verification evidence and the candidate (base commit
   plus task-owned diff if uncommitted), or the explicit handoff/cancellation.
   Record pending delivery actions and retained authority in the handoff.
2. Harvest: any mid-flight choice that is hard to reverse, surprising without
   context, and a real trade-off is a locked decision with its rejected
   alternative; a new domain term is `maestro term add`.
3. `maestro bundle close <id>`: snapshots the trio into the store and archives
   the directory.

The snapshot is the durable memory; after close the directory is disposable
and `maestro search` still recalls the text.

If acceptance includes delivery not yet authorized or proven, leave the work
open with that exact next action and blocker. Otherwise a verified implementation
may close without a commit. Do not mark failed acceptance complete; an explicit
transfer or cancellation records the unresolved failure rather than calling it
PASS. Never stage or commit bundle contents.

Use the shared [review routing](~/maestro/WORKFLOW.md#per-tool-adapters).
A code change after the verdict reruns the affected VERIFY.md scenarios
before close; the old verdict does not cover the new diff.

## Definition of done

Acceptance met, changed surface verified, available test/lint/type/build
checks pass, claims name their falsifier, risky changes carry rollback notes.
Never claim push, release, or publish from local state; those gates are the
user's.
