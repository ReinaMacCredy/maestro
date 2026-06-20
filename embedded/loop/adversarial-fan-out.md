# Adversarial fan-out

WHEN: a claim is contested or high-stakes and must not land wrong.

Spawn independent skeptics that try to *refute* each claim, rather than trust a
single self-verification. The skill (maestro-card `verify.md`) covers the
verify lane; this is the full HOW of running it as a fan-out.

## Rubric: the locked checks, nothing softer

The rubric is the card's locked acceptance checks plus its completion claims.
Do not invent gentler checks to make a claim pass.

## Dispatch: one fresh refuter per claim

    one verifier per claim/check, in parallel

Give each verifier only the claim, the check, and the repo. Instruct it to
refute, and to default to `refuted` when uncertain. Each returns exactly one
line:

    upheld | refuted: <check> - <observed evidence>

- Claude Code: a parallel verify stage, one agent per claim; for higher
  confidence run N refuters per claim and require a majority to uphold.
- Codex: one sub-agent per claim, dispatched together.

Never message a running verifier with new context. If a verifier needs
correcting, start a fresh one.

## Collect: record verdicts, block on refutation

    maestro event create --task-id <id> --claim "<upheld verdict line>"

For a reproducible refutation, block rather than verify:

    maestro task block <id> --reason "adversarial verifier refuted: <what>"

## Stop

All claims upheld -> `maestro task verify <id>`. Any reproducible refutation ->
block; do not run `task verify` over a refutation.
