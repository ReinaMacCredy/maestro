import { loadOwners } from "./usecases/load-owners.usecase.js";
import { loadRiskPolicy } from "./usecases/load-risk-policy.usecase.js";
import { loadAutopilotPolicy } from "./usecases/load-autopilot-policy.usecase.js";
import { loadReleasePolicy } from "./usecases/load-release-policy.usecase.js";
import { loadSensitivePathsGlobs } from "./usecases/load-sensitive-paths-globs.usecase.js";
import { buildDetectPendingLoosenings } from "./usecases/detect-pending-loosenings.usecase.js";
import { buildEffectivePolicyServices } from "./usecases/effective-policy.usecase.js";
import type { Owners } from "./domain/owners-types.js";
import type { RiskPolicy, AutopilotPolicy, ReleasePolicy } from "./domain/policy-types.js";
import type { PendingLoosening } from "./usecases/detect-pending-loosenings.usecase.js";

export interface PolicyServices {
  readonly loadOwners: () => Promise<Owners>;
  /** Raw on-disk policy (used by bootstrap/init paths) */
  readonly getRiskPolicy: () => Promise<RiskPolicy>;
  readonly getAutopilotPolicy: () => Promise<AutopilotPolicy>;
  readonly getReleasePolicy: () => Promise<ReleasePolicy>;
  /** Effective policies (with pending loosenings reverted — Rule 9) */
  readonly getEffectiveRiskPolicy: () => Promise<RiskPolicy>;
  readonly getEffectiveAutopilotPolicy: () => Promise<AutopilotPolicy>;
  readonly getEffectiveReleasePolicy: () => Promise<ReleasePolicy>;
  readonly getEffectiveSensitivePathsGlobs: () => Promise<readonly string[]>;
  /** Currently-pending loosenings list */
  readonly pendingLoosenings: () => Promise<readonly PendingLoosening[]>;
}

export function buildPolicyServices(baseDir: string): PolicyServices {
  const detectPending = buildDetectPendingLoosenings(baseDir);

  const effective = buildEffectivePolicyServices({
    projectRoot: baseDir,
    loadRiskPolicyImpl: loadRiskPolicy,
    loadAutopilotPolicyImpl: loadAutopilotPolicy,
    loadReleasePolicyImpl: loadReleasePolicy,
    loadSensitivePathsGlobsImpl: loadSensitivePathsGlobs,
    detectPendingLooseningsImpl: detectPending,
  });

  return {
    loadOwners: () => loadOwners(baseDir),
    getRiskPolicy: () => loadRiskPolicy(baseDir),
    getAutopilotPolicy: () => loadAutopilotPolicy(baseDir),
    getReleasePolicy: () => loadReleasePolicy(baseDir),
    getEffectiveRiskPolicy: effective.getEffectiveRiskPolicy,
    getEffectiveAutopilotPolicy: effective.getEffectiveAutopilotPolicy,
    getEffectiveReleasePolicy: effective.getEffectiveReleasePolicy,
    getEffectiveSensitivePathsGlobs: effective.getEffectiveSensitivePathsGlobs,
    pendingLoosenings: detectPending,
  };
}
