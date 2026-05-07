import { describe, expect, it } from "bun:test";
import type { AutopilotPolicy, ReleasePolicy, RiskPolicy } from "@/features/policy/index.js";

describe("policy-types", () => {
  it("accepts a well-formed RiskPolicy literal", () => {
    const p: RiskPolicy = {
      id: "risk-policy-1",
      description: "Maps signals to risk classes",
      kind: "risk",
      rows: [
        { signal: "touches-sensitive-path", derivedClass: "high" },
        { signal: "touches-lockfile", derivedClass: "medium", description: "lockfile change" },
      ],
      version: "1.0.0",
    };
    expect(p.kind).toBe("risk");
    expect(p.rows).toHaveLength(2);
  });

  it("accepts a well-formed AutopilotPolicy literal", () => {
    const p: AutopilotPolicy = {
      id: "autopilot-policy-1",
      kind: "autopilot",
      autoMergeAllowed: {
        low: true,
        medium: false,
        high: false,
        critical: false,
      },
      requiredWitnessLevel: {
        low: "agent-claimed-locally",
        medium: "witnessed-by-maestro",
        high: "witnessed-by-ci",
        critical: "witnessed-by-ci",
      },
      version: "1.0.0",
    };
    expect(p.kind).toBe("autopilot");
    expect(p.autoMergeAllowed.low).toBe(true);
    expect(p.requiredWitnessLevel.critical).toBe("witnessed-by-ci");
  });

  it("accepts a well-formed ReleasePolicy literal", () => {
    const p: ReleasePolicy = {
      id: "release-policy-1",
      kind: "release",
      requireSignedCommits: true,
      requireProofMapComplete: false,
      version: "2.0.0",
    };
    expect(p.kind).toBe("release");
    expect(p.requireSignedCommits).toBe(true);
  });

  it("narrows kind discriminant correctly for RiskPolicy", () => {
    type AnyPolicy = RiskPolicy | AutopilotPolicy | ReleasePolicy;
    const p: AnyPolicy = {
      id: "x",
      kind: "risk",
      rows: [],
      version: "1",
    };
    if (p.kind === "risk") {
      // rows is only accessible on RiskPolicy
      expect(p.rows).toBeDefined();
    } else {
      // unreachable in this test — just confirms type narrowing compiles
      expect(true).toBe(false);
    }
  });

  it("narrows kind discriminant correctly for AutopilotPolicy", () => {
    type AnyPolicy = RiskPolicy | AutopilotPolicy | ReleasePolicy;
    const p: AnyPolicy = {
      id: "x",
      kind: "autopilot",
      autoMergeAllowed: { low: false, medium: false, high: false, critical: false },
      requiredWitnessLevel: {
        low: "agent-claimed-locally",
        medium: "agent-claimed-locally",
        high: "witnessed-by-maestro",
        critical: "witnessed-by-ci",
      },
      version: "1",
    };
    if (p.kind === "autopilot") {
      expect(p.autoMergeAllowed).toBeDefined();
    } else {
      expect(true).toBe(false);
    }
  });

  it("narrows kind discriminant correctly for ReleasePolicy", () => {
    type AnyPolicy = RiskPolicy | AutopilotPolicy | ReleasePolicy;
    const p: AnyPolicy = {
      id: "x",
      kind: "release",
      requireSignedCommits: false,
      requireProofMapComplete: true,
      version: "1",
    };
    if (p.kind === "release") {
      expect(p.requireProofMapComplete).toBe(true);
    } else {
      expect(true).toBe(false);
    }
  });
});
