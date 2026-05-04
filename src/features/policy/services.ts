import { loadOwners } from "./usecases/load-owners.usecase.js";
import { loadRiskPolicy } from "./usecases/load-risk-policy.usecase.js";
import { loadAutopilotPolicy } from "./usecases/load-autopilot-policy.usecase.js";
import { loadReleasePolicy } from "./usecases/load-release-policy.usecase.js";
import type { Owners } from "./domain/owners-types.js";
import type { RiskPolicy, AutopilotPolicy, ReleasePolicy } from "./domain/policy-types.js";

export interface PolicyServices {
  readonly loadOwners: () => Promise<Owners>;
  readonly getRiskPolicy: () => Promise<RiskPolicy>;
  readonly getAutopilotPolicy: () => Promise<AutopilotPolicy>;
  readonly getReleasePolicy: () => Promise<ReleasePolicy>;
}

export function buildPolicyServices(baseDir: string): PolicyServices {
  return {
    loadOwners: () => loadOwners(baseDir),
    getRiskPolicy: () => loadRiskPolicy(baseDir),
    getAutopilotPolicy: () => loadAutopilotPolicy(baseDir),
    getReleasePolicy: () => loadReleasePolicy(baseDir),
  };
}
