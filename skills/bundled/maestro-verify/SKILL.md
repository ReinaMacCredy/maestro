---
name: maestro-verify
description: The canonical verification protocol for any task in a maestro project. Documents witness levels, Trust Verifier scope, ProofMap, plan-check, verdict semantics, cost-budget monitoring, AI Reviewer protocol (Rule 1 veto-only), and threat-model production. Cross-referenced by maestro-task, maestro-plan, and maestro-handoff. Read this skill when starting a non-trivial task or before declaring completion.
---

# Maestro Verify

This skill is the canonical verification protocol every agent follows before
claiming a task is done. Read it when planning a non-trivial task or before
declaring completion.

Cross-referenced by `maestro-task` (pre-claim ritual), `maestro-plan`
(plan-check), and `maestro-handoff` (handoff gate). When those skills say
"see maestro-verify", this is the document.

The trigger body below names the steps and points to one reference file per
substantive topic. Open the matching reference when you reach that step in
the pre-claim ritual.

---

## The Pre-Claim Ritual

Run this loop before marking any task done. Steps are ordered; do not skip.

1. **Plan** — write a plan file and run
   `maestro plan check --task <id> --plan-file <path>`. Address
   `scope-widens`, `missing-proof`, and `risk-class-too-low` before writing
   code. Read `reference/plan-and-proof.md` for the plan file schema.

2. **Implement** — write code. Record Evidence after each verification
   command. Witness levels and Evidence kinds: see
   `reference/witness-levels.md`.

   ```bash
   maestro evidence record --task <id> --command "bun test" --exit 0
   ```

3. **Verify** — `maestro task verify --task <id>`. Address every `error`
   finding. The 8 deterministic checks and architecture-lint rules are
   documented in `reference/trust-verifier.md`.

4. **ProofMap** — `maestro task proof --task <id>`. Every criterion must be
   covered at the required witness level. Coverage rules:
   `reference/plan-and-proof.md`.

5. **Verdict** — `maestro verdict request --task <id>`. Exit-code branching,
   cost-budget, AI Reviewer protocol (Rule 1), and threat-model production:
   `reference/verdict.md`.

6. **Branch on exit code:**
   - `0` PASS — claim the task done
     (`maestro task update <id> --status completed --reason "..."`)
   - `1` FAIL — fix the cited findings, loop back to step 3
   - `2` HUMAN — run `maestro handoff create` and stop
   - `3` BLOCK — stop, surface the reason to the user

If retries are accumulating before step 5, run
`maestro task budget --task <id>` to check consumption
(`reference/verdict.md` covers cost-budget interpretation).

### Harness-delta evidence

If `maestro intake` returned `harnessImpact: true` (the change touches
`.maestro/`, `policies/`, `skills/`, or `hooks/`), record a `harness-delta`
evidence row at task close in addition to the product-side verification.
The 7-rung validation ladder and harness-specific checks are documented in
`.maestro/docs/VALIDATION_LADDER.md`.

---

## After the Verdict (when PASS or HUMAN)

| Topic | Reference |
|---|---|
| Auto-merge predicates, review acknowledgement, verdict override | `reference/auto-merge.md` |
| Deploy gate, runtime signals, cross-task conflict, witnessed rollback | `reference/deploy-and-runtime.md` |

These steps run after the Verdict computes; they decide whether the change
ships, not whether the work is done.

---

## Reference Map

The reference files are one level deep — do not link onward from inside
them. Read the file that matches your current step.

- `reference/witness-levels.md` — witness ladder, default autopilot
  thresholds, Evidence kinds added in the harness pivot.
- `reference/trust-verifier.md` — the 8 deterministic checks, severity
  semantics, architecture-lint rules, exit codes.
- `reference/plan-and-proof.md` — plan-check protocol and ProofMap verb;
  the two coverage gates that bracket implementation.
- `reference/verdict.md` — Verdict semantics, reason-field diagnostics,
  cost-budget monitoring, AI Reviewer (Rule 1 veto-only), threat-model
  production.
- `reference/auto-merge.md` — `merge auto` predicates, review
  acknowledgement, verdict override authorization.
- `reference/deploy-and-runtime.md` — deploy gate, runtime signals,
  cross-task conflict evidence, witnessed rollback (Rule 10).

---

## MCP Tools (When Available)

Verification verbs are also exposed via MCP for runtimes that prefer
structured tool calls. The MCP layer is a thin wrapper; semantics match the
CLI exactly.

| MCP tool | CLI equivalent | Notes |
|----------|----------------|-------|
| `maestro_verdict_show` | `maestro verdict show --task <id>` | Returns `code: VERDICT_NOT_FOUND` if no verdict yet |
| `maestro_verdict_request` | `maestro verdict request --task <id>` | Same `PASS / FAIL / HUMAN / BLOCK` decision tree |
| `maestro_contract_show` | `maestro contract show --task <id>` | Optional `version` argument for historical reads |
| `maestro_contract_amend` | `maestro contract amend --task <id>` | `addPaths` / `removePaths` arrays + `reason` |
| `maestro_policy_check` | `maestro policy check --task <id>` | Returns effective risk class and required witness level |
| `maestro_evidence_record` | `maestro evidence record --task <id>` | Structured input for `command` and `manual-note` only; other kinds (`ai-review`, `threat-model`, `plan-check`, `deploy-readiness`, `rollback-exercised`, `cross-task-conflict`, `runtime-signal`, `review-ack`, `verdict-override`) require the CLI |

When acting through MCP, the return shapes are JSON; do not parse them as
CLI text. CLI exit codes have no MCP analog — instead, success vs failure
is signalled by the tool result's `isError` flag and `code` string.
