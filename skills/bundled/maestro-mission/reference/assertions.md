# Assertions

Assertions are validation targets attached to features. A **feature** has its own status (pending / assigned / in-progress / review / done / blocked); the **assertions** attached to that feature have their own results (passed / failed / blocked / waived). The two are distinct.

## Feature status vs assertion result

| Axis | Values |
|---|---|
| `maestro feature update --status` | `pending`, `assigned`, `in-progress`, `review`, `done`, `blocked` |
| Assertion result (inside `--report`) | `passed`, `failed`, `blocked`, `waived` (plus `pending` before reporting) |

## Assertion result states

- `pending`: assertion exists but has not been validated.
- `passed`: assertion succeeded.
- `failed`: assertion ran and did not succeed.
- `blocked`: assertion cannot run because of an upstream dependency or environment issue.
- `waived`: assertion is intentionally skipped, with a recorded reason.

## Update via feature progress

```bash
maestro feature update <featureId> --mission <missionId> \
  --status done \
  --report @report.json
```

`--report` accepts an inline JSON object or `@path/to/report.json` to load from disk. Typical report shape:

```json
{
  "status": "passed",
  "summary": "All unit and integration tests green under the new JWT path.",
  "evidence": [
    { "kind": "log", "path": "logs/auth-tests-20260422.log" },
    { "kind": "command", "value": "bun test tests/integration/auth.test.ts" }
  ]
}
```

For `waived`, include the waiver reason:

```json
{ "status": "waived", "reason": "Covered by manual QA; automated test deferred to follow-up." }
```

## Reading current state

```bash
maestro feature list --mission <missionId> --json
maestro mission show <missionId>
```

The feature prompt (`maestro feature prompt <featureId> --mission <missionId>`) includes assertion definitions so the executing agent knows what must pass.

## Rules

- A feature cannot be closed while an assertion is in `pending` or `blocked` without either waiving it or resolving the underlying blocker.
- `failed` assertions surface to the mission level and influence mission lifecycle.
- `waived` requires a reason in the report; waivers without rationale are rejected.
