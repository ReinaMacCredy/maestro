# Refactoring

Use this recipe for a behavior-preserving change to structure or shape
(rename, extract, inline, dedupe, move). It differs from feature, which adds
behavior, and bug-fix, which corrects it. Ported from pstack's refactoring
playbook (Lauren Tan, MIT; notice in THIRD_PARTY.md).

You own the contract. The structure changes. The behavior does not.

If the cleanup reveals a missing feature or a real bug, split it out and ship
the structural change first against the pinned contract. A redesign is
allowed, but name it and route it to `maestro recipe show feature`. Large or
cross-cutting structural work belongs to `maestro recipe show figure-it-out`.
This recipe is for the focused-to-medium change.

## Steps

1. Pin the behavior contract first. Run `maestro recipe show how` over the
   affected subsystem to learn the contract.
   - Write each behavior the change must preserve as a guard, "the system
     SHALL CONTINUE TO <existing behavior>", so preserved behavior is a
     checkable claim, not an assumption.
   - Back each guard with a runnable command and its observed output.
   - Capture current behavior in a characterization test, snapshot, or
     equivalence harness before any structure moves. An existing suite that
     already pins the behavior counts
     (`~/maestro/WORKFLOW.md#testing-discipline`). If the area has no
     coverage, write the pin before touching structure. Type check and lint
     are not a pin.
   - In a bundle, each guard is a VERIFY.md scenario. Without a bundle, the
     guards go in the work item's acceptance.
2. Name the structure the code is missing, per **Model the Domain**
   (PRINCIPLES.md). Boring code stays when the shape is already clear and
   local. The reshape must delete branches or invalid states, not add
   indirection.
3. Name the target shape. State what the module layout, types, and call graph
   should be if built today, per **Foundational Thinking** and **Redesign from
   First Principles** (PRINCIPLES.md). If the target crosses a function
   boundary, run `maestro recipe show architect` for parallel design
   exploration of the shape before the move.
4. Subtract before you add. Delete dead code, collapse one-caller wrappers,
   drop redundant validators, and remove orphan references before introducing
   the new shape, per **Subtract Before You Add** (PRINCIPLES.md). The smallest
   change that reaches the target shape ships, per the **Laziness Protocol**
   (PRINCIPLES.md). A speculative cleanup that "might help" gets reverted.
5. Move in small behavior-preserving steps, each keeping the pin green.
   - For API reshapes, migrate every caller and delete the old API in the same
     wave, per **Migrate Callers Then Delete Legacy APIs** (PRINCIPLES.md). No
     compatibility shims, no parallel old-and-new paths.
   - Spot-check every rename against the actual files. Renames silently miss
     usages in strings, prose, and back-references.
   - Delegate the mechanical edits to a subagent under the `delegate` profile
     with a brief in maestro-work's dispatch envelope (Objective, Owned scope,
     Excluded scope, Mutation, Stop condition, Evidence required) and a
     specific scope (file paths, the names being moved, the guards to hold).
   - Review its diff yourself before anything else, and write your own
     summary rather than passing its report through.
   - A trivial mechanical edit may go to one delegate. Independent edits fan
     out per `maestro recipe show fan-out`.
   - A fix round or a retry goes to a fresh delegate with consolidated scope
     (the original brief, every later directive, the prior report and branch).
6. Prove behavior is unchanged on the real artifact, not "it compiles", per
   **Prove It Works** (PRINCIPLES.md). Rerun every guard command and compare
   its output with the step 1 observation. For larger reshapes, run an
   equivalence check. That is a script that diffs old-vs-new outputs, a
   recorded baseline replayed against the new code, or a smoke run on the
   running product through the repository's verify-<repo> skill
   (`maestro recipe show verification-skill`).
7. Confirm the change is worth keeping. The success measure is reduced reader
   load, per **Minimize Reader Load** (PRINCIPLES.md). If the diff does not
   lower reader load somewhere, revert it.
8. Rebase into small ordered commits. A subtraction commit, then the reshape,
   then any follow-on cleanup. Shape them per **Sequence Work into Verifiable
   Units** (PRINCIPLES.md), so each behavior-preserving slice stays green
   before the next. Commits happen only under a commit grant (`maestro recipe show opening-a-pr`, The grant); without one, shape the same units in the working tree.
9. Deliver: `maestro recipe show opening-a-pr`.

## Reply

The structure that changed, the guards and pin you held it against, the
equivalence proof, the reader-load delta, and what shipped and what got
reverted. No new behavior.
