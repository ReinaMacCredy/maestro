import { describe, expect, it } from "bun:test";
import { validatePlanInput } from "@/features/plan/domain/plan-validators.js";
import { MaestroError } from "@/shared/errors.js";

describe("validatePlanInput", () => {
  it("returns the parsed plan when the shape is valid", () => {
    const plan = validatePlanInput(
      {
        intendedFiles: ["src/foo.ts", "src/bar.ts"],
        proofSet: [{ criterionId: "c1", evidenceKinds: ["command"] }],
        riskClass: "medium",
        notes: "hello",
      },
      "/tmp/plan.yaml",
    );
    expect(plan.intendedFiles).toEqual(["src/foo.ts", "src/bar.ts"]);
    expect(plan.proofSet).toHaveLength(1);
    expect(plan.riskClass).toBe("medium");
    expect(plan.notes).toBe("hello");
  });

  it("accepts a minimal plan with empty proofSet", () => {
    const plan = validatePlanInput(
      { intendedFiles: ["src/foo.ts"], proofSet: [], riskClass: "low" },
      "/tmp/p.yaml",
    );
    expect(plan.proofSet).toEqual([]);
    expect(plan.notes).toBeUndefined();
  });

  it("throws MaestroError naming missing intendedFiles", () => {
    const fn = () =>
      validatePlanInput({ proofSet: [], riskClass: "medium" }, "/tmp/p.yaml");
    expect(fn).toThrow(MaestroError);
    try {
      fn();
    } catch (err) {
      const e = err as MaestroError;
      expect(e.message).toContain("/tmp/p.yaml");
      expect(e.hints.some((h) => h.includes("intendedFiles"))).toBe(true);
    }
  });

  it("enumerates every missing required field at once", () => {
    const fn = () => validatePlanInput({ notes: "x" }, "/tmp/p.yaml");
    try {
      fn();
    } catch (err) {
      const hints = (err as MaestroError).hints.join("\n");
      expect(hints).toContain("intendedFiles");
      expect(hints).toContain("proofSet");
      expect(hints).toContain("riskClass");
    }
  });

  it("rejects an unknown riskClass with the valid set in the message", () => {
    const fn = () =>
      validatePlanInput(
        { intendedFiles: ["src/foo.ts"], proofSet: [], riskClass: "extreme" },
        "/tmp/p.yaml",
      );
    try {
      fn();
    } catch (err) {
      const e = err as MaestroError;
      expect(e.hints.join("\n")).toMatch(/riskClass.*extreme/);
      expect(e.hints.join("\n")).toContain("low|medium|high|critical");
    }
  });

  it("rejects a non-array intendedFiles", () => {
    expect(() =>
      validatePlanInput(
        { intendedFiles: "src/foo.ts", proofSet: [], riskClass: "low" },
        "/tmp/p.yaml",
      ),
    ).toThrow(MaestroError);
  });

  it("includes a plan-file shape hint after the issues", () => {
    try {
      validatePlanInput({}, "/tmp/p.yaml");
    } catch (err) {
      const hints = (err as MaestroError).hints;
      expect(hints.some((h) => h.includes("Plan file shape"))).toBe(true);
    }
  });

  it("rejects a totally non-object payload (string at root)", () => {
    expect(() => validatePlanInput("nope", "/tmp/p.yaml")).toThrow(MaestroError);
  });
});
