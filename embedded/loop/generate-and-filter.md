# Generate and filter

WHEN: a taste-heavy fork (naming, UX wording, API shape, report structure).

Generate several independent options from different angles, then filter to one
with a fresh judge. The skill (maestro-design Taste Forks) decides *when* a fork
is taste-heavy; this is the full HOW of running the pass.

## Rubric first

Write a 3-5 point rubric into `notes.md` *before* generating anything, so the
judge scores against fixed criteria, not after-the-fact rationalization.

## Generate: 3-5 fresh options, divergent angles

    one generator per angle, in parallel

Ask each generator for ONE concrete option, each from a different angle, e.g.
minimal, user-first, consistency-first. Fresh context per generator so they do
not converge.

- Claude Code: a parallel generate stage, then a single judge stage.
- Codex: parallel generator sub-agents, then one judge.

## Filter: judge against the rubric

A fresh judge scores every option against the rubric and removes duplicates. If
the top scores cluster, run pairwise matches until one option survives.

## Lock: only the survivor is durable

    maestro decision new ... ; maestro decision lock <id> --decision "<survivor>" \
      --rejected "<why each loser lost>"

The generators are scaffolding, not outputs: nothing they produced persists
except the locked decision and the recorded reasons the rejected options lost.

## Stop

One option locked as a decision with its rejected alternatives recorded. Do not
keep multiple "maybe" options alive past the lock.
