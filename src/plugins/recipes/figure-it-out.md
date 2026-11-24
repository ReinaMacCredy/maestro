# Figure it out

Use this recipe when no recipe in `maestro recipe list` fits the task. It is
also the route for a large or cross-cutting effort (a migration across many
call sites, an ambitious multi-part change) and for work the owner steps
away from and wants to trust later, even when a narrower recipe such as
feature fits. Before any code, the deliverable is the workflow itself. It is
a phase list that scales rigor to the task, runs a hypothesis loop, and
leaves a decision trail the owner can audit after stepping away. This recipe
designs one bespoke run; a standing program of many units runs under
`maestro recipe show fan-out` inside SLP. Ported from pstack's figure-it-out
skill (Lauren Tan, MIT; notice in THIRD_PARTY.md).

## Steps

1. Read `~/maestro/PRINCIPLES.md`, the index and every section you expect to
   apply.
2. Frame. Ground first, then commit. Do not start the run until you can
   state three things.
   - The definition of done as a falsifiable predicate, per
     **Prove It Works** (PRINCIPLES.md).
   - The scope, quantified. Rough units and effort, plus the blockers
     grounding found.
   - The rigor level, biased high. One-way doors and high blast radius get
     more; reversible, low-stakes steps get less. Rigor is gates and
     artifacts, not "try harder".

   When fog blocks the predicate itself (you cannot yet say what done looks
   like), stop here and wayfind with maestro-design. Wayfinding plans; it
   does not do. Present the framing and its tradeoffs before committing to a
   long run. Reversible work proceeds per **Never Block on the Human**
   (PRINCIPLES.md), but a multi-hour run earns one checkpoint. Open the trail
   with `maestro work note <id> "start: figure-it-out; predicate <...>; base <sha>"`.
3. Design the workflow. Decompose into atomic, independently landable units.
   Sequence the riskiest unknown first. Scaffold and verification come before
   features, per **Foundational Thinking** (PRINCIPLES.md).
   - Build the verification harness before the work, with the baseline
     captured from the pre-change state, so each check reads as old value
     against new value.
   - For a one-way-door design decision, run `maestro recipe show architect`,
     which runs arena. Skip it for mechanical work whose shape is already
     concrete. A second arena over a settled design is over-engineering, per
     **Laziness Protocol** (PRINCIPLES.md).
   - Decide what fans out. Parallelize only across seams, with each writer in
     its own worktree or branch, per
     **Separate Before Serializing Shared State** (PRINCIPLES.md), and follow
     `maestro recipe show fan-out`. Do not over-fan.
   - Write the phase list down. That list is what the owner reviews. When the
     work meets a Full trigger (`~/maestro/WORKFLOW.md#tiers`), the phase list
     becomes the bundle's SPEC `## Units` through
     `maestro recipe show multi-phase-plan`. On Light it lives in the plan
     list. End it with the deliver step, `maestro recipe show opening-a-pr`.

   Then execute the design. Add its phases to the plan list as concrete items
   between this recipe's step 4 and step 5 entries. Run each under the step 4
   loop and write the step 5 trail as each phase lands, never saved for the
   end.
4. Run the loop. Each unit is an experiment. State the hypothesis, make the
   smallest change, measure against the predicate on the real artifact, keep
   it if it advanced, and revert it if it did not. Verify each unit before
   starting the next, per **Sequence Work into Verifiable Units**
   (PRINCIPLES.md).
   - A `delegate` subagent writes each unit's code. You review its diff and
     verify.
   - Verify by inspecting the artifact, never a self-report. When something
     passes too easily, suspect the observation method before the system.
   - Pair delegated work with a judge. If a worker games the gate, reset and
     harden the contract. If the gate itself is wrong, fix the gate in its own
     change instead of routing around it.
   - A verdict is VERIFIED, NOT VERIFIED or INCONCLUSIVE. Inconclusive is not
     a pass. Do not hide a negative.
5. Keep the decision trail in the store, per
   `~/maestro/WORKFLOW.md#memory`. In a figure-it-out run, every decision
   that changed direction gets its own work note with the reason and the
   evidence. A failed pass is a `failed:` note, and the latest `checkpoint:`
   note names this recipe and the current phase. A durable, hard-to-reverse
   choice becomes a decision per
   `~/maestro/WORKFLOW.md#decisions-and-readiness`. The reviewer reads the
   trail with `maestro work show <id>`. The trail plus the diff is what lets the
   owner come back and trust the work.
6. Verify and hand back. Check the whole against the step 2 predicate on the
   real product, not only the harness, through the repository's verify-<repo>
   skill (`maestro recipe show verification-skill`). Encode any recurring
   correction as a gate, a lint rule, a check or a script, per
   **Encode Lessons in Structure** (PRINCIPLES.md). A correction to doctrine
   is filed with `maestro lesson file`.

## Reply

The phase list you designed, the rigor level and why, the work id whose
trail `maestro work show` shows, what is verified against the predicate, and
what is still open.
