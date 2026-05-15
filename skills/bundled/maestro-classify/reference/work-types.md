# Work Types

Six classifications used by `maestro-classify`. Each one maps to a specific intent and shapes the recommended next step.

## Taxonomy

### new-spec

A new feature with no existing implementation. None of the intended paths exist yet.

Examples:
- "Build a new payments service" → all of `src/features/payments/**` is greenfield
- "Add a CLI verb `maestro foo`" → no existing `foo.command.ts`
- "Create the auth module" → `src/features/auth/` does not exist

Signal: every entry in `intendedPaths` returns false from a filesystem existence check.

### spec-slice

Extending or filling in an existing feature. Paths exist and stay inside a single feature root.

Examples:
- "Add `--json` to `maestro intake`" → all paths under `src/features/intake/`
- "Add a new field to `IntakeResult`" → same feature, one new file plus edits
- "Wire a new use case into the existing handoff feature" → contained in `src/features/handoff/`

Signal: paths exist AND all share one `src/features/<name>/` (or analogous) root.

### change-request

Modifying existing behavior across multiple existing areas. The fallback when nothing more specific fits.

Examples:
- "Fix a bug in intake that affects task creation" → edits in intake + task
- "Refactor the verdict pipeline to add a new gate" → spans verify + verdict + risk
- "Tighten the lane heuristic for `tiny`" → modifies intake logic and tests

Signal: paths exist, span multiple areas, but no `multi-domain` flag.

### initiative

Large cross-domain work. Either the `multi-domain` flag is set, or the change spans 3+ feature areas.

Examples:
- "Add a new auth + authz + audit pipeline" → auth + authz + audit-security flags
- "Build the deploy gate end-to-end" → deploy + runtime + risk + verdict
- "Introduce a desktop app surface" → new top-level dir + cross-cutting wiring

Signal: `multi-domain` flag OR `intendedPaths` spans 3+ top-level feature areas.

### maintenance

Chore-type work: dependency bumps, configuration, tooling, CI files.

Examples:
- "Bump bun to 1.2" → `package.json`, lockfile
- "Update GitHub Actions workflow" → `.github/workflows/*.yml`
- "Add a `.editorconfig`" → root config files only

Signal: all paths are manifests, lockfiles, `.github/**`, or root config files.

### harness-improvement

Changes to the maestro harness itself. Paths under `.maestro/`, `policies/`, `skills/`, or `hooks/`.

Examples:
- "Add a new risk policy" → `policies/risk.yaml`
- "Update the `maestro-verify` skill" → `skills/bundled/maestro-verify/SKILL.md`
- "Add a doc template" → `.maestro/docs/HARNESS.md`

Signal: any path matches `.maestro/**`, `policies/**`, `skills/**`, or `hooks/**` (first-match wins over other categories).

## Decision tree

```
Start with IntakeResult and intendedPaths.

┌─ Any path under .maestro/ | policies/ | skills/ | hooks/?
│    └─ yes → harness-improvement
│
├─ multi-domain flag OR paths span 3+ feature areas?
│    └─ yes → initiative
│
├─ All paths are manifests / .github/** / root config?
│    └─ yes → maintenance
│
├─ None of the paths exist yet?
│    └─ yes → new-spec
│
├─ All paths share a single src/features/<one>/ root?
│    └─ yes → spec-slice
│
└─ else → change-request
```

## Harness-delta heuristic

`harness_delta_detected` is `true` whenever **any** path falls under:

- `.maestro/**` (project state, plans, tasks, telemetry, docs)
- `policies/**` (risk, autopilot, owners)
- `skills/**` (built-in and bundled skill sources)
- `hooks/**` (session/tool hooks)

This is independent of `work_type` — a `change-request` that also touches `policies/risk.yaml` has `harness_delta_detected: true` and should record a `harness-delta` Evidence row at the end of the task.

## Mapping to existing intake lanes

| lane | typical work_type pairings | rationale |
|---|---|---|
| `tiny` | `change-request`, `maintenance`, `spec-slice` | small, contained edits |
| `normal` | `spec-slice`, `change-request`, `harness-improvement` | the common case |
| `high-risk` | `new-spec`, `initiative`, plus anything with hard gates | needs spec + threat model |

The (work_type, lane) pair feeds `generateNextSteps` — see the table in `SKILL.md`.
