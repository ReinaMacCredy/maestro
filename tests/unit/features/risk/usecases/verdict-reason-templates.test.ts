import { describe, expect, it } from "bun:test";
import * as REASONS from "@/features/risk/usecases/verdict-reason-templates.js";

describe("verdict reason templates", () => {
  it("costBudgetExhausted carries the right code and category", () => {
    const r = REASONS.costBudgetExhausted();
    expect(r.code).toBe("cost-budget-exhausted");
    expect(r.category).toBe("cost-budget");
    expect(r.message.length).toBeGreaterThan(0);
  });

  it("trustFindingsError omits findingPaths when empty", () => {
    const r = REASONS.trustFindingsError({
      errorCount: 2,
      findingChecks: ["scope", "secrets"],
      findingPaths: [],
    });
    expect(r.code).toBe("trust-findings-error");
    expect(r.findingChecks).toEqual(["scope", "secrets"]);
    expect((r as { findingPaths?: readonly string[] }).findingPaths).toBeUndefined();
  });

  it("trustFindingsError includes findingPaths when non-empty", () => {
    const r = REASONS.trustFindingsError({
      errorCount: 1,
      findingChecks: ["scope"],
      findingPaths: ["src/foo.ts"],
    });
    expect((r as { findingPaths?: readonly string[] }).findingPaths).toEqual(["src/foo.ts"]);
  });

  it("amendmentBudgetHigh interpolates count and budget", () => {
    const r = REASONS.amendmentBudgetHigh({ amendmentCount: 4, maxAmendments: 5 });
    expect(r.code).toBe("amendment-budget-high");
    expect(r.message).toContain("4");
    expect(r.message).toContain("5");
  });

  it("threatModelRequired returns the static reason", () => {
    const r = REASONS.threatModelRequired();
    expect(r.code).toBe("threat-model-required");
    expect(r.category).toBe("policy");
  });

  it("effectiveRiskCritical names both proposed and derived", () => {
    const r = REASONS.effectiveRiskCritical({
      proposedRiskClass: "medium",
      derivedRiskClass: "critical",
    });
    expect(r.code).toBe("effective-risk-critical");
    expect(r.message).toContain("medium");
    expect(r.message).toContain("critical");
  });

  it("evidenceWitnessLevelInsufficient carries evidenceIds", () => {
    const r = REASONS.evidenceWitnessLevelInsufficient({
      weakCount: 2,
      requiredLevel: "witnessed-by-ci",
      evidenceIds: ["ev_1", "ev_2"],
    });
    expect(r.code).toBe("evidence-witness-level-insufficient");
    expect((r as { evidenceIds?: readonly string[] }).evidenceIds).toEqual(["ev_1", "ev_2"]);
  });

  it("proofMapIncomplete lists uncovered ids in the message", () => {
    const r = REASONS.proofMapIncomplete({ uncoveredIds: ["ac1", "ac2"] });
    expect(r.code).toBe("proof-map-incomplete");
    expect(r.message).toContain("ac1, ac2");
  });

  it("autoMergeNotAllowed names the risk class", () => {
    const r = REASONS.autoMergeNotAllowed("high");
    expect(r.code).toBe("auto-merge-not-allowed");
    expect(r.message).toContain("high");
  });

  it("allChecksPassed is the PASS reason", () => {
    const r = REASONS.allChecksPassed();
    expect(r.code).toBe("all-checks-passed");
    expect(r.category).toBe("policy");
  });
});
