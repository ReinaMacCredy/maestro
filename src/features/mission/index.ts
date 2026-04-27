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
export type { AssertionStorePort } from "./ports/assertion-store.port.js";
export type { CheckpointStorePort } from "./ports/checkpoint-store.port.js";
export type { PrincipleStorePort } from "./ports/principle-store.port.js";

export { FsMissionStoreAdapter } from "./adapters/mission-store.adapter.js";
export { FsFeatureStoreAdapter } from "./feature/adapters/feature-store.adapter.js";
export { FsAssertionStoreAdapter } from "./adapters/assertion-store.adapter.js";
export { FsCheckpointStoreAdapter } from "./adapters/checkpoint-store.adapter.js";
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
export { registerPrincipleCommand } from "./commands/principle.command.js";
export { registerFeatureCommand } from "./feature/commands/feature.command.js";
export { registerValidateCommand } from "./commands/validate.command.js";

export { buildMissionServices } from "./services.js";
export type { MissionServices } from "./services.js";

export type {
  AgentReply,
  ReplyOutcome,
  ReplyAuthor,
  ReplyIngestResult,
} from "./reply/domain/reply-types.js";
export { REPLY_OUTCOMES } from "./reply/domain/reply-types.js";
export { validateAgentReply } from "./reply/domain/reply-validators.js";

export type { ReplyStorePort } from "./reply/ports/reply-store.port.js";
export { FsReplyStoreAdapter } from "./reply/adapters/fs-reply-store.adapter.js";

export {
  writeAgentReply,
  type WriteReplyInput,
} from "./reply/usecases/write-reply.usecase.js";

export {
  ingestReply,
  type IngestReplyDeps,
  type PrincipleOutcomeRecorder,
} from "./reply/usecases/ingest-reply.usecase.js";

export { registerReplyCommand } from "./reply/commands/reply.command.js";
export { buildReplyServices } from "./reply/services.js";
export type { ReplyServices } from "./reply/services.js";
