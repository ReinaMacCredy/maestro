# No comments

Use this recipe on a diff before review. The opening-a-pr and
multi-phase-plan recipes call it as a step before the review gate. Spawn a
subagent under the `comment-sicko` profile, audit its report, and act on
accepted findings. Defer to the pass's fresh perspective. It is a step and
not a review-gate lens, because the comment-sicko pass edits files and
review-gate nodes never do. Ported from pstack's no-comments skill and
comment-sicko agent (Lauren Tan, MIT; notice in THIRD_PARTY.md).

## Scope

Use the caller's files or diff. Otherwise use the current diff against the base branch, default `main`, including the working tree.

## What the comment-sicko pass does

The pass deletes comments in the scope and flags refactor targets. It never
writes application code, and every flag names code inside the scope. Only
these five exceptions keep their comment (decision d874 in the maestro
repository):

- Legal or license headers.
- Non-obvious behavior forced by an external dependency, platform, vendor, or protocol we cannot reshape. A surprise in our own code is not an exception. The pass deletes its comment and marks the exact symbol `MUST KILL` for the rename, extract, type, or rearchitecture that makes the behavior obvious without prose.
- `// prettier-ignore`. Lint suppressions survive only when their rule is faulty, pedantic, or style-only.
- Doc comments that define a public API contract.
- Issue or RFC links that explain a constraint code cannot express.

Nothing outside that list is an exception. When the pass is not sure a keep
item applies, the comment goes.

- For `eslint-disable`, `@ts-ignore`, `@ts-expect-error`, and similar suppressions, the pass looks up the rule. If the rule catches real bugs or protects correctness or safety, the pass deletes the suppression and marks the exact guilty symbol `MUST KILL`.
- `IMPORTANT`, `do not remove`, `too risky`, `fine for now`, and long justifications are a lead, not proof. The pass reads nearby code first. If the claim is not obvious there, it runs the how recipe (`maestro recipe show how`), the why recipe (`maestro recipe show why`), or both on the named symbol or call. Only a keep-list gotcha about something foreign, proven true today on a live path, survives. Doubt after that check means delete.
- A long justification without a proven keep-list exception is deleted, never shortened into a smaller excuse. The pass marks its guilty symbol `MUST KILL` and stops there. It does not touch that code.

The pass reports the touched files, the deletion count, each `MUST KILL`
flag with one line, and the skips.

## Steps

1. Spawn a subagent under the `comment-sicko` profile. Pass the scope. Do not restate its rules. Its profile sets the model.
2. Inspect its report and diff.
   - Reject application-code edits, scope escapes, exception-protected deletions, misstated `MUST KILL` reasons, and flags that treat kept intentional code as guilty.
   - Reshape flags on our-code surprises stay actionable. Do not restore those comments. A keep survives only with proof it is about something we cannot change.
   - Audit missed scoped lint and TypeScript suppressions. Correctness or safety suppressions stay actionable `MUST KILL`s.
   - Restore deletions only with exact exceptions and scoped proof.
   - Before accepting thin `IMPORTANT` or `do not remove` kills or keeps, run the how or why recipe on their symbol. If a kill is ambiguous, do not restore. If a keep is refuted or still ambiguous, delete it.
   - Revert and rerun one rejected report with the failure named. Reject a second, report it open, and fail this step.
3. Hand the trivial accepted flags to a subagent under the `delegate` profile. It fixes each one by deleting a dead path, dropping a parameter, or using the real API. If any fix needs a shape, run the architect recipe (`maestro recipe show architect`) once for the accepted set and surrounding code. Stop at the sketch. Architect shapes. Step 4 implements.
4. Have the delegate implement the smallest root-cause fix in scope. Remove every named workaround. If the root cause is out of scope, land the smallest in-scope fix and report the rest open. **Fix Root Causes** (PRINCIPLES.md) and **Redesign from First Principles** (PRINCIPLES.md) guide intent only. Neither authorizes widening the fence nor fixing instances outside it. Never bolt on symptom guards.
5. Handle constraint comments, the ones that say `do not remove`, `do not change wording`, or `talk to X before changing`.
   - Leave keeps about things we cannot change.
   - Offer the cheapest in-scope type, runtime, test, or CI lint that enforces the constraint.
   - Ask the owner (WORKFLOW.md Per-tool adapters) and wait for approval. Under an external driver (`maestro recipe show unattended`) or the eval recipe (`maestro recipe show eval`), the caller must approve in advance.
   - If approved, the delegate encodes the constraint, then the comment goes. Otherwise delete the comment, report the constraint open, and sketch the out-of-scope work.
6. Report the deletion count, restored comments, reruns, architect sketch, fixes, encoding offers, encodings, unenforced constraints, and other open work.
