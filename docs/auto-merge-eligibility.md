# Auto-Merge Eligibility

`maestro merge auto` runs 8 deterministic eligibility predicates before triggering `gh pr merge --auto`. If any predicate fails, the command exits 1 and prints the failing codes. No merge is attempted.

This document enumerates each predicate in canonical order, explains what triggers it, and explains how to fix it.

---

## Predicates

### 1. `verdict-not-pass`

**What it means:** the task's current verdict decision is not `PASS`.

**Condition:** `verdict.decision !== "PASS"` (decision is `FAIL`, `HUMAN`, or `BLOCK`).

**How to fix:** resolve whatever caused the non-PASS verdict. Run `maestro verdict show --task <id>` to read the reasons, address them, then re-run `maestro verdict request --task <id>`.

---

### 2. `auto-merge-class-disabled`

**What it means:** the effective autopilot policy has `autoMergeAllowed.<riskClass>` set to `false` for the verdict's effective risk class.

**Condition:** `autopilot.yaml` explicitly sets `autoMergeAllowed.<class>: false` (or is absent — all classes default to `false`).

**How to fix:** opt in by setting `autoMergeAllowed.<class>: true` in `.maestro/policies/autopilot.yaml` for the relevant risk class. This is an intentional gate; auto-merge is disabled everywhere by default. L6 does not change the `autoMergeAllowed` field semantics — it existed from L3 — but L6 is the first layer that consumes it.

```yaml
# .maestro/policies/autopilot.yaml
autoMergeAllowed:
  low: true
  medium: true
  high: false
  critical: false
```

---

### 3. `evidence-witness-too-weak`

**What it means:** one or more gating evidence rows have a witness level below `witnessed-by-ci`.

**Condition:** any evidence row with `kind` in `{command, verifier, ai-review, threat-model, plan-check}` has `witness_level` of `agent-claimed-locally` or `agent-claimed-and-not-reproducible`.

**How to fix:** the affected evidence rows need to be re-recorded at `witnessed-by-ci` level. This normally means re-running them inside a CI job via `maestro ci verify`, which ingests job results at the `witnessed-by-ci` level automatically. Evidence recorded locally by the agent stays at `agent-claimed-locally` and does not satisfy this gate.

---

### 4. `forbidden-paths-touched`

**What it means:** the diff contains paths that match the contract's `scope.filesForbidden` globs.

**Condition:** `changedPaths` intersects `contract.scope.filesForbidden`.

**How to fix:** either stop touching the forbidden paths, or remove them from `filesForbidden` via `maestro contract amend --task <id> --remove-path <glob> --reason "<why>"` (consumes one amendment budget slot).

---

### 5. `sensitive-paths-untouched-without-waiver`

**What it means:** the diff touches paths matched by the repo's sensitive-path globs, and no `verdict-override` evidence row exists.

**Condition:** `changedPaths` intersects `policies/sensitive-paths.yaml` globs AND no `verdict-override` evidence row is present for the task.

**How to fix:** two options:
- Record a `verdict override` to add a waiver evidence row: `maestro verdict override --task <id> --pr <n> --reason "<why>"`. Requires the invoking user to be in `owners.yaml.sensitive_waiver`.
- If the paths are not genuinely sensitive, remove them from `sensitive-paths.yaml` (policy tightening/loosening rules apply).

---

### 6. `rollback-not-witnessed`

**What it means:** no `rollback-exercised` evidence row at `witnessed-by-ci` level exists for the task.

**Condition:** `evidenceRows` contains no row where `kind === "rollback-exercised"` and `witness_level === "witnessed-by-ci"`.

**L6/L7 note:** this predicate is a normal output until L7.5 ships the CI producer for `rollback-exercised` evidence. Until then, every auto-merge attempt will fail this check. To unblock auto-merge before L7.5, you can manually record a waiver (see `verdict override`) or wait for the L7.5 rollback-exercise CI step. No workaround exists that satisfies the predicate mechanically until the evidence producer exists.

---

### 7. `review-ack-missing`

**What it means:** the verdict is `HUMAN` at risk class `medium` or higher, but no `review-ack` evidence row has been recorded.

**Condition:** `verdict.decision === "HUMAN"` AND `verdict.effectiveRiskClass >= medium` AND no `review-ack` evidence row with `witness_level >= agent-claimed-locally` exists.

**How to fix:** after a human reviewer has reviewed the change, record a review acknowledgement:

```bash
maestro review ack \
  --task <id> \
  --verdict <verdict-id> \
  --criterion "All tests pass" \
  --criterion "No critical findings"
```

Re-run `maestro merge auto` after recording the ack.

---

### 8. `spec-score-below-threshold`

**What it means:** the linked Mission Spec has a quality score below 1.0.

**Condition:** the task's contract has a `missionId` AND `scoreSpec(spec).score < 1.0`.

**How to fix:** run `maestro spec show --mission <id>` to see which fields are incomplete. Edit the spec with `maestro spec edit --mission <id>` until all required fields are populated and the score reaches `1.0`. If no Spec is associated with the task (no `missionId` on the contract), this predicate is skipped.

---

## "Why isn't my PR auto-merging?" Troubleshooting

Run `maestro merge auto --pr <n> --task <id>` and read the output. It prints the exact failing codes and their messages. The 8 codes in canonical check order:

| Code | Quick diagnosis |
|---|---|
| `verdict-not-pass` | `maestro verdict show --task <id>` to read the verdict reasons |
| `auto-merge-class-disabled` | Check `autoMergeAllowed.<class>` in `autopilot.yaml`; defaults to `false` for all classes |
| `evidence-witness-too-weak` | Gating evidence exists but was recorded locally; needs re-recording via CI |
| `forbidden-paths-touched` | Diff intersects `contract.scope.filesForbidden`; amend or narrow the diff |
| `sensitive-paths-untouched-without-waiver` | Diff touches sensitive paths; record a `verdict override` or adjust policy |
| `rollback-not-witnessed` | Normal until L7.5 ships the `rollback-exercised` evidence producer |
| `review-ack-missing` | HUMAN verdict at `>=medium` risk; run `maestro review ack` after human review |
| `spec-score-below-threshold` | `maestro spec show --mission <id>` shows which fields are missing |

When all 8 predicates pass, `merge auto` triggers `gh pr merge --auto` and exits 0. When any fail, it exits 1 and the PR is not touched.

---

## Reference

- Policy opt-in — `docs/policy-format.md` (autoMergeAllowed field)
- Override flow — `docs/override-flow.md`
- Source — `src/features/merge/usecases/auto-merge-eligible.usecase.ts`
- Eligibility types — `src/features/merge/domain/eligibility-types.ts`
