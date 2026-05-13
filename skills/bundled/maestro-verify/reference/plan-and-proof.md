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
covering it before the Verdict can pass.

```bash
maestro task proof --task <id>
maestro task proof --task <id> --json
```

The verb prints which criteria are covered, by how many rows, and at what
witness levels. A criterion with zero rows is `uncovered`. A criterion with
rows all below the autopilot threshold is `under-witnessed`.

Example output:

```
ProofMap for task tsk-aaa123:
  [covered]         ac-1  "API returns 200 for valid input"  (2 rows, witnessed-by-maestro)
  [under-witnessed] ac-2  "No secrets in response"  (1 row, agent-claimed-locally)
  [uncovered]       ac-3  "Error path returns 422"
```

Run `maestro task proof` after every significant Evidence record and before
requesting a Verdict. Gaps here will cause a `FAIL` or `HUMAN` verdict.
