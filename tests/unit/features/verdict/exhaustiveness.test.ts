import { describe, it, expect } from "bun:test";
import { exitCodeForDecision } from "@/features/verdict/presentation.js";
import type { VerdictDecision } from "@/features/verdict/domain/types.js";

describe("VerdictDecision exhaustiveness", () => {
  it("handles all VerdictDecision cases", () => {
    // This test verifies that exitCodeForDecision handles all cases
    const decisions: VerdictDecision[] = ["PASS", "FAIL", "HUMAN", "BLOCK"];
    
    expect(exitCodeForDecision("PASS")).toBe(0);
    expect(exitCodeForDecision("FAIL")).toBe(1);
    expect(exitCodeForDecision("HUMAN")).toBe(2);
    expect(exitCodeForDecision("BLOCK")).toBe(3);
    
    // If a new decision is added to VerdictDecision union,
    // TypeScript will error at compile time because the switch
    // statement with assertNever ensures exhaustiveness
    for (const decision of decisions) {
      expect(typeof exitCodeForDecision(decision)).toBe("number");
    }
  });
});
