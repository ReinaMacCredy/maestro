# Audit

Use this recipe for a review, risk scan, or verification request. Audit is
read-only unless implementation is separately authorized.

When the symptom does not yet name an owning layer, run the ordered
[triage](triage.md) recipe before selecting probes.

## Loop anatomy

### Perceive

Read the requested scope, acceptance, current diff, relevant tests, and source.
State the exact boundary and the failure classes worth probing.

### Choose

Pick the highest-risk bounded probe that could expose a real defect. Prefer
the real consumer path over internal state or mock interactions.

### Act

Run the probe and inspect the smallest relevant source slice. Do not silently
fix findings during a read-only review.

### Observe

For each candidate, prove reproducibility and impact. Discard speculation.
Rank surviving findings by severity and name the missing regression check.

### Learn

Create or note follow-up work only when the finding is actionable and within
the repository's authority model. Tie every durable lesson to evidence.

### Continue

Return findings first with concrete locations and reproduction paths, or state
that no issues were found. Separate verified findings from residual risk.
