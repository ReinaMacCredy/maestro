/**
 * TUI data model -- read-only DTOs for the mission control dashboard.
 */
import type {
  MissionStatus,
  MilestoneStatus,
  FeatureStatus,
  WorkerReport,
  MilestoneKind,
  MilestoneProfile,
} from "../../domain/mission-types.js";
import type { DoctorCheck, GitFileChange } from "../../domain/types.js";
import type { RuntimeState } from "../../domain/runtime-types.js";
import type { TransportType } from "../../domain/worker-types.js";

export type MissionControlMode = "mission" | "home";
export type LeftPaneMode = "overview" | "preview";

export interface MissionControlHomeAction {
  label: string;
  command: string;
  detail: string;
}

export interface MissionControlHomeHandoff {
  id: string;
  message: string;
  agent: string;
  sessionId?: string;
  sitrep?: string;
  quickstart?: string;
}

export interface MissionControlSessionSidebar {
  agent?: string;
  sessionId?: string;
  transport?: TransportType;
  branch: string;
  workingTreeClean: boolean;
  diffStat: string;
  changedFiles: readonly string[];
  fileChanges?: readonly GitFileChange[];
}

export interface MissionControlConfigSummary {
  configSource: "project" | "global" | "none";
  cassAvailable: boolean;
  gitAvailable: boolean;
  checks: readonly DoctorCheck[];
  missionDirectory: string | null;
  workerTypes: readonly string[];
}

export type MissionControlWorkerHealthStatus =
  | "ready"
  | "busy"
  | "degraded"
  | "missing"
  | "disabled";

export interface MissionControlWorkerHealthCheck {
  label: string;
  ok: boolean;
  detail?: string;
}

export interface MissionControlWorkerHealthRow {
  slug: string;
  label: string;
  status: MissionControlWorkerHealthStatus;
  detail: string;
  lastCheckedAt: string;
  checks: readonly MissionControlWorkerHealthCheck[];
  summary: string;
  bestFor: string;
  tradeoffs: string;
}

export type MissionControlConfigTab =
  | "overview"
  | "effective"
  | "project"
  | "global"
  | "defaults"
  | "workers"
  | "plan"
  | "doctor";

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

export interface MissionControlWorkerFitRecommendation {
  workerSlug: string;
  featureId?: string;
  featureTitle?: string;
  reason: string;
  fallbackReason?: string;
}

export interface MissionControlConfigWorkerChoice {
  slug: string;
  label: string;
  availability: MissionControlWorkerHealthStatus;
  availabilityDetail: string;
  summary: string;
  bestFor: string;
  tradeoffs: string;
  recommendation: MissionControlWorkerFitRecommendation;
}

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
      workerChoices?: readonly MissionControlConfigWorkerChoice[];
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

export interface MissionControlRuntimeProcessRow {
  featureId: string;
  title: string;
  milestoneTitle?: string;
  profile?: MilestoneProfile;
  status: FeatureStatus;
  workerType: string;
  hasReport: boolean;
  isLive: boolean;
  runtimeState?: RuntimeState;
  lastSeenAgeMs?: number;
  failureReason?: string;
  retryCount?: number;
  agent?: string;
  sessionId?: string;
  transport?: TransportType;
  currentActivity?: string;
  lastOutputAgeMs?: number;
  leaseRemainingMs?: number;
  outputLines?: readonly {
    timestamp: string;
    kind: "status" | "stdout" | "stderr";
    text: string;
  }[];
}

export interface MissionControlHomeState {
  headline: string;
  summary: string;
  locationLabel: string;
  checks: readonly DoctorCheck[];
  actions: readonly MissionControlHomeAction[];
  pendingHandoffs: readonly MissionControlHomeHandoff[];
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
  activeWorker: MissionControlWorkerPane | null;
  session: MissionControlSessionSidebar | null;
  pendingHandoffs: readonly MissionControlHomeHandoff[];
  configSummary: MissionControlConfigSummary | null;
  configInspector?: MissionControlConfigInspector | null;
  workerHealth?: readonly MissionControlWorkerHealthRow[];
  runtimeProcesses: readonly MissionControlRuntimeProcessRow[];
  progressLog: readonly MissionControlEvent[];

  // Milestones (for grouping)
  milestones: readonly MissionControlMilestoneRow[];

  // Gate state
  gateBlocked?: boolean;
  gateLabel?: string | null;

  // Footer state
  canPause: boolean;
  canResume: boolean;

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
  workerType: string;
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
  workerType: string;
  description: string;
  preconditions: string | undefined;
  expectedBehavior: string | undefined;
  verificationSteps: readonly string[];
  dependsOn: readonly string[];
  blockedBy?: readonly BlockedByRef[];
  unblocks?: readonly UnblocksRef[];
  fulfills: readonly string[];
  validTransitions: readonly FeatureStatus[];
  runtimeState?: RuntimeState;
  lastSeenAgeMs?: number;
  failureReason?: string;
  retryCount?: number;
  agent?: string;
  sessionId?: string;
}

export type MissionControlFeatureDetail = TaskPreviewPane;

export interface MissionControlWorkerPane {
  featureId: string;
  featureTitle: string;
  workerType: string;
  status: FeatureStatus;
  elapsedMs: number;
  report: WorkerReport | null;
  runtimeState?: RuntimeState;
  lastSeenAgeMs?: number;
  failureReason?: string;
  retryCount?: number;
  agent?: string;
  sessionId?: string;
  transport?: TransportType;
  currentActivity?: string;
  lastOutputAgeMs?: number;
}

export interface MissionControlEvent {
  timestamp: string;
  relativeMs: number;
  kind: "mission" | "feature" | "milestone" | "assertion" | "checkpoint" | "worker";
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
