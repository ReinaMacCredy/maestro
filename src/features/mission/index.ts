// Mission feature public surface
// External consumers should import from "@/features/mission" rather than
// reaching into subpaths.

// Domain types
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

// State helpers
export {
  getValidFeatureTransitions,
  assertMissionTransition,
  canTransitionMission,
  assertFeatureTransition,
  assertAssertionTransition,
  isTerminalAssertionStatus,
} from "./domain/mission-state.js";

// ID helpers
export { generateMissionId } from "./domain/mission-id.js";

// Validators
export { WORKER_TYPE_PATTERN, FEATURE_ID_PATTERN } from "./domain/mission-validators.js";

// Workflow templates
export { BUILT_IN_WORKFLOWS } from "./domain/workflows.js";

// Error factories
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

// Ports
export type { MissionStorePort } from "./ports/mission-store.port.js";
export type { FeatureStorePort } from "./feature/ports/feature-store.port.js";
export type { AssertionStorePort } from "./validation/ports/assertion-store.port.js";
export type { CheckpointStorePort } from "./checkpoint/ports/checkpoint-store.port.js";

// Adapters (classes are still exposed for tests and composition roots)
export { FsMissionStoreAdapter } from "./adapters/mission-store.adapter.js";
export { FsFeatureStoreAdapter } from "./feature/adapters/feature-store.adapter.js";
export { FsAssertionStoreAdapter } from "./validation/adapters/assertion-store.adapter.js";
export { FsCheckpointStoreAdapter } from "./checkpoint/adapters/checkpoint-store.adapter.js";

// Usecases consumed externally
export { generateMissionReport } from "./usecases/mission-report.usecase.js";
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

// Commands
export { registerMissionCommand } from "./commands/mission.command.js";
export { registerMilestoneCommand } from "./commands/milestone.command.js";
export { registerCheckpointCommand } from "./commands/checkpoint.command.js";
export { registerFeatureCommand } from "./feature/commands/feature.command.js";
export { registerValidateCommand } from "./validation/commands/validate.command.js";

// Services (composition root helper)
export { buildMissionServices } from "./services.js";
export type { MissionServices } from "./services.js";
