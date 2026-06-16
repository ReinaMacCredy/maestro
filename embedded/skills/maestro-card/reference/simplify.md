# Simplify

`/simplify` for a card's diff: once the implementation is green, improve the
quality of the changed code -- you are NOT hunting for bugs. Review the diff for
reuse, simplification, efficiency, and altitude issues, then fix what you find.
Do not look for correctness bugs -- that is what `/code-review` is for.

## When

Run on every card after implementation and before `task complete --proof`, on
the working-tree diff (or the card's commit range if you already committed) --
night and day. The review always runs; you only edit when it finds something.
Skip the review only for a purely non-code (docs/config-only) diff, and name
that reason in the completion summary -- "looked trivial" is not a skip.

On a test-first card this IS the red-green-REFACTOR step ([tdd.md](tdd.md) ->
[tdd/refactoring.md](tdd/refactoring.md)): do it once, here, not twice. Simplify
generalizes that refactor step to every card.

## Scope

Only this card's changes -- the diff you just produced. Do not re-review code an
earlier card already shipped; cross-card and repo-wide cleanup is maestro-audit's
job. Read the diff, not whole files, except to grep for a reuse target.

## Phase 1 -- Review (four angles)

Review the diff against the four angles below. The faithful `/simplify` move is
to fan them out as four independent review agents in one batch (one angle each)
so they run concurrently; for a small single-card diff you may walk them inline.
Each finding names `file`, `line`, a one-line summary, and the concrete cost
(what is duplicated, wasted, or harder to maintain).

### Reuse

Flag new code that re-implements something the codebase already has. Grep shared
and utility modules and files adjacent to the change, and name the existing
helper to call instead.

### Simplification

Flag unnecessary complexity the diff adds: redundant or derivable state,
copy-paste with slight variation, deep nesting, dead code left behind. Name the
simpler form that does the same job.

### Efficiency

Flag wasted work the diff introduces: redundant computation or repeated I/O,
independent operations run sequentially, blocking work added to startup or hot
paths. Also flag long-lived objects built from closures or captured environments
-- they keep the entire enclosing scope alive for the object's lifetime (a
memory leak when that scope holds large values); prefer a struct that copies only
the fields it needs. Name the cheaper alternative.

### Altitude

Check that each change is implemented at the right depth, not as a fragile
bandaid. Special cases layered on shared infrastructure are a sign the fix isn't
deep enough -- prefer generalizing the underlying mechanism over adding special
cases.

## Phase 2 -- Apply the fixes

Dedup findings that point at the same line or mechanism, then fix each remaining
one directly in the diff. Skip any finding whose fix would change intended
behavior, reach well outside this card's diff, or that you judge a false positive
-- note the skip rather than arguing with it. Re-run the card's tests; the
cleanup must stay green (a broken test is not a simplification). Finish with a
brief note of what was fixed and what was skipped, or confirm the diff was
already clean.

## Boundaries

- vs maestro-audit: simplify is diff-scoped and APPLIES the fix; audit is
  repo-wide and only PROPOSES. Do not file proposals from here -- edit the diff.
- vs /code-review: simplify is quality only. `/code-review` owns correctness,
  bugs, and security. If you spot a bug here, do NOT fix it under this pass --
  note it for a review/work card so the cleanup diff stays reviewable.

## After

The green suite after cleanup is the simplify pass's proof; it needs no separate
claim. Finish the card per [work.md](work.md). In the unattended loop the morning
report records the simplify outcome ([loop.md](loop.md)).

## Stop

- Do not hunt bugs or security issues; that is `/code-review`.
- Do not widen scope past this card's own diff.
- Do not run it twice on a test-first card -- the refactor step already is it.
- Do not skip the review except for a purely non-code diff, with the reason named.

## Hand-off

Next: diff tidied and green -> finish per [work.md](work.md) and prove per
[verify.md](verify.md).
