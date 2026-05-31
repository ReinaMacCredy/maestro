---
name: qa-slice
version: 1.0.0
description: Replay the affected baseline scenarios after each implementation wave and record proven slices for a feature. The ship gate requires a counting slice (scenarios + evidence) for every behavioral [bl-NNN] in the baseline.
---

# QA Slice

On activation, log the skill activation by piping a compact JSON payload to
`maestro hook record` with `event_type` set to `skill_activation`, `skill_name` set to
`qa-slice`, and `activation_mode` set to `agent_selected`.

Test the changed slice against the real scenarios the baseline named, while the diff is still small
enough to understand.

Run this after every implementation wave on a feature, before review and ship. Each proven slice is
recorded into `.maestro/features/<id>/qa-slices.yaml`. The `feature ship` gate reads that file and
blocks until every behavioral `[bl-NNN]` scenario from the baseline has a counting slice.

## Inputs

- Changed files and commands from the completed wave.
- The `QA Baseline Contract` at `.maestro/features/<id>/baseline.md`, including its `[bl-NNN]` ids.
- The feature's acceptance criteria and validation commands (`maestro feature show <id>`).
- Any artifacts from the wave: screenshots, generated files, API output, CLI output, state snapshots,
  logs, package/build outputs, migration output, or docs/examples.

## Workflow

1. Map changed files to affected behavior surfaces:
   - UI, CLI, API, library exports, persistence, data migration, auth/security, background jobs,
     config, packaging, performance, accessibility, docs/examples, and integration boundaries.
2. Select affected Scenario Matrix entries from the baseline:
   - Replay the `[bl-NNN]` scenarios touched by this wave.
   - If the wave touches a link in a critical workflow chain, replay the smallest useful connected
     chain that includes upstream setup/precondition, the changed link, and the nearest downstream
     consumer or observable output.
   - If the wave touches a safety-critical local invariant, keep the focused isolated probe too. The
     workflow chain proves composition; the isolated probe proves the invariant precisely.
   - If the wave creates new behavior, extend the baseline with a new `[bl-NNN]` entry instead of
     hiding it behind a unit test.
   - Choose by risk dimension, not by checklist category.
3. Select the smallest useful probes:
   - Existing focused tests first.
   - Add one real command/manual/API/UI sequence that traverses the changed workflow link when
     composition risk exists.
   - Do not drop targeted isolated probes for install/recovery, data integrity, auth/security,
     parser/schema, rollback, migration, or destructive-operation contracts.
   - Then smoke commands, snapshot/golden comparisons, fixture checks, API examples, screenshots,
     manual reproduction, installed binary/app checks, render checks, migration dry-runs, state
     comparisons, or targeted static checks.
4. Compare results against the baseline:
   - Preserve existing behavior unless the contract explicitly says it changes.
   - Treat unexplained output/API/schema/UI/permission/performance changes as blockers.
   - Compare real artifacts where possible: before/after command output, screenshots, generated
     files, persisted state, API responses, logs, or package contents.
5. Record each proven slice into `qa-slices.yaml` (see the shape below). A slice counts toward the
   ship gate only when it both references at least one `[bl-NNN]` scenario and carries non-empty
   evidence. A slice with no evidence does not count.
6. For any blocker you cannot clear, return a notes-ready tracker entry. Phrase findings as durable
   behavior reports: expected vs actual, reproduction steps, user/operator impact, and domain
   language. Do not fix code from this skill.

## qa-slices.yaml (machine contract the ship gate reads)

Append-only YAML at `.maestro/features/<id>/qa-slices.yaml`. The ship gate parses it; keep the shape
exact. `scenarios` lists the baseline `[bl-NNN]` ids this slice replayed (bare `bl-001` or bracketed
`[bl-001]` both match). Copy each id's digits verbatim from the baseline, including leading zeros:
`bl-001` and `bl-1` are different ids and will not match. `evidence` is the proof captured. A slice
counts when `scenarios` and `evidence` are both non-empty.

```yaml
slices:
  - at: "2026-05-31T00:00:00Z"
    scenarios: ["bl-001", "bl-002"]
    probes: ["cargo test --test feature_domain"]
    result: pass
    evidence:
      - "feature_domain: 12 passed; 0 failed"
      - "manual: feature new -> accept -> ship round-trips on temp .maestro"
  - at: "2026-05-31T00:10:00Z"
    scenarios: ["bl-003"]
    probes: ["maestro feature ship demo --dry-run"]
    result: pass
    evidence:
      - "dry-run rendered ship blocked: uncovered [bl-003]"
```

Every field except `at`, `scenarios`, and `evidence` is optional. If the gate cannot parse this file
it fails with the path, the parse error, and this shape so you can fix it.

## Output (blocker tracker)

```markdown
### Gate Tracker - QA Slice

- [ ] [qs-001] <severity/confidence> <surface> - <behavior drift or missing proof>
  - Scenario: [bl-NNN] <scenario name and key dimensions>
  - Expected: <expected behavior>
  - Actual: <actual behavior or missing proof>
  - Reproduction: <steps/command/manual flow>
  - Evidence: <command/output/manual check>
  - Artifact: <path/screenshot/output/log/state snapshot, or `None`>
  - Fix path: <recommended fix or probe>
  - Verification: <command or check>
```

If no blockers:

```markdown
### QA Slice

- No blocking QA findings for <wave/scope>.
- Workflow chains replayed: <chain names and steps, or `None: not touched`>
- Scenarios replayed: <bl-NNN scenario names and types>
- Probes run: `<command>`, <manual check>
- Artifacts captured: <paths/descriptions or `None`>
```

## Blocking Rules

- Block on user-visible regressions, data loss/corruption risk, public API/CLI contract drift,
  security/privacy regression, broken compatibility, missing migration proof, or an unverified
  critical path touched by the wave.
- Block when a changed user/operator journey is represented only by a unit test and no observable
  scenario, artifact, or command/manual proof.
- Block when a changed link in a critical workflow chain has only local proof and no upstream/downstream
  chain proof or explicit gap.
- Block when a safety-critical local invariant has only workflow-chain proof and no focused isolated
  probe or explicit gap.
- Block when a changed risk dimension is untested, such as upgraded state, rollback, permissions,
  malformed data, installed artifact behavior, accessibility, or release packaging.
- Do not block on broad nice-to-have coverage. Record it as a follow-up unless the feature goal
  depends on that coverage.
