import { describe, it, expect } from "bun:test";
import { validatePrinciple, validateCreatePrincipleInput } from "@/features/principle/domain/validators.js";
import { DEFAULT_PRINCIPLES } from "@/features/principle/domain/default-principles.js";

describe("validatePrinciple", () => {
  it("accepts valid advisory principle", () => {
    const result = validatePrinciple({
      id: "test-advisory",
      name: "Test Advisory",
      source: "custom",
      rule: "Do the thing",
      profiles: ["implementation"],
      mode: "advisory",
    });
    expect(result.id).toBe("test-advisory");
    expect(result.mode).toBe("advisory");
  });

  it("accepts valid gate principle with gateField and gateCheck", () => {
    const result = validatePrinciple({
      id: "test-gate",
      name: "Test Gate",
      source: "karpathy",
      rule: "Must provide assumptions",
      profiles: ["planning", "implementation"],
      mode: "gate",
      gateField: "assumptions",
      gateCheck: "array_min_length:1",
    });
    expect(result.mode).toBe("gate");
    expect(result.gateField).toBe("assumptions");
    expect(result.gateCheck).toBe("array_min_length:1");
  });

  it("rejects gate principle without gateField", () => {
    expect(() => validatePrinciple({
      id: "bad-gate",
      name: "Bad Gate",
      source: "custom",
      rule: "Missing gate field",
      profiles: ["implementation"],
      mode: "gate",
      gateCheck: "object_non_empty",
    })).toThrow();
  });

  it("rejects gate principle without gateCheck", () => {
    expect(() => validatePrinciple({
      id: "bad-gate-2",
      name: "Bad Gate 2",
      source: "custom",
      rule: "Missing gate check",
      profiles: ["implementation"],
      mode: "gate",
      gateField: "assumptions",
    })).toThrow();
  });

  it("rejects invalid id format", () => {
    expect(() => validatePrinciple({
      id: "Invalid_ID",
      name: "Bad",
      source: "custom",
      rule: "Rule",
      profiles: ["implementation"],
      mode: "advisory",
    })).toThrow();
  });

  it("rejects empty profiles", () => {
    expect(() => validatePrinciple({
      id: "no-profiles",
      name: "No Profiles",
      source: "custom",
      rule: "Rule",
      profiles: [],
      mode: "advisory",
    })).toThrow();
  });

  it("rejects invalid profile values", () => {
    expect(() => validatePrinciple({
      id: "bad-profile",
      name: "Bad Profile",
      source: "custom",
      rule: "Rule",
      profiles: ["nonexistent-profile"],
      mode: "advisory",
    })).toThrow();
  });

  it("rejects extra fields (strict mode)", () => {
    expect(() => validatePrinciple({
      id: "strict-test",
      name: "Strict",
      source: "custom",
      rule: "Rule",
      profiles: ["implementation"],
      mode: "advisory",
      extraField: "should fail",
    })).toThrow();
  });
});

describe("validateCreatePrincipleInput", () => {
  it("defaults source to custom when omitted", () => {
    const result = validateCreatePrincipleInput({
      id: "new-principle",
      name: "New Principle",
      rule: "A rule",
      profiles: ["implementation"],
      mode: "advisory",
    });
    expect(result.source).toBe("custom");
  });

  it("accepts profile strings (validated later by adapter)", () => {
    const result = validateCreatePrincipleInput({
      id: "test",
      name: "Test",
      rule: "Rule",
      profiles: ["implementation", "planning"],
      mode: "advisory",
    });
    expect(result.profiles).toEqual(["implementation", "planning"]);
  });

  it("rejects gate mode without gate-field and gate-check", () => {
    expect(() => validateCreatePrincipleInput({
      id: "gate-no-field",
      name: "Gate No Field",
      rule: "Rule",
      profiles: ["implementation"],
      mode: "gate",
    })).toThrow();
  });
});

describe("DEFAULT_PRINCIPLES", () => {
  it("has exactly 4 principles", () => {
    expect(DEFAULT_PRINCIPLES).toHaveLength(4);
  });

  it("all default principles pass validation", () => {
    for (const principle of DEFAULT_PRINCIPLES) {
      expect(() => validatePrinciple(principle)).not.toThrow();
    }
  });

  it("contains the expected principle ids", () => {
    const ids = DEFAULT_PRINCIPLES.map((p) => p.id);
    expect(ids).toEqual([
      "think-before-coding",
      "simplicity-first",
      "surgical-changes",
      "goal-driven-execution",
    ]);
  });

  it("all karpathy-sourced", () => {
    expect(DEFAULT_PRINCIPLES.every((p) => p.source === "karpathy")).toBe(true);
  });

  it("gate principles have gateField and gateCheck", () => {
    const gates = DEFAULT_PRINCIPLES.filter((p) => p.mode === "gate");
    expect(gates).toHaveLength(3);
    for (const g of gates) {
      expect(g.gateField).toBeDefined();
      expect(g.gateCheck).toBeDefined();
    }
  });
});
