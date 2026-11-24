# Hillclimb

Use this recipe for sustained, scientific improvement of one measurable thing
against a target. It loops hypotheses with a before and after measurement,
keeps a decision log, and lands one commit per accepted win. A one-off fix is
`maestro recipe show bug-fix` or `maestro recipe show perf-issue`. This is the
loop. You own the metric and the experiment's integrity. You supervise and
review, and writing subagents on the strongest model make the attempts. Ported
from pstack's hillclimb playbook (Lauren Tan, MIT; notice in THIRD_PARTY.md).

The core discipline is one change, one measurement, keep or revert. Never stack
untested changes, and never claim a win from code inspection, per
**Prove It Works** (PRINCIPLES.md).

## Steps

1. Ground the workload and the architecture before you choose the metric.
   Trace how the target runs, citing file:line for each step. Name the realistic workload
   dimensions that can move the result (data size, history, state,
   concurrency) and pick a case that reproduces the user's complaint. If no
   case reproduces it, fix the repro instead of climbing. Then fix one metric,
   the direction that counts as better, and a checkable stop predicate that
   pairs a target with a floor on attempts, so a lucky early win cannot end
   the run. "At least 50% better than baseline and at least 10 iterations" has
   this shape. Use the owner's numbers when given, and agree them with the
   owner otherwise. Every accepted win becomes its own commit, so confirm the
   delivery grant here (`maestro recipe show opening-a-pr`).
2. Build the measurement harness, prove its sensitivity, then freeze it, per
   **Build the Lever** (PRINCIPLES.md). A writing subagent on the strongest model
   writes the harness. Run contrasting realistic workloads and confirm that the
   target case reproduces the symptom while easier cases separate as expected.
   If the harness cannot tell them apart, revise the workload or the metric.
   Before you freeze the harness and before any number is reported, run each
   side at least five times alternating, count errors, confirm the work ran,
   and name the limiter. Make the harness print its error count
   and a count of the work done. Once frozen, one repeatable command emits the
   metric, sampled enough to clear the noise. It reports the median of N runs,
   never a single run. Record the baseline metric and a green run of the
   regression gate (the tests that must keep passing) before any change.
3. Open the decision log on the held work item. Every row is a work note with
   the fixed prefix `hillclimb:`. The first row holds the predicate, the
   harness command, the baseline and the gate run:
   `maestro work note <id> "hillclimb: start | predicate <target and floor> | harness <command> | baseline <value> | gate <command> green"`.
   Each attempt adds one row with the same fields in the same order:
   `maestro work note <id> "hillclimb: <n> | <hypothesis> | <change> | before <value> | after <value> | delta <value> | gate <pass or fail> | kept <sha> or reverted | <note>"`.
   Read the log before each attempt with `maestro work show <id> --notes all`.
   The log lives in the store, never in the tree.
4. Ground each hypothesis in the architecture model from step 1, so it names a
   specific mechanism ("defer X off the boot path because it blocks first
   paint"), not "try memoizing something". For a perf metric, order hypotheses
   by the performance mantras in step 2 of `maestro recipe show perf-issue`.
   Borrow only their order, not that step's stop rule.
5. Loop, one hypothesis per iteration:
   - Hand the change to a writing subagent on the strongest model with a tight
     scope. Supervise and review its diff rather than typing it, per
     **Guard the Context Window** (PRINCIPLES.md). When several independent
     hypotheses are live, fan them out to parallel subagents, each in its own
     worktree (`maestro recipe show worktree`), per
     **Separate Before Serializing Shared State** (PRINCIPLES.md).
   - Measure before and after with the frozen harness, and run the regression
     gate.
   - Accept only when the metric moves past the noise and the gate stays green.
     Otherwise revert the change in full. A tweak that "might help" is not
     kept.
   - Commit each accepted win by itself, staging only the files it changed
     (`~/maestro/WORKFLOW.md#concurrency-and-git`), under a commit grant
     (`maestro recipe show opening-a-pr`). Without one, keep each win as a
     work note with its diff and land none. Log the row either way,
     kept or reverted.
   - End each iteration in a check before the next begins, per
     **Sequence Work into Verifiable Units** (PRINCIPLES.md). An unattended run
     borrows only the external driver from `maestro recipe show unattended`,
     not its stop rule.
6. Push past the first plateau. On a stall or several rejects in a row, pivot
   the category, combine near-misses, re-read the source, or try something
   more radical before you conclude the hill is climbed. Correctness and
   simplicity outrank the number. Revert a win that breaks behavior, and keep a
   simplification that holds the number, per **Laziness Protocol**
   (PRINCIPLES.md).
7. Stop when the predicate is met, or when the remaining ideas are marginal and
   not worth their cost. Never relax the predicate to meet it, and never quit
   while cheap untried hypotheses remain. If you are stuck, report it to the
   owner instead of spinning.
8. Deliver: `maestro recipe show opening-a-pr`.
   - Stack the accepted commits in the order they landed.

## Reply

The metric and the target. The baseline and the final value with the percent
delta. The iterations run, with how many were kept and how many reverted. Each
accepted fix on one line. The work item whose `hillclimb:` notes hold the log.
The best idea you would try next if pushed further.
