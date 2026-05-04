import { describe, expect, it } from "bun:test";
import type { Verdict, VerdictDecision } from "@/features/verdict/index.js";

describe("Verdict types", () => {
  it("accepts a fully-populated Verdict literal", () => {
    const v: Verdict = {
      schemaVersion: 1,
      id: "vrd-1714747200123-a1b2c3",
      taskId: "tsk-1714747200123-a1b2c3",
      contractVersion: 2,
      computedAt: "2026-05-04T00:00:00.000Z",
      decision: "PASS",
      proposedRiskClass: "low",
      effectiveRiskClass: "medium",
      reasons: [
        {
          category: "trust",
          code: "SCOPE_OK",
          message: "All files in scope",
          evidenceIds: ["evd-1714747200123-a1b2c3"],
          findingChecks: ["check-scope"],
          policyRuleIds: ["policy-1"],
        },
      ],
      evidenceConsulted: ["evd-1714747200123-a1b2c3"],
      policiesConsulted: [{ file: ".maestro/policies/owners.yaml", version: "1.0.0" }],
      trustVerifier: {
        findingsCount: 1,
        errors: 0,
        warns: 1,
        infos: 0,
      },
    };
    expect(v.schemaVersion).toBe(1);
    expect(v.decision).toBe("PASS");
  });

  it("accepts all four VerdictDecision values", () => {
    const decisions: VerdictDecision[] = ["PASS", "FAIL", "HUMAN", "BLOCK"];
    expect(decisions).toHaveLength(4);
  });

  it("rejects invalid VerdictDecision at type level", () => {
    // @ts-expect-error "UNKNOWN" is not a valid VerdictDecision
    const bad: VerdictDecision = "UNKNOWN";
    void bad;
  });

  it("accepts Verdict with minimal optional fields omitted", () => {
    const v: Verdict = {
      schemaVersion: 1,
      id: "vrd-1714747200123-ffffff",
      taskId: "tsk-1714747200123-ffffff",
      contractVersion: 1,
      computedAt: "2026-05-04T00:00:00.000Z",
      decision: "FAIL",
      effectiveRiskClass: "high",
      reasons: [],
      evidenceConsulted: [],
      policiesConsulted: [],
      trustVerifier: {
        findingsCount: 0,
        errors: 1,
        warns: 0,
        infos: 0,
      },
    };
    expect(v.proposedRiskClass).toBeUndefined();
  });
});
