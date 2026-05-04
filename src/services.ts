import { buildInfraServices, type InfraServices } from "./infra/services.js";
import { buildSessionServices, type SessionServices } from "./features/session/services.js";
import { buildNotesServices, type NotesServices } from "./features/notes/services.js";
import { buildMissionServices, type MissionServices } from "./features/mission/services.js";
import { buildMemoryServices, type MemoryServices } from "./features/memory/services.js";
import { buildHandoffServices, type HandoffServices } from "./features/handoff/services.js";
import { buildRatchetServices, type RatchetServices } from "./features/ratchet/services.js";
import { buildGraphServices, type GraphServices } from "./features/graph/services.js";
import { buildTaskServices, type TaskServices } from "./features/task/services.js";
import { buildBundleServices, type BundleServices } from "./features/bundle/services.js";
import { buildEvidenceServices, type EvidenceServices } from "./features/evidence/services.js";
import { buildSpecServices, type SpecServices } from "./features/spec/services.js";
import { buildPolicyServices, type PolicyServices } from "./features/policy/services.js";
import { buildVerifyServices, type VerifyServices } from "./features/verify/services.js";
import { buildRiskServices, type RiskServices } from "./features/risk/services.js";
import { buildVerdictServices, type VerdictServices } from "./features/verdict/services.js";
import { buildPlanServices, type PlanServices } from "./features/plan/services.js";
import { buildCiServices, type CiServices } from "./features/ci/services.js";

export interface Services extends
  InfraServices,
  SessionServices,
  NotesServices,
  MissionServices,
  MemoryServices,
  HandoffServices,
  RatchetServices,
  GraphServices,
  TaskServices,
  BundleServices,
  EvidenceServices,
  SpecServices,
  PolicyServices,
  VerifyServices,
  RiskServices,
  VerdictServices,
  PlanServices,
  CiServices {
  readonly projectRoot: string;
}

let instance: Services | undefined;

export function initServices(projectDir: string): Services {
  const policyServices = buildPolicyServices(projectDir);
  instance = {
    ...buildInfraServices(projectDir),
    ...buildSessionServices(),
    ...buildNotesServices(projectDir),
    ...buildMissionServices(projectDir),
    ...buildMemoryServices(projectDir),
    ...buildHandoffServices(),
    ...buildRatchetServices(projectDir),
    ...buildGraphServices(),
    ...buildTaskServices(projectDir),
    ...buildBundleServices(),
    ...buildEvidenceServices(projectDir),
    ...buildSpecServices(projectDir),
    ...policyServices,
    ...buildVerifyServices(projectDir),
    ...buildRiskServices(),
    ...buildVerdictServices(projectDir),
    ...buildPlanServices(),
    ...buildCiServices(),
    projectRoot: projectDir,
  };
  return instance;
}

export function getServices(): Services {
  if (!instance) {
    throw new Error("Services not initialized. Call initServices() first.");
  }
  return instance;
}
