# Auto-Merge, Review Acknowledgement, Verdict Override

## Contents

- [Auto-Merge Eligibility](#auto-merge-eligibility)
- [Review Acknowledgement](#review-acknowledgement)
- [Verdict Override](#verdict-override)

## Auto-Merge Eligibility

After a `PASS` verdict and CI witness, an agent can trigger automated merge
via `maestro merge auto`. The command runs 8 deterministic eligibility
predicates. If all pass, it shells `gh pr merge --auto` and exits 0. If any
fail, it prints the failing codes and exits 1.

```bash
maestro merge auto --pr <number> --task <id> [--base <ref>] [--repo <owner/name>] [--json]
```

The 8 predicates (canonical order):

1. **`verdict-not-pass`** — verdict must be `PASS`.
2. **`auto-merge-class-disabled`** — `autoMergeAllowed.<riskClass>` must be `true` in `autopilot.yaml`. Defaults to `false` for all classes; must be explicitly opted in.
3. **`evidence-witness-too-weak`** — all gating evidence kinds (`command`, `verifier`, `ai-review`, `threat-model`, `plan-check`) must be at `witnessed-by-ci` or stronger.
4. **`forbidden-paths-touched`** — diff must not intersect `contract.scope.filesForbidden`.
5. **`sensitive-paths-untouched-without-waiver`** — if diff touches sensitive paths, a `verdict-override` evidence row must exist.
6. **`rollback-not-witnessed`** — a `rollback-exercised` evidence row at `witnessed-by-ci` must exist. **Normal failure until L7.5** ships the CI evidence producer for rollback exercises.
7. **`review-ack-missing`** — if verdict is `HUMAN` at `>=medium` risk, a `review-ack` evidence row must exist.
8. **`spec-score-below-threshold`** — if a Spec is linked, its quality score must be 1.0.

See `docs/auto-merge-eligibility.md` for the full reference, fix
instructions, and troubleshooting table.

## Review Acknowledgement

When a verdict is `HUMAN` at `medium` or higher risk, a human reviewer must
acknowledge the review before auto-merge can proceed. Record the
acknowledgement:

```bash
maestro review ack \
  --task <id> \
  --verdict <verdict-id> \
  --criterion "All tests pass" \
  --criterion "No critical findings"
```

This records a `review-ack` evidence row at `agent-claimed-locally`. The
`--criterion` flag is repeatable. After recording, re-run
`maestro merge auto` to re-evaluate eligibility.

## Verdict Override

An override is an append-only audit record. It does not change the PR check
conclusion or rewrite the verdict. It is a waiver that enables `merge auto`
to proceed when a `sensitive-paths-untouched-without-waiver` finding is
blocking it.

```bash
maestro verdict override \
  --task <id> \
  --pr <number> \
  --reason "<free text, required>"
```

Authorization: the invoking user must be listed in
`owners.yaml.sensitive_waiver`. The file is loaded from the base branch
(Rule 12 — not the PR head, so self-promotion is rejected).

The override records a `verdict-override` evidence row at
`agent-claimed-and-not-reproducible`. The original verdict is unchanged. The
PR check conclusion is unchanged. The override is visible in
`maestro verdict show --task <id>` and in the CI PR check summary.

See `docs/override-flow.md` for the full authorization rules, audit trail
description, and no-silent-pass guarantees.
