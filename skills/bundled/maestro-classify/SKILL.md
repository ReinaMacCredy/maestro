---
name: maestro-classify
description: Classify work type and harness impact before implementation. Wraps `maestro intake` with a 6-type taxonomy (new-spec, spec-slice, change-request, initiative, maintenance, harness-improvement) and emits telemetry. Auto-invokes alongside maestro-intake when an agent is about to start code changes.
---

# Maestro Classify

Plan-time work-type classifier. Wraps `maestro intake` and adds a 6-type classification (`work_type`) plus a `recommended_next_steps` string derived from the (work_type, lane) pair. Emits telemetry to `.maestro/telemetry/classify-events.ndjson` so Phase 2 can refine the heuristic against real usage.

This is a **Phase 1 skill** — its job is to gather telemetry before the same logic is promoted into the `maestro intake` primitive. See `reference/work-types.md` for the taxonomy.

---

## When to activate

Auto-activate when:

1. The user asks for a non-trivial implementation and `maestro-intake` is about to run (or has just run).
2. You are about to claim or create a `maestro task` for a multi-step change and haven't classified the work type yet.
3. The change touches `.maestro/`, `policies/`, or `skills/` (likely harness-delta).

Do not activate for:

- One-line typo fixes the user explicitly scoped to a single file.
- Read-only questions or explanations.
- Pure documentation edits the user asks for directly.

## Hard rules

1. **Run after intake.** Classification consumes `IntakeResult` (lane, flags). If you have not run `maestro intake --paths <paths> --json` yet, run it first.
2. **Real paths only.** Work type is derived from `intendedPaths`. Vague summaries without paths produce vague classifications.
3. **Append telemetry every time.** One NDJSON line per classification. Create `.maestro/telemetry/` if missing.

## Workflow

```text
1. maestro intake --paths <comma-list> --json   → IntakeResult
2. classify(intakeResult, paths) → work_type
3. next_steps = generateNextSteps(work_type, lane)
4. append NDJSON event to .maestro/telemetry/classify-events.ndjson
5. report { work_type, intake_result, recommended_next_steps, harness_delta_detected }
```

## Classification logic

Decide in this order — first match wins:

1. **harness-improvement** — any path matches `.maestro/**`, `policies/**`, `skills/**`, or `hooks/**`.
2. **initiative** — `multi-domain` flag is in `declaredFlags` OR auto-detected, OR `intendedPaths` spans 3+ top-level feature areas.
3. **maintenance** — paths are all package manifests (`package.json`, `bun.lockb`, `Cargo.toml`, `pyproject.toml`, lockfiles), or `.github/**` workflow files, or `.gitignore`/`.editorconfig`-style config.
4. **new-spec** — none of the `intendedPaths` exist on disk yet (fresh feature, no overlap with existing files).
5. **spec-slice** — paths exist AND all live under a single feature root (`src/features/<one-name>/**` or equivalent).
6. **change-request** — fallback. Paths exist and span multiple existing areas or modify behavior elsewhere.

Edge cases:

- **Mixed harness + product paths** → `harness-improvement` wins (rule 1 first-match). Surface this in `harness_delta_detected: true` regardless.
- **No paths supplied** → return `work_type: "change-request"` with `classification_confidence: "low"` and note in telemetry.
- **Manual override** — if the user explicitly says "this is a new-spec" or similar, honor it and set `manual_override: true` in telemetry.

## Next-steps mapping

`generateNextSteps(work_type, lane)` returns one of these strings:

| work_type | tiny | normal | high-risk |
|---|---|---|---|
| `new-spec` | "Create task with `maestro task plan`" | "Create mission spec, then `maestro task plan`" | "Create mission spec with threat model" |
| `spec-slice` | "Create task with `maestro task plan`" | "Create task, reference parent spec" | "Create task with threat model, reference parent spec" |
| `change-request` | "Create task, implement, verify" | "Create task with regression test plan" | "Create task with threat model and regression tests" |
| `initiative` | "Create epic task, break into subtasks" | "Create mission spec, break into tasks" | "Create mission spec with threat model" |
| `maintenance` | "Create chore task, implement directly" | "Create chore task with verification plan" | "Create chore task with impact analysis" |
| `harness-improvement` | "Create task, update harness, verify" | "Create task, record `harness-delta` evidence" | "Create task with policy impact analysis" |

## Output shape

Report this JSON back to the agent loop (and the user):

```json
{
  "work_type": "spec-slice",
  "intake_result": { "lane": "normal", "...": "..." },
  "recommended_next_steps": "Create task, reference parent spec",
  "harness_delta_detected": false
}
```

## Telemetry shape

One NDJSON line per classification, appended to `.maestro/telemetry/classify-events.ndjson`:

```json
{
  "ts": "2026-05-15T10:00:00Z",
  "task_id": "<task-id-or-null>",
  "work_type": "spec-slice",
  "lane": "normal",
  "flags": ["existing-behavior"],
  "paths_count": 3,
  "session_id": "<session-id-or-null>",
  "agent_invoked": true,
  "invocation_method": "auto",
  "classification_confidence": "high",
  "manual_override": false,
  "harness_delta_detected": false
}
```

The directory does not exist by default. Create it (with parents) before the first append.

## Decision gate (Phase 1 promotion)

This skill is **provisional**. Phase 2 promotes the logic into `maestro intake` itself once telemetry shows:

- 100+ classification events
- All 6 work types used (>5% share each)
- <20% misclassification rate on a 50-sample manual review
- 50%+ adoption rate (agent_invoked / total opportunities)
- 3+ valuable edge cases discovered

If adoption drops below 30%, Phase 2 will wire classification directly into the `maestro intake` command (no separate skill required).

## See also

- `reference/work-types.md` — full taxonomy with examples and decision tree
- `skills/bundled/maestro-intake/SKILL.md` — upstream lane classifier
- `skills/bundled/maestro-verify/SKILL.md` — canonical verification protocol
