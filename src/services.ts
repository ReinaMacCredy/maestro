import { buildInfraServices, type InfraServices } from "./infra/services.js";
import { buildLegacyMissionServices, type LegacyMissionServices } from "@/shared/domain/legacy-mission";
import { buildPrincipleServices, type PrincipleServices } from "./features/principle/services.js";
import { buildReplyServices, type ReplyServices } from "./features/reply/services.js";
import { buildHandoffServices, type HandoffServices } from "./features/handoff/services.js";
import { buildTaskServices, type TaskServices } from "@/shared/domain/legacy-task/index.js";
import { buildBundleServices, type BundleServices } from "./features/bundle/services.js";
import { buildEvidenceServices, type EvidenceServices } from "./features/evidence/services.js";
import { FsSpecStoreAdapter } from "@/shared/domain/legacy-spec/index.js";
import type { LegacySpecStorePort } from "@/shared/domain/legacy-spec/index.js";
import { buildPolicyServices, type PolicyServices } from "./features/policy/services.js";
import { buildVerifyServices, type VerifyServices } from "./features/verdict/services.js";
import { buildRiskServices, type RiskServices } from "./features/risk/services.js";
import { buildVerdictServices, type VerdictServices } from "./features/verdict/services.js";
import { buildPlanServices, type PlanServices } from "./features/plan/services.js";
import { buildCiServices, type CiServices } from "./features/ci/services.js";
import { buildMergeServices, type MergeServices } from "./features/merge/services.js";
import { buildDeployServices, type DeployServices } from "./features/deploy/services.js";
import { buildRuntimeServices, type RuntimeServices } from "./features/runtime/services.js";

export interface Services extends
  InfraServices,
  LegacyMissionServices,
  PrincipleServices,
  ReplyServices,
  HandoffServices,
  TaskServices,
  BundleServices,
  EvidenceServices,
  PolicyServices,
  VerifyServices,
  RiskServices,
  VerdictServices,
  PlanServices,
  CiServices,
  MergeServices,
  DeployServices,
  RuntimeServices {
  readonly specStore: LegacySpecStorePort;
  readonly projectRoot: string;
}

export function createServices(
  projectDir: string,
  overrides?: Partial<Services>,
): Services {
  const base: Services = {
    ...buildInfraServices(projectDir),
    ...buildLegacyMissionServices(projectDir),
    ...buildPrincipleServices(projectDir),
    ...buildReplyServices(projectDir),
    ...buildHandoffServices(),
    ...buildTaskServices(projectDir),
    ...buildBundleServices(),
    ...buildEvidenceServices(projectDir),
    specStore: new FsSpecStoreAdapter(projectDir),
    ...buildPolicyServices(projectDir),
    ...buildVerifyServices(projectDir),
    ...buildRiskServices(),
    ...buildVerdictServices(projectDir),
    ...buildPlanServices(),
    ...buildCiServices(),
    ...buildMergeServices(),
    ...buildDeployServices(),
    ...buildRuntimeServices(),
    projectRoot: projectDir,
  };
  return overrides ? { ...base, ...overrides } : base;
}
