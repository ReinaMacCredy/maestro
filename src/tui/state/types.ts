import type {
  MissionStatus,
  MilestoneStatus,
  FeatureStatus,
  MilestoneKind,
  MilestoneProfile,
} from "@/features/mission";
import type { DoctorCheck } from "@/infra/domain/status-types.js";
import type { GitFileChange } from "@/infra/domain/git-types.js";
import type { MissionControlBackgroundMode } from "@/tui/shared/ui-config.js";
import type {
  AgentGridRow,
  DispatchQueueItem,
  EventStreamEntry,
  PrincipleEffectivenessRow,
  ReplyInboxEntry,
  TaskBoardSnapshot,
  TimelineMilestoneEntry,
} from "./screen-types.js";
import type { AutopilotSnapshot } from "./autopilot-screen.js";

export type MissionControlMode = "mission" | "home";
export type LeftPaneMode = "overview" | "preview";

export interface MissionControlHomeAction {
  label: string;
  command: string;
  detail: string;
}

export interface MissionControlSessionSidebar {
  branch: string;
  workingTreeClean: boolean;
  diffStat: string;
  changedFiles: readonly string[];
  fileChanges?: readonly GitFileChange[];
}

export interface MissionControlConfigSummary {
  configSource: "project" | "global" | "none";
  gitAvailable: boolean;
  checks: readonly DoctorCheck[];
  missionDirectory: string | null;
  backgroundMode: MissionControlBackgroundMode;
}

export type MissionControlConfigTab =
  | "overview"
  | "effective"
  | "project"
  | "global"
  | "defaults"
  | "plan"
  | "doctor"
  | "memory";

export type MissionControlConfigValueSource =
  | "project"
  | "global"
  | "default"
  | "mixed"
  | "none";

export type MissionControlConfigSourceBadge = "P" | "G" | "D" | "M" | "";

export type MissionControlConfigEditKind =
  | "readonly"
  | "toggle"
  | "enum"
  | "number-preset";

export interface MissionControlConfigRow {
    keyPath: string;
    label: string;
    section: string;
    valueText: string;
    displayValueText: string;
    source: MissionControlConfigValueSource;
    sourceBadge: MissionControlConfigSourceBadge;
    editKind: MissionControlConfigEditKind;
    editKindLabel: string;
    options?: readonly string[];
    description: string;
    summary: string;
    impactText: string;
    effectiveValueText: string;
    effectiveDisplayValueText: string;
    projectValueText?: string;
    projectDisplayValueText?: string;
    globalValueText?: string;
      globalDisplayValueText?: string;
      defaultValueText?: string;
      defaultDisplayValueText?: string;
    }

export interface MissionControlConfigInspector {
  tabs: readonly MissionControlConfigTab[];
  rowsByTab: Readonly<Record<MissionControlConfigTab, readonly MissionControlConfigRow[]>>;
  hasProjectConfig: boolean;
  hasGlobalConfig: boolean;
  projectPath: string;
  globalPath: string;
  errors: readonly string[];
}

export interface MissionControlHomeState {
  headline: string;
  summary: string;
  locationLabel: string;
  checks: readonly DoctorCheck[];
  actions: readonly MissionControlHomeAction[];
}

export interface BlockedByRef {
  id: string;
  title: string;
  status: FeatureStatus;
}

export interface UnblocksRef {
  id: string;
  title: string;
  status: FeatureStatus;
}

export interface AgentSummaryRow {
  agent: string;
  count: number;
}

export interface DependencyMapRow {
  root: BlockedByRef;
  primaryDependent?: BlockedByRef;
  primaryDependentBlockedByCount?: number;
  hiddenDependentCount: number;
}

export interface MissionOverviewPane {
  missionLabel: string;
  statusLabel: string;
  activeCount: number;
  doneCount: number;
  totalCount: number;
  blockedCount: number;
  currentMilestone: string | null;
  gateLabel: string | null;
  agentSummary: readonly AgentSummaryRow[];
  dependencyMap: readonly DependencyMapRow[];
}

export interface MissionControlMemorySnapshot {
  // v1 memory + graph were retired in Phase 4. This snapshot is kept as a
  // typed null bag so existing consumers don't break.
  readonly _retired?: true;
}

export interface MissionControlSnapshot {
  mode: MissionControlMode;
  // Header
  missionId: string;
  missionTitle: string;
  missionStatus: MissionStatus;
  effectiveStatus: MissionStatus;
  elapsedMs: number;
  featureProgress: { done: number; total: number; active: number };
  statusProgress: MissionControlStatusProgress;
  tokenCounters: { input: number; cached: number; output: number } | null;

  // Left pane
  missionOverview?: MissionOverviewPane | null;
  activeFeature: TaskPreviewPane | null;

  // Right pane (feature list)
  features: readonly MissionControlFeatureRow[];
  taskPreviews?: readonly TaskPreviewPane[];

  // Lower pane
  session: MissionControlSessionSidebar | null;
  configSummary: MissionControlConfigSummary | null;
  configInspector?: MissionControlConfigInspector | null;
  progressLog: readonly MissionControlEvent[];

  // Milestones (for grouping)
  milestones: readonly MissionControlMilestoneRow[];

  // Gate state
  gateBlocked?: boolean;
  gateLabel?: string | null;

  // Footer state
  canPause: boolean;
  canResume: boolean;

  // Memory pane (project-graph context only after v2 cleanup)
  memory?: MissionControlMemorySnapshot | null;

  // Conductor screens
  agentGrid?: readonly AgentGridRow[];
  dispatchQueue?: readonly DispatchQueueItem[];
  eventStream?: readonly EventStreamEntry[];
  taskBoard?: TaskBoardSnapshot | null;
  timelineMilestones?: readonly TimelineMilestoneEntry[];
  replyInbox?: readonly ReplyInboxEntry[];
  principleEffectiveness?: readonly PrincipleEffectivenessRow[];

  // Autopilot (mission mode only)
  autopilot?: AutopilotSnapshot;

  // Home mode
  home: MissionControlHomeState | null;
}

export interface MissionControlMilestoneRow {
  id: string;
  title: string;
  status: MilestoneStatus;
  order: number;
  kind?: MilestoneKind;
  profile?: MilestoneProfile;
}

export interface MissionControlFeatureRow {
  id: string;
  title: string;
  status: FeatureStatus;
  milestoneId: string;
  agentType: string;
  hasReport: boolean;
  blockedByIds?: readonly string[];
  blockedByLabel?: string;
}

export interface TaskPreviewPane {
  id: string;
  title: string;
  status: FeatureStatus;
  milestoneId: string;
  milestoneTitle: string;
  agentType: string;
  description: string;
  preconditions: string | undefined;
  expectedBehavior: string | undefined;
  verificationSteps: readonly string[];
  dependsOn: readonly string[];
  blockedBy?: readonly BlockedByRef[];
  unblocks?: readonly UnblocksRef[];
  fulfills: readonly string[];
  validTransitions: readonly FeatureStatus[];
}

export type MissionControlFeatureDetail = TaskPreviewPane;

export interface MissionControlEvent {
  timestamp: string;
  relativeMs: number;
  kind: "mission" | "feature" | "milestone" | "assertion" | "checkpoint";
  title: string;
  detail?: string;
}

export interface MissionControlStatusProgress {
  completed: number;
  total: number;
  inFlight: number;
  blocked: number;
  queued: number;
  completionPct: number;
}

export type {
  InferredAgentStatus,
  AgentGridRow,
  DispatchQueueItem,
  EventStreamEntry,
  EventStreamEntryKind,
  TaskBoardSnapshot,
  TaskBoardItem,
  TimelineMilestoneEntry,
} from "./screen-types.js";
export type { AutopilotSnapshot, AutopilotTaskRow } from "./autopilot-screen.js";
