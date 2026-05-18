# Plan-Check and ProofMap

The two coverage gates that bracket implementation.

## Plan-Check Protocol

At the start of a non-trivial task, write a plan file and run `maestro plan
check` before writing any code. The verb records a `plan-check` Evidence row
and surfaces three checks.

**Plan file format** (JSON or YAML):

```yaml
intendedFiles:
  - src/features/foo/**
  - tests/unit/features/foo/**
proofSet:
  - criterionId: ac-1
    evidenceKinds: [command]
  - criterionId: ac-2
    evidenceKinds: [command, ai-review]
riskClass: medium
notes: "Optional context"
```

```bash
maestro plan check --task <id> --plan-file ./plan.yaml
maestro plan check --task <id> --plan-file ./plan.yaml --json
```

The three checks:

- **`scope-widens`** — plan touches files not in `contract.scope.filesExpected`. Resolve by narrowing `intendedFiles` or amending the contract before coding.
- **`missing-proof`** — an acceptance criterion from the Spec has no entry in `proofSet`. Every criterion needs a planned proof strategy.
- **`risk-class-too-low`** — `intendedFiles` triggers a higher risk class than `riskClass` declares (Rule 1 plan-time gate). Raise `riskClass` to match.

The verb always exits 0. Agents read the findings and address them before
implementing. A clean plan-check does not mean the task will pass — it means
the plan is internally consistent.

## ProofMap

Every acceptance criterion in the linked Spec needs at least one Evidence row
covering it at the required witness level before the Verdict can pass. The
ProofMap computation runs inside `maestro verdict request` rather than as
a standalone verb. The verdict's `reasons[]` array surfaces gaps through the
`proof-map-incomplete` diagnostic code; the failing criterion ids are listed
alongside.

Example excerpt from a FAIL verdict caused by coverage gaps:

```
Decision:   FAIL
Reasons:
  [coverage] proof-map-incomplete: ac-2 under-witnessed (1 row, agent-claimed-locally)
  [coverage] proof-map-incomplete: ac-3 uncovered (0 rows)
```

Address gaps by recording additional Evidence rows tied to the specific
criterion (`maestro evidence record --task <id> --kind command --criterion
<ac-id>`), then re-run `maestro verdict request`. The `requireProofMapComplete`
policy in `policies/autopilot.yaml` controls whether ProofMap gaps gate the
verdict at PASS-class autopilot thresholds.
