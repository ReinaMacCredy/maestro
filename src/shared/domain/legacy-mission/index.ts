/**
 * Legacy v1 Mission domain -- stable shared location.
 *
 * TUI (11 files), handoff (2), bundle (4), infra (2), and shared/lib all
 * consume these shapes. Moved here from src/features/mission/domain/ so that
 * PR-C can delete the remaining v1 CLI verbs without breaking consumers.
 *
 * This is NOT a rewire. The types, state machine, validators, IDs, workflows,
 * ports, adapters, and missions usecase are byte-for-byte equivalent to what
 * lived under src/features/mission/ -- only the import paths changed.
 */

export type {
  Mission,
  MissionSummary,
  Feature,
  Milestone,
  Assertion,
  Checkpoint,
  AgentReport,
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
} from "./types.js";

export {
  getValidFeatureTransitions,
  getValidMissionTransitions,
  getValidMilestoneTransitions,
  getValidAssertionTransitions,
  assertMissionTransition,
  assertMilestoneTransition,
  canTransitionMission,
  canTransitionMilestone,
  canTransitionFeature,
  assertFeatureTransition,
  assertAssertionTransition,
  canTransitionAssertion,
  isTerminalMissionStatus,
  isTerminalMilestoneStatus,
  isTerminalFeatureStatus,
  isTerminalAssertionStatus,
} from "./state-machine.js";

export { generateMissionId } from "./ids.js";

export {
  MISSION_ID_PATTERN,
  AGENT_TYPE_PATTERN,
  FEATURE_ID_PATTERN,
  AgentReportSchema,
  MilestoneKindSchema,
  MilestoneProfileSchema,
  MilestoneInputSchema,
  MilestoneSchema,
  FeatureSchema,
  AssertionSchema,
  MissionSchema,
  CheckpointSchema,
  validateMission,
  validateMilestone,
  validateFeature,
  validateAssertion,
  validateCheckpoint,
  validateCreateMissionInput,
  validateMissionPlanFile,
  validateWorkflowTemplate,
  validateCreateFeatureInput,
  validateCreateAssertionInput,
  validateUpdateAssertionInput,
  assertNoDanglingReferences,
  assertNoCyclicDependencies,
} from "./validators.js";

export { BUILT_IN_WORKFLOWS } from "./workflows.js";
export type { WorkflowTemplate, WorkflowPhase } from "./workflow-types.js";

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
} from "./errors.js";

export type { MissionStorePort } from "./ports/mission-store.port.js";
export type { FeatureStorePort } from "./ports/feature-store.port.js";
export type { AssertionStorePort } from "./ports/assertion-store.port.js";
export type { CheckpointStorePort } from "./ports/checkpoint-store.port.js";

export { FsMissionStoreAdapter } from "./adapters/mission-store.adapter.js";
export { FsFeatureStoreAdapter } from "./adapters/feature-store.adapter.js";
export { FsAssertionStoreAdapter } from "./adapters/assertion-store.adapter.js";
export { FsCheckpointStoreAdapter } from "./adapters/checkpoint-store.adapter.js";

export { buildMissions } from "./missions.js";
export type {
  ActiveMissionContext,
  MissionFullState,
  Missions,
} from "./missions.js";

export { buildLegacyMissionServices } from "./services.js";
export type { LegacyMissionServices } from "./services.js";

export {
  updateFeature,
  listFeatures,
  parseAgentReport,
  getValidFeatureNextStates,
} from "./feature-lifecycle.usecase.js";
export type {
  ListFeaturesResult,
  UpdateFeatureResult,
} from "./feature-lifecycle.usecase.js";

export {
  createMission,
  listMissions,
  showMission,
  approveMission,
  rejectMission,
  updateMission,
  expandWorkflowTemplate,
  getValidMissionNextStates,
} from "./mission-lifecycle.usecase.js";
export type { CreateMissionResult } from "./mission-lifecycle.usecase.js";

export {
  deriveMissionReport,
  generateMissionReport,
} from "./mission-report.usecase.js";
export type {
  MissionReport,
  MilestoneReportProgress,
} from "./mission-report.usecase.js";

export {
  migrateLegacyWorkerType,
} from "./feature-migration.js";
export type { LegacyWorkerTypeMigration } from "./feature-migration.js";
