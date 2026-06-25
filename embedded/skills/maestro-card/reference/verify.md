# Verify

Use this when a work card needs verification, proof failed, or a feature gate
asks for QA evidence.

## Do

1. Start with `maestro status` or `maestro task next`.
2. Inspect the work:
   `maestro task show <id>` and the locked acceptance checks, or
   `maestro feature show <id>` for feature QA blockers.
3. Run the smallest checks that can falsify the claim from the repo root.
   Record exact command/manual probe and outcome.
4. Complete with matching claim and proof when the card is not yet completed:

   ```sh
   maestro task complete <id> \
     --summary "<what changed>" \
     --claim "<claim>" \
     --proof "<observed evidence>"
   ```

5. If verification fails, run `maestro task proof <id>`, repair the claim or
   evidence, then `maestro task verify <id>`.

## Repair

- Missing claim: complete or update the card with the exact observable claim
  that was proven.
- Missing proof: add `--proof` through `task complete`, or record manual proof:
  `maestro event create --task-id <id> --claim "<claim>"`.
- Stale proof: rerun the falsifying checks and verify again.
- Failed command: fix the work or acceptance command; do not mark verified by
  narration.

## Adversarial Fan-out

Use when a card failed verify twice, risk is high, or many cards support a
feature close. Full orchestration HOW (refuter dispatch, majority, verdict
collection): `maestro loop show adversarial-fan-out`.

1. Rubric is the locked acceptance checks plus completion claims. Do not
   invent softer checks.
2. Spawn one fresh verifier per claim/check. Give only the claim, the check, and
   the repo. Ask it to refute and default to refuted if uncertain.
3. Each verifier returns one line:
   `upheld|refuted: <check> - <observed evidence>`.
4. Record upheld verdicts as evidence:
   `maestro event create --task-id <id> --claim "<verdict line>"`.
5. For reproducible refutations, block the card:
   `maestro task block <id> --reason "adversarial verifier refuted: <what>"`.
   Do not run `task verify` over a refutation.
6. All upheld -> `maestro task verify <id>`.

Never message a running verifier with new context. Start a fresh verifier.

## Feature QA

- Accept or prepare blocker says `handoff` -> run
  `maestro feature finalize <id>`, then retry the blocked command.
- Accept blocker says `qa-baseline` -> write `.maestro/cards/<id>/qa.md` with
  [qa-baseline.md](qa-baseline.md), then rerun accept.
- Close blocker says `qa-slice` -> append counting slices to the fenced
  `slices:` block of `.maestro/cards/<id>/qa.md` with
  [qa-slice.md](qa-slice.md), then rerun close.
- Do not report a feature closed until
  `maestro feature close <id> --outcome "<one line>"` passes.

## Stop

- Do not replace evidence with confidence.
- Do not weaken acceptance checks after implementation to make proof pass.
- Do not continue silently when verification cannot run; state the blocker and
  remaining risk.

## Hand-off

Next: verified card with live siblings -> [work.md](work.md); all children
verified -> [qa-slice.md](qa-slice.md), then [feature.md](feature.md).
