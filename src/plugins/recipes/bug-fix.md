# Bug fix

Use this recipe for a reported defect to reproduce, root-cause, and fix with
runtime evidence. A request that asks only for the cause, with no fix, runs
through step 4 and stops there. Ported from pstack's bug-fix playbook (Lauren
Tan, MIT; notice in THIRD_PARTY.md).

You own this task. Plan, review, verify. Delegate investigation and the fix to
subagents and stay in the lead.

Be scientific. Every shipped line traces to runtime evidence. A
belt-and-suspenders change that "might help" is a hypothesis, not a fix. It
does not ship. When evidence refutes a hypothesis, revert what it motivated.
The smallest change the evidence justifies ships, nothing more.

## Steps

1. Recall first. Run `maestro search "<symptom>"` and `maestro attention`, and
   read prior `failed:` work notes on the same mechanism. A prior failure is
   evidence about which hypotheses are already refuted.
2. Reproduce it yourself on the same product the report came from, through the
   repository's verify-<repo> skill (`maestro recipe show verification-skill`),
   even when a debug or instrumentation protocol says to ask the user to
   reproduce. Capture the exact failing command or input and its output.
   - Ask the owner (WORKFLOW.md Per-tool adapters) only with a stated,
     specific reason the skill cannot reach the target, and only after driving
     it as far as it goes.
   - If it will not reproduce directly, synthesize the trigger, tighten
     conditions, or instrument until it fires.
   - No reproduction means stop and say so. Do not guess from symptoms.
3. Binary-search the cause. Form the candidate hypotheses, then rule them out
   until one survives.
   - Seed them by tracing the affected code path with file:line citations and
     by reading its history (`git log -p`, `git blame`, the PRs behind it) for
     a regression.
   - Write each hypothesis down before its probe ("I believe X causes this
     because Y"), then run the smallest check that can falsify it. No
     hypothesis, no probe.
   - Each pass, take the split that cuts the most remaining problem space (a
     layer, a module, a commit through `git bisect`, an input feature), get
     runtime evidence, eliminate.
   - When program state is unclear, add instrumentation or logging and read it
     as the code runs. Do not guess.
   - Drive a long or stubborn hunt with an external driver
     (`maestro recipe show unattended`).
   - Three refuted hypotheses in a row means stop treating it as a local bug.
     Write down the premise they shared and test it directly, per **Attack the
     Premise** (PRINCIPLES.md). A structural cause or an unsettled design
     decision goes to the maestro-design skill, not a fourth probe.
   - A fix proposed before the cause is known, several changes at once, or
     "probably X" without a check sends you back to step 2.
   - Confirm the surviving mechanism with runtime evidence before step 5.
4. Reduce to the smallest input or state that still fails, then record the
   cause. With a work item, run
   `maestro work note <id> "cause: <one line>; repro: <command>"`; in a
   bundle, `maestro handoff <bundle-id>` renders it into NOTES.md. Without a work item, the
   record goes in the reply. The record holds the root cause, its evidence
   chain, the reduced repro, and the smallest fix direction.
   - Diagnosis is read-only toward production code. Instrumentation and
     harnesses live outside production paths or are reverted before this
     record.
   - A diagnosis-only request stops here and replies with the cause.
5. Plan the fix. If it crosses a function boundary, sketch the new types and
   signatures before any body is written. Delegate the edit to a subagent under
   the `delegate` profile with a brief in maestro-work's dispatch envelope
   (Objective, Owned scope, Excluded scope, Mutation, Stop condition, Evidence
   required) and a specific scope.
   - Review its diff yourself before anything else, and write your own
     summary rather than passing its report through.
   - A trivial mechanical edit may go to one delegate. Independent edits go
     to parallel delegates on disjoint paths, one writer per worktree, and you
     review every diff before it lands.
   - A fix round or a retry goes to a fresh delegate with consolidated scope
     (the original brief, every later directive, the prior report and branch).
6. Verify on the same product through the verify-<repo> skill. The original
   repro now passes. "Inconclusive" or a run against a different product than
   the repro is not a pass. Flag it. Unit tests show branch behavior, not bug
   absence. Name the evidence layer the proof reached (maestro-verify's
   evidence layers). Before you report or act on a number you measured, run
   each side at least five times alternating, count errors, confirm the work
   ran, and name what limits it.
7. Stage the commits so the failing repro lands before the fix in git history.
   When the bug has a cheap local test path, land the focused regression check
   first and any sibling coverage after it. Skip a new test when it would be
   expensive, integration-heavy, or unclear
   (`~/maestro/WORKFLOW.md#testing-discipline`). This is **Sequence Work into
   Verifiable Units** (PRINCIPLES.md), the failing check first and the fix on
   top. Commits happen only under a commit grant (`maestro recipe show opening-a-pr`, The grant); without one, shape the same units in the working tree.
8. Deliver: `maestro recipe show opening-a-pr`.

## Reply

What was broken, the root cause, the fix, and how you verified it. Paste the
failing-then-passing repro output verbatim. Name the regression check, the
failure it produced before the fix, and the passing run after it. When
failing-before evidence could not be shown, say why and name the closest
regression check used instead. A diagnosis-only reply carries the cause, its
evidence chain, the reduced repro, the fix direction, and the work note that
records them.
