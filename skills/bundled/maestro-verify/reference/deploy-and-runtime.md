# Deploy Gate, Runtime Signals, Cross-Task Conflict, Witnessed Rollback

## Contents

- [Deploy Gate](#deploy-gate)
- [Runtime Signals](#runtime-signals)
- [Cross-Task Conflict Evidence (L8.1)](#cross-task-conflict-evidence-l81)
- [Witnessed Rollback (Rule 10)](#witnessed-rollback-rule-10)

## Deploy Gate

`maestro deploy gate` runs four checks and records a `deploy-readiness`
Evidence row. It is **advisory by default** — it does not mutate the
Verdict. Teams wire it into `policies/risk.yaml` if they want it to gate
merges.

```bash
maestro deploy gate --task <id> [--base <ref>] [--json]
```

The four checks (all must pass for `gate=pass`):

| Check | Passes when |
|---|---|
| `feature_flag` | `Spec.rollout_plan.feature_flag` is a non-empty string |
| `canary_plan` | `Spec.rollout_plan.canary.stages` has at least one stage |
| `rollback` | A `rollback-exercised` Evidence row at `witnessed-by-ci` or stronger exists |
| `owner` | `owners.yaml.deploy_approver` has at least one entry |

Exit codes: 0 = gate pass, 1 = gate fail.

When CI runs the gate (teams wire this — it is not automatic): insert
`deploy gate` after the existing `ci verify` step and before merge. The
`deploy-readiness` Evidence row is recorded regardless of pass/fail, giving
teams an audit trail.

How `deploy-readiness` Evidence is consumed: currently advisory. To make it
gate, add a `deploy-readiness` signal to your `policies/risk.yaml`. The
Evidence row carries the full check breakdown — teams can inspect it with
`maestro evidence show <id>`.

See `docs/deploy-gate.md` for the full reference, `Spec.rollout_plan`
example, and troubleshooting table.

## Runtime Signals

`maestro runtime check` queries each signal declared in
`Spec.runtime_signals` and records one `runtime-signal` Evidence row per
signal. Exit code is always 0 — `pass=false` rows are advisory at L7.

```bash
maestro runtime check --task <id> [--provider-base-url <url>] [--json]
```

**`Spec.runtime_signals` schema** (`RuntimeSignal` shape):

```typescript
interface RuntimeSignal {
  name: string;
  description?: string;
  provider: string;    // e.g. "prometheus"
  query: string;       // PromQL or equivalent
  threshold: {
    operator: ">" | "<" | ">=" | "<=" | "==";
    value: number;
  };
  severity: "info" | "warn" | "critical";
}
```

Declare signals in the Spec (edit `.maestro/specs/<slug>.md` directly — there is
no `spec edit` verb). Signals with an unsupported provider are skipped
and recorded with `note: "unsupported provider"`.

**`runtime-signal` Evidence payload** (`RuntimeSignalPayload` shape):

```typescript
interface RuntimeSignalPayload {
  signal_name: string;
  provider: string;
  query: string;
  value: number;
  threshold: number;
  operator: string;
  pass: boolean;
  sampled_at: string; // ISO 8601
  note?: string;      // present when skipped or query errored
}
```

Advisory at L7: `pass=false` does not flip the Verdict by default. Teams add
`runtime-signal` to the evidence gate in `policies/risk.yaml` to make
failing signals blocking.

### Distinct from dev-time observability

`runtime check` is a release gate driven by `Spec.runtime_signals`; it
produces `runtime-signal` Evidence and runs once before the deploy
decision. For *ad-hoc* per-worktree observability while implementing, use
`maestro task observe metrics <promql>` and `maestro task observe logs`
instead (see `maestro-task` skill). Dev observations are advisory and
produce `manual-note` evidence tagged `[dev-observation]` only when
`--record` is set — they never gate the Verdict. Full reference:
`docs/dev-observability.md`.

Provider base URL precedence: `--provider-base-url` flag →
`MAESTRO_PROMETHEUS_URL` env → `http://localhost:9090`.

See `docs/runtime-monitoring.md` for the full reference and adapter guide.

## Cross-Task Conflict Evidence (L8.1)

`maestro ci verify` automatically detects when other open PRs touch
overlapping file paths and records a `kind=cross-task-conflict` Evidence row
when overlap is found.

**What it means for your task:**

- The Evidence row is recorded at `witnessed-by-ci` — CI performed the check, not the agent.
- The Risk Engine raises the effective risk class **one tier per conflict signal** (capped at `critical`). If the current diff is already at `medium` and a conflict row exists, the effective class becomes `high`.
- **Multi-row clamping:** even if multiple `cross-task-conflict` rows exist for the same run, the raise is capped at one tier total. The raise does not compound.
- The conflict is **advisory by default**: it raises risk class, which may push the Verdict from `PASS` to `HUMAN` or `BLOCK` depending on the team's autopilot policy thresholds. It does not unconditionally fail the verify step.
- On `gh api` errors (missing token, rate-limit, etc.), `ci verify` logs a warning and skips the record without failing. Treat a missing row as "unknown" rather than "clean."

**Payload shape:**

```typescript
interface CrossTaskConflictPayload {
  thisPr: number;
  conflictingPrs: number[];
  overlappingPaths: string[];
}
```

**What to do when you see a conflict row:**

1. Run `maestro evidence show <id>` to inspect which PRs and paths overlap.
2. Coordinate with the author of the conflicting PR before merging. The safest resolution is to merge the other PR first, rebase, and re-verify.
3. If the conflict is benign (e.g., unrelated edits to the same config file), document the coordination outcome in a `manual-note` Evidence row before requesting a Verdict.

See `docs/cross-task-conflict.md` for the full reference.

## Witnessed Rollback (Rule 10)

**Rule 10** (plain English): A rollback procedure must be mechanically
witnessed — by CI or by Maestro CLI itself — before a deploy can be
considered safe. A manual note is not sufficient.

`maestro deploy rollback` runs a shell command and records a
`rollback-exercised` Evidence row at the appropriate witness level:

```bash
maestro deploy rollback --task <id> --command "./scripts/rollback.sh" [--json]
```

Witness level assignment:
- `witnessed-by-ci` when `GITHUB_ACTIONS=true`
- `witnessed-by-maestro` when run locally

Both levels satisfy the rollback check in `deploy gate`.
`agent-claimed-locally` does **not** — the evidence must be mechanically
witnessed.

How the deploy gate consumes it: the rollback check in `deploy gate` passes
iff at least one `rollback-exercised` Evidence row at `witnessed-by-ci` or
stronger exists for the task. The Evidence row's `id` is stored in the
`deploy-readiness` payload as `checks.rollback.witness_evidence_id` for
audit purposes.

Recommended CI order:

```yaml
# 1. Exercise rollback (records rollback-exercised at witnessed-by-ci)
- run: maestro deploy rollback --task "$MAESTRO_TASK_ID" --command "./scripts/rollback.sh"
# 2. Gate (rollback check now passes because step 1 produced the evidence)
- run: maestro deploy gate --task "$MAESTRO_TASK_ID"
```

See `docs/deploy-gate.md` for the full reference and sample workflow.
