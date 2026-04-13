export type {
  Mission,
  Feature,
  Milestone,
  Assertion,
  Checkpoint,
  WorkerReport,
  MissionStatus,
  MilestoneStatus,
  FeatureStatus,
  MilestoneKind,
  MilestoneProfile,
  AssertionResult,
  AssertionSurface,
  MilestoneInput,
  CreateMissionInput,
  UpdateMissionInput,
  CreateFeatureInput,
  UpdateFeatureInput,
  CreateAssertionInput,
  UpdateAssertionInput,
  MissionPlanFile,
  MissionPlanFeature,
  CommandRun,
  InteractiveCheck,
  TestCase,
  TestFile,
  DiscoveredIssue,
} from "./domain/mission-types.js";

export {
  getValidFeatureTransitions,
  assertMissionTransition,
  canTransitionMission,
  canTransitionFeature,
  assertFeatureTransition,
  assertAssertionTransition,
  isTerminalAssertionStatus,
} from "./domain/mission-state.js";

export { generateMissionId } from "./domain/mission-id.js";
export {
  WORKER_TYPE_PATTERN,
  FEATURE_ID_PATTERN,
  WorkerReportSchema,
} from "./domain/mission-validators.js";

export { BUILT_IN_WORKFLOWS } from "./domain/workflows.js";
export type { WorkflowTemplate, WorkflowPhase } from "./domain/workflow-types.js";

export {
  missionNotFound,
  milestoneNotFound,
  featureNotFound,
  assertionNotFound,
  invalidMissionTransition,
  invalidMilestoneTransition,
  invalidFeatureTransition,
  invalidAssertionTransition,
  danglingReference,
  cyclicDependency,
  duplicateMilestoneId,
  milestoneNotSealable,
  checkpointNotFound,
} from "./domain/errors.js";

export type {
  Principle,
  CreatePrincipleInput,
  PrincipleMode,
  GateCheckType,
  PrincipleSource,
  PrincipleOutcome,
  PrincipleOutcomeRecord,
  PrincipleEffectiveness,
} from "./domain/principle-types.js";
export {
  buildPrincipleEffectiveness,
  hasSufficientSample,
  PRINCIPLE_SMALL_SAMPLE_THRESHOLD,
} from "./usecases/principle-effectiveness.usecase.js";
export { DEFAULT_PRINCIPLES } from "./domain/default-principles.js";
export { validatePrinciple, validateCreatePrincipleInput } from "./domain/principle-validators.js";

export type { MissionStorePort } from "./ports/mission-store.port.js";
export type { FeatureStorePort } from "./feature/ports/feature-store.port.js";
export type { AssertionStorePort } from "./validation/ports/assertion-store.port.js";
export type { CheckpointStorePort } from "./checkpoint/ports/checkpoint-store.port.js";
export type { PrincipleStorePort } from "./ports/principle-store.port.js";

export { FsMissionStoreAdapter } from "./adapters/mission-store.adapter.js";
export { FsFeatureStoreAdapter } from "./feature/adapters/feature-store.adapter.js";
export { FsAssertionStoreAdapter } from "./validation/adapters/assertion-store.adapter.js";
export { FsCheckpointStoreAdapter } from "./checkpoint/adapters/checkpoint-store.adapter.js";
export { JsonlPrincipleStoreAdapter } from "./adapters/principle-store.adapter.js";

export { deriveMissionReport, generateMissionReport } from "./usecases/mission-report.usecase.js";
export type { MissionReport, MilestoneReportProgress } from "./usecases/mission-report.usecase.js";
export {
  createMission,
  listMissions,
  showMission,
  approveMission,
  rejectMission,
  updateMission,
  expandWorkflowTemplate,
} from "./usecases/mission-lifecycle.usecase.js";
export type { CreateMissionResult } from "./usecases/mission-lifecycle.usecase.js";
export {
  listMilestones,
  getMilestoneStatus,
  sealMilestone,
} from "./usecases/milestone-lifecycle.usecase.js";
export {
  listFeatures,
  updateFeature,
  parseWorkerReport,
} from "./feature/usecases/feature-lifecycle.usecase.js";
export type { ListFeaturesResult, UpdateFeatureResult } from "./feature/usecases/feature-lifecycle.usecase.js";
export {
  showAssertions,
  updateAssertion,
} from "./validation/usecases/validation-lifecycle.usecase.js";
export type { ShowAssertionsResult, UpdateAssertionResult } from "./validation/usecases/validation-lifecycle.usecase.js";

export { registerMissionCommand } from "./commands/mission.command.js";
export { registerMilestoneCommand } from "./commands/milestone.command.js";
export { registerCheckpointCommand } from "./commands/checkpoint.command.js";
export { registerPrincipleCommand } from "./commands/principle.command.js";
export { registerFeatureCommand } from "./feature/commands/feature.command.js";
export { registerValidateCommand } from "./validation/commands/validate.command.js";

export { buildMissionServices } from "./services.js";
export type { MissionServices } from "./services.js";
