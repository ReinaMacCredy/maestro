# Arena

Use this recipe when one attempt at a non-trivial artifact would lock in the
wrong shape. N candidates build the same brief in isolation, a judge scores
them against a rubric, you pick a base criterion by criterion, and you graft
the best parts of the losers into it. Arena is not maestro-council. Council
settles a hard-to-reverse decision with sealed analysis-only seats; arena
picks between built candidates. Ported from pstack's arena skill (Lauren
Tan, MIT; notice in THIRD_PARTY.md).

## Steps

1. Frame. Every candidate gets the same brief, so the brief is the contract.
   - State the artifact each candidate produces.
   - Write the rubric before fan-out. It holds 3 to 6 concrete, gradeable
     criteria for success on this task. It is the picker's tool in step 4.
     Candidates see only the task, never the rubric.
   - Pick N runners under the `arena-runner` profile. Spawn more runners when
     the arena covers several design directions. The same model N times is
     right when the work is generation-bound rather than judgment-sensitive.
   - Give each candidate its own output location per
     **Separate Before Serializing Shared State** (PRINCIPLES.md). For code
     that is a worktree each (`maestro recipe show worktree`). For any other
     artifact it is a directory per candidate under `maestro topic path <slug>`.
2. Fan out. Spawn all N runners at once in the background. Each gets the
   task, the path to the shared grounding, its own output path, and the
   instruction to produce the artifact plus a short rationale naming the
   alternatives it considered and what it rejected. No candidate sees
   another. If a candidate produces nothing, continue with N-1 and note the
   dropout.
3. Cross-judge. Once every candidate is complete, spawn one read-only
   subagent under the `arena-judge` profile, from a different model family
   than the runners where possible. Never spawn it while candidates are still
   writing. It sees the rubric and the candidates by path label, scores each
   criterion, and recommends a base with its rationale. It runs while you do
   step 4.
4. Pick a base. Read every candidate end to end before picking. Score each
   against the rubric criterion by criterion, not on holistic feel, and
   compare with the judge. Agreement on the base confirms the pick.
   Disagreement means one of you is biased or the rubric was ambiguous; read
   both rationales before deciding. Pick the base a future maintainer can
   extend most easily without breaking invariants. When two feel tied, prefer
   the cleaner boundary or the smaller API per **Laziness Protocol**
   (PRINCIPLES.md).
5. Graft. Walk each losing candidate once more for what is worth porting,
   usually one or two things per candidate, not most of it. Fold each graft
   in by hand per **Redesign from First Principles** (PRINCIPLES.md), so the
   result stays coherent under one mental model. Do not paste mechanically.
   When the artifact is code, a `delegate` subagent applies your graft list
   in the base's worktree and you review its diff.
   - When the candidates converge on one shape, that is strong agreement.
     Note it and ship the consensus shape. No graft is needed.
   - When they diverge wildly, step 1 was under-specified. Reframe and
     re-run. Never average the divergence.
6. Verify the synthesized artifact like any other output per
   **Prove It Works** (PRINCIPLES.md). If verification finds a problem the
   arena missed, either the frame was wrong (reframe and re-run) or a
   candidate caught it and you missed the graft (return to step 5). Do not
   paper over it.

If this flow is ever built as a maestro graph, the judge is one scoring node
holding the rubric, not a majority-vote judge panel.

## Reply

The synthesized artifact and a short synthesis note. The note names the base
and why, the judge's verdict, each graft with its source candidate, what was
rejected and why, any convergence or dropouts, and the verification result.
