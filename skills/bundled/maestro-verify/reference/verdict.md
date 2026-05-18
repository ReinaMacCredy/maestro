# Verdict, Cost-Budget, AI Reviewer, Threat Model

## Contents

- [Verdict Semantics](#verdict-semantics)
- [Reason-Field Diagnostics](#reason-field-diagnostics)
- [Cost-Budget Monitoring](#cost-budget-monitoring)
- [AI Reviewer Protocol (Rule 1)](#ai-reviewer-protocol-rule-1)
- [Threat-Model Production](#threat-model-production)

## Verdict Semantics

`maestro verdict request` runs the full decision tree: Trust Verifier +
ProofMap + autopilot policy + cost-budget check. It persists the Verdict to
disk and exits with a code you branch on.

```bash
maestro verdict request --task <id>
maestro verdict request --task <id> --json
```

Exit codes:

| Code | Decision | Action |
|---|---|---|
| `0` | `PASS` | Claim the task done. All criteria met at required witness level. |
| `1` | `FAIL` | Fix the cited findings, then loop back to `maestro task verify`. |
| `2` | `HUMAN` | Stop and surface the verdict to the user — this risk class requires human review before the task can complete. The lifecycle verbs (`task claim`, `task block`) already drop a handoff envelope on disk; nothing extra to invoke here. |
| `3` | `BLOCK` | Stop. A blocker condition is active — cost budget exhausted, critical risk class with no human signoff, or a pending policy loosening still soaking. Surface the reason to the user; do not retry without guidance. |

Inspect an existing Verdict without re-running:

```bash
maestro verdict show --task <id>
maestro verdict show --task <id> --at-version <verdictId>
```

Example text output:

```
Decision:   FAIL
Risk:       medium (proposed: low)
ComputedAt: 2026-05-04T10:22:11Z
Task:       tsk-aaa123
ID:         vrd-bbb456
Reasons:
  [coverage] missing-criterion: ac-3 has no Evidence rows
Evidence consulted: 4
Trust verifier: 1 findings (1 errors, 0 warns, 0 infos)
```

## Reason-Field Diagnostics

The `reasons[]` array surfaces three diagnostic codes you must read on every
non-PASS verdict (full reference: `docs/edge-cases.md`):

- `proof-map-incomplete` — listed alongside any FAIL or HUMAN reason
  whenever acceptance criteria lack covering evidence. Record one
  `evidence record --kind command --criterion <ac-id>` row per uncovered
  criterion before re-requesting.
- `cost-budget-exhausted` — the BLOCK reason names the exhausted limit
  (`costBudget.maxRetries`, `maxWallClockSeconds`, or `maxTokens`) and the
  machine code is mirrored into `findingChecks[0]`. Use that to branch in
  scripts and to know which knob to amend.
- `auto-merge-not-allowed` — the policy gate; not a defect. Combine with
  any `proof-map-incomplete` to know whether to handoff or to first close
  the coverage gap.

## Cost-Budget Monitoring

When retries are accumulating, read the budget state directly from the most
recent verdict envelope (`maestro verdict show --task <id> --json`):

- `costBudgetExhausted: true | false`
- `costBudgetReason: "max-retries" | "max-wall-clock-seconds" | "max-tokens"` (present only when exhausted)

The contract's `costBudget` (visible via `maestro contract show --task <id>`) carries the configured `maxRetries`, `maxWallClockSeconds`, and `maxTokens` caps; the verdict envelope reports which one tripped.

Once any limit is exceeded, the next `verdict request` returns `BLOCK`. At
that point, stop and surface the reason to the user — `maestro contract amend`
can raise the cap if continued execution is warranted.

## AI Reviewer Protocol (Rule 1)

When an agent runtime has reviewer integration, record findings via
`maestro evidence record --kind ai-review`:

```bash
maestro evidence record --task <id> --kind ai-review \
  --reviewer <bug|security|architecture> \
  --findings '<inline-json-or-path>' \
  --confidence <0-1>
```

**Rule 1 (LLM veto-only)** governs how ai-review findings affect risk class:

- Any `error`-severity finding raises `effectiveRiskClass` by one notch.
- A `security`-reviewer `error` always lifts to `critical`.
- A clean ai-review (zero error findings) **never lowers** the deterministic
  baseline. The Risk Engine derives risk class from diff signals; the LLM
  can only raise it.

Example invocations:

```bash
# Security review, one error finding
maestro evidence record --task tsk-aaa --kind ai-review \
  --reviewer security \
  --findings '[{"severity":"error","message":"SQL injection risk in buildQuery"}]' \
  --confidence 0.9

# Architecture review, clean pass
maestro evidence record --task tsk-aaa --kind ai-review \
  --reviewer architecture \
  --findings '[]' \
  --confidence 0.8

# Bug review from a file
maestro evidence record --task tsk-aaa --kind ai-review \
  --reviewer bug \
  --findings ./ai-review-findings.json \
  --confidence 0.7
```

Confidence semantics and per-reviewer finding schemas are documented in
`docs/ai-reviewer-protocol.md` (ships in L4.DOCS).

## Threat-Model Production

When the diff intersects security-relevant paths (anything matched by a
`critical`-class signal in `policies/risk.yaml`), produce a threat-model
document and record it before requesting a Verdict:

```bash
maestro evidence record --task <id> --kind threat-model \
  --threat-model-file ./threat-model.json
```

The file is JSON or YAML with this schema:

```yaml
assets:
  - session tokens
  - password hashes
threatCategories:
  - spoofing
  - tampering
  - info-disclosure
mitigations:
  - threat: session-fixation
    mitigation: rotate token on login
  - threat: weak-hashing
    mitigation: argon2id with workfactor 3
residualRisk: low          # low | medium | high
# optional:
criterion_id: ac-2
```

**Rule 1 applies here too.** Schema-valid presence clears the
`threat-model-required` predicate, but it is not sufficient on its own. Other
gates still apply: the autopilot required-witness threshold and the L6
path-touched gate (reviewed when the PR is opened). Substantive correctness
— whether the threat model meaningfully covers the change — is reviewed by a
human.

See `docs/threat-model-format.md` for the full schema, examples, and Risk
Engine semantics.
