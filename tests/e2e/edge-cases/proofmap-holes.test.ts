/**
 * Edge Case 3 (ProofMap holes): when a verdict goes non-PASS for any reason
 * (trust errors, auto-merge disallowed, etc.), `computeRisk` must also
 * append a `proof-map-incomplete` reason if any acceptance criterion lacks
 * covering evidence. Otherwise agents fixate on the visible failure and ship
 * with silent coverage gaps.
 */
import { describe, it, expect } from "bun:test";
import { computeRisk } from "@/features/risk/usecases/compute-risk.js";
import type { ComputeRiskInput } from "@/features/risk/usecases/compute-risk.js";
import type { Contract } from "@/features/task/index.js";
import { CONTRACT_SCHEMA_VERSION } from "@/features/task/domain/contract/contract-types.js";
import type { EvidenceRow } from "@/features/evidence/index.js";
import type { TrustFinding } from "@/features/verify/index.js";

function makeContract(overrides: Partial<Contract> = {}): Contract {
  return {
    schemaVersion: CONTRACT_SCHEMA_VERSION,
    id: "c-pm-1",
    taskId: "task-pm-001",
    repoRoot: "/repo",
    status: "locked",
    createdAt: "2026-01-01T00:00:00.000Z",
    intent: "Test",
    scope: { filesExpected: [], filesForbidden: [] },
    doneWhen: [
      { id: "ac-1", text: "Acceptance criterion 1" },
      { id: "ac-2", text: "Acceptance criterion 2" },
    ],
    amendments: [],
    createdBy: "agent",
    configSnapshot: { strict: true, overlapPolicy: "fail", rebaseFallback: "best-effort", staleReclaimContractPolicy: "inherit" },
    riskClass: "medium",
    amendmentBudget: { maxAmendments: 4, maxPathsPerAmendment: 5, forbiddenAmendmentPaths: [] },
    ...overrides,
  };
}

function makeInput(overrides: Partial<ComputeRiskInput> = {}): ComputeRiskInput {
  return {
    contract: makeContract(),
    trustFindings: [],
    evidenceRows: [],
    riskPolicy: { kind: "risk", id: "rp", version: "1", rows: [] },
    autopilotPolicy: {
      kind: "autopilot",
      id: "ap",
      version: "1",
      autoMergeAllowed: { low: true, medium: true, high: false, critical: false },
      requiredWitnessLevel: { low: "agent-claimed-locally", medium: "agent-claimed-locally", high: "witnessed-by-maestro", critical: "witnessed-by-maestro" },
    },
    releasePolicy: { kind: "release", id: "rp", version: "1", requireSignedCommits: false, requireProofMapComplete: false },
    derivedRiskClass: "medium",
    amendmentCount: 0,
    ...overrides,
  };
}

describe("Edge Case 3: ProofMap holes appended as diagnostic", () => {
  it("FAIL on trust errors also lists uncovered criteria", () => {
    const verdict = computeRisk(makeInput({
      trustFindings: [{ check: "scope", severity: "error", paths: ["src/foo.ts"] }],
    }));
    expect(verdict.decision).toBe("FAIL");
    const codes = verdict.reasons.map((r) => r.code);
    expect(codes).toContain("trust-findings-error");
    expect(codes).toContain("proof-map-incomplete");
    const pm = verdict.reasons.find((r) => r.code === "proof-map-incomplete");
    expect(pm?.message).toContain("ac-1");
    expect(pm?.message).toContain("ac-2");
  });

  it("HUMAN on autoMergeNotAllowed also lists uncovered criteria", () => {
    const verdict = computeRisk(makeInput({
      derivedRiskClass: "high",
      autopilotPolicy: {
        kind: "autopilot",
        id: "ap",
        version: "1",
        autoMergeAllowed: { low: false, medium: false, high: false, critical: false },
        requiredWitnessLevel: { low: "agent-claimed-locally", medium: "agent-claimed-locally", high: "agent-claimed-locally", critical: "witnessed-by-maestro" },
      },
    }));
    expect(verdict.decision).toBe("HUMAN");
    const codes = verdict.reasons.map((r) => r.code);
    expect(codes).toContain("auto-merge-not-allowed");
    expect(codes).toContain("proof-map-incomplete");
  });

  it("does not duplicate proof-map-incomplete when release policy already emitted it", () => {
    const verdict = computeRisk(makeInput({
      releasePolicy: { kind: "release", id: "rp", version: "1", requireSignedCommits: false, requireProofMapComplete: true },
      autopilotPolicy: {
        kind: "autopilot",
        id: "ap",
        version: "1",
        autoMergeAllowed: { low: false, medium: false, high: false, critical: false },
        requiredWitnessLevel: { low: "agent-claimed-locally", medium: "agent-claimed-locally", high: "agent-claimed-locally", critical: "witnessed-by-maestro" },
      },
    }));
    const codes = verdict.reasons.map((r) => r.code);
    expect(codes.filter((c) => c === "proof-map-incomplete")).toHaveLength(1);
  });

  it("omits proof-map-incomplete when all criteria are covered", () => {
    const covering: EvidenceRow[] = [
      {
        schema_version: 3,
        id: "ev-1",
        task_id: "task-pm-001",
        kind: "command",
        witness_level: "witnessed-by-maestro",
        created_at: "2026-01-01T00:00:00.000Z",
        payload: { command: "bun test", exit: 0, criterion_id: "ac-1" } as any,
      } as any,
      {
        schema_version: 3,
        id: "ev-2",
        task_id: "task-pm-001",
        kind: "command",
        witness_level: "witnessed-by-maestro",
        created_at: "2026-01-01T00:00:00.000Z",
        payload: { command: "bun test", exit: 0, criterion_id: "ac-2" } as any,
      } as any,
    ];
    const verdict = computeRisk(makeInput({
      trustFindings: [{ check: "scope", severity: "error", paths: ["src/foo.ts"] } as TrustFinding],
      evidenceRows: covering,
    }));
    expect(verdict.decision).toBe("FAIL");
    const codes = verdict.reasons.map((r) => r.code);
    expect(codes).not.toContain("proof-map-incomplete");
  });
});
