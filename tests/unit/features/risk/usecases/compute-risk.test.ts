import { describe, it, expect } from "bun:test";
import { computeRisk, applyAIReviewerRiskRaise, applyCrossTaskConflictRiskRaise, requiresThreatModel, hasThreatModelEvidence } from "@/features/risk/usecases/compute-risk.js";
import type { ComputeRiskInput } from "@/features/risk/usecases/compute-risk.js";
import type { Contract, RiskClass } from "@/features/task/index.js";
import type { EvidenceRow, ThreatModelPayload } from "@/features/evidence/index.js";
import type { AIReviewPayload } from "@/features/evidence/index.js";
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

// --- L4.3: AI Reviewer Risk Raise (Rule 1: veto-only) ---

function makeAIReviewRow(
  reviewer: AIReviewPayload["reviewer"],
  findings: AIReviewPayload["findings"],
  overrides: Partial<EvidenceRow<"ai-review">> = {},
): EvidenceRow<"ai-review"> {
  return {
    schema_version: 3,
    id: `ev-${Math.random().toString(36).slice(2, 8)}`,
    task_id: "task-001",
    kind: "ai-review",
    witness_level: "agent-claimed-locally",
    created_at: "2026-01-01T00:00:00.000Z",
    payload: { reviewer, findings, confidence: 0.9 },
    ...overrides,
  };
}

describe("applyAIReviewerRiskRaise", () => {
  it("security reviewer with error finding → critical (from medium)", () => {
    const rows = [makeAIReviewRow("security", [{ severity: "error", message: "injection" }])];
    expect(applyAIReviewerRiskRaise("medium", rows)).toBe("critical");
  });

  it("bug reviewer with error finding on medium → high (one notch)", () => {
    const rows = [makeAIReviewRow("bug", [{ severity: "error", message: "null deref" }])];
    expect(applyAIReviewerRiskRaise("medium", rows)).toBe("high");
  });

  it("bug reviewer with error finding on low → medium (one notch)", () => {
    const rows = [makeAIReviewRow("bug", [{ severity: "error", message: "null deref" }])];
    expect(applyAIReviewerRiskRaise("low", rows)).toBe("medium");
  });

  it("bug reviewer with error finding on high → critical (one notch)", () => {
    const rows = [makeAIReviewRow("bug", [{ severity: "error", message: "null deref" }])];
    expect(applyAIReviewerRiskRaise("high", rows)).toBe("critical");
  });

  it("bug reviewer with error finding on critical → critical (saturates)", () => {
    const rows = [makeAIReviewRow("bug", [{ severity: "error", message: "null deref" }])];
    expect(applyAIReviewerRiskRaise("critical", rows)).toBe("critical");
  });

  it("clean ai-review (zero error findings) on medium → medium (no lowering)", () => {
    const rows = [makeAIReviewRow("security", [{ severity: "info", message: "looks good" }])];
    expect(applyAIReviewerRiskRaise("medium", rows)).toBe("medium");
  });

  it("clean ai-review with only warns → no raise", () => {
    const rows = [makeAIReviewRow("bug", [{ severity: "warn", message: "minor concern" }])];
    expect(applyAIReviewerRiskRaise("low", rows)).toBe("low");
  });

  it("architecture reviewer with error finding on medium → high (one notch)", () => {
    const rows = [makeAIReviewRow("architecture", [{ severity: "error", message: "circular dep" }])];
    expect(applyAIReviewerRiskRaise("medium", rows)).toBe("high");
  });

  it("multiple reviews: 1 security clean + 1 bug error → bug raises one notch, security does not lower", () => {
    const rows = [
      makeAIReviewRow("security", [{ severity: "info", message: "looks good" }]),
      makeAIReviewRow("bug", [{ severity: "error", message: "null deref" }]),
    ];
    // medium + bug error → high; security clean has no effect
    expect(applyAIReviewerRiskRaise("medium", rows)).toBe("high");
  });

  it("no ai-review rows → unchanged", () => {
    const rows: EvidenceRow[] = [makeEvidenceRow()];
    expect(applyAIReviewerRiskRaise("medium", rows)).toBe("medium");
  });
});

describe("computeRisk: ai-review integration (Rule 1)", () => {
  it("security reviewer error on medium baseline → effectiveRiskClass critical, HUMAN verdict", () => {
    const verdict = computeRisk(
      makeInput({
        contract: makeContract({ riskClass: "medium" }),
        derivedRiskClass: "medium",
        evidenceRows: [
          makeEvidenceRow({ witness_level: "witnessed-by-maestro" }),
          makeAIReviewRow("security", [{ severity: "error", message: "injection" }]),
        ],
      }),
    );
    expect(verdict.effectiveRiskClass).toBe("critical");
    expect(verdict.decision).toBe("HUMAN");
  });

  it("bug reviewer error on medium baseline → effectiveRiskClass high", () => {
    const verdict = computeRisk(
      makeInput({
        contract: makeContract({ riskClass: "medium" }),
        derivedRiskClass: "medium",
        autopilotPolicy: makeAutopilotPolicy({
          autoMergeAllowed: { low: true, medium: true, high: false, critical: false },
        }),
        evidenceRows: [
          makeEvidenceRow({ witness_level: "witnessed-by-maestro" }),
          makeAIReviewRow("bug", [{ severity: "error", message: "null deref" }]),
        ],
      }),
    );
    expect(verdict.effectiveRiskClass).toBe("high");
    expect(verdict.decision).toBe("HUMAN");
  });

  it("clean ai-review on medium baseline → effectiveRiskClass still medium", () => {
    const verdict = computeRisk(
      makeInput({
        contract: makeContract({ riskClass: "medium" }),
        derivedRiskClass: "medium",
        evidenceRows: [
          makeEvidenceRow({ witness_level: "witnessed-by-maestro" }),
          makeAIReviewRow("security", [{ severity: "info", message: "all good" }]),
        ],
      }),
    );
    expect(verdict.effectiveRiskClass).toBe("medium");
    expect(verdict.decision).toBe("PASS");
  });
});

// --- L4.3a: threat-model Evidence predicates ---

function makeThreatModelRow(
  payload: Partial<ThreatModelPayload> = {},
  overrides: Partial<EvidenceRow<"threat-model">> = {},
): EvidenceRow<"threat-model"> {
  return {
    schema_version: 3,
    id: `ev-${Math.random().toString(36).slice(2, 8)}`,
    task_id: "task-001",
    kind: "threat-model",
    witness_level: "agent-claimed-locally",
    created_at: "2026-01-01T00:00:00.000Z",
    payload: {
      assets: [],
      threatCategories: [],
      mitigations: [],
      residualRisk: "low",
      ...payload,
    },
    ...overrides,
  };
}

describe("requiresThreatModel", () => {
  it("returns true when derived class is critical AND signal is diff-intersects-sensitive-security", () => {
    expect(requiresThreatModel("critical", "diff-intersects-sensitive-security")).toBe(true);
  });

  it("returns false when derived class is critical but signal is not security-related", () => {
    expect(requiresThreatModel("critical", "diff-modifies-dependency-manifests")).toBe(false);
    expect(requiresThreatModel("critical", "diff-modifies-ci-workflows")).toBe(false);
    expect(requiresThreatModel("critical", "diff-source-only")).toBe(false);
  });

  it("returns false when signal matches but class is not critical", () => {
    expect(requiresThreatModel("high", "diff-intersects-sensitive-security")).toBe(false);
    expect(requiresThreatModel("medium", "diff-intersects-sensitive-security")).toBe(false);
    expect(requiresThreatModel("low", "diff-intersects-sensitive-security")).toBe(false);
  });

  it("returns false when signal is undefined", () => {
    expect(requiresThreatModel("critical", undefined)).toBe(false);
  });
});

describe("hasThreatModelEvidence", () => {
  it("returns true when any row has kind threat-model", () => {
    const rows: EvidenceRow[] = [makeEvidenceRow(), makeThreatModelRow()];
    expect(hasThreatModelEvidence(rows)).toBe(true);
  });

  it("returns true even for empty-content threat-model row (schema-valid presence)", () => {
    const rows: EvidenceRow[] = [makeThreatModelRow({ assets: [], threatCategories: [], mitigations: [] })];
    expect(hasThreatModelEvidence(rows)).toBe(true);
  });

  it("returns false when no threat-model rows exist", () => {
    const rows: EvidenceRow[] = [makeEvidenceRow()];
    expect(hasThreatModelEvidence(rows)).toBe(false);
  });

  it("returns false for empty evidence list", () => {
    expect(hasThreatModelEvidence([])).toBe(false);
  });
});

describe("computeRisk: threat-model-required gate (Edge Case 12)", () => {
  it("critical diff on security-sensitive paths + no threat-model evidence → HUMAN with threat-model-required reason", () => {
    const verdict = computeRisk(
      makeInput({
        contract: makeContract({ riskClass: "medium" }),
        derivedRiskClass: "critical",
        matchedRiskPolicySignal: "diff-intersects-sensitive-security",
        evidenceRows: [makeEvidenceRow({ witness_level: "witnessed-by-maestro" })],
      }),
    );
    expect(verdict.decision).toBe("HUMAN");
    const threatModelReason = verdict.reasons.find((r) => r.code === "threat-model-required");
    expect(threatModelReason).toBeDefined();
    expect(threatModelReason?.category).toBe("policy");
  });

  it("critical diff on security-sensitive paths + schema-valid empty threat-model evidence → no threat-model-required reason", () => {
    const verdict = computeRisk(
      makeInput({
        contract: makeContract({ riskClass: "medium" }),
        derivedRiskClass: "critical",
        matchedRiskPolicySignal: "diff-intersects-sensitive-security",
        evidenceRows: [
          makeEvidenceRow({ witness_level: "witnessed-by-maestro" }),
          makeThreatModelRow(),
        ],
      }),
    );
    // Verdict may still be HUMAN (critical always is), but the threat-model-required reason must be absent.
    expect(verdict.decision).toBe("HUMAN");
    const threatModelReason = verdict.reasons.find((r) => r.code === "threat-model-required");
    expect(threatModelReason).toBeUndefined();
  });

  it("critical diff but non-security signal (e.g. dependency manifests) + no threat-model → no threat-model-required reason", () => {
    const verdict = computeRisk(
      makeInput({
        contract: makeContract({ riskClass: "medium" }),
        derivedRiskClass: "critical",
        matchedRiskPolicySignal: "diff-modifies-dependency-manifests",
        evidenceRows: [makeEvidenceRow({ witness_level: "witnessed-by-maestro" })],
      }),
    );
    expect(verdict.decision).toBe("HUMAN");
    const threatModelReason = verdict.reasons.find((r) => r.code === "threat-model-required");
    expect(threatModelReason).toBeUndefined();
  });

  it("medium risk diff touching src/foo.ts → no threat-model rule applies; threat-model evidence is harmless", () => {
    const verdict = computeRisk(
      makeInput({
        contract: makeContract({ riskClass: "medium" }),
        derivedRiskClass: "medium",
        matchedRiskPolicySignal: "diff-source-only",
        evidenceRows: [
          makeEvidenceRow({ witness_level: "witnessed-by-maestro" }),
          makeThreatModelRow(),
        ],
      }),
    );
    expect(verdict.decision).toBe("PASS");
    const threatModelReason = verdict.reasons.find((r) => r.code === "threat-model-required");
    expect(threatModelReason).toBeUndefined();
  });
});

// ─── applyCrossTaskConflictRiskRaise ─────────────────────────────────────────

function makeCrossTaskConflictRow(conflictingPrs: number[]): EvidenceRow {
  return {
    schema_version: 3,
    id: `ev-ctc-${Math.random().toString(36).slice(2, 8)}`,
    task_id: "task-001",
    kind: "cross-task-conflict" as EvidenceRow["kind"],
    witness_level: "witnessed-by-ci",
    created_at: "2026-01-01T00:00:00.000Z",
    payload: {
      thisPr: 42,
      conflictingPrs,
      overlappingPaths: ["src/shared.ts"],
    } as unknown as EvidenceRow["payload"],
  };
}

describe("applyCrossTaskConflictRiskRaise", () => {
  it("raises low to medium when one conflict row exists", () => {
    const rows = [makeCrossTaskConflictRow([7])];
    expect(applyCrossTaskConflictRiskRaise("low", rows)).toBe("medium");
  });

  it("raises medium to high when one conflict row exists", () => {
    const rows = [makeCrossTaskConflictRow([7])];
    expect(applyCrossTaskConflictRiskRaise("medium", rows)).toBe("high");
  });

  it("raises high to critical when one conflict row exists", () => {
    const rows = [makeCrossTaskConflictRow([7])];
    expect(applyCrossTaskConflictRiskRaise("high", rows)).toBe("critical");
  });

  it("stays critical when already at critical (ceiling)", () => {
    const rows = [makeCrossTaskConflictRow([7])];
    expect(applyCrossTaskConflictRiskRaise("critical", rows)).toBe("critical");
  });

  it("raises by ONE tier total even when multiple conflict rows exist", () => {
    // Multiple rows should not multiply the raise — one tier total
    const rows = [makeCrossTaskConflictRow([7]), makeCrossTaskConflictRow([8])];
    expect(applyCrossTaskConflictRiskRaise("low", rows)).toBe("medium");
  });

  it("does not raise when no cross-task-conflict rows exist", () => {
    const rows: EvidenceRow[] = [makeEvidenceRow({ kind: "command" })];
    expect(applyCrossTaskConflictRiskRaise("medium", rows)).toBe("medium");
  });

  it("does not raise when conflict row has empty conflictingPrs", () => {
    const rows = [makeCrossTaskConflictRow([])];
    expect(applyCrossTaskConflictRiskRaise("low", rows)).toBe("low");
  });
});
