# Maestro Research Examples

## Sales Copilot Regression

Input:

- The user is trying a skill, not approving product scope.
- The domain and stakeholder workflow are not validated.
- The user appears to choose option B while still exploring.
- The intended hosting repo is CloudBrief, but the idea may belong in an
  external or sandbox project.

Expected receipt:

```text
Hosting:
  project: external | sandbox-repo
  rationale: Sales Copilot hosting is not confirmed for the current repo.

Unknowns:
  Blocking:
  - Which sales chat app is canonical?

Stakeholder Actions:
  - question: Which sales chat app is canonical?
    ask: Sales Lead
    status: open
    blocks: integration architecture fork

Recommended First Design Fork:
  Where should Copilot live in the Sales workflow?

Gate:
  NEEDS_STAKEHOLDER
```

Forbidden:

- `READY_FOR_DESIGN` on the current repo
- decision lock
- acceptance criteria
- implementation tasks

## Valid Skip Receipt

Use this when the user gives a settled spec and explicitly does not want
research.

```text
Research Status:
  skipped: true
  skip_reason: settled spec pasted
  skipped_by: agent
  evidence: request.md has concrete problem, users, constraints, acceptance,
    non-goals, and intended repo.

Hosting:
  project: current-repo
  rationale: pasted spec names this repository.

Gate:
  READY_FOR_DESIGN
```

## Risky User Skip

Use this when the user explicitly skips research for high-impact work.

```text
Research Status:
  skipped: true
  skip_reason: user explicit
  skipped_by: user
  unresolved_risks:
  - customer data retention policy is unknown
  - auth boundary is unknown

Gate:
  READY_FOR_DESIGN
```

The gate may proceed only because the user owns the risk. The unresolved risks
must be visible to `maestro-design`.

## Hosting Mismatch

If research says sandbox but the user later targets the current repo:

```text
Gate:
  NEEDS_EVIDENCE

Next:
  Supersede research.md before starting maestro-design in the current repo.
```

## Stale Same-Title Research

A card titled "Sales Copilot" is stale if the problem statement changes from
inbox assistant to browser overlay.

```text
Gate:
  NEEDS_EVIDENCE

Reason:
  Same title, different problem statement.
```

## Prior Art Found

If `maestro grep "<topic>" corpus:memory` finds an existing solved feature:

```text
Gate:
  STOP | PIVOT

Pointer:
  existing card or decision id
```

Do not open a new design unless the delta from prior art is explicit.
