/**
 * Edge Case 4 (cost-budget BLOCK message): the BLOCK reason must name the
 * specific limit that was exceeded so agents know which knob to look at
 * before escalating.
 */
import { describe, it, expect } from "bun:test";
import { costBudgetExhausted } from "@/features/risk/usecases/verdict-reason-templates.js";

describe("Edge Case 4: cost-budget BLOCK reason names the exhausted limit", () => {
  it("includes maxRetries when reason=max-retries", () => {
    const r = costBudgetExhausted("max-retries");
    expect(r.code).toBe("cost-budget-exhausted");
    expect(r.message).toContain("costBudget.maxRetries");
    expect(r.message).toContain("max-retries");
    expect(r.findingChecks).toEqual(["max-retries"]);
  });

  it("includes maxWallClockSeconds when reason=max-wall-clock", () => {
    const r = costBudgetExhausted("max-wall-clock");
    expect(r.message).toContain("costBudget.maxWallClockSeconds");
    expect(r.findingChecks).toEqual(["max-wall-clock"]);
  });

  it("includes maxTokens when reason=max-tokens", () => {
    const r = costBudgetExhausted("max-tokens");
    expect(r.message).toContain("costBudget.maxTokens");
    expect(r.findingChecks).toEqual(["max-tokens"]);
  });

  it("recovery hint always names the next verbs", () => {
    const r = costBudgetExhausted("max-retries");
    expect(r.message).toContain("maestro task budget");
    expect(r.message).toContain("maestro contract amend");
    expect(r.message).toContain("maestro handoff create");
  });

  it("falls back to a generic message when reason is undefined", () => {
    const r = costBudgetExhausted();
    expect(r.code).toBe("cost-budget-exhausted");
    expect(r.message).toContain("Cost budget exhausted");
    expect(r.findingChecks).toBeUndefined();
  });
});
