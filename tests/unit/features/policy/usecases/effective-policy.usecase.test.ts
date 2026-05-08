import { describe, it, expect } from "bun:test";
import { buildEffectivePolicyServices } from "@/features/policy/usecases/effective-policy.usecase.js";
import type { PendingLoosening } from "@/features/policy/usecases/detect-pending-loosenings.usecase.js";
import type { AutopilotPolicy, ReleasePolicy, RiskPolicy } from "@/features/policy/domain/policy-types.js";

// --- Fixture data ---

const BASE_AUTOPILOT: AutopilotPolicy = {
  kind: "autopilot",
  id: "test",
  version: "1",
  autoMergeAllowed: { low: true, medium: false, high: false, critical: false },
  requiredWitnessLevel: {
    low: "agent-claimed-locally",
    medium: "witnessed-by-maestro",
    high: "witnessed-by-maestro",
    critical: "witnessed-by-maestro",
  },
};

const BASE_RELEASE: ReleasePolicy = {
  kind: "release",
  id: "test",
  version: "1",
  requireSignedCommits: false,
  requireProofMapComplete: false,
};

const BASE_RISK: RiskPolicy = {
  kind: "risk",
  id: "test",
  version: "1",
  rows: [
    { signal: "changes-lockfile", derivedClass: "low" },
    { signal: "touches-secrets", derivedClass: "critical" },
  ],
};

function makePendingLoosening(overrides: Partial<PendingLoosening>): PendingLoosening {
  const now = new Date();
  const effectiveAt = new Date(now.getTime() + 29 * 24 * 60 * 60 * 1000).toISOString();
  return {
    commitSha: "abc123",
    commitTime: now.toISOString(),
    effectiveAt,
    kind: "autopilot",
    file: ".maestro/policies/autopilot.yaml",
    edit: {
      description: "test loosening",
      path: "autoMergeAllowed.low",
      oldValue: false,
      newValue: true,
    },
    ...overrides,
  };
}

// --- Tests ---

describe("buildEffectivePolicyServices: autopilot", () => {
  it("pending loosening (autoMergeAllowed false→true) is reverted in effective policy", async () => {
    // BASE_AUTOPILOT already has low: true (the loosening is on-disk).
    // We inject a pending loosening that says this was a false→true change.
    const loosening = makePendingLoosening({
      kind: "autopilot",
      edit: {
        description: "auto_merge_allowed.low: false → true",
        path: "autoMergeAllowed.low",
        oldValue: false,
        newValue: true,
      },
    });

    const services = buildEffectivePolicyServices({
      projectRoot: "/fake",
      loadRiskPolicyImpl: async () => BASE_RISK,
      loadAutopilotPolicyImpl: async () => BASE_AUTOPILOT,
      loadReleasePolicyImpl: async () => BASE_RELEASE,
      loadSensitivePathsGlobsImpl: async () => [],
      detectPendingLooseningsImpl: async () => [loosening],
    });

    const effective = await services.getEffectiveAutopilotPolicy();
    // The loosening (low: true) should be reverted back to false
    expect(effective.autoMergeAllowed.low).toBe(false);
    // Other classes unaffected
    expect(effective.autoMergeAllowed.medium).toBe(false);
    // Witness levels unchanged
    expect(effective.requiredWitnessLevel.high).toBe("witnessed-by-maestro");
  });

  it("pending loosening (requiredWitnessLevel lowered) is reverted in effective policy", async () => {
    const currentPolicy: AutopilotPolicy = {
      ...BASE_AUTOPILOT,
      requiredWitnessLevel: {
        ...BASE_AUTOPILOT.requiredWitnessLevel,
        high: "agent-claimed-locally", // on-disk: lowered
      },
    };

    const loosening = makePendingLoosening({
      kind: "autopilot",
      edit: {
        description: "required_witness_level.high lowered from 'witnessed-by-maestro' to 'agent-claimed-locally'",
        path: "requiredWitnessLevel.high",
        oldValue: "witnessed-by-maestro",
        newValue: "agent-claimed-locally",
      },
    });

    const services = buildEffectivePolicyServices({
      projectRoot: "/fake",
      loadRiskPolicyImpl: async () => BASE_RISK,
      loadAutopilotPolicyImpl: async () => currentPolicy,
      loadReleasePolicyImpl: async () => BASE_RELEASE,
      loadSensitivePathsGlobsImpl: async () => [],
      detectPendingLooseningsImpl: async () => [loosening],
    });

    const effective = await services.getEffectiveAutopilotPolicy();
    // Should be reverted to witnessed-by-maestro
    expect(effective.requiredWitnessLevel.high).toBe("witnessed-by-maestro");
    // Other classes unaffected
    expect(effective.requiredWitnessLevel.low).toBe("agent-claimed-locally");
  });

  it("loosening past 31 days is honored (shows up in effective policy as-is)", async () => {
    // On-disk: low is true. The loosening is 31 days old → effectiveAt has passed.
    // detectPendingLooseningsImpl returns [] (expired items are filtered out).
    const services = buildEffectivePolicyServices({
      projectRoot: "/fake",
      loadRiskPolicyImpl: async () => BASE_RISK,
      loadAutopilotPolicyImpl: async () => BASE_AUTOPILOT,
      loadReleasePolicyImpl: async () => BASE_RELEASE,
      loadSensitivePathsGlobsImpl: async () => [],
      detectPendingLooseningsImpl: async () => [],
    });

    const effective = await services.getEffectiveAutopilotPolicy();
    // No pending loosenings to revert → on-disk value is honored
    expect(effective.autoMergeAllowed.low).toBe(true);
  });
});

describe("buildEffectivePolicyServices: release", () => {
  it("pending loosening (requireSignedCommits true→false) is reverted", async () => {
    const currentRelease: ReleasePolicy = {
      ...BASE_RELEASE,
      requireSignedCommits: false, // loosened on-disk
    };

    const loosening = makePendingLoosening({
      kind: "release",
      file: ".maestro/policies/release.yaml",
      edit: {
        description: "require_signed_commits: true → false",
        path: "requireSignedCommits",
        oldValue: true,
        newValue: false,
      },
    });

    const services = buildEffectivePolicyServices({
      projectRoot: "/fake",
      loadRiskPolicyImpl: async () => BASE_RISK,
      loadAutopilotPolicyImpl: async () => BASE_AUTOPILOT,
      loadReleasePolicyImpl: async () => currentRelease,
      loadSensitivePathsGlobsImpl: async () => [],
      detectPendingLooseningsImpl: async () => [loosening],
    });

    const effective = await services.getEffectiveReleasePolicy();
    expect(effective.requireSignedCommits).toBe(true);
  });
});

describe("buildEffectivePolicyServices: risk", () => {
  it("pending loosening (row removed) is reverted by re-adding the row", async () => {
    // On-disk: touches-secrets row has been removed
    const currentRisk: RiskPolicy = {
      ...BASE_RISK,
      rows: [{ signal: "changes-lockfile", derivedClass: "low" }],
    };

    const loosening = makePendingLoosening({
      kind: "risk",
      file: ".maestro/policies/risk.yaml",
      edit: {
        description: "removed risk row: signal 'touches-secrets' (was derived_class 'critical')",
        path: "rows[touches-secrets]",
        oldValue: { signal: "touches-secrets", derived_class: "critical" },
        newValue: undefined,
      },
    });

    const services = buildEffectivePolicyServices({
      projectRoot: "/fake",
      loadRiskPolicyImpl: async () => currentRisk,
      loadAutopilotPolicyImpl: async () => BASE_AUTOPILOT,
      loadReleasePolicyImpl: async () => BASE_RELEASE,
      loadSensitivePathsGlobsImpl: async () => [],
      detectPendingLooseningsImpl: async () => [loosening],
    });

    const effective = await services.getEffectiveRiskPolicy();
    const signals = effective.rows.map((r) => r.signal);
    expect(signals).toContain("touches-secrets");
    const row = effective.rows.find((r) => r.signal === "touches-secrets");
    expect(row?.derivedClass).toBe("critical");
  });

  it("pending loosening (derived_class lowered) is reverted", async () => {
    // On-disk: changes-lockfile is now 'low' (was 'medium')
    const currentRisk: RiskPolicy = {
      ...BASE_RISK,
      rows: [
        { signal: "changes-lockfile", derivedClass: "low" },
        { signal: "touches-secrets", derivedClass: "critical" },
      ],
    };

    const loosening = makePendingLoosening({
      kind: "risk",
      file: ".maestro/policies/risk.yaml",
      edit: {
        description: "lowered derived_class for signal 'changes-lockfile' from 'medium' to 'low'",
        path: "rows[changes-lockfile].derived_class",
        oldValue: "medium",
        newValue: "low",
      },
    });

    const services = buildEffectivePolicyServices({
      projectRoot: "/fake",
      loadRiskPolicyImpl: async () => currentRisk,
      loadAutopilotPolicyImpl: async () => BASE_AUTOPILOT,
      loadReleasePolicyImpl: async () => BASE_RELEASE,
      loadSensitivePathsGlobsImpl: async () => [],
      detectPendingLooseningsImpl: async () => [loosening],
    });

    const effective = await services.getEffectiveRiskPolicy();
    const row = effective.rows.find((r) => r.signal === "changes-lockfile");
    expect(row?.derivedClass).toBe("medium");
  });
});

describe("buildEffectivePolicyServices: sensitive-paths", () => {
  it("pending loosening (glob removed) is reverted by re-adding the glob", async () => {
    // On-disk: "src/auth/**" was removed (loosened); only "src/secrets/**" remains
    const currentGlobs: readonly string[] = ["src/secrets/**"];

    const loosening = makePendingLoosening({
      kind: "sensitive-paths",
      file: ".maestro/policies/sensitive-paths.yaml",
      edit: {
        description: "removed sensitive-paths glob: 'src/auth/**'",
        path: "globs[src/auth/**]",
        oldValue: "src/auth/**",
        newValue: undefined,
      },
    });

    const services = buildEffectivePolicyServices({
      projectRoot: "/fake",
      loadRiskPolicyImpl: async () => BASE_RISK,
      loadAutopilotPolicyImpl: async () => BASE_AUTOPILOT,
      loadReleasePolicyImpl: async () => BASE_RELEASE,
      loadSensitivePathsGlobsImpl: async () => currentGlobs,
      detectPendingLooseningsImpl: async () => [loosening],
    });

    const effective = await services.getEffectiveSensitivePathsGlobs();
    expect(effective).toContain("src/auth/**");
    expect(effective).toContain("src/secrets/**");
  });

  it("no pending sensitive-paths loosening — on-disk globs are honored as-is", async () => {
    const currentGlobs: readonly string[] = ["src/secrets/**"];

    const services = buildEffectivePolicyServices({
      projectRoot: "/fake",
      loadRiskPolicyImpl: async () => BASE_RISK,
      loadAutopilotPolicyImpl: async () => BASE_AUTOPILOT,
      loadReleasePolicyImpl: async () => BASE_RELEASE,
      loadSensitivePathsGlobsImpl: async () => currentGlobs,
      detectPendingLooseningsImpl: async () => [],
    });

    const effective = await services.getEffectiveSensitivePathsGlobs();
    expect(effective).toEqual(["src/secrets/**"]);
  });

  it("loosenings of other kinds (autopilot, risk) do not affect sensitive-paths globs", async () => {
    const currentGlobs: readonly string[] = ["src/secrets/**"];

    const autopilotLoosening = makePendingLoosening({
      kind: "autopilot",
      edit: {
        description: "auto_merge_allowed.medium: false → true",
        path: "autoMergeAllowed.medium",
        oldValue: false,
        newValue: true,
      },
    });

    const services = buildEffectivePolicyServices({
      projectRoot: "/fake",
      loadRiskPolicyImpl: async () => BASE_RISK,
      loadAutopilotPolicyImpl: async () => BASE_AUTOPILOT,
      loadReleasePolicyImpl: async () => BASE_RELEASE,
      loadSensitivePathsGlobsImpl: async () => currentGlobs,
      detectPendingLooseningsImpl: async () => [autopilotLoosening],
    });

    const effective = await services.getEffectiveSensitivePathsGlobs();
    expect(effective).toEqual(["src/secrets/**"]);
  });

  it("does not double-add a glob that is already on disk", async () => {
    // Edge case: someone re-added the glob between commits but the loosening
    // is still in the lookback window. The revert is a no-op.
    const currentGlobs: readonly string[] = ["src/auth/**", "src/secrets/**"];

    const loosening = makePendingLoosening({
      kind: "sensitive-paths",
      file: ".maestro/policies/sensitive-paths.yaml",
      edit: {
        description: "removed sensitive-paths glob: 'src/auth/**'",
        path: "globs[src/auth/**]",
        oldValue: "src/auth/**",
        newValue: undefined,
      },
    });

    const services = buildEffectivePolicyServices({
      projectRoot: "/fake",
      loadRiskPolicyImpl: async () => BASE_RISK,
      loadAutopilotPolicyImpl: async () => BASE_AUTOPILOT,
      loadReleasePolicyImpl: async () => BASE_RELEASE,
      loadSensitivePathsGlobsImpl: async () => currentGlobs,
      detectPendingLooseningsImpl: async () => [loosening],
    });

    const effective = await services.getEffectiveSensitivePathsGlobs();
    expect(effective.filter((g) => g === "src/auth/**")).toHaveLength(1);
  });
});
