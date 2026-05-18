---
name: maestro-verify
description: The canonical verification protocol for any task in a maestro project. Documents witness levels, Trust Verifier scope, ProofMap, plan-check, verdict semantics, cost-budget monitoring, AI Reviewer protocol (Rule 1 veto-only), and threat-model production. Cross-referenced by `maestro-task` and `maestro-mission`. Read this skill when starting a non-trivial task or before declaring completion.
---

# Maestro Verify

This skill is the canonical verification protocol every agent follows before claiming a task is done. Read it when planning a non-trivial task or before declaring completion.

Cross-referenced by `maestro-task` (pre-ship ritual), `maestro-mission` (plan-check). When those skills say "see maestro-verify", this is the document.

The trigger body below names the steps and points to one reference file per substantive topic. Open the matching reference when you reach that step in the pre-ship ritual.

---

## The pre-ship ritual

Run this loop before marking any task done. Steps are ordered; do not skip.

**Cold resume.** When picking up an unfamiliar session, run `maestro doctor` first (or `./init.sh`, which calls doctor then status). A failing scaffold or stale verdict dimension means the harness itself is broken; fix that before attempting verification. The doctor pass is not a substitute for the steps below. It confirms the floor.

1. **Plan** — write a plan file and run `maestro plan check --task <id> --plan-file <path>`. Address `scope-widens`, `missing-proof`, and `risk-class-too-low` before writing code. Plan file schema: `reference/plan-and-proof.md`.

2. **Implement** — write code. Record evidence after each verification command. Witness levels and evidence kinds: `reference/witness-levels.md`.

   ```bash
   maestro evidence record --task <id> --command "bun test" --exit 0
   ```

3. **Verify (arch-lint)** — `maestro task verify <id>`. Runs the architecture-lint corpus only. Address every `error` finding. Architecture-lint rules: `reference/trust-verifier.md`.

   To record an explicit human verdict instead of running lints, pass `--verdict {human,block} --reason <text>`:

   - `--verdict human` keeps the task at `verifying` (awaiting review), exit code `2`.
   - `--verdict block` transitions the task to `blocked` with `block_reason`, exit code `3`.

4. **Verdict** — `maestro verdict request --task <id>`. Runs the full decision tree: Trust Verifier (8 deterministic checks) + ProofMap coverage + autopilot policy + cost-budget. ProofMap diagnostics surface inside the verdict's `reasons[]` (see `reference/plan-and-proof.md`). Exit-code branching, cost-budget, AI Reviewer protocol (Rule 1), threat-model production: `reference/verdict.md`.

5. **Branch on exit code** (the routing matches `task verify` and `verdict request`):

   - `0` PASS — task is at `ready`. Run `maestro task ship <id>` (or the alias `maestro ship <id>`) to close.
   - `1` FAIL — fix the cited findings, loop back to step 3.
   - `2` HUMAN — task stays at `verifying` with the reason recorded. Surface to the user; do not retry without guidance.
   - `3` BLOCK — task is now `blocked` with `block_reason`. Surface the reason; do not retry.

If retries are accumulating before step 4, the verdict envelope's `costBudgetExhausted` / `costBudgetReason` fields report the budget state directly (see `reference/verdict.md` for cost-budget interpretation). When the budget is exhausted, the next `verdict request` returns BLOCK.

### Harness-delta evidence

If the change touches `.maestro/`, `policies/`, `skills/`, or `hooks/`, record a `harness-delta` evidence row at task close in addition to the product-side verification. The 7-rung validation ladder and harness-specific checks: `.maestro/docs/VALIDATION_LADDER.md`.

---

## After the verdict (when PASS or HUMAN)

| Topic                                                              | Reference                       |
| ------------------------------------------------------------------ | ------------------------------- |
| Auto-merge predicates, review acknowledgement, verdict override    | `reference/auto-merge.md`       |
| Deploy gate, runtime signals, cross-task conflict, rollback witness| `reference/deploy-and-runtime.md` |

These steps run after the verdict computes; they decide whether the change ships, not whether the work is done.

`maestro task observe` is the dev-time inspection counterpart (one-shot PromQL or last-N log lines). It does **not** gate any verdict — `runtime check` is what samples a Spec's `runtime_signals` for pre-ship evidence. Use `task observe` when you want to look at metrics or logs while debugging, and reach for `runtime check` only when the Spec declares signals that must be witnessed before shipping. See `maestro-task` for the `task observe` flag surface.

---

## Reference map

The reference files are one level deep — do not link onward from inside them. Read the file that matches your current step.

- `reference/witness-levels.md` — witness ladder, default autopilot thresholds, evidence kinds.
- `reference/trust-verifier.md` — the 8 deterministic checks, severity semantics, architecture-lint rules, exit codes.
- `reference/plan-and-proof.md` — plan-check protocol and ProofMap verb; the two coverage gates that bracket implementation.
- `reference/verdict.md` — verdict semantics, reason-field diagnostics, cost-budget monitoring, AI Reviewer (Rule 1 veto-only), threat-model production.
- `reference/auto-merge.md` — `merge auto` predicates, review acknowledgement, verdict override authorization.
- `reference/deploy-and-runtime.md` — deploy gate, runtime signals, cross-task conflict evidence, rollback witness (Rule 10).

---

## Hand off cleanly

The next phase after this skill depends on the verdict:

- PASS → return to `maestro-task` step 6 and run `maestro task ship <id>` (or alias `maestro ship <id>`).
- FAIL → stay in the implement → verify loop in `maestro-task`. Do not hand off — fix the cited findings and re-run.
- HUMAN → surface to the user with the recorded reason; do not retry without guidance.
- BLOCK → the next agent enters via `maestro-handoff` and reads the `task:block` envelope. Surface `block_reason` and stop.

Pass a computed verdict envelope with evidence attached — not a partial check. Do not invoke implementation, spec authoring, or planning from this skill.

---

## MCP tools (when available)

Verification verbs are also exposed via MCP for runtimes that prefer structured tool calls. The MCP layer is a thin wrapper; semantics match the CLI exactly.

| MCP tool                  | CLI equivalent                       | Notes                                                                  |
| ------------------------- | ------------------------------------ | ---------------------------------------------------------------------- |
| `maestro_verdict_show`    | `maestro verdict show --task <id>`   | Returns `code: VERDICT_NOT_FOUND` when no verdict has been computed.   |
| `maestro_verdict_request` | `maestro verdict request --task <id>`| Same `PASS / FAIL / HUMAN / BLOCK` decision tree.                      |
| `maestro_contract_show`   | `maestro contract show --task <id>`  | Optional `version` argument for historical reads.                      |
| `maestro_contract_amend`  | `maestro contract amend --task <id>` | `addPaths` / `removePaths` arrays + `reason`.                          |
| `maestro_policy_check`    | `maestro policy check --task <id>`   | Returns effective risk class and required witness level.               |
| `maestro_evidence_record` | `maestro evidence record --task <id>`| Structured input for `command` and `manual-note` only; richer kinds (`ai-review`, `threat-model`, `plan-check`, `deploy-readiness`, `rollback-exercised`, `cross-task-conflict`, `runtime-signal`, `review-ack`, `verdict-override`) require the CLI. |

When acting through MCP, the return shapes are JSON; do not parse them as CLI text. CLI exit codes have no MCP analog — success vs failure is signalled by the tool result's `isError` flag and `code` string.
