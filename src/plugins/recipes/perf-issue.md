# Perf issue

Use this recipe when a measured slowness needs a trace and a fix against a
baseline. You own the measurement story. You plan, review the diff and verify
the numbers, and a writing subagent on the strongest model writes the code. Tie
every fix to a measurement. Reading source is not measuring. For sustained
improvement of one metric against a target, use `maestro recipe show hillclimb`
instead. Ported from pstack's perf-issue playbook (Lauren Tan, MIT; notice in
THIRD_PARTY.md).

## Steps

1. Capture a baseline trace on the path the user waits on, through the
   repository's verify-<repo> skill (`maestro recipe show verification-skill`).
   Save the artifact in the topic folder (`maestro topic path <slug>`).
   Before any number is reported or acted on, the baseline included, run each
   side at least five times alternating under production settings, count
   errors, confirm the work ran, and name the limiter.
2. Ground the hypotheses by tracing the slow path in the code, citing
   file:line for each step. Do not claim a perf
   ceiling you have not run. Try the performance mantras in order, cheapest
   first, and stop at the first one that meets the target:
   1. Don't do it. Stop work whose result nothing uses rather than cheapening it.
   2. Do it, but don't do it again.
   3. Do it less.
   4. Do it later.
   5. Do it when they're not looking.
   6. Do it concurrently.
   7. Do it cheaper.
3. Plan the fix from the trace. If it crosses a function boundary, sketch the
   new types and signatures first. Hand the implementation to a writing subagent
   on the strongest model, with a brief in maestro-work's dispatch
   envelope that names the files, the mechanism the trace names and the target
   number. Review its diff yourself. Capture a post-fix trace the same way as
   the baseline. Verify each attempt before you try the next, per
   **Sequence Work into Verifiable Units** (PRINCIPLES.md).
4. Parse and compare the baseline and post-fix artifacts. Load each into
   sqlite and diff them. A number taken on a different path than the one the
   user waits on is not a pass, and neither is an inconclusive one
   (`~/maestro/WORKFLOW.md#recovery-and-verification`). Flag either.
5. Cite the measurement in the delivered change as one primary number with
   its unit, before and after, and link the artifact for the rest.
6. Deliver: `maestro recipe show opening-a-pr`.

## Reply

The baseline number, the post-fix number, the delta, and the artifact paths.
