import { describe, expect, it } from "bun:test";
import type {
  AmendmentBudget,
  Contract,
  ContractConfigSnapshot,
  ContractScope,
  CostBudget,
  RiskClass,
} from "@/features/task/domain/contract/contract-types.js";
import { CONTRACT_SCHEMA_VERSION } from "@/features/task/domain/contract/contract-types.js";

const baseScope: ContractScope = {
  filesExpected: ["src/foo.ts"],
  filesForbidden: [],
};

const baseConfig: ContractConfigSnapshot = {
  strict: false,
  overlapPolicy: "annotate",
  rebaseFallback: "best-effort",
  staleReclaimContractPolicy: "inherit",
};

describe("CONTRACT_SCHEMA_VERSION", () => {
  it("is 2", () => {
    expect(CONTRACT_SCHEMA_VERSION).toBe(2);
  });
});

describe("Contract type compatibility", () => {
  it("accepts a v1-shaped contract literal (no v2 optional fields)", () => {
    const v1: Contract = {
      schemaVersion: 1,
      id: "c-aabbcc",
      taskId: "tsk-aabbcc",
      repoRoot: ".",
      status: "draft",
      createdAt: new Date().toISOString(),
      intent: "implement login",
      scope: baseScope,
      doneWhen: [],
      amendments: [],
      createdBy: "agent-1",
      configSnapshot: baseConfig,
    };
    // The v1 contract has no missionId, riskClass, amendmentBudget, costBudget.
    expect(v1.schemaVersion).toBe(1);
    expect(v1.missionId).toBeUndefined();
    expect(v1.riskClass).toBeUndefined();
    expect(v1.amendmentBudget).toBeUndefined();
    expect(v1.costBudget).toBeUndefined();
  });

  it("accepts a v2-shaped contract literal with all optional fields populated", () => {
    const budget: AmendmentBudget = {
      maxAmendments: 3,
      maxPathsPerAmendment: 10,
      forbiddenAmendmentPaths: ["dist/"],
    };
    const cost: CostBudget = {
      maxRetries: 2,
      maxWallClockSeconds: 300,
      maxTokens: 50000,
    };
    const v2: Contract = {
      schemaVersion: 2,
      id: "c-ddeeff",
      taskId: "tsk-ddeeff",
      repoRoot: ".",
      status: "locked",
      createdAt: new Date().toISOString(),
      intent: "implement auth middleware",
      scope: baseScope,
      doneWhen: [],
      amendments: [],
      createdBy: "agent-2",
      configSnapshot: baseConfig,
      missionId: "msn-001",
      riskClass: "medium",
      amendmentBudget: budget,
      costBudget: cost,
    };
    expect(v2.schemaVersion).toBe(2);
    expect(v2.missionId).toBe("msn-001");
    expect(v2.riskClass).toBe("medium");
    expect(v2.amendmentBudget?.maxAmendments).toBe(3);
    expect(v2.costBudget?.maxTokens).toBe(50000);
  });
});

describe("RiskClass", () => {
  it("accepts all valid risk class values", () => {
    const classes: RiskClass[] = ["low", "medium", "high", "critical"];
    expect(classes).toHaveLength(4);
  });

  it("excludes invalid values at the type level", () => {
    // @ts-expect-error "extreme" is not a valid RiskClass
    const bad: RiskClass = "extreme";
    // The runtime value exists but the type check above is the real assertion.
    expect(typeof bad).toBe("string");
  });
});
