# Council output-contract patterns

Composable patterns, not a universal schema. The Lead picks and adapts only
what the case needs. Every contract must let a reader tell direct observation,
authority, inference, and uncertainty apart, and name the action each material
conclusion changes.

## Focused decision

For choosing one architecture, product, policy, or strategy route.

```text
POSITION
RECOMMENDATION

DECISION-CRITICAL CLAIMS
- CLAIM
- TYPE: FACT | INFERENCE | CAUSAL CLAIM | FORECAST | VALUE / PREFERENCE | AUTHORITATIVE CONSTRAINT
- EVIDENCE OR AUTHORITY
- VERDICT IMPACT

BEST ALTERNATIVE
STRONGEST COUNTERARGUMENT
PRIMARY FAILURE MODE
FALSIFIER / WHAT WOULD CHANGE MY MIND
UNKNOWNS
CONFIDENCE BASIS: HIGH | MEDIUM | LOW, because ...
```

No numeric confidence.

## Supplied findings or audit

One row per supplied finding, no row cap.

```markdown
| Finding | Disposition | Direct evidence | Classification | Durable route | Confidence/limits |
|---|---|---|---|---|---|
| F001 | confirmed / falsified / narrowed / insufficient coverage | ... | bounded / foundation / architecture / mechanism / proof-only | ... | ... |
```

New findings use the same fields and stay separate from the supplied ones.

## Plan or contract review

One row per gate or obligation: status, governing authority, evidence,
impact, required correction. Keep the user's requirement identity.

## Incident

The smallest truthful timeline, causal claims, containment and recovery
decisions, unknowns, discriminating evidence. No option memo.

## Material proposition ledger

For cross-cutting claims above a larger ledger, or a focused decision.

```markdown
| ID | Type | Proposition | Source/excerpt | Evidence bar | Status | Verdict impact |
|---|---|---|---|---|---|---|
| P1 | FACT | ... | seat, excerpt | direct source evidence | unresolved | high |
```

A proposition is material only when its truth or authority could change the
verdict or the required action.

## Cross-examination response

Rename `PROPOSITION_ID` to the case's identifier (`FINDING_ID`, `GATE_ID`).

```text
PROPOSITION_ID
RESPONSE: CONCEDE | MAINTAIN | NARROW | REVERSE
REASON
DIRECT EVIDENCE
NEW CLAIMS, if any
FALSIFIER
IF TRUE, RECOMMENDATION IMPACT
IF FALSE, RECOMMENDATION IMPACT
```

## Draft-verdict audit

```text
AUDIT RESULT: CLEAR | REVISE | STOP

FINDINGS
- SEVERITY: material | non-material
- CATEGORY: falsified premise | unsupported new claim | unanswered dissent |
  omitted material claim | preference-as-constraint | scope breach |
  action mismatch | vague reopen condition
- EVIDENCE
- REQUIRED CORRECTION

UNCHECKED LIMITATIONS
```

The Auditor names defects; it never issues or replaces the verdict.
