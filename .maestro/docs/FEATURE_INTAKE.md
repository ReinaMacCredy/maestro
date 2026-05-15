# Feature Intake Guide

Before claiming a task or creating a plan, run `maestro intake --paths <paths> --json` to classify the work. This document explains how the output is derived and how to act on it.

## Classification Decision Tree

```text
Start with IntakeResult and intendedPaths.

┌─ Any path under .maestro/ | policies/ | skills/ | hooks/?
│    └─ yes → harness-improvement
│
├─ multi-domain flag OR paths span 3+ top-level dirs?
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

First-match wins. A path that lands in `.maestro/` is `harness-improvement` even if other paths in the same call look like a `spec-slice`.

> **Note for brownfield codebases**: `spec-slice` requires the `src/features/<name>/` layout. Projects that organize code as `src/<domain>/` (without `features/`) will get `change-request` instead — that's the intended fallback. The two work types share most lane-driven next-steps, so adoption doesn't require restructuring.

## Common Patterns

| User Request | Work Type | Rationale |
|---|---|---|
| "Add new API endpoint" | `spec-slice` | Extending existing API surface |
| "Fix bug in login" | `change-request` | Modifying existing behavior |
| "Update dependencies" | `maintenance` | Chore-type work |
| "New auth system" | `initiative` | Multi-domain, large scope |
| "Add risk policy" | `harness-improvement` | Harness modification |
| "Refactor verdict pipeline" | `change-request` | Existing behavior, multi-area |
| "Build the desktop app" | `new-spec` | No prior implementation |

## How to act on the result

The intake response includes both `recommendedNextStep` (lane-derived) and `recommendedNextSteps` (work-type + lane derived). Use the latter when present — it's more specific.

| Work type | Lane | Recommended next step |
|---|---|---|
| `new-spec` | tiny | Create task with `maestro task plan` |
| `new-spec` | normal | Create mission spec, then `maestro task plan` |
| `new-spec` | high-risk | Create mission spec with threat model |
| `spec-slice` | tiny | Create task with `maestro task plan` |
| `spec-slice` | normal | Create task, reference parent spec |
| `spec-slice` | high-risk | Create task with threat model, reference parent spec |
| `change-request` | tiny | Create task, implement, verify |
| `change-request` | normal | Create task with regression test plan |
| `change-request` | high-risk | Create task with threat model and regression tests |
| `initiative` | tiny | Create epic task, break into subtasks |
| `initiative` | normal | Create mission spec, break into tasks |
| `initiative` | high-risk | Create mission spec with threat model |
| `maintenance` | tiny | Create chore task, implement directly |
| `maintenance` | normal | Create chore task with verification plan |
| `maintenance` | high-risk | Create chore task with impact analysis |
| `harness-improvement` | tiny | Create task, update harness, verify |
| `harness-improvement` | normal | Create task, record `harness-delta` evidence |
| `harness-improvement` | high-risk | Create task with policy impact analysis |

## Harness impact

`IntakeResult.harnessImpact` is `true` whenever any path falls under `.maestro/`, `policies/`, `skills/`, or `hooks/` — independent of the work type. When it's true, plan to record a `harness-delta` evidence row at task close.
