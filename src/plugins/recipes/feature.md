# Feature

Use this recipe for new or changed behavior, built from a named data shape.
Multi-unit work in a Full bundle opens its SPEC `## Units` with
`maestro recipe show multi-phase-plan`. Ported from pstack's feature playbook
(Lauren Tan, MIT; notice in THIRD_PARTY.md).

You own the design. Plan, review, verify. Delegate implementation and stay in
the lead.

## Steps

1. Run `maestro recipe show how` over the affected subsystem.
2. Run `maestro recipe show architect` for parallel design exploration.
3. Write the throughput checkpoint as four plan-list items. A dimension that
   does not apply (single file, no fan-out) keeps its item with
   `n/a: <reason>` rather than being dropped.
   - Blocking first steps. Gates run before fan-out.
   - Independent workstreams. Disjoint files, services, or layers
     parallelize. Shared writes serialize.
   - Shared mutable state. Default to splitting the target, per **Separate
     Before Serializing Shared State** (PRINCIPLES.md). Serialize only for real
     invariants.
   - Smallest safe decomposition. If one worker is best, name why.
4. Delegate the code to a subagent under the `delegate` profile with a brief in
   maestro-work's dispatch envelope (Objective, Owned scope, Excluded scope,
   Mutation, Stop condition, Evidence required). The scope names file paths,
   the success criteria, and the data shape with its organizing structure per
   **Model the Domain** (PRINCIPLES.md), chosen before the delegate writes
   logic. Prefer a state machine over scattered booleans, a table or registry
   over branching, and a typed model over repeated shape assumptions.
   - Review its diff yourself before anything else, and write your own
     summary rather than passing its report through.
   - A trivial mechanical edit may go to one delegate. Independent edits fan
     out per `maestro recipe show fan-out`.
   - When the implementation admits several valid shapes (error handling,
     abstraction layer, test structure), delegate through
     `maestro recipe show arena` instead, so the runners bring out the
     alternatives and the cross-judge guards the pick.
   - This delegation is mandatory. There is no skip-with-reason escape, and
     the **Laziness Protocol** (PRINCIPLES.md) does not override it. The gain
     is review separation, not lines saved. A subagent forbidden to spawn
     satisfies this by owning the diff directly with the same review
     separation. No "standing by" reply that waits on a nested agent.
   - A fix round or a retry goes to a fresh delegate with consolidated scope
     (the original brief, every later directive, the prior report and branch).
   - The delegate keeps only the comments `maestro recipe show no-comments`
     keeps. Edits stay surgical. Upstream-derived files are re-grounded against
     their source. A shared-primitive improvement is ported to every consumer,
     and each consumer is verified. Under a commit grant, commit liberally.
5. Verify on the running product through the repository's verify-<repo> skill
   (`maestro recipe show verification-skill`), and name the evidence layer the
   proof reached (maestro-verify's evidence layers). "Inconclusive" or a run
   against a different product is not a pass. Flag it.
6. Build, verify, and land each small unit before the next, per **Sequence
   Work into Verifiable Units** (PRINCIPLES.md), as small ordered commits
   with follow-ups stacked. Commits happen only under a commit grant (`maestro recipe show opening-a-pr`, The grant); without one, shape the same units in the working tree.
7. If the design is contested, run the review gate before delivery: the
   holder asks one reviewer agent to review the diff.
8. Deliver: `maestro recipe show opening-a-pr`.

Code-coupled work (one feature, one migration) goes to a single delegate with
the checkpoint inline. That delegate fans out internally after the blocking
phase. Parent-level fan-out is for slices that produce independent artifacts
(audits, cross-subsystem investigations, competing experiments). Rewrite the
checkpoint at phase boundaries. Spawn a fresh delegate rather than chaining
interrupts.

## Reply

What you built, what you chose and why, the throughput checkpoint, and open
decisions. Use tables for design alternatives.
