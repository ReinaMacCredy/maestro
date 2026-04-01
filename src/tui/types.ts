/**
 * TUI data model -- read-only DTOs for the mission control dashboard.
 */
import type {
  MissionStatus,
  MilestoneStatus,
  FeatureStatus,
  WorkerReport,
} from "../domain/mission-types.js";
import type { DoctorCheck } from "../domain/types.js";
import type { RuntimeState } from "../domain/runtime-types.js";

export type MissionControlMode = "mission" | "home";

export interface MissionControlHomeAction {
  label: string;
  command: string;
  detail: string;
}

export interface MissionControlHomeHandoff {
  id: string;
  message: string;
  agent: string;
}

export interface MissionControlSessionSidebar {
  branch: string;
  workingTreeClean: boolean;
  diffStat: string;
  changedFiles: readonly string[];
}

export interface MissionControlConfigSummary {
  configSource: "project" | "global" | "none";
  cassAvailable: boolean;
  gitAvailable: boolean;
  checks: readonly DoctorCheck[];
  missionDirectory: string | null;
  workerTypes: readonly string[];
}

export interface MissionControlRuntimeProcessRow {
  featureId: string;
  title: string;
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
}

export interface MissionControlHomeState {
  headline: string;
  summary: string;
  locationLabel: string;
  checks: readonly DoctorCheck[];
  actions: readonly MissionControlHomeAction[];
  pendingHandoffs: readonly MissionControlHomeHandoff[];
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

  // Left pane (active feature)
  activeFeature: MissionControlFeatureDetail | null;

  // Right pane (feature list)
  features: readonly MissionControlFeatureRow[];

  // Lower pane
  activeWorker: MissionControlWorkerPane | null;
  session: MissionControlSessionSidebar | null;
  pendingHandoffs: readonly MissionControlHomeHandoff[];
  configSummary: MissionControlConfigSummary | null;
  runtimeProcesses: readonly MissionControlRuntimeProcessRow[];
  progressLog: readonly MissionControlEvent[];

  // Milestones (for grouping)
  milestones: readonly MissionControlMilestoneRow[];

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
}

export interface MissionControlFeatureRow {
  id: string;
  title: string;
  status: FeatureStatus;
  milestoneId: string;
  workerType: string;
  hasReport: boolean;
}

export interface MissionControlFeatureDetail {
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
  fulfills: readonly string[];
  validTransitions: readonly FeatureStatus[];
  runtimeState?: RuntimeState;
  lastSeenAgeMs?: number;
  failureReason?: string;
  retryCount?: number;
  agent?: string;
  sessionId?: string;
}

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
