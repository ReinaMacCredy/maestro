import type { Spec } from "@/features/spec/index.js";
import type { Owners } from "@/features/policy/index.js";
import type { EvidenceRow } from "@/features/evidence/index.js";

export interface DeployReadinessInput {
  readonly spec?: Spec;
  /**
   * Pre-filtered to kind=rollback-exercised at witnessed-by-ci or stronger
   * AND payload.exit === 0. A failed rollback exercise must not pass the gate.
   */
  readonly rollbackEvidence: readonly EvidenceRow[];
  readonly owners: Owners;
}

export interface DeployReadinessResult {
  readonly feature_flag: { ok: boolean; value?: string };
  readonly canary_plan: { ok: boolean; stages?: number };
  readonly rollback: { ok: boolean; witness_evidence_id?: string };
  readonly owner: { ok: boolean; approvers?: readonly string[] };
  readonly gate: "pass" | "fail";
}

export function checkDeployReadiness(input: DeployReadinessInput): DeployReadinessResult {
  // feature_flag: pass iff spec.rollout_plan.feature_flag is a non-empty string
  const flagValue = input.spec?.rollout_plan?.feature_flag;
  const feature_flag = flagValue !== undefined && flagValue.length > 0
    ? { ok: true as const, value: flagValue }
    : { ok: false as const };

  // canary_plan: pass iff spec.rollout_plan.canary.stages has >= 1 entry
  const stages = input.spec?.rollout_plan?.canary?.stages;
  const stageCount = stages?.length ?? 0;
  const canary_plan = stageCount >= 1
    ? { ok: true as const, stages: stageCount }
    : { ok: false as const };

  // rollback: pass iff at least one rollback-exercised evidence row exists
  const firstRollback = input.rollbackEvidence[0];
  const rollback = firstRollback !== undefined
    ? { ok: true as const, witness_evidence_id: firstRollback.id }
    : { ok: false as const };

  // owner: pass iff owners.deployApprovers has >= 1 entry
  const { deployApprovers } = input.owners;
  const owner = deployApprovers.length >= 1
    ? { ok: true as const, approvers: deployApprovers }
    : { ok: false as const };

  const gate: "pass" | "fail" =
    feature_flag.ok && canary_plan.ok && rollback.ok && owner.ok
      ? "pass"
      : "fail";

  return { feature_flag, canary_plan, rollback, owner, gate };
}
