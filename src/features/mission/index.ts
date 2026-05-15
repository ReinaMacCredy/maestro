export type {
  Mission,
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
  MISSION_ID_PATTERN,
  AGENT_TYPE_PATTERN,
  FEATURE_ID_PATTERN,
  AgentReportSchema,
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
} from "@/features/principle";
export {
  buildPrincipleEffectiveness,
  hasSufficientSample,
  PRINCIPLE_SMALL_SAMPLE_THRESHOLD,
} from "@/features/principle";
export { DEFAULT_PRINCIPLES } from "@/features/principle";
export { validatePrinciple, validateCreatePrincipleInput } from "@/features/principle";

export type { MissionStorePort } from "./ports/mission-store.port.js";
export type { FeatureStorePort } from "./feature/ports/feature-store.port.js";
export type { AssertionStorePort } from "./ports/assertion-store.port.js";
export type { CheckpointStorePort } from "./ports/checkpoint-store.port.js";
export type { PrincipleStorePort } from "@/features/principle";

export { FsMissionStoreAdapter } from "./adapters/mission-store.adapter.js";
export { FsFeatureStoreAdapter } from "./feature/adapters/feature-store.adapter.js";
export { FsAssertionStoreAdapter } from "./adapters/assertion-store.adapter.js";
export { FsCheckpointStoreAdapter } from "./adapters/checkpoint-store.adapter.js";
export { JsonlPrincipleStoreAdapter } from "@/features/principle";

export { deriveMissionReport, generateMissionReport } from "./usecases/mission-report.usecase.js";
export type { MissionReport, MilestoneReportProgress } from "./usecases/mission-report.usecase.js";
export { buildMissions } from "./usecases/missions.usecase.js";
export type {
  ActiveMissionContext,
  MissionFullState,
  Missions,
} from "./usecases/missions.usecase.js";
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
  parseAgentReport,
} from "./feature/usecases/feature-lifecycle.usecase.js";
export type { ListFeaturesResult, UpdateFeatureResult } from "./feature/usecases/feature-lifecycle.usecase.js";
export {
  showAssertions,
  updateAssertion,
} from "./usecases/validation-lifecycle.usecase.js";
export type { ShowAssertionsResult, UpdateAssertionResult } from "./usecases/validation-lifecycle.usecase.js";

export { registerMissionCommand } from "./commands/mission.command.js";
export { registerMilestoneCommand } from "./commands/milestone.command.js";
export { registerCheckpointCommand } from "./commands/checkpoint.command.js";
export { registerPrincipleCommand } from "@/features/principle";
export { registerFeatureCommand } from "./feature/commands/feature.command.js";
export { registerValidateCommand } from "./commands/validate.command.js";

export { buildMissionServices } from "./services.js";
export type { MissionServices } from "./services.js";

export type {
  AgentReply,
  ReplyOutcome,
  ReplyAuthor,
  ReplyIngestResult,
} from "@/features/reply";
export { REPLY_OUTCOMES } from "@/features/reply";
export { validateAgentReply } from "@/features/reply";

export type { ReplyStorePort } from "@/features/reply";
export { FsReplyStoreAdapter } from "@/features/reply";

export {
  writeAgentReply,
  type WriteReplyInput,
} from "@/features/reply";

export {
  ingestReply,
  type IngestReplyDeps,
  type PrincipleOutcomeRecorder,
} from "@/features/reply";

export { registerReplyCommand } from "@/features/reply";
