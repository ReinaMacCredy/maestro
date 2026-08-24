---
name: maestro-work
description: Drive one accepted implementation unit test-first - smallest falsifiable behavior, minimum edits, evidence that names the real falsifier.
---
<!-- maestro-skill-version: dev -->

# maestro-work

Use for one accepted implementation unit. Keep the change inside the work
item's acceptance and authority. If the scope is unclear or must expand, stop
and return to `maestro-design`.

## Loop

1. **Perceive** - `maestro work show <id>`, `maestro ready`, relevant source,
   tests, and repository instructions. Name the task-owned dirty paths before
   editing.
2. **Choose** - the smallest behavior falsifiable at the accepted seam. Write
   the agreed failing test before production code. New child work gets
   `--acceptance "<observable result>"`.
3. **Act** - `maestro work start <id>`, then the minimum source and test edits
   for that behavior. No speculative abstractions or dependencies.
4. **Observe** - run the focused test, then type/lint/build checks. Review the
   diff against acceptance; confirm the test could expose the defect.
5. **Learn** - `maestro work note <id> "..."` only for a reusable correction or
   failed approach.
6. **Continue** - `maestro work done <id>` with `--claim`/`--proof` naming the
   real falsifier (the check that would have failed if the claim were wrong).
   In a bundle, overwrite NOTES.md (current state, next action, base commit)
   before releasing the work item.

## Test-first root laws

1. Red tests only transcribe DECIDED behavior at an accepted seam. Having to
   guess the contract means you are still in the design lane - spike, decide,
   then test.
2. Test at the outermost stable seam (here: the CLI) so internals stay free to
   change.
3. Serve the behavior, never the test. A wrong test is shifted openly - make
   the deliberate change and record it with `maestro decision draft` - code is
   never contorted to please a test.
4. A test must be able to kill the bug: no tautologies, no asserting that a
   mock called a mock.
5. No silent test edits. Every shift or deletion of a test records its reason.

Concrete smells and fixes: [references/tdd-antipatterns.md](references/tdd-antipatterns.md).

## Coordination

Isolated lanes and worktrees: [references/worktree.md](references/worktree.md).
Contested files or overlapping sessions:
[references/conflict-handoff.md](references/conflict-handoff.md).
