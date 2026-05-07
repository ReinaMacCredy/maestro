import { describe, expect, it } from "bun:test";
import { classifyIntake } from "@/features/intake/index.js";
import { DEFAULT_RISK_POLICY } from "@/features/policy/index.js";

const SENSITIVE_PATHS: readonly string[] = [".maestro/policies/**"];

describe("classifyIntake", () => {
  it("returns tiny for a single docs-only path with no flags", () => {
    const r = classifyIntake(
      { summary: "fix typo", intendedPaths: ["README.md"] },
      DEFAULT_RISK_POLICY,
      SENSITIVE_PATHS,
    );
    expect(r.lane).toBe("tiny");
    expect(r.autoDetectedFlags).toEqual([]);
    expect(r.hardGatesTriggered).toEqual([]);
    expect(r.recommendedNextStep).toContain("patch directly");
  });

  it("auto-detects auth from the path and triggers a hard gate", () => {
    const r = classifyIntake(
      { summary: "rotate session secret", intendedPaths: ["src/auth/session.ts"] },
      DEFAULT_RISK_POLICY,
      SENSITIVE_PATHS,
    );
    expect(r.autoDetectedFlags).toContain("auth");
    expect(r.hardGatesTriggered).toContain("auth");
    expect(r.lane).toBe("high-risk");
    expect(r.recommendedNextStep).toContain("threat-model");
  });

  it("auto-detects audit-security via the configured sensitive paths", () => {
    const r = classifyIntake(
      { summary: "tweak risk policy", intendedPaths: [".maestro/policies/risk.yaml"] },
      DEFAULT_RISK_POLICY,
      SENSITIVE_PATHS,
    );
    expect(r.autoDetectedFlags).toContain("audit-security");
    expect(r.hardGatesTriggered).toContain("audit-security");
    expect(r.lane).toBe("high-risk");
  });

  it("auto-detects external-systems on dependency manifest changes", () => {
    const r = classifyIntake(
      { summary: "bump zod", intendedPaths: ["package.json", "bun.lock"] },
      DEFAULT_RISK_POLICY,
      SENSITIVE_PATHS,
    );
    expect(r.autoDetectedFlags).toContain("external-systems");
    expect(r.lane).toBe("high-risk");
  });

  it("returns normal for two declared non-gate flags", () => {
    const r = classifyIntake(
      {
        summary: "rename foo",
        intendedPaths: ["src/foo.ts"],
        declaredFlags: ["existing-behavior", "weak-proof"],
      },
      DEFAULT_RISK_POLICY,
      SENSITIVE_PATHS,
    );
    expect(r.declaredFlags).toEqual(["existing-behavior", "weak-proof"]);
    expect(r.hardGatesTriggered).toEqual([]);
    expect(r.lane).toBe("normal");
    expect(r.recommendedNextStep).toContain("task plan");
  });

  it("escalates to high-risk when 4+ flags accumulate without a hard gate", () => {
    const r = classifyIntake(
      {
        summary: "broad refactor",
        intendedPaths: ["src/foo.ts"],
        declaredFlags: ["existing-behavior", "weak-proof", "multi-domain", "public-contracts"],
      },
      DEFAULT_RISK_POLICY,
      SENSITIVE_PATHS,
    );
    expect(r.hardGatesTriggered).toEqual([]);
    expect(r.lane).toBe("high-risk");
  });

  it("dedupes when the same flag is both auto-detected and declared", () => {
    const r = classifyIntake(
      {
        summary: "cookie domain",
        intendedPaths: ["src/auth/cookie.ts"],
        declaredFlags: ["auth"],
      },
      DEFAULT_RISK_POLICY,
      SENSITIVE_PATHS,
    );
    expect(r.autoDetectedFlags).toEqual(["auth"]);
    expect(r.declaredFlags).toEqual(["auth"]);
    expect(r.hardGatesTriggered).toEqual(["auth"]);
    expect(r.lane).toBe("high-risk");
  });

  it("derives risk class from the diff via the policy", () => {
    const r = classifyIntake(
      { summary: "edit docs", intendedPaths: ["docs/foo.md"] },
      DEFAULT_RISK_POLICY,
      SENSITIVE_PATHS,
    );
    expect(r.derivedRiskClass).toBeDefined();
    expect(r.derivedRiskSignal).toBeDefined();
  });
});
