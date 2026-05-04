import { describe, it, expect } from "bun:test";
import { computeRisk } from "@/features/risk/usecases/compute-risk.js";
import type { ComputeRiskInput } from "@/features/risk/usecases/compute-risk.js";
import type { Contract, RiskClass } from "@/features/task/index.js";
import type { EvidenceRow } from "@/features/evidence/index.js";
import type { TrustFinding } from "@/features/verify/index.js";
import type { RiskPolicy, AutopilotPolicy, ReleasePolicy } from "@/features/policy/index.js";
import { CONTRACT_SCHEMA_VERSION } from "@/features/task/domain/contract/contract-types.js";

// --- Factories ---

function makeContract(overrides: Partial<Contract> = {}): Contract {
  return {
    schemaVersion: CONTRACT_SCHEMA_VERSION,
    id: "c-000001",
    taskId: "task-001",
    repoRoot: "/repo",
    status: "locked",
    createdAt: "2026-01-01T00:00:00.000Z",
    intent: "Test task",
    scope: { filesExpected: ["src/foo.ts"], filesForbidden: [] },
    doneWhen: [],
    amendments: [],
    createdBy: "agent",
    configSnapshot: {
      strict: true,
      overlapPolicy: "fail",
      rebaseFallback: "best-effort",
      staleReclaimContractPolicy: "inherit",
    },
    riskClass: "medium",
    amendmentBudget: {
      maxAmendments: 4,
      maxPathsPerAmendment: 5,
      forbiddenAmendmentPaths: [],
    },
    ...overrides,
  };
}

function makeEvidenceRow(
  overrides: Partial<EvidenceRow<"command">> = {},
): EvidenceRow<"command"> {
  return {
    schema_version: 3,
    id: `ev-${Math.random().toString(36).slice(2, 8)}`,
    task_id: "task-001",
    kind: "command",
    witness_level: "witnessed-by-maestro",
    created_at: "2026-01-01T00:00:00.000Z",
    payload: { command: "bun test", exit: 0 },
    ...overrides,
  };
}

function makeTrustFinding(overrides: Partial<TrustFinding> = {}): TrustFinding {
  return {
    check: "scope",
    severity: "info",
    paths: [],
    ...overrides,
  };
}

function makeRiskPolicy(overrides: Partial<RiskPolicy> = {}): RiskPolicy {
  return {
    kind: "risk",
    id: "risk-policy-test",
    version: "1",
    rows: [],
    ...overrides,
  };
}

function makeAutopilotPolicy(overrides: Partial<AutopilotPolicy> = {}): AutopilotPolicy {
  return {
    kind: "autopilot",
    id: "autopilot-policy-test",
    version: "1",
    autoMergeAllowed: {
      low: true,
      medium: true,
      high: false,
      critical: false,
    },
    requiredWitnessLevel: {
      low: "agent-claimed-locally",
      medium: "agent-claimed-locally",
      high: "witnessed-by-maestro",
      critical: "witnessed-by-maestro",
    },
    ...overrides,
  };
}

function makeReleasePolicy(overrides: Partial<ReleasePolicy> = {}): ReleasePolicy {
  return {
    kind: "release",
    id: "release-policy-test",
    version: "1",
    requireSignedCommits: false,
    requireProofMapComplete: false,
    ...overrides,
  };
}

function makeInput(overrides: Partial<ComputeRiskInput> = {}): ComputeRiskInput {
  return {
    contract: makeContract(),
    trustFindings: [],
    evidenceRows: [makeEvidenceRow()],
    riskPolicy: makeRiskPolicy(),
    autopilotPolicy: makeAutopilotPolicy(),
    releasePolicy: makeReleasePolicy(),
    derivedRiskClass: "medium",
    amendmentCount: 0,
    ...overrides,
  };
}

// --- Tests ---

describe("computeRisk", () => {
  describe("Verdict structure", () => {
    it("always returns a valid Verdict shape", () => {
      const verdict = computeRisk(makeInput());
      expect(verdict.schemaVersion).toBe(1);
      expect(typeof verdict.id).toBe("string");
      expect(verdict.id).toMatch(/^vrd-/);
      expect(verdict.taskId).toBe("task-001");
      expect(typeof verdict.computedAt).toBe("string");
      expect(verdict.policiesConsulted).toHaveLength(3);
    });
  });

  describe("PASS: clean diff + strong evidence + no errors + low amendment count", () => {
    it("returns PASS when all checks pass", () => {
      const verdict = computeRisk(
        makeInput({
          derivedRiskClass: "medium",
          trustFindings: [],
          evidenceRows: [makeEvidenceRow({ witness_level: "witnessed-by-maestro" })],
          amendmentCount: 0,
        }),
      );
      expect(verdict.decision).toBe("PASS");
      expect(verdict.effectiveRiskClass).toBe("medium");
      expect(verdict.proposedRiskClass).toBe("medium");
    });
  });

  describe("FAIL: trust finding with severity error", () => {
    it("returns FAIL with category trust", () => {
      const verdict = computeRisk(
        makeInput({
          trustFindings: [
            makeTrustFinding({ check: "scope", severity: "error" }),
            makeTrustFinding({ check: "secrets", severity: "error" }),
          ],
        }),
      );
      expect(verdict.decision).toBe("FAIL");
      const reason = verdict.reasons[0];
      expect(reason?.category).toBe("trust");
      expect(reason?.findingChecks).toContain("scope");
      expect(reason?.findingChecks).toContain("secrets");
    });

    it("cites all error findings in findingChecks", () => {
      const verdict = computeRisk(
        makeInput({
          trustFindings: [
            makeTrustFinding({ check: "scope", severity: "error" }),
            makeTrustFinding({ check: "generated-parity", severity: "warn" }),
          ],
        }),
      );
      expect(verdict.decision).toBe("FAIL");
      const reason = verdict.reasons[0];
      expect(reason?.findingChecks).toHaveLength(1);
      expect(reason?.findingChecks).toContain("scope");
    });
  });

  describe("HUMAN: amendment budget high (Rule 5)", () => {
    it("returns HUMAN when amendmentCount > 75% of maxAmendments", () => {
      const verdict = computeRisk(
        makeInput({
          contract: makeContract({ amendmentBudget: { maxAmendments: 5, maxPathsPerAmendment: 5, forbiddenAmendmentPaths: [] } }),
          amendmentCount: 4, // 4 > floor(5 * 0.75) = 3
        }),
      );
      expect(verdict.decision).toBe("HUMAN");
      const reason = verdict.reasons[0];
      expect(reason?.category).toBe("amendment");
      expect(reason?.code).toBe("amendment-budget-high");
    });

    it("does not trigger at or below 75%", () => {
      const verdict = computeRisk(
        makeInput({
          contract: makeContract({ amendmentBudget: { maxAmendments: 4, maxPathsPerAmendment: 5, forbiddenAmendmentPaths: [] } }),
          amendmentCount: 3, // 3 === floor(4 * 0.75) = 3 → NOT >
        }),
      );
      expect(verdict.decision).toBe("PASS");
    });
  });

  describe("BLOCK: cost budget exhausted (Rule 11)", () => {
    it("returns BLOCK with category cost-budget regardless of other inputs", () => {
      const verdict = computeRisk(
        makeInput({
          costBudgetExhausted: true,
          trustFindings: [makeTrustFinding({ severity: "error", check: "scope" })],
        }),
      );
      expect(verdict.decision).toBe("BLOCK");
      const reason = verdict.reasons[0];
      expect(reason?.category).toBe("cost-budget");
    });

    it("BLOCK takes priority over trust errors", () => {
      const verdict = computeRisk(
        makeInput({
          costBudgetExhausted: true,
          trustFindings: [makeTrustFinding({ severity: "error" })],
        }),
      );
      expect(verdict.decision).toBe("BLOCK");
    });
  });

  describe("HUMAN: effectiveRiskClass critical (Rule 12)", () => {
    it("gameability test — contract proposes low, derived is critical → HUMAN", () => {
      const verdict = computeRisk(
        makeInput({
          contract: makeContract({ riskClass: "low" }),
          derivedRiskClass: "critical",
        }),
      );
      expect(verdict.decision).toBe("HUMAN");
      expect(verdict.effectiveRiskClass).toBe("critical");
      expect(verdict.proposedRiskClass).toBe("low");
      const reason = verdict.reasons[0];
      expect(reason?.category).toBe("risk");
      expect(reason?.code).toBe("effective-risk-critical");
    });

    it("critical contract class → HUMAN", () => {
      const verdict = computeRisk(
        makeInput({
          contract: makeContract({ riskClass: "critical" }),
          derivedRiskClass: "medium",
        }),
      );
      expect(verdict.decision).toBe("HUMAN");
      expect(verdict.effectiveRiskClass).toBe("critical");
    });
  });

  describe("HUMAN: policy disallows auto-merge", () => {
    it("returns HUMAN with category policy when autoMergeAllowed is false for effective class", () => {
      const verdict = computeRisk(
        makeInput({
          autopilotPolicy: makeAutopilotPolicy({
            autoMergeAllowed: {
              low: true,
              medium: false,
              high: false,
              critical: false,
            },
          }),
          derivedRiskClass: "medium",
        }),
      );
      expect(verdict.decision).toBe("HUMAN");
      const reason = verdict.reasons[0];
      expect(reason?.category).toBe("policy");
      expect(reason?.code).toBe("auto-merge-not-allowed");
    });
  });

  describe("HUMAN: insufficient evidence witness level for high risk", () => {
    it("returns HUMAN when evidence is below required witness level for high risk", () => {
      const verdict = computeRisk(
        makeInput({
          contract: makeContract({ riskClass: "high" }),
          derivedRiskClass: "high",
          autopilotPolicy: makeAutopilotPolicy({
            autoMergeAllowed: { low: true, medium: true, high: true, critical: false },
            requiredWitnessLevel: {
              low: "agent-claimed-locally",
              medium: "agent-claimed-locally",
              high: "witnessed-by-maestro",
              critical: "witnessed-by-maestro",
            },
          }),
          evidenceRows: [makeEvidenceRow({ witness_level: "agent-claimed-locally" })],
        }),
      );
      expect(verdict.decision).toBe("HUMAN");
      const reason = verdict.reasons[0];
      expect(reason?.category).toBe("evidence");
      expect(reason?.code).toBe("evidence-witness-level-insufficient");
    });
  });

  describe("effectiveRiskClass = max(contract.riskClass, derived)", () => {
    it("takes derived when derived > contract", () => {
      const verdict = computeRisk(
        makeInput({
          contract: makeContract({ riskClass: "low" }),
          derivedRiskClass: "high",
          autopilotPolicy: makeAutopilotPolicy({
            autoMergeAllowed: { low: true, medium: true, high: false, critical: false },
          }),
        }),
      );
      expect(verdict.effectiveRiskClass).toBe("high");
      expect(verdict.decision).toBe("HUMAN");
    });

    it("takes contract when contract > derived", () => {
      const verdict = computeRisk(
        makeInput({
          contract: makeContract({ riskClass: "high" }),
          derivedRiskClass: "low",
          autopilotPolicy: makeAutopilotPolicy({
            autoMergeAllowed: { low: true, medium: true, high: false, critical: false },
          }),
        }),
      );
      expect(verdict.effectiveRiskClass).toBe("high");
    });
  });

  describe("trustVerifier counts", () => {
    it("counts errors, warns, infos correctly", () => {
      const verdict = computeRisk(
        makeInput({
          trustFindings: [
            makeTrustFinding({ severity: "warn" }),
            makeTrustFinding({ severity: "info" }),
            makeTrustFinding({ severity: "info" }),
          ],
        }),
      );
      expect(verdict.trustVerifier.errors).toBe(0);
      expect(verdict.trustVerifier.warns).toBe(1);
      expect(verdict.trustVerifier.infos).toBe(2);
      expect(verdict.trustVerifier.findingsCount).toBe(3);
    });
  });

  describe("evidenceConsulted", () => {
    it("contains all evidence row IDs", () => {
      const row1 = makeEvidenceRow();
      const row2 = makeEvidenceRow();
      const verdict = computeRisk(makeInput({ evidenceRows: [row1, row2] }));
      expect(verdict.evidenceConsulted).toContain(row1.id);
      expect(verdict.evidenceConsulted).toContain(row2.id);
    });
  });
});
