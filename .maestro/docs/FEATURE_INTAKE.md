# Feature Intake Guide

Before authoring a product-spec, decide the **work type** and the **risk lane**. The work type is recorded in spec frontmatter (`work_type: <one of six>`); the risk lane derives from the spec's sensitive paths + risk class at `task verify` time. This document is the decision tree.

## Classification Decision Tree

```text
Start with the intended file paths.

┌─ Any path under .maestro/ | policies/ | skills/ | hooks/?
│    └─ yes → harness-improvement
│
├─ Paths span 3+ top-level dirs OR cross feature boundaries?
│    └─ yes → initiative
│
├─ All paths are manifests / .github/** / root config?
│    └─ yes → maintenance
│
├─ None of the paths exist yet?
│    └─ yes → new-spec
│
├─ All paths share one src/features/<one>/ root?
│    └─ yes → spec-slice
│
└─ else → change-request
```

First-match wins. A path that lands in `.maestro/` is `harness-improvement` even if other paths in the same spec look like `spec-slice`.

> **Brownfield projects**: `spec-slice` requires a `src/features/<name>/` layout. Projects organized as `src/<domain>/` (without `features/`) get `change-request` instead — that's the intended fallback. The two work types share most lane-driven next-steps, so adoption does not require restructuring.

## Common patterns

| User Request                       | Work Type             | Rationale                                       |
| ---------------------------------- | --------------------- | ----------------------------------------------- |
| "Add new API endpoint"             | `spec-slice`          | Extending existing API surface                  |
| "Fix bug in login"                 | `change-request`      | Modifying existing behavior                     |
| "Update dependencies"              | `maintenance`         | Chore-type work                                 |
| "New auth system"                  | `initiative`          | Multi-domain, large scope (promote to plan)     |
| "Add risk policy"                  | `harness-improvement` | Harness modification                            |
| "Refactor verdict pipeline"        | `change-request`      | Existing behavior, multi-area                   |
| "Build the desktop app"            | `new-spec`            | No prior implementation                         |

## How to act on the result

Work type drives the spec-mode and the authoring path. Risk lane comes from the verify step.

| Work type             | Lane       | Authoring path                                                                 |
| --------------------- | ---------- | ------------------------------------------------------------------------------ |
| `new-spec`            | tiny       | `maestro spec new <slug>` (light) → `maestro task from-spec`                   |
| `new-spec`            | normal     | `maestro spec new <slug> --mode heavy` → `maestro plan from-spec`              |
| `new-spec`            | high-risk  | Heavy spec + threat-model evidence + heavy decomposition                       |
| `spec-slice`          | tiny       | `maestro spec new <slug>` (light), reference parent spec in body               |
| `spec-slice`          | normal     | `maestro spec new <slug>` (light), reference parent spec, name regression test |
| `spec-slice`          | high-risk  | `maestro spec new <slug> --mode heavy`, threat model, regression tests         |
| `change-request`      | tiny       | Light spec, implement, verify                                                  |
| `change-request`      | normal     | Light spec with regression-test acceptance criterion                            |
| `change-request`      | high-risk  | Heavy spec with threat model + regression tests                                |
| `initiative`          | any        | Heavy spec → `plan from-spec` → `plan decompose` (multi-PR)                    |
| `maintenance`         | tiny       | Light spec, implement directly                                                 |
| `maintenance`         | normal     | Light spec with verification plan                                              |
| `maintenance`         | high-risk  | Light spec with impact analysis                                                |
| `harness-improvement` | tiny       | Light spec, update harness, verify, record `harness-delta` evidence            |
| `harness-improvement` | normal     | Light spec, `harness-delta` evidence at task close                             |
| `harness-improvement` | high-risk  | Heavy spec with policy-impact analysis                                         |

## Harness impact

Whenever any path falls under `.maestro/`, `policies/`, `skills/`, or `hooks/`, the change has harness impact and a `harness-delta` evidence row should be recorded at task close.

## See also

- `maestro-design` skill — runs the grill protocol over this decision tree during spec authoring.
- `HARNESS.md` — product delta vs harness delta semantics.
- `VALIDATION_LADDER.md` — how the L0–L7 witness ladder maps to risk lanes.
- `docs/cli-reference.md` — verb-by-verb reference.
