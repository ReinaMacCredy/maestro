# AI Reviewer Protocol

L4 ships an `ai-review` Evidence kind so agent runtimes can record structured
reviewer findings against a task. This document describes what AI Reviewer
Evidence is, how the Risk Engine consumes it, the finding schemas, and the
recording CLI.

Related: `docs/threat-model-format.md`, `docs/witness-levels.md`,
`docs/risk-class-derivation.md`.

---

## Overview

An `ai-review` Evidence row captures the output of an LLM reviewer pass (bug,
security, or architecture) as a structured payload attached to a task. The Risk
Engine reads these rows when computing the effective risk class for a verdict
request.

**Rule 1 — LLM signals are veto-only.** AI Reviewer findings can raise the
derived risk class; they can never lower it. If the deterministic diff signals
produce `medium` and a reviewer finds nothing, the class stays `medium`. If a
security reviewer finds an `error`-severity issue, the class rises.

L4 ships the Evidence kind, the `evidence record` verb, and the Risk Engine
wiring. Running the actual reviewer LLM is the job of the agent runtime.

---

## Reviewer Kinds

Three reviewer kinds are defined at L4. At runtime, the agent selects 1–2
reviewers per task; L6.4 will expand the set.

### `bug`

Correctness reviewer. Catches edge cases, regressions, type mismatches, null
dereferences, and logic errors introduced by the diff.

### `security`

Security reviewer. Checks auth flows, input validation, secrets in code,
injection vectors (SQL, shell, template), insecure defaults, and cross-origin
trust issues. A `security`-reviewer `error` always lifts the effective risk
class to `critical`, regardless of the starting class.

### `architecture`

Architecture reviewer. Identifies boundary violations, unintended coupling,
premature abstraction, and misuse of established patterns in the codebase.

---

## `AIReviewFinding` Schema

Each finding in a review is one object with the following fields:

```typescript
interface AIReviewFinding {
  severity:   "info" | "warn" | "error";
  message:    string;
  paths?:     string[];    // file paths the finding relates to (optional)
  suggestion?: string;     // remediation hint (optional)
}
```

**Severity semantics:**

- `error` — a defect or risk that warrants raising the risk class. The Risk
  Engine acts on `error` findings.
- `warn` — something noteworthy that does not rise to a risk-class raise. The
  Risk Engine does not act on `warn` findings but they appear in the verdict
  output.
- `info` — informational only. No effect on the verdict.

---

## `AIReviewPayload` Schema

The full payload stored in the Evidence row:

```typescript
interface AIReviewPayload {
  reviewer:      "bug" | "security" | "architecture";
  findings:      AIReviewFinding[];
  confidence:    number;       // 0–1 float
  criterion_id?: string;       // acceptance-criterion this row covers (optional)
}
```

---

## Confidence Semantics

`confidence` is a 0–1 float that the reviewer runtime sets to indicate how
reliable its findings are. Convention:

- `< 0.5` — low-confidence finding. Treat as advisory; it may be a false
  positive. Surface it to the agent but do not over-weight it.
- `>= 0.5` — sufficient confidence to act on.

The Risk Engine at L4 treats all `error`-severity findings the same regardless
of confidence — the risk-class raise fires either way. Room for evolution at
L6: a future threshold rule could dampen low-confidence errors.

Runtimes producing reviewer findings should calibrate confidence based on how
much context they used, whether the relevant code was fully visible, and how
well-specified the task contract is.

---

## Recording Guidance

Record reviewer findings immediately after the reviewer LLM completes and
before requesting a verdict.

### Inline JSON

```bash
maestro evidence record --task <id> --kind ai-review \
  --reviewer security \
  --findings '[{"severity":"error","message":"SQL injection risk in buildQuery","paths":["src/db/query.ts"],"suggestion":"Use parameterised queries"}]' \
  --confidence 0.9
```

### From a file

If the reviewer output is large, write it to a file first and pass the path:

```bash
maestro evidence record --task <id> --kind ai-review \
  --reviewer bug \
  --findings ./reviewer-output.json \
  --confidence 0.75
```

The file must be a JSON array of `AIReviewFinding` objects, or a YAML sequence
of the same shape.

### Clean review (no findings)

```bash
maestro evidence record --task <id> --kind ai-review \
  --reviewer architecture \
  --findings '[]' \
  --confidence 0.8
```

A clean review with no `error` findings does not lower the deterministic
baseline; it simply adds an evidence row.

---

## Witness Level

The default witness level for `ai-review` evidence is `agent-claimed-locally`.
When a trusted CI gate ran the reviewer and posted the findings:

```bash
maestro evidence record --task <id> --kind ai-review \
  --reviewer security \
  --findings ./ci-security-review.json \
  --confidence 0.95 \
  --witness witnessed-by-ci
```

See `docs/witness-levels.md` for the full ladder and policy interaction.

---

## `criterion_id` Linkage

When a finding maps to a specific acceptance criterion, set `criterion_id` so
the ProofMap can correlate:

```bash
maestro evidence record --task <id> --kind ai-review \
  --reviewer security \
  --findings '[{"severity":"info","message":"No injection vectors found in input validation paths"}]' \
  --confidence 0.85 \
  --criterion ac-3
```

The ProofMap builder (`src/features/verify/usecases/proof-map.ts`) counts
`ai-review` rows as coverage for the named criterion. Without `criterion_id`,
the row contributes to the overall evidence corpus but does not satisfy any
specific criterion gap.

---

## Rule 1 Invariants

These invariants hold at L4 and are enforced by
`src/features/risk/usecases/compute-risk.ts`:

1. Any `error`-severity finding in an `ai-review` row raises
   `effectiveRiskClass` by one notch (e.g., `medium` → `high`).
2. A `security`-reviewer `error` always lifts `effectiveRiskClass` to
   `critical`, regardless of the starting class.
3. A clean `ai-review` (zero `error` findings) **never lowers** the
   deterministic baseline produced by `deriveRiskClassFromDiff`. The LLM can
   only veto, never approve.

---

## What L4 Does Not Include

L4 ships the Evidence kind, the `evidence record` verb, and the Risk Engine
wiring. It does not include:

- Running the reviewer LLM itself. That is the agent runtime's responsibility.
- Selecting which files to pass to the reviewer. The agent decides.
- Scheduling reviewer invocations automatically. The pre-claim ritual (see
  `maestro-verify`) describes when to run reviewers, but the invocation is
  manual at L4.
- More than 3 reviewer kinds. L6.4 will expand the set.

---

## Full Payload Example

```json
{
  "reviewer": "security",
  "findings": [
    {
      "severity": "error",
      "message": "SQL injection risk: string interpolation in buildQuery",
      "paths": ["src/db/query.ts"],
      "suggestion": "Use parameterised queries via db.prepare()"
    },
    {
      "severity": "warn",
      "message": "Error message leaks internal table name",
      "paths": ["src/db/query.ts"]
    }
  ],
  "confidence": 0.9,
  "criterion_id": "ac-4"
}
```
