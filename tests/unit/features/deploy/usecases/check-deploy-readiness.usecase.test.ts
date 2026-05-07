import { describe, expect, it } from "bun:test";
import { checkDeployReadiness } from "@/features/deploy/usecases/check-deploy-readiness.usecase.js";
import type { DeployReadinessInput } from "@/features/deploy/usecases/check-deploy-readiness.usecase.js";
import type { Spec } from "@/features/spec/index.js";
import type { Owners } from "@/features/policy/index.js";
import type { EvidenceRow } from "@/features/evidence/index.js";

const SPEC_ALL_PASS: Spec = {
  schema_version: 2,
  mission_id: "msn-aaaaaa",
  acceptance_criteria: [],
  non_goals: [],
  runtime_signals: [],
  rollout_plan: {
    feature_flag: "enable-new-deploy",
    canary: {
      stages: [
        { percent: 10, hold_minutes: 30 },
        { percent: 100, hold_minutes: 0 },
      ],
    },
  },
  created_at: "2026-05-05T00:00:00.000Z",
  updated_at: "2026-05-05T00:00:00.000Z",
};

const OWNERS_WITH_APPROVERS: Owners = {
  policyApprovers: [],
  ratchetApprovers: [],
  sensitiveWaivers: [],
  deployApprovers: ["alice", "bob"],
};

const OWNERS_EMPTY: Owners = {
  policyApprovers: [],
  ratchetApprovers: [],
  sensitiveWaivers: [],
  deployApprovers: [],
};

function makeRollbackRow(id: string): EvidenceRow {
  return {
    schema_version: 3,
    id,
    task_id: "tsk-aaaaaa",
    kind: "rollback-exercised",
    witness_level: "witnessed-by-ci",
    created_at: "2026-05-05T08:00:00.000Z",
    payload: { command: "kubectl rollout undo", exit: 0 },
  };
}

describe("checkDeployReadiness", () => {
  it("all four checks pass → gate=pass", () => {
    const input: DeployReadinessInput = {
      spec: SPEC_ALL_PASS,
      rollbackEvidence: [makeRollbackRow("evd-rollback01")],
      owners: OWNERS_WITH_APPROVERS,
    };
    const result = checkDeployReadiness(input);
    expect(result.gate).toBe("pass");
    expect(result.feature_flag.ok).toBe(true);
    expect(result.canary_plan.ok).toBe(true);
    expect(result.rollback.ok).toBe(true);
    expect(result.owner.ok).toBe(true);
  });

  it("feature_flag missing → gate=fail, only feature_flag.ok=false", () => {
    const spec: Spec = {
      ...SPEC_ALL_PASS,
      rollout_plan: {
        ...SPEC_ALL_PASS.rollout_plan,
        feature_flag: undefined,
      },
    };
    const result = checkDeployReadiness({
      spec,
      rollbackEvidence: [makeRollbackRow("evd-rollback01")],
      owners: OWNERS_WITH_APPROVERS,
    });
    expect(result.gate).toBe("fail");
    expect(result.feature_flag.ok).toBe(false);
    expect(result.canary_plan.ok).toBe(true);
    expect(result.rollback.ok).toBe(true);
    expect(result.owner.ok).toBe(true);
  });

  it("feature_flag empty string → gate=fail", () => {
    const spec: Spec = {
      ...SPEC_ALL_PASS,
      rollout_plan: { ...SPEC_ALL_PASS.rollout_plan, feature_flag: "" },
    };
    const result = checkDeployReadiness({
      spec,
      rollbackEvidence: [makeRollbackRow("evd-rollback01")],
      owners: OWNERS_WITH_APPROVERS,
    });
    expect(result.gate).toBe("fail");
    expect(result.feature_flag.ok).toBe(false);
  });

  it("canary stages empty → gate=fail, only canary_plan.ok=false", () => {
    const spec: Spec = {
      ...SPEC_ALL_PASS,
      rollout_plan: {
        ...SPEC_ALL_PASS.rollout_plan,
        canary: { stages: [] },
      },
    };
    const result = checkDeployReadiness({
      spec,
      rollbackEvidence: [makeRollbackRow("evd-rollback01")],
      owners: OWNERS_WITH_APPROVERS,
    });
    expect(result.gate).toBe("fail");
    expect(result.feature_flag.ok).toBe(true);
    expect(result.canary_plan.ok).toBe(false);
    expect(result.rollback.ok).toBe(true);
    expect(result.owner.ok).toBe(true);
  });

  it("no rollback evidence → gate=fail, only rollback.ok=false", () => {
    const result = checkDeployReadiness({
      spec: SPEC_ALL_PASS,
      rollbackEvidence: [],
      owners: OWNERS_WITH_APPROVERS,
    });
    expect(result.gate).toBe("fail");
    expect(result.feature_flag.ok).toBe(true);
    expect(result.canary_plan.ok).toBe(true);
    expect(result.rollback.ok).toBe(false);
    expect(result.owner.ok).toBe(true);
  });

  it("empty deployApprovers → gate=fail, only owner.ok=false", () => {
    const result = checkDeployReadiness({
      spec: SPEC_ALL_PASS,
      rollbackEvidence: [makeRollbackRow("evd-rollback01")],
      owners: OWNERS_EMPTY,
    });
    expect(result.gate).toBe("fail");
    expect(result.feature_flag.ok).toBe(true);
    expect(result.canary_plan.ok).toBe(true);
    expect(result.rollback.ok).toBe(true);
    expect(result.owner.ok).toBe(false);
  });

  it("no spec → all spec-derived checks fail, owner check independent", () => {
    const result = checkDeployReadiness({
      spec: undefined,
      rollbackEvidence: [makeRollbackRow("evd-rollback01")],
      owners: OWNERS_WITH_APPROVERS,
    });
    expect(result.gate).toBe("fail");
    expect(result.feature_flag.ok).toBe(false);
    expect(result.canary_plan.ok).toBe(false);
    expect(result.rollback.ok).toBe(true);
    expect(result.owner.ok).toBe(true);
  });

  it("witness_evidence_id is the first rollback row's id", () => {
    const first = makeRollbackRow("evd-first");
    const second = makeRollbackRow("evd-second");
    const result = checkDeployReadiness({
      spec: SPEC_ALL_PASS,
      rollbackEvidence: [first, second],
      owners: OWNERS_WITH_APPROVERS,
    });
    expect(result.rollback.ok).toBe(true);
    expect((result.rollback as { witness_evidence_id: string }).witness_evidence_id).toBe("evd-first");
  });

  it("feature_flag value is surfaced in result", () => {
    const result = checkDeployReadiness({
      spec: SPEC_ALL_PASS,
      rollbackEvidence: [makeRollbackRow("evd-rollback01")],
      owners: OWNERS_WITH_APPROVERS,
    });
    expect((result.feature_flag as { value: string }).value).toBe("enable-new-deploy");
  });

  it("canary stages count is surfaced in result", () => {
    const result = checkDeployReadiness({
      spec: SPEC_ALL_PASS,
      rollbackEvidence: [makeRollbackRow("evd-rollback01")],
      owners: OWNERS_WITH_APPROVERS,
    });
    expect((result.canary_plan as { stages: number }).stages).toBe(2);
  });

  it("approvers list is surfaced in owner result", () => {
    const result = checkDeployReadiness({
      spec: SPEC_ALL_PASS,
      rollbackEvidence: [makeRollbackRow("evd-rollback01")],
      owners: OWNERS_WITH_APPROVERS,
    });
    expect((result.owner as { approvers: readonly string[] }).approvers).toEqual(["alice", "bob"]);
  });
});
