import { describe, it, expect } from "bun:test";
import { classifyPolicyEdit } from "@/features/policy/usecases/classify-policy-edit.usecase.js";

// --- risk.yaml ---

describe("classifyPolicyEdit: risk", () => {
  it("tightening: row added", () => {
    const oldYaml = `
rows:
  - signal: changes-lockfile
    derived_class: medium
`.trim();
    const newYaml = `
rows:
  - signal: changes-lockfile
    derived_class: medium
  - signal: touches-secrets
    derived_class: critical
`.trim();
    const result = classifyPolicyEdit({ oldYaml, newYaml, kind: "risk" });
    expect(result.tightenings).toHaveLength(1);
    expect(result.tightenings[0].description).toContain("touches-secrets");
    expect(result.loosenings).toHaveLength(0);
  });

  it("loosening: row removed", () => {
    const oldYaml = `
rows:
  - signal: changes-lockfile
    derived_class: medium
  - signal: touches-secrets
    derived_class: critical
`.trim();
    const newYaml = `
rows:
  - signal: changes-lockfile
    derived_class: medium
`.trim();
    const result = classifyPolicyEdit({ oldYaml, newYaml, kind: "risk" });
    expect(result.loosenings).toHaveLength(1);
    expect(result.loosenings[0].description).toContain("touches-secrets");
    expect(result.tightenings).toHaveLength(0);
  });

  it("tightening: derived_class raised", () => {
    const oldYaml = `rows:\n  - signal: changes-lockfile\n    derived_class: low`;
    const newYaml = `rows:\n  - signal: changes-lockfile\n    derived_class: high`;
    const result = classifyPolicyEdit({ oldYaml, newYaml, kind: "risk" });
    expect(result.tightenings).toHaveLength(1);
    expect(result.tightenings[0].description).toMatch(/raised/);
    expect(result.loosenings).toHaveLength(0);
  });

  it("loosening: derived_class lowered", () => {
    const oldYaml = `rows:\n  - signal: changes-lockfile\n    derived_class: critical`;
    const newYaml = `rows:\n  - signal: changes-lockfile\n    derived_class: low`;
    const result = classifyPolicyEdit({ oldYaml, newYaml, kind: "risk" });
    expect(result.loosenings).toHaveLength(1);
    expect(result.loosenings[0].description).toMatch(/lowered/);
    expect(result.tightenings).toHaveLength(0);
  });

  it("no changes: returns empty arrays", () => {
    const yaml = `rows:\n  - signal: changes-lockfile\n    derived_class: medium`;
    const result = classifyPolicyEdit({ oldYaml: yaml, newYaml: yaml, kind: "risk" });
    expect(result.tightenings).toHaveLength(0);
    expect(result.loosenings).toHaveLength(0);
  });

  it("fresh file created: all values are tightenings", () => {
    const result = classifyPolicyEdit({
      oldYaml: "",
      newYaml: `rows:\n  - signal: changes-lockfile\n    derived_class: medium`,
      kind: "risk",
    });
    expect(result.tightenings).toHaveLength(1);
    expect(result.loosenings).toHaveLength(0);
  });

  it("file removed: all values become loosenings", () => {
    const result = classifyPolicyEdit({
      oldYaml: `rows:\n  - signal: changes-lockfile\n    derived_class: medium`,
      newYaml: "",
      kind: "risk",
    });
    expect(result.tightenings).toHaveLength(0);
    expect(result.loosenings).toHaveLength(1);
  });
});

// --- autopilot.yaml ---

describe("classifyPolicyEdit: autopilot", () => {
  it("tightening: autoMergeAllowed true→false", () => {
    const oldYaml = `auto_merge_allowed:\n  low: true\n  medium: false\n  high: false\n  critical: false`;
    const newYaml = `auto_merge_allowed:\n  low: false\n  medium: false\n  high: false\n  critical: false`;
    const result = classifyPolicyEdit({ oldYaml, newYaml, kind: "autopilot" });
    expect(result.tightenings).toHaveLength(1);
    expect(result.tightenings[0].description).toContain("auto_merge_allowed.low");
    expect(result.loosenings).toHaveLength(0);
  });

  it("loosening: autoMergeAllowed false→true", () => {
    const oldYaml = `auto_merge_allowed:\n  low: false\n  medium: false\n  high: false\n  critical: false`;
    const newYaml = `auto_merge_allowed:\n  low: true\n  medium: false\n  high: false\n  critical: false`;
    const result = classifyPolicyEdit({ oldYaml, newYaml, kind: "autopilot" });
    expect(result.loosenings).toHaveLength(1);
    expect(result.loosenings[0].description).toContain("auto_merge_allowed.low");
    expect(result.tightenings).toHaveLength(0);
  });

  it("tightening: requiredWitnessLevel raised", () => {
    const oldYaml = `required_witness_level:\n  high: agent-claimed-locally`;
    const newYaml = `required_witness_level:\n  high: witnessed-by-maestro`;
    const result = classifyPolicyEdit({ oldYaml, newYaml, kind: "autopilot" });
    expect(result.tightenings).toHaveLength(1);
    expect(result.tightenings[0].description).toMatch(/raised/);
    expect(result.loosenings).toHaveLength(0);
  });

  it("loosening: requiredWitnessLevel lowered", () => {
    const oldYaml = `required_witness_level:\n  high: witnessed-by-maestro`;
    const newYaml = `required_witness_level:\n  high: agent-claimed-locally`;
    const result = classifyPolicyEdit({ oldYaml, newYaml, kind: "autopilot" });
    expect(result.loosenings).toHaveLength(1);
    expect(result.loosenings[0].description).toMatch(/lowered/);
    expect(result.tightenings).toHaveLength(0);
  });
});

// --- release.yaml ---

describe("classifyPolicyEdit: release", () => {
  it("tightening: requireSignedCommits false→true", () => {
    const oldYaml = `require_signed_commits: false`;
    const newYaml = `require_signed_commits: true`;
    const result = classifyPolicyEdit({ oldYaml, newYaml, kind: "release" });
    expect(result.tightenings).toHaveLength(1);
    expect(result.tightenings[0].description).toContain("require_signed_commits");
    expect(result.loosenings).toHaveLength(0);
  });

  it("loosening: requireSignedCommits true→false", () => {
    const oldYaml = `require_signed_commits: true`;
    const newYaml = `require_signed_commits: false`;
    const result = classifyPolicyEdit({ oldYaml, newYaml, kind: "release" });
    expect(result.loosenings).toHaveLength(1);
    expect(result.loosenings[0].description).toContain("require_signed_commits");
    expect(result.tightenings).toHaveLength(0);
  });

  it("tightening: requireProofMapComplete false→true", () => {
    const oldYaml = `require_proof_map_complete: false`;
    const newYaml = `require_proof_map_complete: true`;
    const result = classifyPolicyEdit({ oldYaml, newYaml, kind: "release" });
    expect(result.tightenings).toHaveLength(1);
    expect(result.loosenings).toHaveLength(0);
  });

  it("loosening: requireProofMapComplete true→false", () => {
    const oldYaml = `require_proof_map_complete: true`;
    const newYaml = `require_proof_map_complete: false`;
    const result = classifyPolicyEdit({ oldYaml, newYaml, kind: "release" });
    expect(result.loosenings).toHaveLength(1);
    expect(result.tightenings).toHaveLength(0);
  });
});

// --- sensitive-paths.yaml ---

describe("classifyPolicyEdit: sensitive-paths", () => {
  it("tightening: glob added", () => {
    const oldYaml = `globs:\n  - "src/**"`;
    const newYaml = `globs:\n  - "src/**"\n  - ".env*"`;
    const result = classifyPolicyEdit({ oldYaml, newYaml, kind: "sensitive-paths" });
    expect(result.tightenings).toHaveLength(1);
    expect(result.tightenings[0].description).toContain(".env*");
    expect(result.loosenings).toHaveLength(0);
  });

  it("loosening: glob removed", () => {
    const oldYaml = `globs:\n  - "src/**"\n  - ".env*"`;
    const newYaml = `globs:\n  - "src/**"`;
    const result = classifyPolicyEdit({ oldYaml, newYaml, kind: "sensitive-paths" });
    expect(result.loosenings).toHaveLength(1);
    expect(result.loosenings[0].description).toContain(".env*");
    expect(result.tightenings).toHaveLength(0);
  });
});

// --- owners.yaml ---

describe("classifyPolicyEdit: owners", () => {
  it("returns empty arrays (owners changes are not safety policy)", () => {
    const oldYaml = `policy_approver: alice`;
    const newYaml = `policy_approver: bob`;
    const result = classifyPolicyEdit({ oldYaml, newYaml, kind: "owners" });
    expect(result.tightenings).toHaveLength(0);
    expect(result.loosenings).toHaveLength(0);
  });
});
