import { describe, expect, it } from "bun:test";
import { EXEC_PLAN_ID_PATTERN, generateExecPlanId, isExecPlanId } from "@/v2/types/exec-plan.js";

describe("ExecPlan id helpers", () => {
  it("generateExecPlanId produces ids matching the pln-<ts>-<rand> shape", () => {
    const id = generateExecPlanId();
    expect(id.startsWith("pln-")).toBe(true);
    expect(EXEC_PLAN_ID_PATTERN.test(id)).toBe(true);
  });

  it("isExecPlanId accepts generated ids", () => {
    for (let i = 0; i < 8; i++) {
      expect(isExecPlanId(generateExecPlanId())).toBe(true);
    }
  });

  it("isExecPlanId rejects task-shaped or arbitrary strings", () => {
    expect(isExecPlanId("tsk-abc-def")).toBe(false);
    expect(isExecPlanId("pln-")).toBe(false);
    expect(isExecPlanId("pln-abc")).toBe(false);
    expect(isExecPlanId("")).toBe(false);
    expect(isExecPlanId(undefined)).toBe(false);
    expect(isExecPlanId(42)).toBe(false);
  });

  it("generated ids are unique under back-to-back calls", () => {
    const ids = new Set<string>();
    for (let i = 0; i < 16; i++) ids.add(generateExecPlanId());
    expect(ids.size).toBeGreaterThan(1);
  });
});
