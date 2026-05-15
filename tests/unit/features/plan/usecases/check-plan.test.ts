import { describe, expect, it } from "bun:test";
import { checkPlan } from "@/features/plan/usecases/check-plan.js";
import type { PlanInput } from "@/features/plan/domain/types.js";
import type { Contract } from "@/features/task/index.js";
import type { Spec } from "@/shared/domain/legacy-spec/index.js";
import { CONTRACT_SCHEMA_VERSION } from "@/features/task/domain/contract/contract-types.js";

function makeContract(filesExpected: string[] = []): Contract {
  return {
    schemaVersion: CONTRACT_SCHEMA_VERSION,
    id: "c-000001",
    taskId: "tsk-aaaaaa",
    repoRoot: "/repo",
    status: "locked",
    createdAt: "2026-01-01T00:00:00.000Z",
    intent: "Test task",
    scope: { filesExpected, filesForbidden: [] },
    doneWhen: [],
    amendments: [],
    createdBy: "agent",
    configSnapshot: {
      strict: true,
      overlapPolicy: "fail",
      rebaseFallback: "best-effort",
      staleReclaimContractPolicy: "inherit",
    },
  };
}

function makeSpec(criterionIds: string[]): Spec {
  return {
    schema_version: 2,
    mission_id: "mission-001",
    acceptance_criteria: criterionIds.map((id) => ({ id, text: `Criterion ${id}` })),
    non_goals: [],
    runtime_signals: [],
    created_at: "2026-01-01T00:00:00.000Z",
    updated_at: "2026-01-01T00:00:00.000Z",
  };
}

function makePlan(overrides: Partial<PlanInput> = {}): PlanInput {
  return {
    intendedFiles: ["src/foo.ts"],
    proofSet: [],
    riskClass: "low",
    ...overrides,
  };
}

describe("checkPlan", () => {
  it("returns 0 findings when plan is fully within scope, all criteria covered, and risk class matches derived", () => {
    const plan = makePlan({
      intendedFiles: ["src/foo.ts"],
      proofSet: [{ criterionId: "c-001", evidenceKinds: ["command"] }],
      riskClass: "low",
    });
    const contract = makeContract(["src/foo.ts"]);
    const spec = makeSpec(["c-001"]);

    const result = checkPlan({ plan, contract, spec, derivedRiskClass: "low" });

    expect(result.findings).toHaveLength(0);
    expect(result.errorCount).toBe(0);
    expect(result.warnCount).toBe(0);
  });

  it("scope-widens: reports error for files outside contract.scope.filesExpected", () => {
    const plan = makePlan({
      intendedFiles: ["src/foo.ts", "src/secret.ts"],
      riskClass: "low",
    });
    const contract = makeContract(["src/foo.ts"]);

    const result = checkPlan({ plan, contract, derivedRiskClass: "low" });

    expect(result.findings).toHaveLength(1);
    const finding = result.findings[0]!;
    expect(finding.check).toBe("scope-widens");
    expect(finding.severity).toBe("error");
    expect(finding.paths).toEqual(["src/secret.ts"]);
    expect(result.errorCount).toBe(1);
  });

  it("scope check is skipped when filesExpected is empty (no constraint)", () => {
    const plan = makePlan({ intendedFiles: ["src/anything.ts"], riskClass: "low" });
    const contract = makeContract([]);

    const result = checkPlan({ plan, contract, derivedRiskClass: "low" });

    expect(result.findings.some((f) => f.check === "scope-widens")).toBe(false);
  });

  it("missing-proof: reports error for acceptance criteria not covered in proofSet", () => {
    const plan = makePlan({
      intendedFiles: ["src/foo.ts"],
      proofSet: [],
      riskClass: "low",
    });
    const contract = makeContract(["src/foo.ts"]);
    const spec = makeSpec(["c-001"]);

    const result = checkPlan({ plan, contract, spec, derivedRiskClass: "low" });

    expect(result.findings).toHaveLength(1);
    const finding = result.findings[0]!;
    expect(finding.check).toBe("missing-proof");
    expect(finding.severity).toBe("error");
    expect(finding.criterionIds).toEqual(["c-001"]);
    expect(result.errorCount).toBe(1);
  });

  it("risk-class-too-low: reports error when plan.riskClass is below derived class", () => {
    const plan = makePlan({
      intendedFiles: ["src/auth/foo.ts"],
      riskClass: "low",
    });
    const contract = makeContract();

    const result = checkPlan({ plan, contract, derivedRiskClass: "critical" });

    expect(result.findings).toHaveLength(1);
    const finding = result.findings[0]!;
    expect(finding.check).toBe("risk-class-too-low");
    expect(finding.severity).toBe("error");
    expect(finding.message).toContain("low");
    expect(finding.message).toContain("critical");
    expect(result.errorCount).toBe(1);
  });

  it("no missing-proof finding when spec is absent (solo task)", () => {
    const plan = makePlan({
      intendedFiles: ["src/foo.ts"],
      proofSet: [],
      riskClass: "low",
    });
    const contract = makeContract(["src/foo.ts"]);

    const result = checkPlan({ plan, contract, derivedRiskClass: "low" });

    expect(result.findings.some((f) => f.check === "missing-proof")).toBe(false);
    expect(result.errorCount).toBe(0);
  });

  it("risk-class-too-low not raised when plan.riskClass equals derived class", () => {
    const plan = makePlan({ riskClass: "medium" });
    const contract = makeContract();

    const result = checkPlan({ plan, contract, derivedRiskClass: "medium" });

    expect(result.findings.some((f) => f.check === "risk-class-too-low")).toBe(false);
  });

  it("risk-class-too-low not raised when plan.riskClass is higher than derived", () => {
    const plan = makePlan({ riskClass: "high" });
    const contract = makeContract();

    const result = checkPlan({ plan, contract, derivedRiskClass: "medium" });

    expect(result.findings.some((f) => f.check === "risk-class-too-low")).toBe(false);
  });

  it("multiple findings are accumulated", () => {
    const plan = makePlan({
      intendedFiles: ["src/foo.ts", "src/secret.ts"],
      proofSet: [],
      riskClass: "low",
    });
    const contract = makeContract(["src/foo.ts"]);
    const spec = makeSpec(["c-001"]);

    const result = checkPlan({ plan, contract, spec, derivedRiskClass: "critical" });

    expect(result.findings).toHaveLength(3);
    expect(result.errorCount).toBe(3);
  });
});
